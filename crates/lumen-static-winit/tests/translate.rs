//! The half of a window that can be tested without one.
//!
//! Event translation is pure, and it is where a keystroke silently
//! becomes nothing — `key_of` returns `Option` and the loop drops the
//! `None`. That is a real dropped key, so the set it covers should be
//! checked rather than assumed (`docs/plan-desktop.md` T14.0/T14.2).

use lumen_static::{Event, Key, Mods, PointerButton};
use lumen_static_winit::translate::{
    key_of, keyboard, window_event, ClickCounter, Cursor, LINE_SCROLL,
};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{Key as WKey, NamedKey};

/// Identity: the tests that care about scaling live in the shell.
fn logical(x: f32, y: f32) -> (f32, f32) {
    (x, y)
}

#[test]
fn every_named_key_the_framework_has_translates() {
    // The other direction from `key_of`: for each `Key`, a winit
    // name must map to it. A key the framework declares but the window
    // cannot deliver is worse than one it does not declare.
    let pairs = [
        (NamedKey::Enter, Key::Enter),
        (NamedKey::Space, Key::Space),
        (NamedKey::Escape, Key::Escape),
        (NamedKey::Tab, Key::Tab),
        (NamedKey::Backspace, Key::Backspace),
        (NamedKey::Delete, Key::Delete),
        (NamedKey::ArrowLeft, Key::Left),
        (NamedKey::ArrowRight, Key::Right),
        (NamedKey::ArrowUp, Key::Up),
        (NamedKey::ArrowDown, Key::Down),
        (NamedKey::Home, Key::Home),
        (NamedKey::End, Key::End),
    ];
    for (w, k) in pairs {
        assert_eq!(key_of(&WKey::Named(w)), Some(k), "{w:?}");
    }
    assert_eq!(pairs.len(), 12, "every Key variant is covered");
}

#[test]
fn the_keys_t14_2_added_translate() {
    let pairs = [
        (NamedKey::PageUp, Key::PageUp),
        (NamedKey::PageDown, Key::PageDown),
        (NamedKey::Insert, Key::Insert),
        (NamedKey::F1, Key::F(1)),
        (NamedKey::F5, Key::F(5)),
        (NamedKey::F12, Key::F(12)),
    ];
    for (w, k) in pairs {
        assert_eq!(key_of(&WKey::Named(w)), Some(k), "{w:?}");
    }
}

#[test]
fn a_printable_key_becomes_char_lower_cased() {
    // One shortcut-table entry per letter: Shift is reported in the
    // modifiers, not folded into the letter (T14.2).
    assert_eq!(key_of(&WKey::Character("c".into())), Some(Key::Char('c')));
    assert_eq!(key_of(&WKey::Character("C".into())), Some(Key::Char('c')));
    assert_eq!(key_of(&WKey::Character("/".into())), Some(Key::Char('/')));
    // A multi-character label is a dead key or an IME artefact, not a
    // shortcut (T14.8).
    assert_eq!(key_of(&WKey::Character("ab".into())), None);
}

#[test]
fn a_key_the_framework_has_no_name_for_is_none() {
    assert_eq!(key_of(&WKey::Named(NamedKey::F13)), None, "past the F(1..=12) set");
    assert_eq!(key_of(&WKey::Named(NamedKey::MediaPlay)), None, "excluded on purpose");
    assert_eq!(key_of(&WKey::Named(NamedKey::Shift)), None, "modifiers are state, not keys");
}

#[test]
fn a_click_uses_the_last_moves_position() {
    // winit's MouseInput carries no coordinates. A host that does not
    // remember the last move cannot say where a click landed -- which
    // is the whole reason `Cursor` exists.
    let mut c = Cursor::default();
    let moved = WindowEvent::CursorMoved {
        device_id: unsafe_device_id(),
        position: (30.0, 40.0).into(),
    };
    window_event(&moved, &mut c, &mut ClickCounter::default(), Mods::NONE, logical);
    assert_eq!((c.x, c.y), (30.0, 40.0));

    let down = WindowEvent::MouseInput {
        device_id: unsafe_device_id(),
        state: ElementState::Pressed,
        button: MouseButton::Left,
    };
    let t = window_event(&down, &mut c, &mut ClickCounter::default(), Mods::NONE, logical);
    assert_eq!(t.iter().collect::<Vec<_>>(), vec![Event::down(30.0, 40.0)]);
    assert!(t.is_input, "a click is input, for the latency stamp");
}

