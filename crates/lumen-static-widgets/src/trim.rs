//! The small furniture: a divider, a badge, a chip, an avatar, an icon.
//!
//! All five are leaves. Between them they cover the thing a static tree
//! is *good* at — a widget whose whole appearance is a function of one
//! value and its own box, with no children to splice and no depth to
//! carry.

use lumen_static::desc::{NodeDesc, NodeState, NO_PARENT};
use lumen_static::display::{Color, Painter};
use lumen_static::ops::{record_painted, rect_of, resolve_self};
use lumen_static::style::{Edges, Length, Seed, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, Event, LayoutCx, Point, Size, TextStorage};

/// One node, no children, not focusable — the shape every widget in this
/// file has.
const LEAF: &[NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];

/// A hairline divider.
///
/// **Named `Divider`, where Lumen calls it `Rule`.** `lumen_static::Rule`
/// is already a *stylesheet* rule and is in the prelude, so a widget of
/// that name would collide in every file that writes `use
/// lumen_static::prelude::*` — which is every file. A widget set is
/// allowed to differ from its model where the model's name is taken.
///
/// **Its thickness is `Style::height`, not a field**, so a stylesheet
/// sets it and a theme can set it once for every rule in an
/// application. The default is 1 logical pixel, which lands on exactly
/// `scale` device rows: its edges are snapped to the pixel grid where
/// logical becomes physical (M57/A15). This said the renderer antialiased
/// it, and a divider centred on a half pixel drew two full rows
/// (`docs/widget-audit.md`, Divider).
#[derive(Default)]
pub struct Divider;

impl Widget for Divider {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    /// Its size is its style's. Nothing about a divider comes from
    /// content.
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = LEAF;

    const KIND: &'static str = "rule";

    fn type_defaults() -> Style {
        Style {
            width: Length::FILL,
            height: Length::Fixed(1.0),
            background: Color(0xd8, 0xdd, 0xe6, 0xff),
            ..Style::default()
        }
    }

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, lumen_static::style::Fields::of(&[
            lumen_static::style::Fields::WIDTH,
            lumen_static::style::Fields::HEIGHT,
            lumen_static::style::Fields::VISIBLE,
            lumen_static::style::Fields::BACKGROUND,
            lumen_static::style::Fields::BORDER,
            lumen_static::style::Fields::RADIUS,
        ])).with_roles(lumen_static::style::SeedRoles::NONE.background(lumen_static::style::Role::Track))];
}

impl Node for Divider {
    fn layout(
        &mut self,
        _cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = style[i];
        let size = resolve_self(limits, &me, Size { w: 0.0, h: 1.0 });
        record_painted(st, i, limits, size, &me)
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = rect_of(s);
        if r[2] > r[0] && r[3] > r[1] {
            out.rounded_rect(r, s.bg, s.radius);
            lumen_static::ops::border(st, i, out);
        }
    }

    lumen_static::leaf_common!(Divider, 0, nohit);
}

/// A small pill of text: a count, a status, a tag.
///
/// The text is a presentation copy — bind it. `Badge` shrinks to fit it,
/// which is the one thing here that is not rigid.
pub struct Badge<S: TextStorage = &'static str> {
    /// What it reads. Bind it; the widget never writes it.
    pub text: S,
    /// The text's measured box, keyed on a fold of its bytes.
    ///
    /// **It had none** until M26/T26.8: this widget shaped on every
    /// layout, which `--bin bindings` measured at ~410 ns against a
    /// 4.97 ns memo hit — a still frame paying a full shape for text
    /// nobody touched. `text` is a plain public field the application
    /// binds, so the fold stands in for the dirty bit a `Label` has
    /// (`lumen_static::text::FoldedText`).
    measured: lumen_static::text::FoldedText,
}

impl<S: TextStorage> Default for Badge<S> {
    fn default() -> Self {
        Badge { text: S::default(), measured: lumen_static::text::FoldedText::new() }
    }
}

