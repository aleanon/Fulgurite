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
///
/// **8 B**, set by `Fixed(f32)`; `Style` carries two, so a wider variant
/// would grow every node (`tests/cursor.rs`, `a_length_is_eight_bytes`).
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
    /// Its content size, **at most** its share of the remainder in this
    /// ratio — M85/T85.1 (`docs/plan-layout-input.md`). A derived row or
    /// column counts the share over its `Flexible` and `FillPortion`
    /// children together and lays a `Flexible` child out once under that
    /// maximum, so it ends at `min(content, share)`.
    ///
    /// **What it leaves goes to the `FillPortion` children, never to
    /// another `Flexible`**, so every child is still laid out exactly
    /// once: `[Flexible(1) 80 px, FILL]` in 300 px reads 80/220, and
    /// `[Flexible(1) 80 px, Flexible(1) 400 px]` reads 80/150 where CSS's
    /// freeze loop would give the second 220
    /// (`docs/layout-and-storage-decisions.md` §2,
    /// `crates/lumen-static/tests/flexible.rs`). On the cross axis, and in
    /// a stack, wrap or grid — which divide nothing — it resolves as
    /// `Shrink`. `.lss` spells it `flexible` and `flexible(n)`.
    Flexible(u16),
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

    /// A `Flexible` length — M85/T85.1.
    #[inline]
    pub fn is_flexible(&self) -> bool {
        matches!(self, Length::Flexible(_))
    }

    /// A `Flexible` length's ratio; 0 for every other case, as
    /// [`Length::portion`] is for a non-fill.
    #[inline]
    pub fn flex_portion(&self) -> u16 {
        match self {
            Length::Flexible(k) => *k,
            _ => 0,
        }
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

/// What a single-line text does when it does not fit its box —
/// M80/T80.3 (`docs/plan-text-fit.md`). Named so it cannot be confused
/// with `lumen-static-wgpu`'s private `Overflow`.
///
/// **`Clip` and `Ellipsis` are one line, cut in layout, by grapheme**:
/// the widget keeps the longest prefix that fits (with room for `…`
/// under `Ellipsis`) and paints only that, so a run never reaches past
/// its node and its damage stays the node's box. `Clip` is **not** a run
/// clipped mid-glyph: a glyph cut in half says nothing a reader can use.
/// A hard line break ends the visible line and counts as overflow. The
/// cut is logical (byte order); its visual side for right-to-left text is
/// M81's.
///
/// Read by `Label` (default `Wrap`) and `AppBar` (default `Ellipsis`; a
/// bar is one line, so it reads `Wrap` as `Ellipsis`), whose seeds declare
/// [`Fields::OVERFLOW`]. `.lss` spells it `text-overflow: ellipsis`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum TextOverflow {
    /// Break onto as many lines as the text needs — what every node did
    /// before the field existed.
    #[default]
    Wrap,
    /// One line, cut at the last grapheme boundary that fits, no mark.
    Clip,
    /// One line, cut so the prefix and a `…` fit.
    Ellipsis,
}

/// One node's computed style.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub width: Length,
    pub height: Length,
    /// Finite and never negative when it came through the cascade
    /// (M55/T55.5): there is no negative-margin model, and hit testing
    /// assumes a child lies inside its parent.
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
    /// Border thickness in logical px, drawn **inside** the node's box;
    /// 0 draws none (M27/T27.1).
    ///
    /// **A `u8` for the same reason `focus_ring` is one**: a border wider
    /// than 255 px is not a border, and `Style` had no spare tail padding
    /// left — `align`, `focus_ring`, `cursor` and `visible` fill one
    /// 4-byte slot exactly, so a lone `u8` costs four bytes and an `f32`
    /// costs the same four. The byte is taken for the semantics.
    ///
    /// **A `Style` property and not a widget field**, because the
    /// alternative is the one M25/T25.10 ran and had to undo: colour
    /// fields on widgets, then palette roles bolted on so
    /// `Host::set_palette` could re-resolve them. A border on a widget
    /// cannot be restyled by a sheet, cannot follow a palette, and cannot
    /// be set once for every bordered control.
    pub border_width: u8,
    /// The border's colour. `TRANSPARENT` draws none whatever the width.
    pub border_color: crate::display::Color,
    /// Line height in **sixteenths of the font size** — M80/T80.2
    /// (`docs/plan-text-fit.md`). `0` is the measurer's own spacing,
    /// which is what every node had before the field existed, so a tree
    /// that sets nothing paints the same pixels; 16 is 1.0×, 24 is 1.5×,
    /// 255 is 15.9×.
    ///
    /// **CSS's and iced's semantics, not fontdue's**: a run of *n* lines
    /// is `n × pitch` tall, a single line included, with
    /// `pitch = line_height / 16 × font_size`, and each line's glyphs sit
    /// at half-leading inside its pitch. Below the face's own spacing the
    /// box grows by the deficit so ink never leaves it
    /// ([`crate::cx::pitched_height`]).
    ///
    /// **A ratio, not a length**, because it is scale-invariant: `Scaled`
    /// forwards the byte untouched and the renderer multiplies by the
    /// physical size. Set through [`Patch::line_height`], which takes the
    /// ratio as an `f32` and is rounded to sixteenths by the cascade.
    /// Read by `Label`, `TextArea` and `RichText` (their seeds declare
    /// [`Fields::LINE_HEIGHT`]); a rule setting it on anything else is a
    /// `W1010` finding. Free: one of the two spare bytes in `Style`'s
    /// tail, so `Style` stays 80 B (`tests/cursor.rs`).
    pub line_height: u8,
    /// What a text that does not fit does — M80/T80.3: wrap (the
    /// default), or one line cut with or without an ellipsis
    /// ([`TextOverflow`]). Read by `Label` and `AppBar`, which make the cut
    /// in layout, so paint needs no copy of it in `NodeState`. The last of
    /// `Style`'s two spare tail bytes: `Style` stays 80 B
    /// (`tests/cursor.rs`), and the next field grows it (M82).
    pub overflow: TextOverflow,
    /// A gradient behind this node, **by index** — M82/T82.3
    /// (`docs/plan-paint-vocabulary.md`). `0` is none, the default, so a
    /// tree that sets nothing paints what it painted before; `n` is entry
    /// `n - 1` of the program's fill table ([`Fill`], handed to the host
    /// by `Program::fills` and to paint through
    /// [`Painter::fills`](crate::Painter::fills)).
    ///
    /// **An index and not the gradient**, because a `Style` is per node and
    /// a gradient is 50-odd bytes; and because a table entry names its
    /// stops by [`Role`], so a palette swap re-resolves one table rather
    /// than every node. The index never reaches retention or the journal:
    /// [`crate::ops::fill`] paints the entry's resolved stops inline.
    ///
    /// Painted **under** the flat `background`, by every kind that draws a
    /// border (their seeds declare [`Fields::BACKGROUND_FILL`]), so a fill
    /// shows where the background is transparent. Hover and press shading
    /// act on `background` only, so a fill under a transparent background
    /// shows no hover; a disabled node's stops fade to 40% alpha as
    /// `background` does. Not mirrored under RTL.
    ///
    /// **This byte grew `Style` 80 → 84** (`tests/cursor.rs`): M80 had
    /// taken both spare tail bytes, and `docs/plan-iced-roadmap.md` §2.1
    /// budgets the re-budget. 81 of the 84 bytes were used (82 since
    /// M86/T86.1's `transition`).
    pub background_fill: u8,
    /// How long the hover and press shade, and the focus ring, take to
    /// move — M86/T86.1 (`docs/plan-transitions.md`), in **tens of ms**:
    /// `12` is 120 ms, `255` the longest, 2,550 ms. `0`, the default, is
    /// instant, which is what every tree that sets nothing draws.
    ///
    /// Read by the host, not by paint: when a pointer or focus bit flips
    /// on a node whose `transition` is not 0, the host runs the change
    /// over this long and writes the level it has reached into
    /// [`crate::NodeState::pointer_mix`], which [`crate::ops::shaded`]
    /// blends from (behind the core `transitions` feature). Set through
    /// [`Patch::transition_ms`], which takes milliseconds and rounds them;
    /// `.lss` spells it `transition-duration: 150ms`. Declared by every
    /// seed that declares a hover or press shade ([`Fields::TRANSITION`]).
    ///
    /// Byte 82 of 84: `Style` does not grow (`tests/cursor.rs`), and two
    /// tail bytes are left.
    pub transition: u8,
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
    /// Whether this node and everything under it **refuses input** —
    /// M57/A16 (`docs/plan-widget-fixes.md`).
    ///
    /// A disabled node is not hit, not focused and not activated, and
    /// neither is anything inside it; it paints at a reduced alpha so it
    /// reads as unavailable, and the agent publishes it as disabled. No
    /// widget had any way to say this (`docs/widget-audit.md` X12). Free:
    /// it takes a byte of the padding `border_width`'s slot already had,
    /// and `Style` stays 80 bytes (`tests/focus.rs`).
    pub disabled: bool,
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
            border_width: 0,
            border_color: crate::display::Color::TRANSPARENT,
            line_height: 0,
            overflow: TextOverflow::Wrap,
            background_fill: 0,
            transition: 0,
            cursor: Cursor::Default,
            visible: true,
            disabled: false,
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
    /// Which of this kind's colours are **palette roles** rather than
    /// the literal values `defaults` returns — M57/A8
    /// (`docs/plan-widget-fixes.md`).
    ///
    /// Applied by the cascade after `defaults` and before any rule, against
    /// the host's palette, so a widget's own colours follow
    /// `Host::set_palette` with no stylesheet rule at all. Until this, the
    /// palette reached a node only through a rule's `color_role`, and the
    /// widgets' `const fn` defaults were light-palette literals: every
    /// application had to restyle every kind for dark mode, and the
    /// showcase did it for four, so a label read 1.30:1 and a card stayed
    /// white on a dark page (`docs/widget-audit.md` X6). A rule still
    /// overrides a role, literal or role, because rules apply after.
    pub roles: SeedRoles,
}

