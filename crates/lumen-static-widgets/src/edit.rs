//! Where an editing step lands — M45/T45.5, R10-F7.
//!
//! Both editors, `TextInput` and `TextArea`, step their caret and delete
//! through these two functions, and the three composites edit through
//! them. They used to step by `char`, so Backspace after a combining
//! accent, a ZWJ family, a flag or a skin-tone emoji left half of it:
//! 0 of 40 cases whole in `lumen-static-shell/tests/graphemes.rs`
//! (`docs/results-text.md` T45.0).
//!
//! **By extended grapheme cluster where there is an allocator**, from
//! `unicode-segmentation`: the unit a reader sees as one character. It is
//! `no_std` and allocation-free, and it is behind this crate's `alloc`
//! feature all the same, so the leaf profile — no `std`, no allocator,
//! `fixtures/embedded` — links none of its tables and keeps stepping by
//! `char` (`docs/results-embedded.md`). A `char` step still never splits
//! a character's bytes, which is what keeps a slice from panicking.
//!
//! Where a shaper's clusters exist they still *place* the caret; these
//! decide where editing stops.

/// The boundary before `at` in `s`, if any.
pub(crate) fn prev_boundary(s: &str, at: usize) -> Option<usize> {
    let at = at.min(s.len());
    if at == 0 {
        return None;
    }
    #[cfg(feature = "alloc")]
    {
        let mut c = unicode_segmentation::GraphemeCursor::new(at, s.len(), true);
        if let Ok(Some(b)) = c.prev_boundary(s, 0) {
            return Some(b);
        }
    }
    s[..at].char_indices().next_back().map(|(i, _)| i)
}

/// The last boundary at or before `at` in `s` — M80/T80.3, for
/// [`crate::fit::cut`]: a byte offset snapped **down** to where a reader's
/// character starts, so a cut there never keeps half a ZWJ family, a flag
/// or a skin tone. `at` past the end is the end; 0 is always a boundary.
///
/// By grapheme where there is an allocator and by `char` in the leaf
/// tier, the module's rule: a `char` snap still never splits a
/// character's bytes, so the slice it bounds cannot panic.
pub(crate) fn floor_boundary(s: &str, at: usize) -> usize {
    let mut at = at.min(s.len());
    while !s.is_char_boundary(at) {
        at -= 1;
    }
    #[cfg(feature = "alloc")]
    {
        let mut c = unicode_segmentation::GraphemeCursor::new(at, s.len(), true);
        if let Ok(false) = c.is_boundary(s, 0) {
            if let Ok(Some(b)) = c.prev_boundary(s, 0) {
                return b;
            }
        }
    }
    at
}

/// The boundary after `at` in `s`, if any.
pub(crate) fn next_boundary(s: &str, at: usize) -> Option<usize> {
    if at >= s.len() {
        return None;
    }
    #[cfg(feature = "alloc")]
    {
        let mut c = unicode_segmentation::GraphemeCursor::new(at, s.len(), true);
        if let Ok(Some(b)) = c.next_boundary(s, 0) {
            return Some(b);
        }
    }
    s.get(at..)?.chars().next().map(|c| at + c.len_utf8())
}

/// What a paste keeps of `text` — M45/T45.7: every non-control
/// character, and, when `lines` is set, each line ending as one `'\n'`
/// (`\r\n` and a lone `\r` normalised). A single-line field passes
/// `false` and keeps no line break at all.
///
/// Borrowed when nothing is dropped, which is the common paste; an owned
/// copy otherwise. The leaf profile has no allocator to copy into, so
/// there it keeps the text up to the first character it would drop.
pub(crate) fn printable(text: &str, lines: bool) -> Str<'_> {
    let clean = |c: char| !c.is_control() || (lines && c == '\n');
    if !text.contains('\r') && text.chars().all(clean) {
        return Str::Borrowed(text);
    }
    #[cfg(feature = "alloc")]
    {
        let mut out = alloc::string::String::with_capacity(text.len());
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\r' if lines => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    out.push('\n');
                }
                c if clean(c) => out.push(c),
                _ => {}
            }
        }
        Str::Owned(out)
    }
    #[cfg(not(feature = "alloc"))]
    {
        let end = text.char_indices().find(|&(_, c)| c == '\r' || !clean(c)).map_or(text.len(), |(i, _)| i);
        Str::Borrowed(&text[..end])
    }
}

