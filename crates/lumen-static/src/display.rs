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

/// One round of the mixing function every checksum here uses — for
/// **scalars** (a rect's bits, a colour, a size, a tag), never for text.
///
/// Written out three times before 2026-09-02 — twice in this file and
/// once in `ops.rs`, where the shared helper sat with no callers
/// (F6.1). The journal's `measure_into` uses the same constant, so a
/// container's folded signature mixes the way a leaf's does.
///
/// **Linear, and that is why text does not go through it** (M46/T46.1).
/// `h·K + v` over ℤ/2⁶⁴ means two inputs of one length collide whenever
/// a small integer combination of their differences vanishes, which a
/// lattice reduction finds in seconds, and a fold from 0 cannot see a
/// leading zero. For a rect or a colour nobody chooses the difference;
/// for displayed text somebody may. Text goes through [`hash_bytes`].
#[inline]
pub(crate) fn fold(h: u64, v: u64) -> u64 {
    h.wrapping_mul(K).wrapping_add(v)
}

/// `fold`'s multiplier, the golden ratio in 64 bits.
const K: u64 = 0x9E37_79B9_7F4A_7C15;

/// A 64-bit hash of a byte string: **the one text identity** in this
/// crate (M46/T46.1). `Recording::text_spaced` (retention),
/// [`DisplayList::hash_from`] (the agent journal's signatures) and
/// `FoldedText::fold_of` (the `Button`/`Badge`/`Chip` memo key) all call
/// it, so the three cannot disagree about when two strings are the same.
///
/// Seeded with the length, one folded multiply per 8-byte word (the
/// 128-bit product's halves xored, wyhash's step), SplitMix64's
/// finaliser. What that guarantees, and what it does not:
///
/// - **Two strings of different lengths start from different seeds**, so
///   `"x"` and `"\0x"` — equal under a fold from 0 — are separated by
///   construction.
/// - **It is not linear**, so the solved pairs of `fold` (a same-length
///   collision found by lattice reduction) do not carry over, and its
///   step is **not a bijection of the word**, so a second word cannot be
///   *computed* to cancel a first — which the review's rotate-xor-multiply
///   mix allowed with one xor (`docs/plan-hash-identity.md` §6).
/// - **It is unkeyed, and not a security boundary.** Someone who reads
///   this function can search for a collision, and a keyed PRF was
///   measured and refused on cost (`docs/results-retention.md`, "What a
///   text hash guarantees"). What a found collision can do is keep a
///   stale label on screen; it cannot skip anything else.
///
/// The cost against the per-byte fold it replaced is in the same section:
/// cheaper from about a word up, and within its kill rule at 9 B.
#[inline]
pub fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut h = (bytes.len() as u64).wrapping_mul(K);
    let mut words = bytes.chunks_exact(8);
    for w in &mut words {
        let mut b = [0u8; 8];
        b.copy_from_slice(w);
        h = folded_mul(h ^ u64::from_le_bytes(b), P);
    }
    let r = words.remainder();
    if !r.is_empty() {
        h = folded_mul(h ^ tail(r), P);
    }
    finalise(h)
}

/// wyhash's odd multiplier for the per-word step.
const P: u64 = 0xe703_7ed1_a0b4_28db;

/// The low and high halves of the 128-bit product, xored. Unlike a
/// 64-bit multiply this is not invertible, so the step does not map
/// words one-to-one onto states.
#[inline]
fn folded_mul(a: u64, b: u64) -> u64 {
    let p = (a as u128).wrapping_mul(b as u128);
    (p as u64) ^ ((p >> 64) as u64)
}

/// The last 1–7 bytes as one word, without a copy: two overlapping
/// 4-byte loads from four up, three single bytes below. Every byte is
/// read at least once and the length (already in the seed) fixes how
/// they overlap, so for a given length the map is one-to-one.
#[inline]
fn tail(r: &[u8]) -> u64 {
    let n = r.len();
    if n >= 4 {
        let lo = u32::from_le_bytes([r[0], r[1], r[2], r[3]]) as u64;
        let hi = u32::from_le_bytes([r[n - 4], r[n - 3], r[n - 2], r[n - 1]]) as u64;
        lo | (hi << 32)
    } else {
        ((r[0] as u64) << 16) | ((r[n / 2] as u64) << 8) | r[n - 1] as u64
    }
}

/// SplitMix64's finaliser: every input bit reaches every output bit.
#[inline]
fn finalise(mut z: u64) -> u64 {
    z ^= z >> 30;
    z = z.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z ^= z >> 27;
    z = z.wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
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
    ///
    /// `line_height` is the run's line pitch in **sixteenths of `size`**
    /// (16 = 1.0×, 24 = 1.5×), and `0` means the face's own spacing —
    /// what every run painted before M80/T80.1 got, so a tree that sets
    /// nothing paints the same pixels. A ratio, not a length, so
    /// [`Scaled`] forwards it untouched and the renderer multiplies by
    /// the physical size. Where the lines then go is
    /// `FontStack::place_run_into`'s rustdoc (`lumen-static-shell`):
    /// CSS's and iced's half-leading, with a pitch below the face's
    /// spacing keeping its ink inside `rect`.
    Text { rect: Rect, text: (u32, u32), size: f32, weight: f32, line_height: u8, color: Color },
    /// A filled rect with rounded corners. `radius` of 0 is a plain
    /// rect; a renderer that cannot round may treat it as one.
    RoundedRect { rect: Rect, color: Color, radius: f32 },
    /// An **inset** stroked rounded rect: a border, drawn inside `rect`.
    ///
    /// Inset rather than expanded, and that is the whole reason this is a
    /// command instead of a second `RoundedRect` underneath. An underlay
    /// paints *outside* the node's box, which is wrong for a border,
    /// because a bordered and an unbordered control in the same row would
    /// no longer line up. The focus ring was that underlay until M57/A6,
    /// and is now this command too, given a rect outside the node
    /// ([`crate::ops::focus_ring`]).
    ///
    /// **Four `Line`s were tried first, elsewhere, and failed visibly.**
    /// `focus_ring`'s first version drew them and the live gate caught it
    /// within a frame: a square outline over a rounded box covers the
    /// antialiased corner. Four lines also cost four ink rects against
    /// [`crate::retain::MAX_REGIONS`]'s four.
    StrokedRect { rect: Rect, color: Color, radius: f32, width: f32 },
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
    /// tenth, and are [`Cmd::Path`] (M82/T82.5), whose elements live in
    /// the list.
    Line { a: Point, b: Point, width: f32, color: Color },
    /// A linear or radial gradient across `rect`, its corners rounded by
    /// `corner` px — T15.3, widened by M82/T82.2.
    ///
    /// Stops are **inline and bounded** ([`MAX_STOPS`]), so this costs no
    /// allocator either. Four is what a button, a card or a backdrop
    /// needs; a gradient with more stops than that is a texture, and
    /// belongs in an image.
    ///
    /// The shape travels as `kind` and `geom` rather than as a
    /// [`GradShape`], which [`GradShape::from_parts`] rebuilds: an enum
    /// holding three floats is 16 B with its tag and alignment, and this
    /// payload -- 16 + 21 + 1 + 12 + 4 = 54, padded to 56 -- is the one
    /// that sets `Cmd`'s 60 B (`tests/glass_cmd.rs`, K82a in
    /// `docs/plan-paint-vocabulary.md`). `geom` is `[angle, 0, 0]` for
    /// `Linear` and `[cx, cy, radius]` for `Radial`, all relative to
    /// `rect`, so only `corner` is a length.
    Gradient { rect: Rect, stops: Stops, kind: GradKind, geom: [f32; 3], corner: f32 },
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
    /// An arbitrary vector path, filled and/or stroked — M82/T82.5
    /// (`docs/plan-paint-vocabulary.md`, Decisions).
    ///
    /// **The elements live in the list, as `Text`'s bytes do**: `els` is
    /// `(offset, len)` into [`DisplayList::paths`], already through the
    /// path's transform (and [`Scaled`]'s scale), so a renderer reads
    /// device coordinates and never sees an [`Affine`]. `rect` is the ink:
    /// the transformed control-point box grown by `width / 2`, which
    /// contains every flattened curve (a Bézier stays in its hull) and
    /// every round join and cap.
    ///
    /// `fill` is drawn non-zero, then `stroke` over it at `width` with
    /// round joins and caps; either may be transparent. Miter joins,
    /// dashes and even-odd are not built. 16 + 8 + 4 + 4 + 4 = 36 B, well
    /// under the 56 B payload that sets `Cmd`'s size.
    ///
    /// [`Cmd::hash_into`] folds `els.1` and **never the offset** — the
    /// M27/T27.3 rule `Text` follows — and [`DisplayList::hash_from`]
    /// folds the elements themselves.
    Path { rect: Rect, els: (u32, u32), fill: Color, stroke: Color, width: f32 },
    /// Opens a **custom-shader bracket** over `rect` — M83/T83.3
    /// (`docs/plan-gpu-extension.md`, Decisions), behind the core's
    /// `shader` feature.
    ///
    /// The commands up to the matching [`Cmd::ShaderEnd`] (depth-counted;
    /// brackets nest like clips) are the shader's **fallback**, recorded
    /// at paint time by ordinary [`Painter`] calls. A renderer that runs
    /// shader `id` draws it over `rect` and skips to the matching end; one
    /// that cannot — the software `Canvas`, the SVG writer, a GPU whose
    /// pipeline failed — treats this as [`Cmd::PushClip`] and the end as
    /// [`Cmd::PopClip`], so the fallback draws clipped to `rect`. One list
    /// serves both, so the journal hash, `ui.lastDamage` and retention do
    /// not depend on the backend.
    ///
    /// `uniforms` is `(offset, len)` into [`DisplayList::uniforms`], the
    /// `Text` shape. [`Cmd::hash_into`] folds the **length, never the
    /// offset** — the M27/T27.3 rule (61,056 px² of phantom damage when
    /// `Text` folded its offset; `docs/plan-iced-roadmap.md` S2) — and
    /// [`DisplayList::hash_from`] folds the bytes. `id` is an index into
    /// the program's shader table, fixed for a build (the [`ImageId`]
    /// pattern, not a per-frame store index). 16 + 2 + 8 = 26 B of
    /// payload, inside the 56 B that sets `Cmd`'s 60 B
    /// (`tests/glass_cmd.rs`).
    #[cfg(feature = "shader")]
    Shader { rect: Rect, id: ShaderId, uniforms: (u32, u32) },
    /// Closes the innermost open [`Cmd::Shader`] bracket — M83/T83.3. A
    /// renderer that drew the fallback treats it as [`Cmd::PopClip`].
    #[cfg(feature = "shader")]
    ShaderEnd,
}