/// The colours of a [`Seed`] that are palette roles — M57/A8.
///
/// Three bytes, not a [`Patch`]: a seed is a static per widget kind, and
/// these are the only three colour fields `Style` has.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SeedRoles {
    pub color: Option<Role>,
    pub background: Option<Role>,
    pub border: Option<Role>,
}

impl SeedRoles {
    pub const NONE: SeedRoles = SeedRoles { color: None, background: None, border: None };

    pub const fn color(mut self, r: Role) -> Self {
        self.color = Some(r);
        self
    }

    pub const fn background(mut self, r: Role) -> Self {
        self.background = Some(r);
        self
    }

    pub const fn border(mut self, r: Role) -> Self {
        self.border = Some(r);
        self
    }

    /// Resolve these roles into `s` against `palette`.
    pub fn apply(&self, s: &mut Style, palette: &Palette) {
        if let Some(r) = self.color {
            s.color = palette.get(r);
        }
        if let Some(r) = self.background {
            s.background = palette.get(r);
        }
        if let Some(r) = self.border {
            s.border_color = palette.get(r);
        }
    }
}

impl Seed {
    /// **The mask is required** (M26/T26.15).
    ///
    /// ```
    /// # use lumen_static::style::{Fields, Seed, Style};
    /// fn defaults() -> Style { Style::default() }
    /// const S: Seed = Seed::new("mine", defaults, Fields::of(&[Fields::WIDTH]));
    /// ```
    ///
    /// A widget that declares no mask does not compile — which is the
    /// whole point, because the mask it used to get by default was the
    /// lint's off switch:
    ///
    /// ```compile_fail
    /// # use lumen_static::style::{Seed, Style};
    /// fn defaults() -> Style { Style::default() }
    /// const S: Seed = Seed::new("mine", defaults);
    /// ```
    ///
    /// It used to default to [`Fields::ALL`], and this constructor's own
    /// rustdoc called that deliberate: a kind that has not declared its
    /// reads "is not linted rather than wrongly linted". That reasoning
    /// is reversed here. `ALL` is the `W1010` lint's **off switch**, and
    /// it was what a *new* widget got by doing nothing — so the one
    /// widget most likely to have a mistargeted rule was the one the
    /// lint could not speak about. An opt-out should be spelled, which
    /// is what [`Seed::unlinted`] is for.
    ///
    /// Source-breaking for out-of-tree widgets, and taken deliberately:
    /// the seeds array is a required associated const, so a third-party
    /// impl cannot avoid this constructor. Every widget in this
    /// repository already declared a mask — 53 of the 57 call sites —
    /// and the four that did not are `Seed::WIDGET` and three test
    /// fixtures.
    pub const fn new(kind: &'static str, defaults: fn() -> Style, reads: Fields) -> Seed {
        Seed { kind, defaults, reads, roles: SeedRoles::NONE }
    }

    /// This seed with `roles` for its colours — M57/A8. See
    /// [`Seed::roles`](field@Seed::roles).
    pub const fn with_roles(mut self, roles: SeedRoles) -> Seed {
        self.roles = roles;
        self
    }

    /// This kind's style before any rule: its defaults with its roles
    /// resolved against `palette`. Every place that seeds a style from a
    /// seed goes through this, so none can forget the roles.
    pub fn style(&self, palette: &Palette) -> Style {
        let mut s = (self.defaults)();
        self.roles.apply(&mut s, palette);
        s
    }

    /// A seed that opts **out** of the `W1010` lint, spelled.
    ///
    /// For the cases where "reads everything" is the honest answer
    /// rather than an omission: [`Seed::WIDGET`], the padding slots a
    /// wrapper's table carries, and a test fixture that is not about
    /// styling.
    pub const fn unlinted(kind: &'static str, defaults: fn() -> Style) -> Seed {
        Seed { kind, defaults, reads: Fields::ALL, roles: SeedRoles::NONE }
    }


    /// The seed of a node that declares nothing — [`crate::widget::Node`]'s
    /// own defaults, spelled as a type-level fact.
    pub const WIDGET: Seed = Seed::unlinted("widget", plain_style);
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
pub struct Fields(u32);

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
    /// `border_width` and `border_color`, as one bit.
    ///
    /// **One bit for two fields, because they are one property**: a
    /// width with no colour draws nothing and a colour with no width is
    /// invisible, so a node that reads either reads both and a rule that
    /// sets either sets a border. They arrived in T27.1 and this did
    /// not, so a rule that set *only* a border reported the empty set
    /// and the lint's claim — no rule in this sheet is a no-op — was
    /// silently false for every border rule (M27/T27.11).
    ///
    /// The seventeenth bit is why `Fields` is a `u32`: the first sixteen
    /// were all taken.
    pub const BORDER: Fields = Fields(1 << 16);
    /// [`Style::line_height`] — M80/T80.2. The next free bit when it
    /// landed: `Label`, `TextArea` and `RichText` declare it, so the
    /// lint reports a rule that sets it on anything else rather than let
    /// it be a silent no-op.
    pub const LINE_HEIGHT: Fields = Fields(1 << 17);
    /// [`Style::overflow`] — M80/T80.3, the next free bit when it landed.
    /// `Label` and `AppBar` declare it.
    pub const OVERFLOW: Fields = Fields(1 << 18);
    /// [`Style::background_fill`] — M82/T82.3, the next free bit when it
    /// landed. Declared by every kind whose `paint` calls
    /// [`crate::ops::fill`] (the derive's containers, `Repeat`, and the
    /// widgets that draw a border); a rule setting it on any other kind is
    /// a `W1010` finding.
    pub const BACKGROUND_FILL: Fields = Fields(1 << 19);
    /// [`Style::transition`] — M86/T86.1, the next free bit when it
    /// landed. Declared by every seed that declares
    /// [`Fields::HOVER_DIM`] or [`Fields::PRESSED_DIM`]: a transition moves
    /// the shade those fields give, so a kind with no shade has nothing to
    /// move and a rule setting it there is a `W1010` finding.
    pub const TRANSITION: Fields = Fields(1 << 20);
    pub const ALL: Fields = Fields(u32::MAX);

