//! Lowering: syntax to rules — T61.2 (`docs/plan-lss.md` §2.1).
//!
//! The property table ([`PROPERTIES`]: a CSS-shaped declaration name →
//! the `Patch` setter it calls, and the values that setter can take),
//! `@tokens` substitution, the palette roles as tokens, and `@sheet`
//! blocks. The output, [`Sheet`], is this crate's own mirror of
//! `lumen_static::Rule` — the core cannot be a dependency here (the
//! crate's manifest says why) — and it has **two** consumers that turn it
//! into real rules independently: `include_lss!` as `const` tokens, and
//! `lumen_static_shell::lss` as values at run time. T61.5's parity gate
//! is what holds the two to each other.
//!
//! **`scripts/check_lss_setters.sh` holds the table to the core**: every
//! `Patch` setter must be named here, and every name here must be a
//! setter, or `just static` fails. The mirrored enums ([`Align`],
//! [`Cursor`], [`Role`]) are held to theirs the same way.

use std::collections::HashMap;

use crate::ast::{self, Item, Part, Span, Unit, Value};
use crate::parser::Error;

/// A lowered stylesheet: the top-level rules, and each `@sheet` block.
#[derive(Clone, Debug, PartialEq)]
pub struct Sheet {
    pub base: Vec<Rule>,
    pub blocks: Vec<Block>,
}

/// `@sheet name { … }`, lowered.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub name: String,
    pub rules: Vec<Rule>,
    pub span: Span,
}

/// One rule: `lumen_static::Rule::new(selector, Patch::new().setter(..)…)`.
/// A comma list lowers to one of these per selector, in order.
#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub selector: Part,
    /// The setters, in declaration order — which is the order they are
    /// called in, so `color: #fff; color: $text;` ends as the role, as
    /// the two setter calls would.
    pub setters: Vec<Setter>,
    /// Where the selector starts.
    pub span: Span,
}

/// One `Patch` setter call.
#[derive(Clone, Debug, PartialEq)]
pub struct Setter {
    /// The `Patch` method, by name: `"background"`, `"color_role"`, …
    pub method: &'static str,
    pub value: Val,
}

/// A setter's argument, typed as the setter takes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Val {
    F32(f32),
    U8(u8),
    Bool(bool),
    /// `Color(r, g, b, a)`.
    Color([u8; 4]),
    Role(Role),
    Length(Length),
    Edges(Edges),
    Align(Align),
    Cursor(Cursor),
}

/// `lumen_static::style::Length`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Length {
    Fixed(f32),
    Shrink,
    FillPortion(u16),
}

/// `lumen_static::style::Edges`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edges {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

/// Declares a mirror of one of the core's fieldless enums, with the
/// `.lss` keyword for each variant and the variant's Rust name.
macro_rules! mirror {
    ($(#[$m:meta])* $name:ident { $($var:ident = $kw:literal),* $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name { $($var),* }

        impl $name {
            /// Every variant, in the core's declaration order.
            pub const ALL: &'static [$name] = &[$($name::$var),*];

            /// The variant's name in the core — what a lowering emits.
            pub fn variant(self) -> &'static str {
                match self { $($name::$var => stringify!($var)),* }
            }

            /// The word `.lss` spells it with.
            pub fn keyword(self) -> &'static str {
                match self { $($name::$var => $kw),* }
            }

            fn from_keyword(k: &str) -> Option<$name> {
                Self::ALL.iter().copied().find(|v| v.keyword() == k)
            }
        }
    };
}

mirror!(
    /// `lumen_static::style::Align`.
    Align { Start = "start", Center = "center", End = "end", Fill = "fill" }
);

mirror!(
    /// `lumen_static::style::Cursor`.
    Cursor {
        Default = "default",
        Text = "text",
        Pointer = "pointer",
        Grab = "grab",
        Grabbing = "grabbing",
        ColResize = "col-resize",
        RowResize = "row-resize",
        NotAllowed = "not-allowed",
    }
);