impl<S: TextStorage> Badge<S> {
    /// **A constructor, because the memo is private** (M26/T26.8).
    /// `Badge { text }` was the spelling until this widget had a memo
    /// to keep; `Label` has had `new` since it had one.
    pub fn new(text: S) -> Self {
        Badge { text, measured: lumen_static::text::FoldedText::new() }
    }

    /// Replace the text. Named `set` like `Label`'s, so the two are
    /// interchangeable at a call site.
    pub fn set(&mut self, text: S) {
        self.text = text;
    }
}

impl<S: TextStorage> Widget for Badge<S> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    /// **False**, and that is the interesting one. A badge's width comes
    /// from its text, so a text change is a relayout boundary — saying
    /// `true` here would let a changed count keep the old box and clip.
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = LEAF;

    const KIND: &'static str = "badge";

    fn type_defaults() -> Style {
        Style {
            width: Length::Shrink,
            height: Length::Shrink,
            // **Red and round, which is what a badge is.** Lumen's is
            // a notification count sitting on a label, and every system
            // draws that in an alert colour — a blue pill reads as a
            // tag, which is what `Chip` is for. The radius is clamped to
            // half the height in `paint`, so a one- or two-character
            // count comes out circular and a longer one a pill.
            radius: 999.0,
            padding: Edges { left: 7.0, right: 7.0, top: 2.0, bottom: 2.0 },
            background: Color(0xd9, 0x30, 0x25, 0xff),
            color: Color(0xff, 0xff, 0xff, 0xff),
            font_size: 12.0,
            font_weight: 700.0,
            ..Style::default()
        }
    }

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, lumen_static::style::Fields::of(&[
            lumen_static::style::Fields::WIDTH,
            lumen_static::style::Fields::HEIGHT,
            lumen_static::style::Fields::PADDING,
            lumen_static::style::Fields::VISIBLE,
            lumen_static::style::Fields::COLOR,
            lumen_static::style::Fields::BACKGROUND,
            lumen_static::style::Fields::BORDER,
            lumen_static::style::Fields::RADIUS,
            lumen_static::style::Fields::FONT_SIZE,
            lumen_static::style::Fields::FONT_WEIGHT,
        ])).with_roles(lumen_static::style::SeedRoles::NONE.color(lumen_static::style::Role::OnDanger).background(lumen_static::style::Role::Danger))];
}

impl<S: TextStorage> Node for Badge<S> {
    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = style[i];
        let text = self.text.as_ref();
        let key = lumen_static::text::MeasureKey::of(limits, &me, cx.scale());
        let m = self.measured.get_or(key, text, || {
            cx.text.measure_weighted(text, me.font_size, me.font_weight, limits.max_w)
        });
        let mut content = Size { w: m.w + me.padding.horizontal(), h: m.h + me.padding.vertical() };
        // **Never narrower than tall** — M57/B1. The rustdoc promises a
        // one- or two-character count comes out circular, and nothing made
        // it so: "3" was round only because the digit is narrow, and "12"
        // was a pill (`docs/widget-audit.md`, Badge).
        if content.w < content.h {
            content.w = content.h;
        }
        let size = resolve_self(limits, &me, content);
        record_painted(st, i, limits, size, &me)
    }

    /// What a screen reader and the agent are told — M57/A17
    /// (`docs/plan-widget-fixes.md`). This published nothing at all
    /// (`GenericContainer`, no value) until M57.
    #[cfg(feature = "observe")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut alloc::vec::Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        use lumen_static::observe::accesskit as ak;
        node.set_role(ak::Role::Label);
        node.set_value(self.text.as_ref());
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = rect_of(s);
        if r[2] <= r[0] || r[3] <= r[1] {
            return;
        }
        out.rounded_rect(r, s.bg, s.radius.min((r[3] - r[1]) / 2.0));
        lumen_static::ops::border(st, i, out);
        // Centred on the measured text, not drawn from the inner left —
        // which is only centred when nothing widened the box (B1 does).
        let m = self.measured.size();
        let x = r[0] + ((r[2] - r[0]) - m.w).max(0.0) / 2.0;
        out.text_weighted([x, r[1] + s.pad.1, x + m.w, r[3] - s.pad.3], self.text.as_ref(), s.font_size, s.font_weight, s.fg);
    }

    lumen_static::leaf_common!(Badge<S>, 0, nohit);
}

