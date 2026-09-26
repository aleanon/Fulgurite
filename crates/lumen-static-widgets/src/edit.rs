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
