//! Computed style, in a parallel array — plan M1
//! (`docs/layout-and-storage-decisions.md` §3).
//!
//! Style lives *beside* the tree, not inside the widgets: a
//! `[Style; NODE_COUNT]` keyed by node index, exactly like `NodeState`.
//! Widgets stay small (style on every widget would make
//! `size_of::<Label>()` explode and turn §3's "values in the fields"
//! into "everything in the fields"), allocation stays at zero, and the
//! existing change journal diffs it for free.
//!
//! Computed style is *derived*: it is recomputed by the cascade, never
//! migrated across a hot reload.

// `alloc`, not the std prelude: this crate is `no_std` when the
// `std` feature is off (M13/T13.1), and absent entirely without the
// `alloc` feature (T13.4).
#[cfg(feature = "alloc")]
// The cascade witness (debug + alloc) is the only user now that the
// arena walk is gated.
#[cfg(all(debug_assertions, feature = "alloc"))]
use alloc::vec::Vec;

use crate::desc::NodeDesc;

/// How a node claims space along one axis — iced's model, which needs no
/// iteration to resolve (`docs/layout-and-storage-decisions.md` §2).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Length {
    /// Exactly this many pixels.
    Fixed(f32),
    /// The node's own content size.
    #[default]
    Shrink,
    /// Share the remainder with other fills, in this ratio.
    /// `Fill` is `FillPortion(1)`.
    FillPortion(u16),
}

impl Length {
    pub const FILL: Length = Length::FillPortion(1);

    #[inline]
    pub fn portion(&self) -> u16 {
        match self {
            Length::FillPortion(k) => *k,
            _ => 0,
        }
    }

    #[inline]
    pub fn is_fill(&self) -> bool {
        matches!(self, Length::FillPortion(_))
    }
}

/// Per-side lengths, for padding.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Edges {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

impl Edges {
    pub const ZERO: Edges = Edges { left: 0.0, right: 0.0, top: 0.0, bottom: 0.0 };

    pub const fn all(v: f32) -> Edges {
        Edges { left: v, right: v, top: v, bottom: v }
    }

    #[inline]
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    #[inline]
    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

/// Cross-axis alignment of a container's children.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    /// Stretch to the container's cross size.
    Fill,
}

/// What the pointer should look like over a node —
/// `docs/plan-desktop.md` T14.6.
///
/// A *style* property rather than an event or a widget method, because
/// style already cascades and is already per-node: a cursor becomes a
/// rule like any other, and a custom widget gets one without
/// implementing anything.
///
/// The set is the one a desktop application actually uses. A pointer
/// shape nothing here can express is a reason to add a variant, not to
/// make this an open string — the windowing layer has to map it to a
/// platform cursor, and an unmappable name is a silent no-op.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "observe", derive(serde::Serialize, serde::Deserialize))]
pub enum Cursor {
    #[default]
    Default,
    /// An I-beam: over editable text.
    Text,
    /// A hand: over something clickable.
    Pointer,
    Grab,
    Grabbing,
    ColResize,
    RowResize,
    /// Over a control that is present but will not respond.
    NotAllowed,
}

