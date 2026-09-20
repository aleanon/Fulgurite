//! Hot-reload host: drives the §7 sequence against two builds of
//! `hot-app` and asserts every reload class behaved. Exit code is the
//! verdict, so `scripts/hot_reload_demo.sh` doubles as an integration
//! test.
//!
//! The sequence, exactly as the handoff fixes it:
//!   1. `save()` while the old lib is still loaded → pointer-free bytes
//!      in a HOST-owned buffer;
//!   2. drop the entire old tree (old Drop impls run), old lib loaded;
//!   3. never `dlclose` — the old library is leaked deliberately;
//!   4. load the new lib, `fresh()`, then `restore(bytes)`.

use std::ffi::c_void;

use libloading::{Library, Symbol};
// The generic entry points by name from the framework, not by literal:
// both sides referenced twelve byte strings each with nothing checking
// they agreed, so a rename was a runtime `unwrap` at load time in the
// middle of a dylib swap (`docs/plan-review-fixes.md` F6.7).
use lumen_static::reload::symbols;

struct App<'l> {
    lib: &'l Library,
    ptr: *mut c_void,
}

impl<'l> App<'l> {
    fn new(lib: &'l Library) -> Self {
        let ptr = unsafe {
            let f: Symbol<unsafe extern "C" fn() -> *mut c_void> = lib.get(symbols::NEW).unwrap();
            f()
        };
        App { lib, ptr }
    }

    fn call0(&self, name: &[u8]) {
        unsafe {
            let f: Symbol<unsafe extern "C" fn(*mut c_void)> = self.lib.get(name).unwrap();
            f(self.ptr)
        }
    }

    fn call_u32(&self, name: &[u8]) -> u32 {
        unsafe {
            let f: Symbol<unsafe extern "C" fn(*mut c_void) -> u32> = self.lib.get(name).unwrap();
            f(self.ptr)
        }
    }

    fn frame(&self) -> u64 {
        unsafe {
            let f: Symbol<unsafe extern "C" fn(*mut c_void) -> u64> =
                self.lib.get(b"hot_frame").unwrap();
            f(self.ptr)
        }
    }

    fn buffered(&self, name: &[u8]) -> String {
        // Single call with a generously sized host buffer (the fns return
        // the true length; hot_take_log drains, so no size-probe call).
        let mut buf = vec![0u8; 64 * 1024];
        let n = unsafe {
            let f: Symbol<unsafe extern "C" fn(*mut c_void, *mut u8, usize) -> usize> =
                self.lib.get(name).unwrap();
            f(self.ptr, buf.as_mut_ptr(), buf.len())
        };
        assert!(n <= buf.len(), "{}: buffer too small", String::from_utf8_lossy(name));
        String::from_utf8_lossy(&buf[..n]).into_owned()
    }

    fn save(&self) -> Vec<u8> {
        unsafe {
            let f: Symbol<unsafe extern "C" fn(*mut c_void, *mut u8, usize) -> usize> =
                self.lib.get(symbols::SAVE).unwrap();
            let need = f(self.ptr, core::ptr::null_mut(), 0);
            let mut buf = vec![0u8; need]; // HOST-owned, never crosses allocators
            let wrote = f(self.ptr, buf.as_mut_ptr(), buf.len());
            assert_eq!(wrote, need);
            buf
        }
    }

    fn restore(&self, bytes: &[u8]) {
        let rc = unsafe {
            let f: Symbol<unsafe extern "C" fn(*mut c_void, *const u8, usize) -> i32> =
                self.lib.get(symbols::RESTORE).unwrap();
            f(self.ptr, bytes.as_ptr(), bytes.len())
        };
        assert_eq!(rc, 0, "restore failed");
    }

    /// The same two-call probe `save` uses. The old version guessed 64 KB
    /// and sliced `&buf[..n]` with whatever came back — a reload
    /// producing more fallback lines than that panicked the host, after
    /// the dylib had already drained the log. Both halves are fixed;
    /// this is the host's.
    fn take_log(&self) -> String {
        let f: Symbol<unsafe extern "C" fn(*mut u8, usize) -> usize> =
            unsafe { self.lib.get(symbols::TAKE_LOG) }.unwrap();
        let need = unsafe { f(core::ptr::null_mut(), 0) };
        if need == 0 {
            return String::new();
        }
        let mut buf = vec![0u8; need];
        let wrote = unsafe { f(buf.as_mut_ptr(), buf.len()) };
        assert!(wrote <= buf.len(), "hot_take_log wrote past the host's buffer");
        String::from_utf8_lossy(&buf[..wrote]).into_owned()
    }
}

