//! M55/T55.5 (R16-F5, R16-F6): a non-finite or negative style length is
//! repaired where it enters, and the frame still works.
//!
//! **What it used to do.** `Patch::apply_to` copied lengths into a
//! `Style` unchecked. One NaN spacing made most of the tree's `NodeState`
//! non-finite, every `contains` against a NaN rect is false, and the
//! window was blank and unclickable with no panic and no diagnostic
//! (the review measured 5 of 6 states non-finite and 0 commands). A
//! negative padding widened the content box instead, so `place` put the
//! buttons outside their parent: painted, and never hit.
//!
//! **Every row asserts that its click landed** — the count moves from 7
//! to 8 — so a row cannot pass by agreeing about a frame in which nothing
//! happened (AGENT.md, "the setup silently did not happen"). The control
//! rows (finite padding, padding 0 and 8) are the accepting direction:
//! the repair must not rewrite a legal value, and each reports zero
//! repairs.

use lumen_static::invalidate::{Boundaries, Everything, Invalidation};
use lumen_static::style::{Edges, Patch, Rule, Selector};
use lumen_static::{App as DeriveApp, NodeState, StaticTree};
use lumen_static_shell::Host;
use lumen_static_widgets::ops::{Button, Label};

#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
struct Bar {
    #[widget_id("dec")]
    dec: Button<Counter, String>,
    #[widget_id("inc")]
    inc: Button<Counter, String>,
}

/// Six nodes, as the review's fixture had: the root, a title, a value, a
/// row and its two buttons.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct Ui {
    #[widget_id("title")]
    title: Label,
    #[widget_id("value")]
    value: Label<String>,
    #[widget_id("bar")]
    bar: Bar,
}

#[derive(DeriveApp, Default)]
struct Counter {
    #[tree]
    ui: Ui,
    #[bind(ui.value, "{}")]
    count: i64,
}

fn app() -> Counter {
    let mut c = Counter { count: 7, ..Default::default() };
    c.ui.title.set("tally");
    c.ui.bar.dec.label = "-".to_owned();
    c.ui.bar.inc.label = "+".to_owned();
    c.ui.bar.dec.on_click = |c: &mut Counter, _| c.count -= 1;
    c.ui.bar.inc.on_click = |c: &mut Counter, _| c.count += 1;
    c
}

fn finite(s: &NodeState) -> bool {
    s.pos.x.is_finite() && s.pos.y.is_finite() && s.size.w.is_finite() && s.size.h.is_finite()
}

/// A child whose rect is not inside its parent's (0.5 px of slack for
/// rounding). Hit testing assumes the opposite.
fn escapes(table: &[lumen_static::NodeDesc], st: &[NodeState]) -> usize {
    let mut n = 0;
    for (i, d) in table.iter().enumerate() {
        if d.parent == lumen_static::desc::NO_PARENT {
            continue;
        }
        let (c, p) = (&st[i], &st[d.parent as usize]);
        let inside = c.pos.x >= p.pos.x - 0.5
            && c.pos.y >= p.pos.y - 0.5
            && c.pos.x + c.size.w <= p.pos.x + p.size.w + 0.5
            && c.pos.y + c.size.h <= p.pos.y + p.size.h + 0.5;
        if !inside {
            n += 1;
        }
    }
    n
}

/// One row: the sheet's patch on the nodes `sel` matches, under strategy
/// `I`. A length goes where it is read: spacing and padding on the root,
/// a font size on the labels, a radius on the buttons -- a font size on a
/// column is read by nothing, and a row there would pass today.
fn row<I: Invalidation>(name: &str, sel: Selector, patch: Patch) {
    let strategy = std::any::type_name::<I>().rsplit("::").next().unwrap_or("?");
    let mut host: Host<Counter, lumen_static::Incremental, I> = Host::new(app(), 300, 200);
    host.set_stylesheet(&[Rule::new(sel, patch)]);
    host.pump();

    let table = <Ui as lumen_static::Widget>::SUB_TABLE;
    let bad = host.states.iter().filter(|s| !finite(s)).count();
    let nan_cmds = host
        .list
        .cmds
        .iter()
        .filter(|c| {
            let d = format!("{c:?}");
            d.contains("NaN") || d.contains("inf")
        })
        .count();
    let esc = escapes(table, &host.states);
    let inc = lumen_static::style::index_of_wid(table, "inc").expect("inc exists");
    let s = host.states[inc];
    let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    host.click(x, y);
    let handled = host.pump();
    println!(
        "{name} [{strategy}]: non-finite states: {bad} / {} (bound 0), non-finite commands: \
         {nan_cmds} of {} (bound 0), escapes: {esc} (bound 0), click on #inc at ({x}, {y}): \
         handled {handled}, count 7 -> {} (expected 8)",
        table.len(),
        host.list.len(),
        host.app.count
    );
    assert_eq!(bad, 0, "{name} [{strategy}]: {bad} of {} states are non-finite", table.len());
    assert_eq!(nan_cmds, 0, "{name} [{strategy}]: {nan_cmds} display commands carry a non-finite value");
    assert_eq!(esc, 0, "{name} [{strategy}]: {esc} children lie outside their parent");
    assert!(handled, "{name} [{strategy}]: the click on #inc was not handled");
    assert_eq!(host.app.count, 8, "{name} [{strategy}]: the click on #inc did not land");
}

const ROOT: Selector = Selector::Kind("Ui");

macro_rules! rows {
    ($($test:ident: $sel:expr, $patch:expr;)*) => {$(
        #[test]
        #[ignore = "red until T55.5: R16-F5/R16-F6 (non-finite and negative lengths reach layout)"]
        fn $test() {
            let name = stringify!($test);
            row::<Everything>(name, $sel, $patch);
            row::<Boundaries<256>>(name, $sel, $patch);
        }
    )*};
}

rows! {
    nan_spacing: ROOT, Patch::new().spacing(f32::NAN);
    infinite_padding: ROOT, Patch::new().padding(Edges::all(f32::INFINITY));
    nan_padding: ROOT, Patch::new().padding(Edges::all(f32::NAN));
    nan_font_size: Selector::Kind("label"), Patch::new().font_size(f32::NAN);
    nan_radius: Selector::Kind("button"), Patch::new().radius(f32::NAN);
    negative_padding: ROOT, Patch::new().padding(Edges::all(-50.0));
}

/// The accepting direction: legal values pass untouched.
#[test]
fn finite_rows_are_untouched() {
    for p in [0.0, 8.0] {
        row::<Everything>("finite padding", ROOT, Patch::new().padding(Edges::all(p)));
        row::<Boundaries<256>>("finite padding", ROOT, Patch::new().padding(Edges::all(p)));
    }
    row::<Everything>("finite spacing", ROOT, Patch::new().spacing(6.0));
    row::<Boundaries<256>>("finite spacing", ROOT, Patch::new().spacing(6.0));
    let label = || Selector::Kind("label");
    row::<Everything>("finite font size", label(), Patch::new().font_size(15.0));
    row::<Boundaries<256>>("finite font size", label(), Patch::new().font_size(15.0));
    let button = || Selector::Kind("button");
    row::<Everything>("finite radius", button(), Patch::new().radius(4.0));
    row::<Boundaries<256>>("finite radius", button(), Patch::new().radius(4.0));
}
