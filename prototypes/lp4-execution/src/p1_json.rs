//! Strict JSON (RFC 8259) for RPC requests: UTF-8 checked first, duplicate object keys rejected at
//! any depth (compared after unescaping), container nesting <= 16, lone surrogates rejected,
//! numbers kept as their raw lexeme. Only the request id is given a numeric value, and that value
//! follows the accepted C19 semantics: the IEEE-754 binary64 value of the lexeme (JSON.parse).

pub const MAX_DEPTH: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JVal {
    Null,
    Bool(bool),
    Num(String),
    Str(String),
    Arr(Vec<JVal>),
    Obj(Vec<(String, JVal)>),
}

impl JVal {
    pub fn get(&self, key: &str) -> Option<&JVal> {
        match self {
            JVal::Obj(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsonError {
    Utf8,
    Syntax,
    DuplicateKey,
    Depth,
}

impl JsonError {
    pub fn reason(&self) -> &'static str {
        match self {
            JsonError::Utf8 => "utf8",
            JsonError::Syntax => "json",
            JsonError::DuplicateKey => "duplicateKey",
            JsonError::Depth => "depth",
        }
    }
}

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> P<'a> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn eat(&mut self, c: u8) -> Result<(), JsonError> {
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            Err(JsonError::Syntax)
        }
    }

    fn value(&mut self, depth: usize) -> Result<JVal, JsonError> {
        self.ws();
        match self.peek() {
            Some(b'{') => self.object(depth + 1),
            Some(b'[') => self.array(depth + 1),
            Some(b'"') => Ok(JVal::Str(self.string()?)),
            Some(b't') => self.lit(b"true", JVal::Bool(true)),
            Some(b'f') => self.lit(b"false", JVal::Bool(false)),
            Some(b'n') => self.lit(b"null", JVal::Null),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
            _ => Err(JsonError::Syntax),
        }
    }

    fn lit(&mut self, word: &[u8], v: JVal) -> Result<JVal, JsonError> {
        if self.b[self.i..].starts_with(word) {
            self.i += word.len();
            Ok(v)
        } else {
            Err(JsonError::Syntax)
        }
    }

    fn object(&mut self, depth: usize) -> Result<JVal, JsonError> {
        if depth > MAX_DEPTH {
            return Err(JsonError::Depth);
        }
        self.eat(b'{')?;
        let mut members: Vec<(String, JVal)> = Vec::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(JVal::Obj(members));
        }
        loop {
            self.ws();
            if self.peek() != Some(b'"') {
                return Err(JsonError::Syntax);
            }
            let k = self.string()?;
            self.ws();
            self.eat(b':')?;
            let v = self.value(depth)?;
            if members.iter().any(|(x, _)| *x == k) {
                return Err(JsonError::DuplicateKey);
            }
            members.push((k, v));
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(JVal::Obj(members));
                }
                _ => return Err(JsonError::Syntax),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<JVal, JsonError> {
        if depth > MAX_DEPTH {
            return Err(JsonError::Depth);
        }
        self.eat(b'[')?;
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(JVal::Arr(items));
        }
        loop {
            items.push(self.value(depth)?);
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(JVal::Arr(items));
                }
                _ => return Err(JsonError::Syntax),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, JsonError> {
        if self.i + 4 > self.b.len() {
            return Err(JsonError::Syntax);
        }
        let mut v = 0u32;
        for k in 0..4 {
            let c = self.b[self.i + k];
            let d = match c {
                b'0'..=b'9' => c - b'0',
                b'a'..=b'f' => c - b'a' + 10,
                b'A'..=b'F' => c - b'A' + 10,
                _ => return Err(JsonError::Syntax),
            };
            v = v * 16 + d as u32;
        }
        self.i += 4;
        Ok(v)
    }

