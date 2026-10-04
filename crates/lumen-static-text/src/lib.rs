//! Complex-script text — `docs/plan-capabilities.md` T15.8, and the
//! shaper `docs/plan-shaped-paint.md` (M81) paints with.
//!
//! Two things live here:
//!
//! - [`Shaper`], over a [`FaceSource`]: bidirectional levels, a run per
//!   (direction, face), each shaped by rustybuzz in logical order,
//!   greedy line breaking at UAX #14 opportunities, and each line put
//!   in visual order. Its output, a [`ShapedRun`], is glyph keys a
//!   rasteriser can draw and clusters in source order. M81/T81.2 built
//!   it; since M81/T81.3 `lumen-static-shell`'s `FontStack` implements
//!   [`FaceSource`] and paints from it under its `shaping` feature, in
//!   `Shaping::Complex`.
//! - [`ComplexText`], a [`TextMeasure`] over one face: the same
//!   [`Shaper`] through a single-face [`FaceSource`], so a `Host` caller
//!   measures with it as before T81.2.
//!
//! # Why this is a crate and not a feature
//!
//! `docs/text-render-adoption-decision.md` declined a heavyweight text
//! stack twice, and the reasons still hold **for the default**: fontdue
//! measures in 2.9 µs a label and the embedded profile has a baked
//! bitmap face with no shaper at all. What was missing was a way for an
//! application that genuinely needs Arabic or Devanagari to get it
//! *without* changing that default for everyone else.
//!
//! A separate crate is that way, and it is the M15 gating policy's
//! strongest mechanism: nothing here enters a dependency graph unless a
//! manifest names it — since M81/T81.1 that manifest is the shell's,
//! behind its default-off `shaping` feature. The core is untouched,
//! `BakedFace` is untouched, and `just thumb` still builds the core with
//! no `std` and no allocator.
//!
//! # What it changes — measured and painted, under `Complex`
//!
//! Until M81/T81.3 this section was "measurement only": the shaper's box
//! and clusters were right, and the painter drew the *string* with its
//! own cmap lookup in logical order, so Arabic was painted unjoined and
//! left to right (R10-F2, `docs/plan-text-correctness.md` §2.1). Since
//! T81.3 a `FontStack` in `Shaping::Complex` measures, reports clusters
//! **and paints** through [`Shaper`] — in `FontStack::place_run_into`,
//! the one function both backends place glyphs with — so what is painted
//! is the shaper's glyphs (`lumen-static-shell/tests/paint_shaped.rs`).
//! A run in which no character needs shaping keeps fontdue's layout even
//! there: shaping Latin through rustybuzz cost 2.2–3.6× `Basic` (K81a,
//! `docs/results-text.md` M81/T81.3).
//!
//! **A program selects it** (M81/T81.4): `Program::shaping(Shaping::Complex)`
//! carries the mode to the session's measurer, the agent's audit, the
//! window (through `Session::shaping`, before its first backend and after
//! every swap) and the headless shots (`docs/plan-shaped-paint.md`). A
//! bare `Host` caller builds a `Complex` stack and hands clones to its
//! measurer and canvas by hand.
#![forbid(unsafe_code)]

use lumen_static::cx::{Cluster, GlyphSink, TextMeasure};
use lumen_static::desc::Size;
use lumen_static::text::GlyphKey;
use rustybuzz::{Direction, Script, ShapePlan, UnicodeBuffer};
use unicode_bidi::{BidiClass, BidiInfo};
use unicode_linebreak::BreakOpportunity;

/// Where a [`Shaper`] gets its faces: which face draws a character, and
/// that face's bytes — M81/T81.2 (`docs/plan-shaped-paint.md`, "The
/// shaper").
///
/// Two methods so the **rule stays with the stack that owns it**: the
/// shell's `FontStack` answers by its own regular/bold split, its
/// fallback faces and its substitution table, and the shaper never
/// copies that rule into a second crate (the review's T3 (2)). Face ids
/// are the stack's: 0 regular, 1 bold, 2 + k fallback `k`, as
/// [`GlyphKey::face`] names them.
pub trait FaceSource {
    /// The face that draws `ch` at `weight`, and the character to shape
    /// in its place (`ch` itself unless the stack substitutes one, as it
    /// does `\u{2212}` → `-` for a face without a minus sign). The
    /// substitute is shaped with `ch`'s byte offset, so a cluster still
    /// indexes the caller's string.
    fn face_for(&self, ch: char, weight: f32) -> (u8, char);