/// Assert that `needle` appears in `report` as a WHOLE space-delimited field.
///
/// `report.contains(needle)` was wrong in both directions in a `k=v k=v`
/// format: `window=` matched `window=0`, which is exactly the "the repeater
/// window failed to rebuild" case the assertion exists to catch, and
/// `counter=3` would equally have matched `counter=30`. Requiring the match to
/// end at a space or at the end of the report closes both. Needles may still
/// contain spaces inside quotes (`title='renamed by user'`), so this is a
/// suffix-delimiter rule, not a tokenizer.
fn expect(report: &str, needle: &str, what: &str) {
    let matched = report
        .match_indices(needle)
        .any(|(i, _)| report[i + needle.len()..].starts_with(' ') || i + needle.len() == report.len());
    assert!(matched, "{what}: expected field `{needle}` in `{report}`");
    println!("  ok: {what} ({needle})");
}

/// The fixture's only chain — M28/T28.7. A stable, append-only id, which
/// is `docs/plan-tasks.md` §2.1's rule: a descriptor whose `(kind, step)`
/// the loaded build does not recognise is dropped and counted, never
/// guessed at.
const CHAIN_KIND: u64 = 1;

fn main() {
    let mut args = std::env::args().skip(1);
    let (v1_path, v2_path) = (args.next().expect("v1 .so path"), args.next().expect("v2 .so path"));

    // ---- v1: build up user state ----
    let v1: &'static Library = Box::leak(Box::new(unsafe { Library::new(&v1_path) }.unwrap()));
    let app = App::new(v1);
    for _ in 0..3 {
        app.call0(b"hot_bump");
    }
    unsafe {
        let f: Symbol<unsafe extern "C" fn(*mut c_void, *const u8, usize)> =
            v1.get(b"hot_set_title").unwrap();
        let t = b"renamed by user";
        f(app.ptr, t.as_ptr(), t.len());
        let f: Symbol<unsafe extern "C" fn(*mut c_void, usize)> = v1.get(b"hot_set_rows").unwrap();
        f(app.ptr, 47);
        let f: Symbol<unsafe extern "C" fn(*mut c_void, f32)> = v1.get(b"hot_set_scroll").unwrap();
        f(app.ptr, 240.0);
    }
    app.call0(b"hot_set_fancy");
    app.call0(b"hot_set_check");
    app.frame();
    app.frame();
    let before = app.buffered(b"hot_report");
    println!("v1 before swap:\n  {before}");
    expect(&before, "counter=3", "v1 state");
    expect(&before, "mode=Fancy", "v1 fancy mode");
    expect(&before, "footer='footer-v1'", "v1 code");
    expect(&before, "cache=2", "v1 ran two frames");
    expect(&before, "status='count 3'", "the update step puts app state on screen");
    expect(&before, "check=on", "v1 checked the box");
    // M25/T25.10: the tick probe reads the painted mark -- asserted before the swap
    // too, so a probe returning a constant cannot pass both halves.
    let mark_v1 = app.call_u32(b"hot_check_mark");
    println!("v1 tick colour {mark_v1:#010x}");
    assert_eq!(mark_v1, 0xc8_1e_1e_ff, "v1's default paints a red tick");

    // ---- M28/T28.7: start a chain against v1 -------------------------
    //
    // **The chain lives here, in the loader process, outside both
    // dylibs.** That arrangement is forced by §7's sequence rather than
    // chosen: step 2 below destroys the entire app object while the old
    // library is still loaded, so anything the chain needs to survive
    // the swap cannot live in the app.
    //
    // What is held across the swap is the *pending descriptor* — three
    // `u64`s. Not a function pointer: that is the whole point, and the
    // injection for this gate is to hold one and watch the assertion
    // fire with v1's value.
    let step_v1: Symbol<unsafe extern "C" fn(u64, u64, u64) -> u64> =
        unsafe { v1.get(b"hot_step") }.unwrap();
    let out0 = unsafe { step_v1(CHAIN_KIND, 0, 4) };
    assert_eq!(out0, 5, "v1 ran step 0: 4 + 1");
    // The completion says which step is next. An index, not a closure.
    let pending: (u64, u64, u64) = (CHAIN_KIND, 1, out0);
    println!("chain: v1 ran step 0 -> {out0}; pending descriptor {pending:?} held across the swap");

    // ---- the §7 sequence ----
    let bytes = app.save(); // 1. save while old lib loaded (host-owned buffer)
    println!("saved {} bytes of keep-state (JSON)", bytes.len());
    app.call0(symbols::DROP); // 2. old Drop impls run, old lib still loaded
    // 3. v1 is Box::leak'd: never dlclose'd.

    // ---- v2: fresh + restore ----
    let v2: &'static Library = Box::leak(Box::new(unsafe { Library::new(&v2_path) }.unwrap()));
    let app2 = App::new(v2); // 4. fresh() from the new code…
    app2.restore(&bytes); //    …then graft the old values on
    app2.frame();
    let after = app2.buffered(b"hot_report");

    // The size probe is load-bearing, so exercise it directly: a first
    // call with a one-byte buffer must not deliver and must not lose the
    // log (F1.7 — the old version drained it and the host then sliced
    // past its own allocation).
    {
        let f: Symbol<unsafe extern "C" fn(*mut u8, usize) -> usize> =
            unsafe { v2.get(symbols::TAKE_LOG) }.unwrap();
        let mut tiny = [0u8; 1];
        let need = unsafe { f(tiny.as_mut_ptr(), 1) };
        assert!(need > 1, "the fallback log should be longer than one byte, got {need}");
        println!("  ok: undersized buffer refused, {need} bytes still pending");
    }

    // ---- M28/T28.7: the pending completion lands in v2 ----------------
    //
    // The descriptor was made by v1 and is interpreted by v2. This is
    // `docs/events-decision.md` rule 3's whole claim, as an assertion:
    // work in flight carries a **value**, and whatever build is loaded
    // when it lands decides what the value means.
    //
    // v1's step 1 is `payload * 10` and v2's is `payload * 1000`. Both
    // builds agree about step 0, deliberately — if they differed too,
    // this could not tell "v2 ran the second step" from "v2 ran
    // everything".
    {
        let step_v2: Symbol<unsafe extern "C" fn(u64, u64, u64) -> u64> =
            unsafe { v2.get(b"hot_step") }.unwrap();
        let (kind, step, payload) = pending;
        let out1 = unsafe { step_v2(kind, step, payload) };
        println!("chain: v2 ran step {step} on payload {payload} -> {out1}");
        assert_ne!(
            out1, 50,
            "the chain completed into V1's step function (5 * 10): the next step was held              as a code pointer captured at spawn, which is what rule 3 forbids"
        );
        assert_eq!(
            out1, 5000,
            "the chain must complete into v2's step function (5 * 1000); got {out1}"
        );
        println!("  ok: a chain started before the swap completed into the NEW build's step");
    }

    let log = app2.take_log();
    println!("v2 after swap:\n  {after}");
    println!("v2 fallback log:\n  {}", if log.is_empty() { "(empty)" } else { &log });

    // keep: user state survived
    expect(&after, "counter=3", "keep: counter");
    expect(&after, "title='renamed by user'", "keep: nested title");
    expect(&after, "rows=47", "keep: repeater count (fresh gave 0, restore gave 47)");
    expect(&after, "scroll=240", "keep: scroll position");
    // fresh: the edited code won
    expect(&after, "footer='footer-v2'", "fresh: new footer");
    expect(&after, "theme=theme-v2", "fresh: new theme");
    // reset: recomputed after the swap
    expect(&after, "cache=1", "reset: cache recomputed (one post-swap frame)");
    expect(&after, "status='count 3'", "derived: re-derived from the kept counter");
    // §12.2: removed variant fell back and LOGGED
    expect(&after, "mode=Basic", "enum migration: fell back to fresh variant");
    assert!(log.contains("mode"), "fallback must be logged, got: `{log}`");
    println!("  ok: enum fallback logged (§12.2)");
    // the repeater window re-materialized from data after the swap
    // The value, not the prefix: `window=` alone was satisfied by `window=0`.
    // 23, not 24, since M25/T25.5 put a 24 px checkbox in the same 600 px
    // column: the window is whatever the remaining height holds, rebuilt
    // from `rows` and `scroll` after the swap -- which is the claim.
    expect(&after, "window=23", "repeater window rebuilt");
    // M25/T25.10: a toggle's `on` is saved (kept), its mark is not (fresh).
    expect(&after, "check=on", "keep: the toggle's on state");
    let mark_v2 = app2.call_u32(b"hot_check_mark");
    println!("v2 tick colour {mark_v2:#010x}");
    assert_eq!(mark_v2, 0x1e_3c_dc_ff, "fresh: the new build's default paints a blue tick");
    println!("  ok: the mark came from the new build and the on state from the old");

    println!("\nHOT RELOAD DEMO: PASS");
}
