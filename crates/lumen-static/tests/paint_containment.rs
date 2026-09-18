//! **Property:** after paint, nothing reaches the screen outside the
//! root's box.
//!
//! Stated precisely: for every drawing command, its rect *intersected
//! with every enclosing clip* lies inside the root viewport. Commands
//! may exceed their clip — a repeater's window overruns its viewport by
//! construction — and the clip is what stops them; a construct that
//! forgets one has an effective rect equal to its own, outside the root,
//! and fails here.
//!
//! This is the scoped containment invariant from
//! `docs/text-and-scrolling-decisions.md` A1, checked where escaping
//! constructs are actually visible — the display list — rather than over
//! `SUB_TABLE`.
//!
//! That distinction is the whole point. The guard that existed before
//! (`lumen-static-agent/tests/lint.rs`) walks the table, and every
//! construct that *can* violate containment keeps its children off-table:
//! `Repeat`, `List`, `Dyn`, `Either`, `Assembled`. The one mechanism that
//! breaks the invariant was invisible to the test written to catch it,
//! which is how a repeater painted 44px outside its viewport for three
//! milestones (`docs/plan-review-fixes.md` F4.8).
//!
//! The corpus is a fixed array rather than a generator: deterministic,
//! and every case is nameable in a failure message.

use lumen_static::hatch::{Dyn, Either, List, Side};
use lumen_static::ops::{Fixed, Label, Spacer};
use lumen_static::repeat::{noop_bind, Repeat};
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::widget::Node;
use lumen_static::{pass, Cmd, Constraints, DisplayList, NodeState, StaticTree, Widget};

/// Walk the list maintaining a clip stack; return the first command
/// whose *effective* rect — after every enclosing clip — leaves `root`.
fn first_escape(list: &DisplayList, root: [f32; 4]) -> Option<(usize, [f32; 4])> {
    const EPS: f32 = 0.001;
    let mut stack: Vec<[f32; 4]> = Vec::new();
    for (n, c) in list.cmds.iter().enumerate() {
        match c {
            // A line's containment is its bounding box widened by half
            // the stroke, the same rect retention treats as its ink.
            Cmd::Line { a, b, width, .. } => {
                let w = width / 2.0;
                let r = [a.x.min(b.x) - w, a.y.min(b.y) - w, a.x.max(b.x) + w, a.y.max(b.y) + w];
                if let Some(c) = stack.last() {
                    assert!(
                        r[0] >= c[0] - 0.51 && r[1] >= c[1] - 0.51
                            && r[2] <= c[2] + 0.51 && r[3] <= c[3] + 0.51,
                        "command {n} (line) escapes its clip {c:?}: {r:?}"
                    );
                }
            }
            Cmd::PushClip { rect } => stack.push(*rect),
            Cmd::PopClip => {
                stack.pop();
            }
            Cmd::Rect { rect, .. }
            | Cmd::RoundedRect { rect, .. }
            // A border is drawn *inside* its rect, so its containment is
            // its rect -- the same reason retention inks only the rect.
            | Cmd::StrokedRect { rect, .. }
            | Cmd::Shadow { rect, .. }
            | Cmd::Text { rect, .. }
            | Cmd::Image { rect, .. }
            | Cmd::Glass { rect, .. }
            | Cmd::Gradient { rect, .. } => {
                let mut r = *rect;
                // A shadow's ink reaches `spread` past its rect.
                if let Cmd::Shadow { spread, .. } = c {
                    r = [r[0] - spread, r[1] - spread, r[2] + spread, r[3] + spread];
                }
                for clip in &stack {
                    r = [
                        r[0].max(clip[0]),
                        r[1].max(clip[1]),
                        r[2].min(clip[2]),
                        r[3].min(clip[3]),
                    ];
                }
                // Clipped away entirely: draws nothing, so nothing to
                // check.
                if r[0] >= r[2] || r[1] >= r[3] {
                    continue;
                }
                let inside = r[0] >= root[0] - EPS
                    && r[1] >= root[1] - EPS
                    && r[2] <= root[2] + EPS
                    && r[3] <= root[3] + EPS;
                if !inside {
                    return Some((n, r));
                }
            }
        }
    }
    None
}

/// The clip stack must be balanced: an unmatched push leaks the clip
/// onto every later sibling, an unmatched pop drops one early.
fn clips_balanced(list: &DisplayList) -> bool {
    let mut depth = 0i32;
    for c in &list.cmds {
        match c {
            Cmd::PushClip { .. } => depth += 1,
            Cmd::PopClip => depth -= 1,
            _ => {}
        }
        if depth < 0 {
            return false;
        }
    }
    depth == 0
}

