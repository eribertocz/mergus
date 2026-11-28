//! The work of a `.py` model whose steps Polars runs, through `mergus-engine-polars`: each
//! step in its own fresh directory under the model's scratch directory, at the `subprocess`
//! level, with the allowlisted environment the interpreter was located with (MG-REQ-0636.1).

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use arrow::array::RecordBatch;
use arrow::datatypes::SchemaRef;
use arrow::ipc::reader::FileReader;
use arrow::ipc::writer::FileWriter;
use mergus_engine_polars::{Frame, Parameter, PolarsError, Step, execute_step, plan_step};
use mergus_ports::{FileSystem, Python};

use super::{ModelWork, StepContext, StepFailure};

/// What a parameter of a [`PolarsStep`] reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolarsSource {
    /// Another step of the same file, earlier in [`PolarsWork::steps`].
    Step(String),
    /// A model of the project, a step of the plan.
    Model(String),
}

/// One parameter of a [`PolarsStep`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolarsInput {
    /// The parameter's name; the input is passed as this keyword argument.
    pub parameter: String,
    /// What it reads.
    pub source: PolarsSource,
}

/// One Polars step of a `.py` model: a function of its file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolarsStep {
    /// The function's name.
    pub name: String,
    /// Its parameters, in order.
    pub inputs: Vec<PolarsInput>,
}

/// A `.py` model of Polars steps, run by [`execute_step`] one step at a time.
pub struct PolarsWork<'a> {
    /// The interpreter, located once per run with the allowlisted environment.
    pub python: &'a dyn Python,
    /// The file system of the steps' files.
    pub files: &'a dyn FileSystem,
    /// The model.
    pub model: String,
    /// The model's Python file, as an absolute path.
    pub module: PathBuf,
    /// The project's `lib` directory, as an absolute path.
    pub lib: PathBuf,
    /// The steps the model's result reads, directly or through other steps, each after the
    /// steps it reads; the last one is the result.
    pub steps: Vec<PolarsStep>,
    /// The schema of each model it reads, by name.
    pub schemas: BTreeMap<String, SchemaRef>,
}

impl PolarsWork<'_> {
    /// Plans every step against the schemas of its inputs, in fresh directories under
    /// `work_dir`, and returns the schema of the result (MG-REQ-0617.2).
    ///
    /// # Errors
    ///
    /// Fails with the [`StepFailure`] of the engine, or of a step read before it is planned.
    pub fn describe(&self, work_dir: &Path) -> Result<SchemaRef, StepFailure> {
        let mut planned: BTreeMap<&str, SchemaRef> = BTreeMap::new();
        let mut result = None;
        for step in &self.steps {
            let mut parameters = Vec::with_capacity(step.inputs.len());
            for input in &step.inputs {
                let schema = match &input.source {
                    PolarsSource::Step(name) => planned.get(name.as_str()).cloned(),
                    PolarsSource::Model(name) => self.schemas.get(name).cloned(),
                };
                parameters.push(Parameter {
                    name: input.parameter.clone(),
                    schema: schema.ok_or_else(|| self.missing(input))?,
                });
            }
            let schema = plan_step(
                self.python,
                self.files,
                &self.step(step),
                &parameters,
                work_dir,
            )
            .map_err(|error| self.failure(&error))?;
            planned.insert(&step.name, schema.clone());
            result = Some(schema);
        }
        result.ok_or_else(|| self.internal("no step".to_owned()))
    }

    fn step(&self, step: &PolarsStep) -> Step {
        Step {
            model: self.model.clone(),
            step: step.name.clone(),
            module: self.module.clone(),
            lib: self.lib.clone(),
        }
    }

    fn failure(&self, error: &PolarsError) -> StepFailure {
        StepFailure {
            name: self.model.clone(),
            code: error.code(),
            message: error.to_string(),
        }
    }

    fn internal(&self, message: String) -> StepFailure {
        StepFailure {
            name: self.model.clone(),
            code: None,
            message: format!("internal error: {message}"),
        }
    }

    fn missing(&self, input: &PolarsInput) -> StepFailure {
        let name = match &input.source {
            PolarsSource::Step(name) | PolarsSource::Model(name) => name,
        };
        self.internal(format!("no result for `{name}`"))
    }

    /// The rows of the model `name`, read from its result file in `context`.
    fn read_model(&self, context: &StepContext<'_>, name: &str) -> Result<Frame, StepFailure> {
        let path = context
            .inputs
            .get(name)
            .ok_or_else(|| self.internal(format!("no result for `{name}`")))?;
        let failed = |message: String| self.internal(format!("`{}`: {message}", path.display()));
        let stream = self
            .files
            .open(path)
            .map_err(|error| failed(error.to_string()))?;
        let reader = FileReader::try_new(stream, None).map_err(|error| failed(error.to_string()))?;
        let schema = reader.schema();
        let batches = reader
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| failed(error.to_string()))?;
        Ok(Frame {
            name: String::new(),
            schema,
            batches,
        })
    }

    /// Writes `batches` of `schema` as the Arrow IPC file `path`.
    fn write(
        &self,
        path: &Path,
        schema: &SchemaRef,
        batches: &[RecordBatch],
    ) -> Result<(), StepFailure> {
        let failed = |message: String| self.internal(format!("`{}`: {message}", path.display()));
        let stream = self
            .files
            .create(path)
            .map_err(|error| failed(error.to_string()))?;
        let mut writer =
            FileWriter::try_new(stream, schema).map_err(|error| failed(error.to_string()))?;
        for batch in batches {
            writer
                .write(batch)
                .map_err(|error| failed(error.to_string()))?;
        }
        writer.finish().map_err(|error| failed(error.to_string()))?;
        let mut stream = writer
            .into_inner()
            .map_err(|error| failed(error.to_string()))?;
        stream.flush().map_err(|error| failed(error.to_string()))
    }
}

impl ModelWork for PolarsWork<'_> {
    fn run(&self, context: &StepContext<'_>) -> Result<(), StepFailure> {
        let mut models: BTreeMap<&str, Frame> = BTreeMap::new();
        let mut done: BTreeMap<&str, Frame> = BTreeMap::new();
        for step in &self.steps {
            let mut inputs = Vec::with_capacity(step.inputs.len());
            for input in &step.inputs {
                let found = match &input.source {
                    PolarsSource::Step(name) => done.get(name.as_str()),
                    PolarsSource::Model(name) => {
                        if !models.contains_key(name.as_str()) {
                            models.insert(name, self.read_model(context, name)?);
                        }
                        models.get(name.as_str())
                    }
                };
                let found = found.ok_or_else(|| self.missing(input))?;
                inputs.push(Frame {
                    name: input.parameter.clone(),
                    schema: found.schema.clone(),
                    batches: found.batches.clone(),
                });
            }
            let output = execute_step(
                self.python,
                self.files,
                &self.step(step),
                &inputs,
                context.scratch,
            )
            .map_err(|error| self.failure(&error))?;
            done.insert(
                &step.name,
                Frame {
                    name: step.name.clone(),
                    schema: output.schema,
                    batches: output.batches,
                },
            );
        }
        let result = self
            .steps
            .last()
            .and_then(|step| done.get(step.name.as_str()))
            .ok_or_else(|| self.internal("no step".to_owned()))?;
        self.write(context.output, &result.schema, &result.batches)
    }
}
