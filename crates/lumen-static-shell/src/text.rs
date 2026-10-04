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
use lumen_static::text::GlyphKey;

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
    ///
    /// **Parsed on first use** — M50/T50.7, R8-F10. The bytes are read
    /// when the stack is built, so a missing file is still found at
    /// start-up, but fontdue's parse — 8.4 MB of heap for Noto Sans Bold,
    /// about half an idle window's (`docs/results-competitive.md` §3) —
    /// waits for the first run of weight 600 or more, or the first
    /// character the regular face lacks. A `OnceLock`, so the stack stays
    /// `Sync` for the banded rasteriser; shared by every clone, so the
    /// canvas and the measurer parse it once between them.
    bold: std::sync::Arc<Bold>,
    /// The fallback faces, shared by every clone — so the measurer and
    /// the canvas that hold clones of one stack load each face once.
    fallback: std::sync::Arc<[Slot]>,
    /// How a run is laid out — M81/T81.3, [`Shaping`]. One byte; every
    /// clone carries it, so the measurer and the renderers that hold
    /// clones of one stack cannot shape differently.
    shaping: Shaping,
    /// Where each face's bytes come from, for the shaper — M81/T81.3.
    /// Read only under `Shaping::Complex`; see `Sources`.
    #[cfg(feature = "shaping")]
    sources: std::sync::Arc<Sources>,
}

/// How a [`FontStack`] lays a run out — M81/T81.3
/// (`docs/plan-shaped-paint.md`, "The shaping mode is a property of the
/// `FontStack`").
///
/// - `Basic`, the default: fontdue's own layout — cmap glyphs in logical
///   order, whole-pixel advances, its own wrap. Byte for byte what every
///   stack did before the mode existed, with the `shaping` feature
///   compiled in or not (`tests/basic_placements.rs`).
/// - `Complex`, only with the shell's `shaping` feature: the
///   `lumen-static-text` shaper — bidirectional order, joining, marks,
///   UAX #14 line breaks — for the measure, the cluster query **and the
///   paint**, since both backends place glyphs through
///   [`FontStack::place_run_into`].
///
/// Chosen per stack, not per run or per widget: the frame's measurer, the
/// agent's audit measurer and both renderers hold clones of one stack.
/// Not yet selectable in a window: `Program::shaping` is T81.4's.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Shaping {
    #[default]
    Basic = 0,
    #[cfg(feature = "shaping")]
    Complex = 1,
}

