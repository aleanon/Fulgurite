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
// `alloc`, not the std prelude: this crate is `no_std` when the
// `std` feature is off (M13/T13.1), and absent entirely without the
// `alloc` feature (T13.4).
use crate::desc::Point;

#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

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
// Only `DisplayList`'s methods drop empty commands, so this follows it
// behind `alloc` (T13.4).
#[cfg(feature = "alloc")]
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
    /// A straight line from `a` to `b`, `width` px thick — T15.3.
    ///
    /// **Always present, in every profile.** A line needs no buffer and
    /// no allocator, and it is most of what "vector drawing" is asked
    /// for in a user interface: rules, separators, borders, underlines,
    /// focus rings, a chart's axes. Arbitrary paths are the expensive
    /// tenth and are gated separately.
    Line { a: Point, b: Point, width: f32, color: Color },
    /// A linear gradient across `rect` — T15.3.
    ///
    /// Stops are **inline and bounded** ([`MAX_STOPS`]), so this costs no
    /// allocator either. Four is what a button, a card or a backdrop
    /// needs; a gradient with more stops than that is a texture, and
    /// belongs in an image.
    Gradient { rect: Rect, stops: Stops, angle: f32 },
    /// A host-owned image, drawn into `rect` — T15.2.
    ///
    /// **`id` names a surface the host holds; the core never carries
    /// pixels and never decodes.** That is what keeps images available
    /// in the `no_std` profile: an embedded host resolves an id to a
    /// slice in flash, a desktop host to a decoded buffer, and the
    /// vocabulary is the same. `tint` multiplies the source, so a
    /// monochrome icon can be recoloured without a second asset;
    /// `Color::WHITE` is untinted.
    Image { rect: Rect, id: ImageId, tint: Color },
    /// A backdrop filter: the pixels already beneath `rect`, blurred and
    /// tinted — M19/T19.0.
    ///
    /// **The only command that reads what is already there.** Everything
    /// else composes forward, which is why a partial redraw can replay a
    /// command list and get the same pixels; this one is a function of
    /// the frame beneath it, and `docs/plan-glass.md` §5 is the damage
    /// rule that follows.
    ///
    /// `blur` is a Gaussian standard deviation in logical px; 0 is a
    /// plain tint. `tint` multiplies over the blurred backdrop and its
    /// alpha is the strength — **alpha 0 is legitimate**, a pure frosted
    /// panel, which is why the collector does not drop it the way
    /// `rounded_rect` drops a transparent fill.
    ///
    /// `radius` is the panel's own, matching [`Cmd::RoundedRect`]'s
    /// meaning. It is not inherited from a clip: clips are axis-aligned
    /// rects here, so there is nothing to round a panel against.
    ///
    /// Everything else is [`Material`], a struct rather than five more
    /// positional `f32`s — `blur` and `rim` and `refraction` are all
    /// lengths and would transpose silently.
    Glass { rect: Rect, radius: f32, material: Material },
    PushClip { rect: Rect },
    PopClip,
}

/// The region a [`Cmd::Glass`] with this blur **reads**, given the rect
/// it writes — M19/T19.2.
///
/// **One expression, called by both the renderer and the damage
/// recorder.** If the capture were wider than the dependency, a partial
/// redraw would read pixels nothing damaged and the panel would be stale
/// in its outermost ring — and the oracle would pass, because both sides
/// would agree about a frame in which that ring never changed. Writing
/// it twice is how they would drift.
///
/// Three box passes of the size SVG's `feGaussianBlur` specifies reach
/// about `3σ`, and a bilinear tap adds one pixel on each side. The `+ 4`
/// is that tap plus the margin T19.3's downsample will need when it
/// lands, so the dependency does not have to widen again then.
pub fn glass_source(rect: Rect, m: &Material) -> Rect {
    let blur = if m.blur.is_finite() && m.blur > 0.0 { m.blur } else { 0.0 };
    let refract = if m.refraction.is_finite() && m.refraction > 0.0 { m.refraction } else { 0.0 };
    if blur == 0.0 && refract == 0.0 {
        return rect;
    }
    // **`refraction` belongs here too**, and leaving it out was a real
    // hole: a bevel samples the backdrop up to `refraction` px outside
    // the panel, so a source that counted only the blur would read
    // pixels nothing had damaged and the rim would go stale.
    //
    // **And dispersion pushes one channel further still** (M22/T22.2).
    // The symptom of leaving *that* out is not a stale ring but
    // something quieter: the sampler clamps, so the outermost fringe
    // flattens against the capture edge *identically in a full and a
    // partial render*, and every gate in the milestone stays green. The
    // existing `+ 4` would have hidden it up to a dispersion of about
    // 0.36, which is most of the useful range — spending documented
    // slack on a second purpose is the drift the doc-currency rule is
    // about.
    let spread = 1.0 + (m.dispersion as f32 / 255.0) * Material::MAX_DISPERSION;
    let reach = crate::num::ceil(3.0 * blur + refract * spread + 4.0);
    [rect[0] - reach, rect[1] - reach, rect[2] + reach, rect[3] + reach]
}

