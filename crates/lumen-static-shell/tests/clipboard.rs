//! T14.5: copy and paste, without a clipboard.
//!
//! The framework's half is "what is selected" and "type this in". The
//! clipboard itself is the windowing layer's, because a clipboard is a
//! platform and neither the core nor this crate has one.

use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;
use lumen_static::{Mods};
use lumen_static_widgets::{NumberInput, TextArea, TextInput};
use lumen_static_shell::Host;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("a")]
    a: TextInput,
    #[widget_id("b")]
    b: Label,
}

/// A text area and a number field, each **alone** in its own tree: with a
/// focusable sibling, an arrow key moves focus instead of editing, which
/// is M44's to fix, and a case here must not depend on it.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct AreaUi {
    #[widget_id("area")]
    area: TextArea,
}

#[derive(DeriveApp, Default)]
struct AreaApp {
    #[tree]
    ui: AreaUi,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct NumberUi {
    #[widget_id("number")]
    number: NumberInput,
}

#[derive(DeriveApp, Default)]
struct NumberApp {
    #[tree]
    ui: NumberUi,
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
fn paste_skips_tab_and_bell() {
    let mut h = host();
    h.input.focus = Some(1);
    h.paste("a\tb\u{7}c");
    assert_eq!(h.app.ui.a.as_str(), "abc", "tab and bell dropped");
}

/// **The newline test, against a widget that keeps newlines** — M45/T45.0
/// item 7 (R10-F6). This used to paste into a single-line field, where a
/// dropped newline and a kept one look the same; a `TextArea` is where
/// `paste` offering the newline and the widget keeping it are both
/// visible.
#[test]
fn paste_keeps_newlines_in_a_text_area() {
    let mut h: Host<AreaApp> = Host::new(AreaApp::default(), 400, 100);
    h.pump();
    h.input.focus = Some(1);
    let n = h.paste("a\nb\tc");
    println!("pasted \"a\\nb\\tc\": area holds {:?}, count {n} (want \"a\\nbc\", 4)", h.app.ui.area.as_str());
    assert_eq!(h.app.ui.area.as_str(), "a\nbc", "the newline kept, the tab dropped");
    assert_eq!(n, 4, "the count is what was kept");
}

/// **A number field copies** — M45/T45.0 item 11 (R10-F13). `NumberInput`
/// forwarded neither `selection` nor `compose` to the field it owns, so
/// Ctrl+A highlighted its text and a copy found nothing.
#[test]
fn a_number_field_copies_its_selection() {
    let mut h: Host<NumberApp> = Host::new(NumberApp::default(), 400, 100);
    h.pump();
    h.input.focus = Some(1);
    h.post(Event::Modifiers(Mods::CTRL));
    h.post(Event::Key { key: Key::Char('a'), down: true });
    h.post(Event::Modifiers(Mods::NONE));
    h.pump();
    let shown = h.app.ui.number.as_str().to_owned();
    println!("NumberInput after Ctrl+A: shows {shown:?}, copy gives {:?}", h.copy());
    assert!(!shown.is_empty(), "setup: the number field shows nothing");
    assert_eq!(h.copy(), Some(shown.as_str()), "Ctrl+A then copy must give the field's text");
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

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct SearchUi {
    #[widget_id("search")]
    search: lumen_static_widgets::SearchField,
}

#[derive(DeriveApp, Default)]
struct SearchApp {
    #[tree]
    ui: SearchUi,
}

/// **The accept direction for T45.9**: a composite whose field is a node
/// of its own still copies, with no forwarding of its own.
#[test]
fn a_search_field_copies_its_selection() {
    let mut h: Host<SearchApp> = Host::new(SearchApp::default(), 400, 100);
    h.app.ui.search = lumen_static_widgets::SearchField::new("needle");
    h.pump();
    h.post(Event::Key { key: Key::Tab, down: true });
    h.pump();
    assert_eq!(h.input.focus, Some(3), "setup: Tab did not focus the search field's text");
    h.post(Event::Modifiers(Mods::CTRL));
    h.post(Event::Key { key: Key::Char('a'), down: true });
    h.post(Event::Modifiers(Mods::NONE));
    h.pump();
    println!("SearchField after Ctrl+A: copy gives {:?}", h.copy());
    assert_eq!(h.copy(), Some("needle"));
}