/// A custom shader, by its index in the program's shader table —
/// M83/T83.3, behind the core's `shader` feature
/// (`docs/plan-gpu-extension.md`).
///
/// **Not a per-frame store index**: the table is fixed for a build, so an
/// id means the same shader in every frame of it, as an [`ImageId`] does.
/// A swap builds a new host whose first frame repaints everything, so a
/// shader edited under the same id cannot go stale. An id no backend knows
/// draws the bracket's fallback.
#[cfg(feature = "shader")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShaderId(pub u16);

/// The most uniform bytes one [`Cmd::Shader`] carries — M83/T83.3. A
/// bracket's uniforms are one 256-byte-aligned slot of the GPU's
/// per-frame buffer (`docs/plan-gpu-extension.md`, "Pipeline state").
#[cfg(feature = "shader")]
pub const MAX_UNIFORMS: usize = 256;

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
    /// Where a second blob sits, in 1/[`Material::MORPH_UNIT`] px from
    /// the panel's own position — M22/T22.5. `0, 0` means there is no
    /// second blob and the panel is an ordinary rounded rect.
    ///
    /// The two blobs merge through a **smooth minimum** of their signed
    /// distance fields, which is what "liquid" means: a neck forms
    /// between them and pinches off as they separate.
    ///
    /// **Fixed-point, and two values rather than three.** Measured
    /// 2026-09-12: the offset as two `i16` takes `Material` to 36 B and
    /// `Cmd::Glass`'s payload to 56 — exactly the headroom, `Cmd` stays
    /// 60. Adding a uniform scale as a third `i16` takes `Material` to
    /// 40 and `Cmd` to **64**, +7% on every command in every list.
    /// `docs/results-glass.md` §0g. So the second blob is the same size
    /// as the first.
    ///
    /// **It is a length**, which matters at the `Scaled` seam: every
    /// other strength this milestone added is unitless and rides
    /// `..material` correctly, and this one does not.
    pub morph_x: i16,
    /// The second blob's vertical offset — see [`Material::morph_x`].
    pub morph_y: i16,
    /// How strongly the tint follows the backdrop, `0` off to `255`
    /// full — M22/T22.3.
    ///
    /// A fixed tint is legible over the backdrop it was chosen for and
    /// nothing else. At `255` the material runs the whole way from
    /// [`Material::tint`] over dark content to
    /// [`Material::tint_over_light`] over bright content, so what sits
    /// *on* the glass stays readable as content scrolls under it.
    ///
    /// **It is a pure function of the captured backdrop.** There is no
    /// stored value and no damping: the renderer has no path back to
    /// application state, and `render_within` runs once per damaged
    /// region, so anything accumulated here would advance a variable
    /// number of times per frame. The crossing is smoothed in
    /// *luminance* instead — see `docs/plan-liquid.md` §5.
    pub adapt: u8,
    /// The tint used over a bright backdrop, when [`Material::adapt`]
    /// is non-zero — M22/T22.3. Ignored when `adapt` is `0`.
    pub tint_over_light: Color,
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
        adapt: 0,
        tint_over_light: Color(0, 0, 0, 0),
        morph_x: 0,
        morph_y: 0,
    };

    /// Fixed-point steps per pixel in [`Material::morph_x`] — 1/16 px,
    /// which puts the offset's range at ±2048 px and its resolution
    /// below what the sub-pixel read of T22.1 can resolve.
    pub const MORPH_UNIT: f32 = 16.0;

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
        adapt: 190,
        morph_x: 0,
        morph_y: 0,
        // Dark and barely there: over bright content the panel needs to
        // *subtract* light, and the same 18/255 the light tint uses
        // would be invisible against it.
        tint_over_light: Color(10, 10, 14, 26),
    };
}

/// The alpha a renderer that cannot blur paints a glass panel at.
///
/// Not the tint's own, which is typically around 40 of 255: flat, that
/// is invisible, and a fallback nobody can see is one nobody can test.
/// This is a deliberate lie in the direction of being noticed.
pub const GLASS_FALLBACK_ALPHA: u8 = 140;

/// Where the adaptive tint sits between [`Material::tint`] and
/// [`Material::tint_over_light`], given the backdrop's mean luminance
/// in `0..=1` — M22/T22.3.
///
/// **Smooth, not thresholded.** A step would make the tint flip between
/// two values as content scrolls, and a single pixel of scroll can cross
/// a step; a smooth-step curve spreads the crossing over the whole range,
/// so the movement between two frames is bounded by the change in mean
/// luminance between them. That bound is what T22.3's pre-registered
/// exit is stated against — `docs/results-glass.md` §0e.
///
/// It is strictly increasing on `(0, 1)`, which the sweep test relies
/// on; the derivative vanishes at both ends, so a sweep that runs to
/// pure black or pure white measures a plateau rather than the curve.
///
/// It lives here rather than in the shell because the GPU backend
/// (T22.6) must compute the same number, and a second copy of a curve
/// is a parity failure waiting for someone to edit one of them.
/// The second blob's offset in pixels — M22/T22.5.
pub fn morph_offset(m: &Material) -> (f32, f32) {
    (m.morph_x as f32 / Material::MORPH_UNIT, m.morph_y as f32 / Material::MORPH_UNIT)
}

