//! Hot-reload fixture (handoff §7): one app compiled twice — default
//! features is "v1", `--features v2` is "the code you just edited".
//! The host (`fixtures/hot-host`) drives the swap through the `extern "C"`
//! surface at the bottom; nothing but raw pointers and lengths crosses
//! the boundary, and the saved bytes live in a host-owned buffer.
//!
//! v1 → v2 changes, chosen to exercise every reload class:
//! - `FOOTER` text and the fresh `theme` differ         → fresh code wins
//! - `Mode::Fancy` is deleted in v2                      → §12.2 fallback + log
//! - user data (counter, title, rows, scroll) is kept    → survives the swap
//! - `cache_hits` is reset                               → recomputed after

// The FFI safety contract is documented once at the extern-surface section.
#![allow(clippy::missing_safety_doc)]

use lumen_static::ops::{Fixed, Label, Spacer};
use lumen_static::reload::{self, Reloadable};
use lumen_static::repeat::Repeat;
use lumen_static::{pass, Constraints, NodeState, Reloadable as DeriveReloadable, StaticTree, Widget};
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
    /// Derived from `Session.counter` by the update step below, so it is
    /// recomputed rather than migrated across a reload.
    #[reload(fresh)]
    status: Label,
    list: Repeat<RowW>,
    #[reload(fresh)]
    footer: Label, // the new build's footer must win
}

impl Default for Ui {
    fn default() -> Self {
        Ui {
            title: Label { text: "untitled" },
            status: Label { text: "count 0" },
            list: Repeat::new(0, 24.0, bind_row),
            footer: Label { text: FOOTER },
        }
    }
}

#[derive(DeriveReloadable)]
struct Session {
    ui: Ui,
    #[reload(keep)]
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

fn session<'a>(p: *mut c_void) -> &'a mut Session {
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
    session(p).counter += 1;
}

#[no_mangle]
pub unsafe extern "C" fn hot_set_title(p: *mut c_void, ptr: *const u8, len: usize) {
    let bytes = unsafe { core::slice::from_raw_parts(ptr, len) };
    if let Ok(s) = core::str::from_utf8(bytes) {
        session(p).ui.title.text = Box::leak(s.to_owned().into_boxed_str());
    }
}

#[no_mangle]
pub unsafe extern "C" fn hot_set_rows(p: *mut c_void, rows: usize) {
    session(p).ui.list.count = rows;
}

#[no_mangle]
pub unsafe extern "C" fn hot_set_scroll(p: *mut c_void, scroll: f32) {
    session(p).ui.list.scroll = scroll;
}

/// v1 only in spirit: sets the mode this build considers "fancy" (v2 has
/// no such variant and leaves Basic).
#[no_mangle]
pub unsafe extern "C" fn hot_set_fancy(p: *mut c_void) {
    #[cfg(not(feature = "v2"))]
    {
        session(p).mode = Mode::Fancy;
    }
    #[cfg(feature = "v2")]
    {
        let _ = p;
    }
}

/// One frame: layout + paint, returns the paint checksum.
#[no_mangle]
pub unsafe extern "C" fn hot_frame(p: *mut c_void) -> u64 {
    let s = session(p);
    s.cache_hits += 1;
    // The update step: application state reaches the screen here. Widget
    // fields hold the *presentation copy*; sibling fields are the source
    // of truth (`docs/layout-and-storage-decisions.md` §1). Until M3
    // makes text storage owned, the copy must be a `&'static str`, so
    // this is a lookup rather than a `format!`.
    s.ui.status.text = match s.counter {
        0 => "count 0",
        1 => "count 1",
        2 => "count 2",
        3 => "count 3",
        _ => "count many",
    };
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    let sty = pass::default_styles(&s.ui);
    let c = Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 600.0 };
    pass::layout(&mut s.ui, &sty, &mut st, c);
    let mut out = 0u64;
    pass::paint(&s.ui, &st, &mut out);
    out
}

/// Serialize the keep-half into a HOST-owned buffer. Returns the byte
/// count needed; writes only if `cap` suffices (call once with cap 0 to
/// size, then again with the allocation).
#[no_mangle]
pub unsafe extern "C" fn hot_save(p: *mut c_void, buf: *mut u8, cap: usize) -> usize {
    let bytes = serde_json::to_vec(&session(p).save()).unwrap_or_default();
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
            session(p).restore(&v);
            0
        }
        Err(_) => 1,
    }
}

/// Human-readable state summary into a host buffer (same two-call
/// protocol as `hot_save`). The host asserts against this.
#[no_mangle]
pub unsafe extern "C" fn hot_report(p: *mut c_void, buf: *mut u8, cap: usize) -> usize {
    let s = session(p);
    let report = format!(
        "counter={} mode={:?} theme={} title='{}' status='{}' footer='{}' rows={} scroll={} window={} cache={}",
        s.counter,
        s.mode,
        s.theme,
        s.ui.title.text,
        s.ui.status.text,
        s.ui.footer.text,
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