/// A compact control: a filter tag that can be selected, a token that
/// can be removed.
///
/// **It is a control, where [`Badge`] is a label** — M57/B2. This rustdoc
/// advertised "a selected value, a removable token" while the widget was
/// `nohit` with no handlers, so neither could happen
/// (`docs/widget-audit.md`, Chip). Now:
///
/// - `selected` is **controlled**, following `Toggle`'s precedent: the
///   application owns the flag, `on_click` flips it, and a binding writes
///   the copy back. A selected chip is tinted and outlined in its text
///   colour.
/// - `removable` adds a trailing × of [`Chip::REMOVE_W`] px. A click
///   that begins and ends on it runs `on_remove` instead of `on_click`.
/// - Both handlers are optional, and a chip with neither lets a click
///   through to whatever holds it. It is focusable either way — a static
///   tag with no behaviour is a [`Badge`].
///
/// Enter and Space activate `on_click`, as on a button; the keyboard has
/// no remove, because activation is the pass's and it runs one handler.
pub struct Chip<T: TextStorage = &'static str, S: 'static = ()> {
    /// What it reads. Bind it.
    pub text: T,
    /// Whether it reads as selected. A presentation copy — bind it.
    pub selected: bool,
    /// Whether it carries a trailing ×.
    pub removable: bool,
    /// Run on a click, Enter or Space.
    pub on_click: Option<fn(&mut S, &lumen_static::EventCtx)>,
    /// Run on a click on the ×.
    pub on_remove: Option<fn(&mut S, &lumen_static::EventCtx)>,
    /// Whether the gesture in flight is on the ×: set by the press and
    /// the release, read by [`Handlers::handler`](lumen_static::event::Handlers),
    /// which pass 2 asks after pass 1 has run.
    on_x: bool,
    /// Its own width, for which part a press is on.
    width: f32,
    /// The text's measured box, keyed on a fold of its bytes.
    ///
    /// **It had none** until M26/T26.8: this widget shaped on every
    /// layout, which `--bin bindings` measured at ~410 ns against a
    /// 4.97 ns memo hit — a still frame paying a full shape for text
    /// nobody touched. `text` is a plain public field the application
    /// binds, so the fold stands in for the dirty bit a `Label` has
    /// (`lumen_static::text::FoldedText`).
    measured: lumen_static::text::FoldedText,
}

impl<T: TextStorage, S: 'static> Default for Chip<T, S> {
    fn default() -> Self {
        Chip::new(T::default())
    }
}

impl<T: TextStorage, S: 'static> Chip<T, S> {
    /// The ×'s width, inside the right padding.
    pub const REMOVE_W: f32 = 18.0;

    /// **A constructor, because the memo is private** (M26/T26.8).
    /// `Chip { text }` was the spelling until this widget had a memo
    /// to keep; `Label` has had `new` since it had one.
    pub fn new(text: T) -> Self {
        Chip {
            text,
            selected: false,
            removable: false,
            on_click: None,
            on_remove: None,
            on_x: false,
            width: 0.0,
            measured: lumen_static::text::FoldedText::new(),
        }
    }

    pub fn set(&mut self, text: T) {
        self.text = text;
    }

    fn extra(&self) -> f32 {
        if self.removable { Self::REMOVE_W } else { 0.0 }
    }
}

