//! **The interactive firmware** — M52/T52.1 (`docs/plan-embedded-footprint.md`,
//! R11-F4).
//!
//! `firmware.rs` lays out and paints one frame: no `#[bind]`, no event, no
//! cascade. M29 and M33 then grew binding and dispatch by ~1 KB of `.text`,
//! added a per-binding static and a vtable walk, and every embedded gate
//! stayed green, because the only binary they read contained none of it.
//! This one does what a device does: it cascades a three-rule sheet,
//! presses and releases a button, types a character into the field
//! (T13.5's path), runs one `#[bind]` update, and marks and clears a
//! `Boundaries<16>` around all of it.
//!
//! `scripts/check_text.sh` refuses to read its `.text` unless dispatch, an
//! `on_event`, the cascade and a `BindSite` are linked — the same
//! precondition the paint-only firmware has for `Toggle` — so it cannot
//! quietly stop being interactive (`docs/results-embedded.md`).
//!
//! Like `firmware.rs`, not a board bring-up: `fwq.rs` is the one that runs.

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::sync::atomic::{AtomicU32, Ordering};

use embedded::{centre, frame_changed, Checksum, Device, DeviceUi, DEVICE_NODES, SHEET};
use fulgurite::event::InputState;
use fulgurite::invalidate::{Boundaries, Invalidation};
use fulgurite::style::{cascade_deep_with, subtree_range, Palette};
use fulgurite::{pass, wid_index, App, Event, NodeState, PointerButton, Style, Widget};

#[panic_handler]
fn panicked(_: &PanicInfo) -> ! {
    loop {}
}

/// The fold of everything the run produced, so none of it is dead code.
/// `#[no_mangle] pub` for the reason `firmware.rs`'s is; the one
/// exempted symbol in `scripts/bss.allow`.
#[no_mangle]
pub static SINK: AtomicU32 = AtomicU32::new(0);

const INC: usize = wid_index!(DeviceUi, "inc");
const FIELD: usize = wid_index!(DeviceUi, "field");

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let mut dev = Device::new();
    let mut st = [NodeState::default(); DEVICE_NODES];
    let mut sty = [Style::default(); DEVICE_NODES];
    let mut out = Checksum::default();
    let mut inv: Boundaries<16> = Boundaries::default();
    let mut input = InputState::default();
    let table = DeviceUi::SUB_TABLE;

    cascade_deep_with(&mut dev.ui, SHEET, &Palette::LIGHT, &mut sty);
    // A fresh `Boundaries` is "everything", so the first frame is whole.
    frame_changed(&mut dev.ui, &mut st, &sty, inv.view(), &mut out);
    inv.clear();

    let (x, y) = centre(&st, INC);
    let down = Event::PointerDown { x, y, button: PointerButton::Left, clicks: 1 };
    let up = Event::PointerUp { x, y, button: PointerButton::Left };
    let pressed = pass::dispatch(&mut dev, &sty, &mut st, &mut input, &down)
        | pass::dispatch(&mut dev, &sty, &mut st, &mut input, &up);
    if pressed {
        inv.mark(subtree_range(table, INC));
    }
    input.focus = Some(FIELD as u32);
    if pass::dispatch(&mut dev, &sty, &mut st, &mut input, &Event::Text { ch: '7' }) {
        inv.mark(subtree_range(table, FIELD));
    }
    let wrote = dev.update_marked(&mut |r| inv.mark(r));
    frame_changed(&mut dev.ui, &mut st, &sty, inv.view(), &mut out);
    let marks = inv.marks();
    inv.clear();

    let folded = (out.hash ^ (out.hash >> 32)) as u32
        ^ dev.clicks
        ^ (marks.ranges as u32) << 8
        ^ (wrote as u32) << 16;
    SINK.store(folded, Ordering::Relaxed);
    loop {}
}
