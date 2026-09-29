//! The `.lss` parser — adopted from Lumen's `lumen-style` (`parser.rs`),
//! trimmed to v1's grammar (`docs/plan-lss.md` §2.1).
//!
//! Recursive descent over [`crate::lexer`]'s tokens, collecting every
//! error it can find rather than stopping at the first, each with the
//! `line:col` it starts at. **What v1 does not have is refused, not
//! skipped**: a compound selector, a combinator, a `:state`, a nested
//! `&`, `!important`, `@media`, `@keyframes` and `@theme` each get their
//! own message naming the plan's §4. A stylesheet that silently dropped a
//! rule would be the vacuous-gate shape in another medium.
//!
//! What the parser does not judge is *meaning*: whether a property exists
//! and whether its value fits is the lowering's (T61.2), because
//! that is where the property table is.

use crate::ast::*;
use crate::lexer::{lex, Tk, Token};

/// Where the refusals point: the plan section that says why each is out.
const SCOPE: &str = "docs/plan-lss.md §4";

/// One error, and where in the file it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub span: Span,
    pub msg: String,
}

impl Error {
    pub(crate) fn new(span: Span, msg: impl Into<String>) -> Error {
        Error { span, msg: msg.into() }
    }

    /// `file:line:col: message` — the form every consumer prints, so a
    /// terminal or an editor can jump to it.
    pub fn at(&self, file: &str) -> String {
        format!("{file}:{}:{}: {}", self.span.line, self.span.col, self.msg)
    }
}

/// Parse `src`. The stylesheet is usable only if the error list is empty.
pub fn parse(src: &str) -> (Stylesheet, Vec<Error>) {
    let mut p = Parser { toks: lex(src), pos: 0, errors: Vec::new() };
    let sheet = p.stylesheet();
    (sheet, p.errors)
}

struct Parser {
    toks: Vec<Token>,
    pos: usize,
    errors: Vec<Error>,
}

/// A selector part as written, before v1 decides whether it may have it.
enum Raw {
    Part(Part),
    State(String),
}

impl Parser {
    fn cur(&self) -> &Tk {
        &self.toks[self.pos].kind
    }
    fn span(&self) -> Span {
        self.toks[self.pos].span
    }
    fn ws_before(&self) -> bool {
        self.toks[self.pos].ws_before
    }
    fn bump(&mut self) -> Tk {
        let k = self.toks[self.pos].kind.clone();
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        k
    }
    fn at(&self, k: &Tk) -> bool {
        self.cur() == k
    }
    fn eof(&self) -> bool {
        matches!(self.cur(), Tk::Eof)
    }
    fn err(&mut self, span: Span, msg: impl Into<String>) {
        self.errors.push(Error::new(span, msg));
    }

    /// Skip a balanced `{ … }` starting at the current `{`.
    fn skip_block(&mut self) {
        if !self.at(&Tk::LBrace) {
            return;
        }
        let mut depth = 0usize;
        loop {
            match self.bump() {
                Tk::LBrace => depth += 1,
                Tk::RBrace => {
                    depth -= 1;
                    if depth == 0 {
                        return;
                    }
                }
                Tk::Eof => return,
                _ => {}
            }
        }
    }

    /// Skip to the end of whatever starts here: through a balanced block,
    /// or past a `;`, whichever comes first.
    fn skip_statement(&mut self) {
        while !self.eof() {
            match self.cur() {
                Tk::LBrace => return self.skip_block(),
                Tk::Semi => {
                    self.bump();
                    return;
                }
                Tk::RBrace => return,
                _ => {
                    self.bump();
                }
            }
        }
    }

    fn stylesheet(&mut self) -> Stylesheet {
        let mut items = Vec::new();
        let mut sheets: Vec<(String, Span)> = Vec::new();
        while !self.eof() {
            let before = self.pos;
            if let Tk::At(kw) = self.cur().clone() {
                match kw.as_str() {
                    "tokens" => items.push(Item::Tokens(self.tokens_block())),
                    "sheet" => {
                        if let Some(b) = self.sheet_block() {
                            if let Some((_, first)) = sheets.iter().find(|(n, _)| *n == b.name) {
                                let msg = format!(
                                    "`@sheet {}` is defined twice (first at {}:{})",
                                    b.name, first.line, first.col
                                );
                                self.err(b.span, msg);
                            } else {
                                sheets.push((b.name.clone(), b.span));
                                items.push(Item::Sheet(b));
                            }
                        }
                    }
                    _ => self.refuse_at_rule(&kw),
                }
            } else if matches!(self.cur(), Tk::RBrace) {
                let span = self.span();
                self.err(span, "unmatched `}`");
                self.bump();
            } else if let Some(rule) = self.rule() {
                items.push(Item::Rule(rule));
            }
            if self.pos == before {
                let span = self.span();
                let msg = format!("unexpected {}", describe(self.cur()));
                self.err(span, msg);
                self.bump();
            }
        }
        Stylesheet { items }
    }

