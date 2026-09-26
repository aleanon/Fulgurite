//! A bitmap face that measures and draws without an allocator —
//! `docs/plan-embedded.md` T13.7.
//!
//! `FontMetrics` is `fontdue`: it needs `alloc`, it is a `std` crate,
//! and it lives in `lumen-static-shell`, which M13 keeps `std`. So until
//! this module the M13 device had **no measurer at all**, and T13.3's
//! fixture was laying out a `Label` whose glyphs nothing could draw.
//!
//! A [`BakedFace`] is advances and coverage bitmaps in `const` data,
//! indexed by a sorted glyph table. That is what every embedded GUI
//! does, and it is the honest answer for a device whose font lives in
//! flash. `fixtures/bake-font` generates one from a TTF; the data
//! belongs in the *application*, not here — the machinery is generic,
//! the face is the app's, and a GUI framework should not ship a font.
//!
//! **What it gives up, plainly.** One size per baked face: other sizes
//! are advanced by a linear scale, which is not what a hinted rasteriser
//! would produce. No hinting. No complex-script shaping — already
//! recorded as deferred in `docs/text-render-adoption-decision.md`; the
//! first two are M13's and are recorded there too.

use crate::cx::{Cluster, GlyphSink, TextMeasure};
use crate::desc::Size;

/// One baked glyph: its advance, and where its coverage lives in the
/// face's blob.
///
/// `off` indexes [`BakedFace::cov`] rather than each glyph owning a
/// slice, because that is how the data lands in flash — one contiguous
/// blob and a table of offsets, not 95 separate relocations.
#[derive(Clone, Copy, Debug)]
pub struct Glyph {
    pub ch: char,
    /// Pen advance in px at the face's baked size.
    pub advance: f32,
    /// Coverage bitmap dimensions.
    pub w: u8,
    pub h: u8,
    /// Left bearing, and the top edge relative to the baseline
    /// (positive up). A painter places the bitmap at
    /// `(pen + left, baseline - top)`.
    pub left: i32,
    pub top: i32,
    /// Byte offset of this glyph's `w * h` coverage in the face blob.
    pub off: u32,
}

/// A pre-rasterised face: glyph metrics and 8-bit coverage in `const`
/// data.
///
/// Everything is a `&'static` slice, so a face costs no RAM at all — it
/// is addressed in flash where the linker put it.
#[derive(Clone, Copy, Debug)]
pub struct BakedFace {
    /// The size this face was rasterised at. Measuring at another size
    /// scales advances linearly; see the module note.
    pub px: f32,
    pub line_height: f32,
    /// Baseline offset from the top of a line, in px at `px`.
    pub ascent: f32,
    /// Sorted by `ch`, so lookup is a binary search.
    pub glyphs: &'static [Glyph],
    /// Every glyph's coverage, concatenated.
    pub cov: &'static [u8],
}

impl BakedFace {
    /// The glyph for `ch`, or `None` if this face was not baked with it.
    ///
    /// `Option`, not a substituted box: a face is a fixed set, and a
    /// caller that silently draws a placeholder cannot tell "this font
    /// lacks the character" from "this character is a box".
    pub fn glyph(&self, ch: char) -> Option<&Glyph> {
        let i = self.glyphs.binary_search_by(|g| g.ch.cmp(&ch)).ok()?;
        self.glyphs.get(i)
    }

    /// A glyph's `w * h` coverage rows, or an empty slice for a glyph
    /// with no ink (a space).
    pub fn coverage(&self, g: &Glyph) -> &[u8] {
        let start = g.off as usize;
        let end = start + g.w as usize * g.h as usize;
        self.cov.get(start..end).unwrap_or(&[])
    }

    /// Multiplier from the baked size to `size`.
    fn k(&self, size: f32) -> f32 {
        if self.px > 0.0 {
            size / self.px
        } else {
            1.0
        }
    }

    /// Advance of `ch` at `size`, or 0 for a glyph this face lacks.
    pub fn advance(&self, ch: char, size: f32) -> f32 {
        self.glyph(ch).map_or(0.0, |g| g.advance * self.k(size))
    }
}