    /// Every field with its name, for reports and for the tests that
    /// measure a mask field by field.
    pub const EACH: [(Fields, &'static str); 21] = [
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
        (Fields::BORDER, "border"),
        (Fields::LINE_HEIGHT, "line_height"),
        (Fields::OVERFLOW, "overflow"),
        (Fields::BACKGROUND_FILL, "background_fill"),
        (Fields::TRANSITION, "transition"),
    ];

    /// The union of a list — how a widget spells its mask at the
    /// [`Seed`] it declares.
    pub const fn of(list: &[Fields]) -> Fields {
        let mut bits = 0u32;
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
/// iced's shape (`docs/plan-appearance.md` §1): a palette is data, and
/// what is drawn is a function of it. Held by the host, and reached two
/// ways: a stylesheet rule names a [`Role`] instead of a colour
/// ([`Patch::color_role`], M25/T25.11) and the cascade resolves it here;
/// and every `layout` sees it through [`crate::LayoutCx::palette`], for a
/// widget that wants a colour no rule sets.
///
/// Named by role, not by hue, so a dark palette is the same names with
/// different values and a sheet written against one reads correctly
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
    /// A panel raised off the page: a card, a popup, a dialog, a menu —
    /// M57/A8. White on a light page; a step *lighter* than the page on a
    /// dark one, which is how a dark theme shows elevation.
    pub raised: crate::display::Color,
    /// An edge that has to be seen — a field's border, an outline button,
    /// a divider between panes — at ≥ 3:1 against `page` (WCAG 1.4.11).
    pub border: crate::display::Color,
    /// The unfilled part of a track — a slider's, a progress bar's. A
    /// quiet colour: the *filled* part carries the value, in `accent`.
    /// Also a chip's body, which sits on a surface and needs a ground
    /// that differs from it (M57/B2).
    pub track: crate::display::Color,
    /// A placeholder block — a skeleton — subdued against `raised`.
    pub placeholder: crate::display::Color,
    /// The veil behind a modal: translucent, so the page shows through.
    pub scrim: crate::display::Color,
    /// An alert: a badge's count, a destructive action.
    pub danger: crate::display::Color,
    /// Text and marks on `danger`.
    pub on_danger: crate::display::Color,
}

impl Palette {
    /// The light palette, and the default. Its values are the ones the
    /// widgets' own defaults already use, so a tree that never sets a
    /// palette looks exactly as it did before one existed.
    ///
    /// **Its `accent` and `muted` pass WCAG AA as painted** since
    /// M54/T54.6: measured on the showcase's display list under this
    /// palette, not as a table of role pairs; the values and their ratios
    /// are stated once, in `docs/results-a11y-platform.md`.
    pub const LIGHT: Palette = Palette {
        page: crate::display::Color(255, 255, 255, 255),
        surface: crate::display::Color(226, 232, 240, 255),
        // 3.06:1 on `page` (WCAG 1.4.11) — M57/A8. It was 0xbfc4cc at
        // 1.75:1, which is how an unchecked box, an off switch and an
        // unselected radio came to be the faintest thing on the screen.
        control: crate::display::Color(0x8a, 0x94, 0xa6, 255),
        text: crate::display::Color(15, 23, 42, 255),
        // M54/T54.6: (100, 116, 139) painted secondary text at 3.86:1 on
        // `surface`, 54 runs of the showcase's catalogue; this is the
        // first step down its hue that passes AA there and on `page`.
        muted: crate::display::Color(90, 104, 125, 255),
        // M54/T54.6: (53, 99, 239) was 4.07:1 on `surface`; this is the
        // first step down its own hue that passes AA on every ground the
        // contrast harness reports (`docs/results-a11y-platform.md`).
        accent: crate::display::Color(0x31, 0x5c, 0xde, 255),
        on_accent: crate::display::Color(255, 255, 255, 255),
        focus: crate::display::Color(53, 99, 239, 255),
        raised: crate::display::Color(255, 255, 255, 255),
        border: crate::display::Color(0x83, 0x8d, 0x9f, 255),
        track: crate::display::Color(0xd5, 0xdb, 0xe4, 255),
        placeholder: crate::display::Color(0xe4, 0xe9, 0xf0, 255),
        scrim: crate::display::Color(15, 23, 42, 110),
        danger: crate::display::Color(0xdc, 0x26, 0x26, 255),
        on_danger: crate::display::Color(255, 255, 255, 255),
    };

    /// The dark palette: the same roles, for a dark page.
    pub const DARK: Palette = Palette {
        page: crate::display::Color(15, 18, 26, 255),
        surface: crate::display::Color(38, 44, 58, 255),
        // 4.0:1 on `page` — M57/A8; it was (74, 82, 99) at 2.4:1.
        control: crate::display::Color(0x6b, 0x75, 0x89, 255),
        text: crate::display::Color(230, 234, 242, 255),
        muted: crate::display::Color(148, 160, 180, 255),
        accent: crate::display::Color(99, 140, 255, 255),
        on_accent: crate::display::Color(10, 14, 24, 255),
        focus: crate::display::Color(130, 170, 255, 255),
        raised: crate::display::Color(0x1f, 0x25, 0x33, 255),
        // 3:1 on `raised` as well as on `page`: a divider sits on a
        // panel as often as on the page (M57/A8, `palette_defaults.rs`).
        border: crate::display::Color(0x6b, 0x75, 0x89, 255),
        track: crate::display::Color(0x39, 0x41, 0x52, 255),
        placeholder: crate::display::Color(0x2c, 0x33, 0x42, 255),
        scrim: crate::display::Color(0, 0, 0, 150),
        danger: crate::display::Color(0xf8, 0x71, 0x71, 255),
        on_danger: crate::display::Color(0x1a, 0x0a, 0x0a, 255),
    };

    /// The colour this palette gives `role`.
    pub const fn get(&self, role: Role) -> crate::display::Color {
        match role {
            Role::Page => self.page,
            Role::Surface => self.surface,
            Role::Control => self.control,
            Role::Text => self.text,
            Role::Muted => self.muted,
            Role::Accent => self.accent,
            Role::OnAccent => self.on_accent,
            Role::Focus => self.focus,
            Role::Raised => self.raised,
            Role::Border => self.border,
            Role::Track => self.track,
            Role::Placeholder => self.placeholder,
            Role::Scrim => self.scrim,
            Role::Danger => self.danger,
            Role::OnDanger => self.on_danger,
        }
    }
}

/// One of a [`Palette`]'s colours, by name — what a stylesheet rule sets
/// when it themes rather than paints (M25/T25.11).
///
/// A rule that says `color_role(Role::Text)` resolves to whatever the
/// host's palette calls text, and resolves again when the palette is
/// swapped; a rule that says `color(INK)` keeps `INK` under every
/// palette. One byte, so a sheet of them costs what a sheet of colours
/// did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Page,
    Surface,
    Control,
    Text,
    Muted,
    Accent,
    OnAccent,
    Focus,
    Raised,
    Border,
    Track,
    Placeholder,
    Scrim,
    Danger,
    OnDanger,
}

/// One entry of a program's fill table — M82/T82.3
/// (`docs/plan-paint-vocabulary.md`, "A gradient in `Style` is an index").
///
/// A node selects it with [`Style::background_fill`]: `n` is entry
/// `n - 1`. `stops` holds the colours as written; a stop whose `roles`
/// slot is `Some` takes that [`Role`]'s colour from the host's palette
/// instead, by [`Fill::resolve`], which `Host::set_fills` and
/// `Host::set_palette` run — so a fill themes the way a role-named
/// `background` does (M25/T25.11). `shape` is resolution-free
/// ([`GradShape`](crate::display::GradShape)), so a `const` entry needs
/// no node size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fill {
    pub stops: crate::display::Stops,
    pub roles: [Option<Role>; crate::display::MAX_STOPS],
    pub shape: crate::display::GradShape,
}

