//! **A replaced hook is reported as replaced, not as someone else's
//! panic** — M36/T36.1 (`docs/plan-panic-recovery.md` §2.1).
//!
//! Its own test binary, because it replaces the process-wide panic hook
//! — which is exactly what M28's failure tests do while tests run in
//! parallel, and why the guard must survive it.
//!
//! The sequence that would lie: a job panics (M28's spawner catches it,
//! but the recording hook still records it), then something replaces the
//! hook, then the frame panics. The frame's panic leaves no record, so
//! without the guard clearing the record at entry, the *job's* message
//! and location would be reported as the frame's.

use lumen_static::spawn::Immediate;
use lumen_static_shell::Host;
use panic_app::{build, Fx, Ui};

#[test]
fn a_replaced_hook_says_so_instead_of_reporting_a_stale_panic() {
    let mut h: Host<Fx> = Host::new(build(), 300, 200);
    h.set_source_root("fixtures/panic-app");
    h.set_spawner(Box::new(Immediate));
    h.pump();
    let i = lumen_static::style::index_of_wid(<Ui as lumen_static::Widget>::SUB_TABLE, "go").unwrap();
    let s = h.states[i];
    let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);

    // A job's panic, caught by the spawner, under the recording hook.
    h.app.armed = panic_app::JOB_PANIC;
    h.click(x, y);
    assert!(h.guard(|h| h.pump()).is_some() && h.guard(|h| h.pump()).is_some(), "setup: the job's panic escaped");

    // Something replaces the hook; then the frame panics.
    std::panic::set_hook(Box::new(|_| {}));
    h.app.armed = 1;
    h.click(x, y);
    assert!(h.guard(|h| h.pump()).is_none(), "setup: site 1 did not panic");
    let r = h.failure().unwrap();
    println!("{r:?}");
    assert!(!r.message.contains("job"), "a stale job record was reported as the frame's panic: {r:?}");
    assert_eq!(r.note, Some("location unavailable — the panic hook was replaced"));
    assert!(r.message.contains("site 1"), "the frame's own message comes from the payload: {r:?}");
}
