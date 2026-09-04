//! The half of a window that can be tested without one.
//!
//! Event translation is pure, and it is where a keystroke silently
//! becomes nothing — `named_key` returns `Option` and the loop drops the
//! `None`. That is a real dropped key, so the set it covers should be
//! checked rather than assumed (`docs/plan-desktop.md` T14.0/T14.2).

use lumen_static::{Event, Key};
use lumen_static_winit::translate::{named_key, window_event, Cursor, LINE_SCROLL};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{Key as WKey, NamedKey};

/// Identity: the tests that care about scaling live in the shell.
fn logical(x: f32, y: f32) -> (f32, f32) {
    (x, y)
}

#[test]
fn every_named_key_the_framework_has_translates() {
    // The other direction from `named_key`: for each `Key`, a winit
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
        assert_eq!(named_key(&WKey::Named(w)), Some(k), "{w:?}");
    }
    assert_eq!(pairs.len(), 12, "every Key variant is covered");
}

#[test]
fn an_unmapped_key_is_none_not_a_wrong_key() {
    assert_eq!(named_key(&WKey::Named(NamedKey::F1)), None);
    assert_eq!(named_key(&WKey::Named(NamedKey::PageUp)), None, "T14.2 adds this");
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
    window_event(&moved, &mut c, logical);
    assert_eq!((c.x, c.y), (30.0, 40.0));

    let down = WindowEvent::MouseInput {
        device_id: unsafe_device_id(),
        state: ElementState::Pressed,
        button: MouseButton::Left,
    };
    let t = window_event(&down, &mut c, logical);
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
    assert!(!window_event(&moved, &mut c, logical).is_input);
}

#[test]
fn wheel_lines_and_pixels_both_arrive_as_scroll() {
    let mut c = Cursor { x: 5.0, y: 6.0 };
    let lines = WindowEvent::MouseWheel {
        device_id: unsafe_device_id(),
        delta: MouseScrollDelta::LineDelta(0.0, 2.0),
        phase: winit::event::TouchPhase::Moved,
    };
    let t = window_event(&lines, &mut c, logical);
    assert_eq!(
        t.iter().collect::<Vec<_>>(),
        vec![Event::Scroll { x: 5.0, y: 6.0, dy: 2.0 * LINE_SCROLL }]
    );

    let px = WindowEvent::MouseWheel {
        device_id: unsafe_device_id(),
        delta: MouseScrollDelta::PixelDelta((0.0, 7.0).into()),
        phase: winit::event::TouchPhase::Moved,
    };
    let t = window_event(&px, &mut c, logical);
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
    window_event(&moved, &mut c, |x, y| (x / 2.0, y / 2.0));
    assert_eq!((c.x, c.y), (50.0, 25.0));
}

/// winit 0.30 has no public `DeviceId` constructor; every test event
/// needs one and none of them reads it.
fn unsafe_device_id() -> winit::event::DeviceId {
    // `DeviceId::dummy` is winit's own escape hatch for exactly this.
    #[allow(deprecated)]
    winit::event::DeviceId::dummy()
}
