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

// ---- the screen — M36/T36.2 -----------------------------------------

use lumen_static::cx::TextMeasure;
use lumen_static::display::{Color, DisplayList};

/// The screen's background: a dark slate, so it cannot be mistaken for
/// an application's own frame.
pub const PAGE: Color = Color(0x1b, 0x1f, 0x2a, 0xff);
/// The title — "the application panicked" — in a light red.
pub const TITLE: Color = Color(0xff, 0x9e, 0x96, 0xff);
/// The message and the location.
pub const BODY: Color = Color(0xe8, 0xea, 0xed, 0xff);
/// The hint.
pub const DIM: Color = Color(0xb4, 0xbc, 0xc8, 0xff);

const MARGIN: f32 = 24.0;
const TITLE_PX: f32 = 22.0;
const BODY_PX: f32 = 15.0;

/// **The panic screen, as a display list** — M36/T36.2
/// (`docs/plan-panic-recovery.md` §2.3).
///
/// A display list and not a widget tree, deliberately: a tree would go
/// through the layout, text measurement and paint that may have just
/// panicked. The lines are fixed — a title, the message wrapped to the
/// window's width, the location (or why there is none), the application
/// frame when the location is inside the framework, and the hint. The
/// width is measured with `measure`, the same measurer the frame uses.
pub fn failure_frame(
    r: &PanicReport,
    width: u32,
    height: u32,
    measure: &mut dyn TextMeasure,
) -> DisplayList {
    let mut out = DisplayList::new();
    let (w, h) = (width as f32, height as f32);
    out.rect([0.0, 0.0, w, h], PAGE);
    let avail = (w - 2.0 * MARGIN).max(1.0);
    let mut y = MARGIN;
    let line = |out: &mut DisplayList, y: &mut f32, text: &str, px: f32, weight: f32, color: Color| {
        let lh = px * 1.4;
        out.text_weighted([MARGIN, *y, MARGIN + avail, *y + lh], text, px, weight, color);
        *y += lh;
    };
    // **Every line wraps, the title too.** The first version wrapped
    // only the message, and at 260 px the 22 px title ran 288 px wide —
    // the viewport assertion found it.
    for l in wrap("The application panicked", avail, TITLE_PX, measure) {
        line(&mut out, &mut y, &l, TITLE_PX, 700.0, TITLE);
    }
    y += BODY_PX * 0.6;
    for l in wrap(&r.message, avail, BODY_PX, measure) {
        line(&mut out, &mut y, &l, BODY_PX, 400.0, BODY);
    }
    y += BODY_PX * 0.6;
    let at = match (&r.location, r.note) {
        (Some((f, l, c)), _) => format!("at {f}:{l}:{c}"),
        (None, Some(note)) => note.to_string(),
        (None, None) => "location unavailable".to_string(),
    };
    for l in wrap(&at, avail, BODY_PX, measure) {
        line(&mut out, &mut y, &l, BODY_PX, 400.0, BODY);
    }
    if let Some((f, n)) = &r.app_frame {
        let same = r.location.as_ref().is_some_and(|(lf, ll, _)| lf == f && ll == n);
        if !same {
            for l in wrap(&format!("in your code: {f}:{n}"), avail, BODY_PX, measure) {
                line(&mut out, &mut y, &l, BODY_PX, 400.0, BODY);
            }
        }
    }
    if let (Some(note), Some(_)) = (r.note, &r.location) {
        for l in wrap(note, avail, BODY_PX, measure) {
            line(&mut out, &mut y, &l, BODY_PX, 400.0, DIM);
        }
    }
    y += BODY_PX * 0.6;
    for l in wrap("Press R to retry", avail, BODY_PX, measure) {
        line(&mut out, &mut y, &l, BODY_PX, 400.0, DIM);
    }
    out
}

/// Greedy word wrap to `avail` px; a word wider than a line is broken by
/// characters, so nothing runs past the edge.
pub fn wrap(text: &str, avail: f32, px: f32, m: &mut dyn TextMeasure) -> Vec<String> {
    let fits = |m: &mut dyn TextMeasure, s: &str| m.measure(s, px, f32::INFINITY).w <= avail;
    let mut lines = Vec::new();
    for para in text.lines() {
        let mut cur = String::new();
        for word in para.split(' ') {
            let candidate = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
            if fits(m, &candidate) {
                cur = candidate;
                continue;
            }
            if !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
            }
            // The word alone: split it by characters if it is too wide.
            let mut piece = String::new();
            for ch in word.chars() {
                piece.push(ch);
                if !fits(m, &piece) {
                    piece.pop();
                    lines.push(std::mem::take(&mut piece));
                    piece.push(ch);
                }
            }
            cur = piece;
        }
        lines.push(cur);
    }
    lines
}