impl<T: TextStorage, S: 'static> Widget for Chip<T, S> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::FOCUSABLE)];

    const KIND: &'static str = "chip";

    fn type_defaults() -> Style {
        Style {
            width: Length::Shrink,
            height: Length::Shrink,
            radius: 14.0,
            padding: Edges { left: 12.0, right: 12.0, top: 5.0, bottom: 5.0 },
            background: Color(0xe8, 0xed, 0xf7, 0xff),
            color: Color(0x30, 0x3a, 0x4c, 0xff),
            font_size: 13.0,
            font_weight: 500.0,
            // A control: it answers the pointer (M57/B2).
            pressed_dim: 0.18,
            hover_dim: 0.08,
            cursor: lumen_static::style::Cursor::Pointer,
            ..Style::default()
        }
    }

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, lumen_static::style::Fields::of(&[
            lumen_static::style::Fields::WIDTH,
            lumen_static::style::Fields::HEIGHT,
            lumen_static::style::Fields::PADDING,
            lumen_static::style::Fields::VISIBLE,
            lumen_static::style::Fields::CURSOR,
            lumen_static::style::Fields::FOCUS_RING,
            lumen_static::style::Fields::PRESSED_DIM,
            lumen_static::style::Fields::HOVER_DIM,
            lumen_static::style::Fields::COLOR,
            lumen_static::style::Fields::BACKGROUND,
            lumen_static::style::Fields::BORDER,
            lumen_static::style::Fields::RADIUS,
            lumen_static::style::Fields::FONT_SIZE,
            lumen_static::style::Fields::FONT_WEIGHT,
        ]))// **`Track`, not `Surface`** (M57/B2): a chip sits *on* a surface —
        // a card, a panel — so a surface body is invisible exactly where
        // chips are used; the showcase's stage showed three bare words.
        .with_roles(lumen_static::style::SeedRoles::NONE.color(lumen_static::style::Role::Text).background(lumen_static::style::Role::Track))];
}

impl<T: TextStorage, S: 'static> Node for Chip<T, S> {
    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = style[i];
        let text = self.text.as_ref();
        let key = lumen_static::text::MeasureKey::of(limits, &me, cx.scale());
        let m = self.measured.get_or(key, text, || {
            cx.text.measure_weighted(text, me.font_size, me.font_weight, limits.max_w)
        });
        let content = Size { w: m.w + me.padding.horizontal() + self.extra(), h: m.h + me.padding.vertical() };
        let size = resolve_self(limits, &me, content);
        self.width = size.w;
        record_painted(st, i, limits, size, &me)
    }

    /// What a screen reader and the agent are told — M57/A17
    /// (`docs/plan-widget-fixes.md`). This published nothing at all
    /// (`GenericContainer`, no value) until M57.
    #[cfg(feature = "observe")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut alloc::vec::Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        use lumen_static::observe::accesskit as ak;
        node.set_role(ak::Role::Label);
        node.set_value(self.text.as_ref());
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = rect_of(s);
        if r[2] <= r[0] || r[3] <= r[1] {
            return;
        }
        let radius = s.radius.min((r[3] - r[1]) / 2.0);
        out.rounded_rect(r, s.bg, radius);
        lumen_static::ops::border(st, i, out);
        // Selected: a tint and an outline in the text colour, so the
        // state reads on any ground without a new palette role.
        if self.selected {
            let c = s.fg;
            out.rounded_rect(r, Color(c.0, c.1, c.2, 0x24), radius);
            out.stroked_rect(r, c, radius, 1.5);
        }
        // The text centred on the measure, in the space left of the ×.
        let m = self.measured.size();
        let room = (r[2] - r[0]) - self.extra();
        let x = r[0] + (room - m.w).max(0.0) / 2.0;
        let y = r[1] + ((r[3] - r[1]) - m.h).max(0.0) / 2.0;
        out.text_weighted([x, y, x + m.w, y + m.h], self.text.as_ref(), s.font_size, s.font_weight, s.fg);
        if self.removable {
            // Two strokes, 8 px, centred in the × column.
            let cx = r[2] - s.pad.2 - Self::REMOVE_W / 2.0 + 2.0;
            let cy = (r[1] + r[3]) / 2.0;
            let d = 4.0;
            out.line(Point { x: cx - d, y: cy - d }, Point { x: cx + d, y: cy + d }, 1.5, s.fg);
            out.line(Point { x: cx - d, y: cy + d }, Point { x: cx + d, y: cy - d }, 1.5, s.fg);
        }
    }

    fn on_event(&mut self, ev: &Event, cx: &lumen_static::EventCtx) -> bool {
        // Which part the gesture is on, for the handler pass 2 asks for.
        // A press and its release must both be on the × — a press that
        // slides off it and lets go on the body is a click, not a remove.
        let over = self.removable && cx.local.x >= self.width - self.extra() - 8.0;
        match ev {
            Event::PointerDown { .. } => self.on_x = over,
            Event::PointerUp { .. } => self.on_x &= over,
            Event::Key { .. } => self.on_x = false,
            _ => {}
        }
        false
    }

    lumen_static::leaf_common!(Chip<T, S>, NodeDesc::FOCUSABLE);
}

