//! **The cascade-only firmware** — M52/T52.1 (`docs/plan-embedded-footprint.md`,
//! R12-F4).
//!
//! `cascade_deep_with` over the paint-only firmware's own tree and no
//! dispatch. A stylesheet on a device that takes no input should cost the
//! cascade and nothing else; `scripts/check_text.sh` counts the `on_event`
//! symbols it links, whose bound is 0.

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::sync::atomic::{AtomicU32, Ordering};

use embedded::{frame_changed, Checksum, Panel, NODES, SHEET};
use lumen_static::invalidate::Changed;
use lumen_static::style::{cascade_deep_with, Palette};
use lumen_static::{NodeState, Style};

#[panic_handler]
fn panicked(_: &PanicInfo) -> ! {
    loop {}
}

/// See `firmware.rs`'s `SINK`.
#[no_mangle]
pub static SINK: AtomicU32 = AtomicU32::new(0);

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let mut panel = Panel::new();
    let mut st = [NodeState::default(); NODES];
    let mut sty = [Style::default(); NODES];
    let mut out = Checksum::default();

    cascade_deep_with(&mut panel.ui, SHEET, &Palette::LIGHT, &mut sty);
    frame_changed(&mut panel.ui, &mut st, &sty, Changed::ALL, &mut out);

    let folded = (out.hash ^ (out.hash >> 32)) as u32 ^ out.glyphs;
    SINK.store(folded, Ordering::Relaxed);
    loop {}
}
