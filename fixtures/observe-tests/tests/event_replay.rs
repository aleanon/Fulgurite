//! A recorded input session round-trips through JSON -- moved from
//! `crates/fulgurite/tests/events.rs`'s E0 test (M43/T43.7).
//! Recording is a dev concern, so the serde derives ride the `observe`
//! gate -- the vocabulary itself is always present -- and core's own tests
//! no longer build with `observe`: see this package's `Cargo.toml`.

use fulgurite::{Event, InputQueue, Key};

#[test]
fn a_recorded_session_serializes_and_comes_back() {
    let mut q: InputQueue = InputQueue::new();
    q.push(Event::down(1.0, 2.0));
    q.push(Event::Key { key: Key::Enter, down: true });
    q.push(Event::Text { ch: 'x' });
    let recorded: Vec<Event> = q.iter().copied().collect();
    assert_eq!(recorded.len(), 3);
    let json = serde_json::to_string(&recorded).unwrap();
    let replayed: Vec<Event> = serde_json::from_str(&json).unwrap();
    assert_eq!(recorded, replayed, "a session serializes and comes back");
}
