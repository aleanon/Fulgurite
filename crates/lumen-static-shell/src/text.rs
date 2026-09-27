//! A real text stack for the software renderer.
//!
//! `BlockMetrics` — the core's stand-in — reports `char_count × advance`
//! and the renderer draws one filled square per glyph. That is enough to
//! prove the pipeline end to end without a font, which is why it exists,
//! and it is why a window showed a row of black boxes rather than a
//! counter.
//!
//! This is the same seam M7's `lumen-text` adoption will use: a
//! [`TextMeasure`] for layout and a rasterizer for paint, both behind
//! traits the widgets never name. Feature-gated (`truetype`), so the
//! headless oracle and every test still run on the stand-in — a
//! deterministic measurer is what makes golden geometry assertions
//! possible at all.

use std::borrow::Cow;
use fontdue::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle};
use fontdue::Font;
use lumen_static::cx::{Cluster, GlyphSink, TextMeasure};
use lumen_static::Size;

/// A loaded font, shared by the measurer and the rasterizer so the two
/// cannot disagree about advances.
///
/// # Fallback faces — M45/T45.3
///
/// Two primary faces (regular and bold), and after them a list of
/// **fallback** faces tried in order for a character neither primary
/// has: CJK, Hebrew, Arabic, Devanagari, symbols, monochrome emoji. The
/// list is paths, and a face is read and parsed the first time a run
/// needs a character no earlier face has — never at start-up, since a
/// CJK face is tens of megabytes.
///
/// **`?` remains only for a character no face in the stack has.** The
/// tofu doctrine (Lumen's `verifying-apps` skill) is "not a box if any
/// face has it", and until this stack had one face, "any face" meant the
/// one it had: `你好` rendered `??` on a machine with two CJK faces
/// installed (R10-F12).
///
/// **Colour emoji stays `?`.** fontdue rasterises outlines, and a colour
/// emoji face is bitmaps (CBDT) or layered colour glyphs (COLR), which it
/// cannot draw; a monochrome emoji face is in the list and used when one
/// is installed.
///
/// A choice is by list order and nothing else — the first face that has
/// the character, loaded or not — so what a run measures does not depend
/// on which faces an earlier run happened to load.
#[derive(Clone)]
pub struct FontStack {
    regular: std::sync::Arc<Font>,
    /// A bold face when one was found; otherwise `regular` serves both
    /// weights and the request is quietly honoured as best it can be.
    bold: Option<std::sync::Arc<Font>>,
    /// The fallback faces, shared by every clone — so the measurer and
    /// the canvas that hold clones of one stack load each face once.
    fallback: std::sync::Arc<[Slot]>,
}

/// One fallback face: where it is, and the face once something needed it.
struct Slot {
    path: &'static str,
    /// The face's index in a collection (`.ttc`); 0 for a single face.
    index: u32,
    face: std::sync::OnceLock<Option<std::sync::Arc<Font>>>,
}

impl Slot {
    /// The face, read and parsed on first use. `None` when the file is
    /// absent or not a font, which is remembered, so a missing path costs
    /// one failed `read`.
    fn get(&self) -> Option<&Font> {
        self.face
            .get_or_init(|| {
                let bytes = std::fs::read(self.path).ok()?;
                let settings = fontdue::FontSettings { collection_index: self.index, ..Default::default() };
                Font::from_bytes(bytes, settings).ok().map(std::sync::Arc::new)
            })
            .as_deref()
    }
}

/// Where to look for a sans-serif face, in order: (regular, bold).
const CANDIDATES: &[(&str, &str)] = &[
    (
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
    ),
    (
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    ),
    ("/usr/share/fonts/TTF/DejaVuSans.ttf", "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf"),
    ("/System/Library/Fonts/Helvetica.ttc", "/System/Library/Fonts/Helvetica.ttc"),
    ("C:\\Windows\\Fonts\\segoeui.ttf", "C:\\Windows\\Fonts\\segoeuib.ttf"),
];

/// The fallback faces [`FontStack::system`] attaches, tried in this order
/// for a character the primary faces lack: (path, collection index).
///
/// Broad Latin-plus faces first (DejaVu covers Hebrew, Arabic, Greek and
/// Cyrillic), then script faces, then the CJK faces — Droid's is a
/// quarter the size of Noto's and is read first for that reason.
const FALLBACKS: &[(&str, u32)] = &[
    ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 0),
    ("/usr/share/fonts/TTF/DejaVuSans.ttf", 0),
    ("/usr/share/fonts/truetype/noto/NotoSansHebrew-Regular.ttf", 0),
    ("/usr/share/fonts/truetype/noto/NotoSansArabic-Regular.ttf", 0),
    ("/usr/share/fonts/truetype/noto/NotoSansDevanagari-Regular.ttf", 0),
    ("/usr/share/fonts/truetype/noto/NotoSansThai-Regular.ttf", 0),
    ("/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf", 0),
    ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 0),
    ("/usr/share/fonts/truetype/wqy/wqy-microhei.ttc", 0),
    ("/usr/share/fonts/truetype/noto/NotoSansSymbols-Regular.ttf", 0),
    ("/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf", 0),
    ("/usr/share/fonts/truetype/noto/NotoSansMath-Regular.ttf", 0),
    ("/usr/share/fonts/truetype/noto/NotoEmoji-Regular.ttf", 0),
    ("/System/Library/Fonts/Supplemental/Arial Unicode.ttf", 0),
    ("/System/Library/Fonts/PingFang.ttc", 0),
    ("C:\\Windows\\Fonts\\seguisym.ttf", 0),
    ("C:\\Windows\\Fonts\\msyh.ttc", 0),
];