/// Whether this material has a second blob at all.
///
/// **A branch on the command, not on the pixel.** M19 records that glass
/// keeps `fill_rounded_within`'s hard-span coverage precisely so a panel
/// is pixel-identical to a rounded rect of the same geometry, and that a
/// pure SDF ramp disagrees by up to 254/255 on a fractional edge
/// (`docs/plan-glass.md` §4.1). The SDF coverage path is taken only when
/// this is true, so a lone panel is byte-identical to before M22/T22.5.
pub fn morphing(m: &Material) -> bool {
    m.morph_x != 0 || m.morph_y != 0
}

/// The rect a panel inks: **the union of both blobs** — M22/T22.5.
///
/// The command's rect is the ink, the clip test *and* the source. A
/// second blob outside it would be a stale panel (damage never covers
/// it), a visible hole (a damage region covering only the second blob
/// fails the clip test, over a region already cleared) and a short
/// dependency. So the widget emits the union and the renderer derives
/// the blobs back out of it — one expression, used by both, for the same
/// reason [`glass_source`] is.
pub fn glass_union(shape: Rect, m: &Material) -> Rect {
    let (dx, dy) = morph_offset(m);
    [
        shape[0] + dx.min(0.0),
        shape[1] + dy.min(0.0),
        shape[2] + dx.max(0.0),
        shape[3] + dy.max(0.0),
    ]
}

/// The primary blob, recovered from the union [`glass_union`] produced.
///
/// Exactly the input when there is no second blob: the offsets are zero,
/// `min` and `max` of zero are zero, and nothing is added or subtracted.
pub fn glass_primary(union: Rect, m: &Material) -> Rect {
    let (dx, dy) = morph_offset(m);
    [
        union[0] - dx.min(0.0),
        union[1] - dy.min(0.0),
        union[2] - dx.max(0.0),
        union[3] - dy.max(0.0),
    ]
}

pub fn adapt_mix(luma: f32, adapt: u8) -> f32 {
    let l = luma.clamp(0.0, 1.0);
    l * l * (3.0 - 2.0 * l) * (adapt as f32 / 255.0)
}

/// How many colour stops a [`Cmd::Gradient`] carries.
///
/// Four, because that is what a button, a card or a page backdrop
/// needs, and because the array is inline: raising it costs every
/// gradient command whether or not it uses the room -- 5 B a stop since
/// M82/T82.1 (a 1/255 offset and a colour), so [`Stops`] is 21 B.
pub const MAX_STOPS: usize = 4;

/// The stops of a gradient, inline.
///
/// `at[k]` is stop `k`'s offset along the gradient's axis **in 1/255
/// steps**: 0 is the start, 255 the end, and 51 is 0.2 exactly. Stops
/// past `len` are ignored, so a two-stop gradient costs the same as a
/// four-stop one in bytes and less in work.
///
/// **Why a byte and not an `f32`** (M82/T82.1,
/// `docs/plan-paint-vocabulary.md` Decisions): `Gradient` is the payload
/// that sets `Cmd`'s 60 B, and M82/T82.2 widens it with a kind, a
/// geometry and a corner. Byte offsets take `Stops` from 36 B to 21 B
/// (align 1), which is what pays for that. An offset that is a multiple
/// of 1/255 samples exactly as the `f32` it replaces; any other moves by
/// at most half a step, a channel delta of at most 1
/// (`docs/results-paint-vocabulary.md` T82.1). Every consumer reads an
/// offset as `at[k] as f32 / 255.0`, never the raw byte.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stops {
    pub at: [u8; MAX_STOPS],
    pub color: [Color; MAX_STOPS],
    pub len: u8,
}

impl Stops {
    /// The common case: one colour to another. `const` since M82/T82.3,
    /// so a fill table entry can be written with it.
    pub const fn two(from: Color, to: Color) -> Stops {
        Stops {
            at: [0, 255, 0, 0],
            color: [from, to, from, from],
            len: 2,
        }
    }

    /// Stop `k`'s offset as a fraction of the axis, 0..=1.
    pub fn offset(&self, k: usize) -> f32 {
        self.at[k] as f32 / 255.0
    }

    /// The colour at `t` in 0..=1, linearly interpolated between the
    /// stops that bracket it.
    pub fn sample(&self, t: f32) -> Color {
        let n = (self.len as usize).min(MAX_STOPS);
        if n == 0 {
            return Color(0, 0, 0, 0);
        }
        let t = crate::num::clamp(t, 0.0, 1.0);
        if t <= self.offset(0) {
            return self.color[0];
        }
        for k in 1..n {
            let (lo, hi) = (self.offset(k - 1), self.offset(k));
            if t <= hi {
                let span = hi - lo;
                let f = if span > 0.0 { (t - lo) / span } else { 0.0 };
                let (a, b) = (self.color[k - 1], self.color[k]);
                let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * f) as u8;
                return Color(l(a.0, b.0), l(a.1, b.1), l(a.2, b.2), l(a.3, b.3));
            }
        }
        self.color[n - 1]
    }
}

/// Which gradient a [`Cmd::Gradient`] draws — M82/T82.2.
///
/// One byte, so the widened command still fits the 56 B payload that
/// sets `Cmd`'s size (`docs/plan-paint-vocabulary.md`, Decisions). The
/// discriminants are the wire encoding's (`wire.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum GradKind {
    Linear = 0,
    Radial = 1,
}

/// A gradient's shape, **relative to the rect it fills** — M82/T82.2.
///
/// Resolution-free on purpose, for two reasons
/// (`docs/plan-paint-vocabulary.md` Decisions): a `const` table entry
/// cannot know the size of the node it will fill, and [`Scaled`] then
/// forwards the shape unchanged -- only a gradient's `corner` is a
/// length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GradShape {
    /// Along the axis at `angle` radians (0 runs left to right), spanning
    /// the whole rect whatever the angle.
    Linear { angle: f32 },
    /// Circular, from `center` -- a fraction of the rect, `[0.5, 0.5]` its
    /// middle -- out to `radius`, a fraction of the distance from the
    /// centre to the rect's **farthest corner**, so 1.0 is CSS's
    /// `farthest-corner`. Ellipses are not built.
    Radial { center: [f32; 2], radius: f32 },
}

impl GradShape {
    /// The command's `kind` byte.
    pub const fn kind(self) -> GradKind {
        match self {
            GradShape::Linear { .. } => GradKind::Linear,
            GradShape::Radial { .. } => GradKind::Radial,
        }
    }

    /// The command's `geom`: `[angle, 0, 0]` or `[cx, cy, radius]`.
    pub const fn geom(self) -> [f32; 3] {
        match self {
            GradShape::Linear { angle } => [angle, 0.0, 0.0],
            GradShape::Radial { center, radius } => [center[0], center[1], radius],
        }
    }

    /// The shape a command's `kind` and `geom` describe: the inverse of
    /// [`kind`](Self::kind) and [`geom`](Self::geom).
    pub const fn from_parts(kind: GradKind, geom: [f32; 3]) -> GradShape {
        match kind {
            GradKind::Linear => GradShape::Linear { angle: geom[0] },
            GradKind::Radial => GradShape::Radial { center: [geom[0], geom[1]], radius: geom[2] },
        }
    }
}

/// Where a pixel sits along a gradient over a concrete rect: the one
/// formula every renderer and the contrast check use — M82/T82.2.
///
/// Resolved once per command, so a per-pixel [`t`](Self::t) is a dot
/// product (linear) or a length (radial) and no trigonometry. The GPU
/// backend uploads exactly these fields ([`GradAxis::uv`]), so the two
/// backends cannot compute the parameter differently.
///
/// Behind `std`: the axis needs `cos`, `sin` and an exact square root,
/// which `core` lacks and [`crate::num`] does not supply, and nothing
/// below the `std` tier interpolates a gradient -- a firmware painter
/// takes [`Painter::gradient`]'s flat default.
#[cfg(feature = "std")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradAxis {
    radial: bool,
    /// The point `t` is measured from: the rect's centre for a linear
    /// gradient, the resolved `center` for a radial one.
    c: [f32; 2],
    /// The unit axis (linear); unused for radial.
    u: [f32; 2],
    /// The length over which `t` runs 0..1: the rect's extent along the
    /// axis (linear), or the radius in px (radial). Never 0.
    span: f32,
}