/// What a glass panel is made of — M21.
///
/// Six numbers, and the last four are what separate **frosted** from
/// **liquid**. Frosted is a blur: the backdrop goes soft and that is the
/// whole effect. Liquid is mostly *see-through* — the glassiness comes
/// from an edge that bends what is behind it and catches the light,
/// which is [`Material::refraction`] and [`Material::specular`] over a
/// bevel of width [`Material::rim`].
///
/// A struct and not five more arguments: `blur`, `rim` and `refraction`
/// are all lengths in logical px, and a transposed pair of them
/// compiles and looks plausible.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    /// Gaussian standard deviation for the backdrop blur, in logical
    /// px. 0 is a clear lens — which is what a liquid look wants.
    pub blur: f32,
    /// Multiplied over the backdrop; `tint.3` is its strength. Alpha 0
    /// is a real value.
    pub tint: Color,
    /// Width of the bevelled edge, in logical px. The band within `rim`
    /// of the boundary is where the surface curves, and therefore the
    /// only place refraction and specular do anything.
    ///
    /// **Independent of `refraction`, deliberately.** The first design
    /// derived the rim from the displacement, which made a panel with
    /// `refraction = 0` have no specular either — and a see-through
    /// panel with a lit edge is exactly the case worth having.
    pub rim: f32,
    /// How far, in logical px, the bevel displaces what is seen through
    /// it. 0 is flat glass.
    pub refraction: f32,
    /// Strength of the specular highlight, 0..=1.
    ///
    /// Also the **Fresnel** strength, derived rather than given its own
    /// field: reflectance rising at grazing incidence and a highlight on
    /// the rim are the same property of the same edge, and a panel with
    /// no specular should have neither. Costs zero bytes — M22/T22.0.
    pub specular: f32,
    /// Direction the light comes from, radians clockwise from +x in the
    /// **y-down** screen space both backends work in.
    ///
    /// The *azimuth* only. The elevation is a constant in the renderer,
    /// which is why animating this rotates a highlight around a panel
    /// and cannot sweep it across the rim — the motion a tilting device
    /// actually produces. `docs/plan-liquid.md` §13.
    pub light: f32,
    /// Inner-shadow strength, `0` off to `255` full — M22/T22.0.
    ///
    /// The rim facing away from the light, darkened. Lit on one side and
    /// shaded on the other is what reads as *thickness* rather than as a
    /// bright outline.
    ///
    /// **A `u8`, like every strength here, and that was measured.** As
    /// `f32` the four this milestone adds take `Cmd` from 60 bytes to
    /// 64; as `u8` they fit with room to spare
    /// (`docs/results-glass.md` §0). The same trade `Style::focus_ring`
    /// already made.
    pub shadow: u8,
    /// Chromatic dispersion, `0` off to `255` full — M22/T22.2.
    ///
    /// Glass splits light by wavelength, and the colour fringe at a
    /// thick edge is most of what separates a lens from a blur with a
    /// highlight. **Blue deviates more than red** — normal dispersion —
    /// and the ordering is the thing an eye reads as "lens" rather than
    /// as colour noise.
    ///
    /// Scaled so 255 displaces the outer channels by a quarter of the
    /// refraction either way, which is well past subtle.
    pub dispersion: u8,
    /// Chroma multiplier, **`128` is neutral** — M22/T22.0.
    ///
    /// A blur that desaturates looks like fog; colours read *through*
    /// real glass because the material does not eat them. Encoded
    /// against 128 rather than 255 so the useful range (roughly 1.0 to
    /// 1.4) has resolution and 1.0 is exactly representable.
    pub saturation: u8,
}