/// Which faces a run is laid out with, as [`FontStack::prepare`] chose.
enum Faces<'a> {
    /// One face has every character — the steady state for Latin, and
    /// allocation-free.
    One(&'a Font, u8),
    /// Several: each face with its id, and the run's segments as (index
    /// into the faces, byte range of the laid-out text). Boxed, so the
    /// steady state's `prepare` returns 16 bytes of this rather than 48.
    Many(Box<Segments<'a>>),
}

/// A multi-face run's faces and segments — [`Faces::Many`].
struct Segments<'a> {
    faces: Vec<(&'a Font, u8)>,
    segs: Vec<(usize, core::ops::Range<usize>)>,
}

impl Faces<'_> {
    /// The face a glyph's `font_index` names, and its id.
    fn get(&self, i: usize) -> (&Font, u8) {
        match self {
            Faces::One(f, id) => (f, *id),
            Faces::Many(m) => m.faces[i],
        }
    }

    /// The face the run's line metrics and cap shift come from.
    fn first(&self) -> &Font {
        self.get(0).0
    }
}

/// A face's [`GlyphId::face`]: 0 regular, 1 bold, 2 + k fallback k.
const REGULAR: u8 = 0;
const BOLD: u8 = 1;

impl FontStack {
    /// Load a regular face, and optionally a bold one, from bytes. No
    /// fallback faces: a stack built from bytes draws exactly what it was
    /// given, which is what a test with a committed face wants.
    pub fn from_bytes(regular: &[u8], bold: Option<&[u8]>) -> Option<Self> {
        let load = |b: &[u8]| {
            Font::from_bytes(b, fontdue::FontSettings::default()).ok().map(std::sync::Arc::new)
        };
        Some(FontStack { regular: load(regular)?, bold: bold.and_then(load), fallback: std::sync::Arc::new([]) })
    }

