//! Complex-script text — `docs/plan-capabilities.md` T15.8.
//!
//! A [`TextMeasure`] implementation that shapes properly: cluster-
//! correct segmentation, Arabic joining, Indic reordering, and
//! bidirectional runs within a line.
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
//! manifest names it. The core is untouched, `BakedFace` and
//! `FontMetrics` are untouched, and `just thumb` still builds the core
//! with no `std` and no allocator.
//!
//! # What it does and does not do — measurement only
//!
//! It shapes and it orders, and what that changes is **the box and the
//! clusters**: the width a run is laid out in, and where a caret, a
//! selection and a click land. It does **not** change what is painted.
//! Glyphs reach the screen through `lumen-static-shell`'s canvas, which
//! draws the *string* with its own cmap lookup in logical order — so
//! Arabic is painted unjoined and left to right inside a box measured
//! for the joined, right-to-left run (R10-F2,
//! `docs/plan-text-correctness.md` §2.1). This used to say it "changes
//! where the glyphs go", which it never did.
//!
//! **A window cannot install it.** The winit `Session` builds a
//! `FontMetrics`, and a `Program` has no measurer parameter; a `Host`
//! caller passes one to `pump_with` by hand. Shaped paint — the painter
//! consuming this crate's glyphs — is a proposed milestone of its own,
//! and `tests/paint_shaped.rs` is the pin that goes red when it lands.
#![forbid(unsafe_code)]

use lumen_static::cx::{Cluster, GlyphSink, TextMeasure};
use lumen_static::desc::Size;

/// One direction's worth of text, ready to shape.
///
/// A struct rather than seven arguments: clippy is right that a
/// seven-parameter function is unreadable, and naming these makes the
/// bidi loop below say what it is doing.
struct Run<'a> {
    text: &'a str,
    /// Byte offset of this run in the **caller's** string, which is
    /// what makes reported cluster offsets index the caller's text
    /// rather than the run's.
    base: usize,
    px: f32,
    x0: f32,
    line: u16,
    rtl: bool,
}

/// A text stack that shapes complex scripts — **for measurement**.
///
/// **Pair it only with a painter that consumes its clusters, and none
/// exists yet.** Measured with this and painted with `Canvas::with_font`,
/// a run is laid out in the shaper's box and drawn with fontdue's own
/// advances, which are wider for every string (plain ASCII included), so
/// the painter would once have wrapped every label. Since M45/T45.1 the
/// painter lays a run out in its rect plus half a pixel and does not
/// re-wrap one that fits, so the harm left is a horizontal overhang of a
/// few pixels — and Arabic drawn unjoined, left to right
/// (`docs/plan-text-correctness.md` §2.1 B).
pub struct ComplexText {
    face: rustybuzz::Face<'static>,
    /// Fallback for everything geometric: line height, and the sizes of
    /// runs this face cannot cover.
    px_per_em: f32,
}

impl ComplexText {
    /// Build from font bytes that live as long as the program.
    ///
    /// `&'static [u8]`, deliberately: a face borrows its tables, and a
    /// font loaded into a `Vec` and dropped would leave the shaper
    /// pointing at freed memory. An application that loads a font at
    /// runtime leaks it once, which is what every text stack does with
    /// system faces anyway.
    pub fn new(font: &'static [u8], px_per_em: f32) -> Option<Self> {
        Some(ComplexText { face: rustybuzz::Face::from_slice(font, 0)?, px_per_em })
    }