/// One node's computed style.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub width: Length,
    pub height: Length,
    pub padding: Edges,
    /// Gap between a container's children.
    pub spacing: f32,
    /// Text colour.
    pub color: crate::display::Color,
    /// Fill behind this node. `TRANSPARENT` paints nothing.
    pub background: crate::display::Color,
    /// Corner radius for the background, in pixels.
    pub radius: f32,
    /// Text size in pixels.
    pub font_size: f32,
    /// Text weight, CSS-style: 400 regular, 700 bold.
    pub font_weight: f32,
    /// Drop shadow spread in pixels; 0 is none.
    pub shadow: f32,
    /// How much darker this node's background goes while the pointer is
    /// down on it, 0.0–1.0 (C6). 0 is no feedback.
    ///
    /// A scalar rather than a whole second `Patch` per state: pressed
    /// and hovered affordances are almost always a shade of the resting
    /// colour, and a `Style` is stored per node — doubling its size to
    /// express the rare case would cost every node in every tree. A
    /// control needing more than a shade reads `NodeState::pressed` in
    /// its own `paint`.
    pub pressed_dim: f32,
    /// The same while the pointer is merely over it.
    pub hover_dim: f32,
    pub align: Align,
    /// Width of the ring drawn around this node when it has focus, in
    /// logical px; 0 draws none — `docs/plan-capabilities.md` T15.6.
    ///
    /// **A visible focus indicator is the difference between a keyboard
    /// user being able to use an application and not.** It is a
    /// `Style` property rather than a widget's business for the same
    /// reason the cursor is: it cascades, so a stylesheet sets it once
    /// and every focusable widget gets one, including custom ones that
    /// implement nothing.
    ///
    /// **A `u8` and not an `f32`, and that was measured.** As an `f32`
    /// this took `Style` from 72 to 76 bytes — it does not fit the tail
    /// padding the way `cursor`'s one byte did, and the M15 gating
    /// policy says a per-node field either fits or gets re-budgeted
    /// rather than paid for silently. A ring wider than 255 px is not a
    /// ring, so the byte costs nothing real. Asserted in
    /// `tests/cursor.rs`, which is where this struct's budget lives.
    pub focus_ring: u8,
    /// The pointer shape over this node (M14/T14.6).
    ///
    /// Free: `Style` was 72 B and this fits in the tail padding beside
    /// `align` and `visible`. Asserted in
    /// `crates/lumen-static/tests/cursor.rs`, because it would stop
    /// being free if a future field pushed it over — and `Style` is
    /// resolved per node, so a byte here is a byte times every node of
    /// every tree.
    pub cursor: Cursor,
    /// Whether this node and everything under it participates in the
    /// frame.
    ///
    /// `false` means: the subtree resolves to zero size, is not
    /// cross-stretched by its parent, records no drawing commands, and
    /// fails every hit test. All four are enforced —
    /// `crates/lumen-static/tests/visible.rs` asserts each clause of
    /// this sentence for a hidden container *and* a hidden leaf.
    ///
    /// Until 2026-09-02 none of them were. No leaf read this field at
    /// all, and the derive zeroed only the container's own slot, so
    /// descendants kept the previous frame's geometry and were painted
    /// at it. `Option`'s separate `hide()` path did the right thing, so
    /// two mechanisms disagreed. `docs/plan-review-fixes.md` F1.6.
    ///
    /// **Not by an `Either`'s own node**, measured 2026-09-15: it forwards
    /// layout to the branch it shows and resolves no style of its own, so
    /// `visible: false` on it changes nothing. Hide the branch instead.
    /// The sheet lint reports such a rule (M25/T25.2,
    /// `docs/results-appearance.md`).
    pub visible: bool,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            width: Length::Shrink,
            height: Length::Shrink,
            padding: Edges::ZERO,
            spacing: 0.0,
            color: crate::display::Color::BLACK,
            background: crate::display::Color::TRANSPARENT,
            radius: 0.0,
            font_size: 16.0,
            font_weight: 400.0,
            shadow: 0.0,
            pressed_dim: 0.0,
            hover_dim: 0.0,
            align: Align::Start,
            focus_ring: 2,
            cursor: Cursor::Default,
            visible: true,
        }
    }
}

/// A widget kind's own defaults, before any rule applies. Emitted by the
/// derive from `#[widget(...)]` and returned by leaves.
pub type Defaults = Style;

/// Everything the cascade needs about a node that **cannot vary per
/// instance**: its kind name and its own defaults.
///
/// # Why this is not read off the live tree
///
/// It was, until M24/T24.0, through `Node::kind_name` and
/// `Node::defaults` — whose rustdoc has said since F3.2 that the answer
/// *"must depend only on the type, never on `self`'s current state"*.
/// A fact that cannot vary per instance, reachable only through an
/// instance, is stored in the wrong place, and the cost of that was
/// exact: **a hidden subtree has no live node to ask.**
/// `Option::<W>::node` returns the `Option` when `None`, and the enum
/// switch returns the enum itself for an inactive variant's indices —
/// deliberately, so the agent can walk them. So those nodes cascaded
/// from the *container's* kind and defaults. A `CheckBox`'s
/// `Fixed(24.0)` became `Shrink`, which on screen is a control with no
/// size of its own: a widget that looks like a label
/// (`docs/plan-select.md` §1).
///
/// `Selector::Id` never had the problem, because `wid` lives in the
/// const [`crate::desc::NodeDesc`]. This puts the other two inputs
/// where that one always was.
///
/// # `fn() -> Style`, not `Style`
///
/// Forced, not chosen: every `defaults()` body in the tree ends
/// `..Style::default()`, and `Default::default` is not a `const fn`. A
/// thunk costs one pointer per node of `.rodata` and leaves the bodies
/// reading exactly as they did.
#[derive(Clone, Copy)]
pub struct Seed {
    /// The kind name a [`Selector::Kind`] rule matches.
    pub kind: &'static str,
    /// This kind's defaults, before any rule applies.
    pub defaults: fn() -> Style,
    /// The [`Style`] fields this kind's layout and paint actually read —
    /// M25/T25.2.
    ///
    /// What the sheet lint ([`lint_sheet_deep`]) compares a rule against:
    /// a rule setting a field that no node it matches reads is a rule
    /// that compiles and changes nothing. [`Fields::ALL`] from
    /// [`Seed::new`], which never produces a finding — so a kind that has
    /// not declared its reads is not linted rather than wrongly linted,
    /// and `crates/lumen-static-widgets/tests/style_reads.rs` measures
    /// every declared mask against what the kind's output actually does.
    pub reads: Fields,
}