    /// That face's bytes, loading it on first use as the stack's own
    /// fallback slots do. `'static` because a parsed `rustybuzz::Face`
    /// borrows them for as long as the [`Shaper`] keeps it. `None` for a
    /// face that cannot be had; the shaper then draws that character
    /// with face 0.
    fn bytes(&self, face: u8) -> Option<&'static [u8]>;
}

/// One shaped glyph: what a rasteriser draws and where.
///
/// `x` is from the start of its line, in px, the glyph's own offset
/// included; `y` is from the baseline, px, **downward** (rustybuzz's
/// offsets point up and are negated here, as a screen's y runs). Lines
/// are laid out from x = 0 whatever their direction: aligning a line in
/// its box is the caller's, as it is for every measurer here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapedGlyph {
    pub key: GlyphKey,
    pub x: f32,
    pub y: f32,
    pub line: u16,
}

/// One laid-out line of a [`ShapedRun`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapedLine {
    /// The source bytes the line holds, `start..end`.
    pub start: u32,
    pub end: u32,
    /// Its width in px. A line a soft wrap ended does not count the
    /// whitespace it ends in (that hangs past the edge); a line a
    /// paragraph or a line separator ends keeps it, as an unwrapped run
    /// always did.
    pub width: f32,
    /// The paragraph's base direction.
    pub rtl: bool,
}

/// What [`Shaper::shape`] produced: glyphs line by line in **visual**
/// order, clusters in **source** order, and the lines. The caller keeps
/// one and the shaper refills it, so a steady frame allocates nothing
/// for it.
#[derive(Clone, Debug, Default)]
pub struct ShapedRun {
    pub glyphs: Vec<ShapedGlyph>,
    /// Every cluster boundary, in rising byte order with `x` the leading
    /// edge ([`Cluster`]'s contract), then the trailing insertion point
    /// (byte = the text's length), which carries the last paragraph's
    /// direction and sits at that direction's end of the last line.
    ///
    /// **A direction change is two insertion points** — M81/T81.5. Where
    /// the next character (or the text's end) runs the other way, the
    /// offset between them has two places on screen: the trailing edge
    /// of the run that ends there, and the leading edge of what follows.
    /// The first is reported too, *before* the second and with the same
    /// byte, carrying the ending run's `rtl` and line. Leading edges
    /// alone cannot give it: in `abc שלום` the edge between the space
    /// and the Hebrew word is neither cluster's leading edge, so a
    /// selection's tint could not end there (`docs/plan-shaped-paint.md`,
    /// "What building found"). A run with no direction change — every
    /// run that builds no `BidiInfo` — has none, so its list is
    /// unchanged. A reader that wants one place per offset takes the
    /// last cluster with that byte.
    pub clusters: Vec<Cluster>,
    pub lines: Vec<ShapedLine>,
    /// Whether `unicode_bidi` ordered this run: some character has a
    /// right-to-left bidi class. A run without one builds no `BidiInfo`
    /// and allocates nothing for it; one with one allocates what
    /// `unicode_bidi` does, which M81/T81.3's allocation gate counts per
    /// run of this kind.
    pub bidi: bool,
}

impl ShapedRun {
    pub fn new() -> Self {
        Self::default()
    }

    /// Empty all three, keeping their capacity, and clear `bidi`.
    pub fn clear(&mut self) {
        self.glyphs.clear();
        self.clusters.clear();
        self.lines.clear();
        self.bidi = false;
    }

    /// The widest line, px.
    pub fn width(&self) -> f32 {
        self.lines.iter().fold(0.0, |w, l| w.max(l.width))
    }

