//! The paint target — plan T5.1, widened by M10a.
//!
//! Paint stops folding a checksum and starts emitting drawing commands.
//! This is the seam a renderer consumes: widgets describe *what* to
//! draw, and the renderer decides *how*, so a renderer can be injected
//! at the shell as `&mut dyn Renderer` without appearing anywhere in a
//! widget signature (the answer recorded in
//! `docs/plan-agent-parity.md`'s appendix).
//!
//! **There are two seams here, not one.** [`Painter`] is where a widget
//! *writes*; [`Renderer`] is where a frame is *read*. Until M10a a
//! widget wrote into a concrete [`DisplayList`] and the two were
//! conflated — which meant painting required an allocator whether or not
//! anything wanted a list. Collecting into a `DisplayList` is now one
//! implementation of `Painter` among others.
//!
//! In that collecting implementation, text is stored as a range into one
//! shared buffer rather than a `String` per command, so a frame's worth
//! of text costs one growable allocation reused between frames. A
//! `Painter` that does not collect receives the bytes directly and has
//! no buffer to intern into — which is why the trait takes `&str`.

/// Straight sRGB, 8 bits per channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

impl Color {
    pub const BLACK: Color = Color(0, 0, 0, 255);
    pub const TRANSPARENT: Color = Color(0, 0, 0, 0);
}

/// `[x0, y0, x1, y1]`, absolute.
pub type Rect = [f32; 4];

/// One round of the mixing function every checksum here uses.
///
/// Written out three times before 2026-09-02 — twice in this file and
/// once in `ops.rs`, where the shared helper sat with no callers
/// (F6.1). The journal's `measure_into` uses the same constant, so a
/// container's folded signature mixes the way a leaf's does.
#[inline]
pub(crate) fn fold(h: u64, v: u64) -> u64 {
    h.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(v)
}

