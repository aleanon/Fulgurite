//! T13.5: the fixed-capacity text storage that cashes B9's associated
//! type out. The interesting cases are all at the capacity boundary and
//! at multi-byte `char`s, because those are where a byte-counting buffer
//! can break its own UTF-8 invariant.

use lumen_static::cx::{Cluster, GlyphSink};
use lumen_static::{ClusterBuf, InlineClusters, InlineText, TextBuffer};

type T8 = InlineText<8>;

#[test]
fn holds_text_and_reports_room() {
    let mut t = T8::new("abc");
    assert_eq!(t.as_str(), "abc");
    assert_eq!((t.len(), t.remaining()), (3, 5));
    t.set_from_str("");
    assert!(t.is_empty());
}

#[test]
fn set_from_str_truncates_on_a_char_boundary() {
    // 'é' is two bytes: a byte-count truncation at 8 would split the
    // fourth one and leave the buffer invalid UTF-8.
    let mut t = T8::new("ééééé"); // 10 bytes
    assert_eq!(t.as_str(), "éééé", "four whole chars, not eight bytes");
    assert_eq!(t.len(), 8);

    // And the degenerate case: one char wider than the whole buffer.
    let mut small: InlineText<2> = InlineText::new("€"); // 3 bytes
    assert_eq!(small.as_str(), "", "a char that cannot fit at all");
    small.set_from_str("ab");
    assert_eq!(small.as_str(), "ab");
    let _ = &mut t;
}

#[test]
fn insert_str_is_all_or_nothing() {
    let mut t = T8::new("abcdef"); // 6 of 8
    t.insert_str(3, "XY"); // exactly fits
    assert_eq!(t.as_str(), "abcXYdef");

    t.insert_str(0, "!"); // full: refused whole
    assert_eq!(t.as_str(), "abcXYdef", "no partial insert");

    let mut u = T8::new("abcdef");
    u.insert_str(3, "XYZ"); // one byte too many: refused whole
    assert_eq!(u.as_str(), "abcdef", "not a two-byte prefix of XYZ");
}

#[test]
fn remove_range_shifts_the_tail() {
    let mut t = T8::new("abcdef");
    t.remove_range(1..3);
    assert_eq!(t.as_str(), "adef");
    t.remove_range(0..4);
    assert!(t.is_empty());
}

#[test]
fn multibyte_insert_and_remove_keep_utf8() {
    let mut t: InlineText<16> = InlineText::new("aé");
    t.insert_str(1, "€"); // 3 bytes, at a char boundary
    assert_eq!(t.as_str(), "a€é");
    // Remove the euro by its byte range, which is 1..4.
    t.remove_range(1..4);
    assert_eq!(t.as_str(), "aé");
}

#[test]
fn clusters_fill_and_report_full() {
    let mut c: InlineClusters<3> = InlineClusters::default();
    assert!(!c.is_full());
    for i in 0..3 {
        c.cluster(Cluster { byte: i, x: i as f32, line: 0 });
    }
    assert!(c.is_full(), "shape() stops early on this");
    assert_eq!(c.as_slice().len(), 3);

    // Past capacity is dropped, not written out of bounds.
    c.cluster(Cluster { byte: 99, x: 9.0, line: 0 });
    assert_eq!(c.as_slice().len(), 3);
    assert_eq!(c.as_slice()[2].byte, 2);

    c.clear();
    assert!(c.as_slice().is_empty());
    assert!(!c.is_full());
}

#[test]
fn cluster_capacity_can_never_be_too_small() {
    // The associated type pairs InlineText<N> with InlineClusters<N>.
    // That is sound because every cluster boundary is a char boundary
    // and the shortest char is one byte, so N bytes of text yield at
    // most N clusters.
    fn clusters_of<S: TextBuffer>() -> usize {
        core::mem::size_of::<S::Clusters>()
    }
    assert!(clusters_of::<InlineText<8>>() > 0);
}

/// The bug the embedded fixture found on its first run, and the reason
/// `TextBuffer::insert_str` returns `bool`.
///
/// `TextInput::insert` advanced its caret by `s.len()` whether or not
/// the storage took the text. That was invisible while every storage
/// was a `String`, which never refuses; against a bounded one the caret
/// walked past the end of the buffer and the next keystroke tripped
/// `InlineText`'s own bounds assertion.
#[test]
fn a_full_buffer_does_not_strand_the_caret() {
    use lumen_static::TextInput;

    let mut f: TextInput<InlineText<8>> = TextInput::default();
    for ch in "abcdefgh".chars() {
        assert!(f.insert(ch.encode_utf8(&mut [0u8; 4])), "room for {ch}");
    }
    assert_eq!(f.as_str(), "abcdefgh");

    // Past capacity: refused, and -- the actual regression -- the caret
    // stays put, so the next call does not index past the end.
    for _ in 0..20 {
        assert!(!f.insert("x"), "full buffer refuses");
    }
    assert_eq!(f.as_str(), "abcdefgh", "nothing was lost or corrupted");

    // Still usable afterwards: delete one, and a fresh insert lands.
    assert!(f.backspace());
    assert!(f.insert("Z"), "room again after a delete");
    assert_eq!(f.as_str(), "abcdefgZ");
}

/// A multi-byte char that does not fit is refused whole, so the buffer
/// never ends up holding half a `char`.
#[test]
fn a_multibyte_char_that_does_not_fit_is_refused_whole() {
    use lumen_static::TextInput;

    let mut f: TextInput<InlineText<8>> = TextInput::default();
    f.insert("abcdefg"); // 7 of 8
    assert!(!f.insert("é"), "two bytes, one free: refused");
    assert_eq!(f.as_str(), "abcdefg");
    assert!(f.insert("z"), "one byte still fits");
    assert_eq!(f.as_str(), "abcdefgz");
}