impl Seed {
    pub const fn new(kind: &'static str, defaults: fn() -> Style) -> Seed {
        Seed { kind, defaults, reads: Fields::ALL }
    }

    /// The same seed, declaring which fields its kind reads.
    pub const fn reading(mut self, reads: Fields) -> Seed {
        self.reads = reads;
        self
    }

    /// The seed of a node that declares nothing — [`crate::widget::Node`]'s
    /// own defaults, spelled as a type-level fact.
    pub const WIDGET: Seed = Seed::new("widget", plain_style);
}

/// `Style::default` as a plain `fn` item, so [`Seed`] can hold it.
pub fn plain_style() -> Style {
    Style::default()
}

// ---- which fields a kind reads — M25/T25.2 ---------------------------

/// A set of [`Style`] fields, one bit each, in [`Patch`]'s order.
///
/// Two uses: what a [`Patch`] sets ([`Patch::fields`]) and what a kind
/// reads ([`Seed::reads`]). The difference between the two is a rule
/// that does nothing, which is the failure the sheet lint exists for —
/// `Kind("checkbox")` with a `font_weight` compiled and changed nothing
/// before it, and so did every rule naming a kind the tree does not
/// have.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fields(u16);

impl Fields {
    pub const NONE: Fields = Fields(0);
    pub const WIDTH: Fields = Fields(1 << 0);
    pub const HEIGHT: Fields = Fields(1 << 1);
    pub const PADDING: Fields = Fields(1 << 2);
    pub const SPACING: Fields = Fields(1 << 3);
    pub const ALIGN: Fields = Fields(1 << 4);
    pub const CURSOR: Fields = Fields(1 << 5);
    pub const FOCUS_RING: Fields = Fields(1 << 6);
    pub const VISIBLE: Fields = Fields(1 << 7);
    pub const COLOR: Fields = Fields(1 << 8);
    pub const BACKGROUND: Fields = Fields(1 << 9);
    pub const RADIUS: Fields = Fields(1 << 10);
    pub const FONT_SIZE: Fields = Fields(1 << 11);
    pub const FONT_WEIGHT: Fields = Fields(1 << 12);
    pub const SHADOW: Fields = Fields(1 << 13);
    pub const PRESSED_DIM: Fields = Fields(1 << 14);
    pub const HOVER_DIM: Fields = Fields(1 << 15);
    pub const ALL: Fields = Fields(u16::MAX);

    /// Every field with its name, for reports and for the tests that
    /// measure a mask field by field.
    pub const EACH: [(Fields, &'static str); 16] = [
        (Fields::WIDTH, "width"),
        (Fields::HEIGHT, "height"),
        (Fields::PADDING, "padding"),
        (Fields::SPACING, "spacing"),
        (Fields::ALIGN, "align"),
        (Fields::CURSOR, "cursor"),
        (Fields::FOCUS_RING, "focus_ring"),
        (Fields::VISIBLE, "visible"),
        (Fields::COLOR, "color"),
        (Fields::BACKGROUND, "background"),
        (Fields::RADIUS, "radius"),
        (Fields::FONT_SIZE, "font_size"),
        (Fields::FONT_WEIGHT, "font_weight"),
        (Fields::SHADOW, "shadow"),
        (Fields::PRESSED_DIM, "pressed_dim"),
        (Fields::HOVER_DIM, "hover_dim"),
    ];

    /// The union of a list — how a widget spells its mask at the
    /// [`Seed`] it declares.
    pub const fn of(list: &[Fields]) -> Fields {
        let mut bits = 0u16;
        let mut k = 0;
        while k < list.len() {
            bits |= list[k].0;
            k += 1;
        }
        Fields(bits)
    }

    pub const fn union(self, other: Fields) -> Fields {
        Fields(self.0 | other.0)
    }

    pub const fn minus(self, other: Fields) -> Fields {
        Fields(self.0 & !other.0)
    }

    pub const fn contains(self, other: Fields) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The names of the fields in this set.
    pub fn names(self) -> impl Iterator<Item = &'static str> {
        Self::EACH.into_iter().filter(move |(f, _)| self.contains(*f)).map(|(_, n)| n)
    }
}

// ---- status and palette — M25/T25.3 ----------------------------------