impl Fill {
    /// A fill of literal stops, no role.
    pub const fn new(stops: crate::display::Stops, shape: crate::display::GradShape) -> Fill {
        Fill { stops, roles: [None; crate::display::MAX_STOPS], shape }
    }

    /// The same fill with stop `k`'s colour taken from `role`.
    pub const fn role(mut self, k: usize, role: Role) -> Fill {
        self.roles[k] = Some(role);
        self
    }

    /// The fill as painted under `palette`: every role-named stop's colour
    /// replaced by the palette's, the rest untouched.
    pub fn resolve(&self, palette: &Palette) -> Fill {
        let mut out = *self;
        for (k, r) in self.roles.iter().enumerate() {
            if let Some(role) = r {
                out.stops.color[k] = palette.get(*role);
            }
        }
        out
    }
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
///
/// **A length that cannot be laid out is repaired, and reported** — a
/// non-finite value becomes its field's type default, and a
/// non-finite or negative padding edge becomes 0 ([`Patch::needs_repair`], M55/T55.5). The agent
/// reports each such field as `W1012`.
///
/// `PartialEq` since M61/T61.2: the `.lss` parity gates compare a sheet
/// `include_lss!` emitted with one read at run time, and with one written
/// by hand (`docs/plan-lss.md`). A derive, so it costs nothing a caller
/// does not call.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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
    /// `color`, as a palette role resolved at cascade time (M25/T25.11).
    /// **Wins over `color`** when both are set; the setters clear the
    /// other, so only a struct literal can set both.
    pub color_role: Option<Role>,
    /// `background`, the same way.
    pub background_role: Option<Role>,
    pub radius: Option<f32>,
    pub font_size: Option<f32>,
    pub font_weight: Option<f32>,
    pub shadow: Option<f32>,
    pub pressed_dim: Option<f32>,
    pub hover_dim: Option<f32>,
    pub border_width: Option<u8>,
    pub border_color: Option<crate::display::Color>,
    pub border_color_role: Option<Role>,
    /// [`Style::disabled`] — M57/A16.
    pub disabled: Option<bool>,
    /// [`Style::line_height`] as the **ratio** a rule wrote (`1.5`), not
    /// the byte: the cascade rounds it to sixteenths ([`Patch::line_height`]).
    pub line_height: Option<f32>,
    /// [`Style::overflow`] — M80/T80.3.
    pub overflow: Option<TextOverflow>,
    /// [`Style::background_fill`] — M82/T82.3: the fill table index, `0`
    /// for none.
    pub background_fill: Option<u8>,
    /// [`Style::transition`] — M86/T86.1: already in tens of ms, as
    /// [`Patch::transition_ms`] rounded it.
    pub transition: Option<u8>,
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
            color_role: None,
            background_role: None,
            radius: None,
            font_size: None,
            font_weight: None,
            shadow: None,
            pressed_dim: None,
            hover_dim: None,
            border_width: None,
            border_color: None,
            border_color_role: None,
            disabled: None,
            line_height: None,
            overflow: None,
            background_fill: None,
            transition: None,
        }
    }

    /// Refuse input, and look it — M57/A16 ([`Style::disabled`]).
    pub const fn disabled(mut self, d: bool) -> Self {
        self.disabled = Some(d);
        self
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
        self.color_role = None;
        self
    }

    pub const fn background(mut self, c: crate::display::Color) -> Self {
        self.background = Some(c);
        self.background_role = None;
        self
    }

    /// The text or mark colour, as whatever the host's palette calls
    /// `role` — re-resolved when the palette changes (M25/T25.11).
    pub const fn color_role(mut self, role: Role) -> Self {
        self.color_role = Some(role);
        self.color = None;
        self
    }

    /// The background, as a palette role (M25/T25.11).
    pub const fn background_role(mut self, role: Role) -> Self {
        self.background_role = Some(role);
        self.background = None;
        self
    }

    pub const fn border_width(mut self, w: u8) -> Self {
        self.border_width = Some(w);
        self
    }

    pub const fn border_color(mut self, c: crate::display::Color) -> Self {
        self.border_color = Some(c);
        self.border_color_role = None;
        self
    }

    /// A border that follows the palette, re-resolved by
    /// `Host::set_palette` like any other role (M25/T25.11).
    pub const fn border_color_role(mut self, role: Role) -> Self {
        self.border_color_role = Some(role);
        self.border_color = None;
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

    /// Line height as a multiple of the font size — M80/T80.2. `1.5` is
    /// 1.5×; `0.0` (or anything that rounds to it, a NaN included) is the
    /// measurer's own spacing.
    ///
    /// **Stored as written and rounded by the cascade** to the nearest
    /// sixteenth ([`Style::line_height`]), saturating at 255/16: iced's
    /// default 1.3 becomes 21/16 = 1.3125. A ratio rather than a length,
    /// so it means the same at every display scale.
    pub const fn line_height(mut self, ratio: f32) -> Self {
        self.line_height = Some(ratio);
        self
    }

    /// What a text that does not fit does — M80/T80.3: wrap, or one line
    /// cut with ([`TextOverflow::Ellipsis`]) or without
    /// ([`TextOverflow::Clip`]) a `…`. Read by `Label` and `AppBar`.
    pub const fn overflow(mut self, v: TextOverflow) -> Self {
        self.overflow = Some(v);
        self
    }

    /// A gradient behind the node: entry `index - 1` of the program's
    /// fill table, `0` for none — M82/T82.3 ([`Style::background_fill`]).
    /// Records [`Fields::BACKGROUND_FILL`]. `.lss` spells it
    /// `background-fill: 1`.
    pub const fn background_fill(mut self, index: u8) -> Self {
        self.background_fill = Some(index);
        self
    }

    /// How long the hover and press shade and the focus ring take to move,
    /// in ms — M86/T86.1 ([`Style::transition`]). **Rounded to the
    /// nearest 10 ms and saturating at 2,550**, because the field is a
    /// byte of tens of ms; `0` is instant. Records [`Fields::TRANSITION`].
    /// `.lss` spells it `transition-duration: 150ms`, and refuses a value
    /// that is not a multiple of 10 or is above 2,550 rather than round it.
    pub const fn transition_ms(mut self, ms: u16) -> Self {
        let tens = (ms as u32 + 5) / 10;
        self.transition = Some(if tens > 255 { 255 } else { tens as u8 });
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
    /// Apply the fields this patch sets, as written. The cascade repairs
    /// the result when the sheet carries a length that cannot be laid out
    /// ([`Patch::needs_repair`], M55/T55.5).
    fn apply_to(&self, s: &mut Style, palette: &Palette) {
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
        if let Some(v) = self.border_width {
            s.border_width = v;
        }
        if let Some(v) = self.disabled {
            s.disabled = v;
        }
        if let Some(r) = self.border_color_role {
            s.border_color = palette.get(r);
        } else if let Some(v) = self.border_color {
            s.border_color = v;
        }
        if let Some(r) = self.color_role {
            s.color = palette.get(r);
        } else if let Some(v) = self.color {
            s.color = v;
        }
        if let Some(r) = self.background_role {
            s.background = palette.get(r);
        } else if let Some(v) = self.background {
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
        if let Some(v) = self.line_height {
            s.line_height = sixteenths(v);
        }
        if let Some(v) = self.overflow {
            s.overflow = v;
        }
        if let Some(v) = self.background_fill {
            s.background_fill = v;
        }
        if let Some(v) = self.transition {
            s.transition = v;
        }
    }
}

/// A line-height ratio as [`Style::line_height`]'s byte: the nearest
/// sixteenth, saturating at 255, with a negative ratio or a NaN as 0
/// (the measurer's own spacing) — M80/T80.2. `as u8` saturates and maps
/// NaN to 0, which is exactly the rule, so this needs no branch.
#[inline]
pub fn sixteenths(ratio: f32) -> u8 {
    (ratio * 16.0 + 0.5) as u8
}

/// A length that can be laid out (M55/T55.5).
#[inline]
fn ok(v: f32) -> bool {
    v.is_finite()
}

/// A padding edge that can be laid out: finite and not negative. There
/// is no negative-margin model, and hit testing assumes a child lies
/// inside its parent.
#[inline]
fn ok_edge(v: f32) -> bool {
    v.is_finite() && v >= 0.0
}

/// One value a patch carries that the cascade will not apply as written
/// — M55/T55.5. Reported by the sheet lint as [`SheetIssue::Repaired`],
/// and by the agent as `W1012`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Repair {
    /// The `Patch` field, by name (`"padding"`, `"spacing"`, ...).
    pub field: &'static str,
    /// The value as written, as bits so the type can be `Eq`.
    bits: u32,
}

impl Repair {
    /// The value as written: non-finite, or a negative padding edge.
    pub fn value(&self) -> f32 {
        f32::from_bits(self.bits)
    }
}

impl Patch {
    /// Whether this patch carries a length that cannot be laid out — the
    /// cascade's per-sheet question (M55/T55.5, R16-F5, R16-F6).
    ///
    /// When any rule of a sheet answers yes, the cascade repairs each
    /// node's style after its rules apply: a non-finite length becomes
    /// its field's type default (`Style::default()`'s), and a non-finite
    /// or negative padding edge becomes 0. The cascade is the one place
    /// author lengths enter, so it is the choke point, and a clean sheet
    /// pays one scan of its rules per cascade and nothing per node.
    /// Nothing is repaired silently: [`Patch::repairs`] is the same test,
    /// reported by the sheet lint as [`SheetIssue::Repaired`] and by the
    /// agent as `W1012`.
    ///
    /// Before, one NaN spacing made most of a tree's `NodeState`
    /// non-finite — every `contains` against a NaN rect is false, so the
    /// window was blank and unclickable with no diagnostic — and a
    /// negative padding widened the content box, so `place` put children
    /// outside their parent, painted and never hit
    /// (`crates/lumen-static-shell/tests/non_finite_style.rs`). Written apart from [`Patch::repairs`] because
    /// it is linked into every build that cascades, firmware included,
    /// and the field names `repairs` reports are the lint's, not the
    /// frame's (`scripts/check_text.sh` bounds what a stylesheet costs).
    pub fn needs_repair(&self) -> bool {
        let len = |l: Option<Length>| matches!(l, Some(Length::Fixed(x)) if !ok(x));
        let val = |v: Option<f32>| matches!(v, Some(x) if !ok(x));
        len(self.width)
            || len(self.height)
            || self.padding.is_some_and(|e| {
                !(ok_edge(e.left) && ok_edge(e.right) && ok_edge(e.top) && ok_edge(e.bottom))
            })
            || val(self.spacing)
            || val(self.radius)
            || val(self.font_size)
            || val(self.font_weight)
            || val(self.pressed_dim)
            || val(self.hover_dim)
            || val(self.shadow)
    }

    /// Every field this patch sets to a value the cascade repairs — the
    /// same test [`Patch::needs_repair`] applies, one [`Repair`] per field
    /// (a padding reports its first bad edge). M55/T55.5.
    pub fn repairs(&self, report: &mut dyn FnMut(Repair)) {
        let mut bad = |field: &'static str, v: f32| report(Repair { field, bits: v.to_bits() });
        for (field, len) in [("width", self.width), ("height", self.height)] {
            if let Some(Length::Fixed(x)) = len {
                if !ok(x) {
                    bad(field, x);
                }
            }
        }
        if let Some(p) = self.padding {
            if let Some(v) =
                [p.left, p.right, p.top, p.bottom].into_iter().find(|v| !ok_edge(*v))
            {
                bad("padding", v);
            }
        }
        for (field, v) in [
            ("spacing", self.spacing),
            ("radius", self.radius),
            ("font_size", self.font_size),
            ("font_weight", self.font_weight),
            ("pressed_dim", self.pressed_dim),
            ("hover_dim", self.hover_dim),
            ("shadow", self.shadow),
        ] {
            if let Some(v) = v {
                if !ok(v) {
                    bad(field, v);
                }
            }
        }
    }
}

impl Patch {
    /// The fields this patch sets — what the sheet lint compares against
    /// the fields a matched kind reads (M25/T25.2).
    pub const fn fields(&self) -> Fields {
        let mut f = 0u32;
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
        // A role sets the field it resolves into, so the lint judges it
        // by that field (M25/T25.11).
        if self.color.is_some() || self.color_role.is_some() {
            f |= Fields::COLOR.0;
        }
        if self.background.is_some() || self.background_role.is_some() {
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
        // One bit for the pair: see `Fields::BORDER`.
        if self.border_width.is_some()
            || self.border_color.is_some()
            || self.border_color_role.is_some()
        {
            f |= Fields::BORDER.0;
        }
        if self.line_height.is_some() {
            f |= Fields::LINE_HEIGHT.0;
        }
        if self.overflow.is_some() {
            f |= Fields::OVERFLOW.0;
        }
        if self.background_fill.is_some() {
            f |= Fields::BACKGROUND_FILL.0;
        }
        if self.transition.is_some() {
            f |= Fields::TRANSITION.0;
        }
        Fields(f)
    }
}

/// One selector and what it changes. `PartialEq` since M61/T61.2, as
/// [`Patch`] is.
#[derive(Clone, Copy, Debug, PartialEq)]
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
/// styles change only when the sheet is replaced, the palette its roles
/// name is (M25/T25.11), or the tree's shape is (a hot reload), which is
/// exactly when
/// [`Host::set_stylesheet`](../../lumen_static_shell/struct.Host.html),
/// `Host::set_palette` and `Host::new` run it.
///
/// A `pass::frame` helper used to cascade and lay out together, on the
/// theory that the two belong in one step. It had zero callers — every
/// host and test open-coded the pair — and once `defaults()` became
/// state-independent it was not merely unused but misleading, since it
/// implied the cascade had to be re-run for a frame to be correct. It
/// was deleted rather than wired up. `docs/plan-review-fixes.md` F3.3.
///
/// A rule's palette roles resolve against [`Palette::LIGHT`]; a host with
/// a palette of its own calls [`cascade_with`].
pub fn cascade(table: &[NodeDesc], seeds: &[Seed], sheet: Stylesheet, out: &mut [Style]) {
    cascade_with(table, seeds, sheet, &Palette::LIGHT, out);
}

/// [`cascade`], resolving the sheet's palette roles against `palette`
/// (M25/T25.11).
pub fn cascade_with(
    table: &[NodeDesc],
    seeds: &[Seed],
    sheet: Stylesheet,
    palette: &Palette,
    out: &mut [Style],
) {
    cascade_range(table, seeds, sheet, palette, out, 0..table.len());
}

/// [`cascade_with`], restricted to a range of the table.
fn cascade_range(
    table: &[NodeDesc],
    seeds: &[Seed],
    sheet: Stylesheet,
    palette: &Palette,
    out: &mut [Style],
    range: core::ops::Range<usize>,
) {
    debug_assert_eq!(table.len(), out.len());
    debug_assert_eq!(table.len(), seeds.len());
    // **Which specificity levels this sheet actually has** (M26/T26.6).
    //
    // The loop below runs four passes per node so that a rule of higher
    // specificity always lands last — the cascade's whole ordering
    // rule. Three of those four are usually empty: every sheet in this
    // workspace uses `Id` and `Kind` and neither `Class` nor `Any`, so
    // half the passes scanned every rule to match nothing, per node.
    //
    // A property of the *sheet*, so it is computed once here rather
    // than re-derived per node. The pass order is untouched: this skips
    // empty passes, it does not reorder or merge them.
    let mut levels = 0u8;
    // M55/T55.5: whether any rule carries a length that cannot be laid
    // out. Per sheet, like `levels`, so a clean sheet applies its patches
    // as written and pays nothing per node for the repair.
    let mut repair = false;
    for r in sheet {
        levels |= 1 << r.selector.specificity();
        repair |= r.patch.needs_repair();
    }
    for (i, d) in table.iter().enumerate().skip(range.start).take(range.len()) {
        let mut s = seeds[i].style(palette);
        let kind = seeds[i].kind;
        for spec in 0..=3u8 {
            if levels & (1 << spec) == 0 {
                continue;
            }
            for r in sheet {
                if r.selector.specificity() == spec && matches(&r.selector, d, kind) {
                    r.patch.apply_to(&mut s, palette);
                }
            }
        }
        out[i] = s;
    }
    // Its own pass rather than a branch in the loop above, so a clean
    // sheet runs exactly the loop it ran before M55 (the branch measured
    // outside `cascade_deep`'s spread — `docs/results-derive-dx.md`).
    if repair {
        for i in range {
            repair_style(&mut out[i]);
        }
    }
}

/// The cascade's repair of one node's style, after its rules applied —
/// M55/T55.5. A non-finite length takes its field's type default
/// (`Style::default()`'s), and a non-finite or negative padding edge
/// becomes 0. Runs only for a sheet with a rule that
/// [`Patch::needs_repair`]. Cold and out of line: it is a development
/// error's path, and the firmware pays for its code, not for its calls
/// (`docs/results-embedded.md`, M55/T55.5).
#[cold]
#[inline(never)]
fn repair_style(s: &mut Style) {
    let d = Style::default();
    if matches!(s.width, Length::Fixed(x) if !ok(x)) {
        s.width = d.width;
    }
    if matches!(s.height, Length::Fixed(x) if !ok(x)) {
        s.height = d.height;
    }
    let p = &mut s.padding;
    for e in [&mut p.left, &mut p.right, &mut p.top, &mut p.bottom] {
        if !ok_edge(*e) {
            *e = 0.0;
        }
    }
    for (v, d) in [
        (&mut s.spacing, d.spacing),
        (&mut s.radius, d.radius),
        (&mut s.font_size, d.font_size),
        (&mut s.font_weight, d.font_weight),
        (&mut s.pressed_dim, d.pressed_dim),
        (&mut s.hover_dim, d.hover_dim),
        (&mut s.shadow, d.shadow),
    ] {
        if !ok(*v) {
            *v = d;
        }
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
    /// It matches, and sets a length the cascade repairs rather than
    /// applies: a non-finite value, or a negative padding edge — M55/T55.5.
    /// One per field; `W1012` in the agent.
    Repaired(Repair),
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
    // **Ask once whether this tree has any arena at all** — M26/T26.17
    // finding 10.
    //
    // The arena walk below sits inside the rule loop, so a 27-rule sheet
    // asked all 901 nodes of the benchmark tree 27 times whether they
    // owned an arena: 24 327 virtual calls at 5.2 ns, which measured as
    // **three quarters of the whole lint** (166 124 -> 40 486 ns with the
    // walk removed outright — `docs/results-frame-and-types.md` T26.17
    // batch three). `lint_sheet_deep` runs on every `Host::set_stylesheet`.
    //
    // One walk, before the loop, with an early exit at the first arena
    // found. A tree with no arenas now pays 901 asks instead of 901 per
    // rule; a tree with arenas exits on the first one and then behaves
    // exactly as before, so there is no case this makes slower.
    //
    // This is deliberately *not* the plan's half (b), which hoisted the
    // walk by accumulating `matched`/`read` per rule — that needs an
    // array as long as the sheet, and the sheet's length is not known
    // here. Finding 9's const arena-index list is the fix that removes
    // the remaining walk; it needs a per-type const forwarded through
    // 54 `Widget` impls, and a wrapper that forwards it wrongly silently
    // stops styling the arena under it.
    #[cfg(feature = "alloc")]
    let any_arena = {
        let mut found = false;
        for i in 0..W::SUB_TABLE.len() {
            if tree.node_mut(i).arena_count() > 0 {
                found = true;
                break;
            }
        }
        found
    };

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
            if !any_arena {
                break;
            }
            let node = tree.node_mut(i);
            for a in 0..node.arena_count() {
                let Some(arena) = node.arena_mut(a) else { continue };
                // **No `None` arm since M26/T26.10.** It used
                // `Fields::ALL` for a `Dyn` island's nodes, because an
                // island had no seeds to consult — so `Unread` could
                // not fire for anything inside one. Every arena carries
                // its seeds now.
                for (k, d) in arena.table.iter().enumerate() {
                    visit(d, arena.seeds[k].kind, arena.seeds[k].reads);
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
        // M55/T55.5: a length the cascade repaired. Per rule and field,
        // not per node — the value is the rule's, and a rule matching a
        // thousand rows is one thing to fix, as `Unread` is.
        rule.patch.repairs(&mut |x| report(SheetFinding { rule: r, issue: SheetIssue::Repaired(x) }));
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
pub const fn index_of_wid(table: &[NodeDesc], wid: &str) -> Option<usize> {
    // A node without `#[widget_id]` carries `""`, so an empty argument
    // would match the first unnamed node — which is usually the root,
    // and would restyle the whole tree. Nothing is named `""`.
    if wid.is_empty() {
        return None;
    }
    // **`const`, so a caller can resolve an id at compile time**
    // (M26/T26.14). A typo'd or renamed `#[widget_id]` marks *nothing*:
    // this returns `None` — it cannot return a wrong index — and the
    // frame quietly fails to redraw. A `const` lookup turns that into a
    // compile error for any caller willing to take one, which
    // `wid_index!` below is the spelling of.
    //
    // Written as an index loop because `Iterator::position` and `==` on
    // `str` are not `const`; `bytes_eq` is the same comparison spelled
    // for a const context.
    let mut i = 0;
    while i < table.len() {
        if bytes_eq(table[i].wid, wid) {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// How many nodes of `table` carry `wid`.
///
/// **For the addressing path, not for styling** (M26/T26.16). A
/// duplicated `#[widget_id]` is accepted, and an operation that wants
/// *one index* for it would have to pick one of the set. Until
/// M46/T46.2 the host's did — `Host::invalidate` marked one of two,
/// forever. Now `Host::invalidate` and `Host::restyle_subtree` act on
/// every match ([`WidMatches`], [`cascade_wid_with`]), which is how
/// styling already reads an id; only [`crate::wid_index`], which returns
/// a single index, still refuses a duplicate. Styling must keep
/// matching them all: `examples/glasswall` styles four intentional
/// duplicates by selector and addresses none of them, which is why the
/// rejection cannot live in the derive. The derive cannot tell an
/// addressing use from a styling one; [`crate::wid_index`] can, because being
/// asked for an index *is* the addressing use.
pub const fn count_wid(table: &[NodeDesc], wid: &str) -> usize {
    if wid.is_empty() {
        return 0;
    }
    let (mut i, mut n) = (0, 0);
    while i < table.len() {
        if bytes_eq(table[i].wid, wid) {
            n += 1;
        }
        i += 1;
    }
    n
}

/// How many nodes of `table` carry the style class `class` —
/// `#[widget_class]`.
///
/// `const`, as [`count_wid`] is. Written for `include_lss!` (M61/T61.4)
/// and superseded there by [`classes_of`], which collects the classes in
/// one pass instead of walking the table once per class. An empty
/// `class` matches nothing.
pub const fn count_class(table: &[NodeDesc], class: &str) -> usize {
    if class.is_empty() {
        return 0;
    }
    let (mut i, mut n) = (0, 0);
    while i < table.len() {
        let classes = table[i].classes;
        let mut j = 0;
        while j < classes.len() {
            if bytes_eq(classes[j], class) {
                n += 1;
                break;
            }
            j += 1;
        }
        i += 1;
    }
    n
}

/// How many nodes of `table` carry an author id — the length of
/// [`wids_of`]'s array.
///
/// **For `include_lss!`'s tree checks** (M61, `docs/plan-lss.md` §2.3).
/// Asserting each `#id` with [`count_wid`] walks the whole table once per
/// id, and in `const` evaluation that is what a check costs: 42 of them
/// over the showcase's table were 88 ms of a warm build, most of kill rule
/// K2's 100 ms (`docs/results-lss.md`). Collecting the ids once and
/// asserting against the short list walks the table once.
pub const fn wid_count(table: &[NodeDesc]) -> usize {
    let (mut i, mut n) = (0, 0);
    while i < table.len() {
        if !table[i].wid.is_empty() {
            n += 1;
        }
        i += 1;
    }
    n
}

/// Every author id in `table`, in table order (`K` is [`wid_count`]).
pub const fn wids_of<const K: usize>(table: &[NodeDesc]) -> [&'static str; K] {
    let mut out = [""; K];
    let (mut i, mut k) = (0, 0);
    while i < table.len() && k < K {
        if !table[i].wid.is_empty() {
            out[k] = table[i].wid;
            k += 1;
        }
        i += 1;
    }
    out
}

/// How many style classes `table` carries, counting a class once per
/// node that has it — the length of [`classes_of`]'s array.
pub const fn class_count(table: &[NodeDesc]) -> usize {
    let (mut i, mut n) = (0, 0);
    while i < table.len() {
        n += table[i].classes.len();
        i += 1;
    }
    n
}

/// Every style class in `table`, flattened (`K` is [`class_count`]).
pub const fn classes_of<const K: usize>(table: &[NodeDesc]) -> [&'static str; K] {
    let mut out = [""; K];
    let (mut i, mut k) = (0, 0);
    while i < table.len() {
        let classes = table[i].classes;
        let mut j = 0;
        while j < classes.len() && k < K {
            out[k] = classes[j];
            k += 1;
            j += 1;
        }
        i += 1;
    }
    out
}

/// Whether `list` holds `name` — the assertion `include_lss!` makes
/// against [`wids_of`] and [`classes_of`]. An empty `name` is never held.
pub const fn names_contain(list: &[&str], name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let mut i = 0;
    while i < list.len() {
        if bytes_eq(list[i], name) {
            return true;
        }
        i += 1;
    }
    false
}

/// `a == b` for `&str`, in a `const` context.
const fn bytes_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// The table index of `$wid` in `$tree`, **checked at compile time**.
///
/// ```
/// # use lumen_static::{wid_index, StaticTree, Widget};
/// # use lumen_static_widgets::ops::Label;
/// #[derive(StaticTree, Default)]
/// #[widget(column, spacing = 0.0)]
/// struct Ui {
///     #[widget_id("title")]
///     title: Label,
/// }
/// const TITLE: usize = wid_index!(Ui, "title");
/// assert_eq!(TITLE, 1);
/// ```
///
/// A misspelled id fails the build rather than marking nothing:
///
/// ```compile_fail
/// # use lumen_static::{wid_index, StaticTree, Widget};
/// # use lumen_static_widgets::ops::Label;
/// # #[derive(StaticTree, Default)]
/// # #[widget(column, spacing = 0.0)]
/// # struct Ui {
/// #     #[widget_id("title")]
/// #     title: Label,
/// # }
/// const OOPS: usize = wid_index!(Ui, "titel");
/// ```
///
/// So does a duplicated one — addressing it would act on the first of
/// them, silently (M26/T26.16):
///
/// ```compile_fail
/// # use lumen_static::{wid_index, StaticTree, Widget};
/// # use lumen_static_widgets::ops::Label;
/// #[derive(StaticTree, Default)]
/// #[widget(column, spacing = 0.0)]
/// struct Two {
///     #[widget_id("row")]
///     a: Label,
///     #[widget_id("row")]
///     b: Label,
/// }
/// const AMBIGUOUS: usize = wid_index!(Two, "row");
/// ```
#[macro_export]
macro_rules! wid_index {
    ($tree:ty, $wid:literal) => {{
        const IDX: usize = {
            let table = <$tree as $crate::widget::Widget>::SUB_TABLE;
            // **Duplicates are refused here, and only here** — see
            // `count_wid`. Addressing one of two identically named
            // nodes silently acts on the first; styling both is
            // deliberate and must keep working.
            match $crate::style::count_wid(table, $wid) {
                1 => {}
                0 => panic!(concat!("no `#[widget_id(\"", $wid, "\")]` in this tree")),
                _ => panic!(concat!(
                    "`#[widget_id(\"", $wid, "\")]` is on more than one node: \
                     addressing it would act on the first. Styling duplicates \
                     by selector is fine; addressing them is not.",
                )),
            }
            match $crate::style::index_of_wid(table, $wid) {
                Some(i) => i,
                None => panic!("unreachable: counted one"),
            }
        };
        IDX
    }};
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
///
/// Roles resolve against [`Palette::LIGHT`]; see [`cascade_subtree_with`].
pub fn cascade_subtree<W: crate::widget::Widget + ?Sized>(
    tree: &mut W,
    sheet: Stylesheet,
    out: &mut [Style],
    root: usize,
) {
    cascade_subtree_with(tree, sheet, &Palette::LIGHT, out, root);
}

/// [`cascade_subtree`], resolving roles against `palette` (M25/T25.11).
/// The palette must be the one the rest of `out` was cascaded against,
/// for the same reason the sheet must: the debug check compares with a
/// full cascade under it.
pub fn cascade_subtree_with<W: crate::widget::Widget + ?Sized>(
    tree: &mut W,
    sheet: Stylesheet,
    palette: &Palette,
    out: &mut [Style],
    root: usize,
) {
    cascade_roots(tree, sheet, palette, out, core::iter::once(root));
}

/// [`cascade_subtree_with`] for **every** node carrying `#[widget_id(wid)]`
/// — what `Host::restyle_subtree` runs (M46/T46.2). Returns how many
/// subtrees were re-cascaded; 0 means the id matched nothing.
///
/// An id is a set wherever styling reads it: a rule on `Selector::Id`
/// matches every node carrying it, so re-cascading only the first of two
/// left the second on the old sheet. Every root is cascaded **before**
/// the debug witness runs, and the witness checks the union of the
/// ranges: checking after each root would compare the first root's
/// result against a full cascade that has already restyled the second,
/// and fire on a correct call.
pub fn cascade_wid_with<W: crate::widget::Widget + ?Sized>(
    tree: &mut W,
    sheet: Stylesheet,
    palette: &Palette,
    out: &mut [Style],
    wid: &str,
) -> usize {
    let table = W::SUB_TABLE;
    let roots = WidMatches { table, wid, at: 0 };
    let n = roots.clone().count();
    if n > 0 {
        cascade_roots(tree, sheet, palette, out, roots);
    }
    n
}

/// Every main-table index carrying `wid`, in table order — the set
/// [`count_wid`] counts. An empty `wid` matches nothing, as there.
#[derive(Clone)]
pub struct WidMatches<'a> {
    table: &'a [NodeDesc],
    wid: &'a str,
    at: usize,
}

impl<'a> WidMatches<'a> {
    pub fn new(table: &'a [NodeDesc], wid: &'a str) -> Self {
        WidMatches { table, wid, at: 0 }
    }
}

impl Iterator for WidMatches<'_> {
    type Item = usize;
    fn next(&mut self) -> Option<usize> {
        if self.wid.is_empty() {
            return None;
        }
        while self.at < self.table.len() {
            let i = self.at;
            self.at += 1;
            if bytes_eq(self.table[i].wid, self.wid) {
                return Some(i);
            }
        }
        None
    }
}

/// The scoped cascade over one or more roots, then the debug witness
/// over the union of their ranges.
#[cfg_attr(not(feature = "alloc"), allow(unused_variables))]
fn cascade_roots<W: crate::widget::Widget + ?Sized>(
    tree: &mut W,
    sheet: Stylesheet,
    palette: &Palette,
    out: &mut [Style],
    roots: impl Iterator<Item = usize> + Clone,
) {
    let table = W::SUB_TABLE;

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
        cascade_with(table, W::SEEDS, sheet, palette, &mut full);
        full
    };

    for root in roots.clone() {
        let range = subtree_range(table, root);
        cascade_range(table, W::SEEDS, sheet, palette, out, range.clone());
        // Only where arenas exist (M52/T52.4): see `cascade_deep_with`.
        #[cfg(feature = "alloc")]
        for i in range {
            cascade_arenas(tree.node_mut(i), sheet, palette);
        }
    }

    #[cfg(all(debug_assertions, feature = "alloc"))]
    for (i, w) in witness.iter().enumerate() {
        if roots.clone().any(|r| subtree_range(table, r).contains(&i)) {
            continue;
        }
        if out[i] == *w {
            continue;
        }
        // Before blaming the sheet, ask whether the root's id is on
        // another node: then the rule that "matches outside the subtree"
        // matches a second copy of the same id, which is a caller that
        // addressed one of a set (M46/T46.2).
        let root = roots.clone().next().unwrap_or(0);
        let wid = table[root].wid;
        let copies = count_wid(table, wid);
        if copies > 1 {
            panic!(
                "cascade_subtree({root}) left node {i} disagreeing with a full cascade: \
                 `#[widget_id(\"{wid}\")]` is on {copies} nodes, and a scoped cascade of \
                 one of them restyles only that one -- re-cascade every match \
                 (`cascade_wid_with`, which `Host::restyle_subtree` uses)"
            );
        }
        panic!(
            "cascade_subtree({root}) left node {i} disagreeing with a full cascade: the \
             sheet changed a rule that matches outside the subtree\n  scoped: {:?}\n  full:   {w:?}",
            out[i]
        );
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
///
/// Roles resolve against [`Palette::LIGHT`]; a host holding a palette
/// calls [`cascade_deep_with`].
pub fn cascade_deep<W: crate::widget::Widget + ?Sized>(
    tree: &mut W,
    sheet: Stylesheet,
    out: &mut [Style],
) {
    cascade_deep_with(tree, sheet, &Palette::LIGHT, out);
}

/// [`cascade_deep`], resolving the sheet's palette roles against
/// `palette` — what `Host::set_stylesheet` and `Host::set_palette` run
/// (M25/T25.11).
///
/// **Without `alloc` this is `cascade_with` and nothing else** (M52/T52.4):
/// there are no arenas, so the arena loop is compiled out with them. It
/// used to stay in with an empty body, and its `&mut dyn Node` coercion
/// linked every widget's `on_event` and `hit` into a leaf firmware that
/// takes no input. **An `alloc` build still pays that** — the loop asks
/// every node, through the vtable, whether it owns an arena — until a
/// derive-computed "has arenas" constant lets it skip the types that
/// cannot (M26 Phase D finding 9, promoted and not built);
/// `docs/results-embedded.md` records what it costs.
#[cfg_attr(not(feature = "alloc"), allow(unused_variables))]
pub fn cascade_deep_with<W: crate::widget::Widget + ?Sized>(
    tree: &mut W,
    sheet: Stylesheet,
    palette: &Palette,
    out: &mut [Style],
) {
    cascade_with(W::SUB_TABLE, W::SEEDS, sheet, palette, out);
    #[cfg(feature = "alloc")]
    for i in 0..W::SUB_TABLE.len() {
        cascade_arenas(tree.node_mut(i), sheet, palette);
    }
}

/// Re-run the cascade over every **arena** in `tree`, and nothing else.
///
/// **Because an arena is not there when the sheet is.** A stylesheet is
/// applied once, before the first frame; a `Repeat`'s row template lives
/// in an arena that does not exist until the repeater has been laid
/// out. So every rule naming a node inside a repeated row has been dead
/// since arenas landed — silently, because the main table carries every
/// *other* node including an enum's inactive variants, and because the
/// sheet lint runs after a pump and therefore sees an arena the cascade
/// never did (M27/T27.11, found while making a `DataGrid`'s rows line up
/// with its own header).
///
/// The main table is not touched: it was cascaded when the sheet was
/// set, and re-running it every time the application changed would be
/// the per-frame cascade M16 removed.
///
/// A no-op without `alloc`, where no arena exists, and compiled to one:
/// the walk is gated with them (M52/T52.4), so it links no vtable.
#[cfg_attr(not(feature = "alloc"), allow(unused_variables))]
pub fn cascade_arenas_deep<W: crate::widget::Widget + ?Sized>(
    tree: &mut W,
    sheet: Stylesheet,
    palette: &Palette,
) {
    cascade_arenas_marked(tree, sheet, palette, crate::invalidate::Changed::ALL);
}

/// [`cascade_arenas_deep`], over only the nodes `changed` marks —
/// M53/T53.2 (`docs/plan-frame-cost.md`, R6-F3). Returns how many arenas
/// it cascaded.
///
/// **An arena needs the cascade only when it is new.** Its style row is
/// per *template* (`Arena::ensure_style`), not per row, so a repeater
/// scrolling to new rows does not need it; a widget value that was
/// *replaced* — a `Repeat` assigned, an `Option` becoming `Some`, an enum
/// switching to a variant that holds one — does, because the new arena
/// has never seen the sheet. A replacement is a write to the tree, and a
/// write the frame knows about is marked, so the marked nodes are the
/// only ones that can hold a new arena. That is the completeness
/// `Boundaries`' layout already stands on (M41).
///
/// So this walks the nodes inside each marked range and asks only them.
/// `Host::pump_with` ran the whole-table walk on every handled click
/// until T53.2 — every node asked, for a click on a box that owns no
/// arena. [`Changed::ALL`](crate::invalidate::Changed::ALL) — everything
/// marked, and every `Everything` frame — walks the whole table, as
/// before.
///
/// **Not a list of owners kept from the last full walk**, which is what
/// the plan proposed (§2.2(b)): an arena that *appears* inside a
/// narrowed range — the `Option` and enum cases — is not on a list built
/// before it existed, and `lumen-static-shell/tests/arena_marks.rs`
/// shows both. Walking the marked nodes finds it, because the write that
/// made it marked it.
///
/// Without `alloc` it walks nothing and answers 0: the loops are gated
/// with the arenas (M52/T52.4), for the reason [`cascade_arenas_deep`]
/// gives.
#[cfg_attr(not(feature = "alloc"), allow(unused_variables))]
pub fn cascade_arenas_marked<W: crate::widget::Widget + ?Sized>(
    tree: &mut W,
    sheet: Stylesheet,
    palette: &Palette,
    changed: crate::invalidate::Changed<'_>,
) -> usize {
    #[cfg_attr(not(feature = "alloc"), allow(unused_mut))]
    let mut done = 0;
    #[cfg(feature = "alloc")]
    {
        let n = W::SUB_TABLE.len();
        match changed.ranges() {
            None => {
                for i in 0..n {
                    done += cascade_arenas(tree.node_mut(i), sheet, palette);
                }
            }
            Some(ranges) => {
                for r in ranges {
                    for i in r.start..r.end.min(n) {
                        done += cascade_arenas(tree.node_mut(i), sheet, palette);
                    }
                }
            }
        }
    }
    done
}

/// Cascade every arena `node` owns, and every arena under those; how
/// many.
///
/// **Compiled only with `alloc`** (M52/T52.4), because Repeat, List and
/// Dyn — every arena — are. Its callers' loops are gated the same way,
/// and that is the part that matters: the loop hands `tree.node_mut(i)`,
/// a `&mut dyn Node`, to this function, and the coercion instantiates
/// every method of every node type in the tree. With the body compiled
/// out and the loop left in, a style-only leaf firmware linked every
/// widget's `on_event` and `hit` for nothing (R12-F4,
/// `docs/results-embedded.md`).
#[cfg(feature = "alloc")]
fn cascade_arenas(node: &mut dyn crate::widget::Node, sheet: Stylesheet, palette: &Palette) -> usize {
    let mut done = 0;
    for a in 0..node.arena_count() {
        let Some(arena) = node.arena_mut(a) else { continue };
        // Every arena carries its seeds (M26/T26.10); the
        // instance-reading fallback this used to have is deleted.
        cascade_with(arena.table, arena.seeds, sheet, palette, arena.style);
        done += 1;
    }
    // Arenas nested inside an arena's template are not reached: the
    // template is borrowed shared while its style row is borrowed
    // exclusively, so descending needs a second pass the borrow checker
    // will not give from here. A `List<Repeat<_>>` therefore styles the
    // list's rows but not the repeater's. Recorded rather than papered
    // over; the shape that fixes it is an arena walk that yields owned
    // handles, which is F4.4's territory.
    done
}
