//! T14.5: copy and paste, without a clipboard.
//!
//! The framework's half is "what is selected" and "type this in". The
//! clipboard itself is the windowing layer's, because a clipboard is a
//! platform and neither the core nor this crate has one.

use lumen_static::prelude::*;
use lumen_static::{Mods, TextInput};
use lumen_static_shell::Host;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("a")]
    a: TextInput,
    #[widget_id("b")]
    b: Label,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

fn host() -> Host<App> {
    let mut h = Host::new(App::default(), 400, 100);
    h.pump();
    h
}

#[test]
fn copy_is_none_until_something_is_selected() {
    let mut h = host();
    h.app.ui.a.set_str("hello world");
    assert_eq!(h.copy(), None, "nothing focused");

    h.input.focus = Some(1);
    assert_eq!(h.copy(), None, "focused, but nothing selected");

    h.app.ui.a.select_all();
    assert_eq!(h.copy(), Some("hello world"));
}

#[test]
fn copy_from_a_node_with_no_selection_concept_is_none() {
    let mut h = host();
    h.app.ui.b.set("a label");
    // The label is node 2 and has no selection — `None`, not a panic
    // and not the label's text.
    h.input.focus = Some(2);
    assert_eq!(h.copy(), None);
}

#[test]
fn an_out_of_range_focus_does_not_index_out_of_bounds() {
    // `Host.input.focus` is public and a hot reload can shrink a tree
    // under a focus set against the old one (F1.8's rule, one method
    // further on).
    let mut h = host();
    h.input.focus = Some(9999);
    assert_eq!(h.copy(), None);
}

#[test]
fn paste_types_into_the_focused_field() {
    let mut h = host();
    h.input.focus = Some(1);
    assert_eq!(h.paste("hello"), 5);
    assert_eq!(h.app.ui.a.as_str(), "hello");
    assert_eq!(h.app.ui.a.caret(), 5, "the caret followed the text in");
}

#[test]
fn paste_replaces_a_selection_because_it_is_typing() {
    let mut h = host();
    h.app.ui.a.set_str("old text");
    h.input.focus = Some(1);
    h.app.ui.a.select_all();
    h.paste("new");
    assert_eq!(h.app.ui.a.as_str(), "new", "one edit path, so this is free");
}

/// **The case the chunking exists for.** `InputQueue` holds 64 events;
/// a paste is not bounded by that, and a version that filled the queue
/// and stopped would lose the tail silently.
#[test]
fn a_paste_longer_than_the_queue_lands_whole() {
    let mut h = host();
    h.input.focus = Some(1);
    let long: String = std::iter::repeat_n('x', 500).collect();
    assert_eq!(h.paste(&long), 500);
    assert_eq!(h.app.ui.a.as_str().len(), 500, "no character was dropped");
}

#[test]
fn paste_skips_control_characters_but_keeps_newlines() {
    let mut h = host();
    h.input.focus = Some(1);
    // A single-line field refuses the newline itself; what matters here
    // is that `paste` offers it rather than filtering it out, so a
    // `TextArea` would get it.
    h.paste("a\tb\u{7}c");
    assert_eq!(h.app.ui.a.as_str(), "abc", "tab and bell dropped");
}

#[test]
fn copy_after_a_shift_arrow_selection_reads_what_is_highlighted() {
    let mut h = host();
    h.app.ui.a.set_str("hello");
    h.input.focus = Some(1);
    h.post(Event::Key { key: Key::Home, down: true });
    h.post(Event::Modifiers(Mods::SHIFT));
    h.post(Event::Key { key: Key::Right, down: true });
    h.post(Event::Key { key: Key::Right, down: true });
    h.pump();
    assert_eq!(h.copy(), Some("he"), "what a user would see highlighted");
}
