//! Steps return recipes, never data (MG-REQ-0616).

use std::collections::BTreeSet;

use mergus_ports::{PythonSegment, PythonTree};

use super::to_usize;
use crate::PythonModelError;

/// The methods that make data eager (MG-REQ-0616.1).
const EAGER_METHODS: [&str; 6] = [
    "collect",
    "collect_async",
    "fetch",
    "to_pandas",
    "to_arrow",
    "to_dicts",
];

/// The module whose `DataFrame` is eager data.
const POLARS: &str = "polars";

/// Adds to `problems` every eager call of `tree`, anywhere in the file, in the order of its
/// calls (MG-REQ-0616.1).
pub(super) fn check(path: &str, tree: &PythonTree, problems: &mut Vec<PythonModelError>) {
    let polars = polars_names(tree);
    for call in &tree.calls {
        let Some(name) = call.name.as_deref() else {
            continue;
        };
        let written = match call.receiver.as_slice() {
            [] => continue,
            [PythonSegment::Name { name: root }]
                if name == "DataFrame" && polars.contains(root.as_str()) =>
            {
                format!("{root}.DataFrame()")
            }
            _ if EAGER_METHODS.contains(&name) => format!(".{name}()"),
            _ => continue,
        };
        problems.push(PythonModelError::EagerCall {
            path: path.to_owned(),
            line: to_usize(call.line),
            column: to_usize(call.column),
            call: written,
        });
    }
}

/// The names `polars` is imported as: `pl` in `import polars as pl`, `polars` in
/// `import polars` or `import polars.<module>`.
fn polars_names(tree: &PythonTree) -> BTreeSet<&str> {
    tree.imports
        .iter()
        .filter(|import| import.name.is_none() && import.level == 0)
        .filter_map(|import| {
            let module = import.module.as_deref()?;
            match import.alias.as_deref() {
                Some(alias) => (module == POLARS).then_some(alias),
                None => (module == POLARS || module.starts_with("polars.")).then_some(POLARS),
            }
        })
        .collect()
}