/// The colours a theme is made of — data, swapped at runtime — M25/T25.3.
///
/// iced's shape (`docs/plan-appearance.md` §1): a palette is data, and a
/// widget's look is a function of it. Held by the host and reached by
/// every `layout` through [`crate::LayoutCx::palette`]; nothing reads it
/// until a widget declares a look.
///
/// Named by role, not by hue, so a dark palette is the same names with
/// different values and a look written against one reads correctly
/// against the other.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    /// The window behind everything.
    pub page: crate::display::Color,
    /// A raised body: a button, a card, a text field.
    pub surface: crate::display::Color,
    /// A control's resting body where it sits straight on the page: an
    /// unchecked box, an unselected radio, a switch's track — M25/T25.5.
    ///
    /// **Not `surface`**, and the difference was paid for once already:
    /// a toggle's resting body in `surface`'s grey is about sixteen levels
    /// from a light page, which is to say invisible (M23). A control has
    /// to be seen before it is checked.
    pub control: crate::display::Color,
    /// Text on `page` or `surface`.
    pub text: crate::display::Color,
    /// Secondary text and hairlines.
    pub muted: crate::display::Color,
    /// The one colour that says "on" or "primary".
    pub accent: crate::display::Color,
    /// Text and marks on `accent`.
    pub on_accent: crate::display::Color,
    /// The keyboard focus ring.
    pub focus: crate::display::Color,
}

impl Palette {
    /// The light palette, and the default. Its values are the ones the
    /// widgets' own defaults already use, so a tree that never sets a
    /// palette looks exactly as it did before one existed.
    pub const LIGHT: Palette = Palette {
        page: crate::display::Color(255, 255, 255, 255),
        surface: crate::display::Color(226, 232, 240, 255),
        control: crate::display::Color(0xbf, 0xc4, 0xcc, 255),
        text: crate::display::Color(15, 23, 42, 255),
        muted: crate::display::Color(100, 116, 139, 255),
        accent: crate::display::Color(53, 99, 239, 255),
        on_accent: crate::display::Color(255, 255, 255, 255),
        focus: crate::display::Color(53, 99, 239, 255),
    };

    /// The dark palette: the same roles, for a dark page.
    pub const DARK: Palette = Palette {
        page: crate::display::Color(15, 18, 26, 255),
        surface: crate::display::Color(38, 44, 58, 255),
        control: crate::display::Color(74, 82, 99, 255),
        text: crate::display::Color(230, 234, 242, 255),
        muted: crate::display::Color(148, 160, 180, 255),
        accent: crate::display::Color(99, 140, 255, 255),
        on_accent: crate::display::Color(10, 14, 24, 255),
        focus: crate::display::Color(130, 170, 255, 255),
    };
}

impl Default for Palette {
    fn default() -> Self {
        Palette::LIGHT
    }
}

// ---- selectors and rules ----------------------------------------------

