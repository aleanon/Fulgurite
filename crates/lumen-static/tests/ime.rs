//! T14.8: input-method composition.
//!
//! Without this, Chinese, Japanese and Korean input are *impossible*,
//! not merely awkward — a composition never reaches the buffer at all.
//!
//! What is testable headlessly is the whole model: provisional text
//! that replaces itself, commits, cancels, and the one case a bounded
//! storage makes interesting. What is not is the platform handshake,
//! which is twenty lines in `lumen-static-winit` and as untestable as
//! the rest of an event loop.

use lumen_static::event::Composition::{Cancel, Commit, Preedit};
use lumen_static::{InlineText, TextArea, TextInput};

#[test]
fn a_preedit_shows_in_the_buffer_and_is_marked_provisional() {
    // The text *is* in the buffer, so it measures and positions like any
    // other; what makes it provisional is the recorded range.
    let mut f: TextInput = TextInput::new("ab");
    assert!(f.compose(Preedit("ni")));
    assert_eq!(f.as_str(), "abni");
    assert_eq!(f.preedit(), Some(2..4));
    assert_eq!(f.caret(), 4, "the caret follows the composing text");
}

#[test]
fn each_preedit_replaces_the_last() {
    let mut f: TextInput = TextInput::new("ab");
    f.compose(Preedit("n"));
    f.compose(Preedit("ni"));
    f.compose(Preedit("nih"));
    assert_eq!(f.as_str(), "abnih", "not 'abnninih'");
    assert_eq!(f.preedit(), Some(2..5));
}

#[test]
fn commit_replaces_the_provisional_text_and_ends_the_composition() {
    let mut f: TextInput = TextInput::new("ab");
    f.compose(Preedit("nihao"));
    assert!(f.compose(Commit("你好")));
    assert_eq!(f.as_str(), "ab你好", "the committed text, not the romanisation");
    assert_eq!(f.preedit(), None, "nothing provisional is left");
}

#[test]
fn cancel_leaves_the_buffer_as_it_was() {
    let mut f: TextInput = TextInput::new("ab");
    f.compose(Preedit("nihao"));
    assert!(f.compose(Cancel));
    assert_eq!(f.as_str(), "ab");
    assert_eq!(f.preedit(), None);
    assert_eq!(f.caret(), 2, "and the caret back where it started");
}

#[test]
fn an_empty_preedit_clears_it() {
    // winit sends an empty Preedit immediately before every Commit, so
    // this is not an edge case — it is on the normal path.
    let mut f: TextInput = TextInput::new("ab");
    f.compose(Preedit("ni"));
    assert!(f.compose(Preedit("")));
    assert_eq!(f.as_str(), "ab");
    assert_eq!(f.preedit(), None);
}

#[test]
fn composing_with_nothing_in_progress_is_not_a_change() {
    let mut f: TextInput = TextInput::new("ab");
    assert!(!f.compose(Cancel), "nothing to cancel");
    assert!(!f.compose(Preedit("")), "nothing to clear");
    assert_eq!(f.as_str(), "ab");
}

/// **The T13.5 bug in a new place.** A fixed-capacity storage can refuse
/// an insert; a preedit range recorded from what was *offered* rather
/// than what was *taken* would point past the end of the buffer.
#[test]
fn a_storage_that_refuses_the_preedit_records_no_range() {
    let mut f: TextInput<InlineText<4>> = TextInput::new("abcd");
    assert!(!f.compose(Preedit("ni")), "the buffer is full");
    assert_eq!(f.as_str(), "abcd", "and unchanged");
    assert_eq!(f.preedit(), None, "no range pointing past the end");
}

#[test]
fn a_partial_fit_is_refused_whole_and_leaves_no_range() {
    // InlineText::insert_str is all-or-nothing (T13.5), so a two-byte
    // composition into one free byte lands nothing rather than half.
    let mut f: TextInput<InlineText<5>> = TextInput::new("abcd");
    assert!(!f.compose(Preedit("ni")));
    assert_eq!(f.as_str(), "abcd");
    assert_eq!(f.preedit(), None);
}

#[test]
fn typing_over_a_composition_replaces_it() {
    // A composition and a selection can both be live; an ordinary edit
    // must not leave a stale provisional range behind it.
    let mut f: TextInput = TextInput::new("ab");
    f.compose(Preedit("ni"));
    f.compose(Commit("你"));
    assert!(f.insert("!"));
    assert_eq!(f.as_str(), "ab你!");
    assert_eq!(f.preedit(), None);
}

#[test]
fn the_multi_line_widget_composes_too() {
    let mut a: TextArea = TextArea::default();
    a.set_str("one\n");
    a.compose(Preedit("ni"));
    assert_eq!(a.as_str(), "one\nni");
    a.compose(Commit("你"));
    assert_eq!(a.as_str(), "one\n你");
    assert_eq!(a.preedit(), None);
}
