//! M51 (R9-F8): what the GPU frame path allocates that this crate owns.
//!
//! wgpu allocates on every frame to encode commands, and that floor is
//! not this crate's to remove. So the gate measures it with an
//! empty-list frame — one page quad, one pass, one submission — and then
//! a caret-sized `draw_regions` over a label and its caret, and asserts
//! that the **difference** is zero (`docs/plan-gpu-fidelity.md` §2.4).
//! It prints total, floor and difference.
//!
//! **And the floor itself against wgpu's alone** (M51/T51.4). A `Vec`
//! this crate built on *every* frame landed in the empty frame as well, so
//! "caret minus empty" read 0 with `quads` recreated per call — the plan's
//! own injection left the gate green. `Gpu::submit_floor` submits one page
//! quad from the retained scratch and nothing else; the empty frame minus
//! that is what `draw_regions` adds around wgpu, and must be 0 too.
//!
//! What the subtraction means on each adapter: Mesa's own allocations
//! (llvmpipe) go through the C allocator and are invisible to the Rust
//! allocator on both sides; a discrete driver's floor may differ from
//! llvmpipe's, but the difference is this crate's either way.
//!
//! Its own binary, because `#[global_allocator]` is per-binary. Counted
//! per thread, as `fulgurite-shell/tests/alloc_gate.rs` does and for
//! its reason: tests run in parallel threads of one process.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use fulgurite::display::{Color, DisplayList};
use fulgurite_wgpu::Gpu;

thread_local! {
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}
#[global_allocator]
static A: Counting = Counting;

fn allocs() -> usize {
    ALLOCS.with(Cell::get)
}

const W: u32 = 320;
const H: u32 = 120;
const PAGE: Color = Color(250, 250, 250, 255);

/// Allocations made by one call of `f`, read outside any formatting.
fn count(f: impl FnOnce()) -> usize {
    let before = allocs();
    f();
    allocs() - before
}

#[test]
fn a_caret_frame_allocates_nothing_this_crate_owns() {
    let Some(mut g) = Gpu::headless(W, H) else {
        assert!(
            std::env::var("FULGURITE_REQUIRE_GPU").as_deref() != Ok("1"),
            "FULGURITE_REQUIRE_GPU=1 but no adapter"
        );
        eprintln!("gpu: no adapter — skipping");
        return;
    };
    eprintln!("gpu: {}", g.adapter());
    let face = fulgurite_shell::text::FontStack::system()
        .expect("the caret frame shapes a label, which needs a real face");
    g.set_font(face);

    let empty = DisplayList::new();
    let mut caret = DisplayList::new();
    caret.text([10.0, 40.0, 200.0, 60.0], "a label with a caret", 14.0, Color(0, 0, 0, 255));
    caret.rect([150.0, 42.0, 151.0, 58.0], Color(0, 0, 0, 255));
    let region = [[149.0, 40.0, 153.0, 60.0]];
    let whole = [[0.0, 0.0, W as f32, H as f32]];

    // Warm both paths up — atlas filled, buffers grown — until two
    // consecutive floors agree; the review saw 100, 95, 92, 92.
    let mut floors = Vec::new();
    for _ in 0..50 {
        g.draw_regions(&caret, &region, Some(PAGE));
        g.wait();
        let f = count(|| g.draw_regions(&empty, &whole, Some(PAGE)));
        g.wait();
        floors.push(f);
        let n = floors.len();
        if n >= 2 && floors[n - 1] == floors[n - 2] {
            break;
        }
    }
    let floor = *floors.last().unwrap();
    let mut wgpu_only = 0;
    for _ in 0..3 {
        wgpu_only = count(|| g.submit_floor(whole[0], PAGE));
        g.wait();
    }
    let around = floor as i64 - wgpu_only as i64;
    eprintln!("empty frame: floor {floor}, wgpu's submission alone {wgpu_only}, difference {around} (bound 0)");
    assert_eq!(around, 0, "draw_regions allocated {around} around wgpu's own submission on an empty frame");
    let total = count(|| g.draw_regions(&caret, &region, Some(PAGE)));
    g.wait();
    let diff = total as i64 - floor as i64;
    eprintln!(
        "caret frame: total {total}, floor {floor} (settled after {} frames: {floors:?}), \
         difference {diff} (bound 0)",
        floors.len()
    );
    assert_eq!(diff, 0, "the caret frame allocated {diff} more than the empty frame's floor");
}