    fn string(&mut self) -> Result<String, JsonError> {
        self.eat(b'"')?;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = match self.peek() {
                Some(c) => c,
                None => return Err(JsonError::Syntax),
            };
            self.i += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let e = self.peek().ok_or(JsonError::Syntax)?;
                    self.i += 1;
                    match e {
                        b'"' => out.push(b'"'),
                        b'\\' => out.push(b'\\'),
                        b'/' => out.push(b'/'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let cp = if (0xd800..0xdc00).contains(&hi) {
                                if self.peek() != Some(b'\\') || self.b.get(self.i + 1) != Some(&b'u') {
                                    return Err(JsonError::Syntax);
                                }
                                self.i += 2;
                                let lo = self.hex4()?;
                                if !(0xdc00..0xe000).contains(&lo) {
                                    return Err(JsonError::Syntax);
                                }
                                0x10000 + ((hi - 0xd800) << 10) + (lo - 0xdc00)
                            } else if (0xdc00..0xe000).contains(&hi) {
                                return Err(JsonError::Syntax);
                            } else {
                                hi
                            };
                            let ch = std::char::from_u32(cp).ok_or(JsonError::Syntax)?;
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                        _ => return Err(JsonError::Syntax),
                    }
                }
                c if c < 0x20 => return Err(JsonError::Syntax),
                c => out.push(c),
            }
        }
        String::from_utf8(out).map_err(|_| JsonError::Utf8)
    }

    fn digits(&mut self) -> usize {
        let s = self.i;
        while self.i < self.b.len() && self.b[self.i].is_ascii_digit() {
            self.i += 1;
        }
        self.i - s
    }

    fn number(&mut self) -> Result<JVal, JsonError> {
        let s = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        match self.peek() {
            Some(b'0') => self.i += 1,
            Some(c) if c.is_ascii_digit() => {
                self.digits();
            }
            _ => return Err(JsonError::Syntax),
        }
        if self.peek() == Some(b'.') {
            self.i += 1;
            if self.digits() == 0 {
                return Err(JsonError::Syntax);
            }
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            self.i += 1;
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.i += 1;
            }
            if self.digits() == 0 {
                return Err(JsonError::Syntax);
            }
        }
        let lex = std::str::from_utf8(&self.b[s..self.i]).map_err(|_| JsonError::Syntax)?;
        Ok(JVal::Num(lex.to_string()))
    }
}

pub fn parse(bytes: &[u8]) -> Result<JVal, JsonError> {
    if std::str::from_utf8(bytes).is_err() {
        return Err(JsonError::Utf8);
    }
    let mut p = P { b: bytes, i: 0 };
    let v = p.value(0)?;
    p.ws();
    if p.i != bytes.len() {
        return Err(JsonError::Syntax);
    }
    Ok(v)
}

/// True iff `lex` is exactly one RFC 8259 number token (same grammar the parser uses).
pub fn is_number_lexeme(lex: &str) -> bool {
    let mut p = P { b: lex.as_bytes(), i: 0 };
    p.number().is_ok() && p.i == lex.len()
}

/// Accepted C19 id value semantics (JSON.parse / Python json + integral_value): the lexeme's
/// IEEE-754 binary64 value (round-to-nearest-even decimal conversion) must be finite and
/// integral and lie in 0..=2^32-1; -0 normalises to 0. So "1e-400" (underflow to 0) gives 0,
/// "4294967295.0000000001" (rounds to 4294967295) gives 4294967295, "4294967295.9999999999"
/// (rounds to 2^32) and "1e400" (overflow to infinity) are invalid, "2.5" is invalid.
/// Booleans, strings and null never reach this function (they are not number tokens).
pub fn integral_u32(lex: &str) -> Option<u32> {
    if !is_number_lexeme(lex) {
        return None;
    }
    // Rust's str -> f64 conversion is correctly rounded (nearest, ties to even), like JSON.parse.
    let x: f64 = lex.parse().ok()?;
    if !x.is_finite() || x.fract() != 0.0 || x < 0.0 || x > u32::MAX as f64 {
        return None;
    }
    Some(x as u32)
}

