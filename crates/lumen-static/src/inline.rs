//! Fixed-capacity text storage — `docs/plan-embedded.md` T13.5.
//!
//! B9 made [`TextBuffer::Clusters`] an associated type of the *storage*
//! precisely so a fixed-capacity buffer could pair with a fixed-capacity
//! cluster array, and then nothing ever inhabited it: both `TextBuffer`
//! impls needed an allocator and the only [`ClusterBuf`] was
//! `Vec<Cluster>`. [`InlineText`] and [`InlineClusters`] are what cash
//! that design out, and they are what let the `alloc`-less build name a
//! default for `TextInput<S>` at all — a default type parameter cannot
//! be removed by a `cfg`, so without them T13.4 has nothing to say.
//!
//! Both are `const`-sized and neither can allocate. Both are also
//! *bounded*, which a `String` is not, so each has an overflow rule and
//! the two rules deliberately differ — see [`InlineText::insert_str`].

use core::ops::Range;

use crate::cx::{Cluster, GlyphSink};
use crate::text::{ClusterBuf, TextBuffer};

/// UTF-8 text in `N` inline bytes.
///
/// `N` is a **byte** capacity, not a character count: a 64-byte buffer
/// holds 64 ASCII characters or 16 four-byte ones. The type keeps the
/// bytes valid UTF-8 at every point, and every method that could break
/// that refuses instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InlineText<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> Default for InlineText<N> {
    fn default() -> Self {
        InlineText { buf: [0; N], len: 0 }
    }
}

impl<const N: usize> InlineText<N> {
    /// Capacity in bytes.
    pub const CAP: usize = N;

    /// From a `&str`, truncated at a `char` boundary if it does not fit.
    pub fn new(s: &str) -> Self {
        let mut t = Self::default();
        t.set_from_str(s);
        t
    }

    pub fn as_str(&self) -> &str {
        // Every mutation below preserves UTF-8, so this validation
        // always succeeds. It is `unwrap_or` rather than `expect` so a
        // bug degrades to empty text instead of taking the frame down,
        // and `debug_assert` so the bug is still loud where it matters.
        // Validating is O(len) on a buffer of at most `N` bytes.
        let s = core::str::from_utf8(&self.buf[..self.len]);
        debug_assert!(s.is_ok(), "InlineText broke its own UTF-8 invariant");
        s.unwrap_or("")
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Bytes still free.
    pub fn remaining(&self) -> usize {
        N - self.len
    }

    /// Drop the contents, keeping the capacity (there is nothing else
    /// to keep). Mirrors `String::clear`, so a scratch buffer reads the
    /// same in both builds.
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// The largest `n <= max` that is a `char` boundary of `s`.
    fn fit(s: &str, max: usize) -> usize {
        let mut n = max.min(s.len());
        while n > 0 && !s.is_char_boundary(n) {
            n -= 1;
        }
        n
    }
}

impl<const N: usize> AsRef<str> for InlineText<N> {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl<const N: usize> TextBuffer for InlineText<N> {
    /// One cluster per byte is the worst case — every cluster boundary
    /// is a `char` boundary, and the shortest `char` is one byte — so
    /// `N` clusters can never be too few for `N` bytes of text.
    type Clusters = InlineClusters<N>;

    /// Replace the contents, **truncating** at a `char` boundary if `s`
    /// is longer than `N`.
    ///
    /// Truncates where [`insert_str`](Self::insert_str) refuses, and the
    /// asymmetry is deliberate. This is the binding path: a formatted
    /// value too long for the field should show its prefix, the way a
    /// narrow column does. `insert_str` is the typing path, where a
    /// silent partial insert would land text the user did not type.
    fn set_from_str(&mut self, s: &str) {
        let n = Self::fit(s, N);
        self.buf[..n].copy_from_slice(&s.as_bytes()[..n]);
        self.len = n;
    }

    /// Insert at byte offset `at`, or **do nothing** if `s` does not fit
    /// whole. Returns whether it was stored.
    ///
    /// All-or-nothing: a partial insert would either split a `char` —
    /// breaking the UTF-8 invariant — or land a prefix of what the user
    /// typed. The caller uses the returned `bool` to decide whether to
    /// advance its caret; before the trait returned one, a refused
    /// insert left `TextInput`'s caret past the end of the text and the
    /// `debug_assert` below fired on the next keystroke.
    fn insert_str(&mut self, at: usize, s: &str) -> bool {
        debug_assert!(at <= self.len, "insert past the end");
        if s.len() > self.remaining() || at > self.len {
            return false;
        }
        self.buf.copy_within(at..self.len, at + s.len());
        self.buf[at..at + s.len()].copy_from_slice(s.as_bytes());
        self.len += s.len();
        true
    }

    fn remove_range(&mut self, range: Range<usize>) {
        debug_assert!(range.end <= self.len, "remove past the end");
        if range.start > range.end || range.end > self.len {
            return;
        }
        self.buf.copy_within(range.end..self.len, range.start);
        self.len -= range.end - range.start;
    }
}

/// Appends, truncating at a `char` boundary when the buffer fills.
///
/// Returning `Err` on a short write is what makes truncation visible to
/// `core::fmt`, which stops formatting at the first error — so the tail
/// of a too-long `format_args!` costs nothing to not write. The bytes
/// that did fit stay, which is the same rule
/// [`set_from_str`](InlineText::set_from_str) follows.
///
/// This is what lets [`crate::bind::format_into`] run without an
/// allocator: its reusable scratch buffer is a `thread_local!`, which
/// needs `std`, so a `no_std` build formats into one of these on the
/// stack instead.
impl<const N: usize> core::fmt::Write for InlineText<N> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let n = Self::fit(s, self.remaining());
        self.buf[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        if n == s.len() {
            Ok(())
        } else {
            Err(core::fmt::Error)
        }
    }
}

/// Up to `N` shaped cluster positions, inline.
///
/// The [`GlyphSink`] half of the pair. It overrides
/// [`GlyphSink::is_full`], which is the whole reason that method exists:
/// [`crate::cx::TextMeasure::shape`] checks it and stops early, so a run
/// too long for the buffer costs nothing to shape past the point where
/// it would be discarded.
#[derive(Clone, Copy, Debug)]
pub struct InlineClusters<const N: usize> {
    buf: [Cluster; N],
    len: usize,
}

impl<const N: usize> Default for InlineClusters<N> {
    /// Through a `const` element, not a literal in the repeat: when
    /// M81/T81.2 gave `Cluster` its `rtl` field, the literal form stopped
    /// lowering to a clear and `TextInput::default` came out of line at
    /// 602 B, +150 B of firmware `.text`; the `const` form is a clear
    /// again (`docs/results-embedded.md`, M81/T81.2).
    fn default() -> Self {
        const EMPTY: Cluster = Cluster { byte: 0, x: 0.0, line: 0, rtl: false };
        InlineClusters { buf: [EMPTY; N], len: 0 }
    }
}

impl<const N: usize> GlyphSink for InlineClusters<N> {
    fn cluster(&mut self, c: Cluster) {
        if self.len < N {
            self.buf[self.len] = c;
            self.len += 1;
        }
    }

    fn is_full(&self) -> bool {
        self.len == N
    }
}

impl<const N: usize> ClusterBuf for InlineClusters<N> {
    fn clear(&mut self) {
        self.len = 0;
    }

    fn as_slice(&self) -> &[Cluster] {
        &self.buf[..self.len]
    }
}
