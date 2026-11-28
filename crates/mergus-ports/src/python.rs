//! The `Python` port: reads a Python file's syntax tree without importing it (MG-REQ-0611).
//!
//! The tree is a JSON document, version [`PYTHON_TREE_VERSION`]. Lines and columns start at
//! 1, and a column counts characters. A file that parses gives:
//!
//! ```json
//! {"version": 1,
//!  "functions": [{"name": "f", "async": false, "line": 1, "column": 1,
//!                 "parameters": [{"name": "a", "kind": "positional", "line": 1, "column": 7}],
//!                 "returns": [{"kind": "string", "value": "SELECT 1", "line": 2, "column": 5}]}],
//!  "imports": [{"module": "polars", "name": null, "alias": "pl", "level": 0,
//!               "line": 3, "column": 1}],
//!  "calls": [{"name": "first", "keywords": ["keep"], "line": 4, "column": 1,
//!             "receiver": [{"kind": "name", "name": "df"}, {"kind": "attribute", "name": "sort"},
//!                          {"kind": "call"}]}]}
//! ```
//!
//! A file that does not parse gives `{"version": 1, "error": {"message": …, "line": …,
//! "column": …}}`, where line and column may be `null`. [`PythonTree`] and the types it holds
//! document each field.
//!
//! [`Python::run`] runs a Python program in a child process, with arguments and a working
//! directory, and returns its [`ProcessOutput`], the
//! output type of the `Process` port.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{PortError, ProcessOutput};

/// The version of the tree document this crate reads.
pub const PYTHON_TREE_VERSION: u32 = 1;

/// Reads Python files without importing or executing them, and runs Python programs in a
/// child process.
pub trait Python: Send + Sync {
    /// Returns the tree of the Python file whose text is `source`. The file is parsed only:
    /// it is never imported or executed, so nothing it imports runs.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::PythonSyntax`] when `source` is not valid Python, with
    /// [`PortError::TimedOut`] when the interpreter does not answer in time, and with
    /// [`PortError::Failed`] when it cannot run, cannot read a valid file that is too deep or
    /// too large, or answers with something that is not a tree.
    fn parse(&self, source: &str) -> Result<PythonTree, PortError>;

    /// Runs the Python program `script` with the arguments `args`, in the directory
    /// `current_dir`, and returns what it printed and how it exited. The child gets the
    /// environment the interpreter was built with and nothing else. Its standard input is
    /// empty. There is no time limit.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::NotFound`] when the interpreter or `current_dir` does not exist,
    /// and with [`PortError::Failed`] when the child cannot be started or its output cannot
    /// be read. A child that exits with a failure is not an error: its [`ProcessOutput`]
    /// says so.
    fn run(
        &self,
        script: &str,
        args: &[String],
        current_dir: &Path,
    ) -> Result<ProcessOutput, PortError>;
}

/// The tree of a Python file.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PythonTree {
    /// The functions defined at the top level of the file, `def` and `async def`, in order.
    pub functions: Vec<PythonFunction>,
    /// Every import of the file, nested ones included, one per imported name, in order.
    pub imports: Vec<PythonImport>,
    /// Every call of the file, in the order of the tree: a call comes before the calls inside
    /// it, so `df.sort("a").first()` gives `first`, then `sort`.
    pub calls: Vec<PythonCall>,
}

impl PythonTree {
    /// Reads the tree document `json`, as the interpreter prints it.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::PythonSyntax`] when the document reports a syntax error, and
    /// with [`PortError::Failed`] when it is not a tree document of version
    /// [`PYTHON_TREE_VERSION`].
    pub fn from_json(json: &str) -> Result<Self, PortError> {
        let document: Document = serde_json::from_str(json)
            .map_err(|error| PortError::Failed(format!("not a Python tree: {error}")))?;
        if document.version != PYTHON_TREE_VERSION {
            return Err(PortError::Failed(format!(
                "Python tree version {} is not {PYTHON_TREE_VERSION}",
                document.version
            )));
        }
        match document.error {
            Some(error) => Err(PortError::PythonSyntax {
                message: error.message,
                line: error.line,
                column: error.column,
            }),
            None => Ok(Self {
                functions: document.functions,
                imports: document.imports,
                calls: document.calls,
            }),
        }
    }
}