/// A borrowed or, with an allocator, owned string — `Cow` without
/// requiring `alloc`.
pub(crate) enum Str<'a> {
    Borrowed(&'a str),
    #[cfg(feature = "alloc")]
    Owned(alloc::string::String),
}

impl core::ops::Deref for Str<'_> {
    type Target = str;
    fn deref(&self) -> &str {
        match self {
            Str::Borrowed(s) => s,
            #[cfg(feature = "alloc")]
            Str::Owned(s) => s,
        }
    }
}

// --- Words, clicks and line ends — M45/T45.6, R10-F14 -----------------

/// The modifier that makes Left and Right move by word: Alt on macOS,
/// Ctrl elsewhere, which is each platform's own convention.
pub(crate) const WORD: lumen_static::event::Mods =
    if cfg!(target_os = "macos") { lumen_static::event::Mods::ALT } else { lumen_static::event::Mods::CTRL };

/// The modifier that makes Home and End go to the buffer's ends rather
/// than the line's: Cmd on macOS, Ctrl elsewhere.
pub(crate) const JUMP: lumen_static::event::Mods =
    if cfg!(target_os = "macos") { lumen_static::event::Mods::META } else { lumen_static::event::Mods::CTRL };

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Walk `s`'s word segments in order as `(start, end, is_word)`, until
/// `f` answers `true`.
///
/// **By Unicode word boundary where there is an allocator** (UAX #29,
/// `unicode-segmentation`'s `split_word_bound_indices`), so `don't` and
/// `3.14` are one word each; a segment is a word when it holds a letter,
/// a digit or `_`. The leaf profile has no tables linked, as for
/// graphemes above, and groups runs of word characters instead.
fn segments(s: &str, mut f: impl FnMut(usize, usize, bool) -> bool) {
    #[cfg(feature = "alloc")]
    {
        use unicode_segmentation::UnicodeSegmentation;
        for (i, w) in s.split_word_bound_indices() {
            if f(i, i + w.len(), w.chars().any(is_word_char)) {
                return;
            }
        }
    }
    #[cfg(not(feature = "alloc"))]
    {
        let mut chars = s.char_indices().peekable();
        while let Some((i, c)) = chars.next() {
            let word = is_word_char(c);
            let mut end = i + c.len_utf8();
            while let Some(&(j, d)) = chars.peek() {
                if !word || !is_word_char(d) {
                    break;
                }
                end = j + d.len_utf8();
                chars.next();
            }
            if f(i, end, word) {
                return;
            }
        }
    }
}

/// Word-Right from `at`: the end of the next word, skipping whatever
/// separates it — or the buffer's end. `None` at the end already.
pub(crate) fn next_word(s: &str, at: usize) -> Option<usize> {
    if at >= s.len() {
        return None;
    }
    let mut to = s.len();
    segments(s, |_, b, word| {
        let hit = word && b > at;
        if hit {
            to = b;
        }
        hit
    });
    Some(to)
}

/// Word-Left from `at`: the start of the word the caret is in or the one
/// before it — or 0. `None` at the start already.
pub(crate) fn prev_word(s: &str, at: usize) -> Option<usize> {
    let at = at.min(s.len());
    if at == 0 {
        return None;
    }
    let mut to = 0;
    segments(s, |a, _, word| {
        if a >= at {
            return true;
        }
        if word {
            to = a;
        }
        false
    });
    Some(to)
}

/// The segment a double click at `at` selects: the word under it. A
/// click just past a word's last character lands on the boundary after
/// it, and selects that word rather than the space beside it.
pub(crate) fn word_at(s: &str, at: usize) -> core::ops::Range<usize> {
    let mut prev: Option<(usize, usize, bool)> = None;
    let mut got = None;
    segments(s, |a, b, word| {
        if a <= at && at < b {
            got = Some(match prev {
                Some((pa, pb, true)) if !word && a == at && pb == at => pa..pb,
                _ => a..b,
            });
            return true;
        }
        prev = Some((a, b, word));
        false
    });
    got.or_else(|| prev.map(|(a, b, _)| a..b)).unwrap_or(0..0)
}

/// The `\n`-delimited line holding `at`, without its line break.
pub(crate) fn line_at(s: &str, at: usize) -> core::ops::Range<usize> {
    let at = at.min(s.len());
    let lo = s[..at].rfind('\n').map_or(0, |i| i + 1);
    let hi = s[at..].find('\n').map_or(s.len(), |i| at + i);
    lo..hi
}
