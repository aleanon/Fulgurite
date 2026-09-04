//! The host half of the T13.3 gate: run the `#![no_std]` frame under an
//! allocator that aborts, and check it never fires.
//!
//! Two things this deliberately is not:
//!
//! - **Not a flashable artifact.** A real `thumbv7em-none-eabihf` binary needs a
//!   `#[panic_handler]`, an `#[entry]`, `panic = "abort"` and a linker
//!   script with a concrete memory map — a board, which
//!   `docs/plan-embedded.md` "What M13 is not" puts out of scope. The
//!   thumb half of this fixture is the *library* check, `just
//!   thumb`.
//! - **Not the strong half.** A panicking allocator only covers paths
//!   that execute. The static checks — this crate's lib compiling for
//!   thumb, and T13.4's `--no-default-features` build where `Vec` cannot
//!   be *named* — are what actually prove the property.
//!
//! `scripts/check_no_unsafe.sh` scans `crates/*/src`, so the
//! `unsafe impl GlobalAlloc` below is not an M0 violation. Worth saying
//! outright, because a reader will assume it is.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, Ordering};

use embedded::{frame, Checksum, Panel, NODES};
use lumen_static::{NodeState, Style};

static ARMED: AtomicBool = AtomicBool::new(false);

struct Abort;

// Abort, not `panic!`: std's panic machinery boxes its payload, which
// allocates, which re-enters the allocator we are standing in.
// `crates/lumen-static-shell/tests/alloc_gate.rs` counts instead and
// says why; here the frame under test is a single call, so aborting at
// the exact allocation is both simpler and a sharper signal than a count
// read afterwards. `process::abort` allocates nothing.
unsafe impl GlobalAlloc for Abort {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if ARMED.load(Ordering::Relaxed) {
            std::process::abort();
        }
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}

#[global_allocator]
static A: Abort = Abort;

fn main() {
    let mut panel = Panel::new();
    let mut st = [NodeState::default(); NODES];
    let mut sty = [Style::default(); NODES];
    let mut out = Checksum::default();

    // Warm frame outside the window: `Measured` memos and any one-time
    // setup are steady-state concerns, not first-frame ones, and the
    // property M13 needs is about the steady state (§3, §8).
    frame(&mut panel, &mut st, &mut sty, &mut out);
    let warm = (out.hash, out.cmds);

    println!("embedded: {NODES} nodes, arming the allocator for 100 frames");
    ARMED.store(true, Ordering::Relaxed);
    for _ in 0..100 {
        out = Checksum::default();
        frame(&mut panel, &mut st, &mut sty, &mut out);
    }
    ARMED.store(false, Ordering::Relaxed);

    assert_eq!(
        (out.hash, out.cmds),
        warm,
        "a still tree painted a different frame the second time"
    );
    assert!(out.cmds > 0, "the frame drew nothing, so it proved nothing");

    println!(
        "embedded: {} cmds, checksum {:#018x}, zero allocations over 100 frames",
        out.cmds, out.hash
    );
}
