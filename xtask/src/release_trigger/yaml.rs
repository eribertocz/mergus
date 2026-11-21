//! The subset of YAML that GitHub workflows use: block mappings and sequences, flow
//! mappings and sequences, plain and quoted scalars, block scalars and comments.
//!
//! `xtask` has no dependencies, so it reads workflows itself. Anything outside the subset is
//! an error, never a silent pass.

/// A YAML node. Every scalar is kept as text.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Node {
    /// A scalar, unquoted; empty for a null value or a block scalar.
    Scalar(String),
    /// A sequence.
    List(Vec<Node>),
    /// A mapping, in the order of its keys.
    Map(Vec<(String, Node)>),
}

impl Node {
    /// The value of `key` when the node is a mapping that has it.
    pub(crate) fn get(&self, key: &str) -> Option<&Self> {
        match self {
            Self::Map(entries) => entries.iter().find(|(name, _)| name == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The text of a scalar.
    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Self::Scalar(text) => Some(text),
            _ => None,
        }
    }
}

/// Parses a YAML document.
///
/// # Errors
///
/// Fails with a message naming the line when the text is outside the subset.
pub(crate) fn parse(text: &str) -> Result<Node, String> {
    let mut lines = Vec::new();
    for (number, raw) in text.lines().enumerate() {
        let indent = raw.len() - raw.trim_start_matches(' ').len();
        let content = strip_comment(raw.get(indent..).unwrap_or_default()).trim_end();
        if content.starts_with('\t') {
            return Err(format!("line {}: tab used for indentation", number + 1));
        }
        let content = if number == 0 && content == "---" {
            ""
        } else {
            content
        };
        lines.push((indent, content.to_owned()));
    }
    let mut parser = Parser { lines, at: 0 };
    let node = parser.node()?;
    match parser.peek() {
        Some(_) => Err(parser.error("unexpected content")),
        None => Ok(node),
    }
}

/// A line-by-line parser over `(indent, content)` pairs, comments removed.
struct Parser {
    lines: Vec<(usize, String)>,
    at: usize,
}

impl Parser {
    /// The next line with content, skipping blank ones.
    fn peek(&mut self) -> Option<(usize, String)> {
        while self
            .lines
            .get(self.at)
            .is_some_and(|(_, text)| text.is_empty())
        {
            self.at += 1;
        }
        self.lines.get(self.at).cloned()
    }

    fn error(&self, message: &str) -> String {
        format!("line {}: {message}", self.at + 1)
    }

    /// The block node that starts at the next line with content.
    fn node(&mut self) -> Result<Node, String> {
        match self.peek() {
            None => Ok(Node::Scalar(String::new())),
            Some((indent, text)) if is_item(&text) => self.list(indent),
            Some((indent, _)) => self.map(indent),
        }
    }

    /// A block mapping whose keys are at `indent`.
    fn map(&mut self, indent: usize) -> Result<Node, String> {
        let mut entries = Vec::new();
        while let Some((at, text)) = self.peek() {
            if at < indent || (at == indent && is_item(&text)) {
                break;
            }
            if at > indent {
                return Err(self.error("unexpected indentation"));
            }
            let (key, value) = split_key(&text).ok_or_else(|| self.error("expected `key:`"))?;
            self.at += 1;
            let value = self.value(indent, &value)?;
            entries.push((key, value));
        }
        Ok(Node::Map(entries))
    }

    /// A block sequence whose `-` are at `indent`.
    fn list(&mut self, indent: usize) -> Result<Node, String> {
        let mut items = Vec::new();
        while let Some((at, text)) = self.peek() {
            if at < indent || !is_item(&text) {
                break;
            }
            if at > indent {
                return Err(self.error("unexpected indentation"));
            }
            let rest = text.get(1..).unwrap_or_default();
            let inner = indent + 1 + (rest.len() - rest.trim_start().len());
            let rest = rest.trim_start().to_owned();
            let nested = is_item(&rest);
            if nested || (!rest.is_empty() && split_key(&rest).is_some()) {
                if let Some(line) = self.lines.get_mut(self.at) {
                    *line = (inner, rest);
                }
                items.push(if nested {
                    self.list(inner)?
                } else {
                    self.map(inner)?
                });
            } else {
                self.at += 1;
                items.push(self.value(indent, &rest)?);
            }
        }
        Ok(Node::List(items))
    }