    /// Shape one run of a single direction, appending clusters.
    ///
    /// Returns the advance width. The **byte offsets are the caller's**
    /// — `rustybuzz` reports cluster values that are the indices it was
    /// given, and this passes the original slice, so a caret computed
    /// from these indexes the application's own string. Getting that
    /// wrong is how a caret lands mid-character, which is the bug
    /// `TextMeasure::shape`'s contract exists to prevent.
    fn shape_run(&self, run: Run<'_>, sink: &mut dyn GlyphSink) -> f32 {
        let Run { text, base, px, x0, line, rtl } = run;
        let glyphs = self.shape_glyphs(text, rtl);

        let upem = self.face.units_per_em() as f32;
        let scale = if upem > 0.0 { px / upem } else { 0.0 };
        let mut x = x0;
        for (info, pos) in glyphs.glyph_infos().iter().zip(glyphs.glyph_positions()) {
            if !sink.is_full() {
                sink.cluster(Cluster {
                    byte: (base + info.cluster as usize) as u32,
                    x,
                    line,
                });
            }
            x += pos.x_advance as f32 * scale;
        }
        let _ = self.px_per_em;
        x - x0
    }
}

impl ComplexText {
    /// The glyphs `text` shapes into, **in visual order**, as `(glyph id,
    /// x)` at `px` — what a painter that consumed this shaper would draw.
    ///
    /// None does: glyphs reach the screen through
    /// `lumen-static-shell`'s canvas, which draws the *string* with its
    /// own cmap lookup (`docs/plan-text-correctness.md` §2.1). This is
    /// what `tests/paint_shaped.rs` compares the painted glyphs with, so
    /// the claim "the shaper is right, the painter does not use it" is a
    /// checked one.
    pub fn shaped_glyphs(&self, text: &str, px: f32) -> Vec<(u16, f32)> {
        let mut out = Vec::new();
        let upem = self.face.units_per_em() as f32;
        let scale = if upem > 0.0 { px / upem } else { 0.0 };
        let info = unicode_bidi::BidiInfo::new(text, None);
        let mut x = 0.0f32;
        for para in &info.paragraphs {
            let (levels, ranges) = info.visual_runs(para, para.range.clone());
            for range in ranges {
                let glyphs = self.shape_glyphs(&text[range.clone()], levels[range.start].is_rtl());
                for (g, p) in glyphs.glyph_infos().iter().zip(glyphs.glyph_positions()) {
                    out.push((g.glyph_id as u16, x));
                    x += p.x_advance as f32 * scale;
                }
            }
        }
        out
    }

    /// One direction's run through rustybuzz — the one call both
    /// `shape_run` and [`ComplexText::shaped_glyphs`] make.
    fn shape_glyphs(&self, text: &str, rtl: bool) -> rustybuzz::GlyphBuffer {
        let mut buf = rustybuzz::UnicodeBuffer::new();
        buf.push_str(text);
        buf.set_direction(if rtl {
            rustybuzz::Direction::RightToLeft
        } else {
            rustybuzz::Direction::LeftToRight
        });
        rustybuzz::shape(&self.face, &[], buf)
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
        _weight: f32,
        _max_w: f32,
        sink: &mut dyn GlyphSink,
    ) -> Size {
        if text.is_empty() {
            // One cluster: the caret position in an empty field. The
            // same contract every other measurer here honours.
            sink.cluster(Cluster { byte: 0, x: 0.0, line: 0 });
            return Size { w: 0.0, h: self.line_height(size) };
        }

        // Split into runs of one direction each, in *visual* order.
        // This is the half `BakedFace` and `FontMetrics` cannot do: a
        // Hebrew phrase inside an English sentence is two runs that
        // read in opposite directions and must be laid out as such.
        let info = unicode_bidi::BidiInfo::new(text, None);
        let mut x = 0.0f32;
        for para in &info.paragraphs {
            let (levels, ranges) = info.visual_runs(para, para.range.clone());
            for range in ranges {
                let rtl = levels[range.start].is_rtl();
                let run = &text[range.clone()];
                x += self.shape_run(
                    Run { text: run, base: range.start, px: size, x0: x, line: 0, rtl },
                    sink,
                );
            }
        }
        if !sink.is_full() {
            // The insertion point after the last character.
            sink.cluster(Cluster { byte: text.len() as u32, x, line: 0 });
        }
        Size { w: lumen_static::num::ceil(x), h: self.line_height(size) }
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