    /// How many lines the run landed on; at least 1 once shaped.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}

/// One glyph in **logical** order, before lines exist.
#[derive(Clone, Copy, Debug)]
struct Logical {
    id: u16,
    face: u8,
    /// The caller's byte offset of the cluster this glyph belongs to.
    cluster: u32,
    advance: f32,
    dx: f32,
    dy: f32,
    /// A line separator's glyph: it takes no width and is not drawn.
    separator: bool,
}

/// One cluster in logical order: its glyphs are `logical[g0..g1]`.
#[derive(Clone, Copy, Debug)]
struct Unit {
    byte: u32,
    g0: u32,
    g1: u32,
    advance: f32,
    space: bool,
}

/// A paragraph's range and base direction.
#[derive(Clone, Copy, Debug)]
struct Para {
    start: usize,
    end: usize,
    rtl: bool,
    /// Index into `BidiInfo::paragraphs`, when bidi ran.
    index: usize,
}

/// One line's place in the run being built.
#[derive(Clone, Copy, Debug)]
struct LineAt {
    units: (usize, usize),
    soft: bool,
    line: u16,
}

/// A text shaper over a [`FaceSource`] — M81/T81.2
/// (`docs/plan-shaped-paint.md`, "The shaper").
///
/// Per paragraph: `unicode_bidi` levels, **skipped when no character
/// has a right-to-left bidi class**, so a Latin run builds no
/// `BidiInfo`; the paragraph split into runs of one direction and one
/// face; each run shaped by rustybuzz in logical order, characters
/// pushed one by one with their own byte offsets so a substitute keeps
/// its source byte; greedy line breaking at UAX #14 opportunities
/// (`unicode-linebreak`) within `max_w`, a word wider than `max_w`
/// broken between its clusters; then each line reordered visually
/// (`BidiInfo::visual_runs`, which applies the line-level rules).
///
/// It keeps one parsed `rustybuzz::Face` per face id, rustybuzz's shape
/// plans per (face, direction, script), its `UnicodeBuffer`, and its
/// scratch, so shaping again allocates only what `unicode_bidi`
/// allocates (T81.3 counts it). A face is re-parsed when its source
/// hands back different bytes for its id, so one shaper can serve two
/// sources without mixing their faces.
///
/// **Joining across a line break is not re-shaped**: a run is shaped
/// once, before it is broken, so an Arabic word broken mid-word by a box
/// narrower than it keeps the joined forms it had whole.
#[derive(Default)]
pub struct Shaper {
    faces: Vec<Option<(&'static [u8], rustybuzz::Face<'static>)>>,
    plans: Vec<(u8, Direction, Script, ShapePlan)>,
    buffer: Option<UnicodeBuffer>,
    logical: Vec<Logical>,
    units: Vec<Unit>,
    paras: Vec<Para>,
    breaks: Vec<(u32, bool)>,
    /// The trailing insertion points at direction changes, before they
    /// are merged into the run's clusters (`ShapedRun::clusters`).
    closing: Vec<Cluster>,
}

/// Whether `c` is a right-to-left bidi class, or an embedding that can
/// make one. Arabic numbers are included: they can raise a level even in
/// a left-to-right paragraph.
fn rtl_class(c: char) -> bool {
    matches!(
        unicode_bidi::bidi_class(c),
        BidiClass::R | BidiClass::AL | BidiClass::AN | BidiClass::RLE | BidiClass::RLO | BidiClass::RLI
    )
}

/// A character that ends a line by itself: it draws nothing and takes
/// no width.
fn line_separator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

impl Shaper {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether face `id` is parsed and current, parsing it (again, if
    /// its bytes moved) from `src`.
    fn ensure_face<S: FaceSource + ?Sized>(&mut self, src: &S, id: u8) -> bool {
        let Some(bytes) = src.bytes(id) else { return false };
        let i = id as usize;
        if self.faces.len() <= i {
            self.faces.resize_with(i + 1, || None);
        }
        if matches!(&self.faces[i], Some((b, _)) if core::ptr::eq(*b, bytes)) {
            return true;
        }
        self.plans.retain(|p| p.0 != id);
        self.faces[i] = rustybuzz::Face::from_slice(bytes, 0).map(|f| (bytes, f));
        self.faces[i].is_some()
    }

    /// The face and character `src` picks for `ch`, with face 0 standing
    /// in for a face that cannot be loaded.
    fn face_of<S: FaceSource + ?Sized>(&mut self, src: &S, ch: char, weight: f32) -> (u8, char) {
        let (face, sub) = src.face_for(ch, weight);
        if self.ensure_face(src, face) {
            (face, sub)
        } else {
            (0, ch)
        }
    }

    /// Shape `text` at `px` and `weight`, wrapped to `max_w` (no wrap
    /// when it is not finite and positive), into `out`.
    pub fn shape<S: FaceSource + ?Sized>(
        &mut self,
        src: &S,
        text: &str,
        px: f32,
        weight: f32,
        max_w: f32,
        out: &mut ShapedRun,
    ) {
        out.clear();
        if text.is_empty() {
            // One cluster: the caret position in an empty field, the
            // contract every measurer here honours.
            out.clusters.push(Cluster { byte: 0, x: 0.0, line: 0, rtl: false });
            out.lines.push(ShapedLine { start: 0, end: 0, width: 0.0, rtl: false });
            return;
        }
        let wrap = max_w.is_finite() && max_w > 0.0;

        // Bidi only when something can be right to left: a Latin run
        // builds no `BidiInfo` and allocates nothing for it.
        let bidi = if text.chars().any(rtl_class) { Some(BidiInfo::new(text, None)) } else { None };
        out.bidi = bidi.is_some();
        self.paras.clear();
        match &bidi {
            Some(info) => {
                for (index, p) in info.paragraphs.iter().enumerate() {
                    self.paras.push(Para { start: p.range.start, end: p.range.end, rtl: p.level.is_rtl(), index });
                }
            }
            None => self.paras.push(Para { start: 0, end: text.len(), rtl: false, index: 0 }),
        }

        // Break opportunities for the whole text, (byte, mandatory), in
        // rising byte order.
        self.breaks.clear();
        self.breaks.extend(
            unicode_linebreak::linebreaks(text).map(|(b, o)| (b as u32, matches!(o, BreakOpportunity::Mandatory))),
        );

        let mut line: u16 = 0;
        let mut last_rtl = false;
        let mut closing = core::mem::take(&mut self.closing);
        closing.clear();
        for pi in 0..self.paras.len() {
            let para = self.paras[pi];
            last_rtl = para.rtl;
            self.shape_paragraph(src, text, px, weight, para, bidi.as_ref());
            self.break_and_place(px, wrap, max_w, para, bidi.as_ref(), &mut line, out, &mut closing);
        }

        // Clusters arrive per line in visual order; the contract is
        // source order.
        out.clusters.sort_unstable_by_key(|c| c.byte);
        // Each direction change's trailing insertion point goes in
        // before the cluster with its byte (`ShapedRun::clusters`):
        // merged from the back, a regular cluster taking the later slot
        // on a tie. Only a bidi run has any.
        if !closing.is_empty() {
            closing.sort_unstable_by_key(|c| c.byte);
            let (mut i, mut j) = (out.clusters.len(), closing.len());
            out.clusters.extend_from_slice(&closing);
            let mut k = out.clusters.len();
            while j > 0 {
                k -= 1;
                if i > 0 && out.clusters[i - 1].byte >= closing[j - 1].byte {
                    i -= 1;
                    out.clusters[k] = out.clusters[i];
                } else {
                    j -= 1;
                    out.clusters[k] = closing[j];
                }
            }
        }
        self.closing = closing;

        // The insertion point after the last character: the paragraph
        // direction's end of the last line, or the start of a new empty
        // line when the text ends in a line separator.
        let len = text.len() as u32;
        let (x, l) = if text.chars().next_back().is_some_and(line_separator) {
            out.lines.push(ShapedLine { start: len, end: len, width: 0.0, rtl: last_rtl });
            (0.0, line)
        } else {
            let w = out.lines.last().map_or(0.0, |l| l.width);
            (if last_rtl { 0.0 } else { w }, line.saturating_sub(1))
        };
        out.clusters.push(Cluster { byte: len, x, line: l, rtl: last_rtl });
    }

    /// Itemise one paragraph into runs of one direction and one face,
    /// shape each, and leave its glyphs in `self.logical` and its
    /// clusters in `self.units`, both in logical order.
    fn shape_paragraph<S: FaceSource + ?Sized>(
        &mut self,
        src: &S,
        text: &str,
        px: f32,
        weight: f32,
        para: Para,
        bidi: Option<&BidiInfo<'_>>,
    ) {
        self.logical.clear();
        self.units.clear();
        let mut run: Option<(usize, bool, u8)> = None;
        for (off, ch) in text[para.start..para.end].char_indices() {
            let byte = para.start + off;
            let rtl = bidi.is_some_and(|b| b.levels[byte].is_rtl());
            let face = self.face_of(src, ch, weight).0;
            match run {
                Some((s, r, f)) if r != rtl || f != face => {
                    self.shape_run(src, text, (s, byte), r, f, px, weight);
                    run = Some((byte, rtl, face));
                }
                None => run = Some((byte, rtl, face)),
                _ => {}
            }
        }
        if let Some((s, r, f)) = run {
            self.shape_run(src, text, (s, para.end), r, f, px, weight);
        }

        // Clusters, in logical order: consecutive glyphs with one
        // cluster value.
        let mut g = 0usize;
        while g < self.logical.len() {
            let byte = self.logical[g].cluster;
            let mut e = g;
            let mut advance = 0.0;
            while e < self.logical.len() && self.logical[e].cluster == byte {
                advance += self.logical[e].advance;
                e += 1;
            }
            let space = text[byte as usize..].chars().next().is_some_and(char::is_whitespace);
            self.units.push(Unit { byte, g0: g as u32, g1: e as u32, advance, space });
            g = e;
        }
    }

    /// Shape `text[range]`, one direction and one face, appending its
    /// glyphs to `self.logical` in logical order.
    #[allow(clippy::too_many_arguments)]
    fn shape_run<S: FaceSource + ?Sized>(
        &mut self,
        src: &S,
        text: &str,
        (start, end): (usize, usize),
        rtl: bool,
        face: u8,
        px: f32,
        weight: f32,
    ) {
        let mut buf = self.buffer.take().unwrap_or_default();
        buf.clear();
        buf.set_direction(if rtl { Direction::RightToLeft } else { Direction::LeftToRight });
        for (off, ch) in text[start..end].char_indices() {
            // Pushed one by one with the caller's byte: a substitute
            // (`\u{2212}` → `-`) keeps its source offset. A line
            // separator is shaped as a space and drawn as nothing.
            let sub = if line_separator(ch) { ' ' } else { self.face_of(src, ch, weight).1 };
            buf.add(sub, (start + off) as u32);
        }
        buf.guess_segment_properties();
        let (script, direction) = (buf.script(), buf.direction());

        let Some(Some((_, hb))) = self.faces.get(face as usize) else {
            // No face at all, not even face 0: every character still
            // gets a cluster, with no glyph and no width.
            for (off, ch) in text[start..end].char_indices() {
                self.logical.push(Logical {
                    id: 0,
                    face,
                    cluster: (start + off) as u32,
                    advance: 0.0,
                    dx: 0.0,
                    dy: 0.0,
                    separator: line_separator(ch),
                });
            }
            self.buffer = Some(buf);
            return;
        };
        let upem = hb.units_per_em() as f32;
        let scale = if upem > 0.0 { px / upem } else { 0.0 };
        let pi = match self.plans.iter().position(|p| p.0 == face && p.1 == direction && p.2 == script) {
            Some(i) => i,
            None => {
                self.plans.push((face, direction, script, ShapePlan::new(hb, direction, Some(script), None, &[])));
                self.plans.len() - 1
            }
        };
        let glyphs = rustybuzz::shape_with_plan(hb, &self.plans[pi].3, buf);
        let first = self.logical.len();
        for (info, pos) in glyphs.glyph_infos().iter().zip(glyphs.glyph_positions()) {
            let separator = text[info.cluster as usize..].chars().next().is_some_and(line_separator);
            self.logical.push(Logical {
                id: info.glyph_id as u16,
                face,
                cluster: info.cluster,
                advance: if separator { 0.0 } else { pos.x_advance as f32 * scale },
                dx: pos.x_offset as f32 * scale,
                dy: pos.y_offset as f32 * scale,
                separator,
            });
        }
        if rtl {
            // rustybuzz hands a right-to-left run back in visual order;
            // the breaker wants logical.
            self.logical[first..].reverse();
        }
        self.buffer = Some(glyphs.clear());
    }

    /// The break opportunity before `byte`: `Some(true)` mandatory,
    /// `Some(false)` allowed, `None` none.
    fn opportunity(&self, byte: u32) -> Option<bool> {
        self.breaks.binary_search_by_key(&byte, |b| b.0).ok().map(|i| self.breaks[i].1)
    }

    /// Greedy line breaking over the paragraph's clusters, then each
    /// line placed in visual order into `out`.
    #[allow(clippy::too_many_arguments)]
    fn break_and_place(
        &mut self,
        px: f32,
        wrap: bool,
        max_w: f32,
        para: Para,
        bidi: Option<&BidiInfo<'_>>,
        line: &mut u16,
        out: &mut ShapedRun,
        closing: &mut Vec<Cluster>,
    ) {
        let n = self.units.len();
        if n == 0 {
            // An empty paragraph still occupies a line.
            out.lines.push(ShapedLine { start: para.start as u32, end: para.end as u32, width: 0.0, rtl: para.rtl });
            *line = line.saturating_add(1);
            return;
        }
        let mut start = 0usize;
        while start < n {
            // `ink` is the line's width without the whitespace it ends
            // in: a break is judged on what would be visible.
            let (mut ink, mut width) = (0.0f32, 0.0f32);
            let mut last_ok: Option<usize> = None;
            let (mut end, mut soft) = (n, false);
            for k in start..n {
                let u = self.units[k];
                if k > start {
                    match self.opportunity(u.byte) {
                        Some(true) => {
                            end = k;
                            break;
                        }
                        Some(false) => last_ok = Some(k),
                        None => {}
                    }
                }
                let w = width + u.advance;
                let i = if u.space { ink } else { w };
                if wrap && i > max_w && k > start {
                    // Back to the last opportunity on this line, or —
                    // a word wider than the box — between clusters.
                    end = last_ok.unwrap_or(k);
                    soft = true;
                    break;
                }
                (width, ink) = (w, i);
            }
            self.place_line(px, LineAt { units: (start, end), soft, line: *line }, para, bidi, out, closing);
            *line = line.saturating_add(1);
            start = end;
        }
    }

    /// Place one line's units in visual order, and push to `closing` the
    /// trailing insertion point of every visual run whose logical
    /// successor runs the other way: the next unit on the line, or, after
    /// a paragraph's last unit, the text's end in the paragraph's
    /// direction (the trailing insertion point `shape` adds).
    #[allow(clippy::too_many_arguments)]
    fn place_line(
        &self,
        px: f32,
        at: LineAt,
        para: Para,
        bidi: Option<&BidiInfo<'_>>,
        out: &mut ShapedRun,
        closing: &mut Vec<Cluster>,
    ) {
        let (start, end) = at.units;
        let units = &self.units[start..end];
        let lb = units[0].byte as usize;
        let le = if end < self.units.len() { self.units[end].byte as usize } else { para.end };
        let line = at.line;
        let mut pen = 0.0f32;
        let place = |u: &Unit, rtl: bool, pen: &mut f32, out: &mut ShapedRun| {
            let x0 = *pen;
            let glyphs = &self.logical[u.g0 as usize..u.g1 as usize];
            let mut put = |g: &Logical| {
                if !g.separator {
                    out.glyphs.push(ShapedGlyph {
                        key: GlyphKey { px_bits: px.to_bits(), index: g.id, face: g.face, phase: 0 },
                        x: *pen + g.dx,
                        y: -g.dy,
                        line,
                    });
                }
                *pen += g.advance;
            };
            if rtl {
                glyphs.iter().rev().for_each(&mut put);
            } else {
                glyphs.iter().for_each(&mut put);
            }
            // The leading edge: where reading the cluster starts.
            out.clusters.push(Cluster { byte: u.byte, x: if rtl { *pen } else { x0 }, line, rtl });
        };
        match bidi {
            Some(info) => {
                let (levels, runs) = info.visual_runs(&info.paragraphs[para.index], lb..le);
                for r in runs {
                    let rtl = levels[r.start].is_rtl();
                    let inside = units.iter().filter(|u| r.contains(&(u.byte as usize)));
                    let from = pen;
                    if rtl {
                        inside.clone().rev().for_each(|u| place(u, true, &mut pen, out));
                    } else {
                        inside.clone().for_each(|u| place(u, false, &mut pen, out));
                    }
                    // The run's logically last unit, and what follows it.
                    let Some(k) = units.iter().rposition(|u| r.contains(&(u.byte as usize))).map(|p| start + p) else {
                        continue;
                    };
                    let next = if k + 1 < end {
                        let b = self.units[k + 1].byte as usize;
                        (levels[b].is_rtl() != rtl).then_some(b as u32)
                    } else if end == self.units.len() && rtl != para.rtl {
                        Some(para.end as u32)
                    } else {
                        None
                    };
                    if let Some(byte) = next {
                        // An LTR run ends at its right edge, an RTL one at
                        // its left: where the pen left it, or began it.
                        closing.push(Cluster { byte, x: if rtl { from } else { pen }, line, rtl });
                    }
                }
            }
            None => units.iter().for_each(|u| place(u, false, &mut pen, out)),
        }
        // A soft-wrapped line's whitespace hangs past its edge.
        let hanging: f32 =
            if at.soft { units.iter().rev().take_while(|u| u.space).map(|u| u.advance).sum() } else { 0.0 };
        out.lines.push(ShapedLine { start: lb as u32, end: le as u32, width: pen - hanging, rtl: para.rtl });
    }
}

/// A [`FaceSource`] of one face: every character from face 0, as
/// itself. What [`ComplexText`] shapes with.
#[derive(Clone, Copy)]
struct OneFace(&'static [u8]);

impl FaceSource for OneFace {
    fn face_for(&self, ch: char, _weight: f32) -> (u8, char) {
        (0, ch)
    }

    fn bytes(&self, face: u8) -> Option<&'static [u8]> {
        (face == 0).then_some(self.0)
    }
}

/// A text stack that shapes complex scripts — **for measurement**.
///
/// Since M81/T81.2 a [`Shaper`] over a single-face [`FaceSource`], so it
/// wraps to `max_w` at UAX #14 opportunities (it ignored `max_w`
/// before) and reports clusters **in source order with leading-edge
/// x**, which `GlyphSink`'s contract had always said and this type
/// broke by reporting them in visual order. A right-to-left cluster's
/// `x` is its right edge, and its `rtl` is set; a direction change is
/// two insertion points with one byte (M81/T81.5,
/// [`ShapedRun::clusters`]).
///
/// **Its painter is a `Complex` stack** (M81/T81.3): measured with this
/// and painted with a `Basic` `Canvas::with_font`, a run is laid out in
/// the shaper's box and drawn with fontdue's own advances and order —
/// Arabic unjoined, left to right (`docs/plan-text-correctness.md` §2.1
/// B). A `FontStack` in `Shaping::Complex` measures and paints with the
/// same [`Shaper`] instead, which is what a window uses when its program
/// says `Program::shaping(Shaping::Complex)` (M81/T81.4).
pub struct ComplexText {
    source: OneFace,
    /// For the face's vertical metrics.
    face: rustybuzz::Face<'static>,
    shaper: Shaper,
    run: ShapedRun,
}

impl ComplexText {
    /// Build from font bytes that live as long as the program.
    ///
    /// `&'static [u8]`, deliberately: a face borrows its tables, and a
    /// font loaded into a `Vec` and dropped would leave the shaper
    /// pointing at freed memory. An application that loads a font at
    /// runtime leaks it once, which is what every text stack does with
    /// system faces anyway. `px_per_em` is unused: every call carries
    /// its own size.
    pub fn new(font: &'static [u8], px_per_em: f32) -> Option<Self> {
        let _ = px_per_em;
        Some(ComplexText {
            source: OneFace(font),
            face: rustybuzz::Face::from_slice(font, 0)?,
            shaper: Shaper::new(),
            run: ShapedRun::new(),
        })
    }

