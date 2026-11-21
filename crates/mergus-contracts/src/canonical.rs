//! Canonical JSON, as RFC 8785 (the JSON Canonicalization Scheme) defines it (MG-REQ-0208).
//!
//! It is written in-house and checked against the reference vectors of the RFC
//! (MG-REQ-0185). Object members are sorted by the UTF-16 code units of their names, strings
//! keep every character but the ones JSON must escape, numbers are written as ECMAScript
//! writes an IEEE 754 double, and no whitespace is added.

use serde_json::{Map, Number, Value};

/// The largest decimal exponent ECMAScript still writes without an exponent.
const PLAIN_LIMIT: i32 = 21;

/// The smallest decimal exponent ECMAScript still writes without an exponent.
const SMALL_LIMIT: i32 = -6;

/// Serializes `value` as canonical JSON (RFC 8785).
///
/// Numbers are read as IEEE 754 doubles first, as the RFC requires, so an integer beyond
/// 2^53 is written as the double closest to it.
pub fn canonical_json(value: &Value) -> String {
    let mut out = String::new();
    write_value(value, &mut out);
    out
}

fn write_value(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(number) => write_number(number, out),
        Value::String(text) => write_string(text, out),
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_value(item, out);
            }
            out.push(']');
        }
        Value::Object(members) => write_object(members, out),
    }
}

/// Writes the members of an object sorted by the UTF-16 code units of their names.
fn write_object(members: &Map<String, Value>, out: &mut String) {
    let mut sorted: Vec<(&String, &Value)> = members.iter().collect();
    sorted.sort_by(|(left, _), (right, _)| left.encode_utf16().cmp(right.encode_utf16()));
    out.push('{');
    for (index, (name, value)) in sorted.into_iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        write_string(name, out);
        out.push(':');
        write_value(value, out);
    }
    out.push('}');
}

/// Writes a string as ECMAScript's `JSON.stringify` does.
fn write_string(text: &str, out: &mut String) {
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            control if u32::from(control) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", u32::from(control)));
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

/// Writes a number as ECMAScript's `Number.prototype.toString` writes the double closest to
/// it.
fn write_number(number: &Number, out: &mut String) {
    let Some(value) = number.as_f64() else {
        return out.push_str(&number.to_string());
    };
    if value == 0.0 {
        return out.push('0');
    }
    if value < 0.0 {
        out.push('-');
    }
    let (digits, exponent) = shortest_digits(value.abs());
    out.push_str(&ecmascript(&digits, exponent));
}

/// Returns the digits ECMAScript writes for a positive `value`, and the decimal exponent of
/// the first digit: `value` is `d.ddd` times ten to the power `exponent`.
///
/// They are the fewest digits that read back as `value` and, among those, the ones closest
/// to it, the even last digit winning a tie (ECMA-262, Number::toString, step 5).
fn shortest_digits(value: f64) -> (String, i32) {
    let shortest = format!("{value:e}");
    let count = split(&shortest).0.len();
    let closest = format!("{value:.precision$e}", precision = count.saturating_sub(1));
    let chosen = if closest.parse::<f64>() == Ok(value) {
        closest
    } else {
        shortest
    };
    split(&chosen)
}

/// Splits `d.ddde<exponent>` into its digits and its exponent.
fn split(text: &str) -> (String, i32) {
    let (mantissa, exponent) = text.split_once('e').unwrap_or((text, "0"));
    let digits = mantissa.chars().filter(char::is_ascii_digit).collect();
    (digits, exponent.parse().unwrap_or(0))
}

/// Lays out the digits of a positive number as ECMAScript does (ECMA-262, Number::toString).
fn ecmascript(digits: &str, exponent: i32) -> String {
    let count = i32::try_from(digits.len()).unwrap_or(i32::MAX);
    let point = exponent + 1;
    let zeros = |n: i32| "0".repeat(usize::try_from(n).unwrap_or(0));
    if count <= point && point <= PLAIN_LIMIT {
        format!("{digits}{}", zeros(point - count))
    } else if 0 < point && point <= PLAIN_LIMIT {
        let at = usize::try_from(point).unwrap_or(0);
        let (whole, fraction) = digits.split_at_checked(at).unwrap_or((digits, ""));
        format!("{whole}.{fraction}")
    } else if SMALL_LIMIT < point && point <= 0 {
        format!("0.{}{digits}", zeros(-point))
    } else {
        let sign = if exponent < 0 { '-' } else { '+' };
        let magnitude = exponent.unsigned_abs();
        match digits.split_at_checked(1) {
            Some((first, rest)) if !rest.is_empty() => format!("{first}.{rest}e{sign}{magnitude}"),
            _ => format!("{digits}e{sign}{magnitude}"),
        }
    }
}
