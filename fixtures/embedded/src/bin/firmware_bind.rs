//! **The bind-only firmware** — M52/T52.1 (`docs/plan-embedded-footprint.md`).
//!
//! One `#[bind]` update over [`embedded::Device`]'s tree and no pointer
//! dispatch, so what a binding links is priced alone: the `BindSite` static
//! (F-NEW-1, which `scripts/check_bss.sh` names) and `bind::node_of`'s walk
//! (F-NEW-2, whose `Node>::hit` symbols `scripts/check_text.sh` counts).

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::sync::atomic::{AtomicU32, Ordering};

use embedded::{frame_changed, Checksum, Device, DEVICE_NODES};
use fulgurite::invalidate::{Boundaries, Changed, Invalidation};
use fulgurite::{pass, App, NodeState, Style};

#[panic_handler]
fn panicked(_: &PanicInfo) -> ! {
    loop {}
}

/// See `firmware.rs`'s `SINK`.
#[no_mangle]
pub static SINK: AtomicU32 = AtomicU32::new(0);

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let mut dev = Device::new();
    let mut st = [NodeState::default(); DEVICE_NODES];
    let mut sty = [Style::default(); DEVICE_NODES];
    let mut out = Checksum::default();
    let mut inv: Boundaries<16> = Boundaries::default();
    inv.clear();

    pass::default_styles_into(&dev.ui, &mut sty);
    frame_changed(&mut dev.ui, &mut st, &sty, Changed::ALL, &mut out);
    dev.clicks = 3;
    let wrote = dev.update_marked(&mut |r| inv.mark(r));
    frame_changed(&mut dev.ui, &mut st, &sty, inv.view(), &mut out);

    let folded = (out.hash ^ (out.hash >> 32)) as u32 ^ (inv.marks().ranges as u32) ^ (wrote as u32) << 16;
    SINK.store(folded, Ordering::Relaxed);
    loop {}
}