/// JSON string literal for output.
pub fn quote(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for ch in s.chars() {
        match ch {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("{}u{:04x}", '\\', c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// The 16-row native C19 oracle (coordination/lp1-review-r1/c19-native-oracle.json, Node v22.13.1
/// JSON.parse): (id lexeme, normalised id or None when invalid).
#[cfg(test)]
pub(crate) const C19_ORACLE: [(&str, Option<u32>); 16] = [
    ("1", Some(1)),
    ("1.0", Some(1)),
    ("1e0", Some(1)),
    ("-0", Some(0)),
    ("-0.0", Some(0)),
    ("1e-400", Some(0)),
    ("4294967295", Some(4294967295)),
    ("4294967295.0000000001", Some(4294967295)),
    ("4294967295.9999999999", None),
    ("4294967296", None),
    ("-1", None),
    ("2.5", None),
    ("true", None),
    ("\"2\"", None),
    ("null", None),
    ("1e400", None),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// One backslash, so escape sequences in test inputs are assembled at run time.
    const BS: char = '\\';

    #[test]
    fn accepts_and_rejects() {
        // Raw UTF-8 text (non-ASCII) in a normal string, converted with as_bytes().
        let utf8 = "{\"a\":[1,2,{\"b\":null}],\"c\":\"é😀\"}";
        assert!(utf8.bytes().any(|b| b >= 0x80));
        match parse(utf8.as_bytes()).unwrap().get("c") {
            Some(JVal::Str(s)) => assert_eq!(s, "é😀"),
            other => panic!("{other:?}"),
        }
        // The same characters as JSON escapes (a surrogate pair for the emoji).
        let escaped = format!("{{\"c\":\"{BS}u00e9{BS}ud83d{BS}ude00\"}}");
        assert!(escaped.is_ascii());
        assert_eq!(parse(escaped.as_bytes()).unwrap().get("c"), Some(&JVal::Str("é😀".to_string())));
        assert_eq!(parse(br#"{"a":1,"a":2}"#), Err(JsonError::DuplicateKey));
        let esc_dup = format!("{{\"x\":{{\"a\":1,\"{BS}u0061\":2}}}}");
        assert_eq!(parse(esc_dup.as_bytes()), Err(JsonError::DuplicateKey));
        assert_eq!(parse(br#"[{"k":[{"a":0,"a":0}]}]"#), Err(JsonError::DuplicateKey));
        assert_eq!(parse(b"{\"a\":\"\xff\"}"), Err(JsonError::Utf8));
        assert_eq!(parse(br#"{"a":01}"#), Err(JsonError::Syntax));
        assert_eq!(parse(br#"{"a":1.}"#), Err(JsonError::Syntax));
        assert_eq!(parse(br#"{"a":+1}"#), Err(JsonError::Syntax));
        assert_eq!(parse(br#"{"a":NaN}"#), Err(JsonError::Syntax));
        let lone_hi = format!("{{\"a\":\"{BS}ud800\"}}");
        assert_eq!(parse(lone_hi.as_bytes()), Err(JsonError::Syntax));
        let lone_lo = format!("{{\"a\":\"{BS}udc00\"}}");
        assert_eq!(parse(lone_lo.as_bytes()), Err(JsonError::Syntax));
        assert_eq!(parse(b"{\"a\":\"\x01\"}"), Err(JsonError::Syntax));
        assert_eq!(parse(br#"{} x"#), Err(JsonError::Syntax));
        assert_eq!(parse(br#"{"a":1,}"#), Err(JsonError::Syntax));
        assert_eq!(parse(b""), Err(JsonError::Syntax));
    }

    #[test]
    fn quote_escapes_controls() {
        let q = quote("a\u{1}\"");
        assert_eq!(q, format!("\"a{BS}u0001{BS}\"\""));
    }

    #[test]
    fn depth_boundary() {
        let ok = format!("{}{}", "[".repeat(16), "]".repeat(16));
        assert!(parse(ok.as_bytes()).is_ok());
        let deep = format!("{}{}", "[".repeat(17), "]".repeat(17));
        assert_eq!(parse(deep.as_bytes()), Err(JsonError::Depth));
        let huge = "[".repeat(100_000);
        assert_eq!(parse(huge.as_bytes()), Err(JsonError::Depth));
    }

    #[test]
    fn c19_native_oracle_rows() {
        for (lex, want) in C19_ORACLE.iter() {
            assert_eq!(integral_u32(lex), *want, "{lex}");
        }
    }

    #[test]
    fn integral_binary64_semantics() {
        for (lex, want) in [
            ("0", Some(0)),
            ("-0e5", Some(0)),
            ("7.0", Some(7)),
            ("70e-1", Some(7)),
            ("0.7e1", Some(7)),
            ("4294967295.000", Some(u32::MAX)),
            ("42949672950e-1", Some(u32::MAX)),
            ("-1e-400", Some(0)),
            ("1e-5", None),
            ("1.5", None),
            ("1e99999999999", None),
            ("0e99999999999", Some(0)),
            ("4294967296.0", None),
            // Not number tokens: refused before any conversion.
            ("+1", None),
            (".5", None),
            ("1.", None),
            ("01", None),
            ("inf", None),
            ("NaN", None),
            ("1e", None),
            (" 1", None),
            ("", None),
        ] {
            assert_eq!(integral_u32(lex), want, "{lex}");
        }
    }
}