    /// The value of a key or an item at `indent`, whose text after `:` or `-` is `text`.
    fn value(&mut self, indent: usize, text: &str) -> Result<Node, String> {
        if text.starts_with(['|', '>']) {
            while self
                .lines
                .get(self.at)
                .is_some_and(|(at, line)| line.is_empty() || *at > indent)
            {
                self.at += 1;
            }
            return Ok(Node::Scalar(String::new()));
        }
        if !text.is_empty() {
            return flow(text).map_err(|message| self.error(&message));
        }
        match self.peek() {
            Some((at, _)) if at > indent => self.node(),
            Some((at, line)) if at == indent && is_item(&line) => self.list(indent),
            _ => Ok(Node::Scalar(String::new())),
        }
    }
}

/// Whether a line is an item of a block sequence.
fn is_item(text: &str) -> bool {
    text == "-" || text.starts_with("- ")
}

/// Removes a comment: `#` at the start or after a blank, outside quotes.
fn strip_comment(line: &str) -> &str {
    let mut quote = None;
    let mut previous = ' ';
    for (at, c) in line.char_indices() {
        match quote {
            Some(open) if c == open => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '#' && previous.is_whitespace() => return line.get(..at).unwrap_or(line),
            None => {}
        }
        previous = c;
    }
    line
}

/// Splits `key: value` (or `key:`) at the first `:` followed by a blank or the end, outside
/// quotes and flow collections; `None` when the line is no such pair.
fn split_key(text: &str) -> Option<(String, String)> {
    if text.starts_with(['[', '{']) {
        return None;
    }
    let mut parser = Flow::new(text);
    let key = parser.scalar(false).ok()?;
    let rest = parser.rest().strip_prefix(':')?;
    if !(rest.is_empty() || rest.starts_with(' ')) {
        return None;
    }
    Some((key, rest.trim().to_owned()))
}

/// Parses an inline value: a flow collection or a scalar.
fn flow(text: &str) -> Result<Node, String> {
    if !text.starts_with(['[', '{', '"', '\'']) {
        return Ok(Node::Scalar(text.to_owned()));
    }
    let mut parser = Flow::new(text);
    let node = parser.node()?;
    if parser.rest().trim().is_empty() {
        Ok(node)
    } else {
        Err(format!("unexpected `{}`", parser.rest().trim()))
    }
}

/// A parser of flow values over one line.
struct Flow<'a> {
    text: &'a str,
    at: usize,
}

impl<'a> Flow<'a> {
    fn new(text: &'a str) -> Self {
        Self { text, at: 0 }
    }

