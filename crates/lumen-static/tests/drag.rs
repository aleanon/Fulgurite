//! T15.7: dragging within an application.

use lumen_static::{Drag, DragPhase, Event, Point};

fn p(x: f32, y: f32) -> Point {
    Point { x, y }
}

/// **The threshold is what keeps a click a click.**
#[test]
fn a_press_is_not_a_drag_until_it_moves_far_enough() {
    let mut d: Drag<u32> = Drag::default();
    d.press(p(10.0, 10.0), 7);
    assert_eq!(d.phase(), DragPhase::Pending);
    assert_eq!(d.payload(), None, "nothing to drop yet");

    assert!(!d.feed(&Event::PointerMove { x: 12.0, y: 10.0 }), "2 px is a click");
    assert_eq!(d.phase(), DragPhase::Pending);

    assert!(d.feed(&Event::PointerMove { x: 20.0, y: 10.0 }), "10 px is a drag");
    assert_eq!(d.phase(), DragPhase::Active);
    assert_eq!(d.payload(), Some(&7));
}

#[test]
fn a_click_that_never_became_a_drag_drops_nothing() {
    let mut d: Drag<u32> = Drag::default();
    d.press(p(0.0, 0.0), 1);
    d.feed(&Event::PointerMove { x: 1.0, y: 0.0 });
    d.feed(&Event::up(1.0, 0.0));
    assert_eq!(d.take(), None, "a click must not drop anything");
    assert_eq!(d.phase(), DragPhase::Idle);
}

#[test]
fn taking_the_payload_ends_the_drag() {
    let mut d: Drag<&'static str> = Drag::default();
    d.press(p(0.0, 0.0), "row-3");
    d.feed(&Event::PointerMove { x: 40.0, y: 0.0 });
    assert_eq!(d.take(), Some("row-3"));
    assert_eq!(d.phase(), DragPhase::Idle);
    assert_eq!(d.take(), None, "and it cannot be dropped twice");
}

#[test]
fn a_release_ends_the_drag_but_does_not_drop() {
    // The drop is the caller's decision -- only it knows what is under
    // the pointer -- so a release ends the gesture and leaves the
    // choice open for exactly that frame.
    let mut d: Drag<u32> = Drag::default();
    d.press(p(0.0, 0.0), 5);
    d.feed(&Event::PointerMove { x: 40.0, y: 0.0 });
    assert!(d.feed(&Event::up(40.0, 0.0)), "the release is worth a repaint");
    assert_eq!(d.phase(), DragPhase::Idle);
}

#[test]
fn cancel_gives_up_from_either_phase() {
    for far in [1.0, 40.0] {
        let mut d: Drag<u32> = Drag::default();
        d.press(p(0.0, 0.0), 1);
        d.feed(&Event::PointerMove { x: far, y: 0.0 });
        assert!(d.cancel());
        assert_eq!(d.phase(), DragPhase::Idle);
        assert_eq!(d.payload(), None);
        assert!(!d.cancel(), "cancelling nothing is not a change");
    }
}

#[test]
fn delta_tracks_the_pointer_for_a_drag_ghost() {
    let mut d: Drag<u32> = Drag::default();
    d.press(p(10.0, 10.0), 1);
    d.feed(&Event::PointerMove { x: 30.0, y: 25.0 });
    assert_eq!(d.delta(), p(20.0, 15.0));
    assert_eq!(d.at(), p(30.0, 25.0));
}

#[test]
fn events_that_are_not_pointer_events_are_ignored() {
    let mut d: Drag<u32> = Drag::default();
    d.press(p(0.0, 0.0), 1);
    assert!(!d.feed(&Event::Text { ch: 'a' }));
    assert_eq!(d.phase(), DragPhase::Pending, "typing does not drag");
}

#[test]
fn the_payload_is_in_the_type_not_boxed() {
    // The gating policy's second mechanism: the payload is a type
    // parameter, so `Drag<()>` costs the phase, two points, the threshold
    // and an empty `Option`, and there is no `Box<dyn Any>` and no
    // allocator anywhere near it. **Absolute as well as relative** since
    // M54/T54.9 (R15-F8): this test was named for "one byte" and asserted
    // `size_of::<Option<()>>() == 1`, a language fact, while `drag.rs` said
    // a `Drag<()>` was one byte. It is 24.
    let unit = core::mem::size_of::<Drag<()>>();
    let big = core::mem::size_of::<Drag<[u8; 64]>>();
    println!("Drag<()> = {unit} B (bound 24); Drag<[u8; 64]> = {big} B");
    assert!(unit < big, "the payload is in the type, so an unused one costs nothing");
    assert!(unit <= 24, "Drag<()> is {unit} B, over its 24 B bound");
    fn assert_copy<T: Copy>() {}
    assert_copy::<Drag<u32>>();
    assert_copy::<DragPhase>();
}
