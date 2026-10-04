//! Where a one-line text is cut to fit — M80/T80.3
//! (`docs/plan-text-fit.md`, "`Clip` and `Ellipsis` are one line, cut in
//! layout, by grapheme").
//!
//! [`cut`] is the one search `Label` and `AppBar` share: the longest
//! prefix of the text's first line whose width, plus the `…`'s when there
//! is one, fits the room. `AppBar::fit_title` used to do this itself,
//! over **char** boundaries, so a title whose fitting prefix ended inside
//! a ZWJ family, a flag or a skin tone kept half a character before the
//! `…` (`crates/lumen-static-shell/tests/ellipsis_graphemes.rs`, which
//! pinned 20, 10 and 10 such cuts before this module existed).
//!
//! **A binary search over byte offsets, snapped down to a grapheme
//! boundary** with `edit::floor_boundary`, so it walks nothing and
//! allocates nothing: it runs in layout, in steady state, and in the leaf
//! tier, where the snap is to `char` boundaries instead (the leaf profile
//! has no `unicode-segmentation`; `edit.rs`'s tier rule). It measures at
//! most `⌈log2(len)⌉ + 3` strings per cut, which is K80d's bound and
//! `crates/lumen-static-shell/tests/fit_cost.rs`'s gate.
//!
//! **The cut is logical**, in byte order. A right-to-left run's `…`
//! belongs on its visual start, which is M81's.

use lumen_static::cx::TextMeasure;

/// The mark an `Ellipsis` cut ends in.
pub const ELLIPSIS: &str = "…";

/// Where `text` is cut to fit `room` px on one line at `px`/`weight`:
/// `Some((end, width))`, the prefix `&text[..end]` and its measured width,
/// or `None` when the whole text is one line that fits and nothing is cut.
///
/// `mark` reserves room for [`ELLIPSIS`] (`TextOverflow::Ellipsis`);
/// without it (`Clip`) the prefix may take the whole room. **A hard line
/// break ends the visible line and counts as overflow**, so a text with a
/// `'\n'` is always cut, at or before it. A trailing space is dropped
/// before the mark ("Inbox …" is a gap), and `end` is always a grapheme
/// boundary (a `char` boundary in the leaf tier), so the slice is valid
/// and keeps whole characters.
///
/// Measures the first line once, the mark once (when `mark`), and one
/// prefix per halving of the line's byte length: at most
/// `⌈log2(len)⌉ + 3` calls (K80d).
pub fn cut(m: &mut dyn TextMeasure, text: &str, px: f32, weight: f32, room: f32, mark: bool) -> Option<(usize, f32)> {
    // `get`, never an index, throughout: every offset here is a boundary
    // by construction, and an index would link `str`'s slice-error path
    // (its `Debug` escaping and the Unicode printable tables) into the leaf
    // firmware for a panic that cannot happen — 3 KB of `.rodata`
    // measured on the first build (`docs/results-embedded.md` M80/T80.3).
    let line = match text.find('\n') {
        Some(n) => text.get(..n).unwrap_or(text),
        None => text,
    };
    let mut w = |s: &str| if s.is_empty() { 0.0 } else { m.measure_weighted(s, px, weight, f32::INFINITY).w };
    let full = w(line);
    if line.len() == text.len() && full <= room {
        return None;
    }
    let e = if mark { w(ELLIPSIS) } else { 0.0 };
    // The prefix a raw byte offset `b` stands for: snapped down to a
    // grapheme boundary, trailing space dropped. Its length never shrinks
    // as `b` grows, so its width never does either and the search below
    // is a search.
    let at = |b: usize| line.get(..b).unwrap_or("");
    let prefix = |b: usize| at(crate::edit::floor_boundary(line, b)).trim_end().len();
    // The largest `b` in 0..=len whose prefix fits. `lo` is known to fit
    // (the empty prefix, width 0, is taken to fit: a box narrower than
    // the mark still shows the mark, as every platform's does).
    let (mut lo, mut hi, mut lo_w) = (0usize, line.len(), 0.0f32);
    if full + e <= room {
        // Only a hard break made this a cut: the whole first line fits.
        let p = prefix(line.len());
        return Some((p, if p == line.len() { full } else { w(at(p)) }));
    }
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        let width = w(at(prefix(mid)));
        if width + e <= room {
            lo = mid;
            lo_w = width;
        } else {
            hi = mid - 1;
        }
    }
    Some((prefix(lo), lo_w))
}