// ---- the shapes -------------------------------------------------------

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Row {
    ink: Fixed,
    name: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Pane {
    a: Label,
    b: Fixed,
    c: Spacer,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct WithRepeat {
    #[widget_id("subject")]
    inner: Repeat<Row>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct WithList {
    #[widget_id("subject")]
    inner: List<Row>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct WithDyn {
    #[widget_id("subject")]
    inner: Dyn<()>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct WithEither {
    #[widget_id("subject")]
    inner: Either<Pane, Row>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Plain {
    #[widget_id("subject")]
    inner: Pane,
}

/// Lay a shape out under `height`, optionally forcing the subject to a
/// fixed size so `resolve_self` shrinks it below its content.
fn run<T: Widget + Node + Default>(
    setup: impl FnOnce(&mut T),
    height: f32,
    squeeze: Option<f32>,
    label: &str,
) {
    let mut ui = T::default();
    setup(&mut ui);

    let mut styles = pass::default_styles(&ui);
    if let Some(h) = squeeze {
        let sheet = [Rule::new(Selector::Id("subject"), Patch::new().height(Length::Fixed(h)))];
        lumen_static::style::cascade(
            T::SUB_TABLE,
            T::SEEDS,
            &sheet,
            &mut styles,
        );
    }

    let mut st = vec![NodeState::default(); T::NODE_COUNT];
    let view = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: height };
    pass::layout(&mut ui, &styles, &mut st, view);
    let mut list = DisplayList::new();
    pass::paint(&ui, &st, &mut list);

    assert!(clips_balanced(&list), "{label}: clip stack is unbalanced");
    let root = lumen_static::ops::rect_of(&st[0]);
    if let Some((n, r)) = first_escape(&list, root) {
        panic!("{label}: command {n} draws at {r:?}, outside the root {root:?}");
    }
}

fn rows(n: usize) -> Vec<Row> {
    (0..n)
        .map(|i| {
            let mut r = Row { ink: Fixed { w: 30.0, h: 20.0 }, ..Default::default() };
            r.name.set(if i % 2 == 0 { "even" } else { "odd" });
            r
        })
        .collect()
}

#[test]
fn nothing_escapes_its_clip_across_the_corpus() {
    // Viewports: taller than content, shorter, and an exact multiple of
    // the item extent — the third because "rarely an exact multiple" is
    // the reason a repeater materializes an extra row at all.
    for &h in &[400.0f32, 100.0, 96.0] {
        // Repeat, at four scroll positions including two off an item
        // boundary and one at the end.
        for &scroll in &[0.0f32, 12.0, 0.5, 24.0 * 40.0] {
            run::<WithRepeat>(
                |ui| {
                    ui.inner = Repeat::new(50, 24.0, noop_bind);
                    ui.inner.scroll = scroll;
                },
                h,
                None,
                &format!("Repeat h={h} scroll={scroll}"),
            );
        }

        // List, free and squeezed below its content.
        for squeeze in [None, Some(30.0)] {
            run::<WithList>(
                |ui| ui.inner = List::new(rows(10)),
                h,
                squeeze,
                &format!("List h={h} squeeze={squeeze:?}"),
            );
        }

        // Dyn island, free and squeezed.
        for squeeze in [None, Some(15.0)] {
            run::<WithDyn>(
                |ui| ui.inner = Dyn::new(Pane::default()),
                h,
                squeeze,
                &format!("Dyn h={h} squeeze={squeeze:?}"),
            );
        }

        // Either, both branches.
        for side in [Side::A, Side::B] {
            run::<WithEither>(
                |ui| {
                    ui.inner = Either::new(Pane::default(), Row::default());
                    ui.inner.show(side);
                },
                h,
                None,
                &format!("Either h={h} side={side:?}"),
            );
        }

        // And an ordinary derived container, which must also hold.
        run::<Plain>(|_| {}, h, None, &format!("Plain h={h}"));
    }
}

#[test]
fn a_nested_clip_scope_composes() {
    // A List of Rows inside a squeezed subject: two clip scopes, the
    // inner one inside the outer.
    run::<WithList>(
        |ui| ui.inner = List::new(rows(20)),
        60.0,
        Some(40.0),
        "List squeezed inside a short viewport",
    );
}