mirror!(
    /// `lumen_static::style::Role` — spelled `$page`, `$on-accent`, …: a
    /// colour a `.lss` names by role follows `Host::set_palette`, exactly
    /// as `Patch::color_role` does (M25/T25.11).
    Role {
        Page = "page",
        Surface = "surface",
        Control = "control",
        Text = "text",
        Muted = "muted",
        Accent = "accent",
        OnAccent = "on-accent",
        Focus = "focus",
        Raised = "raised",
        Border = "border",
        Track = "track",
        Placeholder = "placeholder",
        Scrim = "scrim",
        Danger = "danger",
        OnDanger = "on-danger",
    }
);

/// What a declaration's value must be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Takes {
    /// `Length`: `12px`, `fixed(12px)`, `fill`, `fill(2)`, `shrink`.
    Length,
    /// `Edges`: one to four px lengths, in CSS order.
    Edges,
    /// An `f32` in px: `12px`.
    Px,
    /// A bare `f32`: `600`, `0.07`.
    Number,
    /// A `u8` in px: `2px`.
    Byte,
    /// A `Color`, or a palette role (the property's `role` setter).
    Colour,
    Align,
    Cursor,
    Bool,
}

impl Takes {
    fn describe(self) -> &'static str {
        match self {
            Takes::Length => "a length: `12px`, `fixed(12px)`, `fill`, `fill(2)` or `shrink`",
            Takes::Edges => "one to four lengths in px, in CSS order: `4px`, `4px 8px`, `4px 8px 2px 1px`",
            Takes::Px => "a length in px, like `12px`",
            Takes::Number => "a bare number, like `600` or `0.5`",
            Takes::Byte => "a whole number of px from 0 to 255, like `2px`",
            Takes::Colour => "a colour: `#rrggbb`, `#rrggbbaa`, or a palette role like `$accent`",
            Takes::Align => "`start`, `center`, `end` or `fill`",
            Takes::Cursor => {
                "`default`, `text`, `pointer`, `grab`, `grabbing`, `col-resize`, `row-resize` or `not-allowed`"
            }
            Takes::Bool => "`true` or `false`",
        }
    }
}

/// One declaration v1 accepts.
#[derive(Clone, Copy, Debug)]
pub struct Property {
    /// The `.lss` name.
    pub name: &'static str,
    /// The `Patch` setter it calls.
    pub setter: &'static str,
    /// The setter a palette role calls instead, for a colour.
    pub role_setter: Option<&'static str>,
    pub takes: Takes,
}

/// **The property table**: every declaration v1 accepts, and the `Patch`
/// setter it lowers to. CSS-shaped names where CSS has one.
///
/// One entry per line in this exact shape: `scripts/check_lss_setters.sh`
/// reads the `setter:` and `role_setter:` strings to prove every `Patch`
/// setter has a declaration.
pub const PROPERTIES: &[Property] = &[
    Property { name: "width", setter: "width", role_setter: None, takes: Takes::Length },
    Property { name: "height", setter: "height", role_setter: None, takes: Takes::Length },
    Property { name: "padding", setter: "padding", role_setter: None, takes: Takes::Edges },
    Property { name: "spacing", setter: "spacing", role_setter: None, takes: Takes::Px },
    Property { name: "align", setter: "align", role_setter: None, takes: Takes::Align },
    Property { name: "visible", setter: "visible", role_setter: None, takes: Takes::Bool },
    Property { name: "cursor", setter: "cursor", role_setter: None, takes: Takes::Cursor },
    Property { name: "focus-ring", setter: "focus_ring", role_setter: None, takes: Takes::Byte },
    Property { name: "color", setter: "color", role_setter: Some("color_role"), takes: Takes::Colour },
    Property { name: "background", setter: "background", role_setter: Some("background_role"), takes: Takes::Colour },
    Property { name: "border-width", setter: "border_width", role_setter: None, takes: Takes::Byte },
    Property { name: "border-color", setter: "border_color", role_setter: Some("border_color_role"), takes: Takes::Colour },
    Property { name: "border-radius", setter: "radius", role_setter: None, takes: Takes::Px },
    Property { name: "font-size", setter: "font_size", role_setter: None, takes: Takes::Px },
    Property { name: "font-weight", setter: "font_weight", role_setter: None, takes: Takes::Number },
    Property { name: "box-shadow", setter: "shadow", role_setter: None, takes: Takes::Px },
    Property { name: "pressed-dim", setter: "pressed_dim", role_setter: None, takes: Takes::Number },
    Property { name: "hover-dim", setter: "hover_dim", role_setter: None, takes: Takes::Number },
    Property { name: "disabled", setter: "disabled", role_setter: None, takes: Takes::Bool },
];

