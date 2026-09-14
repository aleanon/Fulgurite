//! The small furniture: a divider, a badge, a chip, an avatar, an icon.
//!
//! All five are leaves. Between them they cover the thing a static tree
//! is *good* at — a widget whose whole appearance is a function of one
//! value and its own box, with no children to splice and no depth to
//! carry.

use lumen_static::desc::{NodeDesc, NodeState, NO_PARENT};
use lumen_static::display::{Color, Painter};
use lumen_static::ops::{record_painted, rect_of, resolve_self};
use lumen_static::style::{Align, Edges, Length, Seed, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{leaf_common, Constraints, LayoutCx, Size, TextStorage};

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
/// application. The default is 1 logical pixel, which the renderer
/// antialiases to whatever the scale factor makes of it.
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
        &[Seed::new(Self::KIND, Self::type_defaults).reading(lumen_static::style::Fields::of(&[
            lumen_static::style::Fields::WIDTH,
            lumen_static::style::Fields::HEIGHT,
            lumen_static::style::Fields::VISIBLE,
            lumen_static::style::Fields::BACKGROUND,
            lumen_static::style::Fields::RADIUS,
        ]))];
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
        }
    }

    leaf_common!(Divider, 0, nohit);
}

/// A small pill of text: a count, a status, a tag.
///
/// The text is a presentation copy — bind it. `Badge` shrinks to fit it,
/// which is the one thing here that is not rigid.
pub struct Badge<S: TextStorage = &'static str> {
    /// What it reads. Bind it; the widget never writes it.
    pub text: S,
}

impl<S: TextStorage> Default for Badge<S> {
    fn default() -> Self {
        Badge { text: S::default() }
    }
}

impl<S: TextStorage> Badge<S> {
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
        &[Seed::new(Self::KIND, Self::type_defaults).reading(lumen_static::style::Fields::of(&[
            lumen_static::style::Fields::WIDTH,
            lumen_static::style::Fields::HEIGHT,
            lumen_static::style::Fields::PADDING,
            lumen_static::style::Fields::VISIBLE,
            lumen_static::style::Fields::COLOR,
            lumen_static::style::Fields::BACKGROUND,
            lumen_static::style::Fields::RADIUS,
            lumen_static::style::Fields::FONT_SIZE,
            lumen_static::style::Fields::FONT_WEIGHT,
        ]))];
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
        let m = cx.text.measure_weighted(
            self.text.as_ref(),
            me.font_size,
            me.font_weight,
            limits.max_w,
        );
        let content = Size { w: m.w + me.padding.horizontal(), h: m.h + me.padding.vertical() };
        let size = resolve_self(limits, &me, content);
        record_painted(st, i, limits, size, &me)
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = rect_of(s);
        if r[2] <= r[0] || r[3] <= r[1] {
            return;
        }
        out.rounded_rect(r, s.bg, s.radius.min((r[3] - r[1]) / 2.0));
        let inner = [r[0] + s.pad.0, r[1] + s.pad.1, r[2] - s.pad.2, r[3] - s.pad.3];
        out.text_weighted(inner, self.text.as_ref(), s.font_size, s.font_weight, s.fg);
    }

    leaf_common!(Badge<S>, 0, nohit);
}

/// A badge with a quieter palette and a border-ish fill: a filter tag, a
/// selected value, a removable token.
///
/// The same shape as [`Badge`] with different `defaults`, and that is
/// the whole difference — which is the argument for it being a separate
/// type rather than a field: `kind_name` feeds the cascade and the
/// agent's selectors, so `chip` and `badge` are selectable apart.
pub struct Chip<S: TextStorage = &'static str> {
    /// What it reads. Bind it.
    pub text: S,
}

impl<S: TextStorage> Default for Chip<S> {
    fn default() -> Self {
        Chip { text: S::default() }
    }
}