    /// An at-rule v1 does not have: say which and why, and skip it.
    fn refuse_at_rule(&mut self, kw: &str) {
        let span = self.span();
        let msg = match kw {
            "media" | "keyframes" => format!(
                "`@{kw}` is not in v1: it needs cascade machinery this framework does not have ({SCOPE})"
            ),
            "theme" => format!(
                "`@theme` is not in v1: a colour that follows the theme is a palette role -- \
                 `$page`, `$surface`, `$text`, `$accent` ... -- which `Host::set_palette` \
                 re-resolves ({SCOPE})"
            ),
            _ => format!("unknown at-rule `@{kw}`: v1 has `@tokens` and `@sheet`"),
        };
        self.err(span, msg);
        self.bump();
        self.skip_statement();
    }

    fn tokens_block(&mut self) -> Vec<Binding> {
        self.bump(); // @tokens
        let mut out: Vec<Binding> = Vec::new();
        if !self.expect_open("`@tokens`") {
            return out;
        }
        while !self.at(&Tk::RBrace) && !self.eof() {
            let span = self.span();
            let name = match self.cur().clone() {
                Tk::Ident(s) => {
                    self.bump();
                    s
                }
                other => {
                    self.err(span, format!("expected a token name, found {}", describe(&other)));
                    self.skip_statement();
                    continue;
                }
            };
            if !self.at(&Tk::Colon) {
                let (s, found) = (self.span(), describe(self.cur()));
                self.err(s, format!("expected `:` after the token name `{name}`, found {found}"));
                self.skip_statement();
                continue;
            }
            self.bump();
            let Some(value) = self.value(&format!("`${name}`")) else {
                self.skip_statement();
                continue;
            };
            self.end_declaration();
            if let Some(first) = out.iter().find(|b| b.name == name) {
                let msg = format!(
                    "token `${name}` is defined twice (first at {}:{})",
                    first.span.line, first.span.col
                );
                self.err(span, msg);
                continue;
            }
            out.push(Binding { name, value, span });
        }
        self.expect_close();
        out
    }

    fn sheet_block(&mut self) -> Option<SheetBlock> {
        let span = self.span();
        self.bump(); // @sheet
        let name = match self.cur().clone() {
            Tk::Ident(s) => {
                self.bump();
                s
            }
            other => {
                let s = self.span();
                self.err(s, format!("expected a name after `@sheet`, found {}", describe(&other)));
                self.skip_statement();
                return None;
            }
        };
        if !self.expect_open(&format!("`@sheet {name}`")) {
            return None;
        }
        let mut rules = Vec::new();
        while !self.at(&Tk::RBrace) && !self.eof() {
            let before = self.pos;
            if let Tk::At(kw) = self.cur().clone() {
                let s = self.span();
                match kw.as_str() {
                    "tokens" | "sheet" => {
                        self.err(s, format!("`@{kw}` belongs at the top level, not inside `@sheet {name}`"));
                        self.bump();
                        self.skip_statement();
                    }
                    _ => self.refuse_at_rule(&kw),
                }
            } else if let Some(r) = self.rule() {
                rules.push(r);
            }
            if self.pos == before {
                let s = self.span();
                let msg = format!("unexpected {}", describe(self.cur()));
                self.err(s, msg);
                self.bump();
            }
        }
        self.expect_close();
        Some(SheetBlock { name, rules, span })
    }

    /// Consume `{`, or report what was there instead.
    fn expect_open(&mut self, what: &str) -> bool {
        if self.at(&Tk::LBrace) {
            self.bump();
            return true;
        }
        let (s, found) = (self.span(), describe(self.cur()));
        self.err(s, format!("expected `{{` after {what}, found {found}"));
        self.skip_statement();
        false
    }

    fn expect_close(&mut self) {
        if self.at(&Tk::RBrace) {
            self.bump();
        } else {
            let s = self.span();
            self.err(s, "expected `}` before the end of the file");
        }
    }