impl Default for Material {
    fn default() -> Self {
        Material::FROSTED
    }
}

impl Material {
    /// A frosted panel: blur, a light tint, no bevel. What M19 shipped.
    pub const FROSTED: Material = Material {
        blur: 12.0,
        tint: Color(255, 255, 255, 40),
        rim: 0.0,
        refraction: 0.0,
        specular: 0.0,
        light: -0.9,
        shadow: 0,
        saturation: Material::NEUTRAL_SATURATION,
        dispersion: 0,
    };

    /// How far `dispersion` at 255 pushes the outer channels, as a
    /// fraction of the refraction.
    pub const MAX_DISPERSION: f32 = 0.25;

    /// The `saturation` value that changes nothing.
    ///
    /// Named because a *test* needs it: nothing else asserts that a new
    /// field is inert at its default, and a non-neutral one silently
    /// re-baselines every existing glass test
    /// (`docs/plan-liquid.md` §11).
    pub const NEUTRAL_SATURATION: u8 = 128;

    /// A see-through panel whose edge bends and catches the light.
    ///
    /// Barely any blur — the glassiness is the bevel, not the softness,
    /// which is the whole difference between this and [`FROSTED`].
    ///
    /// [`FROSTED`]: Material::FROSTED
    pub const LIQUID: Material = Material {
        blur: 3.0,
        tint: Color(255, 255, 255, 18),
        rim: 13.0,
        refraction: 11.0,
        specular: 0.9,
        light: -0.9,
        shadow: 85,
        saturation: 148,
        dispersion: 90,
    };
}

/// The alpha a renderer that cannot blur paints a glass panel at.
///
/// Not the tint's own, which is typically around 40 of 255: flat, that
/// is invisible, and a fallback nobody can see is one nobody can test.
/// This is a deliberate lie in the direction of being noticed.
pub const GLASS_FALLBACK_ALPHA: u8 = 140;

/// How many colour stops a [`Cmd::Gradient`] carries.
///
/// Four, because that is what a button, a card or a page backdrop
/// needs, and because the array is inline: raising it costs every
/// gradient command whether or not it uses the room.
pub const MAX_STOPS: usize = 4;

/// The stops of a gradient, inline.
///
/// `offset` is 0..=1 along the gradient's axis. Stops past `len` are
/// ignored, so a two-stop gradient costs the same as a four-stop one in
/// bytes and less in work.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stops {
    pub at: [f32; MAX_STOPS],
    pub color: [Color; MAX_STOPS],
    pub len: u8,
}

impl Stops {
    /// The common case: one colour to another.
    pub fn two(from: Color, to: Color) -> Stops {
        Stops {
            at: [0.0, 1.0, 0.0, 0.0],
            color: [from, to, from, from],
            len: 2,
        }
    }

    /// The colour at `t` in 0..=1, linearly interpolated between the
    /// stops that bracket it.
    pub fn sample(&self, t: f32) -> Color {
        let n = (self.len as usize).min(MAX_STOPS);
        if n == 0 {
            return Color(0, 0, 0, 0);
        }
        let t = crate::num::clamp(t, 0.0, 1.0);
        if t <= self.at[0] {
            return self.color[0];
        }
        for k in 1..n {
            if t <= self.at[k] {
                let span = self.at[k] - self.at[k - 1];
                let f = if span > 0.0 { (t - self.at[k - 1]) / span } else { 0.0 };
                let (a, b) = (self.color[k - 1], self.color[k]);
                let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * f) as u8;
                return Color(l(a.0, b.0), l(a.1, b.1), l(a.2, b.2), l(a.3, b.3));
            }
        }
        self.color[n - 1]
    }
}

