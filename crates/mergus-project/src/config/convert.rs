//! TOML 1.0 to JSON (MG-REQ-0365), keeping the line of every key and value, the literal
//! check of MG-REQ-0364 and the literal-secret check of MG-REQ-0320.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde_json::{Map, Number, Value};
use toml::Spanned;
use toml::de::{DeTable, DeValue};
use toml::value::{Datetime, Offset};

use super::secret::is_literal_secret;
use crate::ProjectError;

/// One step from a value to one of its children.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Segment {
    /// A key of a table.
    Key(String),
    /// An index of an array.
    Index(usize),
}

/// Writes a path as a dotted key, indexes in brackets: `gold.location`, `hosts[1]`.
pub(crate) fn dotted(path: &[Segment]) -> String {
    let mut out = String::new();
    for segment in path {
        match segment {
            Segment::Key(key) => {
                if !out.is_empty() {
                    out.push('.');
                }
                out.push_str(key);
            }
            Segment::Index(index) => {
                let _ = write!(out, "[{index}]");
            }
        }
    }
    out
}

/// The last key of a path: the key of an array item is the key of its array (MG-REQ-0363).
fn last_key(path: &[Segment]) -> Option<&str> {
    path.iter().rev().find_map(|segment| match segment {
        Segment::Key(key) => Some(key.as_str()),
        Segment::Index(_) => None,
    })
}

/// The lines of a key and of its value, from 1.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Lines {
    /// The line of the key, or of the value for an array item.
    pub(crate) key: usize,
    /// The line of the value.
    pub(crate) value: usize,
}

/// A configuration file read as JSON.
pub(crate) struct Converted {
    /// The JSON of the file.
    pub(crate) value: Value,
    /// The lines of every key and value, by path.
    pub(crate) lines: BTreeMap<Vec<Segment>, Lines>,
}

/// Reads `text` as TOML 1.0 and maps it to JSON. Problems are pushed to `problems`; `None`
/// means the text is not TOML 1.0.
pub(crate) fn convert(
    path: &str,
    text: &str,
    problems: &mut Vec<ProjectError>,
) -> Option<Converted> {
    let starts = LineStarts::new(text);
    let root = match DeTable::parse(text) {
        Ok(root) => root,
        Err(error) => {
            let line = error.span().map_or(1, |span| starts.line(span.start));
            problems.push(ProjectError::InvalidToml {
                path: path.to_owned(),
                line,
                message: error.message().to_owned(),
            });
            return None;
        }
    };
    let mut walk = Walk {
        path,
        starts,
        lines: BTreeMap::new(),
        problems,
        failed: false,
    };
    let value = walk.table(root.get_ref(), &mut Vec::new());
    (!walk.failed).then_some(Converted {
        value,
        lines: walk.lines,
    })
}

/// The byte offsets where each line starts.
struct LineStarts(Vec<usize>);

impl LineStarts {
    fn new(text: &str) -> Self {
        let breaks = text.match_indices('\n').map(|(offset, _)| offset + 1);
        Self(std::iter::once(0).chain(breaks).collect())
    }

    fn line(&self, offset: usize) -> usize {
        self.0.partition_point(|start| *start <= offset)
    }
}

struct Walk<'a> {
    path: &'a str,
    starts: LineStarts,
    lines: BTreeMap<Vec<Segment>, Lines>,
    problems: &'a mut Vec<ProjectError>,
    failed: bool,
}

impl Walk<'_> {
    fn table(&mut self, table: &DeTable<'_>, at: &mut Vec<Segment>) -> Value {
        let mut object = Map::new();
        for (key, value) in table {
            at.push(Segment::Key(key.get_ref().to_string()));
            let key_line = self.starts.line(key.span().start);
            let converted = self.value(value, key_line, at);
            object.insert(key.get_ref().to_string(), converted);
            at.pop();
        }
        Value::Object(object)
    }

    fn value(
        &mut self,
        value: &Spanned<DeValue<'_>>,
        key_line: usize,
        at: &mut Vec<Segment>,
    ) -> Value {
        let line = self.starts.line(value.span().start);
        self.lines.insert(
            at.clone(),
            Lines {
                key: key_line,
                value: line,
            },
        );
        match value.get_ref() {
            DeValue::String(text) => {
                self.string(text, line, at);
                Value::String(text.to_string())
            }
            DeValue::Integer(integer) => self.integer(integer.as_str(), integer.radix(), line),
            DeValue::Float(float) => self.float(float.as_str(), line, at),
            DeValue::Boolean(flag) => Value::Bool(*flag),
            DeValue::Datetime(datetime) => Value::String(rfc3339(datetime)),
            DeValue::Array(items) => {
                let mut array = Vec::with_capacity(items.len());
                for (index, item) in items.iter().enumerate() {
                    at.push(Segment::Index(index));
                    let item_line = self.starts.line(item.span().start);
                    array.push(self.value(item, item_line, at));
                    at.pop();
                }
                Value::Array(array)
            }
            DeValue::Table(table) => self.table(table, at),
        }
    }

    /// The checks of a string value: no expression (MG-REQ-0364), no literal secret
    /// (MG-REQ-0320).
    fn string(&mut self, text: &str, line: usize, at: &[Segment]) {
        if ["${", "{{", "{%"]
            .iter()
            .any(|marker| text.contains(marker))
        {
            self.problems.push(ProjectError::Expression {
                path: self.path.to_owned(),
                line,
                key: dotted(at),
            });
        }
        if is_literal_secret(last_key(at), text) {
            self.problems.push(ProjectError::LiteralSecret {
                path: self.path.to_owned(),
                line,
                key: dotted(at),
            });
        }
    }

    fn integer(&mut self, digits: &str, radix: u32, line: usize) -> Value {
        if let Ok(integer) = i64::from_str_radix(digits, radix) {
            return Value::Number(integer.into());
        }
        self.failed = true;
        self.problems.push(ProjectError::InvalidToml {
            path: self.path.to_owned(),
            line,
            message: "integer out of the 64-bit range".to_owned(),
        });
        Value::Null
    }

    fn float(&mut self, text: &str, line: usize, at: &[Segment]) -> Value {
        let number = text.parse::<f64>().ok().and_then(Number::from_f64);
        number.map_or_else(
            || {
                self.failed = true;
                self.problems.push(ProjectError::NonFiniteFloat {
                    path: self.path.to_owned(),
                    line,
                    key: dotted(at),
                });
                Value::Null
            },
            Value::Number,
        )
    }
}

/// Writes a TOML date or time in the RFC 3339 form of MG-REQ-0365.
fn rfc3339(datetime: &Datetime) -> String {
    let mut out = String::new();
    if let Some(date) = datetime.date {
        let _ = write!(out, "{:04}-{:02}-{:02}", date.year, date.month, date.day);
    }
    if let Some(time) = datetime.time {
        if datetime.date.is_some() {
            out.push('T');
        }
        let _ = write!(
            out,
            "{:02}:{:02}:{:02}",
            time.hour, time.minute, time.second
        );
        if time.nanosecond > 0 {
            let fraction = format!("{:09}", time.nanosecond);
            out.push('.');
            out.push_str(fraction.trim_end_matches('0'));
        }
    }
    match datetime.offset {
        None => {}
        Some(Offset::Z | Offset::Custom { minutes: 0 }) => out.push('Z'),
        Some(Offset::Custom { minutes }) => {
            let sign = if minutes < 0 { '-' } else { '+' };
            let minutes = minutes.unsigned_abs();
            let _ = write!(out, "{sign}{:02}:{:02}", minutes / 60, minutes % 60);
        }
    }
    out
}
