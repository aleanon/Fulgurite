//! Hot-reload fixture (handoff §7): one app compiled twice — default
//! features is "v1", `--features v2` is "the code you just edited".
//! The host (`fixtures/hot-host`) drives the swap through the `extern "C"`
//! surface at the bottom; nothing but raw pointers and lengths crosses
//! the boundary, and the saved bytes live in a host-owned buffer.
//!
//! v1 → v2 changes, chosen to exercise every reload class:
//! - `status` is a `#[bind]` on the kept counter        → regenerated, not migrated
//! - `FOOTER` text and the fresh `theme` differ         → fresh code wins
//! - `Mode::Fancy` is deleted in v2                      → §12.2 fallback + log
//! - user data (counter, title, rows, scroll) is kept    → survives the swap
//! - `cache_hits` is reset                               → recomputed after

// The FFI safety contract is documented once at the extern-surface section.
#![allow(clippy::missing_safety_doc)]

use lumen_static::display::{Cmd, Color};
use lumen_static::ops::{Fixed, Label, Spacer};
use lumen_static::reload;
use lumen_static::repeat::Repeat;
use lumen_static::{
    pass, CheckBox, App as DeriveApp, Constraints, NodeState, Reloadable as DeriveReloadable, StaticTree,
    Widget,
};
use serde::{Deserialize, Serialize};

#[cfg(not(feature = "v2"))]
const FOOTER: &str = "footer-v1";
#[cfg(feature = "v2")]
const FOOTER: &str = "footer-v2";

#[cfg(not(feature = "v2"))]
const THEME: &str = "theme-v1";
#[cfg(feature = "v2")]
const THEME: &str = "theme-v2";

/// The checkbox's tick colour, which the edited code changes (M25/T25.10).
/// `Toggle`'s `Reloadable` keeps only `on`, so the tick comes from the new
/// build's `Default` after a swap — the fresh class, for a colour field.
#[cfg(not(feature = "v2"))]
const CHECK_MARK: Color = Color(200, 30, 30, 255);
#[cfg(feature = "v2")]
const CHECK_MARK: Color = Color(30, 60, 220, 255);