/// A host-owned image.
///
/// Opaque on purpose: the core never looks inside one, so a host may
/// number its surfaces however it likes — an index into a `const` array
/// on an embedded target, a slot in a cache on a desktop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "observe", derive(serde::Serialize, serde::Deserialize))]
pub struct ImageId(pub u32);

/// Where a [`Painter`] resolves an [`ImageId`] to pixels.
///
/// Implemented by the host, not the core. `no_std`-clean: the pixels are
/// a borrowed slice, so an embedded host hands back a `&'static [u8]`
/// in flash and nothing is allocated or copied.
pub trait ImageSource {
    /// RGBA8 pixels and dimensions for `id`, or `None` if the host has
    /// no such surface — which a renderer must treat as "draw nothing",
    /// not as an error. A missing asset should not take a frame down.
    fn image(&self, id: ImageId) -> Option<(u32, u32, &[u8])>;
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
            Cmd::Line { a, b, width, color } => {
                f(h, 8);
                for v in [a.x, a.y, b.x, b.y, *width] {
                    f(h, v.to_bits() as u64);
                }
                f(h, u32::from_be_bytes([color.0, color.1, color.2, color.3]) as u64);
            }
            Cmd::Gradient { rect, stops, angle } => {
                f(h, 9);
                for v in rect {
                    f(h, v.to_bits() as u64);
                }
                f(h, angle.to_bits() as u64);
                f(h, stops.len as u64);
                for k in 0..(stops.len as usize).min(MAX_STOPS) {
                    f(h, stops.at[k].to_bits() as u64);
                    let c = stops.color[k];
                    f(h, u32::from_be_bytes([c.0, c.1, c.2, c.3]) as u64);
                }
            }
            Cmd::Image { rect, id, tint } => {
                // **7, not 6.** `Shadow` also hashed as 6 until
                // 2026-09-10, so a shadow and an image with the same
                // rect and colour folded to the same value and retention
                // could not tell one becoming the other apart. Nothing
                // pinned a literal hash, so the fix cost nothing;
                // `Recording`'s parallel numbering had it right all
                // along, which is how the two disagreed unnoticed.
                f(h, 7);
                for v in rect {
                    f(h, v.to_bits() as u64);
                }
                f(h, id.0 as u64);
                f(h, u32::from_be_bytes([tint.0, tint.1, tint.2, tint.3]) as u64);
            }
            Cmd::Glass { rect, radius, material: m } => {
                f(h, 10);
                for v in rect {
                    f(h, v.to_bits() as u64);
                }
                f(h, radius.to_bits() as u64);
                // Every field, and `blur` and `refraction` especially:
                // both size the region this command *reads*, so a hash
                // that skipped one would let a widened source through
                // with no damage and leave the outer ring stale.
                for v in [m.blur, m.rim, m.refraction, m.specular, m.light] {
                    f(h, v.to_bits() as u64);
                }
                f(h, u32::from_be_bytes([m.tint.0, m.tint.1, m.tint.2, m.tint.3]) as u64);
                f(h, ((m.shadow as u64) << 16) | ((m.saturation as u64) << 8) | m.dispersion as u64);
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

    /// A straight line. Provided as a no-op so every existing painter
    /// keeps compiling.
    fn line(&mut self, a: Point, b: Point, width: f32, color: Color) {
        let _ = (a, b, width, color);
    }

    /// A linear gradient across `rect`, at `angle` radians.
    ///
    /// Provided as a **flat fill of the first stop**, not a no-op: a
    /// renderer that cannot interpolate should still put something
    /// approximately right on the screen, and a missing background is a
    /// worse failure than an un-graded one.
    fn gradient(&mut self, rect: Rect, stops: Stops, angle: f32) {
        let _ = angle;
        self.rect(rect, stops.sample(0.0));
    }

    /// Draw a host-owned image into `rect`, multiplied by `tint`.
    ///
    /// Provided as a no-op, so every existing painter keeps compiling
    /// and a renderer that has no image support draws nothing rather
    /// than failing to build.
    fn image(&mut self, rect: Rect, id: ImageId, tint: Color) {
        let _ = (rect, id, tint);
    }
    /// A backdrop filter — M19/T19.0.
    ///
    /// **Provided as a degraded *draw*, not a no-op**, following
    /// `gradient`: a renderer that cannot read its own destination
    /// should still put a panel where the panel is.
    ///
    /// The fallback raises the tint's alpha to [`GLASS_FALLBACK_ALPHA`].
    /// A realistic glass tint is around 16% white, and painting that
    /// flat gives a rectangle you cannot see — which would make every
    /// test of this fallback a test that nothing happened. Ugly and
    /// visible beats faithful and invisible.
    ///
    /// **Wrappers must override this.** A provided body acts on `self`,
    /// so a wrapper that inherits it rewrites the command into a
    /// `rounded_rect` instead of forwarding it. That is not theoretical:
    /// `Deferring` inherited four such bodies and silently dropped every
    /// `Line` and `Image` under an overlay pass until 2026-09-10.
    fn glass(&mut self, rect: Rect, radius: f32, material: Material) {
        let t = material.tint;
        let a = t.3.max(GLASS_FALLBACK_ALPHA);
        self.rounded_rect(rect, Color(t.0, t.1, t.2, a), radius);
    }

    fn push_clip(&mut self, rect: Rect);
    fn pop_clip(&mut self);

    /// A run at the default weight. Provided, so an implementation only
    /// has to write the weighted form.
    fn text(&mut self, rect: Rect, s: &str, size: f32, color: Color) {
        self.text_weighted(rect, s, size, 400.0, color);
    }

    /// The bounds nothing outside of can be seen — the surface, in the
    /// painter's own coordinates. `None` means "unbounded, cull
    /// nothing", which is what every painter answered before T16.12 and
    /// what a painter with no surface (the SVG backend, a checksum
    /// sink) answers still.
    ///
    /// **A container may use this to skip children entirely.** That is
    /// the difference between a 10 000-row list emitting 10 000 text
    /// commands for the renderer to discard one at a time, and emitting
    /// the forty a viewport can show. GTK and Qt both cull in their
    /// paint paths; this crate did not, and `docs/results-competitive.md`
    /// §5b is what that cost.
    ///
    /// Deliberately **not** the current clip stack, which would be
    /// tighter and would require a painter to track one. A superset is
    /// sound for culling — it discards less — and it costs nothing to
    /// answer.
    fn clip_bounds(&self) -> Option<Rect> {
        None
    }

    /// Ask for node `i` to be painted in the **overlay phase** instead
    /// of here — `docs/plan-capabilities.md` T15.0.
    ///
    /// Returns whether the request was taken. **A widget that gets
    /// `false` must paint inline**, which is what makes a painter with
    /// no overlay support a working configuration rather than a broken
    /// one: the content still appears, clipped by its ancestors and
    /// under its later siblings, which is exactly what it did before
    /// overlays existed.
    ///
    /// Provided as a refusal, so every existing `Painter` — including
    /// `DisplayList`, `Scaled`, `Recording` and any host's own — keeps
    /// compiling and keeps its current behaviour.
    fn defer(&mut self, i: u32) -> bool {
        let _ = i;
        false
    }
}

/// One frame's drawing commands, in paint order.
#[cfg(feature = "alloc")]
#[derive(Default, Debug)]
pub struct DisplayList {
    pub cmds: Vec<Cmd>,
    /// Text bytes for this frame; `Cmd::Text` holds ranges into it.
    pub text: String,
    /// The surface this frame is destined for, if the host said
    /// (T16.12). Reported through [`Painter::clip_bounds`] so a
    /// container can cull children that cannot be seen.
    ///
    /// `None` by default, which is what keeps a bare `DisplayList` —
    /// the shape every benchmark arm and every unit test builds —
    /// emitting exactly the commands it emitted before T16.12. A
    /// culled paint number must not be compared with an arm that does
    /// not cull, and this is what makes opting in explicit.
    pub bounds: Option<Rect>,
}

#[cfg(feature = "alloc")]
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

