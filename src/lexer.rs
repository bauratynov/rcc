use crate::error::{CompileError, Span};

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Int(i64),
    Float(f64),
    Str(String),
    Char(i64),
    Ident(String),
    Keyword(String),
    Punct(String),
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

static KEYWORDS: &[&str] = &[
    "return", "if", "else", "for", "while", "do", "int", "char", "short",
    "long", "void", "struct", "union", "enum", "typedef", "sizeof", "static",
    "extern", "signed", "unsigned", "const", "volatile", "switch", "case",
    "default", "break", "continue", "goto", "float", "double", "auto",
    "register", "inline", "_Bool", "_Noreturn", "_Atomic", "_Alignof",
    "_Alignas", "_Thread_local", "__attribute__", "__declspec",
    "__cdecl", "__stdcall", "__restrict", "__restrict__", "__inline",
    "restrict",
    "__alignof__", "typeof", "_Generic", "asm", "__asm__", "__volatile__",
];

static MULTI_CHAR_PUNCTS: &[&str] = &[
    "<<=", ">>=", "...", "==", "!=", "<=", ">=", "->", "+=", "-=", "*=",
    "/=", "++", "--", "%=", "&=", "|=", "^=", "&&", "||", "<<", ">>", "##",
];

struct Lexer<'a> {
    source: &'a str,
    bytes: &'a [u8],
    pos: usize,
    tokens: Vec<Token>,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            pos: 0,
            tokens: Vec::new(),
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.bytes.get(self.pos + offset).copied()
    }

    fn advance(&mut self) -> u8 {
        let ch = self.bytes[self.pos];
        self.pos += 1;
        ch
    }

    fn err(&self, msg: impl Into<String>, offset: usize, len: usize) -> CompileError {
        CompileError::new(msg, Span::new(offset, len))
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn skip_line_comment(&mut self) {
        self.pos += 2;
        while self.pos < self.bytes.len() && self.bytes[self.pos] != b'\n' {
            self.pos += 1;
        }
    }

    fn skip_block_comment(&mut self) -> Result<(), CompileError> {
        let start = self.pos;
        self.pos += 2;
        loop {
            if self.pos + 1 >= self.bytes.len() {
                return Err(self.err("unclosed block comment", start, 2));
            }
            if self.bytes[self.pos] == b'*' && self.bytes[self.pos + 1] == b'/' {
                self.pos += 2;
                return Ok(());
            }
            self.pos += 1;
        }
    }

    fn read_number(&mut self) -> Result<(), CompileError> {
        let start = self.pos;
        let ch = self.bytes[self.pos];

        // Hex: 0x...
        if ch == b'0' && self.peek_at(1).map_or(false, |c| c == b'x' || c == b'X') {
            self.pos += 2;
            if !self.peek().map_or(false, |c| c.is_ascii_hexdigit()) {
                return Err(self.err("expected hex digit after '0x'", start, self.pos - start));
            }
            while self.peek().map_or(false, |c| c.is_ascii_hexdigit()) {
                self.pos += 1;
            }
            // Check for hex float: 0x10.1p0
            if self.peek() == Some(b'.') || self.peek().map_or(false, |c| c == b'p' || c == b'P') {
                if self.peek() == Some(b'.') {
                    self.pos += 1;
                    while self.peek().map_or(false, |c| c.is_ascii_hexdigit()) { self.pos += 1; }
                }
                if self.peek().map_or(false, |c| c == b'p' || c == b'P') {
                    self.pos += 1;
                    if self.peek().map_or(false, |c| c == b'+' || c == b'-') { self.pos += 1; }
                    while self.peek().map_or(false, |c| c.is_ascii_digit()) { self.pos += 1; }
                }
                if self.peek().map_or(false, |c| c == b'f' || c == b'F' || c == b'l' || c == b'L') { self.pos += 1; }
                // Parse hex float via string — use 0 as placeholder
                let val = 0.0f64; // TODO: proper hex float parsing
                self.tokens.push(Token::new(TokenKind::Float(val), Span::new(start, self.pos - start)));
                return Ok(());
            }
            let hex_end = self.pos;
            self.skip_int_suffix();
            let text = &self.source[start + 2..hex_end];
            let val = u64::from_str_radix(text, 16).map(|v| v as i64).map_err(|_| {
                self.err(format!("invalid hex number: '{}'", &self.source[start..self.pos]), start, self.pos - start)
            })?;
            self.tokens.push(Token::new(TokenKind::Int(val), Span::new(start, self.pos - start)));
            return Ok(());
        }

        // Binary: 0b...
        if ch == b'0' && self.peek_at(1).map_or(false, |c| c == b'b' || c == b'B') {
            self.pos += 2;
            if !self.peek().map_or(false, |c| c == b'0' || c == b'1') {
                return Err(self.err("expected binary digit after '0b'", start, self.pos - start));
            }
            while self.peek().map_or(false, |c| c == b'0' || c == b'1') {
                self.pos += 1;
            }
            let bin_end = self.pos;
            self.skip_int_suffix();
            let text = &self.source[start + 2..bin_end];
            let val = u64::from_str_radix(text, 2).map(|v| v as i64).map_err(|_| {
                self.err(format!("invalid binary number: '{}'", &self.source[start..self.pos]), start, self.pos - start)
            })?;
            self.tokens.push(Token::new(TokenKind::Int(val), Span::new(start, self.pos - start)));
            return Ok(());
        }

        // Octal: starts with 0
        if ch == b'0' && self.peek_at(1).map_or(false, |c| c.is_ascii_digit()) {
            self.pos += 1;
            while self.peek().map_or(false, |c| c >= b'0' && c <= b'7') {
                self.pos += 1;
            }
            let oct_end = self.pos;
            self.skip_int_suffix();
            let text = &self.source[start + 1..oct_end];
            let val = u64::from_str_radix(text, 8).map(|v| v as i64).map_err(|_| {
                self.err(format!("invalid octal number: '{}'", &self.source[start..self.pos]), start, self.pos - start)
            })?;
            self.tokens.push(Token::new(TokenKind::Int(val), Span::new(start, self.pos - start)));
            return Ok(());
        }

        // Decimal integer or float
        while self.peek().map_or(false, |c| c.is_ascii_digit()) {
            self.pos += 1;
        }

        // Check for float: has dot or exponent
        let is_float = self.peek() == Some(b'.') && self.peek_at(1).map_or(true, |c| c != b'.')
            || self.peek().map_or(false, |c| c == b'e' || c == b'E');

        if is_float {
            if self.peek() == Some(b'.') {
                self.pos += 1;
                while self.peek().map_or(false, |c| c.is_ascii_digit()) {
                    self.pos += 1;
                }
            }
            if self.peek().map_or(false, |c| c == b'e' || c == b'E') {
                self.pos += 1;
                if self.peek().map_or(false, |c| c == b'+' || c == b'-') {
                    self.pos += 1;
                }
                while self.peek().map_or(false, |c| c.is_ascii_digit()) {
                    self.pos += 1;
                }
            }
            let float_end = self.pos;
            // Skip float suffix (f, F, l, L)
            if self.peek().map_or(false, |c| c == b'f' || c == b'F' || c == b'l' || c == b'L') {
                self.pos += 1;
            }
            let text = &self.source[start..float_end];
            let val: f64 = text.parse().map_err(|_| {
                self.err(format!("invalid float: '{}'", &self.source[start..self.pos]), start, self.pos - start)
            })?;
            self.tokens.push(Token::new(TokenKind::Float(val), Span::new(start, self.pos - start)));
        } else {
            let dec_end = self.pos;
            // Check for float suffix: 1f → 1.0f
            if self.peek().map_or(false, |c| c == b'f' || c == b'F')
                && !self.peek_at(1).map_or(false, |c| c.is_ascii_alphanumeric() || c == b'_')
            {
                self.pos += 1; // skip f
                let text = &self.source[start..dec_end];
                let val: f64 = text.parse().unwrap_or(0.0);
                self.tokens.push(Token::new(TokenKind::Float(val), Span::new(start, self.pos - start)));
            } else {
                self.skip_int_suffix();
                let text = &self.source[start..dec_end];
                let val: i64 = text.parse::<i64>()
                    .or_else(|_| text.parse::<u64>().map(|v| v as i64))
                    .map_err(|_| {
                        self.err(format!("invalid number: '{}'", &self.source[start..self.pos]), start, self.pos - start)
                    })?;
                self.tokens.push(Token::new(TokenKind::Int(val), Span::new(start, self.pos - start)));
            }
        }
        Ok(())
    }

    fn skip_int_suffix(&mut self) {
        // ULL, LLU, ull, llu, etc.
        let remaining = &self.source[self.pos..];
        for suffix in &["ULL", "ull", "Ull", "uLL", "LLU", "llu", "llU", "LLu",
                        "UL", "ul", "Ul", "uL", "LU", "lu", "Lu", "lU",
                        "LL", "ll", "U", "u", "L", "l"] {
            if remaining.starts_with(suffix) {
                self.pos += suffix.len();
                return;
            }
        }
    }

    fn read_string(&mut self) -> Result<(), CompileError> {
        let start = self.pos;
        self.pos += 1; // skip opening "
        let mut s = String::new();

        loop {
            if self.pos >= self.bytes.len() || self.bytes[self.pos] == b'\n' {
                return Err(self.err("unclosed string literal", start, 1));
            }
            if self.bytes[self.pos] == b'"' {
                self.pos += 1;
                break;
            }
            if self.bytes[self.pos] == b'\\' {
                self.pos += 1;
                s.push(self.read_escape_char(start)?);
            } else {
                s.push(self.bytes[self.pos] as char);
                self.pos += 1;
            }
        }

        self.tokens.push(Token::new(TokenKind::Str(s), Span::new(start, self.pos - start)));
        Ok(())
    }

    fn read_char_literal(&mut self) -> Result<(), CompileError> {
        let start = self.pos;
        self.pos += 1; // skip opening '

        if self.pos >= self.bytes.len() {
            return Err(self.err("unclosed char literal", start, 1));
        }

        let val = if self.bytes[self.pos] == b'\\' {
            self.pos += 1;
            self.read_escape_char(start)? as i64
        } else {
            // Read all bytes until closing quote (handles multi-byte UTF-8)
            let mut v: i64 = 0;
            while self.pos < self.bytes.len() && self.bytes[self.pos] != b'\'' {
                v = (v << 8) | (self.bytes[self.pos] as i64);
                self.pos += 1;
            }
            v
        };

        if self.pos >= self.bytes.len() || self.bytes[self.pos] != b'\'' {
            return Err(self.err("unclosed char literal", start, self.pos - start));
        }
        self.pos += 1; // skip closing '

        self.tokens.push(Token::new(TokenKind::Char(val), Span::new(start, self.pos - start)));
        Ok(())
    }

    fn read_escape_char(&mut self, start: usize) -> Result<char, CompileError> {
        if self.pos >= self.bytes.len() {
            return Err(self.err("unexpected end of escape sequence", start, 1));
        }
        let ch = self.advance();
        match ch {
            b'a' => Ok('\x07'),
            b'b' => Ok('\x08'),
            b't' => Ok('\t'),
            b'n' => Ok('\n'),
            b'v' => Ok('\x0b'),
            b'f' => Ok('\x0c'),
            b'r' => Ok('\r'),
            b'\\' => Ok('\\'),
            b'\'' => Ok('\''),
            b'"' => Ok('"'),
            b'0' if !self.peek().map_or(false, |c| c >= b'0' && c <= b'7') => Ok('\0'),
            b'x' => {
                // Hex escape
                let mut val = 0u32;
                if !self.peek().map_or(false, |c| c.is_ascii_hexdigit()) {
                    return Err(self.err("invalid hex escape sequence", self.pos - 2, 2));
                }
                while self.peek().map_or(false, |c| c.is_ascii_hexdigit()) {
                    val = val * 16 + hex_digit(self.advance());
                }
                // For wide chars, allow values > 0x10FFFF by truncating to byte
                Ok(char::from_u32(val & 0xFF).unwrap_or(val as u8 as char))
            }
            b'0'..=b'7' => {
                // Octal escape
                let mut val = (ch - b'0') as u32;
                for _ in 0..2 {
                    if self.peek().map_or(false, |c| c >= b'0' && c <= b'7') {
                        val = val * 8 + (self.advance() - b'0') as u32;
                    }
                }
                char::from_u32(val).ok_or_else(|| self.err("invalid octal escape value", start, self.pos - start))
            }
            _ => Ok(ch as char),
        }
    }

    fn read_ident_or_keyword(&mut self) {
        let start = self.pos;
        while self.pos < self.bytes.len()
            && (self.bytes[self.pos].is_ascii_alphanumeric() || self.bytes[self.pos] == b'_'
                || self.bytes[self.pos] == b'$' || self.bytes[self.pos] >= 0x80)
        {
            self.pos += 1;
        }
        let text = &self.source[start..self.pos];
        let span = Span::new(start, self.pos - start);

        if KEYWORDS.contains(&text) {
            self.tokens.push(Token::new(TokenKind::Keyword(text.to_string()), span));
        } else {
            self.tokens.push(Token::new(TokenKind::Ident(text.to_string()), span));
        }
    }

    fn read_punct(&mut self) {
        let remaining = &self.source[self.pos..];
        for &punct in MULTI_CHAR_PUNCTS {
            if remaining.starts_with(punct) {
                let span = Span::new(self.pos, punct.len());
                self.tokens.push(Token::new(TokenKind::Punct(punct.to_string()), span));
                self.pos += punct.len();
                return;
            }
        }
        let ch = self.bytes[self.pos] as char;
        self.tokens.push(Token::new(
            TokenKind::Punct(ch.to_string()),
            Span::new(self.pos, 1),
        ));
        self.pos += 1;
    }

    fn tokenize_all(&mut self) -> Result<Vec<Token>, CompileError> {
        while self.pos < self.bytes.len() {
            let ch = self.bytes[self.pos];

            if ch.is_ascii_whitespace() {
                self.skip_whitespace();
                continue;
            }

            // Line comment
            if ch == b'/' && self.peek_at(1) == Some(b'/') {
                self.skip_line_comment();
                continue;
            }

            // Block comment
            if ch == b'/' && self.peek_at(1) == Some(b'*') {
                self.skip_block_comment()?;
                continue;
            }

            // Number (including floats starting with .)
            if ch.is_ascii_digit() || (ch == b'.' && self.peek_at(1).map_or(false, |c| c.is_ascii_digit())) {
                self.read_number()?;
                continue;
            }

            // String literal
            // Wide/unicode string/char prefixes: L"...", L'...', u"...", U"..."
            if (ch == b'L' || ch == b'u' || ch == b'U')
                && self.peek_at(1).map_or(false, |c| c == b'"' || c == b'\'')
            {
                self.pos += 1; // skip prefix
                if self.bytes[self.pos] == b'"' {
                    self.read_string()?;
                } else {
                    self.read_char_literal()?;
                }
                continue;
            }
            // u8"..." prefix
            if ch == b'u' && self.peek_at(1) == Some(b'8')
                && self.peek_at(2).map_or(false, |c| c == b'"')
            {
                self.pos += 2;
                self.read_string()?;
                continue;
            }

            if ch == b'"' {
                self.read_string()?;
                continue;
            }

            // Char literal
            if ch == b'\'' {
                self.read_char_literal()?;
                continue;
            }

            // Identifier or keyword
            if ch.is_ascii_alphabetic() || ch == b'_' || ch == b'$' || ch >= 0x80 {
                self.read_ident_or_keyword();
                continue;
            }

            // Punctuation
            if ch.is_ascii_punctuation() {
                self.read_punct();
                continue;
            }

            return Err(self.err(
                format!("unexpected character: '{}'", ch as char),
                self.pos, 1,
            ));
        }

        self.tokens.push(Token::new(TokenKind::Eof, Span::new(self.pos, 0)));
        Ok(std::mem::take(&mut self.tokens))
    }
}

fn hex_digit(c: u8) -> u32 {
    match c {
        b'0'..=b'9' => (c - b'0') as u32,
        b'a'..=b'f' => (c - b'a' + 10) as u32,
        b'A'..=b'F' => (c - b'A' + 10) as u32,
        _ => 0,
    }
}

pub fn tokenize(_filename: &str, source: &str) -> Result<Vec<Token>, CompileError> {
    let mut lexer = Lexer::new(source);
    lexer.tokenize_all()
}