    /// The same stack, with `faces` — (path, collection index) — tried in
    /// order for a character the primary faces lack. Each is read the
    /// first time a run needs it.
    pub fn with_fallbacks(mut self, faces: &[(&'static str, u32)]) -> Self {
        self.fallback = faces
            .iter()
            .map(|&(path, index)| Slot { path, index, face: std::sync::OnceLock::new() })
            .collect();
        self
    }

    /// The face for a CSS-style weight: 600 and up is bold.
    fn face(&self, weight: f32) -> (&Font, u8) {
        match (&self.bold, weight >= 600.0) {
            (Some(b), true) => (b, BOLD),
            _ => (&self.regular, REGULAR),
        }
    }

    /// Find a system sans-serif face, with the platform's fallback faces
    /// behind it (this module's `FALLBACKS` list).
    ///
    /// Returns `None` rather than panicking: a caller can fall back to
    /// the block stand-in and still show *something*, which is better
    /// than a window that fails to open on a machine with unusual font
    /// paths.
    pub fn system() -> Option<Self> {
        CANDIDATES
            .iter()
            .find_map(|(r, b)| {
                let regular = std::fs::read(r).ok()?;
                let bold = std::fs::read(b).ok();
                Self::from_bytes(&regular, bold.as_deref())
            })
            .map(|s| s.with_fallbacks(FALLBACKS))
    }

    /// Which face draws `ch` at `weight`: 0 for a primary face, `1 + k`
    /// for fallback `k`, `None` when no face in the stack has it and it is
    /// substituted. Loads fallback faces as laying a run out would.
    ///
    /// For a test that prints where each character came from
    /// (`tests/font_fallback.rs`), and the answer to "why is this a `?`".
    pub fn face_of(&self, ch: char, weight: f32) -> Option<usize> {
        let (wanted, _) = self.face(weight);
        if ch.is_whitespace() || wanted.lookup_glyph_index(ch) != 0 || self.other(weight).0.lookup_glyph_index(ch) != 0 {
            return Some(0);
        }
        self.fallback_for(ch).map(|(k, _)| k + 1)
    }

    /// The primary face that is not the one `weight` asked for.
    fn other(&self, weight: f32) -> (&Font, u8) {
        match self.face(weight) {
            (_, REGULAR) => self.bold.as_deref().map_or((&*self.regular, REGULAR), |b| (b, BOLD)),
            _ => (&self.regular, REGULAR),
        }
    }

    /// The first fallback face that has `ch`, loading faces in order
    /// until one does.
    fn fallback_for(&self, ch: char) -> Option<(usize, &Font)> {
        self.fallback.iter().enumerate().find_map(|(k, slot)| {
            slot.get().filter(|f| f.lookup_glyph_index(ch) != 0).map(|f| (k, f))
        })
    }

    /// Choose the faces a run is laid out with, and substitute what no
    /// face has.
    ///
    /// Tofu doctrine (Lumen's `verifying-apps` skill): a glyph the
    /// chosen face lacks must not render as a box if any face has it.
    /// Try the requested weight, then the other primary face; then, **per
    /// character**, the fallback faces in order (M45/T45.3); only then
    /// substitute — per character rather than per run, so one missing
    /// symbol does not box the whole label.
    ///
    /// The returned text is borrowed when nothing was substituted, which
    /// is every steady-state call; substitution allocates, and so does a
    /// run that needs more than one face — both are fallback paths.
    fn prepare<'t>(&self, text: &'t str, weight: f32) -> (Faces<'_>, Cow<'t, str>) {
        let (wanted, wanted_id) = self.face(weight);
        let has_all =
            |f: &Font| text.chars().all(|c| c.is_whitespace() || f.lookup_glyph_index(c) != 0);
        // **One scan of the chosen face, not two** — M26/T26.17 Phase
        // D #13. This used to call `has_all(wanted)` to pick a face and
        // then `has_all(font)` again on the very face it had just asked
        // about, so the steady state — the wanted face covers the text —
        // scanned every character twice. Carrying the answer out of the
        // choice costs one `bool` and is worth 5% of a layout that
        // re-measures every row (`docs/results-frame-and-types.md`
        // T26.17 finding 13).
        if has_all(wanted) {
            return (Faces::One(wanted, wanted_id), Cow::Borrowed(text));
        }
        let (other, other_id) = self.other(weight);
        if has_all(other) {
            return (Faces::One(other, other_id), Cow::Borrowed(text));
        }
        // Per character: the face each one is drawn from, as an index
        // into `faces`, and a substitute where no face has it.
        let mut faces: Vec<(&Font, u8)> = vec![(wanted, wanted_id)];
        let mut segs: Vec<(usize, core::ops::Range<usize>)> = Vec::new();
        let mut out = String::with_capacity(text.len());
        let mut substituted = false;
        for c in text.chars() {
            let (fi, ch) = if c.is_whitespace() || wanted.lookup_glyph_index(c) != 0 {
                (0, c)
            } else if let Some((k, f)) = self.fallback_for(c) {
                let id = 2 + k as u8;
                let fi = faces.iter().position(|(_, i)| *i == id).unwrap_or_else(|| {
                    faces.push((f, id));
                    faces.len() - 1
                });
                (fi, c)
            } else {
                substituted = true;
                let sub = match c {
                    '\u{2212}' | '\u{2013}' | '\u{2014}' => '-',
                    '\u{2018}' | '\u{2019}' => '\'',
                    '\u{201c}' | '\u{201d}' => '"',
                    _ => '?',
                };
                (0, sub)
            };
            let at = out.len();
            out.push(ch);
            match segs.last_mut() {
                Some((f, r)) if *f == fi => r.end = out.len(),
                _ => segs.push((fi, at..out.len())),
            }
        }
        if faces.len() == 1 {
            // No fallback face drew anything: the old single-face path,
            // substitutions and all.
            let text = if substituted { Cow::Owned(out) } else { Cow::Borrowed(text) };
            return (Faces::One(wanted, wanted_id), text);
        }
        let text = if substituted { Cow::Owned(out) } else { Cow::Borrowed(text) };
        (Faces::Many(Box::new(Segments { faces, segs })), text)
    }

    /// Run fontdue's line layout for one styled run, one segment per
    /// face.
    ///
    /// No per-glyph user data: fontdue reports a glyph's byte offset
    /// within the segment it was appended in, and carrying the segment's
    /// start on every glyph cost the steady state 3% of a label's layout
    /// for a path only a multi-face run takes (`docs/results-text.md`
    /// T45.3). `shape_into` recovers the caller's offsets for that path
    /// instead.
    ///
    /// **Into a layout the caller keeps** — M50/T50.2, R8-F1. This built
    /// a fresh fontdue `Layout` per run, whose glyph, output and line
    /// `Vec`s then grew from empty on every call: allocations on every
    /// frame that drew or measured text. `reset` clears them and keeps
    /// their capacity, so a layout that has laid out a run this long
    /// before lays this one out without touching the allocator. The
    /// multi-face fallback path still collects its face list, which is a
    /// run no single face covers.
    fn run(faces: &Faces<'_>, text: &str, px: f32, max_w: f32, layout: &mut Layout) {
        layout.reset(&LayoutSettings {
            max_width: if max_w.is_finite() && max_w > 0.0 { Some(max_w) } else { None },
            ..LayoutSettings::default()
        });
        match faces {
            Faces::One(f, _) => layout.append(&[*f], &TextStyle::new(text, px, 0)),
            Faces::Many(m) => {
                let fonts: Vec<&Font> = m.faces.iter().map(|(f, _)| *f).collect();
                for (fi, r) in &m.segs {
                    layout.append(&fonts, &TextStyle::new(&text[r.clone()], px, *fi));
                }
            }
        }
    }

    /// A laid-out run's box: per line, the larger of **where the pen
    /// ends** and where the ink ends — M45/T45.1, R10-F1.
    ///
    /// fontdue's glyph `x` is the *ink* origin — the pen plus the glyph's
    /// left bearing, floored — and this used to take `x + advance` as the
    /// right edge. A glyph with a negative bearing (`j`) put its ink
    /// origin left of its pen, so a run ending in one measured a pixel or
    /// two short, and the painter, which wraps inside the measured box,
    /// moved the `j` to a second line below it: 600 of 1,200 cases in
    /// `tests/text_containment.rs` (`docs/results-text.md` T45.0).
    ///
    /// The pen is rebuilt the way fontdue advances it — from each line's
    /// start, by `ceil(advance)`, nothing for a control character — so
    /// the width is exactly the space fontdue's own wrap needs the run to
    /// have. The ink term keeps an overhanging glyph (an italic tail, a
    /// wide `W`) inside the box as well.
    fn extent(faces: &Faces<'_>, layout: &Layout, px: f32) -> Size {
        // Monomorphised per case, so the one-face steady state asks no
        // question per glyph about which face drew it.
        match faces {
            Faces::One(f, _) => Self::extent_by(|_| f, layout, px),
            Faces::Many(m) => Self::extent_by(|i| m.faces[i].0, layout, px),
        }
    }

    /// [`FontStack::extent`], with the face for a glyph's `font_index`.
    fn extent_by<'f>(face: impl Fn(usize) -> &'f Font, layout: &Layout, px: f32) -> Size {
        let glyphs = layout.glyphs();
        let (mut w, mut h) = (0.0f32, 0.0f32);
        let mut line = |range: core::ops::Range<usize>| {
            let mut pen = 0.0f32;
            for g in glyphs.get(range).unwrap_or(&[]) {
                if !g.char_data.is_control() {
                    pen += face(g.font_index).metrics_indexed(g.key.glyph_index, px).advance_width.ceil();
                }
                w = w.max(pen).max(g.x + g.width as f32);
                h = h.max(g.y + g.height as f32);
            }
        };
        match layout.lines() {
            Some(lines) => {
                for l in lines {
                    line(l.glyph_start..(l.glyph_end + 1).max(l.glyph_start));
                }
            }
            None => line(0..glyphs.len()),
        }
        // Ceiled: `layout` sizes the box from this.
        Size { w: w.ceil(), h: h.max(layout.height()).ceil() }
    }

