//! The `.lss` lexer — adopted from Lumen's `lumen-style` (`lexer.rs`).
//!
//! Whitespace is significant between selector parts (`#a .b` is a
//! descendant combinator, `#a.b` a compound), so each token records
//! whether whitespace or a comment preceded it. Two changes from Lumen's:
//! a number keeps its **text**, so the lowering parses it straight to
//! `f32` (`ast::Value::Number`); and a character no rule uses becomes
//! [`Tk::Other`] instead of an empty identifier, so the parser can name
//! it.

use crate::ast::{Span, Unit};

/// A lexical token.
#[derive(Clone, Debug, PartialEq)]
pub enum Tk {
    /// An identifier: letters, digits, `-`, `_`.
    Ident(String),
    /// `@keyword`.
    At(String),
    /// `#hash` — an id selector or a hex colour; the parser decides.
    Hash(String),
    /// `$ident`, a token reference.
    Var(String),
    /// A double-quoted string.
    Str(String),
    /// A number as written, with its unit.
    Num(String, Unit),
    /// A number with a unit v1 does not know (`12em`).
    NumBadUnit(String, String),
    Dot,
    Colon,
    Semi,
    Comma,
    LBrace,
    RBrace,
    LParen,
    RParen,
    /// `>`, the child combinator — refused.
    Gt,
    /// `&`, a nested rule — refused.
    Amp,
    /// `!`, as in `!important` — refused.
    Bang,
    /// `*`.
    Star,
    /// Any other character (`+`, `~`, `<`, `=`, …).
    Other(char),
    /// End of input.
    Eof,
}

/// A token, where it starts, and whether trivia preceded it.
#[derive(Clone, Debug)]
pub struct Token {
    pub kind: Tk,
    pub span: Span,
    pub ws_before: bool,
}

struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    line: u32,
    col: u32,
}

/// Tokenize `src`. Lexing never fails; what it cannot use becomes a
/// token the parser refuses with a span.
pub fn lex(src: &str) -> Vec<Token> {
    let mut lx = Lexer { src: src.as_bytes(), pos: 0, line: 1, col: 1 };
    let mut out = Vec::new();
    loop {
        let ws = lx.skip_trivia();
        let span = Span { line: lx.line, col: lx.col };
        let kind = lx.next_kind();
        let eof = kind == Tk::Eof;
        out.push(Token { kind, span, ws_before: ws });
        if eof {
            break;
        }
    }
    out
}

impl Lexer<'_> {
    fn peek(&self) -> u8 {
        self.src.get(self.pos).copied().unwrap_or(0)
    }
    fn peek2(&self) -> u8 {
        self.src.get(self.pos + 1).copied().unwrap_or(0)
    }
    fn bump(&mut self) -> u8 {
        let c = self.peek();
        self.pos += 1;
        if c == b'\n' {
            self.line += 1;
            self.col = 1;
        } else if c & 0xC0 != 0x80 {
            // Columns count characters, not UTF-8 continuation bytes.
            self.col += 1;
        }
        c
    }

    /// Skip whitespace and `//` / `/* */` comments; whether any was.
    fn skip_trivia(&mut self) -> bool {
        let start = self.pos;
        loop {
            match self.peek() {
                b' ' | b'\t' | b'\r' | b'\n' => {
                    self.bump();
                }
                b'/' if self.peek2() == b'/' => {
                    while self.peek() != b'\n' && self.pos < self.src.len() {
                        self.bump();
                    }
                }
                b'/' if self.peek2() == b'*' => {
                    self.bump();
                    self.bump();
                    while !(self.peek() == b'*' && self.peek2() == b'/') && self.pos < self.src.len() {
                        self.bump();
                    }
                    self.bump();
                    self.bump();
                }
                _ => break,
            }
        }
        self.pos != start
    }

    fn next_kind(&mut self) -> Tk {
        if self.pos >= self.src.len() {
            return Tk::Eof;
        }
        let c = self.peek();
        let single = match c {
            b'{' => Some(Tk::LBrace),
            b'}' => Some(Tk::RBrace),
            b'(' => Some(Tk::LParen),
            b')' => Some(Tk::RParen),
            b':' => Some(Tk::Colon),
            b';' => Some(Tk::Semi),
            b',' => Some(Tk::Comma),
            b'&' => Some(Tk::Amp),
            b'!' => Some(Tk::Bang),
            b'*' => Some(Tk::Star),
            b'>' => Some(Tk::Gt),
            _ => None,
        };
        if let Some(t) = single {
            self.bump();
            return t;
        }
        match c {
            b'@' => {
                self.bump();
                Tk::At(self.read_ident())
            }
            b'#' => {
                self.bump();
                Tk::Hash(self.read_ident())
            }
            b'$' => {
                self.bump();
                Tk::Var(self.read_ident())
            }
            b'"' => Tk::Str(self.read_string()),
            b'.' if self.peek2().is_ascii_digit() => self.read_number(),
            b'.' => {
                self.bump();
                Tk::Dot
            }
            b'0'..=b'9' => self.read_number(),
            b'-' if self.peek2().is_ascii_digit() || self.peek2() == b'.' => self.read_number(),
            c if is_ident_start(c) => Tk::Ident(self.read_ident()),
            _ => {
                // One character, decoded, so a non-ASCII one is named
                // whole rather than as its first byte.
                let rest = core::str::from_utf8(&self.src[self.pos..]).unwrap_or("?");
                let ch = rest.chars().next().unwrap_or('?');
                for _ in 0..ch.len_utf8() {
                    self.bump();
                }
                Tk::Other(ch)
            }
        }
    }

    fn read_ident(&mut self) -> String {
        let mut s = String::new();
        if self.peek() == b'-' {
            s.push('-');
            self.bump();
        }
        while is_ident_continue(self.peek()) {
            s.push(self.bump() as char);
        }
        s
    }

    fn read_string(&mut self) -> String {
        self.bump(); // opening quote
        let start = self.pos;
        while self.peek() != b'"' && self.pos < self.src.len() {
            self.bump();
        }
        let s = String::from_utf8_lossy(&self.src[start..self.pos]).into_owned();
        self.bump(); // closing quote, if present
        s
    }

    fn read_number(&mut self) -> Tk {
        let mut s = String::new();
        if self.peek() == b'-' {
            s.push('-');
            self.bump();
        }
        while self.peek().is_ascii_digit() || self.peek() == b'.' {
            s.push(self.bump() as char);
        }
        if self.peek() == b'%' {
            self.bump();
            return Tk::Num(s, Unit::Percent);
        }
        let mut suffix = String::new();
        while is_ident_continue(self.peek()) {
            suffix.push(self.bump() as char);
        }
        match suffix.as_str() {
            "" => Tk::Num(s, Unit::None),
            "px" => Tk::Num(s, Unit::Px),
            _ => Tk::NumBadUnit(s, suffix),
        }
    }
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_ident_continue(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'-' || c == b'_'
}
