//! The effective options of each model, from the folder `mergus.toml` files, with the origin
//! of every value (MG-REQ-0332 to MG-REQ-0335, MG-REQ-0367).

use std::collections::BTreeMap;
use std::path::Path;

use mergus_ports::FileSystem;
use serde_json::{Map, Value};

use super::convert::Segment;
use super::folder::{ALL, FolderFile, parent, read_folders};
use crate::{Layout, LayoutError, Model, Publication};

/// Where the effective value of an option comes from (MG-REQ-0367).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Origin {
    /// A folder `mergus.toml`.
    File {
        /// The path of the file, relative to the project root, separated by `/`.
        path: String,
        /// The line of the key, from 1.
        line: usize,
    },
    /// The folder of the model: `publish` of MG-REQ-0304.
    Convention,
    /// The default of the option.
    Default,
}

/// One value and its origin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Setting {
    /// The value, as JSON; `null` for an option that has no value.
    pub value: Value,
    /// Where it comes from.
    pub origin: Origin,
}

/// The effective value of one option.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effective {
    /// An option taken whole from the most specific level that sets it.
    Single(Setting),
    /// A list option, `checks` or `tags`: the union of every level, one origin per item
    /// (MG-REQ-0333). It is empty, its default, when no level sets it.
    List(Vec<Setting>),
}

/// The effective options of one model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelOptions {
    /// The model name.
    pub name: String,
    /// The path of its file, relative to the project root.
    pub path: String,
    /// Its publication: the effective `publish`, which overrides the folder rule.
    pub publication: Publication,
    /// Every option of the phase, by name.
    pub options: BTreeMap<String, Effective>,
}

/// How the effective value of an option is found.
enum Rule {
    /// The first level that sets it, or this default.
    First(fn() -> Value),
    /// The union of every level.
    Union,
    /// The first level that sets it, or the folder convention.
    Convention,
}

/// The options of the phase and their defaults (section 1, MG-REQ-0366).
const OPTIONS: [(&str, Rule); 15] = [
    ("by", Rule::First(|| Value::Null)),
    ("checks", Rule::Union),
    ("deletes", Rule::First(|| Value::from("ignore"))),
    ("fresh", Rule::First(|| Value::Null)),
    ("inherit", Rule::First(|| Value::Bool(true))),
    ("key", Rule::First(|| Value::Null)),
    ("lookback", Rule::First(|| Value::from(0))),
    ("materialize", Rule::First(|| Value::Bool(false))),
    ("nondeterministic", Rule::First(|| Value::Array(Vec::new()))),
    ("output", Rule::First(|| Value::Null)),
    ("owner", Rule::First(|| Value::Null)),
    ("partition_by", Rule::First(|| Value::Null)),
    ("publish", Rule::Convention),
    ("refresh", Rule::First(|| Value::from("full"))),
    ("tags", Rule::Union),
];

/// Reads the folder `mergus.toml` files of the project at `root` and returns the effective
/// options of every model of `layout`, in its order.
///
/// # Errors
///
/// Fails with [`LayoutError::Port`] when a file cannot be read, [`LayoutError::Schema`] when
/// the bundled folder schema is broken, and [`LayoutError::Invalid`] with every problem
/// found, file by file in walking order, each file's problems ordered by line, code and key.
pub fn read_model_options(
    files: &dyn FileSystem,
    root: &Path,
    layout: &Layout,
) -> Result<Vec<ModelOptions>, LayoutError> {
    let mut problems = Vec::new();
    let folders = read_folders(files, root, layout, &mut problems)?;
    if !problems.is_empty() {
        return Err(LayoutError::Invalid(problems));
    }
    let by_folder: BTreeMap<&str, &FolderFile> = folders
        .iter()
        .map(|file| (file.folder.as_str(), file))
        .collect();
    Ok(layout
        .models
        .iter()
        .map(|model| resolve(model, &by_folder))
        .collect())
}

/// One table of a folder file that may set options of a model.
struct Level<'a> {
    file: &'a FolderFile,
    section: &'a str,
    table: &'a Map<String, Value>,
}

