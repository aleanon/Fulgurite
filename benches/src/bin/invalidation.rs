//! What the relayout boundary buys — `docs/plan-invalidation.md`
//! T16.7–T16.10.
//!
//! Three frames of the same tree, differing only in what the frame was
//! told changed:
//!
//! - **`Everything`** — the whole-tree walk, and the oracle. What this
//!   crate did before T16.9.
//! - **`Boundaries`, one subtree marked** — the case an application
//!   hits when one thing changed and it said so.
//! - **`Boundaries`, nothing marked** — a still frame, which is what an
//!   idle window and an animation between ticks produce.
//!
//! Paint is the larger stage of a small-update frame (121 µs against 55
//! at 10 000 rows, `docs/results-competitive.md` §5b) and is not skipped
//! yet — T16.8 — so quoting a whole frame here would credit that
//! milestone with work it has not done.
//!
//! **Two more rows since M26/T26.0**, because M26 changes both and
//! neither had ever been priced: `place`, which takes no relayout
//! boundary at all (it is handed no context, so it cannot see what
//! changed), and `dispatch`, which publishes hover, press and focus by
//! walking every node in the state array on every pointer event.

use std::time::Instant;

use benches::Ten10;
use lumen_static::cx::{BlockMetrics, LayoutCx};
use lumen_static::invalidate::{Boundaries, Changed, Invalidation};
use lumen_static::widget::Node;
use lumen_static::{pass, Constraints, NodeState, Style, Widget};

const VIEW: Constraints =
    Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 600.0 };

/// Min of `n`, in microseconds. Min rather than mean: the question is
/// what the pass costs, and every sample above the minimum is something
/// else the machine was doing.
fn best(n: u32, mut f: impl FnMut()) -> f64 {
    let mut lo = f64::INFINITY;
    for _ in 0..n {
        let t = Instant::now();
        f();
        lo = lo.min(t.elapsed().as_secs_f64() * 1e6);
    }
    lo
}

fn main() {
    let mut app = Ten10::default();
    let styles: Vec<Style> = pass::default_styles(&app);
    let mut st = vec![NodeState::default(); Ten10::NODE_COUNT];
    let table = <Ten10 as Widget>::SUB_TABLE;

    // Warm: the first pass fills every `Measured` cache and every
    // `NodeState`, so the runs below all start from a laid-out tree —
    // which is the state a frame actually begins in.
    for _ in 0..50 {
        let mut m = BlockMetrics;
        let mut cx = LayoutCx::new(&mut m);
        app.layout(&mut cx, VIEW, &styles, &mut st, 0);
    }

    println!("invalidation: layout of {} nodes, min of 2000\n", Ten10::NODE_COUNT);

    // `LayoutCx::new` supplies `Changed::ALL`, which is exactly what
    // `Everything::view()` returns — so this row is the oracle's path,
    // measured through the constructor every existing caller uses.
    let full = best(2000, || {
        let mut m = BlockMetrics;
        let mut cx = LayoutCx::new(&mut m);
        app.layout(&mut cx, VIEW, &styles, &mut st, 0);
    });
    println!("{:<34}{full:9.2} us", "Everything (the oracle)");

    // One subtree marked. The fixture is ten columns of ten, so a
    // marked column is a tenth of the tree — the same subtree
    // `benches --bin cascade` scopes its restyle to.
    let root = table
        .iter()
        .position(|d| d.parent == 0 && d.child_count > 0)
        .expect("a container child of the root");
    let range = lumen_static::style::subtree_range(table, root);
    let marked = best(2000, || {
        let mut b: Boundaries<16> = Boundaries::default();
        b.clear();
        b.mark(range.clone());
        let mut m = BlockMetrics;
        let mut cx = LayoutCx::with_changed(&mut m, b.view());
        app.layout(&mut cx, VIEW, &styles, &mut st, 0);
    });
    println!(
        "{:<34}{marked:9.2} us   {:.1}x   (nodes {}..{})",
        "Boundaries, one subtree marked",
        full / marked,
        range.start,
        range.end
    );

    let still = best(2000, || {
        let mut m = BlockMetrics;
        let mut cx = LayoutCx::with_changed(&mut m, Changed::NONE);
        app.layout(&mut cx, VIEW, &styles, &mut st, 0);
    });
    println!(
        "{:<34}{still:9.2} us   {:.1}x",
        "Boundaries, still frame",
        full / still
    );

    println!(
        "\n   the skip needs BOTH a clean subtree and unchanged constraints;\n   \
         a resize defeats it by design, which is what `Everything` measures."
    );

    // ---- place and dispatch — M26/T26.0 --------------------------------
    //
    // Measured after the rows above so the tree is laid out, which is the
    // state both passes actually run in.

    // `place` has no boundary to take: `Node::place` receives no
    // `LayoutCx`, so `Changed` is unreachable from it and every node is
    // repositioned whatever changed. That is the whole row — there is no
    // second variant to compare it against until M26/T26.17 #1 adds one.
    let placed = best(2000, || {
        app.place(lumen_static::Point::default(), &styles, &mut st, 0);
    });
    println!("\n{:<34}{placed:9.2} us   (whole tree; place takes no `Changed`)", "place");

    // One `PointerMove`, through the real dispatch path. The entry point
    // is named on purpose: `pass::dispatch` alone is what the per-node
    // publishes cost, while `Host::pump` after an event is what the
    // `Everything`/`Boundaries` split above applies to — measuring the
    // second and calling it the first is how a row prices the wrong
    // quantity (M26 §0 rule 2).
    let mut dapp = benches::Ten10App::default();
    let dstyles: Vec<Style> = pass::default_styles(&dapp.ui);
    let mut dst = vec![NodeState::default(); Ten10::NODE_COUNT];
    {
        let mut m = BlockMetrics;
        let mut cx = LayoutCx::new(&mut m);
        dapp.ui.layout(&mut cx, VIEW, &dstyles, &mut dst, 0);
        dapp.ui.place(lumen_static::Point::default(), &dstyles, &mut dst, 0);
    }
    let mut input = lumen_static::event::InputState::default();
    let ev = lumen_static::Event::PointerMove { x: 12.0, y: 12.0 };
    let dispatched = best(2000, || {
        let _ = pass::dispatch(&mut dapp, &dstyles, &mut dst, &mut input, &ev);
    });
    println!(
        "{:<34}{dispatched:9.2} us   {:.2} ns/node over {} nodes",
        "dispatch, one PointerMove",
        dispatched * 1000.0 / Ten10::NODE_COUNT as f64,
        Ten10::NODE_COUNT
    );
}
