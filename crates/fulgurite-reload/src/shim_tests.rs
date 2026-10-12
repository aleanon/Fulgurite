//! The generated shims, exported and called **in-process** — M49/T49.0
//! (`docs/plan-reload-hardening.md`, R4-F6 and R4-F8).
//!
//! Every other exercise of `export_reloadable_app!` goes through
//! dlopen, which miri cannot run, so the crate's `unsafe` was reached
//! by nothing that checks for undefined behaviour. Exporting the shims
//! into this test binary and calling them as ordinary `extern "C"`
//! functions reaches the same code with no dynamic loader:
//! `just miri-reload` runs this module under miri, and `cargo test`
//! runs it in a debug build, whose standard library checks
//! `slice::from_raw_parts`'s preconditions.
//!
//! **What is counted.** Each call goes through [`call`], which records
//! the shim's name; the round trip asserts the set it reached against
//! [`EXPECTED`] and prints both, so a shim dropped from the exercise is
//! visible in the output rather than silently unexercised.

use std::sync::Mutex;

use fulgurite::reload::{json, log_fallback, Reloadable, Value};

/// A tiny application: one kept number.
struct Probe {
    n: u32,
}

impl Reloadable for Probe {
    fn save(&self) -> Value {
        json!({ "n": self.n })
    }
    fn restore(&mut self, saved: &Value) {
        match saved.get("n").and_then(Value::as_u64) {
            Some(n) => self.n = n as u32,
            None => log_fallback("Probe", "n", "missing"),
        }
    }
}

fn boot() -> Probe {
    Probe { n: 7 }
}

crate::export_reloadable_app!(Probe, boot);
crate::export_abi_stamp!();

/// The shims this module must reach, by name.
const EXPECTED: &[&str] = &[
    "hot_new",
    "hot_save(null, 0)",
    "hot_save",
    "hot_restore",
    "hot_drop",
    "hot_take_panic(null, 0)",
    "hot_take_panic",
    "fulgurite_abi_stamp(null, 0)",
    "fulgurite_abi_stamp",
];

static REACHED: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

fn call<R>(name: &'static str, f: impl FnOnce() -> R) -> R {
    REACHED.lock().unwrap().push(name);
    f()
}

#[test]
fn a_real_round_trip_reaches_every_shim() {
    let p = call("hot_new", || unsafe { hot_new() });
    assert!(!p.is_null(), "hot_new returned null");
    let need = call("hot_save(null, 0)", || unsafe { hot_save(p, core::ptr::null_mut(), 0) });
    let mut buf = vec![0u8; need];
    let wrote = call("hot_save", || unsafe { hot_save(p, buf.as_mut_ptr(), buf.len()) });
    assert_eq!(wrote, need, "the sizing call and the writing call disagree");
    assert_eq!(&buf, br#"{"n":7}"#);

    let saved = br#"{"n":42}"#;
    let rc = call("hot_restore", || unsafe { hot_restore(p, saved.as_ptr(), saved.len()) });
    assert_eq!(rc, 0, "a real (ptr, len) restore must return 0");
    assert_eq!(unsafe { &*(p as *const Probe) }.n, 42, "the restore did not land");

    // Nothing panicked, so there is no report: both calls answer zero.
    let size = call("hot_take_panic(null, 0)", || unsafe { hot_take_panic(core::ptr::null_mut(), 0) });
    let mut small = [0u8; 4];
    let drained = call("hot_take_panic", || unsafe { hot_take_panic(small.as_mut_ptr(), small.len()) });
    assert_eq!((size, drained), (0, 0), "a report exists though nothing panicked");

    let stamp = crate::abi::stamp(&[]);
    let len = call("fulgurite_abi_stamp(null, 0)", || unsafe { fulgurite_abi_stamp(core::ptr::null_mut(), 0) });
    assert_eq!(len, stamp.len(), "the sizing call does not give the stamp's length");
    let read = call("fulgurite_abi_stamp", || unsafe { crate::abi::read(fulgurite_abi_stamp) });
    assert_eq!(read, stamp, "the exported stamp is not this image's");

    call("hot_drop", || unsafe { hot_drop(p) });

    let mut reached = REACHED.lock().unwrap().clone();
    reached.sort_unstable();
    reached.dedup();
    let mut want = EXPECTED.to_vec();
    want.sort_unstable();
    println!("shims exercised: {} / expected {}", reached.len(), want.len());
    assert_eq!(reached, want, "the exercise did not reach every shim it names");
}

/// R4-F6: `(null, 0)` is how the two-call protocol spells "nothing", and
/// `SAVE` and the stamp accept it. `hot_restore` handed it straight to
/// `slice::from_raw_parts`, which requires a non-null pointer even for an
/// empty slice.
#[test]
fn restore_accepts_null_and_zero() {
    let p = unsafe { hot_new() };
    let rc = unsafe { hot_restore(p, core::ptr::null(), 0) };
    println!("hot_restore(p, null, 0) = {rc} (1: not a saved state)");
    assert_eq!(rc, 1, "an empty state is not a readable one");
    assert_eq!(unsafe { &*(p as *const Probe) }.n, 7, "a refused restore changed the value");
    unsafe { hot_drop(p) };
}

/// R4-F6: a drop of null returns without touching it.
#[test]
fn drop_accepts_null() {
    unsafe { hot_drop(core::ptr::null_mut()) };
}

/// R4-F6: a save through a null application is refused, not read.
#[test]
fn save_refuses_a_null_application() {
    let n = unsafe { hot_save(core::ptr::null_mut(), core::ptr::null_mut(), 0) };
    assert_eq!(n, usize::MAX, "a null application has nothing to save");
}