    /// The width the painter lays a run out in, given the rect it was
    /// handed — M45/T45.1.
    ///
    /// **A run that measured as one line is never re-wrapped.** The rect
    /// is the measured box, but it reaches here as the difference of its
    /// two edges, each a logical position times the display scale, and
    /// off the pixel grid that difference rounds *down* in `f32` as often
    /// as not — a whole-pixel run then wraps its last glyph (92 of 1,200
    /// cases without this, `docs/results-text.md` T45.1). Half a pixel
    /// absorbs the round-down, and is less
    /// than any glyph's advance, so a run that genuinely does not fit
    /// still wraps where the measurer said it would.
    fn paint_width(max_w: f32) -> f32 {
        if max_w.is_finite() {
            max_w + 0.5
        } else {
            max_w
        }
    }

    /// Lay out one run and report where each inked glyph goes, **without
    /// rasterising any of them**.
    ///
    /// The shaping half of the software canvas's text path, split out for
    /// `lumen-static-wgpu`: a GPU backend shapes every run every frame but
    /// rasterises a glyph only the first time it is seen, because the
    /// pixels live in an atlas on the device. The software canvas does the
    /// same since M50/T50.2, with its coverage cache in host memory.
    ///
    /// **Both paths shape here**, which is the point. The canvas calls
    /// [`FontStack::place_run_into`], of which this is the allocating
    /// wrapper, so the CPU and GPU renderers cannot disagree about where a
    /// glyph goes — and "the two backends are pixel-comparable" is a claim
    /// about rasterisation only, which is the claim `tests/parity.rs` in
    /// that crate can actually check.
    ///
    /// **Allocates**: a fresh [`ShapeScratch`] and the returned `Vec`. A
    /// caller that shapes every frame keeps a scratch and calls
    /// [`FontStack::place_run_into`] (M51/T51.4's GPU path).
    pub fn place_run(&self, text: &str, px: f32, weight: f32, max_w: f32) -> (Vec<Placed>, Size) {
        let mut scratch = ShapeScratch::new();
        let size = self.place_run_into(&mut scratch, text, px, weight, max_w);
        (scratch.placed, size)
    }

    /// [`FontStack::place_run`] into a scratch the caller keeps — M50/T50.2.
    ///
    /// The placements are in [`ShapeScratch::placed`] until the next call.
    /// **Allocation-free once the scratch is warm** — once its layout and
    /// placement buffer have held a run this long and its cap-shift table
    /// has seen this face and size — on the steady-state path, a run one
    /// primary face covers with nothing substituted. A run that needs a
    /// fallback face or a substitute allocates for that, as
    /// `FontStack::prepare` says.
    pub fn place_run_into(&self, scratch: &mut ShapeScratch, text: &str, px: f32, weight: f32, max_w: f32) -> Size {
        let (faces, prepared) = self.prepare(text, weight);
        let text: &str = &prepared;
        // From the run's first face, for every glyph: one baseline, one
        // shift, however many faces drew it. Looked up before the run is
        // laid out, because a miss lays out a probe of its own.
        let shift = scratch.caps.shift(faces.get(0).1, faces.first(), px);
        Self::run(&faces, text, px, Self::paint_width(max_w), &mut scratch.layout);
        SHAPED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);