/// What a rule matches on. Deliberately small: id, class and kind are the
/// three axes Lumen's `.lss` cascade uses too, and they are all static
/// per node, so matching is a table scan with no allocation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Selector {
    /// `#save` — an author id.
    Id(&'static str),
    /// `.row` — a class.
    Class(&'static str),
    /// `label` — a widget kind name.
    /// A widget kind, as [`Node::kind_name`](crate::widget::Node::kind_name)
    /// reports it — **and the two are not spelled alike**. Built-in
    /// leaves are lowercase (`label`, `button`, `fixed`, `spacer`);
    /// derived containers use the struct's own ident (`Ui`, `Controls`).
    /// So `Selector::Kind("label")` and `Selector::Kind("Ui")` are both
    /// correct. Documented nowhere until 2026-09-02
    /// (`docs/plan-review-fixes.md` F7.5).
    Kind(&'static str),
    /// Everything.
    Any,
}

impl Selector {
    /// Specificity, highest wins: id > class > kind > any.
    #[inline]
    pub fn specificity(&self) -> u8 {
        match self {
            Selector::Id(_) => 3,
            Selector::Class(_) => 2,
            Selector::Kind(_) => 1,
            Selector::Any => 0,
        }
    }
}

/// What a rule changes. `None` fields leave the cascaded value alone, so
/// rules compose rather than overwrite.
#[derive(Clone, Copy, Debug, Default)]
pub struct Patch {
    pub width: Option<Length>,
    pub height: Option<Length>,
    pub padding: Option<Edges>,
    pub spacing: Option<f32>,
    pub align: Option<Align>,
    pub cursor: Option<Cursor>,
    pub focus_ring: Option<u8>,
    pub visible: Option<bool>,
    pub color: Option<crate::display::Color>,
    pub background: Option<crate::display::Color>,
    pub radius: Option<f32>,
    pub font_size: Option<f32>,
    pub font_weight: Option<f32>,
    pub shadow: Option<f32>,
    pub pressed_dim: Option<f32>,
    pub hover_dim: Option<f32>,
}

impl Patch {
    pub const fn new() -> Patch {
        Patch {
            width: None,
            height: None,
            padding: None,
            spacing: None,
            align: None,
            cursor: None,
            focus_ring: None,
            visible: None,
            color: None,
            background: None,
            radius: None,
            font_size: None,
            font_weight: None,
            shadow: None,
            pressed_dim: None,
            hover_dim: None,
        }
    }

    pub const fn width(mut self, l: Length) -> Self {
        self.width = Some(l);
        self
    }
    pub const fn height(mut self, l: Length) -> Self {
        self.height = Some(l);
        self
    }
    pub const fn padding(mut self, e: Edges) -> Self {
        self.padding = Some(e);
        self
    }
    pub const fn spacing(mut self, v: f32) -> Self {
        self.spacing = Some(v);
        self
    }
    pub const fn align(mut self, a: Align) -> Self {
        self.align = Some(a);
        self
    }
    pub const fn visible(mut self, v: bool) -> Self {
        self.visible = Some(v);
        self
    }

    /// The pointer shape over the nodes this rule matches (T14.6).
    pub const fn cursor(mut self, c: Cursor) -> Self {
        self.cursor = Some(c);
        self
    }

    /// Width of the focus ring, in logical px; 0 for none (T15.6).
    pub const fn focus_ring(mut self, w: u8) -> Self {
        self.focus_ring = Some(w);
        self
    }

    pub const fn color(mut self, c: crate::display::Color) -> Self {
        self.color = Some(c);
        self
    }

    pub const fn background(mut self, c: crate::display::Color) -> Self {
        self.background = Some(c);
        self
    }

    pub const fn radius(mut self, r: f32) -> Self {
        self.radius = Some(r);
        self
    }

    pub const fn font_size(mut self, px: f32) -> Self {
        self.font_size = Some(px);
        self
    }

    pub const fn font_weight(mut self, w: f32) -> Self {
        self.font_weight = Some(w);
        self
    }

    pub const fn shadow(mut self, spread: f32) -> Self {
        self.shadow = Some(spread);
        self
    }

    /// How much darker the background goes while pressed (0.0–1.0).
    pub const fn pressed_dim(mut self, v: f32) -> Self {
        self.pressed_dim = Some(v);
        self
    }

    /// The same while hovered.
    pub const fn hover_dim(mut self, v: f32) -> Self {
        self.hover_dim = Some(v);
        self
    }

    #[inline]
    fn apply_to(&self, s: &mut Style) {
        if let Some(v) = self.width {
            s.width = v;
        }
        if let Some(v) = self.height {
            s.height = v;
        }
        if let Some(v) = self.padding {
            s.padding = v;
        }
        if let Some(v) = self.spacing {
            s.spacing = v;
        }
        if let Some(v) = self.align {
            s.align = v;
        }
        if let Some(v) = self.visible {
            s.visible = v;
        }
        if let Some(v) = self.cursor {
            s.cursor = v;
        }
        if let Some(v) = self.focus_ring {
            s.focus_ring = v;
        }
        if let Some(v) = self.color {
            s.color = v;
        }
        if let Some(v) = self.background {
            s.background = v;
        }
        if let Some(v) = self.radius {
            s.radius = v;
        }
        if let Some(v) = self.font_size {
            s.font_size = v;
        }
        if let Some(v) = self.font_weight {
            s.font_weight = v;
        }
        if let Some(v) = self.pressed_dim {
            s.pressed_dim = v;
        }
        if let Some(v) = self.hover_dim {
            s.hover_dim = v;
        }
        if let Some(v) = self.shadow {
            s.shadow = v;
        }
    }
}

impl Patch {
    /// The fields this patch sets — what the sheet lint compares against
    /// the fields a matched kind reads (M25/T25.2).
    pub const fn fields(&self) -> Fields {
        let mut f = 0u16;
        if self.width.is_some() {
            f |= Fields::WIDTH.0;
        }
        if self.height.is_some() {
            f |= Fields::HEIGHT.0;
        }
        if self.padding.is_some() {
            f |= Fields::PADDING.0;
        }
        if self.spacing.is_some() {
            f |= Fields::SPACING.0;
        }
        if self.align.is_some() {
            f |= Fields::ALIGN.0;
        }
        if self.cursor.is_some() {
            f |= Fields::CURSOR.0;
        }
        if self.focus_ring.is_some() {
            f |= Fields::FOCUS_RING.0;
        }
        if self.visible.is_some() {
            f |= Fields::VISIBLE.0;
        }
        if self.color.is_some() {
            f |= Fields::COLOR.0;
        }
        if self.background.is_some() {
            f |= Fields::BACKGROUND.0;
        }
        if self.radius.is_some() {
            f |= Fields::RADIUS.0;
        }
        if self.font_size.is_some() {
            f |= Fields::FONT_SIZE.0;
        }
        if self.font_weight.is_some() {
            f |= Fields::FONT_WEIGHT.0;
        }
        if self.shadow.is_some() {
            f |= Fields::SHADOW.0;
        }
        if self.pressed_dim.is_some() {
            f |= Fields::PRESSED_DIM.0;
        }
        if self.hover_dim.is_some() {
            f |= Fields::HOVER_DIM.0;
        }
        Fields(f)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Rule {
    pub selector: Selector,
    pub patch: Patch,
}

impl Rule {
    pub const fn new(selector: Selector, patch: Patch) -> Rule {
        Rule { selector, patch }
    }
}

/// A stylesheet is just a slice of rules — a `.lss` parser (parity Phase
/// C) would produce one.
pub type Stylesheet<'a> = &'a [Rule];

fn matches(sel: &Selector, d: &NodeDesc, kind: &str) -> bool {
    match sel {
        Selector::Any => true,
        Selector::Kind(k) => kind == *k,
        Selector::Id(id) => d.wid == *id,
        Selector::Class(c) => d.classes.contains(c),
    }
}

/// Resolve every node's style: kind defaults first, then rules in
/// ascending specificity, then declaration order within a specificity.
///
/// One flat pass over the table — the cascade is the only place that
/// still walks the table linearly, and it re-runs only when the sheet or
/// the tree changes, not every frame.
/// **When to call this.** Not per frame. The result depends only on the
/// stylesheet, the table, and each kind's `defaults()` — and
/// [`Node::defaults`](crate::widget::Node::defaults) is required to
/// depend on the type alone, never on instance state. So the computed
/// styles change only when the sheet is replaced or the tree's shape is
/// (a hot reload), which is exactly when
/// [`Host::set_stylesheet`](../../lumen_static_shell/struct.Host.html)
/// and `Host::new` run it.
///
/// A `pass::frame` helper used to cascade and lay out together, on the
/// theory that the two belong in one step. It had zero callers — every
/// host and test open-coded the pair — and once `defaults()` became
/// state-independent it was not merely unused but misleading, since it
/// implied the cascade had to be re-run for a frame to be correct. It
/// was deleted rather than wired up. `docs/plan-review-fixes.md` F3.3.
pub fn cascade(table: &[NodeDesc], seeds: &[Seed], sheet: Stylesheet, out: &mut [Style]) {
    cascade_range(table, seeds, sheet, out, 0..table.len());
}

/// [`cascade`], restricted to a range of the table.
fn cascade_range(
    table: &[NodeDesc],
    seeds: &[Seed],
    sheet: Stylesheet,
    out: &mut [Style],
    range: core::ops::Range<usize>,
) {
    debug_assert_eq!(table.len(), out.len());
    debug_assert_eq!(table.len(), seeds.len());
    for (i, d) in table.iter().enumerate().skip(range.start).take(range.len()) {
        let mut s = (seeds[i].defaults)();
        let kind = seeds[i].kind;
        for spec in 0..=3u8 {
            for r in sheet {
                if r.selector.specificity() == spec && matches(&r.selector, d, kind) {
                    r.patch.apply_to(&mut s);
                }
            }
        }
        out[i] = s;
    }
}

/// [`cascade`] for a table whose seeds are not knowable statically.
///
/// **One caller: an arena under a [`crate::hatch::Dyn`].** Type erasure
/// is the island's purpose, so the boxed content's kinds cannot reach a
/// const array — the boxed instance is the only thing that knows them,
/// and it is live by construction, so asking it is sound here where it
/// was not for the main table.
///
/// `alloc`-gated because its only caller is: `Dyn`, `Repeat` and `List`
/// are all behind that feature (T13.4), so a tree with no allocator has
/// no arena and no instance to fall back to.
#[cfg(feature = "alloc")]
fn cascade_from_instance(
    table: &[NodeDesc],
    defaults: &dyn Fn(usize) -> Defaults,
    kind_of: &dyn Fn(usize) -> &'static str,
    sheet: Stylesheet,
    out: &mut [Style],
) {
    debug_assert_eq!(table.len(), out.len());
    for (i, d) in table.iter().enumerate() {
        let mut s = defaults(i);
        let kind = kind_of(i);
        for spec in 0..=3u8 {
            for r in sheet {
                if r.selector.specificity() == spec && matches(&r.selector, d, kind) {
                    r.patch.apply_to(&mut s);
                }
            }
        }
        out[i] = s;
    }
}

/// What the sheet lint found wrong with one rule — M25/T25.2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SheetIssue {
    /// The selector matches no node: a kind, id or class spelled
    /// differently from the tree's. `Kind("Label")` against a tree of
    /// `label`s is the documented case (`book/src/styling.md`).
    Unmatched,
    /// It matches, and these fields it sets are read by **none** of the
    /// nodes it matches — a checkbox given a `font_weight`.
    Unread(Fields),
}

/// One rule the lint found doing nothing, by its index in the sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SheetFinding {
    pub rule: usize,
    pub issue: SheetIssue,
}

/// Report every rule in `sheet` that cannot change `tree`'s frame —
/// M25/T25.2.
///
/// A rule is reported when its selector matches no node, or when some
/// field it sets is read by no node it matches ([`Seed::reads`]). The
/// walk reaches arena templates the way [`cascade_deep`] does, because a
/// rule matching only a repeater's row is a rule that works. A `Dyn`
/// island's kinds are asked of its live template and counted as reading
/// every field, since its content's masks cannot reach a const array.
///
/// **Arenas nested inside an arena's template are not reached**, the
/// same limit `cascade_arenas` records: a rule matching only there is
/// reported as unmatched. Conservative in the wrong direction for that
/// one shape, and stated rather than hidden.
///
/// O(rules × nodes), with no allocation. Not per frame: a host runs it
/// when a sheet is set.
#[cfg_attr(not(feature = "alloc"), allow(unused_variables))]
pub fn lint_sheet_deep<W: crate::widget::Widget + ?Sized>(
    tree: &mut W,
    sheet: Stylesheet,
    report: &mut dyn FnMut(SheetFinding),
) {
    for (r, rule) in sheet.iter().enumerate() {
        let mut matched = false;
        let mut read = Fields::NONE;
        let mut visit = |d: &NodeDesc, kind: &str, reads: Fields| {
            if matches(&rule.selector, d, kind) {
                matched = true;
                read = read.union(reads);
            }
        };
        for (i, d) in W::SUB_TABLE.iter().enumerate() {
            visit(d, W::SEEDS[i].kind, W::SEEDS[i].reads);
        }
        #[cfg(feature = "alloc")]
        for i in 0..W::SUB_TABLE.len() {
            let node = tree.node_mut(i);
            for a in 0..node.arena_count() {
                let Some(arena) = node.arena_mut(a) else { continue };
                match arena.seeds {
                    Some(seeds) => {
                        for (k, d) in arena.table.iter().enumerate() {
                            visit(d, seeds[k].kind, seeds[k].reads);
                        }
                    }
                    None => {
                        for (k, d) in arena.table.iter().enumerate() {
                            visit(d, arena.template.node(k).kind_name(), Fields::ALL);
                        }
                    }
                }
            }
        }
        if !matched {
            report(SheetFinding { rule: r, issue: SheetIssue::Unmatched });
            continue;
        }
        let unread = rule.patch.fields().minus(read);
        if !unread.is_empty() {
            report(SheetFinding { rule: r, issue: SheetIssue::Unread(unread) });
        }
    }
}

/// The half-open index range of the subtree rooted at `root`.
///
/// The table is DFS pre-order, so a subtree is *contiguous* — the
/// property that makes a scoped cascade a slice rather than a walk. The
/// end is the root's next sibling, or, when it has none, the next
/// sibling of the nearest ancestor that has one.
pub fn subtree_range(table: &[NodeDesc], root: usize) -> core::ops::Range<usize> {
    let mut i = root;
    loop {
        let d = &table[i];
        if d.next_sibling != 0 {
            return root..d.next_sibling as usize;
        }
        if d.parent == crate::desc::NO_PARENT {
            return root..table.len();
        }
        i = d.parent as usize;
    }
}

/// The main-table index of the node carrying `#[widget_id(wid)]`.
///
/// A table scan — 901 pointer comparisons on the largest fixture, next
/// to the ~36 µs a full cascade costs there
/// (`docs/results-cascade-cost.md`) — so an app may address a subtree by
/// the name it already wrote rather than by an index arithmetic it would
/// have to keep in step with the struct.
pub fn index_of_wid(table: &[NodeDesc], wid: &str) -> Option<usize> {
    // A node without `#[widget_id]` carries `""`, so an empty argument
    // would match the first unnamed node — which is usually the root,
    // and would restyle the whole tree. Nothing is named `""`.
    if wid.is_empty() {
        return None;
    }
    table.iter().position(|d| d.wid == wid)
}

/// Re-cascade one subtree: the node at `root` and its descendants.
///
/// **Sound because every [`Selector`] is node-local.** `Id`, `Class`,
/// `Kind` and `Any` all read the node's own descriptor and kind; none
/// reads an ancestor, a sibling or a position. So a node's resolved
/// style is a function of that node and the sheet, and re-resolving a
/// contiguous range cannot disagree with re-resolving the table. A
/// descendant combinator would end that property, and adding one means
/// revisiting this function in the same commit.
///
/// **The caller's contract:** `sheet` must differ from the one last
/// cascaded only in rules that match *inside* the subtree. It is the one
/// thing this cannot check cheaply, so in debug builds it checks it
/// expensively — a full cascade is run and every style outside the
/// range is compared. A release build skips that; a test does not, which
/// is where the mistake will be caught.
pub fn cascade_subtree<W: crate::widget::Widget + ?Sized>(
    tree: &mut W,
    sheet: Stylesheet,
    out: &mut [Style],
    root: usize,
) {
    let table = W::SUB_TABLE;
    let range = subtree_range(table, root);

    // Gated on `alloc` as well as the profile (M13/T13.3): this check
    // allocates a whole second style array, and it is gated on
    // `debug_assertions`, which is a *profile*, not a feature — so
    // without the `alloc` clause an embedded debug build would allocate
    // inside the cascade, on the one target where that is least
    // affordable. The check survives wherever a `Vec` is nameable,
    // which is every build that has a stylesheet to get wrong.
    #[cfg(all(debug_assertions, feature = "alloc"))]
    let witness: Vec<Style> = {
        let mut full = out.to_vec();
        cascade(table, W::SEEDS, sheet, &mut full);
        full
    };

    cascade_range(table, W::SEEDS, sheet, out, range.clone());
    for i in range.clone() {
        cascade_arenas(tree.node_mut(i), sheet);
    }

    #[cfg(all(debug_assertions, feature = "alloc"))]
    for (i, w) in witness.iter().enumerate() {
        if !range.contains(&i) {
            debug_assert_eq!(
                out[i], *w,
                "cascade_subtree({root}) left node {i} disagreeing with a full cascade: the \
                 sheet changed a rule that matches outside the subtree"
            );
        }
    }
}

/// Cascade a tree *and every off-table arena under it*.
///
/// [`cascade`] walks one table, which is every host's entry point — and
/// every construct that keeps children off the table was therefore
/// unreachable by a stylesheet. A rule could not set the padding of a
/// repeater's row, and an `#[widget_id]` inside a template was dead for
/// styling as well as for agent selectors. Only the repeater's case was
/// documented as a known limit; `Either`, `List`, `Dyn` and the enum
/// derive inherited it silently, which is the difference between a
/// decision and an accident (`docs/plan-review-fixes.md` F4.5).
///
/// Recursion is by *template*: an arena's instances share one style row,
/// so a rule matching inside a template applies to every instance of it.
/// Per-instance styling would need per-instance rows and a selector
/// vocabulary that can name a row; neither exists, and this does not
/// pretend otherwise.
pub fn cascade_deep<W: crate::widget::Widget + ?Sized>(
    tree: &mut W,
    sheet: Stylesheet,
    out: &mut [Style],
) {
    cascade(W::SUB_TABLE, W::SEEDS, sheet, out);
    for i in 0..W::SUB_TABLE.len() {
        cascade_arenas(tree.node_mut(i), sheet);
    }
}

/// Cascade every arena `node` owns, and every arena under those.
// Both parameters are unused without `alloc`: nothing implements the
// arena hooks then, because Repeat, List and Dyn are all gated.
#[cfg_attr(not(feature = "alloc"), allow(unused_variables))]
fn cascade_arenas(node: &mut dyn crate::widget::Node, sheet: Stylesheet) {
    #[cfg(feature = "alloc")]
    for a in 0..node.arena_count() {
        let Some(arena) = node.arena_mut(a) else { continue };
        let table = arena.table;
        match arena.seeds {
            Some(seeds) => cascade(table, seeds, sheet, arena.style),
            // A `Dyn` island: see `cascade_from_instance`.
            None => {
                let template = arena.template;
                cascade_from_instance(
                    table,
                    &|i| template.node(i).defaults(),
                    &|i| template.node(i).kind_name(),
                    sheet,
                    arena.style,
                );
            }
        }
    }
    // Arenas nested inside an arena's template are not reached: the
    // template is borrowed shared while its style row is borrowed
    // exclusively, so descending needs a second pass the borrow checker
    // will not give from here. A `List<Repeat<_>>` therefore styles the
    // list's rows but not the repeater's. Recorded rather than papered
    // over; the shape that fixes it is an arena walk that yields owned
    // handles, which is F4.4's territory.
}