#[cfg(feature = "std")]
impl GradAxis {
    pub fn new(rect: Rect, kind: GradKind, geom: [f32; 3]) -> GradAxis {
        let (w, h) = (rect[2] - rect[0], rect[3] - rect[1]);
        match kind {
            GradKind::Linear => {
                let (ux, uy) = (geom[0].cos(), geom[0].sin());
                // Half-extent of the box along the gradient's axis, so t
                // spans 0..1 across the whole rect whatever the angle.
                let half = (w * ux).abs() / 2.0 + (h * uy).abs() / 2.0;
                let span = if half > 0.0 { half * 2.0 } else { 1.0 };
                GradAxis { radial: false, c: [(rect[0] + rect[2]) / 2.0, (rect[1] + rect[3]) / 2.0], u: [ux, uy], span }
            }
            GradKind::Radial => {
                let (cx, cy) = (rect[0] + geom[0] * w, rect[1] + geom[1] * h);
                // The farthest corner, so `radius` 1.0 reaches it.
                let dx = (cx - rect[0]).abs().max((rect[2] - cx).abs());
                let dy = (cy - rect[1]).abs().max((rect[3] - cy).abs());
                let r = geom[2] * (dx * dx + dy * dy).sqrt();
                GradAxis { radial: true, c: [cx, cy], u: [0.0, 0.0], span: if r > 0.0 { r } else { 1.0 } }
            }
        }
    }

    /// The gradient parameter at `(x, y)`, unclamped ([`Stops::sample`]
    /// clamps): 0 at the start (the centre, for radial), 1 at the end.
    #[inline]
    pub fn t(&self, x: f32, y: f32) -> f32 {
        let (px, py) = (x - self.c[0], y - self.c[1]);
        if self.radial {
            (px * px + py * py).sqrt() / self.span
        } else {
            (px * self.u[0] + py * self.u[1]) / self.span + 0.5
        }
    }

    /// The four floats a GPU needs to compute [`t`](Self::t) itself:
    /// `(ux, uy, span, 0)` for linear (measured from the rect's centre),
    /// `(cx, cy, radius, 1)` for radial.
    pub fn uv(&self) -> [f32; 4] {
        if self.radial {
            [self.c[0], self.c[1], self.span, 1.0]
        } else {
            [self.u[0], self.u[1], self.span, 0.0]
        }
    }
}

/// One element of a path — M82/T82.5 ([`Painter::path`], [`Cmd::Path`]).
///
/// SVG's and every 2D API's vocabulary: a subpath starts at a `Move`,
/// continues through straight `Line`s and quadratic or cubic Béziers
/// (control points first, the end point last), and a `Close` returns it
/// to its start. A fill closes every subpath whether it says `Close` or
/// not; a stroke draws the closing segment only when it does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathEl {
    Move(Point),
    Line(Point),
    Quad(Point, Point),
    Cubic(Point, Point, Point),
    Close,
}

impl PathEl {
    /// This element with every point through `xf`.
    pub fn map(self, xf: &Affine) -> PathEl {
        let m = |p: Point| affine_apply(xf, p);
        match self {
            PathEl::Move(p) => PathEl::Move(m(p)),
            PathEl::Line(p) => PathEl::Line(m(p)),
            PathEl::Quad(a, p) => PathEl::Quad(m(a), m(p)),
            PathEl::Cubic(a, b, p) => PathEl::Cubic(m(a), m(b), m(p)),
            PathEl::Close => PathEl::Close,
        }
    }

    /// The element's points, control points included, in order: zero for
    /// `Close`, up to three for `Cubic`. The count is the first value.
    pub fn points(&self) -> (usize, [Point; 3]) {
        let z = Point::default();
        match *self {
            PathEl::Move(p) | PathEl::Line(p) => (1, [p, z, z]),
            PathEl::Quad(a, p) => (2, [a, p, z]),
            PathEl::Cubic(a, b, p) => (3, [a, b, p]),
            PathEl::Close => (0, [z; 3]),
        }
    }

    /// The element's tag: its wire byte and the first thing its hash
    /// folds (`wire.rs`).
    pub const fn tag(&self) -> u8 {
        match self {
            PathEl::Move(_) => 0,
            PathEl::Line(_) => 1,
            PathEl::Quad(..) => 2,
            PathEl::Cubic(..) => 3,
            PathEl::Close => 4,
        }
    }
}

/// A 2D affine transform, `[a, b, c, d, e, f]` as in SVG's
/// `matrix(a b c d e f)`: a point goes to `(a·x + c·y + e, b·x + d·y + f)`
/// ([`affine_apply`]) — M82/T82.5.
///
/// **A per-path argument, not a transform stack**
/// (`docs/plan-paint-vocabulary.md`, Decisions): only [`Cmd::Path`] can
/// be transformed, so no other command needs a rule for a rotated
/// context, and neither `Deferring`'s phase 2 nor [`Scaled`] has a scope
/// to balance. Hit testing stays axis-aligned on the node's box.
pub type Affine = [f32; 6];

/// The transform that moves nothing.
pub const AFFINE_IDENTITY: Affine = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `p` through `xf` (see [`Affine`]).
#[inline]
pub fn affine_apply(xf: &Affine, p: Point) -> Point {
    Point { x: xf[0] * p.x + xf[2] * p.y + xf[4], y: xf[1] * p.x + xf[3] * p.y + xf[5] }
}

/// How a [`Painter::path`] is painted — M82/T82.5.
///
/// `fill` is non-zero winding; `stroke` is drawn over it, `width` px wide
/// with round joins and caps. A transparent colour skips that half, and
/// a path with both transparent (or a stroke of no width and no fill)
/// records nothing.
///
/// **`width` is in the painter's units and is not transformed by the
/// path's [`Affine`]**: a path built in a unit frame and scaled up by its
/// transform keeps the stroke it asked for. [`Scaled`] does scale it,
/// because that is logical px becoming physical.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathPaint {
    pub fill: Color,
    pub stroke: Color,
    pub width: f32,
}

/// The box of every point of `els` through `xf`, control points
/// included, **not** grown by any stroke: `None` when there is no point,
/// or one is not finite. [`DisplayList::path`] and `Recording::path` both
/// call it, so the list's ink and retention's cannot disagree about where
/// a path is.
pub fn path_bounds(els: &[PathEl], xf: &Affine) -> Option<Rect> {
    let mut r = [f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY];
    let mut any = false;
    for e in els {
        let (n, ps) = e.points();
        for p in &ps[..n] {
            let q = affine_apply(xf, *p);
            if !(q.x.is_finite() && q.y.is_finite()) {
                return None;
            }
            r = [r[0].min(q.x), r[1].min(q.y), r[2].max(q.x), r[3].max(q.y)];
            any = true;
        }
    }
    any.then_some(r)
}

