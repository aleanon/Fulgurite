//! **The stack, measured** — M52/T52.6 (`docs/plan-embedded-footprint.md`
//! §2.6, R11-F3).
//!
//! `docs/results-embedded.md` reported `stack_bytes(DEPTH)` — depth × a
//! per-level constant — as "Measured", and `fixtures/embedded` asserted
//! that model at compile time. A 4 KB local in a widget's `layout` left the
//! assert green, because `DEPTH` did not change. This runs the fixture
//! under qemu (`-M mps2-an386`, a Cortex-M4F), paints the stack with a
//! pattern before each call, and after it scans for the deepest word the
//! call overwrote. `scripts/check_stack.sh` (`just stack`) reads the lines
//! it prints over semihosting and holds each release figure to 4,096 B.
//!
//! **Calibrated first**: a function with a known 2,000 B local must
//! measure 2,000–2,100 B, so a broken painting method fails loudly
//! instead of reading 0 (or 64 KB).
//!
//! The only firmware that runs, and so the only one with a vector table,
//! a reset handler and its own memory map (`fwq.ld`, linked by
//! `build.rs`). `unsafe` is confined here: reading `sp`, the semihosting
//! trap, the FPU enable, and painting memory below the live stack, none
//! of which safe Rust can express. `scripts/check_no_unsafe.sh` covers
//! `crates/*/src`, and this is a fixture's harness.

#![no_std]
#![no_main]

use core::arch::asm;
use core::fmt::Write;
use core::hint::black_box;
use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};

use embedded::{frame_changed, Checksum, Device, DeviceUi, Ui, DEVICE_NODES};
use lumen_static::event::InputState;
use lumen_static::invalidate::{Boundaries, Changed, Invalidation};
use lumen_static::prelude::*;
use lumen_static::{pass, stack_bytes, wid_index, NodeState, Style};

// ---- the machine: vector table, reset, semihosting ----------------------

extern "C" {
    /// Bottom of the painted region (`fwq.ld`): the stack grows down to it.
    static _stack_bottom: u32;
}

/// The reset vector. The initial SP is the table's first word, which
/// `fwq.ld` writes from the top of RAM.
#[link_section = ".vector_table.reset"]
#[no_mangle]
pub static RESET: unsafe extern "C" fn() -> ! = reset;

/// NMI through SysTick, every one a fault: nothing here enables an
/// interrupt, so any exception is a harness failure and must end the run
/// rather than hang qemu.
#[link_section = ".vector_table.exceptions"]
#[no_mangle]
pub static EXCEPTIONS: [unsafe extern "C" fn() -> !; 14] = [fault; 14];

#[no_mangle]
pub unsafe extern "C" fn reset() -> ! {
    // The FPU is off at reset, and this is a hard-float target: CP10 and
    // CP11 get full access before any code that may use an FP register.
    unsafe {
        let cpacr = 0xE000_ED88 as *mut u32;
        write_volatile(cpacr, read_volatile(cpacr) | (0xF << 20));
        asm!("dsb", "isb");
    }
    run()
}

unsafe extern "C" fn fault() -> ! {
    say(format_args!("fwq: fault -- an exception was taken"));
    exit(false)
}

#[panic_handler]
fn panicked(_: &PanicInfo) -> ! {
    say(format_args!("fwq: panic"));
    exit(false)
}

/// Semihosting: `op` in r0, its argument in r1, `bkpt 0xab`.
fn semihost(op: u32, arg: usize) -> u32 {
    let r: u32;
    unsafe { asm!("bkpt #0xab", inout("r0") op => r, in("r1") arg, options(nostack)) };
    r
}

/// Semihosting's exit call, with the application-exit reason (qemu exits
/// 0) or a run-time error (qemu exits 1).
fn exit(ok: bool) -> ! {
    semihost(0x18, if ok { 0x2_0026 } else { 0x2_0023 });
    loop {}
}

/// One line over semihosting's write-a-string call, formatted into a
/// fixed buffer.
fn say(args: core::fmt::Arguments<'_>) {
    struct Line {
        buf: [u8; 160],
        len: usize,
    }
    impl Write for Line {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            for &b in s.as_bytes() {
                if self.len < self.buf.len() - 2 {
                    self.buf[self.len] = b;
                    self.len += 1;
                }
            }
            Ok(())
        }
    }
    let mut l = Line { buf: [0; 160], len: 0 };
    let _ = l.write_fmt(args);
    l.buf[l.len] = b'\n';
    l.buf[l.len + 1] = 0;
    semihost(0x04, l.buf.as_ptr() as usize);
}

// ---- the method: paint, call, scan --------------------------------------

const PAINT: u32 = 0xC5C5_C5C5;

#[inline(always)]
fn sp() -> usize {
    let sp: usize;
    unsafe { asm!("mov {}, sp", out(reg) sp, options(nomem, nostack)) };
    sp
}