impl<S: TextStorage> Chip<S> {
    pub fn set(&mut self, text: S) {
        self.text = text;
    }
}

impl<S: TextStorage> Widget for Chip<S> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = LEAF;

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
            ..Style::default()
        }
    }

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults).reading(lumen_static::style::Fields::of(&[
            lumen_static::style::Fields::WIDTH,
            lumen_static::style::Fields::HEIGHT,
            lumen_static::style::Fields::PADDING,
            lumen_static::style::Fields::VISIBLE,
            lumen_static::style::Fields::COLOR,
            lumen_static::style::Fields::BACKGROUND,
            lumen_static::style::Fields::RADIUS,
            lumen_static::style::Fields::FONT_SIZE,
            lumen_static::style::Fields::FONT_WEIGHT,
        ]))];
}

impl<S: TextStorage> Node for Chip<S> {
    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = style[i];
        let m = cx.text.measure_weighted(
            self.text.as_ref(),
            me.font_size,
            me.font_weight,
            limits.max_w,
        );
        let content = Size { w: m.w + me.padding.horizontal(), h: m.h + me.padding.vertical() };
        let size = resolve_self(limits, &me, content);
        record_painted(st, i, limits, size, &me)
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = rect_of(s);
        if r[2] <= r[0] || r[3] <= r[1] {
            return;
        }
        out.rounded_rect(r, s.bg, s.radius.min((r[3] - r[1]) / 2.0));
        let inner = [r[0] + s.pad.0, r[1] + s.pad.1, r[2] - s.pad.2, r[3] - s.pad.3];
        out.text_weighted(inner, self.text.as_ref(), s.font_size, s.font_weight, s.fg);
    }

    leaf_common!(Chip<S>, 0, nohit);
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
}

impl<S: TextStorage> Default for Avatar<S> {
    fn default() -> Self {
        Avatar { initials: S::default() }
    }
}

impl<S: TextStorage> Avatar<S> {
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
            align: Align::Center,
            ..Style::default()
        }
    }

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults).reading(lumen_static::style::Fields::of(&[
            lumen_static::style::Fields::WIDTH,
            lumen_static::style::Fields::HEIGHT,
            lumen_static::style::Fields::VISIBLE,
            lumen_static::style::Fields::COLOR,
            lumen_static::style::Fields::BACKGROUND,
            lumen_static::style::Fields::FONT_SIZE,
            lumen_static::style::Fields::FONT_WEIGHT,
        ]))];
}

const AVATAR: f32 = 40.0;

impl<S: TextStorage> Node for Avatar<S> {
    fn layout(
        &mut self,
        _cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = style[i];
        let size = resolve_self(limits, &me, Size { w: AVATAR, h: AVATAR });
        record_painted(st, i, limits, size, &me)
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = rect_of(s);
        let (w, h) = (r[2] - r[0], r[3] - r[1]);
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        out.rounded_rect(r, s.bg, w.min(h) / 2.0);
        // Roughly centred: the text primitive takes a box and aligns
        // within it, so the box is the avatar inset by a quarter of its
        // height — which puts a cap-height glyph on the middle line
        // without the widget needing to measure anything.
        let inset = h * 0.26;
        out.text_weighted(
            [r[0], r[1] + inset, r[2], r[3] - inset],
            self.shown(),
            s.font_size,
            s.font_weight,
            s.fg,
        );
    }

    leaf_common!(Avatar<S>, 0, nohit);
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
        &[Seed::new(Self::KIND, Self::type_defaults).reading(lumen_static::style::Fields::of(&[
            lumen_static::style::Fields::WIDTH,
            lumen_static::style::Fields::HEIGHT,
            lumen_static::style::Fields::VISIBLE,
            lumen_static::style::Fields::COLOR,
        ]))];
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
        }
    }

    leaf_common!(Icon, 0, nohit);
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
impl<S, T: TextStorage> lumen_static::event::Handlers<S> for Chip<T> {
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
