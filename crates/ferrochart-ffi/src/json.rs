//! A small JSON reader and writer for the C ABI:
//! indicator parameters, drawings and saved states. `serde_json` is only a
//! dev dependency, and these documents are small.
//!
//! Strict RFC 8259 input (no comments, no trailing commas), at most
//! [`MAX_DEPTH`] levels of nesting. Object keys keep their order; a repeated
//! key is an error. Non-finite numbers are written as `null`.

use std::fmt::Write as _;

/// Deepest nesting of arrays and objects accepted.
pub(crate) const MAX_DEPTH: usize = 64;

/// A JSON value.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    /// Members in document order.
    Object(Vec<(String, Value)>),
}

impl Value {
    /// The member `key` of an object.
    pub(crate) fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The member `key`, `None` if absent or `null`.
    pub(crate) fn opt(&self, key: &str) -> Option<&Value> {
        self.get(key).filter(|v| **v != Value::Null)
    }

    pub(crate) fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(x) => Some(*x),
            _ => None,
        }
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub(crate) fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub(crate) fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    pub(crate) fn members(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Object(m) => Some(m),
            _ => None,
        }
    }

    /// An object from `(key, value)` pairs.
    pub(crate) fn object<const N: usize>(members: [(&str, Value); N]) -> Value {
        Value::Object(
            members
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v))
                .collect(),
        )
    }

    /// Serializes compactly.
    pub(crate) fn to_json(&self) -> String {
        let mut out = String::new();
        write_value(self, &mut out);
        out
    }
}

impl From<f64> for Value {
    fn from(x: f64) -> Self {
        Value::Number(x)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_owned())
    }
}