/// Where a primary face's bytes can be had again — M81/T81.3.
///
/// fontdue keeps no bytes once it has parsed a face, and rustybuzz
/// borrows them for as long as it shapes with it, so a stack that is to
/// shape must be able to get them back: from the file it read
/// ([`FontStack::from_files`], [`FontStack::system`]), or from the
/// caller's own `'static` bytes ([`FontStack::from_static`]). A stack
/// built by [`FontStack::from_bytes`] keeps no way back, and under
/// `Shaping::Complex` lays its runs out as `Basic` does.
#[derive(Clone, Copy)]
#[cfg_attr(not(feature = "shaping"), allow(dead_code))]
enum Origin {
    File(&'static str),
    Static(&'static [u8]),
    Unknown,
}

/// Each face's bytes, for the shaper — M81/T81.3. Shared by every clone
/// (one `Arc`), so the measurer and the canvas ask once between them.
///
/// The bytes are read the first time a `Complex` run needs the face,
/// never under `Basic`: K81c bounds what is kept at the face's file size
/// plus 64 KiB (`tests/font_heap.rs`). A face read from a file is
/// **interned once per process** by (path, collection index), so two
/// stacks over the same face, and a stack built again, keep one copy.
#[cfg(feature = "shaping")]
struct Sources {
    regular: Origin,
    bold: Origin,
    /// Per face id (0 regular, 1 bold, 2 + k fallback `k`).
    bytes: Box<[std::sync::OnceLock<Option<&'static [u8]>>]>,
}

#[cfg(feature = "shaping")]
impl Sources {
    fn new(regular: Origin, bold: Origin, fallbacks: usize) -> Self {
        Sources { regular, bold, bytes: (0..2 + fallbacks).map(|_| std::sync::OnceLock::new()).collect() }
    }
}

/// The process's interned face files, by (path, collection index) —
/// `Sources`. Leaked on purpose: a parsed `rustybuzz::Face` borrows its
/// bytes for as long as any shaper keeps it, which is the process. A hot
/// swap loads a new library image with its own table, so each swap reads
/// the faces once again (M37: v1 never unloads).
#[cfg(feature = "shaping")]
fn intern_face(path: &'static str, index: u32) -> Option<&'static [u8]> {
    type Table = Vec<(&'static str, u32, &'static [u8])>;
    static TABLE: std::sync::Mutex<Table> = std::sync::Mutex::new(Vec::new());
    let mut t = TABLE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&(_, _, b)) = t.iter().find(|e| e.0 == path && e.1 == index) {
        return Some(b);
    }
    let bytes: &'static [u8] = Box::leak(std::fs::read(path).ok()?.into_boxed_slice());
    t.push((path, index, bytes));
    Some(bytes)
}

/// The bold face: its bytes until something needs it, then the face.
struct Bold {
    bytes: std::sync::Mutex<Option<Vec<u8>>>,
    face: std::sync::OnceLock<Option<Font>>,
}

impl Bold {
    fn new(bytes: Option<&[u8]>) -> Self {
        Bold { bytes: std::sync::Mutex::new(bytes.map(<[u8]>::to_vec)), face: std::sync::OnceLock::new() }
    }

    /// The face, parsed on first call; `None` when there were no bytes or
    /// they did not parse, which is remembered. The bytes are dropped once
    /// parsed.
    fn get(&self) -> Option<&Font> {
        self.face
            .get_or_init(|| {
                let bytes = self.bytes.lock().unwrap_or_else(|e| e.into_inner()).take()?;
                Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()
            })
            .as_ref()
    }
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

/// A face's id ([`GlyphKey::face`]): 0 regular, 1 bold, 2 + k fallback k.
const REGULAR: u8 = 0;
const BOLD: u8 = 1;

impl FontStack {
    /// Load a regular face, and optionally a bold one, from bytes. No
    /// fallback faces: a stack built from bytes draws exactly what it was
    /// given, which is what a test with a committed face wants.
    ///
    /// The regular face is parsed here; the bold one is kept as bytes and
    /// parsed on its first use (M50/T50.7).
    ///
    /// The bytes are not kept, so under `Shaping::Complex` a stack built
    /// here has nothing to shape from and lays its runs out as `Basic`
    /// does; [`FontStack::from_static`] and [`FontStack::from_files`] keep
    /// a way back to them (M81/T81.3).
    pub fn from_bytes(regular: &[u8], bold: Option<&[u8]>) -> Option<Self> {
        Self::build(regular, bold, Origin::Unknown, Origin::Unknown)
    }

    /// [`FontStack::from_bytes`] over bytes that live as long as the
    /// program (an `include_bytes!`, a leaked buffer) — M81/T81.3. The
    /// stack remembers them, so `Shaping::Complex` shapes from them with
    /// no copy; under `Basic` it holds two references more and nothing
    /// else.
    pub fn from_static(regular: &'static [u8], bold: Option<&'static [u8]>) -> Option<Self> {
        let b = bold.map_or(Origin::Unknown, Origin::Static);
        Self::build(regular, bold, Origin::Static(regular), b)
    }

    /// A stack over two face files, read here (so a missing one is found
    /// now) — what [`FontStack::system`] builds from each candidate pair.
    /// Under `Shaping::Complex` a face's bytes are read again, once per
    /// process, the first time a run needs them (M81/T81.3); under `Basic`
    /// they are never kept.
    pub fn from_files(regular: &'static str, bold: Option<&'static str>) -> Option<Self> {
        let r = std::fs::read(regular).ok()?;
        let b = bold.and_then(|p| std::fs::read(p).ok());
        let bo = match (bold, &b) {
            (Some(p), Some(_)) => Origin::File(p),
            _ => Origin::Unknown,
        };
        Self::build(&r, b.as_deref(), Origin::File(regular), bo)
    }

    fn build(regular: &[u8], bold: Option<&[u8]>, r: Origin, b: Origin) -> Option<Self> {
        let load = |b: &[u8]| {
            Font::from_bytes(b, fontdue::FontSettings::default()).ok().map(std::sync::Arc::new)
        };
        #[cfg(not(feature = "shaping"))]
        let _ = (r, b);
        Some(FontStack {
            regular: load(regular)?,
            bold: std::sync::Arc::new(Bold::new(bold)),
            fallback: std::sync::Arc::new([]),
            shaping: Shaping::Basic,
            #[cfg(feature = "shaping")]
            sources: std::sync::Arc::new(Sources::new(r, b, 0)),
        })
    }

    /// The same stack, with `faces` — (path, collection index) — tried in
    /// order for a character the primary faces lack. Each is read the
    /// first time a run needs it.
    pub fn with_fallbacks(mut self, faces: &[(&'static str, u32)]) -> Self {
        self.fallback = faces
            .iter()
            .map(|&(path, index)| Slot { path, index, face: std::sync::OnceLock::new() })
            .collect();
        #[cfg(feature = "shaping")]
        {
            self.sources = std::sync::Arc::new(Sources::new(self.sources.regular, self.sources.bold, faces.len()));
        }
        self
    }

    /// The same stack, laying runs out by `shaping` — M81/T81.3. Every
    /// clone made after this carries the mode; a measurer built over an
    /// older clone keeps the old one, so build the measurer and the
    /// renderer from the stack this returns.
    pub fn with_shaping(mut self, shaping: Shaping) -> Self {
        self.shaping = shaping;
        self
    }

    /// The mode this stack lays runs out by.
    pub fn shaping(&self) -> Shaping {
        self.shaping
    }

    /// The parsed fontdue face a [`GlyphKey::face`] names, if this stack
    /// has it.
    fn font_of(&self, face: u8) -> Option<&Font> {
        match face {
            REGULAR => Some(self.regular.as_ref()),
            BOLD => self.bold.get(),
            k => self.fallback.get(k as usize - 2)?.get(),
        }
    }

    /// The face for a CSS-style weight: 600 and up is bold.
    fn face(&self, weight: f32) -> (&Font, u8) {
        match (weight >= 600.0).then(|| self.bold.get()).flatten() {
            Some(b) => (b, BOLD),
            _ => (&self.regular, REGULAR),
        }
    }

    /// Find a system sans-serif face, with the platform's fallback faces
    /// behind it (this module's `FALLBACKS` list).
    ///
    /// Both primary faces' files are **read** here, so a missing one is
    /// found at start-up; the bold face is **parsed** on the first run
    /// that needs it — fontdue's parse is megabytes of heap per face, and
    /// an application that never draws bold should not hold one
    /// (M50/T50.7, `docs/results-competitive.md` §3).
    ///
    /// Returns `None` rather than panicking: a caller can fall back to
    /// the block stand-in and still show *something*, which is better
    /// than a window that fails to open on a machine with unusual font
    /// paths.
    pub fn system() -> Option<Self> {
        CANDIDATES
            .iter()
            .find_map(|&(r, b)| Self::from_files(r, Some(b)))
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
            (_, REGULAR) => self.bold.get().map_or((&*self.regular, REGULAR), |b| (b, BOLD)),
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
                (0, substitute(c))
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
    ///
    /// At the face's own line spacing (`line_height` 0); a run at a pitch
    /// goes through [`FontStack::place_run_into`].
    pub fn place_run(&self, text: &str, px: f32, weight: f32, max_w: f32) -> (Vec<Placed>, Size) {
        let mut scratch = ShapeScratch::new();
        let size = self.place_run_into(&mut scratch, text, px, weight, 0, max_w);
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
    ///
    /// **`line_height` is the pitch, in sixteenths of `px`** — M80/T80.1,
    /// [`lumen_static::Cmd::Text`]. `0` is fontdue's own placement, byte
    /// for byte what this did before the byte existed. Otherwise, with
    /// `pitch = line_height / 16 × px`, CSS's and iced's rule, not
    /// fontdue's:
    ///
    /// - line *k*'s box starts at `k × pitch`, and its glyphs sit at
    ///   half-leading inside it: moved down by *k* × pitch + (pitch −
    ///   *nls*)/2 − *top*, where *nls* is the line's `max_new_line_size`
    ///   and *top* its top as fontdue laid it out. A single line is
    ///   pitched too.
    /// - **A pitch below the face's spacing keeps its ink in the box.**
    ///   With `d = max(0, nls − pitch)` (`nls` the largest line's), every
    ///   line moves down a further `d/2`, so line 0's top and the last
    ///   line's bottom land on the box's edges, and the box is
    ///   `n × pitch + d` tall, the ink kept by the max
    ///   `FontStack::extent_by` already takes. At or above the face's
    ///   spacing `d` is 0 and the box is exactly `n × pitch`. Nothing clips a run and `Recording`
    ///   damages its rect, so ink outside it would be left stale.
    ///
    /// **Under `Shaping::Complex`** (M81/T81.3) a run that
    /// [`needs_shaping`] is laid out by `lumen-static-text`'s `Shaper`
    /// instead of fontdue: bidi order, joining, marks, UAX #14 breaks at
    /// the painter's width. Each glyph sits on fontdue's ceil'd ascent and
    /// line size, with the same cap shift, and is re-placed at the pitch
    /// by the rule below; its `x` keeps the shaper's fraction, which the
    /// backends round. Placements come out in visual order. A run with
    /// nothing to shape takes the fontdue path whatever the mode
    /// (`tests/basic_placements.rs` holds it to `BASIC_CHECKSUM`).
    ///
    /// fontdue's `LayoutSettings::line_height` is **not** used: it scales
    /// the face's ceil'd new-line size and only between lines, so a ratio
    /// passed through paints 1.5× as about 2 em and leaves a single line
    /// unscaled. Re-placing from `layout.lines()` with a forward cursor
    /// allocates nothing (`tests/line_height.rs`).
    pub fn place_run_into(
        &self,
        scratch: &mut ShapeScratch,
        text: &str,
        px: f32,
        weight: f32,
        line_height: u8,
        max_w: f32,
    ) -> Size {
        #[cfg(feature = "shaping")]
        if self.complex(text) {
            return self.place_shaped(scratch, text, px, weight, line_height, max_w);
        }
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
        let mut pitched = Pitched::new(layout, px, line_height);
        for (gi, g) in layout.glyphs().iter().enumerate() {
            let dy = pitched.dy(gi);
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
                id: GlyphId::from_key(GlyphKey { px_bits: px.to_bits(), index: g.key.glyph_index, face, phase: 0 }),
                x: g.x,
                y: g.y + dy + shift,
                w: g.width as u32,
                h: g.height as u32,
            });
        }
        PLACED.fetch_add(out.len() as u64, core::sync::atomic::Ordering::Relaxed);
        // The box by the same rule `measure_run` uses (`extent`), so the
        // two cannot disagree.
        let size = Self::extent(&faces, layout, px);
        if line_height == 0 {
            return size;
        }
        // At a pitch: `n × pitch + d`, or the moved ink if it reaches
        // further, measured as `extent_by` measures ink (without the cap
        // shift, which centres capitals in the line box it was given).
        let mut pitched = Pitched::new(layout, px, line_height);
        let mut ink = 0.0f32;
        for (gi, g) in layout.glyphs().iter().enumerate() {
            ink = ink.max(g.y + pitched.dy(gi) + g.height as f32);
        }
        Size { w: size.w, h: pitched.height().max(ink).ceil() }
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
    /// byte-identical bitmaps. The key's `phase` is 0 on every id this
    /// stack makes until M81b gives it a raster of its own. That is what makes an atlas cache
    /// correct rather than approximately correct.
    pub fn rasterize(&self, id: GlyphId) -> Option<(u32, u32, Vec<u8>)> {
        let GlyphKey { px_bits, index, face, .. } = id.0;
        let font = self.font_of(face)?;
        let (m, cov) = font.rasterize_indexed(index, f32::from_bits(px_bits));
        RASTERISED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        Some((m.width as u32, m.height as u32, cov))
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
    ///
    /// Under `Shaping::Complex` the shaper lays the run out instead
    /// (M81/T81.3), into the scratch's `ShapedRun`, and the box is
    /// `shaped_extent`'s — the rule `place_run_into`'s `Complex` branch
    /// sizes by, so measure and paint still cannot disagree.
    pub(crate) fn measure_run(&self, text: &str, px: f32, weight: f32, max_w: f32, scratch: &mut ShapeScratch) -> Size {
        #[cfg(feature = "shaping")]
        if self.complex(text) {
            MEASURED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
            return self.shape_complex(scratch, text, px, weight, max_w);
        }
        let layout = &mut scratch.layout;
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
        scratch: &mut ShapeScratch,
    ) -> Size {
        #[cfg(feature = "shaping")]
        if self.complex(text) {
            // The shaper's clusters, already in the contract's terms:
            // source order, leading-edge x, `rtl` set (M81/T81.2).
            MEASURED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
            let size = self.shape_complex(scratch, text, px, weight, max_w);
            for c in &scratch.shaped.run.clusters {
                if sink.is_full() {
                    break;
                }
                sink.cluster(*c);
            }
            return size;
        }
        let layout = &mut scratch.layout;
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
                sink.cluster(Cluster { byte: byte as u32, x: g.x, line, rtl: false });
            } else if step {
                caller.next();
            }
            end = (g.x + adv, line);
        }
        // The insertion point after the last character.
        if !sink.is_full() {
            sink.cluster(Cluster { byte: text.len() as u32, x: end.0, line: end.1, rtl: false });
        }
        Self::extent(&faces, layout, px)
    }
}

/// The `Complex` path — M81/T81.3 (`docs/plan-shaped-paint.md`, "Shaping
/// happens where it already happens"). The shaper lays a run out; these
/// put its glyphs on the baseline, the cap shift and the line pitch the
/// `Basic` path uses, so a widget sized under one mode is sized by the
/// same rule under the other.
#[cfg(feature = "shaping")]
impl FontStack {
    /// Whether runs take the shaper: the mode is `Complex` and the
    /// regular face's bytes can be had (a [`FontStack::from_bytes`] stack's
    /// cannot, and keeps `Basic`'s layout).
    ///
    /// **And the run needs shaping** ([`needs_shaping`]) — K81a's
    /// response (`docs/plan-shaped-paint.md`, `docs/results-text.md`
    /// M81/T81.3): shaping Latin through rustybuzz cost 2.17× (1 KB) and
    /// 3.63× (100 KB) of `Basic` on `--bin textarea`, over the 1.5× bound,
    /// so a run with nothing to shape is laid out by fontdue under
    /// `Complex` too, byte-identical to `Basic` (`tests/basic_placements.rs`).
    fn complex(&self, text: &str) -> bool {
        self.shaping == Shaping::Complex
            && needs_shaping(text)
            && lumen_static_text::FaceSource::bytes(self, REGULAR).is_some()
    }

    /// The line metrics a shaped run is stacked by: the face `weight`
    /// picks, fontdue's ceil'd ascent and new-line size, as its layout
    /// takes them (`Layout::append`).
    fn shaped_lines(&self, weight: f32, px: f32) -> (f32, f32) {
        match self.face(weight).0.horizontal_line_metrics(px) {
            Some(m) => (m.ascent.ceil(), m.new_line_size.ceil()),
            None => (px, (px * 1.2).ceil()),
        }
    }

    /// Shape `text` into the scratch's run and return its box.
    fn shape_complex(&self, scratch: &mut ShapeScratch, text: &str, px: f32, weight: f32, max_w: f32) -> Size {
        let sh = &mut scratch.shaped;
        sh.shaper.shape(self, text, px, weight, max_w, &mut sh.run);
        COMPLEX.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        if sh.run.bidi {
            BIDI.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        }
        self.shaped_extent(&sh.run, px, weight)
    }

    /// Where shaped glyph `g` inks, unpitched: (x, top) from the run's
    /// top-left, and its bitmap size; `None` for a glyph with no ink or a
    /// face this stack does not have.
    fn shaped_ink(&self, g: &lumen_static_text::ShapedGlyph, px: f32, ascent: f32, nls: f32) -> Option<(f32, f32, u32, u32)> {
        let m = self.font_of(g.key.face)?.metrics_indexed(g.key.index, px);
        if m.width == 0 || m.height == 0 {
            return None;
        }
        // fontdue's own placement of a bitmap against its baseline
        // (`Layout::append`): the bounds' xmin, and the top floored.
        let baseline = g.line as f32 * nls + ascent;
        let top = baseline + (g.y - m.bounds.height - m.bounds.ymin).floor();
        Some((g.x + m.bounds.xmin, top, m.width as u32, m.height as u32))
    }

    /// A shaped run's box, by `extent_by`'s rule: per line the larger of
    /// where the pen ends (the line's width) and where the ink ends, and
    /// the lines' height or the ink's, whichever reaches further.
    fn shaped_extent(&self, run: &lumen_static_text::ShapedRun, px: f32, weight: f32) -> Size {
        let (ascent, nls) = self.shaped_lines(weight, px);
        let mut w = run.width();
        let mut h = run.line_count().max(1) as f32 * nls;
        for g in &run.glyphs {
            if let Some((x, top, gw, gh)) = self.shaped_ink(g, px, ascent, nls) {
                w = w.max(x + gw as f32);
                h = h.max(top + gh as f32);
            }
        }
        Size { w: w.ceil(), h: h.ceil() }
    }

    /// `place_run_into` under `Complex`: shape at the painter's width
    /// (`paint_width`), then each glyph on its line's baseline with the
    /// cap shift, re-placed at the pitch by M80's rule (`Pitched`), which
    /// for a shaped run's uniform lines is line *k* moved by *k* × pitch +
    /// (pitch − nls)/2 + d/2 − *k* × nls.
    fn place_shaped(&self, scratch: &mut ShapeScratch, text: &str, px: f32, weight: f32, line_height: u8, max_w: f32) -> Size {
        let size = self.shape_complex(scratch, text, px, weight, Self::paint_width(max_w));
        SHAPED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        let (font, id) = self.face(weight);
        let shift = scratch.caps.shift(id, font, px);
        let (ascent, nls) = self.shaped_lines(weight, px);
        let lines = scratch.shaped.run.line_count().max(1) as f32;
        let (pitch, deficit) = if line_height == 0 {
            (0.0, 0.0)
        } else {
            let p = line_height as f32 / 16.0 * px;
            (p, (nls - p).max(0.0))
        };
        let dy = |line: u16| {
            if line_height == 0 {
                0.0
            } else {
                let k = line as f32;
                k * pitch + (pitch - nls) / 2.0 + deficit / 2.0 - k * nls
            }
        };
        let out = &mut scratch.placed;
        out.clear();
        let mut ink = 0.0f32;
        for g in &scratch.shaped.run.glyphs {
            let Some((x, top, w, h)) = self.shaped_ink(g, px, ascent, nls) else { continue };
            let d = dy(g.line);
            ink = ink.max(top + d + h as f32);
            out.push(Placed { id: GlyphId::from_key(g.key), x, y: top + d + shift, w, h });
        }
        PLACED.fetch_add(out.len() as u64, core::sync::atomic::Ordering::Relaxed);
        if line_height == 0 {
            return size;
        }
        Size { w: size.w, h: (lines * pitch + deficit).max(ink).ceil() }
    }
}

/// Whether a run has anything for a shaper to do — K81a's response
/// (M81/T81.3). One pass over the characters, each tested against a short
/// list of ranges that **need no shaping**: no right-to-left bidi class,
/// no joining script, no combining mark, no bidi or joiner control and no
/// variation selector — Latin with its extensions and IPA, spacing
/// modifiers, Greek and Cyrillic (less their combining marks), Armenian,
/// Latin and Greek extended, general punctuation (less the zero-width,
/// joiner and bidi controls), super/subscripts and currency, the symbol
/// and arrow blocks, kana (less the combining voicing marks), unified CJK
/// ideographs and Hangul syllables. Anything else — Hebrew, Arabic, every
/// Indic and South-East Asian script, combining marks, emoji and every
/// supplementary-plane character — goes to the shaper. Conservative by
/// construction: a character not listed costs speed, never correctness.
pub fn needs_shaping(text: &str) -> bool {
    text.chars().any(|c| {
        let c = c as u32;
        !(c < 0x300
            || (0x370..0x483).contains(&c)
            || (0x48a..0x591).contains(&c)
            || (0x1e00..0x2000).contains(&c)
            || (0x2010..0x2028).contains(&c)
            || (0x2030..0x2066).contains(&c)
            || (0x2070..0x20d0).contains(&c)
            || (0x2100..0x2c00).contains(&c)
            || (0x3040..0x3099).contains(&c)
            || (0x30a0..0x3100).contains(&c)
            || (0x4e00..0xa000).contains(&c)
            || (0xac00..0xd7a4).contains(&c))
    })
}

/// The stack's own rule, for the shaper — M81/T81.3 (`docs/plan-shaped-paint.md`,
/// "The shaper"): which face draws a character, and its bytes.
#[cfg(feature = "shaping")]
impl lumen_static_text::FaceSource for FontStack {
    /// Per character, in `prepare`'s order: the face `weight` asks for
    /// (whitespace always), the other primary face, the first fallback
    /// face that has it, and otherwise the asked-for face with
    /// `substitute`'s character.
    fn face_for(&self, ch: char, weight: f32) -> (u8, char) {
        let (wanted, id) = self.face(weight);
        if ch.is_whitespace() || wanted.lookup_glyph_index(ch) != 0 {
            return (id, ch);
        }
        let (other, oid) = self.other(weight);
        if other.lookup_glyph_index(ch) != 0 {
            return (oid, ch);
        }
        match self.fallback_for(ch) {
            Some((k, _)) => (2 + k as u8, ch),
            None => (id, substitute(ch)),
        }
    }

    /// The face's bytes, read on first use and kept for the process
    /// (`Sources`, `intern_face`).
    fn bytes(&self, face: u8) -> Option<&'static [u8]> {
        let slot = self.sources.bytes.get(face as usize)?;
        *slot.get_or_init(|| {
            let origin = match face {
                REGULAR => self.sources.regular,
                BOLD => self.sources.bold,
                k => {
                    let s = self.fallback.get(k as usize - 2)?;
                    return intern_face(s.path, s.index);
                }
            };
            match origin {
                Origin::File(p) => intern_face(p, 0),
                Origin::Static(b) => Some(b),
                Origin::Unknown => None,
            }
        })
    }
}

/// What a character no face in the stack has is drawn as — the tofu
/// doctrine's last resort (`FontStack::prepare`), shared with the shaper's
/// `FaceSource::face_for` so the two paths substitute alike.
fn substitute(c: char) -> char {
    match c {
        '\u{2212}' | '\u{2013}' | '\u{2014}' => '-',
        '\u{2018}' | '\u{2019}' => '\'',
        '\u{201c}' | '\u{201d}' => '"',
        _ => '?',
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
/// Runs laid out by the shaper under `Shaping::Complex` — [`text_counters`].
static COMPLEX: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
/// Of those, runs `unicode_bidi` ordered — [`text_counters`].
static BIDI: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

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
    /// Runs the `lumen-static-text` shaper laid out, for a measure, a
    /// cluster query or a paint, under `Shaping::Complex` (M81/T81.3).
    /// Always 0 without the shell's `shaping` feature.
    pub complex: u64,
    /// Of `complex`, the runs `unicode_bidi` ordered (a right-to-left
    /// character present): the only shaped runs that allocate on a warm
    /// scratch (`tests/alloc_gate.rs` `mod complex`).
    pub bidi: u64,
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
        complex: COMPLEX.load(Relaxed),
        bidi: BIDI.load(Relaxed),
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

/// Where each line of a laid-out run moves to at a line pitch — M80/T80.1.
///
/// [`FontStack::place_run_into`]'s rule, as a forward cursor over the
/// layout's lines, so re-placing a run allocates nothing and costs one
/// comparison per glyph. Inert (every `dy` 0) at `line_height` 0, which
/// is how the run that sets nothing keeps fontdue's placement exactly.
struct Pitched<'l> {
    lines: &'l [fontdue::layout::LinePosition],
    /// `line_height / 16 × px`; 0 when the run keeps the face's spacing.
    pitch: f32,
    /// `max(0, nls − pitch)`, `nls` the largest line's new-line size.
    deficit: f32,
    li: usize,
}

impl<'l> Pitched<'l> {
    fn new(layout: &'l Layout, px: f32, line_height: u8) -> Self {
        let lines = layout.lines().map_or(&[][..], |l| l.as_slice());
        if line_height == 0 {
            return Pitched { lines: &[], pitch: 0.0, deficit: 0.0, li: 0 };
        }
        let pitch = line_height as f32 / 16.0 * px;
        let nls = lines.iter().fold(0.0f32, |m, l| m.max(l.max_new_line_size));
        Pitched { lines, pitch, deficit: (nls - pitch).max(0.0), li: 0 }
    }

    /// How far glyph `gi` moves down. Glyphs must be asked in order. A
    /// glyph in no line (an empty line's range is backwards) moves with
    /// the line the cursor is on, as `shape_into` reports it.
    fn dy(&mut self, gi: usize) -> f32 {
        let lines = self.lines;
        while self.li + 1 < lines.len() && gi > lines[self.li].glyph_end {
            self.li += 1;
        }
        let Some(l) = lines.get(self.li) else { return 0.0 };
        let top = l.baseline_y - l.max_ascent;
        self.li as f32 * self.pitch + (self.pitch - l.max_new_line_size) / 2.0 + self.deficit / 2.0 - top
    }

    /// `n × pitch + max(0, nls − pitch)`: the box the lines' boxes fill.
    fn height(&self) -> f32 {
        self.lines.len() as f32 * self.pitch + self.deficit
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
    /// The shaper and the run it refills, under `Shaping::Complex`
    /// (M81/T81.3): its parsed faces, shape plans and buffers kept, so a
    /// steady frame shapes without allocating (`tests/alloc_gate.rs`
    /// `mod complex`).
    #[cfg(feature = "shaping")]
    shaped: Shaped,
}

/// A [`ShapeScratch`]'s shaper and its output — M81/T81.3.
#[cfg(feature = "shaping")]
#[derive(Default)]
struct Shaped {
    shaper: lumen_static_text::Shaper,
    run: lumen_static_text::ShapedRun,
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
        ShapeScratch {
            layout: Layout::new(CoordinateSystem::PositiveYDown),
            placed: Vec::new(),
            caps: CapShifts::default(),
            #[cfg(feature = "shaping")]
            shaped: Shaped::default(),
        }
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

    /// Make sure glyph `id` is in the cache, rasterising it if not — the
    /// frame thread's half (`TextState::prepare` in `software.rs`). A
    /// glyph too large to keep is counted `uncached` and left for the
    /// band that draws it to rasterise.
    pub(crate) fn fill(&mut self, font: &FontStack, id: GlyphId) {
        if self.index.contains_key(&id) {
            self.stats.hits += 1;
            return;
        }
        let Some((w, h, cov)) = font.rasterize(id) else { return };
        let (w, h) = (w as usize, h as usize);
        let n = w * h;
        if n > Self::MAX_GLYPH || w > u16::MAX as usize || h > u16::MAX as usize {
            self.stats.uncached += 1;
            return;
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
    }

    /// Glyph `id`'s coverage and size, if the cache holds it — the bands'
    /// half, through a shared `&`.
    pub(crate) fn get(&self, id: GlyphId) -> Option<(&[u8], usize, usize)> {
        let &(off, w, h) = self.index.get(&id)?;
        let (off, w, h) = (off as usize, w as usize, h as usize);
        Some((&self.arena[off..off + w * h], w, h))
    }
}

/// A hasher for [`GlyphId`]: four small integers, so a multiply-and-fold
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
///
/// **A newtype over the core's [`GlyphKey`]** since M81/T81.1
/// (`docs/plan-shaped-paint.md`, D1 c): the shaper in `lumen-static-text`
/// speaks `GlyphKey`, which this crate cannot hand it as its own type
/// once it depends on that crate. The field is private, so an id is made
/// only here, by the stack that rasterises it ([`FontStack::place_run`]),
/// and the contract above stays this crate's: [`GlyphId::key`] reads one
/// out, and nothing outside the shell turns a key back into an id. Still
/// 8 B (the key's spare byte is its `phase`, 0 for every glyph until
/// M81b), so `GlyphCache` and the GPU atlas keep their sizes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GlyphId(GlyphKey);

impl GlyphId {
    /// An id for `key`, which this stack's placement produced. Crate-only:
    /// a key from elsewhere names a face of some other stack.
    pub(crate) fn from_key(key: GlyphKey) -> Self {
        GlyphId(key)
    }

    /// The glyph's index in its face — for a test that compares what was
    /// painted with what a shaper produced (`docs/plan-text-correctness.md`
    /// T45.0 item 2). Equal indices in the same face and size are the
    /// same bitmap; the index alone does not say which face.
    pub fn index(&self) -> u16 {
        self.0.index
    }

    /// The id as the core's [`GlyphKey`], field for field: its size bits,
    /// glyph index, face (0 regular, 1 bold, 2 + k the stack's fallback
    /// `k`; M45/T45.3) and phase.
    pub fn key(&self) -> GlyphKey {
        self.0
    }
}

impl From<GlyphId> for GlyphKey {
    fn from(id: GlyphId) -> GlyphKey {
        id.key()
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
    /// The scratch every measure and shape query lays its run out into —
    /// M50/T50.2. A fresh layout per measure grew three `Vec`s from empty
    /// each time: 4.60 allocations a frame on a label that changed every
    /// frame (`docs/results-allocation.md`). A [`ShapeScratch`] since
    /// M81/T81.3, so a `Complex` stack's measure keeps its shaper too.
    scratch: ShapeScratch,
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
        FontMetrics { stack, scale: 1.0, generation: next_generation(), scratch: ShapeScratch::new() }
    }

    /// A measurer that reports logical sizes for a display with this
    /// scale factor.
    pub fn with_scale(stack: FontStack, scale: f32) -> Self {
        let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
        FontMetrics { stack, scale, generation: next_generation(), scratch: ShapeScratch::new() }
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
        let s = self.stack.measure_run(text, size * k, weight, max_w * k, &mut self.scratch);
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
            sink.cluster(Cluster { byte: 0, x: 0.0, line: 0, rtl: false });
            return Size { w: 0.0, h: self.line_height(size) };
        }
        // Clusters are reported in logical x, because a caret is placed
        // by `layout` and `paint`, both of which speak logical.
        let k = self.scale;
        let mut scaled = LogicalClusters { inner: sink, scale: k };
        let s = self.stack.shape_into(text, size * k, weight, max_w * k, &mut scaled, &mut self.scratch);
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