        let layout = &scratch.layout;
        let out = &mut scratch.placed;
        out.clear();
        for g in layout.glyphs() {
            if g.width == 0 || g.height == 0 {
                continue;
            }
            // Which face `prepare` actually chose — not which one the
            // weight asked for. A bold run in a stack with no bold face is
            // rasterised from `regular`, and an id that recorded the
            // request rather than the outcome would collide with the
            // regular run's glyphs at the same size while naming a
            // different face.
            let face = faces.get(g.font_index).1;
            out.push(Placed {
                id: GlyphId { index: g.key.glyph_index, px_bits: px.to_bits(), face },
                x: g.x,
                y: g.y + shift,
                w: g.width as u32,
                h: g.height as u32,
            });
        }
        PLACED.fetch_add(out.len() as u64, core::sync::atomic::Ordering::Relaxed);
        // The box by the same rule `measure_run` uses (`extent`), so the
        // two cannot disagree.
        Self::extent(&faces, layout, px)
    }

    /// The coverage mask for one glyph identity: `(width, height, cov)`.
    ///
    /// **Allocates**, twice per call: fontdue's accumulator and the
    /// coverage `Vec` it returns. So nothing calls it per frame: the
    /// software canvas keeps what it returns in a coverage cache keyed on
    /// the id (M50/T50.2) and the GPU backend in its atlas, and each asks
    /// again only for a glyph it has not seen. Until T50.2 the canvas
    /// called it for every glyph of every drawn run on every frame, which
    /// was most of the 25.80 allocations a frame `docs/results-allocation.md`
    /// records; its time against `vello_cpu`, which cached across frames
    /// already, is `docs/results-vello.md` §4.
    ///
    /// `None` when the id names a face this stack does not have, which
    /// cannot happen for an id this stack produced and is a "draw
    /// nothing" for one that came from somewhere else — the same rule
    /// [`lumen_static::ImageSource`] follows for a missing asset.
    ///
    /// Deterministic in the id alone: fontdue's raster key is
    /// `(glyph index, px)` with no subpixel phase, so equal ids give
    /// byte-identical bitmaps. That is what makes an atlas cache
    /// correct rather than approximately correct.
    pub fn rasterize(&self, id: GlyphId) -> Option<(u32, u32, Vec<u8>)> {
        let font = match (id.face, &self.bold) {
            (REGULAR, _) => self.regular.as_ref(),
            (BOLD, Some(b)) => b.as_ref(),
            (BOLD, None) => return None,
            (k, _) => self.fallback.get(k as usize - 2)?.get()?,
        };
        let (m, cov) = font.rasterize_indexed(id.index, f32::from_bits(id.px_bits));
        RASTERISED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        Some((m.width as u32, m.height as u32, cov))
    }

    /// Lay out one run and return each glyph's raster and position.
    ///
    /// [`FontStack::place_run`] followed by [`FontStack::rasterize`] per
    /// glyph, **allocating both**. Only a band worker of the threaded
    /// rasteriser still calls it, because a worker cannot borrow the
    /// canvas's cache mutably; M50/T50.3 shapes on the frame thread and
    /// removes it.
    #[cfg(feature = "threads")]
    pub(crate) fn glyphs(
        &self,
        text: &str,
        px: f32,
        weight: f32,
        max_w: f32,
    ) -> (Vec<Glyph>, Size) {
        let (placed, size) = self.place_run(text, px, weight, max_w);
        let out = placed
            .into_iter()
            .filter_map(|p| {
                let (w, h, cov) = self.rasterize(p.id)?;
                Some(Glyph { x: p.x, y: p.y, w: w as usize, h: h as usize, cov })
            })
            .collect();
        (out, size)
    }

    /// The size of a run, **without rasterising it**.
    ///
    /// `measure` used to call [`Self::glyphs`] and keep only its `Size`,
    /// throwing away a `Vec` of freshly rasterised coverage bitmaps —
    /// so *measuring* text rasterised it, and layout paid for pixels no
    /// one looked at. fontdue has no glyph cache, so that was a real
    /// raster per glyph per measure.
    ///
    /// It cost **2.9 µs a label**, which at 3000 labels is 8.7 ms of a
    /// frame; `docs/results-competitive.md` has the before and after.
    /// The box is `extent`'s, which `place_run` uses too, so the two
    /// cannot disagree about a box — which they must not, because
    /// `layout` sizes from this and `paint` lays the run out inside it.
    ///
    /// Into the caller's `layout`, which [`FontMetrics`] keeps, so a
    /// measure allocates nothing once that layout is warm (M50/T50.2).
    pub(crate) fn measure_run(&self, text: &str, px: f32, weight: f32, max_w: f32, layout: &mut Layout) -> Size {
        let (faces, prepared) = self.prepare(text, weight);
        let text: &str = &prepared;
        Self::run(&faces, text, px, max_w, layout);
        MEASURED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        Self::extent(&faces, layout, px)
    }

    /// Shape one run and report every cluster boundary to `sink`.
    ///
    /// No rasterization: this is the query a caret, a selection or a
    /// click-to-offset makes, and it must not cost a glyph cache. The
    /// trailing boundary (an insertion point after the last character)
    /// is reported too, so a run of `n` characters yields `n + 1`
    /// clusters — the caret positions a text field actually has.
    ///
    /// Byte offsets are into the *caller's* string. When a glyph was
    /// substituted the laid-out string differs from it, and since
    /// substitution is one char for one char the offsets are mapped back
    /// through the character correspondence rather than reported from
    /// the substituted copy — `\u{2212}` is three bytes and `-` is one,
    /// so reporting the copy's offsets would put a caret mid-character.
    pub(crate) fn shape_into(
        &self,
        text: &str,
        px: f32,
        weight: f32,
        max_w: f32,
        sink: &mut dyn GlyphSink,
        layout: &mut Layout,
    ) -> Size {
        let (faces, prepared) = self.prepare(text, weight);
        let laid: &str = &prepared;
        Self::run(&faces, laid, px, max_w, layout);
        let layout = &*layout;
        MEASURED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        // **Offsets in the caller's string, by walking it in step** —
        // M45/T45.10, R16-F7. fontdue emits one glyph per character of
        // the laid-out text, and substitution and a face split are both
        // one character for one, so glyph `n`'s offset is the caller's
        // `n`th character's. Two cases need it: a substituted run, whose
        // laid-out copy has different byte lengths (`\u{2212}` is three,
        // `-` one, so the copy's offsets would put a caret mid-character),
        // and a multi-face run, which fontdue counts from each segment's
        // start. This used to find each glyph's character with a
        // `position` and an `nth` over the whole text, glyphs × chars:
        // one substituted glyph anywhere made a 20 KB keystroke cost 94
        // times a 2 KB one (`docs/results-text.md` T45.10). A borrowed,
        // one-face run keeps fontdue's own offset, which is already the
        // caller's.
        let step = matches!(prepared, Cow::Owned(_)) || matches!(faces, Faces::Many(..));
        let mut caller = text.char_indices().map(|(i, _)| i);

        // **A cursor, not a search** — M45/T45.10, R16-F7. Lines and
        // glyphs are both in order, so a glyph's line is the current one
        // or a later one. This searched every line for every glyph, glyphs
        // × lines, and a keystroke in a 200 KB area cost 60 times one in
        // 20 KB (`docs/results-text.md` T45.10). An empty line's range is
        // backwards (its end before its start) and is stepped over; a
        // glyph in no line reports line 0, as the search did.
        let lines = layout.lines().map_or(&[][..], |l| l.as_slice());
        let mut li = 0usize;
        let mut line_of = |gi: usize| -> u16 {
            while li < lines.len() && gi > lines[li].glyph_end {
                li += 1;
            }
            match lines.get(li) {
                Some(l) if gi >= l.glyph_start => li as u16,
                _ => 0,
            }
        };

        let mut end = (0.0f32, 0u16);
        for (gi, g) in layout.glyphs().iter().enumerate() {
            let adv = faces.get(g.font_index).0.metrics_indexed(g.key.glyph_index, px).advance_width;
            let line = line_of(gi);
            if !sink.is_full() {
                let byte = if step { caller.next().unwrap_or(text.len()) } else { g.byte_offset };
                sink.cluster(Cluster { byte: byte as u32, x: g.x, line });
            } else if step {
                caller.next();
            }
            end = (g.x + adv, line);
        }
        // The insertion point after the last character.
        if !sink.is_full() {
            sink.cluster(Cluster { byte: text.len() as u32, x: end.0, line: end.1 });
        }
        Self::extent(&faces, layout, px)
    }
}

/// Every glyph [`FontStack::rasterize`] has produced in this process.
static RASTERISED: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// How many glyphs this process has rasterised — a counter for a test
/// that bounds what a frame draws by what is visible
/// (`docs/plan-text-correctness.md` T45.0 item 14, R8-F3).
///
/// A count and not a clock, so it answers the same under load. One
/// relaxed add per rasterised glyph, beside a rasterisation that costs
/// microseconds. Process-wide: a test reads it before and after the
/// frame it bounds, alone in its binary.
pub fn glyphs_rasterised() -> u64 {
    RASTERISED.load(core::sync::atomic::Ordering::Relaxed)
}

