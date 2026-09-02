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

use lumen_static::ops::{Fixed, Label, Spacer};
use lumen_static::reload::{self, Reloadable};
use lumen_static::repeat::Repeat;
use lumen_static::{
    pass, App as DeriveApp, Constraints, NodeState, Reloadable as DeriveReloadable, StaticTree,
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
    #[reload(fresh)]
    footer: Label, // the new build's footer must win
}

impl Default for Ui {
    fn default() -> Self {
        Ui {
            title: Label::new("untitled"),
            status: Label::default(),
            list: Repeat::new(0, 24.0, bind_row),
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

use std::ffi::c_void;

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

/// `App::fresh()` from this dylib's code.
#[no_mangle]
pub unsafe extern "C" fn hot_new() -> *mut c_void {
    Box::into_raw(Box::new(Session::fresh())) as *mut c_void
}

/// Drop the tree while this dylib is still loaded (§7 sequence step 2).
#[no_mangle]
pub unsafe extern "C" fn hot_drop(p: *mut c_void) {
    drop(unsafe { Box::from_raw(p as *mut Session) });
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

/// Serialize the keep-half into a HOST-owned buffer. Returns the byte
/// count needed; writes only if `cap` suffices (call once with cap 0 to
/// size, then again with the allocation).
#[no_mangle]
pub unsafe extern "C" fn hot_save(p: *mut c_void, buf: *mut u8, cap: usize) -> usize {
    let bytes = serde_json::to_vec(&unsafe { session(p) }.save()).unwrap_or_default();
    if bytes.len() <= cap && !buf.is_null() {
        unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, bytes.len()) };
    }
    bytes.len()
}

/// Graft saved bytes onto this dylib's fresh tree. Returns 0 on success.
#[no_mangle]
pub unsafe extern "C" fn hot_restore(p: *mut c_void, ptr: *const u8, len: usize) -> i32 {
    let bytes = unsafe { core::slice::from_raw_parts(ptr, len) };
    match serde_json::from_slice(bytes) {
        Ok(v) => {
            unsafe { session(p) }.restore(&v);
            0
        }
        Err(_) => 1,
    }
}

/// Human-readable state summary into a host buffer (same two-call
/// protocol as `hot_save`). The host asserts against this.
#[no_mangle]
pub unsafe extern "C" fn hot_report(p: *mut c_void, buf: *mut u8, cap: usize) -> usize {
    let s = unsafe { session(p) };
    let report = format!(
        "counter={} mode={:?} theme={} title='{}' status='{}' footer='{}' rows={} scroll={} window={} cache={}",
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
    let joined = reload::take_log().join("\n");
    let bytes = joined.as_bytes();
    if bytes.len() <= cap && !buf.is_null() {
        unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, bytes.len()) };
    }
    bytes.len()
}