/// A round token carrying initials.
///
/// **Initials, not an image**, and deliberately: an avatar that took an
/// [`lumen_static::ImageId`] would be an `Image` with a radius, which
/// the core already has. What this adds is the *fallback* every avatar
/// needs anyway — the two letters shown while the picture is missing,
/// loading, or was never set.
pub struct Avatar<S: TextStorage = &'static str> {
    /// Up to two characters. Longer is trimmed rather than refused: an
    /// avatar is decoration, and a panic here would take a frame down
    /// over a name.
    pub initials: S,
    /// The shown initials' measured box, so `paint` can centre them —
    /// M57/B3. Keyed on a fold of the bytes, as `Badge`'s is.
    measured: lumen_static::text::FoldedText,
}

impl<S: TextStorage> Default for Avatar<S> {
    fn default() -> Self {
        Avatar { initials: S::default(), measured: lumen_static::text::FoldedText::new() }
    }
}

impl<S: TextStorage> Avatar<S> {
    /// A constructor, because the memo is private (M57/B3) — the same
    /// reason `Badge::new` exists.
    pub fn new(initials: S) -> Self {
        Avatar { initials, measured: lumen_static::text::FoldedText::new() }
    }

    pub fn set(&mut self, initials: S) {
        self.initials = initials;
    }

    /// The first two `char`s — **not bytes**, or a two-letter initial
    /// from any alphabet with multi-byte letters would slice a character
    /// in half and panic on the way to a `&str`.
    fn shown(&self) -> &str {
        let s = self.initials.as_ref();
        match s.char_indices().nth(2) {
            Some((at, _)) => &s[..at],
            None => s,
        }
    }
}

impl<S: TextStorage> Widget for Avatar<S> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    /// Rigid: an avatar is a fixed circle whatever is in it.
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = LEAF;

    const KIND: &'static str = "avatar";

    fn type_defaults() -> Style {
        Style {
            width: Length::Fixed(AVATAR),
            height: Length::Fixed(AVATAR),
            radius: AVATAR / 2.0,
            background: Color(0x5b, 0x6b, 0x8c, 0xff),
            color: Color(0xff, 0xff, 0xff, 0xff),
            font_size: 15.0,
            font_weight: 700.0,
            ..Style::default()
        }
    }

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, lumen_static::style::Fields::of(&[
            lumen_static::style::Fields::WIDTH,
            lumen_static::style::Fields::HEIGHT,
            lumen_static::style::Fields::VISIBLE,
            lumen_static::style::Fields::COLOR,
            lumen_static::style::Fields::BACKGROUND,
            lumen_static::style::Fields::BORDER,
            lumen_static::style::Fields::FONT_SIZE,
            lumen_static::style::Fields::FONT_WEIGHT,
        ])).with_roles(lumen_static::style::SeedRoles::NONE.color(lumen_static::style::Role::Page).background(lumen_static::style::Role::Muted))];
}

