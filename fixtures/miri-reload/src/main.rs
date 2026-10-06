//! `just miri-reload`: lumen-static-reload's raw-pointer code under miri
//! — M49/T49.3 (`docs/plan-reload-hardening.md`, R4-F8).
//!
//! **The files are the crate's own**, included by path: every pointer
//! operation the generated shims, the panic drain and the loader's
//! session round trip perform lives in `raw.rs`, and the panic record in
//! `panic.rs`, both dependency-free for exactly this. What is exercised
//! is each call shape a shim makes, counted and printed against the
//! expected number, so a shape dropped from the exercise shows.
//!
//! Also a plain `cargo run -p miri-reload`, where it is only a smoke run.

#[path = "../../../crates/lumen-static-reload/src/raw.rs"]
pub mod raw;

#[path = "../../../crates/lumen-static-reload/src/panic.rs"]
pub mod panic;

use std::ffi::c_void;

/// Stand-in for a library's session: a trait object, as `Box<dyn Session>`.
trait Session {
    fn count(&self) -> u32;
    fn bump(&mut self);
}

struct Counter(u32);

impl Session for Counter {
    fn count(&self) -> u32 {
        self.0
    }
    fn bump(&mut self) {
        self.0 += 1;
    }
}

const STAMP: &[u8] = b"rustc=x\nsource lumen-static=0123\n";

/// `lumen_abi_stamp`'s shape: the two-call protocol through `copy_out`.
unsafe extern "C" fn stamp(buf: *mut u8, cap: usize) -> usize {
    unsafe { raw::copy_out(STAMP, buf, cap) }
}

/// `hot_take_panic`'s shape.
unsafe extern "C" fn take_panic(buf: *mut u8, cap: usize) -> usize {
    unsafe { panic::take(buf, cap) }
}

/// `lumen_boot`'s shape since M89/T89.7: a program index, then the saved
/// bytes. 0 boots a session from them; an index with no window program
/// answers null and records why with `panic::refuse`, as
/// `export_program!` does.
unsafe extern "C" fn boot(index: u16, saved: *const u8, len: usize) -> *mut c_void {
    let bytes = unsafe { raw::bytes(saved, len) };
    panic::guard(core::ptr::null_mut(), || {
        if index == 0 {
            let session: Box<dyn Session> = Box::new(Counter(bytes.len() as u32));
            raw::give(session)
        } else {
            panic::refuse(format!("no window program {index}"));
            core::ptr::null_mut()
        }
    })
}

/// The shapes, by name: what `main` must reach.
const EXPECTED: usize = 16;

fn main() {
    let mut reached = Vec::new();
    let mut hit = |name: &'static str| reached.push(name);

    // `hot_restore` / `lumen_boot`: saved bytes, or none.
    let saved = b"{\"n\":42}".to_vec();
    assert_eq!(unsafe { raw::bytes(saved.as_ptr(), saved.len()) }, &saved[..]);
    hit("bytes(ptr, len)");
    assert!(unsafe { raw::bytes(core::ptr::null(), 0) }.is_empty());
    hit("bytes(null, 0)");
    assert!(unsafe { raw::bytes(core::ptr::null(), 99) }.is_empty());
    hit("bytes(null, 99)");

    // `hot_save` and the stamp: size, then write; a small buffer untouched.
    let src = b"hello";
    assert_eq!(unsafe { raw::copy_out(src, core::ptr::null_mut(), 0) }, 5);
    hit("copy_out(null, 0)");
    let mut small = [7u8; 3];
    assert_eq!(unsafe { raw::copy_out(src, small.as_mut_ptr(), small.len()) }, 5);
    assert_eq!(small, [7, 7, 7], "a buffer too small was written");
    hit("copy_out(small)");
    let mut exact = [0u8; 5];
    assert_eq!(unsafe { raw::copy_out(src, exact.as_mut_ptr(), exact.len()) }, 5);
    assert_eq!(&exact, src);
    hit("copy_out(exact)");

    // `hot_new` / `hot_save` / `hot_restore` / `hot_drop` on a plain value.
    let p = raw::give(Counter(1));
    unsafe { raw::get_mut::<Counter>(p) }.expect("live").bump();
    assert_eq!(unsafe { raw::get::<Counter>(p) }.expect("live").0, 2);
    hit("give/get/get_mut");
    drop(unsafe { raw::take::<Counter>(p) }.expect("live"));
    hit("take");
    assert!(unsafe { raw::take::<Counter>(core::ptr::null_mut()) }.is_none());
    assert!(unsafe { raw::get::<Counter>(core::ptr::null()) }.is_none());
    assert!(unsafe { raw::get_mut::<Counter>(core::ptr::null_mut()) }.is_none());
    hit("null tree");

    // `lumen_boot` / `Build::boot` / `Hot::drop` / `lumen_drop`: a boxed
    // trait object, as a thin pointer, both ways.
    let session: Box<dyn Session> = Box::new(Counter(5));
    let p: *mut c_void = raw::give(session);
    let mut back = unsafe { raw::take::<Box<dyn Session>>(p) }.expect("live");
    back.bump();
    assert_eq!(back.count(), 6);
    let p = raw::give(*back);
    drop(unsafe { raw::take::<Box<dyn Session>>(p) });
    hit("Box<dyn Session> round trip");

    // `abi::read` against a stamp export.
    assert_eq!(unsafe { raw::read(stamp) }, STAMP);
    hit("read(stamp)");

    // `panic::guard` and `hot_take_panic`: a caught panic, a report that
    // survives a too-small buffer, then drains once.
    let r = panic::guard(7, || -> i32 { panic!("caught in the library") });
    assert_eq!(r, 7);
    hit("guard(panic)");
    let mut tiny = [0u8; 4];
    let need = unsafe { take_panic(tiny.as_mut_ptr(), tiny.len()) };
    assert!(need > 4, "the report is longer than four bytes: {need}");
    hit("take_panic(small)");
    let report = String::from_utf8(unsafe { raw::read(take_panic) }).expect("utf-8");
    assert!(report.contains("caught in the library"), "{report}");
    assert_eq!(unsafe { take_panic(core::ptr::null_mut(), 0) }, 0, "the report drains once");
    hit("take_panic(read, then empty)");

    // `lumen_boot(index, ..)` (M89/T89.7): index 0 boots from the bytes;
    // index 9 is null, with its refusal in `hot_take_panic`.
    let p = unsafe { boot(0, saved.as_ptr(), saved.len()) };
    let s = unsafe { raw::take::<Box<dyn Session>>(p) }.expect("index 0 boots");
    assert_eq!(s.count(), saved.len() as u32);
    hit("lumen_boot(0, saved)");
    let p = unsafe { boot(9, core::ptr::null(), 0) };
    assert!(p.is_null(), "an unknown window program must answer null");
    let report = String::from_utf8(unsafe { raw::read(take_panic) }).expect("utf-8");
    assert_eq!(report, "no window program 9", "{report}");
    hit("lumen_boot(9): refused");

    reached.sort_unstable();
    reached.dedup();
    println!("shims exercised: {} / expected {EXPECTED}", reached.len());
    assert_eq!(reached.len(), EXPECTED, "{reached:?}");
    println!("MIRI RELOAD: PASS");
}