fn write_value(v: &Value, out: &mut String) {
    match v {
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(x) if x.is_finite() => {
            // Shortest form that reads back to the same f64.
            let _ = write!(out, "{x}");
        }
        Value::Null | Value::Number(_) => out.push_str("null"),
        Value::String(s) => write_string(s, out),
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(x, out);
            }
            out.push(']');
        }
        Value::Object(m) => {
            out.push('{');
            for (i, (k, x)) in m.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(k, out);
                out.push(':');
                write_value(x, out);
            }
            out.push('}');
        }
    }
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Parses a JSON document.
///
/// # Errors
///
/// A message with the byte offset of the first error.
pub(crate) fn parse(text: &str) -> Result<Value, String> {
    let mut p = Parser {
        s: text.as_bytes(),
        pos: 0,
    };
    p.ws();
    let v = p.value(0)?;
    p.ws();
    if p.pos != p.s.len() {
        return Err(p.err("unexpected data after the value"));
    }
    Ok(v)
}

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn err(&self, what: &str) -> String {
        format!("invalid JSON at byte {}: {what}", self.pos)
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.pos).copied()
    }

    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn eat(&mut self, lit: &[u8]) -> bool {
        if self.s[self.pos..].starts_with(lit) {
            self.pos += lit.len();
            true
        } else {
            false
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, String> {
        match self.peek() {
            Some(b'{') => self.object(depth + 1),
            Some(b'[') => self.array(depth + 1),
            Some(b'"') => self.string().map(Value::String),
            Some(b't') if self.eat(b"true") => Ok(Value::Bool(true)),
            Some(b'f') if self.eat(b"false") => Ok(Value::Bool(false)),
            Some(b'n') if self.eat(b"null") => Ok(Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.err("expected a value")),
            None => Err(self.err("unexpected end")),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, String> {
        if depth > MAX_DEPTH {
            return Err(self.err("nested too deeply"));
        }
        self.pos += 1;
        let mut members: Vec<(String, Value)> = Vec::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::Object(members));
        }
        loop {
            self.ws();
            if self.peek() != Some(b'"') {
                return Err(self.err("expected a key"));
            }
            let key = self.string()?;
            if members.iter().any(|(k, _)| *k == key) {
                return Err(self.err(&format!("repeated key '{key}'")));
            }
            self.ws();
            if self.peek() != Some(b':') {
                return Err(self.err("expected ':'"));
            }
            self.pos += 1;
            self.ws();
            let v = self.value(depth)?;
            members.push((key, v));
            self.ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Object(members));
                }
                _ => return Err(self.err("expected ',' or '}'")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, String> {
        if depth > MAX_DEPTH {
            return Err(self.err("nested too deeply"));
        }
        self.pos += 1;
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.ws();
            items.push(self.value(depth)?);
            self.ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(self.err("expected ',' or ']'")),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self
            .s
            .get(self.pos..self.pos + 4)
            .ok_or_else(|| self.err("short \\u escape"))?;
        let text = std::str::from_utf8(digits).map_err(|_| self.err("bad \\u escape"))?;
        let v = u32::from_str_radix(text, 16).map_err(|_| self.err("bad \\u escape"))?;
        self.pos += 4;
        Ok(v)
    }

    fn string(&mut self) -> Result<String, String> {
        self.pos += 1; // opening quote
        let mut out = String::new();
        loop {
            let start = self.pos;
            while let Some(c) = self.peek() {
                if c == b'"' || c == b'\\' || c < 0x20 {
                    break;
                }
                self.pos += 1;
            }
            // The input is a &str and the run stops at ASCII bytes only, so
            // it is valid UTF-8.
            out.push_str(
                std::str::from_utf8(&self.s[start..self.pos])
                    .map_err(|_| self.err("invalid UTF-8"))?,
            );
            match self.peek() {
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    let e = self.peek().ok_or_else(|| self.err("unexpected end"))?;
                    self.pos += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let code = if (0xD800..0xDC00).contains(&hi) {
                                if !self.eat(b"\\u") {
                                    return Err(self.err("unpaired surrogate"));
                                }
                                let lo = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&lo) {
                                    return Err(self.err("unpaired surrogate"));
                                }
                                0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                            } else {
                                hi
                            };
                            out.push(
                                char::from_u32(code)
                                    .ok_or_else(|| self.err("invalid \\u escape"))?,
                            );
                        }
                        _ => return Err(self.err("invalid escape")),
                    }
                }
                Some(_) => return Err(self.err("control character in a string")),
                None => return Err(self.err("unterminated string")),
            }
        }
    }

    fn number(&mut self) -> Result<Value, String> {
        let start = self.pos;
        self.eat(b"-");
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.pos += 1;
                }
            }
            _ => return Err(self.err("invalid number")),
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.err("invalid number"));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.err("invalid number"));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        let text = std::str::from_utf8(&self.s[start..self.pos])
            .map_err(|_| self.err("invalid number"))?;
        let x: f64 = text.parse().map_err(|_| self.err("invalid number"))?;
        if !x.is_finite() {
            return Err(self.err("number out of range"));
        }
        Ok(Value::Number(x))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let text = r#"{"a":[1,-2.5,1e300,0.1,true,false,null],"s":"q\"\\\n\t\u0001é😀","o":{}}"#;
        let v = parse(text).unwrap();
        assert_eq!(
            v.get("a").unwrap().as_array().unwrap()[3],
            Value::Number(0.1)
        );
        assert_eq!(v.get("s").unwrap().as_str().unwrap(), "q\"\\\n\t\u{1}é😀");
        assert_eq!(parse(&v.to_json()).unwrap(), v);
        assert_eq!(
            parse(r#""\ud83d\ude00""#).unwrap(),
            Value::String("😀".into())
        );
        assert_eq!(Value::Number(f64::NAN).to_json(), "null");
        assert_eq!(
            Value::Number(1_700_000_000_000.0).to_json(),
            "1700000000000"
        );
    }

    #[test]
    fn rejects_invalid_documents() {
        for bad in [
            "",
            "{",
            "[1,]",
            "{\"a\":1,}",
            "01",
            "1.",
            "-",
            "1e",
            "tru",
            "\"a",
            "\"\\x\"",
            "\"\\ud800\"",
            "{\"a\":1,\"a\":2}",
            "[1] 2",
            "{a:1}",
            "\"\u{1}\"",
            "1e999",
        ] {
            assert!(parse(bad).is_err(), "{bad:?} parsed");
        }
        let deep = "[".repeat(MAX_DEPTH + 1) + &"]".repeat(MAX_DEPTH + 1);
        assert!(parse(&deep).is_err());
        let ok = "[".repeat(MAX_DEPTH) + &"]".repeat(MAX_DEPTH);
        assert!(parse(&ok).is_ok());
    }
}