/// Names a CSS writer reaches for that v1 spells differently, or does
/// not have: a hint the edit distance cannot find.
const ALIASES: &[(&str, &str)] = &[
    ("gap", "did you mean `spacing`?"),
    ("radius", "did you mean `border-radius`?"),
    ("background-color", "did you mean `background`?"),
    ("shadow", "did you mean `box-shadow`?"),
    ("margin", "v1 has no margin: `padding` on the parent, or `spacing` between siblings"),
    ("opacity", "v1 has no opacity"),
];

/// Parse and lower `src`: every error, or the sheet.
pub fn compile(src: &str) -> Result<Sheet, Vec<Error>> {
    let (sheet, mut errors) = crate::parser::parse(src);
    match lower(&sheet) {
        Ok(s) if errors.is_empty() => Ok(s),
        Ok(_) => Err(errors),
        Err(mut e) => {
            errors.append(&mut e);
            errors.sort_by_key(|e| (e.span.line, e.span.col));
            Err(errors)
        }
    }
}

/// Lower a parsed stylesheet.
pub fn lower(sheet: &ast::Stylesheet) -> Result<Sheet, Vec<Error>> {
    let mut cx = Lower { tokens: HashMap::new(), errors: Vec::new() };
    // Tokens first, wherever they are written: a rule may use a token
    // defined below it.
    for item in &sheet.items {
        if let Item::Tokens(bs) = item {
            for b in bs {
                cx.define(b);
            }
        }
    }
    let mut out = Sheet { base: Vec::new(), blocks: Vec::new() };
    for item in &sheet.items {
        match item {
            Item::Rule(r) => cx.rule(r, &mut out.base),
            Item::Sheet(s) => {
                if !is_rust_ident(&s.name) {
                    cx.err(
                        s.span,
                        format!(
                            "`@sheet {}`: a sheet's name must be a Rust identifier, because \
                             `include_lss!` selects it by name",
                            s.name
                        ),
                    );
                }
                let mut rules = Vec::new();
                for r in &s.rules {
                    cx.rule(r, &mut rules);
                }
                out.blocks.push(Block { name: s.name.clone(), rules, span: s.span });
            }
            Item::Tokens(_) => {}
        }
    }
    if cx.errors.is_empty() {
        Ok(out)
    } else {
        Err(cx.errors)
    }
}

impl Sheet {
    /// The rules of the sheet `block` names — the top-level rules and
    /// then the block's — or the top-level rules alone for `None`. The
    /// order is the cascade's declaration order, so a block's rule wins a
    /// tie of specificity with a top-level one.
    pub fn select(&self, block: Option<&str>) -> Result<Vec<&Rule>, String> {
        let mut rules: Vec<&Rule> = self.base.iter().collect();
        if let Some(name) = block {
            match self.blocks.iter().find(|b| b.name == name) {
                Some(b) => rules.extend(b.rules.iter()),
                None => {
                    let have: Vec<String> = self.blocks.iter().map(|b| format!("`{}`", b.name)).collect();
                    return Err(if have.is_empty() {
                        format!("no `@sheet {name}`: the file has no `@sheet` blocks")
                    } else {
                        format!("no `@sheet {name}`: the file has {}", have.join(", "))
                    });
                }
            }
        }
        Ok(rules)
    }

    /// Every rule in the file, whichever block it is in — what
    /// `include_lss!` checks against the tree, so a mistake in a block no
    /// invocation selects is still a build error.
    pub fn all_rules(&self) -> impl Iterator<Item = &Rule> {
        self.base.iter().chain(self.blocks.iter().flat_map(|b| b.rules.iter()))
    }
}

struct Lower<'a> {
    tokens: HashMap<&'a str, &'a ast::Binding>,
    errors: Vec<Error>,
}

impl<'a> Lower<'a> {
    fn err(&mut self, span: Span, msg: impl Into<String>) {
        self.errors.push(Error::new(span, msg));
    }

