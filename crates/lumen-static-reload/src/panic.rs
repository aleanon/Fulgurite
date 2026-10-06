//! **A panic inside a loaded library is caught there, with the library's
//! own hook** — M36/T36.4 (`docs/plan-panic-recovery.md` §2.5).
//!
//! A `cdylib` links its own copy of std, so it has its own panic hook: a
//! hook the loader installs never sees a panic raised in library code,
//! and the location — which exists only inside a hook — is lost. So the
//! hook, the record and the catch all live here, in the crate the
//! **library** links, and every loaded build gets fresh copies of these
//! statics. [`guard`] installs the hook on the first shim call of each
//! build.
//!
//! The report crosses the boundary as plain bytes, drained by the
//! generated `hot_take_panic`. Not `extern "C-unwind"`: the payload is a
//! `Box<dyn Any>` from another build, and a `TypeId` is not stable
//! across builds. Without a catch, a panic reaching an `extern "C"`
//! boundary aborts the loader (Rust 1.81).
//!
//! **In a file of its own, using only `std` and [`crate::raw`]**
//! (M49/T49.3), so `fixtures/miri-reload` can include it by path and
//! run it under miri: `cargo miri` cannot build this crate.

use std::cell::RefCell;
use std::sync::{Mutex, Once};

thread_local! {
    static LAST: RefCell<Option<String>> = const { RefCell::new(None) };
}

static HOOK: Once = Once::new();
static REPORT: Mutex<Option<String>> = Mutex::new(None);

/// Install this build's recording hook. It chains the previous hook,
/// so stderr still gets std's report. Once per loaded build.
pub fn install() {
    HOOK.call_once(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let msg = match info.payload().downcast_ref::<&str>() {
                Some(s) => (*s).to_string(),
                None => info.payload().downcast_ref::<String>().cloned().unwrap_or_default(),
            };
            let line = match info.location() {
                Some(l) => format!("panicked at {}:{}:{}: {msg}", l.file(), l.line(), l.column()),
                None => format!("panicked (no location): {msg}"),
            };
            let _ = LAST.try_with(|c| *c.borrow_mut() = Some(line));
            prev(info);
        }));
    });
}

/// Run a shim's body, catching a panic. On a panic the report is
/// kept for `hot_take_panic` and `fallback` is returned.
pub fn guard<R>(fallback: R, f: impl FnOnce() -> R) -> R {
    install();
    let _ = LAST.try_with(|c| c.borrow_mut().take());
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(payload) => {
            let recorded = LAST.try_with(|c| c.borrow_mut().take()).ok().flatten();
            let line = recorded.unwrap_or_else(|| {
                let msg = match payload.downcast_ref::<&str>() {
                    Some(s) => (*s).to_string(),
                    None => payload.downcast_ref::<String>().cloned().unwrap_or_default(),
                };
                format!("panicked (location unavailable — this build's hook did not record): {msg}")
            });
            if let Ok(mut r) = REPORT.lock() {
                *r = Some(line);
            }
            fallback
        }
    }
}

/// **Record a refusal as the last report, without panicking** —
/// M89/T89.7: `lumen_boot` asked for a window program the library's table
/// has no entry for answers null, and the loader reads why through
/// `hot_take_panic` as it would a panic's report. No panic, so nothing is
/// printed and no hook runs.
pub fn refuse(msg: String) {
    if let Ok(mut r) = REPORT.lock() {
        *r = Some(msg);
    }
}

/// Drain the last report into `(buf, cap)` — the two-call protocol:
/// returns the length, and keeps the report when `cap` is too small.
/// Zero means no panic since the last drain.
///
/// # Safety
/// `(buf, cap)` must be a valid writable range, or `(null, 0)`.
pub unsafe fn take(buf: *mut u8, cap: usize) -> usize {
    let Ok(mut r) = REPORT.lock() else { return 0 };
    let Some(s) = r.as_ref() else { return 0 };
    // SAFETY: as the caller vouches.
    let n = unsafe { crate::raw::copy_out(s.as_bytes(), buf, cap) };
    if n <= cap && !buf.is_null() {
        *r = None;
    }
    n
}
