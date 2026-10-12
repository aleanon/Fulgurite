//! What the `Painter` sink costs — M10a's gate.
//!
//! `Node::paint` used to write into a concrete `&mut DisplayList`
//! through inherent, inlinable methods. It now writes through
//! `&mut dyn Painter`. The recursion into children was *already* an
//! indirect call for ordinary trees (`call_child_ref` emits
//! `&dyn Node`), so what is new is the indirection on the **leaf builder
//! calls** — and in a `#[fuse]`d tree, where paint was 100% static, it
//! is the only indirection there is. `#[fuse]` exists to devirtualize
//! (`docs/results-traversal.md`), so a change that reintroduces a call
//! it cannot remove has to be measured rather than assumed.
//!
//! Run: `cargo run --release -p benches --bin paint_sink`

use std::hint::black_box;

use benches::bench;
use fulgurite::display::{Color, DisplayList, Painter};

const R: [f32; 4] = [1.0, 2.0, 30.0, 40.0];
const C: Color = Color(10, 20, 30, 255);

/// One node's worth of painting, against a concrete list — what the
/// call looked like before M10a.
#[inline(never)]
fn paint_concrete(out: &mut DisplayList, n: usize) {
    for _ in 0..n {
        out.rounded_rect(R, C, 4.0);
        out.text_weighted(R, "label", 16.0, 400.0, C);
    }
}

/// The same, through the trait object — what it looks like now.
#[inline(never)]
fn paint_sink(out: &mut dyn Painter, n: usize) {
    for _ in 0..n {
        out.rounded_rect(R, C, 4.0);
        out.text_weighted(R, "label", 16.0, 400.0, C);
    }
}

fn main() {
    black_box(());
    println!("paint_sink: concrete `&mut DisplayList` vs `&mut dyn Painter`\n");

    // Command counts spanning the real range: the tally emits ~34, a
    // 900-node tree ~1-2k (docs/results-frame-cost.md).
    for nodes in [17usize, 500, 1000] {
        let cmds = nodes * 2;
        let mut a = DisplayList::new();
        bench(&format!("{cmds} cmds  concrete"), 20_000, || {
            a.clear();
            paint_concrete(&mut a, nodes);
            a.len() as u64
        });
        let mut b = DisplayList::new();
        bench(&format!("{cmds} cmds  dyn Painter"), 20_000, || {
            b.clear();
            paint_sink(&mut b, nodes);
            b.len() as u64
        });
        println!();
    }
    println!(
        "The difference is the per-command indirect call. Compare it against\n\
         a frame's render cost in docs/results-frame-cost.md before calling it\n\
         significant: render p95 is ~0.089 ms."
    );
}