    fn define(&mut self, b: &'a ast::Binding) {
        if let Some(role) = Role::from_keyword(&b.name) {
            self.err(
                b.span,
                format!(
                    "`@tokens` name `{}` shadows the palette role `${}`: a colour written through it \
                     would stop following the palette -- rename the token (docs/plan-lss.md §2.1)",
                    b.name,
                    role.keyword()
                ),
            );
            return;
        }
        if contains_var(&b.value) {
            self.err(b.span, format!("token `${}`: a token's value cannot name another token", b.name));
            return;
        }
        self.tokens.insert(&b.name, b);
    }

    fn rule(&mut self, r: &ast::Rule, out: &mut Vec<Rule>) {
        let mut setters = Vec::new();
        for d in &r.declarations {
            if let Some(s) = self.declaration(d) {
                setters.push(s);
            }
        }
        for sel in &r.selectors {
            out.push(Rule { selector: sel.part.clone(), setters: setters.clone(), span: sel.span });
        }
    }

    fn declaration(&mut self, d: &ast::Declaration) -> Option<Setter> {
        let Some(prop) = PROPERTIES.iter().find(|p| p.name == d.property) else {
            let hint = ALIASES
                .iter()
                .find(|(n, _)| *n == d.property)
                .map(|(_, h)| h.to_string())
                .or_else(|| {
                    let names: Vec<&str> = PROPERTIES.iter().map(|p| p.name).collect();
                    did_you_mean(&d.property, &names).map(|n| format!("did you mean `{n}`?"))
                });
            let msg = match hint {
                Some(h) => format!("unknown property `{}`; {h}", d.property),
                None => format!("unknown property `{}`", d.property),
            };
            self.err(d.span, msg);
            return None;
        };
        // `$name`: a token's value, or a palette role.
        let value = match self.resolve(&d.value, d.value_span, prop) {
            Resolved::Value(v) => v,
            Resolved::Role(role) => {
                return match prop.role_setter {
                    Some(method) => Some(Setter { method, value: Val::Role(role) }),
                    None => {
                        self.err(
                            d.value_span,
                            format!(
                                "`${}` is a palette role, which is a colour; `{}` takes {}",
                                role.keyword(),
                                prop.name,
                                prop.takes.describe()
                            ),
                        );
                        None
                    }
                };
            }
            Resolved::Failed => return None,
        };
        match typed(prop.takes, &value) {
            Some(v) => Some(Setter { method: prop.setter, value: v }),
            None => {
                let via = match &d.value {
                    Value::Var(v) => format!(" (`${v}` is `{value}`)"),
                    _ => String::new(),
                };
                self.err(
                    d.value_span,
                    format!("`{}` takes {}, not `{}`{via}", prop.name, prop.takes.describe(), d.value),
                );
                None
            }
        }
    }

    /// Substitute every `$token`. A value that is exactly one role is
    /// returned as the role; a role anywhere else is an error.
    fn resolve(&mut self, v: &Value, span: Span, prop: &Property) -> Resolved {
        if let Value::Var(name) = v {
            if let Some(role) = Role::from_keyword(name) {
                return Resolved::Role(role);
            }
        }
        let mut failed = false;
        let out = self.substitute(v, span, prop, &mut failed);
        if failed {
            Resolved::Failed
        } else {
            Resolved::Value(out)
        }
    }

    fn substitute(&mut self, v: &Value, span: Span, prop: &Property, failed: &mut bool) -> Value {
        match v {
            Value::Var(name) => {
                if let Some(b) = self.tokens.get(name.as_str()) {
                    return b.value.clone();
                }
                if Role::from_keyword(name).is_some() {
                    self.err(
                        span,
                        format!(
                            "`${name}` is a palette role, which is a colour; `{}` takes {}",
                            prop.name,
                            prop.takes.describe()
                        ),
                    );
                } else {
                    let mut known: Vec<&str> = self.tokens.keys().copied().collect();
                    known.extend(Role::ALL.iter().map(|r| r.keyword()));
                    let msg = match did_you_mean(name, &known) {
                        Some(h) => format!("unknown token `${name}`; did you mean `${h}`?"),
                        None => format!("unknown token `${name}`"),
                    };
                    self.err(span, msg);
                }
                *failed = true;
                v.clone()
            }
            Value::List(items) => Value::List(items.iter().map(|i| self.substitute(i, span, prop, failed)).collect()),
            Value::Function(n, args) => {
                Value::Function(n.clone(), args.iter().map(|a| self.substitute(a, span, prop, failed)).collect())
            }
            other => other.clone(),
        }
    }
}