#[test]
fn a_move_is_not_counted_as_input() {
    // It fires continuously; stamping it would reset the latency clock
    // every frame and report near-zero for every click.
    let mut c = Cursor::default();
    let moved = WindowEvent::CursorMoved {
        device_id: unsafe_device_id(),
        position: (1.0, 2.0).into(),
    };
    assert!(!window_event(&moved, &mut c, &mut ClickCounter::default(), Mods::NONE, logical).is_input);
}

#[test]
fn wheel_lines_and_pixels_both_arrive_as_scroll() {
    let mut c = Cursor { x: 5.0, y: 6.0 };
    let lines = WindowEvent::MouseWheel {
        device_id: unsafe_device_id(),
        delta: MouseScrollDelta::LineDelta(0.0, 2.0),
        phase: winit::event::TouchPhase::Moved,
    };
    let t = window_event(&lines, &mut c, &mut ClickCounter::default(), Mods::NONE, logical);
    assert_eq!(
        t.iter().collect::<Vec<_>>(),
        vec![Event::Scroll { x: 5.0, y: 6.0, dy: 2.0 * LINE_SCROLL }]
    );

    let px = WindowEvent::MouseWheel {
        device_id: unsafe_device_id(),
        delta: MouseScrollDelta::PixelDelta((0.0, 7.0).into()),
        phase: winit::event::TouchPhase::Moved,
    };
    let t = window_event(&px, &mut c, &mut ClickCounter::default(), Mods::NONE, logical);
    assert_eq!(t.iter().collect::<Vec<_>>(), vec![Event::Scroll { x: 5.0, y: 6.0, dy: 7.0 }]);
}

#[test]
fn logical_conversion_is_applied_to_positions() {
    // Everything above the renderer is logical; winit reports physical
    // (M10b). The conversion happens once, here.
    let mut c = Cursor::default();
    let moved = WindowEvent::CursorMoved {
        device_id: unsafe_device_id(),
        position: (100.0, 50.0).into(),
    };
    window_event(&moved, &mut c, &mut ClickCounter::default(), Mods::NONE, |x, y| (x / 2.0, y / 2.0));
    assert_eq!((c.x, c.y), (50.0, 25.0));
}

/// winit 0.30 has no public `DeviceId` constructor; every test event
/// needs one and none of them reads it.
fn unsafe_device_id() -> winit::event::DeviceId {
    // `DeviceId::dummy` is winit's own escape hatch for exactly this.
    #[allow(deprecated)]
    winit::event::DeviceId::dummy()
}

#[test]
fn modifiers_changed_reaches_the_queue() {
    // In the queue, not beside it: a recorded session must replay with
    // the modifiers it was recorded with (T14.1).
    let mut c = Cursor::default();
    let ev = WindowEvent::ModifiersChanged(winit::keyboard::ModifiersState::CONTROL.into());
    let t = window_event(&ev, &mut c, &mut ClickCounter::default(), Mods::NONE, logical);
    assert_eq!(t.iter().collect::<Vec<_>>(), vec![Event::Modifiers(Mods::CTRL)]);
    assert!(!t.is_input, "a modifier change is not a click for latency purposes");
}

/// **The bug this rule exists to prevent**: Ctrl+C firing the shortcut
/// *and* typing a "c" — invisible until a text field has focus.
#[test]
fn ctrl_c_does_not_also_type_a_c() {
    let k = WKey::Character("c".into());
    let plain = keyboard(&k, Some("c"), true, Mods::NONE);
    assert_eq!(
        plain.iter().collect::<Vec<_>>(),
        vec![Event::Key { key: Key::Char('c'), down: true }, Event::Text { ch: 'c' }],
        "typing produces both"
    );

    let ctrl = keyboard(&k, Some("c"), true, Mods::CTRL);
    assert_eq!(
        ctrl.iter().collect::<Vec<_>>(),
        vec![Event::Key { key: Key::Char('c'), down: true }],
        "the shortcut produces the key and no text"
    );
}

#[test]
fn shift_still_types() {
    // Shift+A *is* how a capital arrives, which is why `is_shortcut`
    // masks Shift off rather than testing for an empty modifier set.
    let t = keyboard(&WKey::Character("A".into()), Some("A"), true, Mods::SHIFT);
    assert!(
        t.iter().any(|e| matches!(e, Event::Text { ch: 'A' })),
        "Shift+A types: {:?}",
        t.iter().collect::<Vec<_>>()
    );
}