    /// A backdrop filter, dropped only when it has **no area**.
    ///
    /// Deliberately *not* `rounded_rect`'s rule, which also drops a
    /// fully transparent colour. `tint.3 == 0` with a non-zero blur is a
    /// pure frosted panel — the plainest thing this command does — and
    /// copying the neighbouring guard would have dropped it.
    pub fn glass(&mut self, rect: Rect, radius: f32, material: Material) {
        if has_area(rect) {
            self.cmds.push(Cmd::Glass { rect, radius, material });
        }
    }

    /// A line, dropped when it is zero-length or hairline-thin.
    pub fn line(&mut self, a: Point, b: Point, width: f32, color: Color) {
        if width > 0.0 && (a.x != b.x || a.y != b.y) {
            self.cmds.push(Cmd::Line { a, b, width, color });
        }
    }

    /// A gradient, dropped when it has no area or no stops.
    pub fn gradient(&mut self, rect: Rect, stops: Stops, angle: f32) {
        if has_area(rect) && stops.len > 0 {
            self.cmds.push(Cmd::Gradient { rect, stops, angle });
        }
    }

    /// An image, dropped when it has no area — the same rule the other
    /// commands follow.
    pub fn image(&mut self, rect: Rect, id: ImageId, tint: Color) {
        if has_area(rect) {
            self.cmds.push(Cmd::Image { rect, id, tint });
        }
    }