impl Level<'_> {
    /// The origin of `option` at this level: the file and the line of its key.
    fn origin(&self, option: &str) -> Origin {
        let at = [
            Segment::Key(self.section.to_owned()),
            Segment::Key(option.to_owned()),
        ];
        Origin::File {
            path: self.file.path.clone(),
            line: self
                .file
                .converted
                .lines
                .get(&at[..])
                .map_or(1, |lines| lines.key),
        }
    }
}

/// The effective options of `model`.
fn resolve(model: &Model, folders: &BTreeMap<&str, &FolderFile>) -> ModelOptions {
    let levels = levels(model, folders);
    let options: BTreeMap<String, Effective> = OPTIONS
        .iter()
        .map(|(option, rule)| {
            (
                (*option).to_owned(),
                effective(model, &levels, option, rule),
            )
        })
        .collect();
    let publication = match &options.get("publish") {
        Some(Effective::Single(setting)) => publication(&setting.value),
        _ => model.publication.clone(),
    };
    ModelOptions {
        name: model.name.clone(),
        path: model.path.clone(),
        publication,
        options,
    }
}

/// The levels of `model`, most specific first (MG-REQ-0332): its section, `[all]` of its
/// folder, then `[all]` of each parent folder, nearest first, unless the section says
/// `inherit = false` (MG-REQ-0334).
fn levels<'a>(model: &'a Model, folders: &BTreeMap<&str, &'a FolderFile>) -> Vec<Level<'a>> {
    let own = parent(&model.path);
    let mut levels: Vec<Level<'a>> = Vec::new();
    let mut inherit = true;
    if let Some(level) = level(folders, own, &model.name) {
        inherit = level.table.get("inherit") != Some(&Value::Bool(false));
        levels.push(level);
    }
    let mut folder = own;
    loop {
        levels.extend(level(folders, folder, ALL));
        if !inherit || folder == "models" || folder.is_empty() {
            break;
        }
        folder = parent(folder);
    }
    levels
}

/// The table `section` of the file of `folder`, when there is one.
fn level<'a>(
    folders: &BTreeMap<&str, &'a FolderFile>,
    folder: &str,
    section: &'a str,
) -> Option<Level<'a>> {
    let file = *folders.get(folder)?;
    let (section, value) = file.converted.value.as_object()?.get_key_value(section)?;
    Some(Level {
        file,
        section,
        table: value.as_object()?,
    })
}

/// The effective value of `option` for `model` under `rule`.
fn effective(model: &Model, levels: &[Level<'_>], option: &str, rule: &Rule) -> Effective {
    let first = levels.iter().find_map(|level| {
        level.table.get(option).map(|value| Setting {
            value: value.clone(),
            origin: level.origin(option),
        })
    });
    match rule {
        Rule::Union => Effective::List(union(levels, option)),
        Rule::First(default) => Effective::Single(first.unwrap_or_else(|| Setting {
            value: default(),
            origin: Origin::Default,
        })),
        Rule::Convention => Effective::Single(first.unwrap_or_else(|| {
            Setting {
                value: model
                    .published_name()
                    .map_or(Value::Bool(false), Value::String),
                origin: Origin::Convention,
            }
        })),
    }
}

/// The items of the list `option` of every level, in precedence order, without duplicates
/// (MG-REQ-0333).
fn union(levels: &[Level<'_>], option: &str) -> Vec<Setting> {
    let mut items: Vec<Setting> = Vec::new();
    for level in levels {
        let values = level.table.get(option).and_then(Value::as_array);
        for value in values.into_iter().flatten() {
            if !items.iter().any(|item| item.value == *value) {
                items.push(Setting {
                    value: value.clone(),
                    origin: level.origin(option),
                });
            }
        }
    }
    items
}

/// The publication a valid `publish` value gives: `gold.<schema>.<table>` or `false`.
fn publication(value: &Value) -> Publication {
    let parts = value.as_str().and_then(|name| {
        let mut parts = name.splitn(3, '.').skip(1);
        Some((parts.next()?.to_owned(), parts.next()?.to_owned()))
    });
    parts.map_or(Publication::Intermediate, |(schema, table)| {
        Publication::Gold { schema, table }
    })
}
