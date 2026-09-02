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

struct App<'l> {
    lib: &'l Library,
    ptr: *mut c_void,
}

impl<'l> App<'l> {
    fn new(lib: &'l Library) -> Self {
        let ptr = unsafe {
            let f: Symbol<unsafe extern "C" fn() -> *mut c_void> = lib.get(b"hot_new").unwrap();
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
                self.lib.get(b"hot_save").unwrap();
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
                self.lib.get(b"hot_restore").unwrap();
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
            unsafe { self.lib.get(b"hot_take_log") }.unwrap();
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
    app.frame();
    app.frame();
    let before = app.buffered(b"hot_report");
    println!("v1 before swap:\n  {before}");
    expect(&before, "counter=3", "v1 state");
    expect(&before, "mode=Fancy", "v1 fancy mode");
    expect(&before, "footer='footer-v1'", "v1 code");
    expect(&before, "cache=2", "v1 ran two frames");
    expect(&before, "status='count 3'", "the update step puts app state on screen");

    // ---- the §7 sequence ----
    let bytes = app.save(); // 1. save while old lib loaded (host-owned buffer)
    println!("saved {} bytes of keep-state (JSON)", bytes.len());
    app.call0(b"hot_drop"); // 2. old Drop impls run, old lib still loaded
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
            unsafe { v2.get(b"hot_take_log") }.unwrap();
        let mut tiny = [0u8; 1];
        let need = unsafe { f(tiny.as_mut_ptr(), 1) };
        assert!(need > 1, "the fallback log should be longer than one byte, got {need}");
        println!("  ok: undersized buffer refused, {need} bytes still pending");
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
    expect(&after, "window=24", "repeater window rebuilt");

    println!("\nHOT RELOAD DEMO: PASS");
}