const AVATAR: f32 = 40.0;

impl<S: TextStorage> Node for Avatar<S> {
    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = style[i];
        let size = resolve_self(limits, &me, Size { w: AVATAR, h: AVATAR });
        // Measured so paint can centre them (M57/B3).
        let key = lumen_static::text::MeasureKey::of(limits, &me, cx.scale());
        let s = self.initials.as_ref();
        let shown = match s.char_indices().nth(2) {
            Some((at, _)) => &s[..at],
            None => s,
        };
        self.measured.get_or(key, shown, || cx.text.measure_weighted(shown, me.font_size, me.font_weight, f32::INFINITY));
        record_painted(st, i, limits, size, &me)
    }

    /// What a screen reader and the agent are told — M57/A17
    /// (`docs/plan-widget-fixes.md`). This published nothing at all
    /// (`GenericContainer`, no value) until M57.
    #[cfg(feature = "observe")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut alloc::vec::Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        use lumen_static::observe::accesskit as ak;
        node.set_role(ak::Role::Image);
        node.set_value(self.shown());
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = rect_of(s);
        let (w, h) = (r[2] - r[0], r[3] - r[1]);
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        out.rounded_rect(r, s.bg, w.min(h) / 2.0);
        lumen_static::ops::border(st, i, out);
        // **Centred on the measured initials** — M57/B3. This passed the
        // whole avatar to the text primitive on the belief that it aligns
        // within its box; it draws from the box's left edge, so the
        // initials sat 9.5–14 px left of centre and a "J" poked out of the
        // circle (`docs/widget-audit.md`, bug 2). Clipped to the avatar,
        // so a wide pair cannot spill.
        let m = self.measured.size();
        let x = r[0] + (w - m.w) / 2.0;
        let y = r[1] + (h - m.h) / 2.0;
        out.push_clip(r);
        out.text_weighted([x, y, x + m.w, y + m.h], self.shown(), s.font_size, s.font_weight, s.fg);
        out.pop_clip();
    }

    lumen_static::leaf_common!(Avatar<S>, 0, nohit);
}

/// Which mark an [`Icon`] draws.
///
/// **A closed enum, not a font codepoint**, and that is the decision
/// worth recording. A glyph-based icon set needs the icon font present
/// and correct at every scale factor on every platform, and the failure
/// mode when it is not is the missing-glyph box — which is exactly how
/// `examples/glasswall` lost twenty-two of its twenty-four icons. These
/// are drawn from primitives the renderer already has, so they cannot go
/// missing and they scale with the box.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Symbol {
    /// A right-pointing triangle.
    #[default]
    Play,
    /// Two vertical bars.
    Pause,
    /// A filled square.
    Stop,
    /// A tick.
    Check,
    /// A cross.
    Close,
    /// A chevron, pointing down.
    Down,
    /// A plus.
    Plus,
    /// A minus.
    Minus,
    /// A magnifier: a ring and a handle — M27/T27.5, for `SearchField`.
    ///
    /// Drawn from primitives like every other symbol here. A glyph would
    /// need the icon font present and correct at every scale on every
    /// platform, and the failure mode when it is not is the
    /// missing-glyph box — which is how `examples/glasswall` lost
    /// twenty-two of its twenty-four icons.
    Search,
}

/// A small mark drawn from primitives — see [`Symbol`].
#[derive(Default)]
pub struct Icon {
    pub symbol: Symbol,
}

impl Icon {
    pub fn new(symbol: Symbol) -> Self {
        Icon { symbol }
    }
}