enum Resolved {
    Value(Value),
    Role(Role),
    Failed,
}

fn contains_var(v: &Value) -> bool {
    match v {
        Value::Var(_) => true,
        Value::List(items) | Value::Function(_, items) => items.iter().any(contains_var),
        _ => false,
    }
}

/// The value as the setter takes it, or `None` when it cannot.
fn typed(takes: Takes, v: &Value) -> Option<Val> {
    match takes {
        Takes::Px => px(v).map(Val::F32),
        Takes::Number => match v {
            Value::Number(n, Unit::None) => f32_of(n).map(Val::F32),
            _ => None,
        },
        Takes::Byte => {
            let p = px(v)?;
            (p.fract() == 0.0 && (0.0..=255.0).contains(&p)).then_some(Val::U8(p as u8))
        }
        Takes::Colour => match v {
            Value::Color(c) => Some(Val::Color(*c)),
            _ => None,
        },
        Takes::Length => length(v).map(Val::Length),
        Takes::Edges => {
            let items: Vec<&Value> = match v {
                Value::List(items) => items.iter().collect(),
                one => vec![one],
            };
            let e: Vec<f32> = items.iter().map(|i| px(i)).collect::<Option<_>>()?;
            let (top, right, bottom, left) = match e.as_slice() {
                [a] => (*a, *a, *a, *a),
                [v, h] => (*v, *h, *v, *h),
                [t, h, b] => (*t, *h, *b, *h),
                [t, r, b, l] => (*t, *r, *b, *l),
                _ => return None,
            };
            Some(Val::Edges(Edges { left, right, top, bottom }))
        }
        Takes::Align => keyword(v).and_then(Align::from_keyword).map(Val::Align),
        Takes::Cursor => keyword(v).and_then(Cursor::from_keyword).map(Val::Cursor),
        Takes::Bool => match keyword(v)? {
            "true" => Some(Val::Bool(true)),
            "false" => Some(Val::Bool(false)),
            _ => None,
        },
    }
}

fn keyword(v: &Value) -> Option<&str> {
    match v {
        Value::Keyword(k) => Some(k),
        _ => None,
    }
}

/// `12px` as `12.0`.
fn px(v: &Value) -> Option<f32> {
    match v {
        Value::Number(n, Unit::Px) => f32_of(n),
        _ => None,
    }
}

/// The number's text straight to `f32` — see `ast::Value::Number`.
fn f32_of(text: &str) -> Option<f32> {
    text.parse::<f32>().ok().filter(|v| v.is_finite())
}

fn length(v: &Value) -> Option<Length> {
    match v {
        Value::Number(_, Unit::Px) => px(v).map(Length::Fixed),
        Value::Keyword(k) if k == "fill" => Some(Length::FillPortion(1)),
        Value::Keyword(k) if k == "shrink" => Some(Length::Shrink),
        Value::Function(f, args) if f == "fixed" && args.len() == 1 => px(&args[0]).map(Length::Fixed),
        Value::Function(f, args) if f == "fill" && args.len() == 1 => match &args[0] {
            Value::Number(n, Unit::None) => n.parse::<u16>().ok().filter(|k| *k > 0).map(Length::FillPortion),
            _ => None,
        },
        _ => None,
    }
}

fn is_rust_ident(s: &str) -> bool {
    let mut c = s.chars();
    matches!(c.next(), Some(f) if f.is_ascii_alphabetic() || f == '_') && c.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The closest candidate within edit distance 2 — Lumen's did-you-mean.
fn did_you_mean<'a>(word: &str, candidates: &[&'a str]) -> Option<&'a str> {
    candidates
        .iter()
        .map(|c| (*c, levenshtein(word, c)))
        .filter(|&(_, d)| d <= 2)
        .min_by_key(|&(_, d)| d)
        .map(|(c, _)| c)
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}