    /// `selectors { declarations }`. `None` when a selector was refused —
    /// the body is still parsed, so its own errors are reported too.
    fn rule(&mut self) -> Option<Rule> {
        let selectors = self.selector_list();
        if !self.at(&Tk::LBrace) {
            if selectors.is_some() {
                let (s, found) = (self.span(), describe(self.cur()));
                self.err(s, format!("expected `{{` after the selector, found {found}"));
            }
            self.skip_statement();
            return None;
        }
        self.bump();
        let mut declarations = Vec::new();
        while !self.at(&Tk::RBrace) && !self.eof() {
            let before = self.pos;
            match self.cur().clone() {
                Tk::Amp => {
                    let s = self.span();
                    self.err(
                        s,
                        format!(
                            "nested rule `&`: v1 has no nesting -- write the selector out as a rule of \
                             its own ({SCOPE})"
                        ),
                    );
                    self.skip_statement();
                }
                Tk::At(kw) => {
                    let s = self.span();
                    self.err(s, format!("`@{kw}` inside a rule: at-rules belong at the top level"));
                    self.bump();
                    self.skip_statement();
                }
                _ => {
                    if let Some(d) = self.declaration() {
                        declarations.push(d);
                    }
                }
            }
            if self.pos == before {
                self.bump();
            }
        }
        self.expect_close();
        Some(Rule { selectors: selectors?, declarations })
    }

    fn selector_list(&mut self) -> Option<Vec<Selector>> {
        let mut sels = Vec::new();
        let mut ok = true;
        loop {
            match self.selector() {
                Some(s) => sels.push(s),
                None => ok = false,
            }
            if self.at(&Tk::Comma) {
                self.bump();
                continue;
            }
            break;
        }
        ok.then_some(sels)
    }

    fn starts_part(&self) -> bool {
        matches!(self.cur(), Tk::Hash(_) | Tk::Dot | Tk::Colon | Tk::Star | Tk::Ident(_))
    }

    fn raw_part(&mut self) -> Option<Raw> {
        match self.cur().clone() {
            Tk::Hash(s) => {
                self.bump();
                Some(Raw::Part(Part::Id(s)))
            }
            Tk::Dot => {
                self.bump();
                match self.cur().clone() {
                    Tk::Ident(s) => {
                        self.bump();
                        Some(Raw::Part(Part::Class(s)))
                    }
                    _ => Some(Raw::Part(Part::Class(String::new()))),
                }
            }
            Tk::Colon => {
                self.bump();
                match self.cur().clone() {
                    Tk::Ident(s) => {
                        self.bump();
                        Some(Raw::State(s))
                    }
                    _ => Some(Raw::State(String::new())),
                }
            }
            Tk::Star => {
                self.bump();
                Some(Raw::Part(Part::Any))
            }
            Tk::Ident(s) => {
                self.bump();
                Some(Raw::Part(Part::Kind(s)))
            }
            _ => None,
        }
    }

    /// The source text of tokens `from..self.pos`, for a message.
    fn text(&self, from: usize) -> String {
        let mut s = String::new();
        for (i, t) in self.toks[from..self.pos].iter().enumerate() {
            if i > 0 && t.ws_before {
                s.push(' ');
            }
            s.push_str(&token_text(&t.kind));
        }
        s
    }