impl Widget for Icon {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = LEAF;

    const KIND: &'static str = "icon";

    fn type_defaults() -> Style {
        Style {
            width: Length::Fixed(ICON),
            height: Length::Fixed(ICON),
            color: Color(0x30, 0x3a, 0x4c, 0xff),
            ..Style::default()
        }
    }

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, lumen_static::style::Fields::of(&[
            lumen_static::style::Fields::WIDTH,
            lumen_static::style::Fields::HEIGHT,
            lumen_static::style::Fields::VISIBLE,
            lumen_static::style::Fields::COLOR,
        ])).with_roles(lumen_static::style::SeedRoles::NONE.color(lumen_static::style::Role::Text))];
}

const ICON: f32 = 20.0;

impl Node for Icon {
    fn layout(
        &mut self,
        _cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = style[i];
        let size = resolve_self(limits, &me, Size { w: ICON, h: ICON });
        record_painted(st, i, limits, size, &me)
    }

    /// What a screen reader and the agent are told — M57/A17
    /// (`docs/plan-widget-fixes.md`). This published nothing at all
    /// (`GenericContainer`, no value) until M57.
    #[cfg(feature = "observe")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut alloc::vec::Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        use lumen_static::observe::accesskit as ak;
        node.set_role(ak::Role::Image);
        node.set_value(alloc::format!("{:?}", self.symbol));
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        use lumen_static::Point;
        let s = &st[i];
        let r = rect_of(s);
        let (w, h) = (r[2] - r[0], r[3] - r[1]);
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        // Everything is expressed in the box's own units, so an icon is
        // the same mark at 12 px and at 96.
        let d = w.min(h);
        let (cx, cy) = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
        let k = d * 0.30;
        let stroke = (d * 0.12).max(1.0);
        let c = s.fg;
        let at = |x: f32, y: f32| Point { x: cx + x, y: cy + y };
        match self.symbol {
            // A triangle from three horizontal runs: the renderer has no
            // polygon, and a run per row at this size is under twenty
            // commands. `Cmd::Line` with a width is the primitive that
            // exists, so the fill is lines.
            Symbol::Play => {
                let steps = (k * 2.0).max(2.0) as i32;
                for n in 0..steps {
                    let t = n as f32 / (steps - 1).max(1) as f32;
                    let half = k * (1.0 - t);
                    let x = cx - k * 0.7 + t * k * 1.4;
                    out.line(
                        lumen_static::Point { x, y: cy - half },
                        lumen_static::Point { x, y: cy + half },
                        (k * 1.4 / steps as f32).max(1.0),
                        c,
                    );
                }
            }
            Symbol::Pause => {
                let bw = k * 0.42;
                out.rect([cx - k * 0.62, cy - k, cx - k * 0.62 + bw, cy + k], c);
                out.rect([cx + k * 0.20, cy - k, cx + k * 0.20 + bw, cy + k], c);
            }
            Symbol::Stop => out.rect([cx - k, cy - k, cx + k, cy + k], c),
            Symbol::Check => {
                out.line(at(-k, 0.0), at(-k * 0.25, k * 0.7), stroke, c);
                out.line(at(-k * 0.25, k * 0.7), at(k, -k * 0.75), stroke, c);
            }
            Symbol::Close => {
                out.line(at(-k, -k), at(k, k), stroke, c);
                out.line(at(-k, k), at(k, -k), stroke, c);
            }
            Symbol::Down => {
                out.line(at(-k, -k * 0.4), at(0.0, k * 0.45), stroke, c);
                out.line(at(0.0, k * 0.45), at(k, -k * 0.4), stroke, c);
            }
            Symbol::Plus => {
                out.line(at(-k, 0.0), at(k, 0.0), stroke, c);
                out.line(at(0.0, -k), at(0.0, k), stroke, c);
            }
            Symbol::Minus => out.line(at(-k, 0.0), at(k, 0.0), stroke, c),
            Symbol::Search => {
                // A hollow ring plus a handle. Chords off
                // `chart::unit`'s quarter-turn table, for the reason that
                // table exists: **no trig in a `no_std` crate that has
                // needed none**, and `Spinner`'s ring is drawn the same
                // way. `Cmd` has no stroked ellipse either — only the
                // inset stroked *rect* M27/T27.1 added, which cannot
                // round to a circle this small without reading as a blob.
                //
                // The ring is offset up and left so the handle has room
                // inside the icon's own box rather than overhanging it.
                let (ox, oy) = (-k * 0.18, -k * 0.18);
                let r = k * 0.58;
                let n = 16;
                let ring = |seg: usize| {
                    let u = crate::chart::unit(seg * (crate::chart::STEPS_ROUND / n));
                    at(ox + u.0 * r, oy + u.1 * r)
                };
                for seg in 0..n {
                    out.line(ring(seg), ring((seg + 1) % n), stroke, c);
                }
                // The handle, from the ring's lower-right corner out to
                // the icon's own.
                let d = core::f32::consts::FRAC_1_SQRT_2;
                out.line(at(ox + r * d, oy + r * d), at(k * 0.92, k * 0.92), stroke, c);
            }
        }
    }

    lumen_static::leaf_common!(Icon, 0, nohit);
}

