//! **A panic shows where it happened, and the host survives it** —
//! M36/T36.1 (`docs/plan-panic-recovery.md`).
//!
//! Compiled in wherever panics unwind; **used** where the window crate
//! decides — debug builds, and release builds with its `reload` feature
//! (plan rule 3: a shipped application should crash where a crash
//! reporter can see it). The decision is the window crate's, not this
//! crate's, because the workspace's dev profile builds this crate with
//! `debug-assertions = false` for speed: a `cfg(debug_assertions)` here
//! was false in every debug build, which is how the first version made
//! recovery absent from exactly the builds it was for. A headless caller
//! opts in by calling `Host::guard`.
//!
//! # A hook for the location, a catch for control
//!
//! `std::panic::set_hook` is the only place `PanicHookInfo::location()`
//! exists, and the payload `catch_unwind` returns has no location in it.
//! So [`install`](crate::recover::install) sets a hook that **records** — message, location and a
//! backtrace — into a thread-local and then calls the previous hook, so
//! stderr still gets the ordinary report; and [`Host::guard`](crate::Host::guard)
//! **catches**, turns the record into a [`PanicReport`](crate::recover::PanicReport), and restores
//! the host. The guard clears the record before it runs, as hot_ice
//! does, so the stale record of a job panic M28's spawner already caught
//! cannot be reported as the frame's.
//!
//! Chaining the previous hook is safe here and was not in hot_ice: this
//! crate never unloads a library, so the previous hook cannot belong to
//! one.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

/// What a caught panic said, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanicReport {
    pub message: String,
    /// `file:line:column` as the panic reported it, or `None` when the
    /// hook had been replaced (see [`PanicReport::note`]).
    pub location: Option<(String, u32, u32)>,
    /// The nearest frame in the **application's** code, from the
    /// backtrace — the line to open when `location` is inside the
    /// framework. `None` without debug info, or with no source root set.
    pub app_frame: Option<(String, u32)>,
    /// Why something is missing, in words for the screen.
    pub note: Option<&'static str>,
}

struct Captured {
    message: String,
    location: Option<(String, u32, u32)>,
    backtrace: String,
}

thread_local! {
    static LAST: RefCell<Option<Captured>> = const { RefCell::new(None) };
}

static INSTALLED: AtomicBool = AtomicBool::new(false);

/// Install the recording hook, once per process (and again after a
/// caught panic finds it replaced). Idempotent.
pub fn install() {
    if INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = match info.payload().downcast_ref::<&str>() {
            Some(s) => (*s).to_string(),
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => s.clone(),
                None => "(a panic with a non-string payload)".to_string(),
            },
        };
        let location = info.location().map(|l| (l.file().to_string(), l.line(), l.column()));
        let backtrace = format!("{}", std::backtrace::Backtrace::force_capture());
        let _ = LAST.try_with(|c| *c.borrow_mut() = Some(Captured { message, location, backtrace }));
        prev(info);
    }));
}

/// Install again, chaining whatever hook is current — after a caught
/// panic found no record, which means someone replaced this hook.
fn reinstall() {
    INSTALLED.store(false, Ordering::SeqCst);
    install();
}

/// Forget any record — the guard's first act.
pub(crate) fn clear() {
    let _ = LAST.try_with(|c| c.borrow_mut().take());
}

/// Turn the last record into a report. `root` is the application's
/// source directory, relative to where it was built (e.g.
/// `fixtures/panic-app`), used to find the application frame.
pub(crate) fn report(payload: &(dyn std::any::Any + Send), root: Option<&str>) -> PanicReport {
    let captured = LAST.try_with(|c| c.borrow_mut().take()).ok().flatten();
    let Some(c) = captured else {
        // The hook did not record: it has been replaced. Say so rather
        // than show a blank line, and put it back for next time.
        reinstall();
        let message = match payload.downcast_ref::<&str>() {
            Some(s) => (*s).to_string(),
            None => payload.downcast_ref::<String>().cloned().unwrap_or_default(),
        };
        return PanicReport {
            message,
            location: None,
            app_frame: None,
            note: Some("location unavailable — the panic hook was replaced"),
        };
    };
    let app_frame = root.and_then(|r| app_frame(&c.backtrace, r));
    // Only worth saying when the location is *not* already the
    // application's — then the missing frame is the one a person needs.
    let in_app = |f: &str| root.is_some_and(|r| f.starts_with(r));
    let note = match (&app_frame, &c.location) {
        (None, Some((f, _, _))) if root.is_some() && !in_app(f) => {
            Some("no application frame in the backtrace — built without debug info?")
        }
        _ => None,
    };
    PanicReport { message: c.message, location: c.location, app_frame, note }
}

/// The first backtrace frame whose source lies under `root`. The
/// rendering prints `at <path>:<line>:<col>` under each symbol, and the
/// path is absolute **or relative to the current directory** — `./src/
/// lib.rs` from inside the package, which is where `cargo test` runs a
/// test. The first version matched the raw path and found nothing from a
/// test, so a relative path is resolved against the current directory
/// before `<root>/src/` is looked for in it.
fn app_frame(backtrace: &str, root: &str) -> Option<(String, u32)> {
    let needle = format!("{}/src/", root.trim_end_matches('/'));
    let cwd = std::env::current_dir().ok();
    for line in backtrace.lines() {
        let Some(raw) = line.trim().strip_prefix("at ") else { continue };
        let resolved;
        let at = if raw.starts_with('/') {
            raw
        } else {
            let Some(cwd) = &cwd else { continue };
            resolved = cwd.join(raw.trim_start_matches("./")).to_string_lossy().into_owned();
            resolved.as_str()
        };
        let Some(k) = at.find(&needle) else { continue };
        let rel = &at[k..];
        let mut parts = rel.rsplitn(3, ':');
        let _col = parts.next();
        let line_no = parts.next().and_then(|n| n.parse().ok());
        let file = parts.next();
        if let (Some(f), Some(n)) = (file, line_no) {
            return Some((f.to_string(), n));
        }
    }
    None
}