    /// One selector. v1 takes exactly one simple part; anything more is
    /// refused here, by kind, and the rest of the selector is consumed.
    fn selector(&mut self) -> Option<Selector> {
        let span = self.span();
        let start = self.pos;
        let first = match self.raw_part() {
            Some(r) => r,
            None => {
                let msg = format!(
                    "expected a selector (`#id`, `.class`, a kind or `*`), found {}",
                    describe(self.cur())
                );
                self.err(span, msg);
                return None;
            }
        };
        // A compound: more parts with no whitespace between them.
        let mut states = Vec::new();
        let mut parts = 1;
        if let Raw::State(s) = &first {
            states.push(s.clone());
        }
        while self.starts_part() && !self.ws_before() {
            match self.raw_part() {
                Some(Raw::State(s)) => states.push(s),
                Some(Raw::Part(_)) => parts += 1,
                None => break,
            }
        }
        let compound = self.text(start);
        let mut refused = false;
        if !states.is_empty() {
            self.err(
                span,
                format!(
                    "`:{}` is a state selector, which v1 does not have -- for hover and press, the \
                     declarations are `hover-dim` and `pressed-dim` ({SCOPE})",
                    states[0]
                ),
            );
            refused = true;
        } else if parts > 1 {
            self.err(
                span,
                format!(
                    "compound selector `{compound}`: v1 takes one simple selector per rule -- `#id`, \
                     `.class`, a kind or `*` ({SCOPE})"
                ),
            );
            refused = true;
        }
        // A combinator: `>`, or whitespace and another part.
        let child = self.at(&Tk::Gt);
        if child || (self.ws_before() && self.starts_part()) {
            while !matches!(self.cur(), Tk::LBrace | Tk::Comma | Tk::Eof) {
                self.bump();
            }
            let whole = self.text(start);
            let msg = if child {
                format!(
                    "child combinator `>` in `{whole}`: v1 matches a node by its own id, class or \
                     kind, not by its parent ({SCOPE})"
                )
            } else {
                format!(
                    "descendant combinator in `{whole}`: v1 matches a node by its own id, class or \
                     kind, not by its ancestors ({SCOPE})"
                )
            };
            if !refused {
                self.err(span, msg);
            }
            return None;
        }
        if refused {
            return None;
        }
        let Raw::Part(part) = first else { unreachable!("a state was refused above") };
        if matches!(&part, Part::Class(c) if c.is_empty()) {
            self.err(span, "expected a class name after `.`");
            return None;
        }
        Some(Selector { part, span })
    }

    fn declaration(&mut self) -> Option<Declaration> {
        let span = self.span();
        let property = match self.cur().clone() {
            Tk::Ident(s) => {
                self.bump();
                s
            }
            other => {
                self.err(span, format!("expected a property name, found {}", describe(&other)));
                self.skip_statement();
                return None;
            }
        };
        if !self.at(&Tk::Colon) {
            let (s, found) = (self.span(), describe(self.cur()));
            self.err(s, format!("expected `:` after `{property}`, found {found}"));
            self.skip_statement();
            return None;
        }
        self.bump();
        let value_span = self.span();
        let value = self.value(&format!("`{property}`"));
        if self.at(&Tk::Bang) {
            let s = self.span();
            self.err(s, format!("`!important` is not in v1: specificity and source order decide ({SCOPE})"));
            self.bump();
            if matches!(self.cur(), Tk::Ident(i) if i == "important") {
                self.bump();
            }
        }
        self.end_declaration();
        Some(Declaration { property, value: value?, span, value_span })
    }

    /// A declaration ends in `;`, or at the `}` that closes its rule.
    fn end_declaration(&mut self) {
        match self.cur() {
            Tk::Semi => {
                self.bump();
            }
            Tk::RBrace | Tk::Eof => {}
            _ => {
                let (s, found) = (self.span(), describe(self.cur()));
                self.err(s, format!("expected `;`, found {found}"));
                while !matches!(self.cur(), Tk::Semi | Tk::RBrace | Tk::Eof) {
                    self.bump();
                }
                if self.at(&Tk::Semi) {
                    self.bump();
                }
            }
        }
    }

    /// A value: a run of atoms up to `;`, `}`, `!` or `)`.
    fn value(&mut self, of: &str) -> Option<Value> {
        let span = self.span();
        let mut atoms = Vec::new();
        let mut ok = true;
        loop {
            if matches!(self.cur(), Tk::Semi | Tk::RBrace | Tk::Bang | Tk::RParen | Tk::Eof) {
                break;
            }
            if self.at(&Tk::Comma) {
                let s = self.span();
                self.err(s, format!("`,` in the value of {of}: a list is separated by spaces"));
                self.bump();
                ok = false;
                continue;
            }
            match self.atom() {
                Some(Some(a)) => atoms.push(a),
                Some(None) => ok = false,
                None => {
                    let (s, found) = (self.span(), describe(self.cur()));
                    self.err(s, format!("unexpected {found} in the value of {of}"));
                    self.bump();
                    ok = false;
                }
            }
        }
        if atoms.is_empty() {
            if ok {
                self.err(span, format!("expected a value for {of}"));
            }
            return None;
        }
        if !ok {
            return None;
        }
        Some(if atoms.len() == 1 { atoms.pop().expect("one atom") } else { Value::List(atoms) })
    }