#[derive(Serialize, Deserialize, Debug, Default, Clone, Copy, PartialEq)]
enum Mode {
    #[default]
    Basic,
    #[cfg(not(feature = "v2"))]
    Fancy,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
struct RowW {
    icon: Fixed,
    name: Label,
    gap: Spacer,
}

fn bind_row(item: &mut RowW, row: usize) {
    item.icon.w = (row % 50) as f32 + 1.0;
    item.icon.h = 10.0;
}

#[derive(StaticTree, DeriveReloadable)]
#[widget(column, spacing = 2.0)]
struct Ui {
    title: Label, // nested keep: user's edits survive
    /// Derived from `Session.counter` by a `#[bind]` on that field, so it
    /// is regenerated after a swap rather than migrated. A binding is
    /// code: the new dylib supplies it, and nothing about it appears in
    /// the saved bytes.
    #[reload(fresh)]
    status: Label<String>,
    list: Repeat<RowW>,
    /// M25/T25.10: `on` is saved and must survive; `mark` is not and comes
    /// from the new build.
    #[widget_id("check")]
    check: CheckBox<Session>,
    #[reload(fresh)]
    footer: Label, // the new build's footer must win
}

impl Default for Ui {
    fn default() -> Self {
        Ui {
            title: Label::new("untitled"),
            status: Label::default(),
            list: Repeat::new(0, 24.0, bind_row),
            check: CheckBox { mark: CHECK_MARK, ..Default::default() },
            footer: Label::new(FOOTER),
        }
    }
}

#[derive(DeriveReloadable, DeriveApp)]
struct Session {
    #[tree]
    ui: Ui,
    #[reload(keep)]
    #[bind(ui.status, "count {}")]
    counter: u32,
    #[reload(keep)]
    mode: Mode,
    #[reload(fresh)]
    theme: &'static str,
    #[reload(reset)]
    cache_hits: u64,
}

impl Default for Session {
    fn default() -> Self {
        Session { ui: Ui::default(), counter: 0, mode: Mode::Basic, theme: THEME, cache_hits: 0 }
    }
}

// ---- extern "C" surface (§7: raw pointers and lengths only) -----------------
//
// Safety: every fn below is `unsafe` — callers must pass an app pointer
// obtained from `hot_new` of THIS library build, and valid (ptr, len)
// buffer pairs. The host upholds this by construction. The contract is
// stated once here rather than per fn (see the crate-level lint allow).

use std::cell::RefCell;
use std::ffi::c_void;

// The four generic entry points — `hot_new`, `hot_drop`, `hot_save`,
// `hot_restore` — come from the framework rather than being written out
// here. They were about 90 lines of hand-rolled FFI, and the size-probe
// protocol in them had drifted from the loader's
// (`docs/plan-review-fixes.md` F6.7). The app-specific verbs below stay
// hand-written: there is nothing to share about `hot_bump`.
lumen_static_reload::export_reloadable_app!(Session);

thread_local! {
    /// Fallback lines taken from the core but not yet delivered to the
    /// host. See `hot_take_log`.
    static PENDING_LOG: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Reborrow the host's app pointer.
///
/// # Safety
///
/// `p` must come from `hot_new` of *this* library build and must not be
/// aliased for the duration of the returned borrow. Each `extern "C"` fn
/// below calls this once and lets the borrow die before returning, which
/// is what upholds the second half.
///
/// This was a *safe* fn until 2026-09-02. It manufactured a `&mut` with
/// a caller-chosen lifetime from a raw pointer, so two live `&mut
/// Session` could be produced with no `unsafe` at any call site — and
/// the optimizer already acted on the aliasing (review PoC: `a.n = 1; b.n
/// = 2;` printed `a.n=2`). No call site did it; nothing but habit
/// prevented one. `docs/plan-review-fixes.md` F1.2.
unsafe fn session<'a>(p: *mut c_void) -> &'a mut Session {
    unsafe { &mut *(p as *mut Session) }
}



#[no_mangle]
pub unsafe extern "C" fn hot_bump(p: *mut c_void) {
    unsafe { session(p) }.counter += 1;
}

#[no_mangle]
pub unsafe extern "C" fn hot_set_title(p: *mut c_void, ptr: *const u8, len: usize) {
    let bytes = unsafe { core::slice::from_raw_parts(ptr, len) };
    if let Ok(s) = core::str::from_utf8(bytes) {
        unsafe { session(p) }.ui.title.set(Box::leak(s.to_owned().into_boxed_str()));
    }
}

#[no_mangle]
pub unsafe extern "C" fn hot_set_rows(p: *mut c_void, rows: usize) {
    unsafe { session(p) }.ui.list.count = rows;
}

#[no_mangle]
pub unsafe extern "C" fn hot_set_scroll(p: *mut c_void, scroll: f32) {
    unsafe { session(p) }.ui.list.scroll = scroll;
}

/// v1 only in spirit: sets the mode this build considers "fancy" (v2 has
/// no such variant and leaves Basic).
#[no_mangle]
pub unsafe extern "C" fn hot_set_fancy(p: *mut c_void) {
    #[cfg(not(feature = "v2"))]
    {
        unsafe { session(p) }.mode = Mode::Fancy;
    }
    #[cfg(feature = "v2")]
    {
        let _ = p;
    }
}

/// Check the box — v1 does, before the swap (M25/T25.5).
#[no_mangle]
pub unsafe extern "C" fn hot_set_check(p: *mut c_void) {
    let s = unsafe { session(p) };
    s.ui.check.on = true;
}

/// The colour the checkbox's tick was painted in, RGBA packed high byte
/// first; `0` if nothing was painted (M25/T25.5).
///
/// Read off a laid-out and painted frame, not off the look, so it is what
/// the screen would show. It does not count as a frame: `cache_hits` is
/// untouched, because the host asserts on it.
#[no_mangle]
pub unsafe extern "C" fn hot_check_mark(p: *mut c_void) -> u32 {
    let s = unsafe { session(p) };
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    let sty = pass::default_styles(&s.ui);
    let c = Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 600.0 };
    pass::layout(&mut s.ui, &sty, &mut st, c);
    let mut out = lumen_static::DisplayList::new();
    pass::paint(&s.ui, &st, &mut out);
    let Some(i) = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "check") else {
        return 0;
    };
    let b = st[i];
    let inside = |r: &[f32; 4]| {
        r[0] >= b.pos.x - 0.5
            && r[2] <= b.pos.x + b.size.w + 0.5
            && r[1] >= b.pos.y - 0.5
            && r[3] <= b.pos.y + b.size.h + 0.5
    };
    out.cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::RoundedRect { rect, color, .. } if inside(rect) => Some(*color),
            _ => None,
        })
        .next_back()
        .map_or(0, |c| u32::from_be_bytes([c.0, c.1, c.2, c.3]))
}

