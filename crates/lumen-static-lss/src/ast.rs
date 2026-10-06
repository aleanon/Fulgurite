//! The `.lss` syntax tree — v1's grammar (`docs/plan-lss.md` §2.1).
//!
//! Adopted from Lumen's `lumen-style` (`ast.rs`) and trimmed to what the
//! cascade here can apply: a rule has a list of **simple** selectors and
//! no nested rules, and the only at-rules are `@tokens` and `@sheet`.
//! Everything Lumen's grammar adds beyond that is refused by the parser
//! with a message, not represented here.

/// A source position, 1-based — the `line:col` of every error.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Span {
    pub line: u32,
    pub col: u32,
}

/// A parsed stylesheet: top-level items in source order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stylesheet {
    pub items: Vec<Item>,
}

/// A top-level item.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// A style rule.
    Rule(Rule),
    /// `@tokens { name: value; … }` — compile-time constants, used as
    /// `$name`.
    Tokens(Vec<Binding>),
    /// `@sheet name { rules }` — a named block of rules, chosen by name
    /// (`include_lss!(Tree, "f.lss", name)`). Not CSS: it stands in for
    /// the run-time classes this framework does not have (§2.1).
    Sheet(SheetBlock),
}

/// `@sheet name { … }`.
#[derive(Clone, Debug, PartialEq)]
pub struct SheetBlock {
    pub name: String,
    pub rules: Vec<Rule>,
    pub span: Span,
}

/// `name: value;` inside `@tokens`.
#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    pub name: String,
    pub value: Value,
    pub span: Span,
}

/// `selector, selector { declarations }`. A comma list is several rules
/// with one patch — each selector still one simple part.
#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub selectors: Vec<Selector>,
    pub declarations: Vec<Declaration>,
}

/// One simple selector and where it starts.
#[derive(Clone, Debug, PartialEq)]
pub struct Selector {
    pub part: Part,
    pub span: Span,
}

/// The four selectors the cascade matches on — `lumen_static::Selector`'s
/// variants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Part {
    /// `#id`.
    Id(String),
    /// `.class`.
    Class(String),
    /// A bare word: a widget kind, spelled as `Node::kind_name` reports
    /// it (lowercase leaves, struct idents for containers).
    Kind(String),
    /// `*`.
    Any,
}

impl core::fmt::Display for Part {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Part::Id(s) => write!(f, "#{s}"),
            Part::Class(s) => write!(f, ".{s}"),
            Part::Kind(s) => f.write_str(s),
            Part::Any => f.write_str("*"),
        }
    }
}

/// `property: value;`.
#[derive(Clone, Debug, PartialEq)]
pub struct Declaration {
    pub property: String,
    pub value: Value,
    /// Where the property name starts.
    pub span: Span,
    /// Where the value starts.
    pub value_span: Span,
}

/// A parsed value: one atom, or a space-separated list of them.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A number, **as written**, and its unit. The text is kept so a
    /// lowering can parse it straight to `f32`: through `f64` first, a
    /// decimal can round twice and land one ulp from the literal a person
    /// would have typed — which K1's byte comparison would see.
    Number(String, Unit),
    /// `#rrggbb` or `#rrggbbaa`.
    Color([u8; 4]),
    /// A bare word (`center`, `fill`, `true`).
    Keyword(String),
    /// A `$token` reference.
    Var(String),
    /// `name(args…)` — `fixed(12px)`, `fill(2)`.
    Function(String, Vec<Value>),
    /// A space-separated list: `padding: 4px 8px`.
    List(Vec<Value>),
    /// A double-quoted string. No property takes one; kept so the error
    /// can say so rather than calling it a syntax error.
    Str(String),
}

/// A number's unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    /// Unitless.
    None,
    /// Logical pixels.
    Px,
    /// Percent — lexed so the error can name it; no property takes one.
    Percent,
    /// Milliseconds — M86/T86.1, for `transition-duration: 150ms`.
    Ms,
}

impl core::fmt::Display for Value {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Value::Number(n, Unit::None) => f.write_str(n),
            Value::Number(n, Unit::Px) => write!(f, "{n}px"),
            Value::Number(n, Unit::Percent) => write!(f, "{n}%"),
            Value::Number(n, Unit::Ms) => write!(f, "{n}ms"),
            Value::Color([r, g, b, a]) => write!(f, "#{r:02x}{g:02x}{b:02x}{a:02x}"),
            Value::Keyword(k) => f.write_str(k),
            Value::Var(v) => write!(f, "${v}"),
            Value::Str(s) => write!(f, "\"{s}\""),
            Value::Function(name, args) => {
                write!(f, "{name}(")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{a}")?;
                }
                f.write_str(")")
            }
            Value::List(items) => {
                for (i, a) in items.iter().enumerate() {
                    if i > 0 {
                        f.write_str(" ")?;
                    }
                    write!(f, "{a}")?;
                }
                Ok(())
            }
        }
    }
}
