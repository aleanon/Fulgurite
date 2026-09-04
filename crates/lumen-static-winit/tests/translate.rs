//! The half of a window that can be tested without one.
//!
//! Event translation is pure, and it is where a keystroke silently
//! becomes nothing — `key_of` returns `Option` and the loop drops the
//! `None`. That is a real dropped key, so the set it covers should be
//! checked rather than assumed (`docs/plan-desktop.md` T14.0/T14.2).

use lumen_static::{Event, Key, Mods};
use lumen_static_winit::translate::{key_of, keyboard, window_event, Cursor, LINE_SCROLL};
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
    window_event(&moved, &mut c, Mods::NONE, logical);
    assert_eq!((c.x, c.y), (30.0, 40.0));

    let down = WindowEvent::MouseInput {
        device_id: unsafe_device_id(),
        state: ElementState::Pressed,
        button: MouseButton::Left,
    };
    let t = window_event(&down, &mut c, Mods::NONE, logical);
    assert_eq!(t.iter().collect::<Vec<_>>(), vec![Event::PointerDown { x: 30.0, y: 40.0 }]);
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
    assert!(!window_event(&moved, &mut c, Mods::NONE, logical).is_input);
}

#[test]
fn wheel_lines_and_pixels_both_arrive_as_scroll() {
    let mut c = Cursor { x: 5.0, y: 6.0 };
    let lines = WindowEvent::MouseWheel {
        device_id: unsafe_device_id(),
        delta: MouseScrollDelta::LineDelta(0.0, 2.0),
        phase: winit::event::TouchPhase::Moved,
    };
    let t = window_event(&lines, &mut c, Mods::NONE, logical);
    assert_eq!(
        t.iter().collect::<Vec<_>>(),
        vec![Event::Scroll { x: 5.0, y: 6.0, dy: 2.0 * LINE_SCROLL }]
    );

    let px = WindowEvent::MouseWheel {
        device_id: unsafe_device_id(),
        delta: MouseScrollDelta::PixelDelta((0.0, 7.0).into()),
        phase: winit::event::TouchPhase::Moved,
    };
    let t = window_event(&px, &mut c, Mods::NONE, logical);
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
    window_event(&moved, &mut c, Mods::NONE, |x, y| (x / 2.0, y / 2.0));
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
    let t = window_event(&ev, &mut c, Mods::NONE, logical);
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
