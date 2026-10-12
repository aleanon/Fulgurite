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
//! **The session asserts that it happened** (M41/T41.0, R18-F8): every
//! click must be handled and move the count, and each pointer step must
//! land on the node it meant. A session whose clicks miss is two
//! strategies agreeing about frames in which nothing occurred — the
//! failure T16.7 shipped with and the review re-created by injection.
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

use fulgurite::invalidate::{Boundaries, Everything, Invalidation};
use fulgurite_widgets::ops::{Button, Label, Spacer};
use fulgurite::style::{Edges, Length, Patch, Rule, Selector};
use fulgurite::{App as DeriveApp, Color, NodeState, StaticTree};
use fulgurite_shell::Host;

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

/// One-of-two, so the session drives a variant switch — M26/T26.1.
///
/// Both variants are derived containers on purpose: the relayout
/// boundary that made the enum defect invisible lives at the top of a
/// *container's* `layout`, so an enum of leaves could not exhibit it and
/// this fixture would have gone on agreeing with itself.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct PaneA {
    #[widget_id("pane_a")]
    text: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct PaneB {
    #[widget_id("pane_b")]
    text: Label,
}

#[derive(StaticTree)]
enum Pane {
    A(PaneA),
    B(PaneB),
}

impl Default for Pane {
    fn default() -> Self {
        Pane::A(PaneA::default())
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    top: Spacer,
    #[widget_id("card")]
    card: Card,
    /// M26/T26.1: until this fixture held an enum, the oracle's
    /// "byte-identical frames" claim could not fail on a hidden-subtree
    /// defect, because no tree it drove had a hidden subtree at all.
    #[widget_id("pane")]
    pane: Pane,
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
    host: &Host<Counter, fulgurite::Incremental, I>,
    wid: &str,
) -> (f32, f32) {
    let table = <Ui as fulgurite::Widget>::SUB_TABLE;
    let i = fulgurite::style::index_of_wid(table, wid).expect("id exists");
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
    let mut host: Host<Counter, fulgurite::Incremental, I> = Host::new(app(), 300, 220);
    host.set_stylesheet(&sheet());

    let mut out = Vec::new();
    let record = |h: &Host<Counter, fulgurite::Incremental, I>| {
        (h.states.clone(), h.list.hash_from(0), h.list.len())
    };

    host.pump();
    out.push(record(&host));

    // **Every click must land** — M41/T41.0, for R18-F8. This session's
    // clicks were never checked: `Length::Fixed(220.0)` on the card (the
    // review's I4a, T16.7's original defect) put `inc` outside its parent
    // and both headline tests stayed green, comparing two sessions in
    // which no click did anything.
    let mut landed = 0;
    for step in 0..12 {
        let mut clicked = None;
        match step % 4 {
            // A click, which runs a handler: the conservative path,
            // where the set collapses to everything.
            0 => {
                let (x, y) = centre_of(&host, "inc");
                host.click(x, y);
                clicked = Some(host.app.count);
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
            // A variant switch, marked the way an application would mark
            // it — M26/T26.1. The enum is *not* what is marked on the
            // other steps, so the frames in between are exactly the ones
            // where a hidden-subtree defect shows.
            3 if step == 3 => {
                host.app.ui.pane = Pane::B(PaneB::default());
                assert!(host.invalidate("pane"), "the pane id must exist");
            }
            // A frame where nothing at all happened.
            _ => {}
        }
        let handled = host.pump();
        if let Some(before) = clicked {
            assert!(handled, "click {} did not land: pump reported nothing handled", step / 4);
            assert_eq!(host.app.count, before + 1, "click {} did not land: the count did not move", step / 4);
            landed += 1;
        }
        out.push(record(&host));
    }
    println!(
        "{}: clicks landed {landed}/3, final count {}",
        std::any::type_name::<I>().rsplit("::").next().unwrap_or("?"),
        host.app.count
    );
    assert_eq!((landed, host.app.count), (3, 3), "the session's clicks did not all land");

    // **Pointer state no handler reports** — M25/T25.0.
    //
    // Until then this session never hovered, so the oracle compared two
    // strategies that agreed about every frame it drove while `Boundaries`
    // painted a stale hover and press shade on every real one. Each step
    // asserts its setup landed: the bit is on the node meant, and the
    // move and the press were *unhandled* — a handled event reaches
    // `mark_all` and would make the step prove nothing.
    let table = <Ui as fulgurite::Widget>::SUB_TABLE;
    let dec = fulgurite::style::index_of_wid(table, "dec").expect("id exists");
    let (x, y) = centre_of(&host, "dec");

    host.post(fulgurite::Event::PointerMove { x, y });
    assert!(!host.pump(), "a move onto a button must be unrouted, or this step is vacuous");
    assert!(host.states[dec].hovered, "the move did not land on `dec`");
    out.push(record(&host));

    host.post(fulgurite::Event::down(x, y));
    assert!(!host.pump(), "a press is activated on release, so it must be unhandled here");
    assert!(host.states[dec].pressed, "the press did not latch on `dec`");
    out.push(record(&host));

    host.post(fulgurite::Event::up(x, y));
    host.pump();
    assert!(!host.states[dec].pressed, "the release did not clear the press");
    out.push(record(&host));

    host.post(fulgurite::Event::PointerMove { x: 1.0, y: 1.0 });
    assert!(!host.pump(), "a move off the button must be unrouted");
    assert!(!host.states[dec].hovered, "the move did not leave `dec`");
    out.push(record(&host));

    out
}

#[test]
fn a_bounded_walk_produces_the_same_frames_as_the_whole_tree_walk() {
    same(&drive::<Everything>(), &drive::<Boundaries<16>>());
}

fn same(oracle: &[(Vec<fulgurite::NodeState>, u64, usize)], bounded: &[(Vec<fulgurite::NodeState>, u64, usize)]) {
    assert_eq!(oracle.len(), bounded.len(), "the two sessions took different numbers of steps");
    for (step, (a, b)) in oracle.iter().zip(bounded).enumerate() {
        assert_eq!(a.2, b.2, "step {step}: different command counts");
        assert_eq!(a.1, b.1, "step {step}: the display lists differ");
        assert_eq!(a.0.len(), b.0.len(), "step {step}: different node counts");
        for (i, (sa, sb)) in a.0.iter().zip(&b.0).enumerate() {
            assert_eq!(sa.size, sb.size, "step {step}, node {i}: size");
            assert_eq!(sa.pos, sb.pos, "step {step}, node {i}: position");
        }
    }
}

/// **The shipped `N`** — M34/T34.0. The window ships `Boundaries<256>`
/// (`fulgurite_winit::Inval`) and M26/T26.5 was a wrap at exactly 256
/// marks, so the type a user gets goes through the oracle too, not only
/// the `<16>` every other test here uses.
#[test]
fn the_shipped_boundaries_256_produces_the_same_frames_too() {
    let (oracle, bounded) = (drive::<Everything>(), drive::<Boundaries<256>>());
    same(&oracle, &bounded);
    println!("Boundaries<256>: {} steps, identical to Everything", oracle.len());
}

#[test]
fn the_set_is_spent_by_the_frame_that_reads_it() {
    // A mark applies to exactly one frame. If it survived, a single
    // invalidate would dirty every subsequent frame and the machinery
    // would buy nothing; if it were cleared too early, the frame that
    // needed it would skip.
    let mut host: Host<Counter, fulgurite::Incremental, Boundaries<16>> =
        Host::new(app(), 300, 220);
    host.set_stylesheet(&sheet());
    host.pump();

    assert!(host.invalidate("footer"));
    host.pump();
    assert!(
        host.changed().is_clean(0..<Ui as fulgurite::Widget>::NODE_COUNT),
        "the mark should have been spent by the frame that read it"
    );
}

#[test]
fn a_handler_collapses_the_set_and_a_still_frame_does_not() {
    let mut host: Host<Counter, fulgurite::Incremental, Boundaries<16>> =
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
    let mut host: Host<Counter, fulgurite::Incremental, Boundaries<16>> =
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
    let mut host: Host<Counter, fulgurite::Incremental, Boundaries<16>> =
        Host::new(app(), 300, 220);
    assert!(!host.invalidate("no-such-id"));
    assert!(host.invalidate("card"));
}

/// The size of the node carrying `wid`.
fn size_of<I: Invalidation>(
    host: &Host<Counter, fulgurite::Incremental, I>,
    wid: &str,
) -> fulgurite::Size {
    let table = <Ui as fulgurite::Widget>::SUB_TABLE;
    let i = fulgurite::style::index_of_wid(table, wid).expect("id exists");
    host.states[i].size
}

#[test]
fn the_skip_actually_fires_and_this_is_what_it_costs() {
    // **Proof that T16.9 does something.** The oracle test passing is
    // necessary and not sufficient: it would also pass if the boundary
    // check never fired, which is how a whole class of optimisation
    // lands looking correct and doing nothing.
    //
    // So this asserts the *observable consequence* of skipping — a
    // clean subtree keeps last frame's geometry — and in doing so
    // documents the contract of an explicit dirty set, which is the
    // sharp edge of `Boundaries`:
    //
    //   **mutate without invalidating and you get a stale frame.**
    //
    // That is why `Boundaries` was opt-in until M34/T34.2 made it the
    // default on evidence, and why an unsplit handler running marks
    // everything: the only mutations `Boundaries` can be trusted with are
    // the ones something told it about.
    let mut bounded: Host<Counter, fulgurite::Incremental, Boundaries<16>> =
        Host::new(app(), 300, 220);
    bounded.set_stylesheet(&sheet());
    bounded.pump();
    let before = size_of(&bounded, "title");

    // A much longer title, and no invalidate.
    bounded.app.ui.card.title.set("a considerably longer title than before");
    bounded.pump();
    assert_eq!(
        size_of(&bounded, "title"),
        before,
        "the subtree was clean, so layout kept last frame's size — the skip fired"
    );

    // Told about it, the very next frame catches up.
    assert!(bounded.invalidate("card"));
    bounded.pump();
    let after = size_of(&bounded, "title");
    assert!(after.w > before.w, "invalidating re-laid it out: {before:?} -> {after:?}");

    // The oracle never skips, so it never goes stale in the first
    // place — same mutation, same absence of an invalidate.
    let mut oracle: Host<Counter, fulgurite::Incremental, Everything> =
        Host::new(app(), 300, 220);
    oracle.set_stylesheet(&sheet());
    oracle.pump();
    assert_eq!(size_of(&oracle, "title"), before, "the two start in the same place");
    oracle.app.ui.card.title.set("a considerably longer title than before");
    oracle.pump();
    assert_eq!(
        size_of(&oracle, "title"),
        after,
        "Everything re-lays out unconditionally, so it is never stale"
    );
}

#[test]
fn changed_constraints_defeat_the_skip_even_when_nothing_is_marked() {
    // The second half of the boundary condition. A subtree may be
    // perfectly clean and still need re-laying out, because its parent
    // handed it different limits — which is what happens to every child
    // of a container that changed width. Checking the dirty set alone
    // would produce a tree laid out for the old window.
    let mut host: Host<Counter, fulgurite::Incremental, Boundaries<16>> =
        Host::new(app(), 300, 220);
    host.set_stylesheet(&sheet());
    host.pump();
    let narrow = size_of(&host, "card");

    host.resize(500, 220);
    host.pump();
    let wide = size_of(&host, "card");
    assert!(wide.w > narrow.w, "a resize must re-lay out: {narrow:?} -> {wide:?}");

    // And a still frame after it does not undo the work.
    host.pump();
    assert_eq!(size_of(&host, "card"), wide);
}
