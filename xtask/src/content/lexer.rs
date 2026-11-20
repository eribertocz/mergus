//! A small Rust tokenizer: enough to find functions, braces and string literals while
//! skipping comments, character literals and lifetimes.

/// The kind of a token.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Kind {
    /// An identifier, keyword or number.
    Word(String),
    /// One punctuation character.
    Punct(char),
    /// A string literal of any form; holds the text between its quotes, escapes kept.
    Str(String),
}

/// A token and the line it starts on, starting at 1.
#[derive(Debug)]
pub(super) struct Token {
    pub(super) kind: Kind,
    pub(super) line: usize,
}

/// The string prefixes of Rust: `b"..."`, `r"..."`, `br#"..."#`, `c"..."`, `cr"..."`.
const STRING_PREFIXES: [&str; 5] = ["b", "r", "br", "c", "cr"];

/// Splits Rust source into tokens.
pub(super) fn tokens(source: &str) -> Vec<Token> {
    lex(source).tokens
}

/// Returns the lines, starting at 1, where a `//` comment starts, outside string literals.
pub(super) fn line_comments(source: &str) -> Vec<usize> {
    lex(source).line_comments
}

fn lex(source: &str) -> Lexer {
    let mut lexer = Lexer {
        chars: source.chars().collect(),
        at: 0,
        line: 1,
        tokens: Vec::new(),
        line_comments: Vec::new(),
    };
    lexer.run();
    lexer
}

struct Lexer {
    chars: Vec<char>,
    at: usize,
    line: usize,
    tokens: Vec<Token>,
    line_comments: Vec<usize>,
}

impl Lexer {
    fn peek(&self, offset: usize) -> Option<char> {
        self.chars.get(self.at + offset).copied()
    }

    /// Moves forward one character, counting lines.
    fn bump(&mut self) {
        if self.peek(0) == Some('\n') {
            self.line += 1;
        }
        self.at += 1;
    }

    fn push(&mut self, kind: Kind, line: usize) {
        self.tokens.push(Token { kind, line });
    }

    fn run(&mut self) {
        while let Some(current) = self.peek(0) {
            let line = self.line;
            match current {
                '/' if self.peek(1) == Some('/') => self.line_comment(),
                '/' if self.peek(1) == Some('*') => self.block_comment(),
                '"' => {
                    let text = self.string(0, false);
                    self.push(Kind::Str(text), line);
                }
                '\'' => self.quote_or_lifetime(),
                c if c.is_alphanumeric() || c == '_' => self.word(),
                c if c.is_whitespace() => self.bump(),
                c => {
                    self.bump();
                    self.push(Kind::Punct(c), line);
                }
            }
        }
    }

    fn line_comment(&mut self) {
        self.line_comments.push(self.line);
        while self.peek(0).is_some_and(|c| c != '\n') {
            self.bump();
        }
    }

    /// Skips a block comment, which may nest.
    fn block_comment(&mut self) {
        let mut depth = 0_usize;
        while let Some(current) = self.peek(0) {
            if current == '/' && self.peek(1) == Some('*') {
                depth += 1;
                self.bump();
            } else if current == '*' && self.peek(1) == Some('/') {
                depth -= 1;
                self.bump();
                if depth == 0 {
                    self.bump();
                    return;
                }
            }
            self.bump();
        }
    }

    /// Reads a string whose opening `"` is at the current position, closed by `"` and
    /// `hashes` times `#`. Escapes are kept as written and skipped unless the string is
    /// `raw`.
    fn string(&mut self, hashes: usize, raw: bool) -> String {
        self.bump();
        let mut text = String::new();
        while let Some(current) = self.peek(0) {
            if current == '\\' && !raw {
                text.push(current);
                self.bump();
                if let Some(escaped) = self.peek(0) {
                    text.push(escaped);
                    self.bump();
                }
                continue;
            }
            if current == '"' && (1..=hashes).all(|offset| self.peek(offset) == Some('#')) {
                for _ in 0..=hashes {
                    self.bump();
                }
                return text;
            }
            text.push(current);
            self.bump();
        }
        text
    }

    fn word(&mut self) {
        let line = self.line;
        let mut word = String::new();
        while let Some(current) = self.peek(0) {
            if !(current.is_alphanumeric() || current == '_') {
                break;
            }
            word.push(current);
            self.bump();
        }
        if STRING_PREFIXES.contains(&word.as_str()) {
            let raw = word.contains('r');
            let mut hashes = 0;
            while raw && self.peek(hashes) == Some('#') {
                hashes += 1;
            }
            if self.peek(hashes) == Some('"') {
                for _ in 0..hashes {
                    self.bump();
                }
                let text = self.string(hashes, raw);
                self.push(Kind::Str(text), line);
                return;
            }
            if word == "b" && self.peek(0) == Some('\'') {
                self.quote_or_lifetime();
                return;
            }
        }
        self.push(Kind::Word(word), line);
    }

    /// Skips a character literal such as `'a'` or `'\n'`, or the quote of a lifetime.
    fn quote_or_lifetime(&mut self) {
        if self.peek(1) == Some('\\') {
            self.bump();
            self.bump();
            self.bump();
            while self.peek(0).is_some_and(|c| c != '\'' && c != '\n') {
                self.bump();
            }
            self.bump();
        } else if self.peek(2) == Some('\'') {
            self.bump();
            self.bump();
            self.bump();
        } else {
            self.bump();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Kind, line_comments, tokens};

    fn kinds(source: &str) -> Vec<Kind> {
        tokens(source).into_iter().map(|token| token.kind).collect()
    }

    fn word(text: &str) -> Kind {
        Kind::Word(text.to_owned())
    }

    fn string(text: &str) -> Kind {
        Kind::Str(text.to_owned())
    }

    #[test]
    fn comments_characters_and_lifetimes_are_skipped() {
        let source = "a // b {\n/* c /* d */ { */ '{' '\\'' 'x: &'x str";
        assert_eq!(
            kinds(source),
            [
                word("a"),
                word("x"),
                Kind::Punct(':'),
                Kind::Punct('&'),
                word("x"),
                word("str"),
            ]
        );
    }

    #[test]
    fn strings_of_every_form_are_one_token() {
        let source = "\"a\\\"{\" r#\"b\"{\"# b\"c\" br\"d\\\" e";
        assert_eq!(
            kinds(source),
            [
                string("a\\\"{"),
                string("b\"{"),
                string("c"),
                string("d\\"),
                word("e"),
            ]
        );
    }

    #[test]
    fn line_comments_outside_strings_are_found() {
        let source = "// a\nlet s = \"\n// not a comment\n\";\nx; // b\n/* // c */";
        assert_eq!(line_comments(source), [1, 5]);
    }

    #[test]
    fn tokens_carry_their_line() {
        let lines: Vec<usize> = tokens("a\n\"b\nc\" d\n\ne")
            .into_iter()
            .map(|token| token.line)
            .collect();
        assert_eq!(lines, [1, 2, 3, 5]);
    }
}