/// Whether a rect encloses anything a renderer could paint.
#[inline]
fn has_area(r: Rect) -> bool {
    r[2] > r[0] && r[3] > r[1]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cmd {
    Rect { rect: Rect, color: Color },
    /// `text` indexes [`DisplayList::text`].
    Text { rect: Rect, text: (u32, u32), size: f32, weight: f32, color: Color },
    /// A filled rect with rounded corners. `radius` of 0 is a plain
    /// rect; a renderer that cannot round may treat it as one.
    RoundedRect { rect: Rect, color: Color, radius: f32 },
    /// A soft shadow under the rounded rect `rect`: `color` at the edge
    /// falling to nothing `spread` px out. Decorative — a renderer that
    /// cannot draw one skips it. One command, not a stack: the first
    /// version emitted 18 translucent rounded rects per shadow and that
    /// was 97% of a frame (`docs/results-frame-cost.md`).
    Shadow { rect: Rect, color: Color, radius: f32, spread: f32 },
    PushClip { rect: Rect },
    PopClip,
}

impl Cmd {
    /// Content hash. Floats are hashed by bit pattern, which is what
    /// makes a command list comparable frame to frame — the change
    /// journal's "did this node's content change" signal.
    pub fn hash_into(&self, h: &mut u64) {
        #[inline]
        fn f(h: &mut u64, v: u64) {
            *h = fold(*h, v);
        }
        match self {
            Cmd::Rect { rect, color } => {
                f(h, 1);
                for v in rect {
                    f(h, v.to_bits() as u64);
                }
                f(h, u32::from_be_bytes([color.0, color.1, color.2, color.3]) as u64);
            }
            Cmd::RoundedRect { rect, color, radius } => {
                f(h, 5);
                for v in rect {
                    f(h, v.to_bits() as u64);
                }
                f(h, u32::from_be_bytes([color.0, color.1, color.2, color.3]) as u64);
                f(h, radius.to_bits() as u64);
            }
            Cmd::Shadow { rect, color, radius, spread } => {
                f(h, 6);
                for v in rect {
                    f(h, v.to_bits() as u64);
                }
                f(h, u32::from_be_bytes([color.0, color.1, color.2, color.3]) as u64);
                f(h, radius.to_bits() as u64);
                f(h, spread.to_bits() as u64);
            }
            Cmd::Text { rect, text, size, weight, color } => {
                f(h, 2);
                for v in rect {
                    f(h, v.to_bits() as u64);
                }
                f(h, ((text.0 as u64) << 32) | text.1 as u64);
                f(h, size.to_bits() as u64);
                f(h, weight.to_bits() as u64);
                f(h, u32::from_be_bytes([color.0, color.1, color.2, color.3]) as u64);
            }
            Cmd::PushClip { rect } => {
                f(h, 3);
                for v in rect {
                    f(h, v.to_bits() as u64);
                }
            }
            Cmd::PopClip => f(h, 4),
        }
    }
}

/// Where a widget's `paint` puts what it draws — M10a.
///
/// `Node::paint` writes through this rather than into a concrete
/// [`DisplayList`], so the **caller owns the storage**. That is the
/// shape this crate has now chosen four times, for the same reason each
/// time: [`crate::TextMeasure`] (T5.2), [`crate::GlyphSink`] (T7.2b),
/// [`Renderer`], and this. A desktop host collects into a
/// `DisplayList`; a host with no allocator implements this directly on
/// a rasteriser and builds no list at all.
///
/// The alternative considered was a fixed-capacity
/// `DisplayList<const N: usize>`, whose const would have been viral —
/// through every `paint` signature and the derive's generated one. This
/// is one parameter type across sixteen call sites
/// (`docs/plan-architecture-v2.md` M10a).
///
/// **Text arrives as `&str`, not as a range.** `Cmd::Text` carries a
/// `(offset, len)` into [`DisplayList::text`], which only a collector
/// can assign; an immediate-mode implementation has nowhere to intern
/// it. So the trait takes the bytes and `DisplayList`'s impl does the
/// interning it already did.
pub trait Painter {
    fn rect(&mut self, rect: Rect, color: Color);
    fn text_weighted(&mut self, rect: Rect, s: &str, size: f32, weight: f32, color: Color);
    fn rounded_rect(&mut self, rect: Rect, color: Color, radius: f32);
    fn shadow(&mut self, rect: Rect, radius: f32, spread: f32);
    fn push_clip(&mut self, rect: Rect);
    fn pop_clip(&mut self);

    /// A run at the default weight. Provided, so an implementation only
    /// has to write the weighted form.
    fn text(&mut self, rect: Rect, s: &str, size: f32, color: Color) {
        self.text_weighted(rect, s, size, 400.0, color);
    }
}

/// One frame's drawing commands, in paint order.
#[derive(Default, Debug)]
pub struct DisplayList {
    pub cmds: Vec<Cmd>,
    /// Text bytes for this frame; `Cmd::Text` holds ranges into it.
    pub text: String,
}

impl DisplayList {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reuse the buffers between frames — the allocations are the point
    /// of holding a list rather than rebuilding one.
    pub fn clear(&mut self) {
        self.cmds.clear();
        self.text.clear();
    }

    /// Record a filled rect. A rect with no area is dropped: it cannot
    /// draw, and recording it would put phantom content in front of the
    /// journal's per-node signature and the audit's "is anything
    /// visible" check. This is also what makes `Style::visible = false`
    /// reach the screen as absence rather than as a zero-sized command
    /// (`docs/plan-review-fixes.md` F1.6).
    pub fn rect(&mut self, rect: Rect, color: Color) {
        if has_area(rect) {
            self.cmds.push(Cmd::Rect { rect, color });
        }
    }

    /// Record a text run. Dropped when the box has no area, for the same
    /// reason as [`DisplayList::rect`].
    pub fn text(&mut self, rect: Rect, s: &str, size: f32, color: Color) {
        self.text_weighted(rect, s, size, 400.0, color);
    }

    /// Record a text run at a given weight (400 regular, 700 bold).
    pub fn text_weighted(&mut self, rect: Rect, s: &str, size: f32, weight: f32, color: Color) {
        if !has_area(rect) {
            return;
        }
        let start = self.text.len() as u32;
        self.text.push_str(s);
        let range = (start, self.text.len() as u32 - start);
        self.cmds.push(Cmd::Text { rect, text: range, size, weight, color });
    }

    /// Record a filled rect with rounded corners.
    pub fn rounded_rect(&mut self, rect: Rect, color: Color, radius: f32) {
        if has_area(rect) && color.3 != 0 {
            self.cmds.push(Cmd::RoundedRect { rect, color, radius });
        }
    }

    /// A soft drop shadow: `spread` px of translucent rounded rects
    /// stepping outward, darkest nearest the box. Cheap, and reads as a
    /// blur at the sizes a card uses; a renderer with a real blur may
    /// collapse these later.
    pub fn shadow(&mut self, rect: Rect, radius: f32, spread: f32) {
        if !has_area(rect) || spread <= 0.0 {
            return;
        }
        // Dropped a little, like light from above; ~30% black at the
        // edge. The renderer owns the falloff curve.
        let r = [rect[0], rect[1] + spread * 0.35, rect[2], rect[3] + spread * 0.35];
        self.cmds.push(Cmd::Shadow { rect: r, color: Color(0, 0, 0, 80), radius, spread });
    }

    pub fn push_clip(&mut self, rect: Rect) {
        self.cmds.push(Cmd::PushClip { rect });
    }

    pub fn pop_clip(&mut self) {
        self.cmds.push(Cmd::PopClip);
    }

    // NOTE: the bodies stay here, on the inherent methods, and
    // `impl Painter for DisplayList` (below) forwards to them. That
    // keeps `DisplayList`'s own API unchanged, so `hash_from` and
    // `str_of` still work on a concrete list and the journal keeps
    // signing per-node command ranges exactly as before.

    /// The string a `Cmd::Text` refers to.
    pub fn str_of(&self, range: (u32, u32)) -> &str {
        &self.text[range.0 as usize..(range.0 + range.1) as usize]
    }

    /// Hash of `cmds[from..]` — how the journal takes a node's content
    /// signature without building a list per node.
    pub fn hash_from(&self, from: usize) -> u64 {
        let mut h = 0u64;
        for c in &self.cmds[from.min(self.cmds.len())..] {
            c.hash_into(&mut h);
            if let Cmd::Text { text, .. } = c {
                for b in self.str_of(*text).as_bytes() {
                    h = fold(h, *b as u64);
                }
            }
        }
        h
    }

    pub fn len(&self) -> usize {
        self.cmds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cmds.is_empty()
    }
}

/// What a renderer must do with a list. Implemented at the shell — the
/// renderer never appears in a widget signature.
pub trait Renderer {
    fn render(&mut self, list: &DisplayList, width: u32, height: u32);
}

/// Collecting into a list is *one* implementation of [`Painter`], not
/// the definition of painting.
impl Painter for DisplayList {
    fn rect(&mut self, rect: Rect, color: Color) {
        DisplayList::rect(self, rect, color)
    }
    fn text_weighted(&mut self, rect: Rect, s: &str, size: f32, weight: f32, color: Color) {
        DisplayList::text_weighted(self, rect, s, size, weight, color)
    }
    fn rounded_rect(&mut self, rect: Rect, color: Color, radius: f32) {
        DisplayList::rounded_rect(self, rect, color, radius)
    }
    fn shadow(&mut self, rect: Rect, radius: f32, spread: f32) {
        DisplayList::shadow(self, rect, radius, spread)
    }
    fn push_clip(&mut self, rect: Rect) {
        DisplayList::push_clip(self, rect)
    }
    fn pop_clip(&mut self) {
        DisplayList::pop_clip(self)
    }
}