    /// One atom. `None`: nothing here is an atom. `Some(None)`: there was
    /// one, and it was refused (already reported).
    fn atom(&mut self) -> Option<Option<Value>> {
        let span = self.span();
        Some(match self.cur().clone() {
            Tk::Num(n, u) => {
                self.bump();
                Some(Value::Number(n, u))
            }
            Tk::NumBadUnit(n, unit) => {
                self.bump();
                self.err(span, format!("unknown unit in `{n}{unit}`: v1 takes `px` or a bare number"));
                None
            }
            Tk::Hash(h) => {
                self.bump();
                match hex_colour(&h) {
                    Some(c) => Some(Value::Color(c)),
                    None => {
                        self.err(span, format!("invalid colour `#{h}`: v1 takes `#rrggbb` or `#rrggbbaa`"));
                        None
                    }
                }
            }
            Tk::Var(v) => {
                self.bump();
                Some(Value::Var(v))
            }
            Tk::Str(s) => {
                self.bump();
                Some(Value::Str(s))
            }
            Tk::Ident(name) => {
                self.bump();
                if self.at(&Tk::LParen) && !self.ws_before() {
                    self.bump();
                    let mut args = Vec::new();
                    let mut ok = true;
                    while !self.at(&Tk::RParen) && !self.eof() && !self.at(&Tk::Semi) && !self.at(&Tk::RBrace) {
                        match self.value(&format!("`{name}(..)`")) {
                            Some(v) => args.push(v),
                            None => ok = false,
                        }
                        if self.at(&Tk::Comma) {
                            self.bump();
                        }
                    }
                    if self.at(&Tk::RParen) {
                        self.bump();
                    } else {
                        let s = self.span();
                        self.err(s, format!("expected `)` to close `{name}(`"));
                        return Some(None);
                    }
                    ok.then_some(Value::Function(name, args))
                } else {
                    Some(Value::Keyword(name))
                }
            }
            _ => return None,
        })
    }
}

/// `#rrggbb` or `#rrggbbaa` as RGBA.
fn hex_colour(h: &str) -> Option<[u8; 4]> {
    if !(h.len() == 6 || h.len() == 8) || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?, if h.len() == 8 { byte(6)? } else { 0xff }])
}

/// A token as it was written, for a message quoting source.
fn token_text(t: &Tk) -> String {
    match t {
        Tk::Ident(s) => s.clone(),
        Tk::At(s) => format!("@{s}"),
        Tk::Hash(s) => format!("#{s}"),
        Tk::Var(s) => format!("${s}"),
        Tk::Str(s) => format!("\"{s}\""),
        Tk::Num(n, Unit::None) => n.clone(),
        Tk::Num(n, Unit::Px) => format!("{n}px"),
        Tk::Num(n, Unit::Percent) => format!("{n}%"),
        Tk::NumBadUnit(n, u) => format!("{n}{u}"),
        Tk::Dot => ".".into(),
        Tk::Colon => ":".into(),
        Tk::Semi => ";".into(),
        Tk::Comma => ",".into(),
        Tk::LBrace => "{".into(),
        Tk::RBrace => "}".into(),
        Tk::LParen => "(".into(),
        Tk::RParen => ")".into(),
        Tk::Gt => ">".into(),
        Tk::Amp => "&".into(),
        Tk::Bang => "!".into(),
        Tk::Star => "*".into(),
        Tk::Other(c) => c.to_string(),
        Tk::Eof => String::new(),
    }
}

/// A token for "found X" in a message.
fn describe(t: &Tk) -> String {
    match t {
        Tk::Eof => "the end of the file".into(),
        other => format!("`{}`", token_text(other)),
    }
}

impl core::fmt::Display for Stylesheet {
    /// The sheet as canonical `.lss` — what the round-trip test parses
    /// back.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        fn rule(f: &mut core::fmt::Formatter<'_>, r: &Rule, indent: &str) -> core::fmt::Result {
            let sels: Vec<String> = r.selectors.iter().map(|s| s.part.to_string()).collect();
            writeln!(f, "{indent}{} {{", sels.join(", "))?;
            for d in &r.declarations {
                writeln!(f, "{indent}    {}: {};", d.property, d.value)?;
            }
            writeln!(f, "{indent}}}")
        }
        for item in &self.items {
            match item {
                Item::Rule(r) => rule(f, r, "")?,
                Item::Tokens(bs) => {
                    writeln!(f, "@tokens {{")?;
                    for b in bs {
                        writeln!(f, "    {}: {};", b.name, b.value)?;
                    }
                    writeln!(f, "}}")?;
                }
                Item::Sheet(s) => {
                    writeln!(f, "@sheet {} {{", s.name)?;
                    for r in &s.rules {
                        rule(f, r, "    ")?;
                    }
                    writeln!(f, "}}")?;
                }
            }
        }
        Ok(())
    }
}
