//! **Which overlays trap Tab, and where focus goes when one closes** —
//! M44/T44.0 item 11, closing R3-F7 (`docs/plan-input-semantics.md`
//! §1.6, §2.3).
//!
//! The Tab trap applied to the topmost overlay of *any* kind. A tooltip
//! has no focusables, so Tab while one showed set focus to `None` and
//! kept it dead while the pointer rested there. And nothing returned
//! focus to the control that opened a modal: Escape left it on a panel
//! button with no area, which the next event's C5 check cleared.
//!
//! Through a `Host` under its default strategy, so the overlay list is
//! the one paint produced, exactly as in a window.

use lumen_static::invalidate::Boundaries;
use lumen_static::prelude::*;
use lumen_static::retain::Incremental;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{Event, EventCtx, Key};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;
use lumen_static_widgets::{Modal, Tooltip};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct Panel {
    #[widget_id("ok")]
    ok: Button<App>,
    #[widget_id("cancel")]
    cancel: Button<App>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct Ui {
    #[widget_id("first")]
    first: Button<App>,
    #[widget_id("tipped")]
    tipped: Tooltip<Button<App>>,
    #[widget_id("trigger")]
    trigger: Button<App>,
    #[widget_id("last")]
    last: Button<App>,
    #[widget_id("modal")]
    modal: Modal<Panel>,
}

/// Unsplit: the trigger's handler opens the modal, and `after_update`
/// closes it when the veil asks — a `Veil` requests its dismissal rather
/// than closing itself (M27/T27.7).
#[derive(DeriveApp, Default)]
#[app(after_update = close_on_request)]
struct App {
    #[tree]
    ui: Ui,
}

fn close_on_request(a: &mut App) -> bool {
    if a.ui.modal.take_dismissed() {
        a.ui.modal.close();
        true
    } else {
        false
    }
}

fn build() -> App {
    let mut a = App::default();
    a.ui.first.label = "first";
    a.ui.tipped = Tooltip::new(Button::default(), "a tip");
    a.ui.tipped.content.label = "tipped";
    a.ui.trigger.label = "open";
    a.ui.trigger.on_click = |s: &mut App, _: &EventCtx| {
        s.ui.modal.open();
    };
    a.ui.last.label = "last";
    a.ui.modal.panel.ok.label = "ok";
    a.ui.modal.panel.cancel.label = "cancel";
    a
}

type H = Host<App, Incremental, Boundaries<256>>;

fn host() -> H {
    let mut h: H = Host::new(build(), 320, 320);
    h.set_stylesheet(&[Rule::new(
        Selector::Kind("button"),
        Patch::new().width(Length::Fixed(80.0)).height(Length::Fixed(28.0)),
    )]);
    h.pump();
    h.pump();
    h
}

fn idx(wid: &str) -> u32 {
    lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).expect(wid) as u32
}

/// The tipped button's index: the content of the `Tooltip`.
fn tipped_button() -> u32 {
    idx("tipped") + 1
}

fn key(h: &mut H, key: Key) {
    h.post(Event::Key { key, down: true });
    h.pump();
}

/// Where Tab goes from `first` with nothing open — the answer a tooltip
/// must not change.
fn tab_from_first_with_nothing_open() -> Option<u32> {
    let mut h = host();
    h.input.focus = Some(idx("first"));
    key(&mut h, Key::Tab);
    h.input.focus
}

#[test]
fn tab_passes_through_an_open_tooltip() {
    let expected = tab_from_first_with_nothing_open();
    assert_eq!(expected, Some(tipped_button()), "setup: Tab from first reaches the tipped button");

    let mut h = host();
    let b = h.states[tipped_button() as usize];
    h.post(Event::PointerMove { x: b.pos.x + b.size.w / 2.0, y: b.pos.y + b.size.h / 2.0 });
    h.pump();
    h.pump();
    assert!(h.app.ui.tipped.is_open(), "setup: hovering did not open the tooltip");
    // The tip's toast: the first node after the popup with any area.
    let tip = (idx("tipped") + 2..idx("trigger")).find(|&i| h.states[i as usize].size.w > 0.0).expect("setup: the tip has no area");
    let t = h.states[tip as usize];
    println!("setup: the tip (node {tip}) is laid out at {:?}", (t.pos, t.size));
    assert!(t.size.w > 0.0 && t.size.h > 0.0, "setup: the open tooltip has no area");

    h.input.focus = Some(idx("first"));
    key(&mut h, Key::Tab);
    println!(
        "tooltip open, Tab from first: focus {:?}, expected {:?} (the same as with no tooltip)",
        h.input.focus, expected
    );
    assert_eq!(h.input.focus, expected, "an open tooltip changed where Tab goes");
}

#[test]
fn escape_from_a_modal_returns_focus_to_its_trigger() {
    let mut h = host();
    h.input.focus = Some(idx("trigger"));
    key(&mut h, Key::Enter);
    h.pump();
    assert!(h.app.ui.modal.open, "setup: Enter on the trigger did not open the modal");
    let ok = h.states[idx("ok") as usize];
    println!("setup: the modal's ok button at {:?}", (ok.pos, ok.size));
    assert!(ok.size.w > 0.0 && ok.size.h > 0.0, "setup: the open modal's panel has no area");

    // Tab cycles inside it: the accept direction, which the trap must keep.
    let inside = [idx("ok"), idx("cancel")];
    let mut seen = Vec::new();
    for _ in 0..4 {
        key(&mut h, Key::Tab);
        seen.push(h.input.focus);
    }
    println!("Tab x4 in the modal: {seen:?}, inside {inside:?}");
    assert!(seen.iter().all(|f| f.is_some_and(|f| inside.contains(&f))), "Tab left the modal");
    assert!(seen.contains(&Some(inside[0])) && seen.contains(&Some(inside[1])), "Tab did not cycle");

    key(&mut h, Key::Escape);
    h.pump();
    assert!(!h.app.ui.modal.open, "setup: Escape did not close the modal");
    // One more event, the one C5 clears a stale focus on.
    h.post(Event::Modifiers(lumen_static::Mods::NONE));
    h.pump();
    println!("after Escape: focus {:?}, expected the trigger {}", h.input.focus, idx("trigger"));
    assert_eq!(h.input.focus, Some(idx("trigger")), "closing the modal did not return focus to its trigger");
}