/// A function defined at the top level of the file.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PythonFunction {
    /// Its name.
    pub name: String,
    /// `true` for `async def`.
    #[serde(rename = "async")]
    pub is_async: bool,
    /// Its parameters, in order.
    pub parameters: Vec<PythonParameter>,
    /// Its `return` statements, in order, without those of the functions, classes and lambdas
    /// nested in it.
    pub returns: Vec<PythonReturn>,
    /// The line of `def`.
    pub line: u32,
    /// The column of `def`.
    pub column: u32,
}

/// A parameter of a function.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PythonParameter {
    /// Its name, without `*` or `**`.
    pub name: String,
    /// How it is passed.
    pub kind: PythonParameterKind,
    /// Its line.
    pub line: u32,
    /// Its column.
    pub column: u32,
}

/// How a parameter is passed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PythonParameterKind {
    /// Before `/`.
    PositionalOnly,
    /// By position or by name.
    Positional,
    /// `*args`.
    VarPositional,
    /// After `*` or `*args`.
    KeywordOnly,
    /// `**kwargs`.
    VarKeyword,
}

/// A `return` statement and the kind of what it returns. (`deny_unknown_fields` does not
/// work with `flatten`; the kinds of [`PythonReturned`] name their fields.)
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct PythonReturn {
    /// What it returns.
    #[serde(flatten)]
    pub value: PythonReturned,
    /// The line of `return`.
    pub line: u32,
    /// The column of `return`.
    pub column: u32,
}

/// What a `return` statement returns.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PythonReturned {
    /// A string literal, such as literal SQL; `value` is its text.
    String {
        /// The text of the literal.
        value: String,
    },
    /// Nothing: a bare `return`, or `return None`.
    None,
    /// Any other expression; `node` is its `ast` class name, such as `Call` or `JoinedStr`.
    Expression {
        /// The `ast` class name of the expression.
        node: String,
    },
}

/// One imported name.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PythonImport {
    /// The module: `a.b` in `import a.b` and `from a.b import c`; `None` in `from . import c`.
    pub module: Option<String>,
    /// The name taken from the module: `c` in `from a.b import c`; `None` for `import a.b`.
    pub name: Option<String>,
    /// The name after `as`, if any.
    pub alias: Option<String>,
    /// The number of leading dots of a relative import; 0 for an absolute one.
    pub level: u32,
    /// The line of the import statement.
    pub line: u32,
    /// The column of the import statement.
    pub column: u32,
}

/// A call.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PythonCall {
    /// The name called: `f` in `f(x)`, the method `m` in `a.m(x)`; `None` when what is called
    /// is neither a name nor an attribute, as in `f()()`.
    pub name: Option<String>,
    /// The chain the called name is reached from, from its root: `[df, .sort, ()]` for
    /// `df.sort("a").first()`, empty for `f(x)`. When `name` is `None`, it is the whole
    /// expression that is called.
    pub receiver: Vec<PythonSegment>,
    /// The names of its keyword arguments, in order; `None` for `**mapping`.
    pub keywords: Vec<Option<String>>,
    /// Its line.
    pub line: u32,
    /// Its column.
    pub column: u32,
}

/// One step of a receiver chain.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PythonSegment {
    /// A name, the root of the chain.
    Name {
        /// The name.
        name: String,
    },
    /// `.name`.
    Attribute {
        /// The attribute.
        name: String,
    },
    /// A call of what comes before: `(…)`.
    Call,
    /// A subscript of what comes before: `[…]`.
    Subscript,
    /// A root that is not a name, such as a literal; `node` is its `ast` class name.
    Other {
        /// The `ast` class name of the expression.
        node: String,
    },
}

/// The document the interpreter prints: a tree, or a syntax error.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    #[serde(default)]
    error: Option<SyntaxError>,
    #[serde(default)]
    functions: Vec<PythonFunction>,
    #[serde(default)]
    imports: Vec<PythonImport>,
    #[serde(default)]
    calls: Vec<PythonCall>,
}

/// The syntax error of a document.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SyntaxError {
    message: String,
    line: Option<u32>,
    column: Option<u32>,
}
