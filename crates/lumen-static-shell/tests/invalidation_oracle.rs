//! T16.7's gate: a bounded walk must produce the same frame as the
//! whole-tree walk.
//!
//! `Everything` is the oracle. For every step of a driven session, the
//! `NodeState` array and the display list produced under
//! `Boundaries<N>` must be **identical** — not close — to the ones
//! produced under `Everything`. Any disagreement is a frame a user would
//! see wrong, and no test that renders only one of them would ever catch
//! it.
//!
//! This is the same shape `damage_oracle.rs` uses for retention, and for
//! the same reason: a cargo feature could not express it, because both
//! have to be alive in one process.
//!
//! **It is written before anything consumes the dirty set**, deliberately.
//! Today layout and paint still walk everything under both parameters, so
//! most of these assertions pass trivially — which is exactly when the
//! harness should exist, because T16.8 and T16.9 are the commits that can
//! break it and they should find it already standing.

use lumen_static::invalidate::{Boundaries, Everything, Invalidation};
use lumen_static::ops::{Button, Label, Spacer};
use lumen_static::style::{Edges, Length, Patch, Rule, Selector};
use lumen_static::{App as DeriveApp, Color, NodeState, StaticTree};
use lumen_static_shell::Host;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 6.0)]
struct Bar {
    #[widget_id("dec")]
    dec: Button<Counter, String>,
    gap: Spacer,
    #[widget_id("inc")]
    inc: Button<Counter, String>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct Card {
    #[widget_id("title")]
    title: Label,
    #[widget_id("value")]
    value: Label<String>,
    bar: Bar,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    top: Spacer,
    #[widget_id("card")]
    card: Card,
    #[widget_id("footer")]
    footer: Label,
}

#[derive(DeriveApp, Default)]
struct Counter {
    #[tree]
    ui: Ui,
    #[bind(ui.card.value, "{}")]
    count: i64,
}

/// The centre of the node carrying `wid`, from the laid-out state —
/// the same way `examples/counter` finds a button rather than guessing
/// a coordinate that silently stops landing when the layout moves.
fn centre_of<I: Invalidation>(
    host: &Host<Counter, lumen_static::Incremental, I>,
    wid: &str,
) -> (f32, f32) {
    let table = <Ui as lumen_static::Widget>::SUB_TABLE;
    let i = lumen_static::style::index_of_wid(table, wid).expect("id exists");
    let s = &host.states[i];
    (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
}

/// `sheet()` with the card's font size overridden.
///
/// `restyle_subtree` requires the new sheet to differ from the last one
/// cascaded **only** in rules matching inside the subtree, and debug
/// builds check it against a full cascade. Passing a sheet containing
/// just the card rule drops the `Ui` rule and fails that check — which
/// is the API working, and was my mistake writing this test.
fn sheet_with_card_font(px: f32) -> Vec<Rule> {
    let mut rules = sheet();
    rules.push(Rule::new(Selector::Id("card"), Patch::new().font_size(px)));
    rules
}

fn sheet() -> Vec<Rule> {
    vec![
        Rule::new(
            Selector::Kind("Ui"),
            Patch::new().background(Color(20, 24, 34, 255)).padding(Edges::all(8.0)),
        ),
        // `Fill`, not `Fixed`: a fixed 220 px card inside a 300 px
        // window left the `inc` button laid out at x=256, outside its
        // own parent, so every click in this file landed on the root
        // and the handler never ran. The oracle test still passed --
        // both arms agreed about a frame in which nothing happened,
        // which is agreement about nothing.
        Rule::new(Selector::Kind("Card"), Patch::new().width(Length::FILL)),
    ]
}

fn app() -> Counter {
    let mut c = Counter::default();
    c.ui.card.title.set("tally");
    c.ui.footer.set("footer");
    c.ui.card.bar.dec.label = "-".to_owned();
    c.ui.card.bar.inc.label = "+".to_owned();
    c.ui.card.bar.dec.on_click = |c: &mut Counter, _| c.count -= 1;
    c.ui.card.bar.inc.on_click = |c: &mut Counter, _| c.count += 1;
    c
}

/// One driven session, parameterised by the invalidation strategy.
///
/// The *same* steps in the same order, so the two runs are comparable
/// step by step rather than only at the end — a divergence that
/// self-corrects would otherwise pass.
fn drive<I: Invalidation>() -> Vec<(Vec<NodeState>, u64, usize)> {
    let mut host: Host<Counter, lumen_static::Incremental, I> = Host::new(app(), 300, 220);
    host.set_stylesheet(&sheet());

    let mut out = Vec::new();
    let record = |h: &Host<Counter, lumen_static::Incremental, I>| {
        (h.states.clone(), h.list.hash_from(0), h.list.len())
    };

    host.pump();
    out.push(record(&host));

    for step in 0..12 {
        match step % 4 {
            // A click, which runs a handler: the conservative path,
            // where the set collapses to everything.
            0 => {
                let (x, y) = centre_of(&host, "inc");
                host.click(x, y);
            }
            // A direct mutation with an explicit invalidate, which is
            // the precise path.
            1 => {
                host.app.ui.footer.set("changed");
                assert!(host.invalidate("footer"), "the id must exist");
            }
            // A scoped restyle, which invalidates its own subtree.
            2 => {
                let s = sheet_with_card_font(14.0 + step as f32);
                assert!(host.restyle_subtree(&s, "card"));
            }
            // A frame where nothing at all happened.
            _ => {}
        }
        host.pump();
        out.push(record(&host));
    }
    out
}

#[test]
fn a_bounded_walk_produces_the_same_frames_as_the_whole_tree_walk() {
    let oracle = drive::<Everything>();
    let bounded = drive::<Boundaries<16>>();

    assert_eq!(oracle.len(), bounded.len(), "the two sessions took different numbers of steps");
    for (step, (a, b)) in oracle.iter().zip(&bounded).enumerate() {
        assert_eq!(a.2, b.2, "step {step}: different command counts");
        assert_eq!(a.1, b.1, "step {step}: the display lists differ");
        assert_eq!(a.0.len(), b.0.len(), "step {step}: different node counts");
        for (i, (sa, sb)) in a.0.iter().zip(&b.0).enumerate() {
            assert_eq!(sa.size, sb.size, "step {step}, node {i}: size");
            assert_eq!(sa.pos, sb.pos, "step {step}, node {i}: position");
        }
    }
}

#[test]
fn the_set_is_spent_by_the_frame_that_reads_it() {
    // A mark applies to exactly one frame. If it survived, a single
    // invalidate would dirty every subsequent frame and the machinery
    // would buy nothing; if it were cleared too early, the frame that
    // needed it would skip.
    let mut host: Host<Counter, lumen_static::Incremental, Boundaries<16>> =
        Host::new(app(), 300, 220);
    host.set_stylesheet(&sheet());
    host.pump();

    assert!(host.invalidate("footer"));
    host.pump();
    assert!(
        host.changed().is_clean(0..<Ui as lumen_static::Widget>::NODE_COUNT),
        "the mark should have been spent by the frame that read it"
    );
}

#[test]
fn a_handler_collapses_the_set_and_a_still_frame_does_not() {
    let mut host: Host<Counter, lumen_static::Incremental, Boundaries<16>> =
        Host::new(app(), 300, 220);
    host.set_stylesheet(&sheet());
    host.pump();

    // A still frame leaves nothing marked.
    host.pump();
    assert!(host.changed().can_skip(), "a frame where nothing happened must not collapse");

    // A click runs a handler, which may have touched anything.
    let (x, y) = centre_of(&host, "inc");
    host.click(x, y);
    let handled = host.pump();
    assert!(handled, "the click must actually land, or this test proves nothing");
    // The mark is spent by that same pump, so observe it the only way
    // that is observable from outside: the *next* frame is clean again.
    host.pump();
    assert!(host.changed().can_skip());
}

#[test]
fn a_resize_invalidates_everything() {
    let mut host: Host<Counter, lumen_static::Incremental, Boundaries<16>> =
        Host::new(app(), 300, 220);
    host.set_stylesheet(&sheet());
    host.pump();
    host.pump();
    assert!(host.changed().can_skip());

    host.resize(400, 300);
    // Before the pump reads it, the set must already say "everything":
    // every laid-out position changed.
    assert!(!host.changed().can_skip(), "a resize cannot leave anything clean");
}

#[test]
fn an_unknown_widget_id_is_reported_not_swallowed() {
    let mut host: Host<Counter, lumen_static::Incremental, Boundaries<16>> =
        Host::new(app(), 300, 220);
    assert!(!host.invalidate("no-such-id"));
    assert!(host.invalidate("card"));
}