/// Runs laid out for painting, in this process — [`text_counters`].
static SHAPED: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
/// Runs laid out for a measure or a shape query — [`text_counters`].
static MEASURED: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
/// Glyphs laid out by paint-side shaping — [`text_counters`].
static PLACED: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
/// Glyphs the software canvas blitted — [`text_counters`].
static DRAWN: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// What the text stack has done in this process, as counts — M50/T50.0.
///
/// For a test that must assert its own setup before it reads a quantity
/// (`docs/plan-raster-allocation.md` §0 rule 3): that a frame shaped a
/// run, measured one, drew glyphs. Counts and not clocks, so they answer
/// the same under load. One relaxed add per run or per glyph, beside
/// work that costs microseconds — the same trade [`glyphs_rasterised`]
/// makes. Process-wide: a test reads them before and after the frames it
/// bounds, one test at a time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextCounters {
    /// Runs laid out to be painted (`place_run` and the canvas's own
    /// shaping).
    pub shaped: u64,
    /// Inked glyphs those runs placed — what a frame shaped, whether or
    /// not any of it was drawn. The quantity a long run inflates.
    pub placed: u64,
    /// Runs laid out to be measured or shaped for clusters.
    pub measured: u64,
    /// Glyph coverage masks produced by fontdue ([`glyphs_rasterised`]):
    /// cache misses, since M50/T50.2.
    pub rasterised: u64,
    /// Glyphs the software canvas blitted.
    pub drawn: u64,
}

/// The process's [`TextCounters`], now.
pub fn text_counters() -> TextCounters {
    use core::sync::atomic::Ordering::Relaxed;
    TextCounters {
        shaped: SHAPED.load(Relaxed),
        placed: PLACED.load(Relaxed),
        measured: MEASURED.load(Relaxed),
        rasterised: RASTERISED.load(Relaxed),
        drawn: DRAWN.load(Relaxed),
    }
}

/// Count `n` glyphs blitted — the software canvas's call.
pub(crate) fn count_drawn(n: u64) {
    DRAWN.fetch_add(n, core::sync::atomic::Ordering::Relaxed);
}

/// Divides cluster positions back to logical units on the way out.
struct LogicalClusters<'a> {
    inner: &'a mut dyn GlyphSink,
    scale: f32,
}

impl GlyphSink for LogicalClusters<'_> {
    fn cluster(&mut self, c: Cluster) {
        self.inner.cluster(Cluster { x: c.x / self.scale, ..c });
    }
    fn is_full(&self) -> bool {
        self.inner.is_full()
    }
}

/// How far to move a run so its capitals are centred in its line box.
///
/// The line box is what gets centred in a button, and its middle is not
/// where the eye puts the middle of a word: capitals and digits hang
/// from the ascent, and Noto's ascent is 1.07 em against a 0.71 em cap,
/// so line-box-centred text reads ~2 px low at 16 px and 6 px low at
/// 76 px. This is the "vertical trim" design tools apply. Measured from
/// where the layout actually puts an `H` — its rasterised rows, not the
/// face's cap-height figure — because the bitmap can sit a pixel below
/// the metric baseline, and `tests/glyph_placement.rs` checks ink, not
/// metrics. Descenders still hang below, as they should.
fn cap_shift(font: &Font, px: f32) -> f32 {
    let Some(line) = font.horizontal_line_metrics(px).map(|m| m.new_line_size) else {
        return 0.0;
    };
    let mut probe = Layout::new(CoordinateSystem::PositiveYDown);
    probe.reset(&LayoutSettings::default());
    probe.append(&[font], &TextStyle::new("H", px, 0));
    match probe.glyphs().first() {
        Some(h) if h.height > 0 => (line / 2.0 - (h.y + h.height as f32 / 2.0)).round(),
        _ => 0.0,
    }
}

/// Reusable shaping state for [`FontStack::place_run_into`] — M50/T50.2.
///
/// One fontdue layout, one placement buffer and a cap-shift table. Owned
/// by whoever shapes every frame — the software canvas, and M51's GPU
/// path — so that [`FontStack`] stays immutable and `Sync`, which the
/// banded rasteriser needs (`docs/plan-raster-allocation.md` §2.1: a
/// `RefCell` inside the stack would make it `!Sync`, and a `Mutex` would
/// put a lock on every glyph of every worker).
pub struct ShapeScratch {
    layout: Layout,
    placed: Vec<Placed>,
    caps: CapShifts,
}

impl Default for ShapeScratch {
    fn default() -> Self {
        Self::new()
    }
}

impl ShapeScratch {
    /// Empty; it grows to the longest run it is asked to shape and keeps
    /// that capacity.
    pub fn new() -> Self {
        ShapeScratch { layout: Layout::new(CoordinateSystem::PositiveYDown), placed: Vec::new(), caps: CapShifts::default() }
    }

    /// The last run's placements.
    pub fn placed(&self) -> &[Placed] {
        &self.placed
    }
}

/// [`cap_shift`]'s answers, per (face, size) — M50/T50.2.
///
/// `cap_shift` lays out a probe `H` to find where the layout puts it,
/// which is a second fontdue layout per run, and it depends only on the
/// face and the size. A fixed table rather than a map, so a lookup never
/// allocates: a screen uses a handful of (face, size) pairs, and a
/// thirty-third evicts round-robin and costs one probe when it returns.
struct CapShifts {
    keys: [(u8, u32); CapShifts::N],
    shift: [f32; CapShifts::N],
    len: usize,
    next: usize,
}

impl Default for CapShifts {
    fn default() -> Self {
        CapShifts { keys: [(0, 0); CapShifts::N], shift: [0.0; CapShifts::N], len: 0, next: 0 }
    }
}

