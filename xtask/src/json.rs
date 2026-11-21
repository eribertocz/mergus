//! A minimal JSON reader, enough for the output of `cargo metadata` and of the GitHub REST
//! API.
//!
//! `xtask` has no dependencies, so it reads JSON itself. Numbers keep their text.

use crate::failure::Failure;

/// A JSON value.
#[derive(Clone)]
pub(crate) enum Json {
    /// `null`.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A number, as written in the document.
    Number(String),
    /// A string.
    String(String),
    /// An array.
    Array(Vec<Json>),
    /// An object, its members in document order.
    Object(Vec<(String, Json)>),
}

impl Json {
    /// Parses a complete JSON document.
    ///
    /// # Errors
    ///
    /// Fails if `text` is not a single valid JSON value.
    pub(crate) fn parse(text: &str) -> Result<Self, Failure> {
        let mut parser = Parser {
            bytes: text.as_bytes(),
            at: 0,
        };
        let value = parser.value()?;
        parser.skip_space();
        if parser.at == parser.bytes.len() {
            Ok(value)
        } else {
            Err(parser.error("trailing characters"))
        }
    }

    /// Returns the member `key` of an object, or `None`.
    pub(crate) fn get(&self, key: &str) -> Option<&Self> {
        self.members()
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    /// Returns the string, or `None` for any other value.
    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(text) => Some(text),
            _ => None,
        }
    }

    /// Returns the number as an unsigned integer, or `None` for any other value.
    pub(crate) fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Number(text) => text.parse().ok(),
            _ => None,
        }
    }

    /// Returns the items of an array, or nothing for any other value.
    pub(crate) fn items(&self) -> &[Self] {
        match self {
            Self::Array(items) => items,
            _ => &[],
        }
    }

    /// Takes the items of an array, or returns `None` for any other value.
    pub(crate) fn into_items(self) -> Option<Vec<Self>> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    /// Returns the members of an object, or nothing for any other value.
    pub(crate) fn members(&self) -> &[(String, Self)] {
        match self {
            Self::Object(members) => members,
            _ => &[],
        }
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn value(&mut self) -> Result<Json, Failure> {
        self.skip_space();
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => self.string().map(Json::String),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(self.error("expected a value")),
        }
    }

    fn object(&mut self) -> Result<Json, Failure> {
        self.at += 1;
        let mut members = Vec::new();
        if self.next_is(b'}') {
            return Ok(Json::Object(members));
        }
        loop {
            self.skip_space();
            let key = self.string()?;
            self.skip_space();
            self.consume(b':')?;
            members.push((key, self.value()?));
            if !self.separator(b'}')? {
                return Ok(Json::Object(members));
            }
        }
    }

    fn array(&mut self) -> Result<Json, Failure> {
        self.at += 1;
        let mut items = Vec::new();
        if self.next_is(b']') {
            return Ok(Json::Array(items));
        }
        loop {
            items.push(self.value()?);
            if !self.separator(b']')? {
                return Ok(Json::Array(items));
            }
        }
    }

    /// Consumes `,` and returns `true`, or consumes `close` and returns `false`.
    fn separator(&mut self, close: u8) -> Result<bool, Failure> {
        self.skip_space();
        match self.peek() {
            Some(b',') => {
                self.at += 1;
                Ok(true)
            }
            Some(byte) if byte == close => {
                self.at += 1;
                Ok(false)
            }
            _ => Err(self.error("expected `,` or the end of the container")),
        }
    }

    fn string(&mut self) -> Result<String, Failure> {
        self.consume(b'"')?;
        let mut bytes = Vec::new();
        loop {
            match self.bump() {
                Some(b'"') => break,
                Some(b'\\') => self.escape(&mut bytes)?,
                Some(byte) if byte >= 0x20 => bytes.push(byte),
                _ => return Err(self.error("unterminated string")),
            }
        }
        String::from_utf8(bytes).map_err(|_| self.error("invalid UTF-8 in a string"))
    }

    fn escape(&mut self, bytes: &mut Vec<u8>) -> Result<(), Failure> {
        let decoded = match self.bump() {
            Some(byte @ (b'"' | b'\\' | b'/')) => char::from(byte),
            Some(b'b') => '\u{8}',
            Some(b'f') => '\u{c}',
            Some(b'n') => '\n',
            Some(b'r') => '\r',
            Some(b't') => '\t',
            Some(b'u') => self.unicode()?,
            _ => return Err(self.error("invalid escape")),
        };
        let mut buffer = [0; 4];
        bytes.extend_from_slice(decoded.encode_utf8(&mut buffer).as_bytes());
        Ok(())
    }

    fn unicode(&mut self) -> Result<char, Failure> {
        let high = self.hex4()?;
        let code = if (0xD800..0xDC00).contains(&high) {
            self.consume(b'\\')?;
            self.consume(b'u')?;
            let low = self.hex4()?;
            if !(0xDC00..0xE000).contains(&low) {
                return Err(self.error("invalid surrogate pair"));
            }
            0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00)
        } else {
            high
        };
        char::from_u32(code).ok_or_else(|| self.error("invalid code point"))
    }

    fn hex4(&mut self) -> Result<u32, Failure> {
        let digits = self
            .bytes
            .get(self.at..self.at + 4)
            .and_then(|digits| std::str::from_utf8(digits).ok())
            .and_then(|digits| u32::from_str_radix(digits, 16).ok())
            .ok_or_else(|| self.error("invalid `\\u` escape"))?;
        self.at += 4;
        Ok(digits)
    }

    fn number(&mut self) -> Result<Json, Failure> {
        let start = self.at;
        while matches!(
            self.peek(),
            Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
        ) {
            self.at += 1;
        }
        let digits = self.bytes.get(start..self.at).unwrap_or_default();
        let text = std::str::from_utf8(digits).unwrap_or_default();
        if text.parse::<f64>().is_ok() {
            Ok(Json::Number(text.to_owned()))
        } else {
            Err(self.error("invalid number"))
        }
    }

    fn literal(&mut self, word: &str, value: Json) -> Result<Json, Failure> {
        let rest = self.bytes.get(self.at..).unwrap_or_default();
        if rest.starts_with(word.as_bytes()) {
            self.at += word.len();
            Ok(value)
        } else {
            Err(self.error("expected a value"))
        }
    }

    fn consume(&mut self, byte: u8) -> Result<(), Failure> {
        if self.bump() == Some(byte) {
            Ok(())
        } else {
            Err(self.error(&format!("expected `{}`", char::from(byte))))
        }
    }

    fn next_is(&mut self, byte: u8) -> bool {
        self.skip_space();
        let found = self.peek() == Some(byte);
        if found {
            self.at += 1;
        }
        found
    }

    fn skip_space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let byte = self.peek();
        self.at += 1;
        byte
    }

    fn error(&self, message: &str) -> Failure {
        Failure::new(format!(
            "invalid JSON at byte {}: {message}",
            self.at.min(self.bytes.len())
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::Json;

    #[test]
    fn objects_arrays_and_strings_are_read() {
        let json =
            Json::parse(r#" {"a": [1, -2.5e3, true, null], "b": {"c": "d"}, "e": []} "#).unwrap();
        assert_eq!(json.get("a").unwrap().items().len(), 4);
        let inner = json.get("b").unwrap();
        assert_eq!(inner.get("c").and_then(Json::as_str), Some("d"));
        assert!(json.get("e").unwrap().items().is_empty());
        assert!(json.get("missing").is_none());
    }

    #[test]
    fn numbers_keep_their_value() {
        let json = Json::parse(r#"{"a": 42, "b": -1, "c": [7]}"#).unwrap();
        assert_eq!(json.get("a").and_then(Json::as_u64), Some(42));
        assert_eq!(json.get("b").and_then(Json::as_u64), None);
        let items = json.get("c").unwrap().clone().into_items().unwrap();
        assert_eq!(items.first().and_then(Json::as_u64), Some(7));
    }

    #[test]
    fn escapes_are_decoded() {
        let json = Json::parse(r#""a\"\\\/\n\té😀 ñ""#).unwrap();
        assert_eq!(json.as_str(), Some("a\"\\/\n\té😀 ñ"));
    }

    #[test]
    fn invalid_documents_fail() {
        for text in ["", "{", "[1,]", r#"{"a" 1}"#, "tru", r#""\x""#, "1 2", "-"] {
            assert!(Json::parse(text).is_err(), "{text:?} should fail");
        }
    }
}
