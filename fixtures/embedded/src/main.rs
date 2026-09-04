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

use embedded::{frame, type_char, Checksum, Panel, NODES};
use lumen_static::{NodeState, Style};

/// The `field`'s index in `Ui`'s node table: title, badge, field. An
/// embedded app has no agent to ask, so it is a constant.
const FIELD: u32 = 3;

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

    assert_eq!(
        (out.hash, out.cmds),
        warm,
        "a still tree painted a different frame the second time"
    );
    assert!(out.cmds > 0, "the frame drew nothing, so it proved nothing");

    // T13.5's gate: type into a TextInput<InlineText<64>> through the
    // real dispatch path, still with the allocator armed. `Label` and
    // `Button` never allocate because they never change; an editable
    // field is the widget that would.
    let mut focus = Some(FIELD);
    for ch in "hello embedded".chars() {
        assert!(type_char(&mut panel, &mut st, &sty, &mut focus, ch), "the field took it");
    }
    frame(&mut panel, &mut st, &mut sty, &mut out);

    // Past capacity the buffer refuses rather than growing — the whole
    // difference between a fixed storage and a String.
    for _ in 0..200 {
        type_char(&mut panel, &mut st, &sty, &mut focus, 'x');
    }
    frame(&mut panel, &mut st, &mut sty, &mut out);
    ARMED.store(false, Ordering::Relaxed);

    let typed = panel.ui.field.as_str();
    assert!(typed.starts_with("hello embedded"), "typed text landed: {typed:?}");
    assert_eq!(typed.len(), 64, "filled to InlineText<64>'s capacity and stopped");

    // T13.7: the run reached real glyphs, not just a &str that nothing
    // could draw. `missing` is zero because the baked face covers
    // printable ASCII and nothing here types outside it.
    assert!(out.glyphs > 0, "no glyph resolved through the baked face");
    assert_eq!(out.missing, 0, "the face lacked a character the app used");

    println!(
        "embedded: {} cmds, {} glyphs ({} bytes of face), checksum {:#018x}, \
         {} bytes typed, zero allocations",
        out.cmds,
        out.glyphs,
        embedded::face::FACE.cov.len(),
        out.hash,
        typed.len()
    );
}