impl CapShifts {
    const N: usize = 32;

    fn shift(&mut self, face: u8, font: &Font, px: f32) -> f32 {
        let key = (face, px.to_bits());
        if let Some(i) = self.keys[..self.len].iter().position(|k| *k == key) {
            return self.shift[i];
        }
        let v = cap_shift(font, px);
        let i = if self.len < Self::N {
            self.len += 1;
            self.len - 1
        } else {
            let i = self.next;
            self.next = (self.next + 1) % Self::N;
            i
        };
        self.keys[i] = key;
        self.shift[i] = v;
        v
    }
}

/// What a software canvas's glyph cache has done — M50/T50.2
/// ([`crate::software::Canvas::glyph_cache_stats`]).
///
/// `uncached` is the number `docs/plan-raster-allocation.md` §5 names as
/// the risk to watch: a glyph drawn straight from fontdue because it did
/// not fit, which allocates. It must read 0 after warm-up on a real
/// screen; `just grid` and showcase's headless frame print it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheStats {
    /// Lookups answered from the cache.
    pub hits: u64,
    /// Lookups that rasterised a glyph and kept it.
    pub misses: u64,
    /// Glyphs rasterised and drawn without being kept: larger than the
    /// arena's per-glyph limit. Allocates, and is never skipped.
    pub uncached: u64,
    /// Times the cache emptied itself to make room.
    pub resets: u64,
}

/// The software canvas's coverage cache — M50/T50.2, R8-F1.
///
/// Keyed on [`GlyphId`], whose contract is that equal ids are
/// byte-identical bitmaps (no subpixel phase), so a hit is exact rather
/// than approximate. One arena of coverage bytes and an index, both sized
/// **once**, on first use: a lookup that hits never allocates, and a
/// miss allocates only inside fontdue.
///
/// **Bounded, and a miss never drops a glyph** (plan §2.1). When the
/// arena or the index is full the cache empties itself and starts over,
/// which costs re-rasterising what the next frames draw and nothing
/// else. A glyph too large to keep is rasterised and drawn anyway, and
/// counted as `uncached`: a cache that dropped glyphs to keep a zero
/// would be the GPU atlas's defect (M51, R9-F1), on purpose avoided.
pub(crate) struct GlyphCache {
    index: std::collections::HashMap<GlyphId, (u32, u16, u16), GlyphHash>,
    arena: Vec<u8>,
    stats: CacheStats,
}

impl Default for GlyphCache {
    fn default() -> Self {
        GlyphCache { index: std::collections::HashMap::with_hasher(GlyphHash), arena: Vec::new(), stats: CacheStats::default() }
    }
}

impl GlyphCache {
    /// Glyphs the index holds before it resets.
    const ENTRIES: usize = 4096;
    /// Coverage bytes the arena holds before it resets: at 16 px a Latin
    /// glyph is about 150 bytes, so this is several thousand glyphs at
    /// body sizes and still dozens at display sizes.
    const BYTES: usize = 2 << 20;
    /// A glyph larger than this is drawn without being kept, so one huge
    /// glyph cannot empty the cache for everything else.
    const MAX_GLYPH: usize = Self::BYTES / 8;

    pub(crate) fn stats(&self) -> CacheStats {
        self.stats
    }

    /// Call `draw` with glyph `id`'s coverage and size, from the cache or
    /// rasterised into it. `None` — nothing drawn — only when the stack
    /// cannot rasterise the id at all ([`FontStack::rasterize`]).
    pub(crate) fn with<R>(&mut self, font: &FontStack, id: GlyphId, draw: impl FnOnce(&[u8], usize, usize) -> R) -> Option<R> {
        if let Some(&(off, w, h)) = self.index.get(&id) {
            self.stats.hits += 1;
            let (w, h) = (w as usize, h as usize);
            let off = off as usize;
            return Some(draw(&self.arena[off..off + w * h], w, h));
        }
        let (w, h, cov) = font.rasterize(id)?;
        let (w, h) = (w as usize, h as usize);
        let n = w * h;
        if n > Self::MAX_GLYPH || w > u16::MAX as usize || h > u16::MAX as usize {
            self.stats.uncached += 1;
            return Some(draw(&cov, w, h));
        }
        if self.arena.capacity() == 0 {
            // First use: size both once. Nothing after this grows either.
            self.arena.reserve_exact(Self::BYTES);
            self.index.reserve(Self::ENTRIES);
        }
        if self.arena.len() + n > Self::BYTES || self.index.len() >= Self::ENTRIES {
            self.arena.clear();
            self.index.clear();
            self.stats.resets += 1;
        }
        let off = self.arena.len();
        self.arena.extend_from_slice(&cov);
        self.index.insert(id, (off as u32, w as u16, h as u16));
        self.stats.misses += 1;
        Some(draw(&self.arena[off..off + n], w, h))
    }
}

/// A hasher for [`GlyphId`]: three small integers, so a multiply-and-fold
/// rather than SipHash, whose resistance to chosen keys buys nothing for
/// ids the stack itself produced.
#[derive(Clone, Copy, Default)]
pub(crate) struct GlyphHash;

impl std::hash::BuildHasher for GlyphHash {
    type Hasher = GlyphHasher;
    fn build_hasher(&self) -> GlyphHasher {
        GlyphHasher(0)
    }
}

pub(crate) struct GlyphHasher(u64);

impl std::hash::Hasher for GlyphHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.write_u64(*b as u64);
        }
    }
    fn write_u8(&mut self, v: u8) {
        self.write_u64(v as u64);
    }
    fn write_u16(&mut self, v: u16) {
        self.write_u64(v as u64);
    }
    fn write_u32(&mut self, v: u32) {
        self.write_u64(v as u64);
    }
    fn write_u64(&mut self, v: u64) {
        self.0 = (self.0.rotate_left(5) ^ v).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

/// The identity of one rasterised glyph.
///
/// Opaque: equal ids mean identical coverage bitmaps, and that is the
/// whole contract. It carries no `fontdue` type, so a cache keyed on it
/// — `lumen-static-wgpu`'s atlas — does not inherit this crate's font
/// dependency in its own API.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GlyphId {
    index: u16,
    /// The size in **bits**, because a raster key must be comparable
    /// and `f32` is not `Eq`. Two runs at the same size hash together;
    /// two at sizes a hair apart do not, which is correct — they
    /// rasterise differently.
    px_bits: u32,
    /// Which face: 0 regular, 1 bold, 2 + k the stack's fallback `k`
    /// (M45/T45.3; a `bool` for bold before). A glyph index names a
    /// glyph only within its face.
    face: u8,
}