    fn rest(&self) -> &'a str {
        self.text.get(self.at..).unwrap_or_default()
    }

    fn skip_blanks(&mut self) {
        let rest = self.rest();
        self.at += rest.len() - rest.trim_start().len();
    }

    /// Consumes `c` after blanks, and returns whether it was there.
    fn eat(&mut self, c: char) -> bool {
        self.skip_blanks();
        let found = self.rest().starts_with(c);
        if found {
            self.at += c.len_utf8();
        }
        found
    }

    fn node(&mut self) -> Result<Node, String> {
        self.skip_blanks();
        if self.eat('[') {
            let mut items = Vec::new();
            while !self.eat(']') {
                items.push(self.node()?);
                self.separator(']')?;
            }
            return Ok(Node::List(items));
        }
        if self.eat('{') {
            let mut entries = Vec::new();
            while !self.eat('}') {
                let key = self.scalar(true)?;
                let value = if self.eat(':') {
                    self.node()?
                } else {
                    Node::Scalar(String::new())
                };
                entries.push((key, value));
                self.separator('}')?;
            }
            return Ok(Node::Map(entries));
        }
        self.scalar(true).map(Node::Scalar)
    }

    /// After an item: a `,`, or the closing `close`, which is left for the caller.
    fn separator(&mut self, close: char) -> Result<(), String> {
        if self.eat(',') {
            return Ok(());
        }
        self.skip_blanks();
        if self.rest().starts_with(close) {
            Ok(())
        } else {
            Err(format!("expected `,` or `{close}`"))
        }
    }

    /// A quoted or plain scalar. In a flow collection a plain one ends at `,`, `]`, `}` or
    /// `: `; outside, only at `: ` or the end.
    fn scalar(&mut self, in_flow: bool) -> Result<String, String> {
        self.skip_blanks();
        let rest = self.rest();
        if let Some(quote) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') {
            return self.quoted(quote);
        }
        let mut end = rest.len();
        for (at, c) in rest.char_indices() {
            let colon = c == ':'
                && rest.get(at + 1..).is_none_or(|after| {
                    after.is_empty() || after.starts_with([' ', ',', ']', '}'])
                });
            if colon || (in_flow && matches!(c, ',' | ']' | '}')) {
                end = at;
                break;
            }
        }
        self.at += end;
        Ok(rest.get(..end).unwrap_or_default().trim().to_owned())
    }

    /// A scalar between `quote`s: `\` escapes in double quotes, `''` in single quotes.
    fn quoted(&mut self, quote: char) -> Result<String, String> {
        let mut value = String::new();
        let mut chars = self.rest().char_indices().skip(1).peekable();
        while let Some((at, c)) = chars.next() {
            match c {
                '\\' if quote == '"' => {
                    if let Some((_, escaped)) = chars.next() {
                        value.push(escaped);
                    }
                }
                _ if c == quote
                    && quote == '\''
                    && chars.peek().is_some_and(|(_, n)| *n == '\'') =>
                {
                    chars.next();
                    value.push('\'');
                }
                _ if c == quote => {
                    self.at += at + 1;
                    return Ok(value);
                }
                _ => value.push(c),
            }
        }
        Err("unclosed quote".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::{Node, parse};

    fn scalar(text: &str) -> Node {
        Node::Scalar(text.to_owned())
    }

    #[test]
    fn mg_req_1715_block_and_flow_collections_are_read() {
        let text = "---\non:\n  push:\n    tags: ['v*', \"x\\\"y\"]\njobs:\n  - a: 1 # c\n    \
                    b: 'it''s'\n  - - z\n  - { k: [1, 2], e }\nrun: |\n  x: y\n\n  - z\nend: \"#\"\n";
        let node = parse(text).unwrap();
        let push = node.get("on").and_then(|on| on.get("push")).unwrap();
        assert_eq!(
            push.get("tags"),
            Some(&Node::List(vec![scalar("v*"), scalar("x\"y")]))
        );
        let Some(Node::List(jobs)) = node.get("jobs") else {
            panic!("jobs is not a list");
        };
        assert_eq!(jobs[0].get("b").and_then(Node::as_str), Some("it's"));
        assert_eq!(jobs[1], Node::List(vec![scalar("z")]));
        assert_eq!(
            jobs[2],
            Node::Map(vec![
                ("k".to_owned(), Node::List(vec![scalar("1"), scalar("2")])),
                ("e".to_owned(), scalar("")),
            ])
        );
        assert_eq!(node.get("run"), Some(&scalar("")));
        assert_eq!(node.get("end").and_then(Node::as_str), Some("#"));
    }

    #[test]
    fn mg_req_1715_text_outside_the_subset_fails() {
        for text in [
            "a:\n    b: 1\n  c: 2\n",
            "a: [1\n",
            "a: 'x\n",
            "a\n",
            "a: {b: 1} x\n",
        ] {
            assert!(parse(text).is_err(), "{text}");
        }
    }
}