/// **No handlers.** Every widget in a tree must answer `Handlers`, and a
/// widget that carries no `on_*` field answers `None` — dispatch then
/// walks past it. This is one impl per type rather than a blanket one
/// because a blanket `impl<W, S> Handlers<S> for W` would collide with
/// every widget that *does* have handlers.
impl<S> lumen_static::event::Handlers<S> for Divider {
    fn handler(
        &self,
        _sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &lumen_static::EventCtx)> {
        None
    }
}

/// **No handlers.** Every widget in a tree must answer `Handlers`, and a
/// widget that carries no `on_*` field answers `None` — dispatch then
/// walks past it. This is one impl per type rather than a blanket one
/// because a blanket `impl<W, S> Handlers<S> for W` would collide with
/// every widget that *does* have handlers.
impl<S, T: TextStorage> lumen_static::event::Handlers<S> for Badge<T> {
    fn handler(
        &self,
        _sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &lumen_static::EventCtx)> {
        None
    }
}

/// **No handlers.** Every widget in a tree must answer `Handlers`, and a
/// widget that carries no `on_*` field answers `None` — dispatch then
/// walks past it. This is one impl per type rather than a blanket one
/// because a blanket `impl<W, S> Handlers<S> for W` would collide with
/// every widget that *does* have handlers.
impl<S: 'static, T: TextStorage> lumen_static::event::Handlers<S> for Chip<T, S> {
    fn handler(
        &self,
        _sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &lumen_static::EventCtx)> {
        if self.on_x { self.on_remove } else { self.on_click }
    }
}

/// **No handlers.** Every widget in a tree must answer `Handlers`, and a
/// widget that carries no `on_*` field answers `None` — dispatch then
/// walks past it. This is one impl per type rather than a blanket one
/// because a blanket `impl<W, S> Handlers<S> for W` would collide with
/// every widget that *does* have handlers.
impl<S, T: TextStorage> lumen_static::event::Handlers<S> for Avatar<T> {
    fn handler(
        &self,
        _sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &lumen_static::EventCtx)> {
        None
    }
}

/// **No handlers.** Every widget in a tree must answer `Handlers`, and a
/// widget that carries no `on_*` field answers `None` — dispatch then
/// walks past it. This is one impl per type rather than a blanket one
/// because a blanket `impl<W, S> Handlers<S> for W` would collide with
/// every widget that *does* have handlers.
impl<S> lumen_static::event::Handlers<S> for Icon {
    fn handler(
        &self,
        _sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &lumen_static::EventCtx)> {
        None
    }
}
