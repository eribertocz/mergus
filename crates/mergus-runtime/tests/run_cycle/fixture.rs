//! A small project of three `.sql` models in the simulated file system, run on the real
//! DuckDB in memory: `stg_orders`, then `orders_big` and `orders_count`, which read it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use mergus_plan::{Cleared, Gate, GateCheck, GateInput, Plan, PlanRequest, evaluate};
use mergus_ports::FileSystem;
use mergus_ports::real::{RealDatabase, RealSqlParser};
use mergus_runtime::{
    ModelWork, ReadPorts, ReadProject, SqlInput, SqlWork, StepContext, StepFailure, Work,
    plan_project, read_project,
};
use mergus_sim::SimFileSystem;

pub const ROOT: &str = "/p";
pub const STG: &str = "SELECT 1 AS id UNION ALL SELECT 2 AS id UNION ALL SELECT 3 AS id";
pub const BIG: &str = "SELECT id FROM stg_orders WHERE id > 1";
pub const COUNT: &str = "SELECT count(*) AS n FROM stg_orders";

#[cfg(test)]
pub fn root() -> &'static Path {
    Path::new(ROOT)
}

/// A file system holding `files`, each relative to the root.
#[cfg(test)]
pub fn project(files: &[(&str, &str)]) -> SimFileSystem {
    let system = SimFileSystem::new();
    system.create_dir_all(root()).unwrap();
    for (path, text) in files {
        let path = root().join(path);
        system.create_dir_all(path.parent().unwrap()).unwrap();
        system.write(&path, text.as_bytes()).unwrap();
    }
    system
}

/// The three models, with `count` as the SQL of `orders_count`.
#[cfg(test)]
pub fn orders(count: &str) -> SimFileSystem {
    project(&[
        ("project.toml", "name = \"sales\"\n"),
        ("models/stg_orders.sql", STG),
        ("models/orders_big.sql", BIG),
        ("models/orders_count.sql", count),
    ])
}

#[cfg(test)]
pub fn read(files: &dyn FileSystem) -> ReadProject {
    let parser = RealSqlParser::new();
    let ports = ReadPorts {
        files,
        parser: &parser,
        python: None,
        database: None,
    };
    read_project(&ports, root()).unwrap()
}

/// The plan of every model of `project`, at most `max_parallel` at once.
#[cfg(test)]
pub fn plan(project: &ReadProject, available: &BTreeSet<String>, max_parallel: u32) -> Plan {
    let names = project
        .models
        .iter()
        .map(|model| model.options.name.clone())
        .collect();
    let mut request = PlanRequest::new("0.4.0", "dev", names);
    request.max_parallel = max_parallel.try_into().unwrap();
    plan_project(project, &request, available).unwrap()
}

/// Stage 4 with every gate but credentials waiting for a later phase.
#[cfg(test)]
pub fn cleared() -> Cleared {
    let checks = Gate::ALL
        .into_iter()
        .filter(|gate| *gate != Gate::Credentials)
        .map(|gate| {
            let phase = "9.0".to_owned();
            (gate, GateCheck::NotApplicable { phase })
        })
        .collect();
    let input = GateInput {
        checks,
        required: Vec::new(),
        existence: BTreeMap::new(),
    };
    evaluate(&input).unwrap().clear().unwrap()
}

#[cfg(test)]
fn schema(name: &str, data_type: DataType) -> SchemaRef {
    Arc::new(Schema::new(vec![Field::new(name, data_type, true)]))
}

/// What the works of a run did, shared by them.
#[cfg(test)]
#[derive(Clone, Default)]
pub struct Log {
    pub events: Arc<Mutex<Vec<String>>>,
    running: Arc<AtomicUsize>,
    pub most_at_once: Arc<AtomicUsize>,
}

#[cfg(test)]
impl Log {
    pub fn push(&self, event: String) {
        self.events.lock().unwrap().push(event);
    }

    pub fn events(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }
}

/// A work that logs its start and end, and cancels the run after it ran when `cancels`.
#[cfg(test)]
pub struct Logged<'a> {
    inner: SqlWork<'a>,
    log: Log,
    cancels: bool,
}

#[cfg(test)]
impl ModelWork for Logged<'_> {
    fn run(&self, context: &StepContext<'_>) -> Result<(), StepFailure> {
        let now = self.log.running.fetch_add(1, Ordering::SeqCst) + 1;
        self.log.most_at_once.fetch_max(now, Ordering::SeqCst);
        self.log.push(format!("start {}", context.model));
        let result = self.inner.run(context);
        self.log.push(format!("end {}", context.model));
        self.log.running.fetch_sub(1, Ordering::SeqCst);
        if self.cancels {
            context.cancel.cancel();
        }
        result
    }
}

/// The DuckDB work of each model of the fixture, logged; the model `cancelling` cancels the
/// run once it ran.
#[cfg(test)]
pub fn models<'a>(
    files: &'a dyn FileSystem,
    database: &'a RealDatabase,
    project: &ReadProject,
    log: &Log,
    cancelling: Option<&str>,
) -> Work<'a> {
    let stg = SqlInput {
        name: "stg_orders".to_owned(),
        schema: schema("id", DataType::Int32),
        boundary: None,
    };
    let mut models: BTreeMap<String, Box<dyn ModelWork + 'a>> = BTreeMap::new();
    for model in &project.models {
        let name = model.options.name.as_str();
        let (inputs, output) = match name {
            "stg_orders" => (Vec::new(), schema("id", DataType::Int32)),
            "orders_big" => (vec![stg.clone()], schema("id", DataType::Int32)),
            _ => (vec![stg.clone()], schema("n", DataType::Int64)),
        };
        let inner = SqlWork {
            database,
            files,
            sql: model.code.clone(),
            params: Vec::new(),
            macros: Vec::new(),
            inputs,
            output,
        };
        let logged = Logged {
            inner,
            log: log.clone(),
            cancels: cancelling == Some(name),
        };
        models.insert(name.to_owned(), Box::new(logged));
    }
    Work {
        extractions: Vec::new(),
        models,
    }
}

/// A real database for in-memory sessions; its extension cache is never used.
#[cfg(test)]
pub fn database() -> RealDatabase {
    RealDatabase::new(&PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("mergus-runtime-cache"))
}

/// Every path under `dir`, files and directories, sorted.
#[cfg(test)]
pub fn tree(files: &dyn FileSystem, dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        let Ok(entries) = files.list_dir(&next) else {
            continue;
        };
        for entry in entries {
            let shown = entry.strip_prefix(root()).unwrap().display().to_string();
            found.push(shown);
            pending.push(entry);
        }
    }
    found.sort();
    found
}

/// The fingerprint of `model` in `plan`.
#[cfg(test)]
pub fn fingerprint(plan: &Plan, model: &str) -> String {
    plan.document()
        .steps
        .iter()
        .find(|step| step.model == model)
        .unwrap()
        .fingerprint
        .clone()
}