    pub fn push_clip(&mut self, rect: Rect) {
        self.cmds.push(Cmd::PushClip { rect });
    }

    pub fn pop_clip(&mut self) {
        self.cmds.push(Cmd::PopClip);
    }

    /// Declare the surface, so painting can cull to it (T16.12).
    ///
    /// Survives [`DisplayList::clear`]: it is a property of where the
    /// frame is going, not of the frame.
    pub fn set_bounds(&mut self, bounds: Option<Rect>) {
        self.bounds = bounds;
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
// Names `DisplayList` in its signature, so it follows `alloc` (T13.4).
// A host with no list implements `Painter` and rasterises as paint
// arrives -- which is what M10a's seam is for.
#[cfg(feature = "alloc")]
pub trait Renderer {
    fn render(&mut self, list: &DisplayList, width: u32, height: u32);
}

/// A [`Painter`] that multiplies everything by a scale factor and
/// forwards — M10b.
///
/// This is how logical pixels reach a HiDPI surface. Layout, hit
/// testing and everything an agent reports stay in **logical** units, so
/// a 44 px control is 44 px on every display and a rule like "a pointer
/// target is at least 24 px" means what WCAG meant. The physical
/// coordinates a rasteriser needs are produced here, at the last
/// possible moment.
///
/// It exists as a wrapper rather than as a scale field on the renderer
/// because [`Painter`] made it possible: scaling is a transformation of
/// what is drawn, not a property of what draws it, and nothing but this
/// type has to know.
///
/// Text is scaled too — `size` is in logical pixels and reaches the
/// rasteriser in physical ones. Whether the *metrics* were measured at
/// the physical size is the measurer's business (see
/// `lumen-static-shell`'s `FontMetrics::with_scale`), and getting that
/// wrong is what makes scaled text drift from its box.
pub struct Scaled<'a> {
    inner: &'a mut dyn Painter,
    scale: f32,
}

impl<'a> Scaled<'a> {
    /// Wrap `inner`, multiplying by `scale`. A `scale` of 1.0 is a
    /// forwarding no-op, which is the desktop case and costs one
    /// multiply per coordinate.
    pub fn new(inner: &'a mut dyn Painter, scale: f32) -> Self {
        Scaled { inner, scale: if scale.is_finite() && scale > 0.0 { scale } else { 1.0 } }
    }

