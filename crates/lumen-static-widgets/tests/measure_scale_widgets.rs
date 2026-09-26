//! **The editable widgets' shaped memo keys on the display scale** —
//! M45/T45.0 item 4, for R2-F6 (`docs/plan-text-correctness.md` §1.2).
//!
//! M26/T26.17 #12 put the scale into `MeasureKey`, which `Label` and the
//! rest use. `TextInput` and `TextArea` key their shaped clusters on a
//! hand-written tuple — text length, size, weight and a width — that has
//! no scale, so the monitor drag that key fixed for `Label` survives in
//! the two editable widgets: the logical limits unchanged, the scale
//! doubled, the memo served, and the caret placed from the old scale's
//! shaping.
//!
//! The measurer is `measure_scale.rs`'s `Hinted` — non-linear in the
//! scale, as a hinted face is — **with clusters**, because a `TextArea`'s
//! box does not move with its text (it is as wide as its limits) and only
//! its caret can show the stale shaping.

use lumen_static::cx::{Cluster, GlyphSink};
use lumen_static::{Constraints, LayoutCx, Node, NodeState, Size, Style, TextMeasure};
use lumen_static_widgets::{TextArea, TextInput};

#[path = "../../lumen-static/tests/support/hinted.rs"]
mod hinted;
use hinted::Hinted;

/// `Hinted`, reporting a cluster per character at `n * size + scale`.
struct HintedClusters(Hinted);

impl TextMeasure for HintedClusters {
    fn measure(&mut self, t: &str, size: f32, max_w: f32) -> Size {
        self.0.measure(t, size, max_w)
    }
    fn measure_weighted(&mut self, t: &str, size: f32, weight: f32, max_w: f32) -> Size {
        self.0.measure_weighted(t, size, weight, max_w)
    }
    fn shape(&mut self, t: &str, size: f32, weight: f32, max_w: f32, sink: &mut dyn GlyphSink) -> Size {
        let k = self.0.scale;
        for (n, (b, _)) in t.char_indices().enumerate() {
            sink.cluster(Cluster { byte: b as u32, x: n as f32 * size + k, line: 0 });
        }
        sink.cluster(Cluster { byte: t.len() as u32, x: t.chars().count() as f32 * size + k, line: 0 });
        self.0.measure_weighted(t, size, weight, max_w)
    }
}

const LIMITS: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 400.0 };

/// Lay `w` out at `scale` and return its width and the x of its caret
/// as paint draws it (the caret is the narrow rect paint emits when the
/// node is focused).
fn at<N: Node>(w: &mut N, scale: f32) -> (f32, f32) {
    let mut m = HintedClusters(Hinted::at(scale));
    let mut cx = LayoutCx::new(&mut m).with_scale(scale);
    let sty = [Style::default()];
    let mut st = [NodeState::default()];
    let size = w.layout(&mut cx, LIMITS, &sty, &mut st, 0);
    st[0].focused = true;
    let mut list = lumen_static::DisplayList::new();
    w.paint(&st, 0, &mut list);
    let caret = list
        .cmds
        .iter()
        .rev()
        .find_map(|c| match c {
            lumen_static::Cmd::Rect { rect, .. } if (rect[2] - rect[0] - 1.0).abs() < 1e-3 => Some(rect[0]),
            _ => None,
        })
        .unwrap_or(f32::NAN);
    (size.w, caret)
}

fn drag<N: Node>(name: &str, mut fresh: impl FnMut() -> N) -> bool {
    let mut w = fresh();
    let one = at(&mut w, 1.0);
    let served = at(&mut w, 2.0);
    let mut f = fresh();
    let want = at(&mut f, 2.0);
    let ok = served == want;
    println!("{name}: 1x {one:?}; 2x after a drag {served:?}; fresh at 2x {want:?} (width, caret x) | bound: equal -> {ok}");
    ok
}

#[test]
fn a_scale_change_reshapes_the_editable_widgets() {
    let input = drag("TextInput", || TextInput::<String>::new("POSITIVE"));
    let area = drag("TextArea", || TextArea::<String>::new("POSITIVE"));
    assert!(input && area, "served != fresh after a scale change: TextInput {input}, TextArea {area}");
}

/// **The accept direction**: an unchanged scale still hits, one call.
#[test]
fn an_unchanged_scale_still_hits_the_shaped_memo() {
    let mut w = TextInput::<String>::new("POSITIVE");
    let mut m = HintedClusters(Hinted::at(2.0));
    for _ in 0..3 {
        let mut cx = LayoutCx::new(&mut m).with_scale(2.0);
        let (sty, mut st) = ([Style::default()], [NodeState::default()]);
        w.layout(&mut cx, LIMITS, &sty, &mut st, 0);
    }
    println!("three layouts at 2x: {} measurer calls (bound 1)", m.0.calls.get());
    assert_eq!(m.0.calls.get(), 1);
}