/// One frame: layout + paint, returns the paint checksum.
#[no_mangle]
pub unsafe extern "C" fn hot_frame(p: *mut c_void) -> u64 {
    let s = unsafe { session(p) };
    s.cache_hits += 1;
    // The update step: application state reaches the screen here. Widget
    // fields hold the *presentation copy*; sibling fields are the source
    // of truth (`docs/layout-and-storage-decisions.md` §1). The body is
    // generated from the `#[bind]` on `Session.counter` — a real frame
    // loop calls this from `pump`; this fixture drives frames by hand
    // across an FFI boundary, so it calls it here.
    lumen_static::event::App::update(s);
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    let sty = pass::default_styles(&s.ui);
    let c = Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 600.0 };
    pass::layout(&mut s.ui, &sty, &mut st, c);
    let mut out = lumen_static::DisplayList::new();
    pass::paint(&s.ui, &st, &mut out);
    // The host asserts on a stable digest of the frame rather than on
    // the command list itself, which does not cross the FFI boundary.
    out.hash_from(0)
}



/// Human-readable state summary into a host buffer (same two-call
/// protocol as `hot_save`). The host asserts against this.
#[no_mangle]
pub unsafe extern "C" fn hot_report(p: *mut c_void, buf: *mut u8, cap: usize) -> usize {
    let s = unsafe { session(p) };
    let report = format!(
        "counter={} mode={:?} theme={} title='{}' status='{}' footer='{}' rows={} scroll={} window={} cache={} check={}",
        s.counter,
        s.mode,
        s.theme,
        s.ui.title.as_str(),
        s.ui.status.as_str(),
        s.ui.footer.as_str(),
        s.ui.list.count,
        s.ui.list.scroll,
        s.ui.list.window_len(),
        s.cache_hits,
        if s.ui.check.on { "on" } else { "off" },
    );
    let bytes = report.as_bytes();
    if bytes.len() <= cap && !buf.is_null() {
        unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, bytes.len()) };
    }
    bytes.len()
}

/// Drain this dylib's reload-fallback log into a host buffer (lines
/// joined with '\n'; same two-call protocol).
#[no_mangle]
pub unsafe extern "C" fn hot_take_log(buf: *mut u8, cap: usize) -> usize {
    // Staged, because `reload::take_log` is a `mem::take`: the old
    // version drained the core's log and *then* found the host's buffer
    // too small, so the diagnostic the log exists to preserve was
    // destroyed by the call that failed to deliver it — and the host
    // then panicked slicing past its own buffer. Draining into staging
    // and clearing only after a successful copy makes the two-call size
    // probe safe to use here, exactly as `hot_save` already uses it.
    // `docs/plan-review-fixes.md` F1.7.
    PENDING_LOG.with(|p| {
        let mut pending = p.borrow_mut();
        let fresh = reload::take_log().join("\n");
        if !fresh.is_empty() {
            if !pending.is_empty() {
                pending.push('\n');
            }
            pending.push_str(&fresh);
        }
        let n = pending.len();
        if n <= cap && !buf.is_null() {
            unsafe { core::ptr::copy_nonoverlapping(pending.as_ptr(), buf, n) };
            pending.clear();
        }
        n
    })
}

// ---- M28/T28.7: the step verb -----------------------------------------------
//
// The chain lives in the **host**, not here: the inbox, the epoch and
// the pending descriptor are all in `fixtures/hot-host`, outside both
// dylibs. This is the one thing a dylib contributes — *what a step
// means* — and it is reached through primitives, like every other verb
// in this fixture.
//
// `docs/step4-hot-reload.md`'s honesty list forbids a framework type
// crossing this seam: v1 and v2 each link their own copy and no layout
// agreement is promised. So a `Descriptor` crosses as three `u64`s and
// is reassembled on the far side, which is what
// `docs/plan-tasks.md` §2.1 means by "crosses as primitives".

/// Interpret one step of a chain, and return what it produced.
///
/// **v1 and v2 answer differently on purpose.** The gate starts a chain
/// against v1, swaps, and delivers the pending completion here; the
/// assertion is that the answer is v2's — that is, the new build
/// interpreted the old build's in-flight work, which is the whole claim
/// `docs/events-decision.md`'s rule 3 is about.
///
/// Takes no session pointer: a step is a pure function of the
/// descriptor, and giving it the app would invite a step that mutates
/// state the host is about to restore over.
#[no_mangle]
pub unsafe extern "C" fn hot_step(kind: u64, step: u64, payload: u64) -> u64 {
    // Kind 1 is the fixture's only chain. An unrecognised `(kind, step)`
    // answers 0, which the host reads as "this build does not know that
    // step" and counts — never guessing, which is what would make a
    // renumbering silently run the wrong step.
    if kind != 1 {
        return 0;
    }
    match step {
        // Step 0 is the same in both builds, so the host can start a
        // chain against v1 and see an *unremarkable* first result. If
        // this differed too, the gate could not tell "v2 ran the second
        // step" from "v2 ran everything".
        0 => payload + 1,
        #[cfg(not(feature = "v2"))]
        1 => payload * 10,
        #[cfg(feature = "v2")]
        1 => payload * 1000,
        _ => 0,
    }
}