#[test]
fn enter_activates_without_typing_a_carriage_return() {
    // winit reports "\r" as the text of Enter. Typing it into a field
    // would insert a carriage return beside the `Key` that already
    // means "activate".
    let t = keyboard(&WKey::Named(NamedKey::Enter), Some("\r"), true, Mods::NONE);
    assert_eq!(
        t.iter().collect::<Vec<_>>(),
        vec![Event::Key { key: Key::Enter, down: true }],
        "the control character is dropped"
    );
}

#[test]
fn a_key_release_never_types() {
    let t = keyboard(&WKey::Character("c".into()), Some("c"), false, Mods::NONE);
    assert_eq!(t.iter().collect::<Vec<_>>(), vec![Event::Key { key: Key::Char('c'), down: false }]);
}

#[test]
fn a_right_click_is_not_a_left_click() {
    let mut c = Cursor { x: 1.0, y: 2.0 };
    let mut n = ClickCounter::default();
    let ev = |b| WindowEvent::MouseInput {
        device_id: unsafe_device_id(),
        state: ElementState::Pressed,
        button: b,
    };
    let t = window_event(&ev(MouseButton::Right), &mut c, &mut n, Mods::NONE, logical);
    assert_eq!(
        t.iter().collect::<Vec<_>>(),
        vec![Event::PointerDown { x: 1.0, y: 2.0, button: PointerButton::Right, clicks: 1 }]
    );
    // Back/forward are excluded on purpose, and produce nothing rather
    // than a wrong button.
    let t = window_event(&ev(MouseButton::Back), &mut c, &mut n, Mods::NONE, logical);
    assert_eq!(t.iter().count(), 0);
}

#[test]
fn clicks_count_up_to_three_then_start_over() {
    let mut n = ClickCounter::default();
    assert_eq!(n.press(10.0, 10.0, PointerButton::Left), 1);
    assert_eq!(n.press(10.0, 10.0, PointerButton::Left), 2);
    assert_eq!(n.press(10.0, 10.0, PointerButton::Left), 3);
    assert_eq!(n.press(10.0, 10.0, PointerButton::Left), 1, "a fourth starts a new sequence");
}

#[test]
fn a_click_too_far_away_restarts_the_count() {
    let mut n = ClickCounter::default();
    assert_eq!(n.press(10.0, 10.0, PointerButton::Left), 1);
    assert_eq!(n.press(10.0 + 6.0, 10.0, PointerButton::Left), 1, "6 px is past the slop");
    assert_eq!(n.press(10.0 + 6.0, 10.0, PointerButton::Left), 2, "and within it now");
}

#[test]
fn a_different_button_restarts_the_count() {
    let mut n = ClickCounter::default();
    assert_eq!(n.press(0.0, 0.0, PointerButton::Left), 1);
    assert_eq!(n.press(0.0, 0.0, PointerButton::Right), 1, "not a left double-click");
}

#[test]
fn a_slow_second_click_is_a_new_single() {
    let mut n = ClickCounter::default();
    assert_eq!(n.press(0.0, 0.0, PointerButton::Left), 1);
    std::thread::sleep(lumen_static_winit::translate::MULTI_CLICK_TIME + std::time::Duration::from_millis(20));
    assert_eq!(n.press(0.0, 0.0, PointerButton::Left), 1);
}

/// **One dropped file is one `Dropped::File`** — M54/T54.7 (R15-F5).
///
/// winit emits one `DroppedFile` per file, so the translation carries one
/// path, not a `Vec` that always held exactly one. A pointer move is not a
/// drop. Needs `dnd`; `just test` runs it (`dev-shape`).
#[cfg(feature = "dnd")]
#[test]
fn a_dropped_file_is_one_drop_carrying_its_path() {
    use lumen_static_winit::translate::{dropped, Dropped};
    let path = std::path::PathBuf::from("/tmp/lumen-static/dropped.txt");
    let got = dropped(&WindowEvent::DroppedFile(path.clone()));
    println!("DroppedFile({}) -> {got:?}", path.display());
    assert_eq!(got, Some(Dropped::File(path)), "the drop did not carry its path");
    let moved = WindowEvent::CursorMoved {
        device_id: unsafe_device_id(),
        position: winit::dpi::PhysicalPosition::new(3.0, 4.0),
    };
    let n = dropped(&moved).into_iter().count();
    println!("CursorMoved -> {n} drops");
    assert_eq!(n, 0, "a pointer move translated to a drop");
}