impl TextMeasure for BakedFace {
    fn measure(&mut self, text: &str, size: f32, max_w: f32) -> Size {
        self.shape(text, size, 400.0, max_w, &mut crate::cx::NoSink)
    }

    /// One face, so weight is ignored — the same choice `BlockMetrics`
    /// makes, and for the same reason: a baked face has exactly the
    /// weights someone baked.
    fn measure_weighted(&mut self, text: &str, size: f32, weight: f32, max_w: f32) -> Size {
        let _ = weight;
        self.measure(text, size, max_w)
    }

    /// Measure and report clusters, wrapping at `max_w`.
    ///
    /// Wraps at the last space that fits, and hard-breaks a word with no
    /// break in it — the minimum a text box needs to be usable. **True
    /// since M45/T45.12** (R10-F10): until then an overflow rewound only
    /// the widest-line counter, so the word's glyphs already placed
    /// stayed on the old line and the break landed mid-word, with the
    /// moved half-word counted in the width. A wrapped line's width is
    /// its last word's end, not its trailing space
    /// (`fixtures/embedded/tests/wrap.rs`). It is
    /// not fontdue's line breaker and does not claim to match it; what
    /// T13.7's gate checks is that the *glyph coverage* agrees, which is
    /// the part a bitmap face can be wrong about silently.
    fn shape(
        &mut self,
        text: &str,
        size: f32,
        weight: f32,
        max_w: f32,
        sink: &mut dyn GlyphSink,
    ) -> Size {
        let _ = weight;
        let lh = self.line_height(size);
        if text.is_empty() {
            // One cluster: the caret position in an empty field. Same
            // contract as the desktop measurer.
            sink.cluster(Cluster { byte: 0, x: 0.0, line: 0 });
            return Size { w: 0.0, h: lh };
        }

        let wrap = max_w > 0.0;
        // `ink` is where the current line's last non-space glyph ends: a
        // line finished by a wrap is as wide as its words, not its
        // trailing space. `widest` is updated only when a line finishes,
        // so a word that moves to the next line never widens this one.
        let (mut x, mut ink, mut line, mut widest) = (0.0f32, 0.0f32, 0u16, 0.0f32);
        let mut prev_space = true;

        let mut chars = text.char_indices();
        while let Some((byte, ch)) = chars.next() {
            let adv = self.advance(ch, size);
            if wrap && ch != ' ' && x > 0.0 {
                // A word that would overflow moves whole to a new line —
                // measured in a first pass over its own bytes, since
                // there is no allocator to buffer its clusters in
                // (M45/T45.12). A word wider than the box still breaks
                // at a glyph: the second test, once it is on its own
                // line.
                let breaks = if prev_space {
                    // A clone of the iterator, not a slice and a `split`:
                    // those link the pattern machinery and a panicking
                    // char-boundary check into a firmware that had
                    // neither, and cost 4.5 KB of `.text` when tried.
                    let mut ww = adv;
                    for (_, c) in chars.clone() {
                        if c == ' ' {
                            break;
                        }
                        ww += self.advance(c, size);
                    }
                    x + ww > max_w
                } else {
                    x + adv > max_w
                };
                if breaks {
                    widest = widest.max(ink);
                    x = 0.0;
                    ink = 0.0;
                    line += 1;
                }
            }
            if !sink.is_full() {
                sink.cluster(Cluster { byte: byte as u32, x, line });
            }
            x += adv;
            if ch != ' ' {
                ink = x;
            }
            prev_space = ch == ' ';
        }
        // The last line keeps its trailing spaces, as an unwrapped run
        // always did: a caret after "abc " sits past the space.
        widest = widest.max(x);
        // The insertion point after the last character.
        if !sink.is_full() {
            sink.cluster(Cluster { byte: text.len() as u32, x, line });
        }
        Size { w: crate::num::ceil(widest), h: crate::num::ceil(lh * (line as f32 + 1.0)) }
    }

    fn line_height(&mut self, size: f32) -> f32 {
        self.line_height * self.k(size)
    }
}