fn bottom() -> usize {
    &raw const _stack_bottom as usize
}

/// Paint every word from the stack's bottom to a margin below this
/// function's own frame. Returns the top of what was painted.
#[inline(never)]
fn paint() -> usize {
    let top = sp() - 64;
    let mut p = bottom();
    while p < top {
        unsafe { write_volatile(p as *mut u32, PAINT) };
        p += 4;
    }
    top
}

/// The lowest address a call overwrote: the first word, scanning up from
/// the bottom, that no longer holds the paint.
#[inline(never)]
fn low_water(top: usize) -> usize {
    let mut p = bottom();
    while p < top && unsafe { read_volatile(p as *const u32) } == PAINT {
        p += 4;
    }
    p
}

/// Bytes of stack `f` used below the caller's `sp`, the call included.
#[inline(never)]
fn measure(f: &mut dyn FnMut()) -> usize {
    let top = paint();
    let at = sp();
    f();
    at - low_water(top)
}

/// The calibration: a 2,000 B local, every byte written, kept alive.
#[inline(never)]
fn calibrate() {
    let mut buf = [0u8; 2000];
    for (i, b) in buf.iter_mut().enumerate() {
        *b = i as u8;
    }
    black_box(&mut buf);
}

// ---- the depth sweep: the same tree, nested deeper ----------------------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct D4 {
    inner: Ui,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct D5 {
    inner: D4,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct D6 {
    inner: D5,
}

/// A cold frame of `W`, measured. Arrays are sized for the deepest.
#[inline(never)]
fn sweep<W: lumen_static::Widget + Default>() -> (usize, usize) {
    let mut w = W::default();
    let mut st = [NodeState::default(); 32];
    let mut sty = [Style::default(); 32];
    let n = W::NODE_COUNT;
    pass::default_styles_into(&w, &mut sty[..n]);
    let mut out = Checksum::default();
    let used = measure(&mut || frame_changed(&mut w, &mut st[..n], &sty[..n], Changed::ALL, &mut out));
    black_box(out.hash);
    (W::DEPTH, used)
}

// ---- the run ------------------------------------------------------------

const FIELD: usize = wid_index!(DeviceUi, "field");

#[inline(never)]
fn run() -> ! {
    let profile = if cfg!(debug_assertions) { "dev" } else { "release" };
    let model = stack_bytes(<DeviceUi as lumen_static::Widget>::DEPTH);
    say(format_args!("fwq: profile {profile}, depth {}, stack region {} B", <DeviceUi as lumen_static::Widget>::DEPTH, sp() - bottom()));

    let cal = measure(&mut || calibrate());
    say(format_args!("calibrate: {cal} B (known local 2000 B)"));

    let mut dev = Device::new();
    let mut st = [NodeState::default(); DEVICE_NODES];
    let mut sty = [Style::default(); DEVICE_NODES];
    let mut out = Checksum::default();
    let mut input = InputState::default();
    let mut inv: Boundaries<16> = Boundaries::default();
    pass::default_styles_into(&dev.ui, &mut sty);

    let cold = measure(&mut || frame_changed(&mut dev.ui, &mut st, &sty, Changed::ALL, &mut out));
    say(format_args!("stack: cold frame {cold} B / 4096 (model {model})"));
    let warm = measure(&mut || frame_changed(&mut dev.ui, &mut st, &sty, Changed::ALL, &mut out));
    say(format_args!("stack: warm frame {warm} B / 4096 (model {model})"));

    input.focus = Some(FIELD as u32);
    let mut typed = false;
    let key = measure(&mut || typed = pass::dispatch(&mut dev, &sty, &mut st, &mut input, &Event::Text { ch: 'q' }));
    say(format_args!("stack: typed char {key} B / 4096 (model {model})"));

    dev.clicks = 7;
    inv.clear();
    let mut wrote = lumen_static::event::Wrote::Nothing;
    let bind = measure(&mut || wrote = dev.update_marked(&mut |r| inv.mark(r)));
    say(format_args!("stack: bound update {bind} B / 4096 (model {model})"));

    // The setup must have happened, or the figures above priced no-ops.
    let landed = typed && dev.ui.field.as_str() == "q" && matches!(wrote, lumen_static::event::Wrote::Marked);
    say(format_args!(
        "landed: typed {typed} (field {:?}), bind wrote {wrote:?} ({} ranges marked), {} cmds painted",
        dev.ui.field.as_str(),
        inv.marks().ranges,
        out.cmds
    ));

    for (depth, used) in [sweep::<Ui>(), sweep::<D4>(), sweep::<D5>(), sweep::<D6>()] {
        say(format_args!("sweep: depth {depth} cold frame {used} B (model {})", stack_bytes(depth)));
    }

    say(format_args!("fwq: done"));
    exit(landed)
}