    /// The glyphs `text` shapes into, **in visual order**, as `(glyph id,
    /// x)` at `px`, unwrapped — what a painter that consumes this shaper
    /// draws. `x` includes the glyph's own offset.
    ///
    /// What `lumen-static-shell/tests/paint_shaped.rs` compares a
    /// `Complex` stack's painted glyphs with, since M81/T81.3 equal.
    pub fn shaped_glyphs(&self, text: &str, px: f32) -> Vec<(u16, f32)> {
        let mut run = ShapedRun::new();
        Shaper::new().shape(&self.source, text, px, 400.0, f32::INFINITY, &mut run);
        run.glyphs.iter().map(|g| (g.key.index, g.x)).collect()
    }
}

impl TextMeasure for ComplexText {
    fn measure(&mut self, text: &str, size: f32, max_w: f32) -> Size {
        self.shape(text, size, 400.0, max_w, &mut lumen_static::NoSink)
    }

    fn shape(
        &mut self,
        text: &str,
        size: f32,
        weight: f32,
        max_w: f32,
        sink: &mut dyn GlyphSink,
    ) -> Size {
        self.shaper.shape(&self.source, text, size, weight, max_w, &mut self.run);
        for c in &self.run.clusters {
            if sink.is_full() {
                break;
            }
            sink.cluster(*c);
        }
        let lines = self.run.line_count().max(1) as f32;
        Size { w: lumen_static::num::ceil(self.run.width()), h: self.line_height(size) * lines }
    }

    fn line_height(&mut self, size: f32) -> f32 {
        let upem = self.face.units_per_em() as f32;
        if upem <= 0.0 {
            return size * 1.2;
        }
        let m = self.face.ascender() as f32 - self.face.descender() as f32
            + self.face.line_gap() as f32;
        m * size / upem
    }
}