/// M83/T83.5: a warm frame with **two** custom shaders allocates nothing
/// this crate owns (`docs/plan-gpu-extension.md`, T83.5, "Allocation").
///
/// **Not against the empty frame's floor, as the plan wrote**: each
/// bracket is a render pass of its own, and wgpu allocates per pass
/// (measured: the two-shader frame's three passes read 98 against the
/// one-pass floor's 61 on the RTX 4070, `docs/results-shader.md` T83.5),
/// so that bound fails with nothing of this crate's in it. The quantity is
/// the frame minus [`Gpu::resubmit`] — wgpu's own submission of the same
/// instances, splits, slots and passes, nothing rebuilt — which is
/// `submit_floor`'s method (M51/T51.4) for a frame of several passes. The
/// slots are staged in the frame scratch and written with one
/// `write_buffer`; each pass binds a group built at `set_shaders`, with
/// dynamic offsets from a fixed array.
///
/// Injection: stage each bracket's uniforms through a fresh `Vec`, and
/// the difference reads 2 (one per bracket).
#[cfg(feature = "shader")]
#[test]
fn a_frame_with_two_shaders_allocates_nothing_this_crate_owns() {
    use fulgurite::display::ShaderId;
    use fulgurite_shell::shader::{ShaderDesc, ShaderTable};
    let Some(mut g) = Gpu::headless(W, H) else {
        assert!(std::env::var("FULGURITE_REQUIRE_GPU").as_deref() != Ok("1"), "FULGURITE_REQUIRE_GPU=1 but no adapter");
        eprintln!("gpu: no adapter — skipping");
        return;
    };
    eprintln!("gpu: {}", g.adapter());
    const SOLID: &str = "struct U { color: vec4<f32> };
        @group(0) @binding(1) var<uniform> u: U;
        @fragment fn fs_main(in: FulguriteVarying) -> @location(0) vec4<f32> { return u.color; }";
    const TABLE: [ShaderDesc; 2] = [ShaderDesc::new("a", SOLID, 16), ShaderDesc::new("b", SOLID, 16)];
    assert_eq!(g.set_shaders(ShaderTable::new(&TABLE)), 0);
    let empty = DisplayList::new();
    let mut two = DisplayList::new();
    let bytes = [0x3fu8; 16];
    for (k, x) in [(0u16, 20.0f32), (1, 160.0)] {
        two.shader([x, 20.0, x + 100.0, 100.0], ShaderId(k), &bytes);
        two.rect([x, 20.0, x + 100.0, 100.0], Color(90, 90, 90, 255));
        two.shader_end();
    }
    let whole = [[0.0, 0.0, W as f32, H as f32]];
    let mut totals = Vec::new();
    let mut alone = Vec::new();
    for _ in 0..50 {
        let t = count(|| g.draw_regions(&two, &whole, Some(PAGE)));
        g.wait();
        let a = count(|| g.resubmit());
        g.wait();
        totals.push(t);
        alone.push(a);
        let n = totals.len();
        if n >= 3 && totals[n - 1] == totals[n - 2] && alone[n - 1] == alone[n - 2] {
            break;
        }
    }
    let floor = count(|| g.draw_regions(&empty, &whole, Some(PAGE)));
    g.wait();
    let passes_before = g.shader_stats().passes;
    let total = count(|| g.draw_regions(&two, &whole, Some(PAGE)));
    g.wait();
    let passes = g.shader_stats().passes - passes_before;
    let wgpu_only = count(|| g.resubmit());
    g.wait();
    let diff = total as i64 - wgpu_only as i64;
    eprintln!(
        "two shaders: total {total}, wgpu's submission of the same frame {wgpu_only}, difference {diff} \
         (bound 0); one-pass empty floor {floor}, for the record; shader passes in the frame {passes} \
         (setup 2); settled after {} frames, totals {totals:?}",
        totals.len()
    );
    assert_eq!(passes, 2, "the shaders did not run -- the setup did not happen");
    assert_eq!(diff, 0, "the two-shader frame allocated {diff} around wgpu's own submission of it");
}
