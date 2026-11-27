//! DuckDB type names, as `DESCRIBE` writes them: their width at a boundary (MG-REQ-0633) and
//! whether they lose information from DuckDB to Polars (MG-REQ-0634.2).

/// The width counted for a type that has no fixed width: `VARCHAR`, `BLOB` and nested types
/// (MG-REQ-0633).
const VARIABLE_WIDTH: u64 = 64;

/// The widest precision of a `DECIMAL` that crosses without loss (MG-REQ-0634.2).
const MAX_DECIMAL_DIGITS: u32 = 38;

/// The scalar types the planner knows, with their fixed width in bytes, or `None` for a
/// type of variable width. `HUGEINT`, `UHUGEINT`, `BIT` and `INTERVAL` are known, and lossy.
const SCALARS: [(&str, Option<u64>); 24] = [
    ("BOOLEAN", Some(1)),
    ("TINYINT", Some(1)),
    ("UTINYINT", Some(1)),
    ("SMALLINT", Some(2)),
    ("USMALLINT", Some(2)),
    ("INTEGER", Some(4)),
    ("UINTEGER", Some(4)),
    ("BIGINT", Some(8)),
    ("UBIGINT", Some(8)),
    ("HUGEINT", Some(16)),
    ("UHUGEINT", Some(16)),
    ("FLOAT", Some(4)),
    ("DOUBLE", Some(8)),
    ("DATE", Some(4)),
    ("TIME", Some(8)),
    ("TIMESTAMP", Some(8)),
    ("TIMESTAMP_S", Some(8)),
    ("TIMESTAMP_MS", Some(8)),
    ("TIMESTAMP_NS", Some(8)),
    ("TIMESTAMP WITH TIME ZONE", Some(8)),
    ("INTERVAL", Some(16)),
    ("UUID", Some(16)),
    ("VARCHAR", None),
    ("BLOB", None),
];

/// The scalar types that lose information from DuckDB to Polars (MG-REQ-0634.2).
const LOSSY: [&str; 4] = ["HUGEINT", "UHUGEINT", "BIT", "INTERVAL"];

/// A type name, read.
enum Parsed<'a> {
    /// A scalar type, by its upper-case name.
    Scalar(String),
    /// `DECIMAL(<digits>, <scale>)`, or `DECIMAL` alone with `None`.
    Decimal(Option<u32>),
    /// A list or array of the inner type.
    List(&'a str),
    /// `STRUCT(...)` or `MAP(...)`, with the types of its members.
    Nested(Vec<&'a str>),
    /// `UNION(...)`.
    Union,
    /// `ENUM(...)`.
    Enum,
    /// A name the planner does not know.
    Unknown,
}

/// Returns the width in bytes counted for one value of `data_type` (MG-REQ-0633): its fixed
/// width, or 64 for `VARCHAR`, `BLOB`, nested, `ENUM` and unknown types.
pub(crate) fn width(data_type: &str) -> u64 {
    match parse(data_type) {
        Parsed::Scalar(name) => SCALARS
            .iter()
            .find(|(known, _)| *known == name)
            .and_then(|(_, width)| *width)
            .unwrap_or(VARIABLE_WIDTH),
        Parsed::Decimal(_) => 16,
        Parsed::List(_) | Parsed::Nested(_) | Parsed::Union | Parsed::Enum | Parsed::Unknown => {
            VARIABLE_WIDTH
        }
    }
}

/// Whether `data_type` loses information from DuckDB to Polars (MG-REQ-0634.2): `HUGEINT`,
/// `UHUGEINT`, `UNION`, `BIT`, `INTERVAL`, a `DECIMAL` of more than 38 digits, a type the
/// planner does not know, or a list, struct or map holding one of these.
pub(crate) fn lossy(data_type: &str) -> bool {
    match parse(data_type) {
        Parsed::Scalar(name) => LOSSY.contains(&name.as_str()),
        Parsed::Decimal(digits) => digits.is_some_and(|digits| digits > MAX_DECIMAL_DIGITS),
        Parsed::List(inner) => lossy(inner),
        Parsed::Nested(members) => members.into_iter().any(lossy),
        Parsed::Enum => false,
        Parsed::Union | Parsed::Unknown => true,
    }
}

/// Reads `data_type`.
fn parse(data_type: &str) -> Parsed<'_> {
    let text = data_type.trim();
    if let Some(inner) = list_element(text) {
        return Parsed::List(inner);
    }
    let Some((head, arguments)) = call(text) else {
        let name = text.to_ascii_uppercase();
        if name == "DECIMAL" {
            return Parsed::Decimal(None);
        }
        return if SCALARS.iter().any(|(known, _)| *known == name) || name == "BIT" {
            Parsed::Scalar(name)
        } else {
            Parsed::Unknown
        };
    };
    match head.to_ascii_uppercase().as_str() {
        "DECIMAL" => split(arguments)
            .first()
            .and_then(|digits| digits.parse().ok())
            .map_or(Parsed::Unknown, |digits| Parsed::Decimal(Some(digits))),
        "STRUCT" => Parsed::Nested(split(arguments).into_iter().map(member_type).collect()),
        "MAP" => Parsed::Nested(split(arguments)),
        "UNION" => Parsed::Union,
        "ENUM" => Parsed::Enum,
        _ => Parsed::Unknown,
    }
}

/// The element type of `<type>[]` or `<type>[<n>]`.
fn list_element(text: &str) -> Option<&str> {
    let open = text.strip_suffix(']')?.rfind('[')?;
    let size = text.get(open + 1..text.len() - 1)?;
    size.bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| text.get(..open))
        .flatten()
}

/// The name and the text between the parentheses of `<name>(<arguments>)`.
fn call(text: &str) -> Option<(&str, &str)> {
    let open = text.find('(')?;
    let arguments = text.strip_suffix(')')?.get(open + 1..)?;
    Some((text.get(..open)?.trim(), arguments))
}

/// The arguments of `text`, split at the commas outside parentheses and quotes, trimmed.
fn split(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut depth, mut quoted, mut start) = (0_usize, None, 0);
    for (at, character) in text.char_indices() {
        match (character, quoted) {
            ('"' | '\'', None) => quoted = Some(character),
            (_, Some(quote)) if character == quote => quoted = None,
            (_, Some(_)) => {}
            ('(', None) => depth += 1,
            (')', None) => depth = depth.saturating_sub(1),
            (',', None) if depth == 0 => {
                parts.push(text.get(start..at).unwrap_or("").trim());
                start = at + 1;
            }
            _ => {}
        }
    }
    parts.push(text.get(start..).unwrap_or("").trim());
    parts
}

/// The type of the struct member `<name> <type>`, the name bare or double-quoted.
fn member_type(member: &str) -> &str {
    let rest = if let Some(quoted) = member.strip_prefix('"') {
        quoted.find('"').and_then(|end| quoted.get(end + 1..))
    } else {
        member
            .find(char::is_whitespace)
            .and_then(|end| member.get(end..))
    };
    rest.unwrap_or("").trim()
}
