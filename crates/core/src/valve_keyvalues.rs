use crate::error::{CoreError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyValueEntry {
    pub name: String,
    pub value: Option<String>,
    pub object: Option<KeyValuesObject>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KeyValuesObject {
    pub entries: Vec<KeyValueEntry>,
}

impl KeyValuesObject {
    pub fn get_string(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|e| e.object.is_none() && e.name.eq_ignore_ascii_case(name))
            .and_then(|e| e.value.as_deref())
    }

    pub fn get_objects(&self, name: &str) -> Vec<&KeyValuesObject> {
        self.entries
            .iter()
            .filter(|e| e.object.is_some() && e.name.eq_ignore_ascii_case(name))
            .filter_map(|e| e.object.as_ref())
            .collect()
    }
}

/// Valve KeyValues (VDF) parser mirroring the C# implementation.
pub struct ValveKeyValues;

const MAX_NESTING_DEPTH: usize = 128;

impl ValveKeyValues {
    pub fn parse(text: &str) -> Result<KeyValuesObject> {
        let mut parser = Parser { text, position: 0 };
        let result = parser.read_object(false, 0)?;
        if parser.read_token()?.is_some() {
            return Err(CoreError::InvalidData(
                "Unexpected trailing KeyValues content.".to_string(),
            ));
        }
        Ok(result)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TokenKind {
    Value,
    Open,
    Close,
}

#[derive(Debug, Clone)]
struct Token {
    kind: TokenKind,
    value: String,
}

struct Parser<'a> {
    text: &'a str,
    position: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.text[self.position..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.position += c.len_utf8();
        Some(c)
    }

    fn read_object(&mut self, requires_close: bool, depth: usize) -> Result<KeyValuesObject> {
        if depth > MAX_NESTING_DEPTH {
            return Err(CoreError::InvalidData(
                "KeyValues nesting is too deep.".to_string(),
            ));
        }
        let mut entries = Vec::new();
        loop {
            let key = self.read_token()?;
            let Some(key) = key else {
                if requires_close {
                    return Err(CoreError::InvalidData(
                        "KeyValues object is not closed.".to_string(),
                    ));
                }
                return Ok(KeyValuesObject { entries });
            };
            if key.kind == TokenKind::Close {
                if !requires_close {
                    return Err(CoreError::InvalidData(
                        "Unexpected closing brace.".to_string(),
                    ));
                }
                return Ok(KeyValuesObject { entries });
            }
            if key.kind != TokenKind::Value {
                return Err(CoreError::InvalidData(
                    "A KeyValues key was expected.".to_string(),
                ));
            }
            let value = self.read_token()?.ok_or_else(|| {
                CoreError::InvalidData(format!("KeyValues entry '{}' has no value.", key.value))
            })?;
            match value.kind {
                TokenKind::Open => entries.push(KeyValueEntry {
                    name: key.value,
                    value: None,
                    object: Some(self.read_object(true, depth + 1)?),
                }),
                TokenKind::Value => entries.push(KeyValueEntry {
                    name: key.value,
                    value: Some(value.value),
                    object: None,
                }),
                TokenKind::Close => {
                    return Err(CoreError::InvalidData(format!(
                        "KeyValues entry '{}' has an invalid value.",
                        key.value
                    )));
                }
            }
        }
    }

    fn read_token(&mut self) -> Result<Option<Token>> {
        self.skip_trivia();
        let Some(c) = self.peek() else {
            return Ok(None);
        };
        if c == '{' {
            self.bump();
            return Ok(Some(Token {
                kind: TokenKind::Open,
                value: "{".to_string(),
            }));
        }
        if c == '}' {
            self.bump();
            return Ok(Some(Token {
                kind: TokenKind::Close,
                value: "}".to_string(),
            }));
        }
        if c == '"' {
            return Ok(Some(Token {
                kind: TokenKind::Value,
                value: self.read_quoted()?,
            }));
        }
        let start = self.position;
        while let Some(next) = self.peek() {
            if next.is_whitespace() || next == '{' || next == '}' {
                break;
            }
            self.bump();
        }
        Ok(Some(Token {
            kind: TokenKind::Value,
            value: self.text[start..self.position].to_string(),
        }))
    }

    fn read_quoted(&mut self) -> Result<String> {
        self.bump(); // opening quote
        let mut value = String::new();
        while let Some(character) = self.bump() {
            if character == '"' {
                return Ok(value);
            }
            if character != '\\' {
                value.push(character);
                continue;
            }
            let Some(escaped) = self.bump() else {
                break;
            };
            value.push(match escaped {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '"' => '"',
                '\\' => '\\',
                other => other,
            });
        }
        Err(CoreError::InvalidData(
            "Quoted KeyValues string is not closed.".to_string(),
        ))
    }

    fn skip_trivia(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() || c == '\u{FEFF}' => {
                    self.bump();
                }
                Some('/') => {
                    let rest = &self.text[self.position..];
                    if rest.starts_with("//") {
                        self.position += 2;
                        while let Some(c) = self.peek() {
                            if c == '\n' {
                                break;
                            }
                            self.bump();
                        }
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
    }
}