/// A content hash of a path's elements — M82/T82.5: each element's tag
/// and every coordinate's bits, seeded with the count. What
/// [`DisplayList::hash_from`] folds for a [`Cmd::Path`] (whose own
/// [`Cmd::hash_into`] sees only the count) and what `Recording::path`
/// folds, so retention and the journal agree about when two paths are the
/// same.
pub fn hash_path(els: &[PathEl]) -> u64 {
    let mut h = fold(0, els.len() as u64);
    for e in els {
        h = fold(h, e.tag() as u64);
        let (n, ps) = e.points();
        for p in &ps[..n] {
            h = fold(h, ((p.x.to_bits() as u64) << 32) | p.y.to_bits() as u64);
        }
    }
    h
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
            Cmd::StrokedRect { rect, color, radius, width } => {
                f(h, 11);
                for v in rect {
                    f(h, v.to_bits() as u64);
                }
                f(h, radius.to_bits() as u64);
                f(h, width.to_bits() as u64);
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
            Cmd::Text { rect, text, size, weight, line_height, color } => {
                f(h, 2);
                for v in rect {
                    f(h, v.to_bits() as u64);
                }
                // **The length, not the range** — M27/T27.3.
                //
                // This folded `(start, end)`, which is an offset into the
                // *shared* text buffer. `journal::measure_into` paints
                // every node into one list, so a node whose text changed
                // length shifted the offset of every node painted after
                // it, and every one of those was reported as a content
                // change it had not made. In the showcase, the readout
                // going from "checked: false" to "checked: true" — one
                // byte shorter — reported the 954 px note at the bottom
                // of the window as repainted, and **61 056 px² of
                // phantom damage** for a click on a 24x24 checkbox.
                //
                // The length still distinguishes runs of different size.
                // The *bytes* are folded by
                // [`DisplayList::hash_from`], which is the only caller
                // that has the buffer to read them from, and is what
                // every production path uses.
                //
                // **`text` is `(offset, len)`, not `(start, end)`** — see
                // [`DisplayList::str_of`], which slices
                // `text[o..o + len]`. The first attempt at this fix wrote
                // `text.1 - text.0`, which is offset-dependent again and
                // fixed nothing; the test above went straight back to
                // red, which is the only reason it was caught.
                f(h, text.1 as u64);
                f(h, size.to_bits() as u64);
                f(h, weight.to_bits() as u64);
                // The pitch moves every line after the first, inside a box
                // that need not change size (M80/T80.1).
                f(h, *line_height as u64);
                f(h, u32::from_be_bytes([color.0, color.1, color.2, color.3]) as u64);
            }
            Cmd::Line { a, b, width, color } => {
                f(h, 8);
                for v in [a.x, a.y, b.x, b.y, *width] {
                    f(h, v.to_bits() as u64);
                }
                f(h, u32::from_be_bytes([color.0, color.1, color.2, color.3]) as u64);
            }
            Cmd::Gradient { rect, stops, kind, geom, corner } => {
                f(h, 9);
                for v in rect {
                    f(h, v.to_bits() as u64);
                }
                // M82/T82.2: the shape and the corner, so a radial whose
                // centre moves inside the same rect is a change.
                f(h, *kind as u64);
                for v in geom {
                    f(h, v.to_bits() as u64);
                }
                f(h, corner.to_bits() as u64);
                f(h, stops.len as u64);
                for k in 0..(stops.len as usize).min(MAX_STOPS) {
                    f(h, stops.at[k] as u64);
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
                f(h, ((m.shadow as u64) << 24)
                    | ((m.saturation as u64) << 16)
                    | ((m.dispersion as u64) << 8)
                    | m.adapt as u64);
                let t2 = m.tint_over_light;
                f(h, u32::from_be_bytes([t2.0, t2.1, t2.2, t2.3]) as u64);
                f(h, ((m.morph_x as u16 as u64) << 16) | m.morph_y as u16 as u64);
            }
            Cmd::PushClip { rect } => {
                f(h, 3);
                for v in rect {
                    f(h, v.to_bits() as u64);
                }
            }
            Cmd::PopClip => f(h, 4),
            Cmd::Path { rect, els, fill, stroke, width } => {
                f(h, 12);
                for v in rect {
                    f(h, v.to_bits() as u64);
                }
                // **The count, never the offset** (M82/T82.5), for
                // `Text`'s reason above: `els.0` indexes the list's shared
                // element store, so a path painted earlier that gained an
                // element would shift every later path's offset and report
                // it changed. The elements are folded by
                // [`DisplayList::hash_from`].
                f(h, els.1 as u64);
                f(h, u32::from_be_bytes([fill.0, fill.1, fill.2, fill.3]) as u64);
                f(h, u32::from_be_bytes([stroke.0, stroke.1, stroke.2, stroke.3]) as u64);
                f(h, width.to_bits() as u64);
            }
            #[cfg(feature = "shader")]
            Cmd::Shader { rect, id, uniforms } => {
                f(h, 13);
                for v in rect {
                    f(h, v.to_bits() as u64);
                }
                f(h, id.0 as u64);
                // **The length, never the offset** (M83/T83.3), for
                // `Text`'s reason above: `uniforms.0` indexes the list's
                // shared uniform buffer, so a bracket painted earlier whose
                // uniforms grew would shift every later bracket's offset
                // and report it changed. The bytes are folded by
                // [`DisplayList::hash_from`].
                f(h, uniforms.1 as u64);
            }
            #[cfg(feature = "shader")]
            Cmd::ShaderEnd => f(h, 14),
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
/// The rect a shadow of `spread` under `rect` is drawn around: dropped
/// by a third of its spread, like light from above.
pub fn shadow_drop(rect: Rect, spread: f32) -> Rect {
    [rect[0], rect[1] + spread * 0.35, rect[2], rect[3] + spread * 0.35]
}

/// **Everything a shadow of `spread` under `rect` can touch** — M57/A14.
///
/// The drop above, then the spread on every side. Retention's damage for
/// a shadow was the *undropped* rect inflated by the spread, so the lowest
/// third of every shadow was outside it: a popup that shrank left the
/// bottom of its old shadow on screen as a few grey rows
/// (`docs/widget-audit.md`, Combobox). One function, used by both the
/// list that draws the shadow and the recording that damages it, so the
/// two cannot drift apart again.
pub fn shadow_ink(rect: Rect, spread: f32) -> Rect {
    let r = shadow_drop(rect, spread);
    [r[0] - spread, r[1] - spread, r[2] + spread, r[3] + spread]
}

pub trait Painter {
    fn rect(&mut self, rect: Rect, color: Color);
    /// A text run at a line pitch of `line_height` sixteenths of `size`,
    /// `0` meaning the face's own spacing (M80/T80.1; the semantics are
    /// [`Cmd::Text`]'s).
    ///
    /// **Required**, for the reason [`Painter::stroked_rect`] gives: the
    /// byte has no lossy fallback, and a wrapper that inherited a body
    /// would drop it. Every wrapper (`Scaled`, `Deferring`, `Recording`)
    /// implements this one and **not** [`Painter::text_weighted`], so no
    /// wrapper holds two text bodies that must agree.
    fn text_spaced(&mut self, rect: Rect, s: &str, size: f32, weight: f32, line_height: u8, color: Color);
    /// A run at the face's own line spacing. **Provided**, forwarding to
    /// [`Painter::text_spaced`] with `0`, exactly as [`Painter::text`]
    /// forwards here. That is not the provided-method trap the rest of
    /// this trait warns about: the body calls a *required* method on the
    /// same painter, which a wrapper implements by forwarding, so nothing
    /// is dropped whichever of the three a caller uses.
    fn text_weighted(&mut self, rect: Rect, s: &str, size: f32, weight: f32, color: Color) {
        self.text_spaced(rect, s, size, weight, 0, color);
    }
    fn rounded_rect(&mut self, rect: Rect, color: Color, radius: f32);
    fn shadow(&mut self, rect: Rect, radius: f32, spread: f32);
    /// A border: an inset stroked rounded rect (M27/T27.1).
    ///
    /// **Required, deliberately, and this is the one design note worth
    /// reading before adding another method here.** Six of this trait's
    /// methods are provided, and a provided body acts on the *wrapper*
    /// rather than forwarding — which is how `Deferring` silently dropped
    /// every `Line` and `Image` under an overlay pass for a year. A
    /// border has no lossy fallback that is worth that risk, so this one
    /// is required and every wrapper of `Painter` is a compile error
    /// until it forwards.
    fn stroked_rect(&mut self, rect: Rect, color: Color, radius: f32, width: f32);

    /// A straight line. Provided as a no-op so every existing painter
    /// keeps compiling.
    fn line(&mut self, a: Point, b: Point, width: f32, color: Color) {
        let _ = (a, b, width, color);
    }

    /// A gradient across `rect` in `shape`, its corners rounded by
    /// `corner` px (M82/T82.2; [`Cmd::Gradient`]).
    ///
    /// Provided as a **flat fill of the first stop**, rounded by the
    /// same corner, not a no-op: a renderer that cannot interpolate
    /// should still put something approximately right on the screen, and
    /// a missing background is a worse failure than an un-graded one.
    /// This is `docs/plan-iced-roadmap.md` §2.2's documented exception to
    /// "a new method is required"; the three wrappers ([`Scaled`],
    /// `Deferring`, `Recording`) override it.
    fn gradient(&mut self, rect: Rect, stops: Stops, shape: GradShape, corner: f32) {
        let _ = shape;
        self.rounded_rect(rect, stops.sample(0.0), corner);
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

    /// The fill table, resolved against the palette — M82/T82.3
    /// (`docs/plan-paint-vocabulary.md`). [`crate::ops::fill`] reads entry
    /// `NodeState::fill - 1` from it and paints it as an ordinary
    /// [`Painter::gradient`], so no command ever carries the index.
    ///
    /// **Required**, for [`Painter::stroked_rect`]'s reason: a wrapper
    /// that inherited an empty default would silently drop every fill
    /// painted through it — `Deferring`'s phase 2 included. A wrapper
    /// forwards its inner painter's answer; a leaf with no table (the
    /// firmware's `Checksum`, a test sink) answers `&[]` and paints the
    /// flat background alone. `DisplayList` answers what the host gave
    /// [`DisplayList::set_fills`]. Accepted firmware growth by owner
    /// decision D6 (`docs/plan-iced-roadmap.md`).
    fn fills(&self) -> &[crate::style::Fill];

    /// An arbitrary path, its points through `xf`, filled and stroked as
    /// `paint` says — M82/T82.5 ([`Cmd::Path`], [`PathEl`], [`Affine`]).
    ///
    /// **Required**, for [`Painter::stroked_rect`]'s reason: a wrapper
    /// that inherited a body would drop every path painted through it,
    /// `Deferring`'s phase 2 included, and there is no lossy fallback a
    /// leaf could draw instead. A wrapper forwards — [`Scaled`] after
    /// pre-multiplying `xf` by its scale and scaling `width` — and
    /// `Recording` hashes `xf`, the elements and the paint and inks the
    /// transformed box. The transform is this argument and nothing else:
    /// there is no transform stack. Accepted firmware growth by owner
    /// decision D3 (`docs/plan-iced-roadmap.md`).
    fn path(&mut self, els: &[PathEl], xf: Affine, paint: PathPaint);

    /// Open a custom-shader bracket over `rect` — M83/T83.3
    /// ([`Cmd::Shader`]), behind the core's `shader` feature. What is
    /// painted until the matching [`Painter::shader_end`] is the shader's
    /// **fallback**; `uniforms` (at most [`MAX_UNIFORMS`] bytes) is handed
    /// to shader `id` as its `@binding(1)` block.
    ///
    /// **Provided, as `push_clip(rect)`, and that is a documented leaf
    /// contract, not a convenience** (`docs/plan-iced-roadmap.md` §2.2's
    /// one exception to "a new method is required"). A leaf that cannot
    /// run shaders should draw the fallback clipped to the rect, which is
    /// exactly what this body makes it do. A *required* method behind a
    /// feature would instead break every leaf painter in any crate that
    /// does not forward the feature — the firmware's `Checksum`, the
    /// retain bench's `Sink` — the moment anything in the graph turned
    /// `lumen-static/shader` on.
    ///
    /// **Wrappers must override this and [`Painter::shader_end`]**, as
    /// [`Painter::glass`] says: an inherited body turns the bracket into a
    /// plain clip on the wrapper and the shader is never seen downstream.
    /// [`Scaled`], `Deferring` and `Recording` override both
    /// (`tests/shader_cmd.rs`, `deferring_forwards_the_bracket`).
    #[cfg(feature = "shader")]
    fn shader(&mut self, rect: Rect, id: ShaderId, uniforms: &[u8]) {
        let _ = (id, uniforms);
        self.push_clip(rect);
    }

    /// Close the innermost open bracket — M83/T83.3. Provided as
    /// `pop_clip()`, the other half of [`Painter::shader`]'s leaf
    /// default; wrappers override it with that method.
    #[cfg(feature = "shader")]
    fn shader_end(&mut self) {
        self.pop_clip();
    }

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
///
/// **`Clone`, so a copy carries every store** (M82/T82.5, adopting
/// M83/T83.3's design): winit's `hot::Barred` copies a session's list,
/// and a copy built field by field — or with `..Default::default()` —
/// would compile and leave [`DisplayList::paths`] empty while each
/// [`Cmd::Path`] still indexed into it.
#[cfg(feature = "alloc")]
#[derive(Default, Debug, Clone)]
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
    /// The fill table this frame resolves `NodeState::fill` against —
    /// M82/T82.3, answered by [`Painter::fills`]. Set by the host
    /// ([`DisplayList::set_fills`]) and, like `bounds`, kept across
    /// [`DisplayList::clear`]: it is a property of the program and its
    /// palette, not of one frame. Empty by default, so a bare list paints
    /// no fill and every existing test's command stream is unchanged.
    pub fills: Vec<crate::style::Fill>,
    /// Path elements for this frame, already through each path's
    /// transform; `Cmd::Path` holds `(offset, len)` ranges into it, as
    /// `Cmd::Text` does into `text` — M82/T82.5. Cleared with the frame,
    /// so its allocation is reused like `text`'s.
    pub paths: Vec<PathEl>,
    /// Uniform bytes for this frame's shader brackets — M83/T83.3, behind
    /// `shader`. `Cmd::Shader` holds `(offset, len)` ranges into it, as
    /// `Cmd::Text` does into `text`; cleared with the frame so its
    /// allocation is reused, and copied by `Clone` with the commands that
    /// index it (winit's `hot::Barred`).
    #[cfg(feature = "shader")]
    pub uniforms: Vec<u8>,
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
        self.paths.clear();
        #[cfg(feature = "shader")]
        self.uniforms.clear();
    }

    /// Open a shader bracket (M83/T83.3, [`Cmd::Shader`]): the bytes are
    /// appended to [`DisplayList::uniforms`] and the command holds their
    /// range.
    ///
    /// **Never dropped**, unlike [`DisplayList::rect`]'s empty rect: the
    /// matching [`DisplayList::shader_end`] is recorded unconditionally,
    /// so dropping the open would unbalance it, as a dropped
    /// [`DisplayList::push_clip`] would. Bytes past [`MAX_UNIFORMS`] are
    /// not recorded; a debug build asserts.
    #[cfg(feature = "shader")]
    pub fn shader(&mut self, rect: Rect, id: ShaderId, uniforms: &[u8]) {
        debug_assert!(uniforms.len() <= MAX_UNIFORMS, "{} uniform bytes, past MAX_UNIFORMS", uniforms.len());
        let bytes = &uniforms[..uniforms.len().min(MAX_UNIFORMS)];
        let start = self.uniforms.len() as u32;
        self.uniforms.extend_from_slice(bytes);
        self.cmds.push(Cmd::Shader { rect, id, uniforms: (start, bytes.len() as u32) });
    }

    /// Close the innermost shader bracket (M83/T83.3, [`Cmd::ShaderEnd`]).
    #[cfg(feature = "shader")]
    pub fn shader_end(&mut self) {
        self.cmds.push(Cmd::ShaderEnd);
    }

    /// The uniform bytes a `Cmd::Shader` refers to.
    #[cfg(feature = "shader")]
    pub fn uniforms_of(&self, range: (u32, u32)) -> &[u8] {
        &self.uniforms[range.0 as usize..(range.0 + range.1) as usize]
    }

    /// Record a path (M82/T82.5, [`Cmd::Path`]): `xf` is applied as the
    /// elements are copied into [`DisplayList::paths`], and the command's
    /// rect is [`path_bounds`] grown by `width / 2`.
    ///
    /// Dropped when nothing would draw — no element with a point, a
    /// non-finite one, both paints transparent (a stroke of no width
    /// counts as transparent), or an ink box with no area — for
    /// [`DisplayList::rect`]'s reason.
    pub fn path(&mut self, els: &[PathEl], xf: Affine, paint: PathPaint) {
        let stroked = paint.stroke.3 != 0 && paint.width > 0.0;
        if paint.fill.3 == 0 && !stroked {
            return;
        }
        let Some(b) = path_bounds(els, &xf) else { return };
        let w = if paint.width > 0.0 { paint.width / 2.0 } else { 0.0 };
        let rect = [b[0] - w, b[1] - w, b[2] + w, b[3] + w];
        if !has_area(rect) {
            return;
        }
        let start = self.paths.len() as u32;
        self.paths.extend(els.iter().map(|e| e.map(&xf)));
        self.cmds.push(Cmd::Path {
            rect,
            els: (start, els.len() as u32),
            fill: paint.fill,
            stroke: paint.stroke,
            width: paint.width,
        });
    }

    /// The elements a `Cmd::Path` refers to.
    pub fn els_of(&self, range: (u32, u32)) -> &[PathEl] {
        &self.paths[range.0 as usize..(range.0 + range.1) as usize]
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
        self.text_spaced(rect, s, size, weight, 0, color);
    }

    /// Record a text run at a line pitch of `line_height` sixteenths of
    /// `size` (`0` = the face's own spacing) — M80/T80.1, `Cmd::Text`.
    pub fn text_spaced(&mut self, rect: Rect, s: &str, size: f32, weight: f32, line_height: u8, color: Color) {
        if !has_area(rect) {
            return;
        }
        let start = self.text.len() as u32;
        self.text.push_str(s);
        let range = (start, self.text.len() as u32 - start);
        self.cmds.push(Cmd::Text { rect, text: range, size, weight, line_height, color });
    }

    /// Record a filled rect with rounded corners.
    pub fn rounded_rect(&mut self, rect: Rect, color: Color, radius: f32) {
        if has_area(rect) && color.3 != 0 {
            self.cmds.push(Cmd::RoundedRect { rect, color, radius });
        }
    }

    /// A border, inset into `rect` (M27/T27.1).
    ///
    /// Zero width or a transparent colour records nothing: a `Style` with
    /// no border is the common case by a wide margin, and a command that
    /// paints nothing still costs a retention hash and an ink rect
    /// against [`crate::retain::MAX_REGIONS`].
    pub fn stroked_rect(&mut self, rect: Rect, color: Color, radius: f32, width: f32) {
        if width <= 0.0 || color.3 == 0 {
            return;
        }
        self.cmds.push(Cmd::StrokedRect { rect, color, radius, width });
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
        let r = shadow_drop(rect, spread);
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
    pub fn gradient(&mut self, rect: Rect, stops: Stops, shape: GradShape, corner: f32) {
        if has_area(rect) && stops.len > 0 {
            self.cmds.push(Cmd::Gradient { rect, stops, kind: shape.kind(), geom: shape.geom(), corner });
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

    /// Install the resolved fill table — M82/T82.3. Copied, and kept
    /// across [`DisplayList::clear`] as `bounds` is; the host calls it on
    /// `set_fills` and `set_palette`, for its frame list and for the
    /// shadow oracle's scratch list.
    pub fn set_fills(&mut self, fills: &[crate::style::Fill]) {
        self.fills.clear();
        self.fills.extend_from_slice(fills);
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
    /// signature without building a list per node. A text run's bytes and
    /// a path's elements (M82/T82.5) are folded here, because only the
    /// list holds them; their commands fold a length, never an offset.
    pub fn hash_from(&self, from: usize) -> u64 {
        let mut h = 0u64;
        for c in &self.cmds[from.min(self.cmds.len())..] {
            c.hash_into(&mut h);
            match c {
                Cmd::Text { text, .. } => h = fold(h, hash_bytes(self.str_of(*text).as_bytes())),
                // The elements, which `hash_into` cannot reach (M82/T82.5).
                Cmd::Path { els, .. } => h = fold(h, hash_path(self.els_of(*els))),
                // The uniform bytes, which `hash_into` cannot reach
                // (M83/T83.3): same offset and length, new bytes, is a
                // change.
                #[cfg(feature = "shader")]
                Cmd::Shader { uniforms, .. } => h = fold(h, hash_bytes(self.uniforms_of(*uniforms))),
                _ => {}
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
/// A plain rect's edges land on device pixels ([`Painter::rect`] on
/// this type, M40/T40.3); every other shape keeps its exact scaled
/// geometry.
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
    /// Wrap `inner`, multiplying by `scale`. At a `scale` of 1.0 every
    /// method forwards its input unchanged **except `rect`**, which
    /// snaps a fractional edge to a whole pixel at every scale
    /// (M40/T40.3) — so this is no longer a pure forwarding wrapper at
    /// 1.0, and costs one multiply per coordinate plus, for a plain
    /// rect, four roundings.
    pub fn new(inner: &'a mut dyn Painter, scale: f32) -> Self {
        Scaled { inner, scale: if scale.is_finite() && scale > 0.0 { scale } else { 1.0 } }
    }

    #[inline]
    fn r(&self, r: Rect) -> Rect {
        [r[0] * self.scale, r[1] * self.scale, r[2] * self.scale, r[3] * self.scale]
    }

    /// [`Scaled::r`], with every edge **on the device pixel grid** —
    /// M57/A15 (`docs/plan-widget-fixes.md`).
    ///
    /// Layout centres things, and centring lands on halves: a divider at
    /// y 428.5–429.5 was rasterised as two full rows, because the
    /// software renderer rounds an axis-aligned edge outward
    /// (`docs/widget-audit.md` X10) — so a 1 px hairline drew 2 px wide
    /// and a bordered button's edge smeared. Rounding the edges here,
    /// where logical becomes physical, is the one place every fill and
    /// border passes, at any scale. An edge pair that would round to
    /// nothing keeps its physical extent instead, so a hairline narrower
    /// than a pixel is never erased.
    #[inline]
    fn snapped(&self, r: Rect) -> Rect {
        let p = self.r(r);
        let (x0, x1) = snap_pair(p[0], p[2]);
        let (y0, y1) = snap_pair(p[1], p[3]);
        [x0, y0, x1, y1]
    }
}

/// Round an edge pair to whole pixels, unless that would leave nothing.
#[inline]
fn snap_pair(a: f32, b: f32) -> (f32, f32) {
    let (ra, rb) = (crate::num::round(a), crate::num::round(b));
    if rb > ra || b <= a {
        (ra, rb)
    } else {
        (a, b)
    }
}

/// `x` to the nearest integer, **ties to even** — the FPU's own rounding,
/// borrowed by adding and subtracting 1.5 · 2²³: at that magnitude an
/// `f32` has no fractional bits, so the addition itself rounds.
///
/// Not `f32::round`, which is std-only in a `no_std` core, and not a
/// floor-and-compare on `crate::num::floor`: that was built first, branchy
/// and then branch-free, and both cost several times a plain forward on
/// a rect-heavy list, where this costs a few nanoseconds a rect
/// (`docs/results-gpu.md` §2a has the arms). Ties go to even rather than
/// up, which changes which of two equally near pixels a `.5` edge takes
/// and nothing else: the rule is one function of one value, so two rects
/// that share an edge still agree on it.
///
/// For finite `x` only — [`Scaled`]'s `rect` checks first. Valid for
/// |x| < 2²²; anything larger has no fraction to round and is returned
/// as it is. Rust never reassociates float arithmetic, so the
/// two operations cannot be folded away.
#[inline]
fn nearest(x: f32) -> f32 {
    const MAGIC: f32 = 12_582_912.0; // 1.5 · 2²³
    // Finite by the caller's check; at 2²² and above there is no
    // fraction left to round.
    if x.abs() >= 4_194_304.0 {
        return x;
    }
    (x + MAGIC) - MAGIC
}

/// One axis of a snapped rect: each edge to its nearest device pixel,
/// independently, and at least one pixel between them.
#[inline]
fn snap_axis(a: f32, b: f32) -> (f32, f32) {
    let (p, q) = (nearest(a), nearest(b));
    (p, q.max(p + 1.0))
}

impl Painter for Scaled<'_> {
    /// **Snapped to device pixels** — M40/T40.3. Each scaled edge rounds
    /// to the nearest whole pixel, and an axis whose two edges round
    /// together keeps one pixel.
    ///
    /// Without it a 1-logical-px divider at logical y = 22 and scale
    /// 1.25 became `27.5..28.75`, which the software `fill` covers
    /// `floor..ceil` — two whole rows where it now inks one — while the
    /// GPU ramped the same edges, so the backends disagreed on every
    /// hairline at every fractional scale (R9-F5; the ratios, before and
    /// after, are `docs/results-gpu.md` §2a). A whole-pixel edge is one
    /// both backends draw identically.
    ///
    /// **Edges, not origin and size**: two rects that share an edge still
    /// share it after rounding, with no gap row and no overlap row. The
    /// price is that a hairline is one or two pixels depending on where
    /// it sits at 1.5, which is what platform toolkits draw too.
    ///
    /// This is the plain-rect path. Rounded and stroked rects are snapped
    /// too, through `Scaled::snapped` (M57/A15): a bordered button's
    /// edge and a focus ring smeared across two rows at a fractional
    /// layout edge, and the SDF the backends share agrees on a snapped
    /// rect as well as on an exact one. A text run's origin goes to the
    /// grid and its size is kept. Lines, gradients, images and glass keep
    /// their exact geometry. An empty or
    /// non-finite rect is forwarded unsnapped, so snapping can never
    /// turn nothing into a pixel. Applies at 1.0 too, where a fractional
    /// *layout* edge over-inked by the same mechanism
    /// (`crates/lumen-static-shell/tests/fractional_ink.rs`;
    /// the kill rule and its cost are `docs/results-gpu.md` §2a).
    fn rect(&mut self, rect: Rect, color: Color) {
        let r = self.r(rect);
        if !(r[0] < r[2] && r[1] < r[3] && r.iter().all(|v| v.is_finite())) {
            self.inner.rect(r, color);
            return;
        }
        let (x0, x1) = snap_axis(r[0], r[2]);
        let (y0, y1) = snap_axis(r[1], r[3]);
        self.inner.rect([x0, y0, x1, y1], color);
    }
    /// The one text body: `text` and `text_weighted` arrive here through
    /// their provided forwards (M80/T80.1), so every run is rounded by it.
    /// `line_height` is a ratio and is forwarded unscaled.
    fn text_spaced(&mut self, rect: Rect, s: &str, size: f32, weight: f32, line_height: u8, color: Color) {
        // The run's *origin* on the grid, its size kept: a glyph run
        // starting on a half pixel is rasterised soft (M57/A15).
        let r = self.r(rect);
        let (dx, dy) = (crate::num::round(r[0]) - r[0], crate::num::round(r[1]) - r[1]);
        let r = [r[0] + dx, r[1] + dy, r[2] + dx, r[3] + dy];
        let size = size * self.scale;
        self.inner.text_spaced(r, s, size, weight, line_height, color);
    }
    fn rounded_rect(&mut self, rect: Rect, color: Color, radius: f32) {
        let r = self.snapped(rect);
        let radius = radius * self.scale;
        self.inner.rounded_rect(r, color, radius);
    }
    fn stroked_rect(&mut self, rect: Rect, color: Color, radius: f32, width: f32) {
        let r = self.snapped(rect);
        // The width scales too. A border that kept its logical width on a
        // 2x display would be half as thick as everything around it.
        self.inner.stroked_rect(r, color, radius * self.scale, width * self.scale);
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
    fn gradient(&mut self, rect: Rect, stops: Stops, shape: GradShape, corner: f32) {
        let r = self.r(rect);
        // The shape is not scaled: it is relative to the rect (an angle,
        // or fractions of it). Only the corner is a length (M82/T82.2).
        self.inner.gradient(r, stops, shape, corner * self.scale);
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
        // `blur`, `rim`, `refraction` and the morph offset are lengths
        // and scale; `tint` is a colour, `specular`, `saturation`,
        // `dispersion` and `adapt` are unitless strengths and `light` is
        // an angle, and none of those do.
        //
        // **The rest pattern fails open for a length**, which is why the
        // morph offset is named here and every unitless field M22 added
        // is not: `..material` silently does the right thing for a
        // fraction and the wrong thing for a distance, and the symptom
        // is a second blob that sits at the correct place at 1x and
        // half-way there at 2x.
        let q = |v: i16| {
            let scaled = v as f32 * self.scale;
            // Saturating, because the offset's range is finite and a
            // wrapped one would put the blob on the opposite side.
            scaled.clamp(i16::MIN as f32, i16::MAX as f32) as i16
        };
        let m = Material {
            blur: material.blur * self.scale,
            rim: material.rim * self.scale,
            refraction: material.refraction * self.scale,
            morph_x: q(material.morph_x),
            morph_y: q(material.morph_y),
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
    /// Forwarded: a table entry is resolution-free, and the gradient it
    /// becomes is scaled by [`Scaled::gradient`] like any other.
    fn fills(&self) -> &[crate::style::Fill] {
        self.inner.fills()
    }
    /// `xf` is **pre-multiplied** by the scale — the scale applied after
    /// the path's own transform — and `width` scales with it (M82/T82.5).
    /// Forwarding `xf` unscaled would draw every path at its logical size
    /// on a 2x display (`lumen-static-shell/tests/path_damage.rs`).
    fn path(&mut self, els: &[PathEl], xf: Affine, paint: PathPaint) {
        let k = self.scale;
        let xf = [xf[0] * k, xf[1] * k, xf[2] * k, xf[3] * k, xf[4] * k, xf[5] * k];
        self.inner.path(els, xf, PathPaint { width: paint.width * k, ..paint });
    }
    /// The rect scales; the uniform bytes are forwarded untouched
    /// (M83/T83.3) — they are the shader's own data, and the shader is
    /// handed the scale in its frame block. Overridden because the
    /// provided body would forward a clip, not the bracket.
    #[cfg(feature = "shader")]
    fn shader(&mut self, rect: Rect, id: ShaderId, uniforms: &[u8]) {
        let r = self.r(rect);
        self.inner.shader(r, id, uniforms);
    }
    #[cfg(feature = "shader")]
    fn shader_end(&mut self) {
        self.inner.shader_end();
    }
}

/// Collecting into a list is *one* implementation of [`Painter`], not
/// the definition of painting.
#[cfg(feature = "alloc")]
impl Painter for DisplayList {
    fn rect(&mut self, rect: Rect, color: Color) {
        DisplayList::rect(self, rect, color)
    }
    fn text_spaced(&mut self, rect: Rect, s: &str, size: f32, weight: f32, line_height: u8, color: Color) {
        DisplayList::text_spaced(self, rect, s, size, weight, line_height, color)
    }
    fn rounded_rect(&mut self, rect: Rect, color: Color, radius: f32) {
        DisplayList::rounded_rect(self, rect, color, radius)
    }
    fn stroked_rect(&mut self, rect: Rect, color: Color, radius: f32, width: f32) {
        DisplayList::stroked_rect(self, rect, color, radius, width)
    }
    fn shadow(&mut self, rect: Rect, radius: f32, spread: f32) {
        DisplayList::shadow(self, rect, radius, spread)
    }
    fn line(&mut self, a: Point, b: Point, width: f32, color: Color) {
        DisplayList::line(self, a, b, width, color)
    }
    fn gradient(&mut self, rect: Rect, stops: Stops, shape: GradShape, corner: f32) {
        DisplayList::gradient(self, rect, stops, shape, corner)
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
    fn fills(&self) -> &[crate::style::Fill] {
        &self.fills
    }
    fn path(&mut self, els: &[PathEl], xf: Affine, paint: PathPaint) {
        DisplayList::path(self, els, xf, paint)
    }
    #[cfg(feature = "shader")]
    fn shader(&mut self, rect: Rect, id: ShaderId, uniforms: &[u8]) {
        DisplayList::shader(self, rect, id, uniforms)
    }
    #[cfg(feature = "shader")]
    fn shader_end(&mut self) {
        DisplayList::shader_end(self)
    }
}