    #[inline]
    fn r(&self, r: Rect) -> Rect {
        [r[0] * self.scale, r[1] * self.scale, r[2] * self.scale, r[3] * self.scale]
    }
}

impl Painter for Scaled<'_> {
    fn rect(&mut self, rect: Rect, color: Color) {
        let r = self.r(rect);
        self.inner.rect(r, color);
    }
    fn text_weighted(&mut self, rect: Rect, s: &str, size: f32, weight: f32, color: Color) {
        let r = self.r(rect);
        let size = size * self.scale;
        self.inner.text_weighted(r, s, size, weight, color);
    }
    fn rounded_rect(&mut self, rect: Rect, color: Color, radius: f32) {
        let r = self.r(rect);
        let radius = radius * self.scale;
        self.inner.rounded_rect(r, color, radius);
    }
    fn shadow(&mut self, rect: Rect, radius: f32, spread: f32) {
        let r = self.r(rect);
        let (radius, spread) = (radius * self.scale, spread * self.scale);
        self.inner.shadow(r, radius, spread);
    }
    fn line(&mut self, a: Point, b: Point, width: f32, color: Color) {
        let k = self.scale;
        let p = |q: Point| Point { x: q.x * k, y: q.y * k };
        self.inner.line(p(a), p(b), width * k, color);
    }
    fn gradient(&mut self, rect: Rect, stops: Stops, angle: f32) {
        let r = self.r(rect);
        // The angle is not scaled: it is an angle.
        self.inner.gradient(r, stops, angle);
    }
    fn image(&mut self, rect: Rect, id: ImageId, tint: Color) {
        let r = self.r(rect);
        self.inner.image(r, id, tint);
    }
    /// `rect`, `radius` and `blur` are lengths and scale; `tint` is a
    /// colour and does not.
    ///
    /// Getting `blur` wrong here is invisible at scale 1.0 — every test
    /// and every non-HiDPI desktop — and gives a half-strength blur on a
    /// 2x display. That is the shape of bug this whole wrapper exists to
    /// prevent, so it is spelled out field by field.
    fn glass(&mut self, rect: Rect, radius: f32, material: Material) {
        let r = self.r(rect);
        // `blur`, `rim` and `refraction` are lengths and scale; `tint`
        // is a colour, `specular` a 0..=1 strength and `light` an angle,
        // and none of those do.
        let m = Material {
            blur: material.blur * self.scale,
            rim: material.rim * self.scale,
            refraction: material.refraction * self.scale,
            ..material
        };
        self.inner.glass(r, radius * self.scale, m);
    }
    fn push_clip(&mut self, rect: Rect) {
        let r = self.r(rect);
        self.inner.push_clip(r);
    }
    fn pop_clip(&mut self) {
        self.inner.pop_clip();
    }
    fn defer(&mut self, i: u32) -> bool {
        self.inner.defer(i)
    }
    /// Back to **logical** units. Everything above the renderer is
    /// logical, so a container culling against this must be handed the
    /// same coordinate system its `NodeState` is in — dividing here is
    /// the mirror of the multiplication every other method does.
    fn clip_bounds(&self) -> Option<Rect> {
        let k = self.scale;
        self.inner.clip_bounds().map(|r| [r[0] / k, r[1] / k, r[2] / k, r[3] / k])
    }
}

/// Collecting into a list is *one* implementation of [`Painter`], not
/// the definition of painting.
#[cfg(feature = "alloc")]
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
    fn line(&mut self, a: Point, b: Point, width: f32, color: Color) {
        DisplayList::line(self, a, b, width, color)
    }
    fn gradient(&mut self, rect: Rect, stops: Stops, angle: f32) {
        DisplayList::gradient(self, rect, stops, angle)
    }
    fn image(&mut self, rect: Rect, id: ImageId, tint: Color) {
        DisplayList::image(self, rect, id, tint)
    }
    fn glass(&mut self, rect: Rect, radius: f32, material: Material) {
        DisplayList::glass(self, rect, radius, material)
    }
    fn push_clip(&mut self, rect: Rect) {
        DisplayList::push_clip(self, rect)
    }
    fn pop_clip(&mut self) {
        DisplayList::pop_clip(self)
    }
    fn clip_bounds(&self) -> Option<Rect> {
        self.bounds
    }
}
