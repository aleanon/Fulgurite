//! A **linked** `thumbv7em-none-eabihf` binary, so `.text` and `.bss`
//! are measurable — `docs/plan-embedded.md` T13.9.
//!
//! `cargo check` proves none of what T13.9 asks: it type-checks a
//! library and stops, with no codegen, no link, no missing intrinsics
//! and no section sizes. This is the smallest thing that produces real
//! ones — a panic handler, an entry symbol, and one frame that the
//! optimizer cannot delete.
//!
//! It is still **not a board bring-up**, which
//! `docs/plan-embedded.md` "What M13 is not" excludes: there is no
//! memory map, no vector table, no peripheral. The linker is told only
//! where to start. Flashing this would do nothing useful; measuring it
//! answers the one question the milestone actually asked.
//!
//! Built by `just firmware-size`, which is where the numbers in
//! `docs/results-embedded.md` come from.

#![no_std]
#![no_main]

use core::panic::PanicInfo;

use embedded::{frame, Checksum, Panel, NODES};
use lumen_static::{NodeState, Style};

#[panic_handler]
fn panicked(_: &PanicInfo) -> ! {
    loop {}
}

/// Where the checksum lands, so the whole frame cannot be optimized
/// away as dead. A `static mut` would need `unsafe`; this is an atomic
/// for the same reason the rest of the core has none (M0).
///
/// `AtomicU32`, not `AtomicU64`: **Cortex-M4 has no 64-bit atomics**,
/// so `core::sync::atomic::AtomicU64` does not exist on this target.
/// The kind of thing only a real build finds — `cargo check` on a
/// library would never have said so.
static SINK: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let mut panel = Panel::new();
    let mut st = [NodeState::default(); NODES];
    let mut sty = [Style::default(); NODES];
    let mut out = Checksum::default();

    frame(&mut panel, &mut st, &mut sty, &mut out);
    let folded = (out.hash ^ (out.hash >> 32)) as u32 ^ out.glyphs;
    SINK.store(folded, core::sync::atomic::Ordering::Relaxed);

    loop {}
}