impl GlyphId {
    /// The glyph's index in its face — for a test that compares what was
    /// painted with what a shaper produced (`docs/plan-text-correctness.md`
    /// T45.0 item 2). Equal indices in the same face and size are the
    /// same bitmap; the index alone does not say which face.
    pub fn index(&self) -> u16 {
        self.index
    }
}

/// Where one inked glyph of a run goes, without its pixels.
#[derive(Clone, Copy, Debug)]
pub struct Placed {
    pub id: GlyphId,
    /// Offset from the run's top-left, in the same units `px` was given
    /// in — physical pixels, since that is what a renderer draws.
    pub x: f32,
    pub y: f32,
    pub w: u32,
    pub h: u32,
}

/// One rasterized glyph: an 8-bit coverage mask and where to put it.
#[cfg(feature = "threads")]
pub(crate) struct Glyph {
    pub x: f32,
    pub y: f32,
    pub w: usize,
    pub h: usize,
    pub cov: Vec<u8>,
}

/// [`TextMeasure`] backed by a real face — what `layout` asks.
pub struct FontMetrics {
    stack: FontStack,
    /// Physical pixels per logical pixel (M10b).
    ///
    /// Layout speaks logical units, but glyphs are rasterised at
    /// physical ones, and a face's advances are **not** linear in size:
    /// hinting and rounding mean measuring at 16 px and doubling is not
    /// measuring at 32 px. So this measures at `size * scale` and
    /// divides the answer back down. Get it wrong and text drifts out of
    /// the box that was laid out for it — visible as a clipped last
    /// glyph, which is exactly how "POSITIVE" once became "POSITIV"
    /// (`docs/results-frame-cost.md`).
    scale: f32,
    /// This measurer's [`TextMeasure::generation`] — M45/T45.2. Fresh
    /// from a process-wide counter on construction and on every
    /// [`FontMetrics::set_stack`], never 0 (the stand-ins' value), so a
    /// memo keyed on it cannot serve one stack's box to another.
    generation: u32,
    /// The layout every measure and shape query lays its run out into —
    /// M50/T50.2. A fresh one per measure grew three `Vec`s from empty
    /// each time: 4.60 allocations a frame on a label that changed every
    /// frame (`docs/results-allocation.md`).
    layout: Layout,
}

/// The next [`FontMetrics`] generation. Starts at 1: 0 is every
/// stand-in's.
static NEXT_GENERATION: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(1);

fn next_generation() -> u32 {
    NEXT_GENERATION.fetch_add(1, core::sync::atomic::Ordering::Relaxed).max(1)
}

impl FontMetrics {
    /// A measurer for a 1:1 display.
    pub fn new(stack: FontStack) -> Self {
        FontMetrics { stack, scale: 1.0, generation: next_generation(), layout: Layout::new(CoordinateSystem::PositiveYDown) }
    }

    /// A measurer that reports logical sizes for a display with this
    /// scale factor.
    pub fn with_scale(stack: FontStack, scale: f32) -> Self {
        let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
        FontMetrics { stack, scale, generation: next_generation(), layout: Layout::new(CoordinateSystem::PositiveYDown) }
    }

    /// Measure with a different font stack from now on. A new
    /// [`TextMeasure::generation`], so every text memo measured with the
    /// old one re-measures (M45/T45.2).
    pub fn set_stack(&mut self, stack: FontStack) {
        self.stack = stack;
        self.generation = next_generation();
    }

    /// Update the scale factor in place — a window can move between
    /// monitors.
    pub fn set_scale(&mut self, scale: f32) {
        if scale.is_finite() && scale > 0.0 {
            self.scale = scale;
        }
    }
}

impl TextMeasure for FontMetrics {
    fn measure(&mut self, text: &str, size: f32, max_w: f32) -> Size {
        self.measure_weighted(text, size, 400.0, max_w)
    }

    fn measure_weighted(&mut self, text: &str, size: f32, weight: f32, max_w: f32) -> Size {
        if text.is_empty() {
            return Size { w: 0.0, h: 0.0 };
        }
        let k = self.scale;
        // `measure_run`, not `glyphs`: measuring must not rasterise.
        let s = self.stack.measure_run(text, size * k, weight, max_w * k, &mut self.layout);
        Size { w: s.w / k, h: s.h / k }
    }

    fn shape(
        &mut self,
        text: &str,
        size: f32,
        weight: f32,
        max_w: f32,
        sink: &mut dyn GlyphSink,
    ) -> Size {
        if text.is_empty() {
            // One cluster: the caret position in an empty field.
            sink.cluster(Cluster { byte: 0, x: 0.0, line: 0 });
            return Size { w: 0.0, h: self.line_height(size) };
        }
        // Clusters are reported in logical x, because a caret is placed
        // by `layout` and `paint`, both of which speak logical.
        let k = self.scale;
        let mut scaled = LogicalClusters { inner: sink, scale: k };
        let s = self.stack.shape_into(text, size * k, weight, max_w * k, &mut scaled, &mut self.layout);
        Size { w: s.w / k, h: s.h / k }
    }

    fn generation(&self) -> u32 {
        self.generation
    }

    fn line_height(&mut self, size: f32) -> f32 {
        let k = self.scale;
        self.stack
            .regular
            .horizontal_line_metrics(size * k)
            .map(|m| m.new_line_size / k)
            .unwrap_or(size * 1.2)
    }
}
