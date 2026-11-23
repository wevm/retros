//! Reads JSON, such as a `report.json` that `uphill --note` keeps, into a tree of values. Objects
//! keep their keys in source order. The reader takes the JSON that Uphill writes and rejects text
//! that is not JSON, but it does not report where the text went wrong.

/// A JSON value.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Json {
    Array(Vec<Json>),
    Bool(bool),
    Null,
    Number(f64),
    Object(Vec<(String, Json)>),
    String(String),
}

impl Json {
    /// Parses one JSON value, with only whitespace around it. Returns `None` for text that is not
    /// JSON.
    pub(crate) fn parse(text: &str) -> Option<Json> {
        let mut reader = Reader {
            bytes: text.as_bytes(),
            at: 0,
        };
        let value = reader.value()?;
        reader.space();
        (reader.at == reader.bytes.len()).then_some(value)
    }

    /// Returns the value of an object's key, or `None` when the value is not an object or has no
    /// such key.
    pub(crate) fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Self::Object(fields) => fields
                .iter()
                .find_map(|(name, value)| (name == key).then_some(value)),
            _ => None,
        }
    }

    /// Returns an array's values, or none for any other value.
    pub(crate) fn items(&self) -> &[Json] {
        match self {
            Self::Array(values) => values,
            _ => &[],
        }
    }

    /// Returns a number, or `None` for any other value.
    pub(crate) fn number(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value),
            _ => None,
        }
    }

    /// Returns a string, or `None` for any other value.
    pub(crate) fn text(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }
}

/// A position in the text being read.
struct Reader<'a> {
    at: usize,
    bytes: &'a [u8],
}

impl Reader<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    /// Consumes `word` and returns `value`, or `None` when the text does not continue with it.
    fn word(&mut self, word: &str, value: Json) -> Option<Json> {
        let end = self.at + word.len();
        (self.bytes.get(self.at..end)? == word.as_bytes()).then(|| {
            self.at = end;
            value
        })
    }

    fn value(&mut self) -> Option<Json> {
        self.space();
        match self.peek()? {
            b'{' => self.object(),
            b'[' => self.array(),
            b'"' => self.string().map(Json::String),
            b't' => self.word("true", Json::Bool(true)),
            b'f' => self.word("false", Json::Bool(false)),
            b'n' => self.word("null", Json::Null),
            _ => self.number(),
        }
    }

    fn object(&mut self) -> Option<Json> {
        self.at += 1;
        let mut fields = Vec::new();
        self.space();
        if self.peek()? == b'}' {
            self.at += 1;
            return Some(Json::Object(fields));
        }
        loop {
            self.space();
            if self.peek()? != b'"' {
                return None;
            }
            let key = self.string()?;
            self.space();
            if self.peek()? != b':' {
                return None;
            }
            self.at += 1;
            fields.push((key, self.value()?));

            self.space();
            match self.peek()? {
                b',' => self.at += 1,
                b'}' => {
                    self.at += 1;
                    return Some(Json::Object(fields));
                }
                _ => return None,
            }
        }
    }

    fn array(&mut self) -> Option<Json> {
        self.at += 1;
        let mut values = Vec::new();
        self.space();
        if self.peek()? == b']' {
            self.at += 1;
            return Some(Json::Array(values));
        }
        loop {
            values.push(self.value()?);

            self.space();
            match self.peek()? {
                b',' => self.at += 1,
                b']' => {
                    self.at += 1;
                    return Some(Json::Array(values));
                }
                _ => return None,
            }
        }
    }

    fn string(&mut self) -> Option<String> {
        self.at += 1;
        let mut out = String::new();
        loop {
            // A run of plain characters up to the next quote or escape copies as it is.
            let start = self.at;
            while !matches!(self.peek()?, b'"' | b'\\') {
                self.at += 1;
            }
            out.push_str(std::str::from_utf8(self.bytes.get(start..self.at)?).ok()?);

            let quote = self.peek()? == b'"';
            self.at += 1;
            if quote {
                return Some(out);
            }
            let escaped = self.peek()?;
            self.at += 1;
            match escaped {
                b'"' => out.push('"'),
                b'\\' => out.push('\\'),
                b'/' => out.push('/'),
                b'b' => out.push('\u{8}'),
                b'f' => out.push('\u{c}'),
                b'n' => out.push('\n'),
                b'r' => out.push('\r'),
                b't' => out.push('\t'),
                b'u' => {
                    let high = self.hex()?;
                    let code = if (0xD800..0xDC00).contains(&high) {
                        // A character outside the Basic Multilingual Plane is a surrogate pair.
                        if self.bytes.get(self.at..self.at + 2)? != b"\\u" {
                            return None;
                        }
                        self.at += 2;
                        let low = self.hex()?;
                        0x10000 + ((high - 0xD800) << 10) + low.checked_sub(0xDC00)?
                    } else {
                        high
                    };
                    out.push(char::from_u32(code)?);
                }
                _ => return None,
            }
        }
    }

    fn hex(&mut self) -> Option<u32> {
        let digits = std::str::from_utf8(self.bytes.get(self.at..self.at + 4)?).ok()?;
        self.at += 4;
        u32::from_str_radix(digits, 16).ok()
    }

    fn number(&mut self) -> Option<Json> {
        let start = self.at;
        while matches!(
            self.peek(),
            Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
        ) {
            self.at += 1;
        }
        let text = std::str::from_utf8(self.bytes.get(start..self.at)?).ok()?;
        text.parse().ok().map(Json::Number)
    }
}
