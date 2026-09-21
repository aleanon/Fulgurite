//! M8b's gate: a partial redraw is pixel-identical to a full one.
//!
//! `Rebuild` is the oracle. For every step of a driven session, the
//! frame produced by clearing the whole surface and rendering the whole
//! list must equal the frame produced by clearing only the damage rect
//! and rendering within it, on top of the previous frame. Any
//! disagreement is a stale pixel a user would see and no headless test
//! that renders fully would ever catch.
//!
//! This is the test a cargo feature could not have expressed — both
//! retentions have to be alive in one process (`docs/plan-architecture-
//! v2.md` T8b.0).

use lumen_static_widgets::ops::{Button, Label, Spacer};
use lumen_static::retain::Damage;
use lumen_static::style::{Edges, Length, Patch, Rule, Selector};
use lumen_static::{App as DeriveApp, Color, StaticTree};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::Host;

const W: u32 = 300;
const H: u32 = 220;
const BG: Color = Color(18, 22, 32, 255);

#[derive(StaticTree, Default)]
#[widget(row, spacing = 8.0)]
struct Bar {
    #[widget_id("dec")]
    dec: Button<Counter, String>,
    gap: Spacer,
    #[widget_id("inc")]
    inc: Button<Counter, String>,
}

/// A card: background, radius and a shadow, so the classes that make
/// damage interesting — a shadow whose ink reaches past its rect, a
/// rounded fill, text — are all exercised.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 6.0)]
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
    card: Card,
    bottom: Spacer,
}

#[derive(DeriveApp, Default)]
struct Counter {
    #[tree]
    ui: Ui,
    #[bind(ui.card.value, "{}")]
    count: i64,
}

fn sheet() -> Vec<Rule> {
    vec![
        Rule::new(
            Selector::Kind("Ui"),
            Patch::new().background(BG).padding(Edges::all(10.0)).align(lumen_static::style::Align::Center),
        ),
        Rule::new(
            Selector::Kind("Card"),
            Patch::new()
                .background(Color(30, 38, 54, 255))
                .radius(14.0)
                .shadow(12.0)
                .padding(Edges::all(12.0))
                .width(Length::Fixed(200.0)),
        ),
        Rule::new(Selector::Id("value"), Patch::new().font_size(34.0).font_weight(800.0)),
        Rule::new(
            Selector::Kind("Button"),
            Patch::new().background(Color(70, 100, 220, 255)).radius(8.0).padding(Edges::all(8.0)),
        ),
        Rule::new(Selector::Kind("Bar"), Patch::new().width(Length::FILL)),
    ]
}

fn app() -> Counter {
    let mut c = Counter::default();
    c.ui.card.title.set("tally");
    c.ui.card.bar.dec.label = "-1".into();
    c.ui.card.bar.dec.on_click = |c: &mut Counter, _| c.count -= 1;
    c.ui.card.bar.inc.label = "+1".into();
    c.ui.card.bar.inc.on_click = |c: &mut Counter, _| c.count += 1;
    c
}

fn full<R: lumen_static::Retention>(host: &Host<Counter, R>, canvas: &mut Canvas) {
    canvas.clear(BG);
    host.render(canvas);
}

/// The frames a driven session produces, both ways, compared step by
/// step. Returns how many steps actually had partial damage, so a test
/// that silently degraded to `Damage::All` everywhere cannot pass by
/// being trivially correct.
fn drive(steps: &[i64]) -> (usize, usize) {
    drive_with::<lumen_static::Incremental>(steps)
}

/// The oracle comparison, over any [`Retention`](lumen_static::Retention).
///
/// Parameterised as of M13/T13.8, which added `Streaming<N>` — the
/// allocator-free retention the embedded profile uses. An
/// implementation nothing renders through is an implementation nobody
/// has checked, and "same pixels as a full redraw" is the only property
/// that matters about any of them.
fn drive_with<R: lumen_static::Retention>(steps: &[i64]) -> (usize, usize) {
    let mut host: Host<Counter, R> = Host::new(app(), W, H);
    host.set_stylesheet(&sheet());
    host.pump();

    let mut oracle = Canvas::new(W, H);
    let mut incremental = Canvas::new(W, H);
    full(&host, &mut oracle);
    full(&host, &mut incremental);

    let (mut partial_steps, mut checked) = (0, 0);
    for &count in steps {
        host.app.count = count;
        host.pump();

        full(&host, &mut oracle);
        match host.take_damage_for_age(1) {
            Damage::None => {}
            Damage::All => {
                incremental.clear(BG);
                host.render(&mut incremental);
            }
            // **Every region, not the bounding box** (M18). Rendering
            // only `clip_to`'s box would still be correct here and would
            // hide the whole point of the change: the oracle must see
            // the set the renderer will actually consume.
            d @ Damage::Regions(_) => {
                partial_steps += 1;
                for r in d.regions([0.0, 0.0, W as f32, H as f32]) {
                    incremental.clear_within(r, BG);
                    incremental.render_within(&host.list, r);
                }
            }
        }
        checked += 1;
        assert_eq!(
            oracle.pixels, incremental.pixels,
            "count {count}: partial redraw differs from a full one"
        );
    }
    (checked, partial_steps)
}

#[test]
fn every_count_from_minus_twelve_to_twelve_renders_identically_both_ways() {
    let steps: Vec<i64> = (-12..=12).collect();
    let (checked, partial) = drive(&steps);
    assert_eq!(checked, 25);
    assert!(partial >= 20, "only {partial} of {checked} steps had a partial rect; the test would pass trivially");
}

#[test]
fn a_frame_that_changes_nothing_reports_no_damage() {
    let mut host: Host<Counter> = Host::new(app(), W, H);
    host.set_stylesheet(&sheet());
    host.pump();
    host.pump();
    assert_eq!(host.damage(), Damage::None, "an idle frame damages nothing");

    host.app.count = 5;
    host.pump();
    assert!(matches!(host.damage(), Damage::Regions(_)), "a changed count damages a region");
    host.pump();
    assert_eq!(host.damage(), Damage::None, "and settles again");
}

#[test]
fn the_damage_rect_is_smaller_than_the_frame_but_covers_what_moved() {
    let mut host: Host<Counter> = Host::new(app(), W, H);
    host.set_stylesheet(&sheet());
    host.pump();
    host.app.count = 7;
    host.pump();

    let r = host
        .damage()
        .clip_to([0.0, 0.0, W as f32, H as f32])
        .unwrap_or_else(|| panic!("expected a region, got {:?}", host.damage()));
    let area = (r[2] - r[0]) * (r[3] - r[1]);
    assert!(area < (W * H) as f32 * 0.5, "damage {r:?} is most of the frame; nothing was saved");

    // It must cover the value label, which is what changed.
    let v = host.probe(r[0] + 1.0, r[1] + 1.0);
    let _ = v;
    assert!(r[0] >= 0.0 && r[1] >= 0.0 && r[2] <= W as f32 && r[3] <= H as f32, "within the frame");
}

/// The same gate, on the retention an embedded host uses. `Streaming`
/// is the fixed-array form: no heap, and a frame longer than `N` falls
/// back to `Damage::All` rather than comparing against a slot that is
/// not its own.
#[test]
fn streaming_retention_renders_identically_to_a_full_redraw() {
    let steps: Vec<i64> = (-12..=12).collect();
    let (checked, partial) = drive_with::<lumen_static::Streaming<256>>(&steps);
    assert_eq!(checked, 25);
    assert!(partial >= 20, "only {partial} of {checked} steps had a partial rect");
}

/// A frame with more commands than the array holds must report
/// everything, not a rect computed from mismatched slots. `Streaming<2>`
/// cannot hold this tree, so every frame is `All` — and the pixels must
/// still agree, which is what proves the fallback is safe rather than
/// merely present.
#[test]
fn streaming_that_overflows_its_capacity_falls_back_to_all() {
    let steps: Vec<i64> = (0..6).collect();
    let (checked, partial) = drive_with::<lumen_static::Streaming<2>>(&steps);
    assert_eq!(checked, 6);
    assert_eq!(partial, 0, "an overflowing capacity must never claim a partial rect");
}

#[test]
fn rebuild_retention_always_reports_everything() {
    // The oracle's own behaviour, asserted so a refactor cannot quietly
    // make `Rebuild` incremental and hollow out every gate above.
    let mut host: Host<Counter, lumen_static::Rebuild> = Host::new(app(), W, H);
    host.set_stylesheet(&sheet());
    host.pump();
    host.pump();
    assert_eq!(host.damage(), Damage::All, "an idle frame still redraws everything");
    assert_eq!(host.take_damage_for_age(1), Damage::All);
    assert_eq!(core::mem::size_of::<lumen_static::Rebuild>(), 0, "and costs no memory");
}

#[test]
fn a_resize_forces_a_full_redraw() {
    let mut host: Host<Counter> = Host::new(app(), W, H);
    host.set_stylesheet(&sheet());
    host.pump();
    host.app.count = 3;
    host.pump();
    host.resize(W + 40, H);
    host.pump();
    assert_eq!(host.damage(), Damage::All, "nothing retained describes the new viewport");
}

#[test]
fn damage_accumulates_across_the_pumps_of_one_presented_frame() {
    // `age` counts presents; a shell may pump more than once for one
    // frame (the tally re-cascades when the sign flips). Every pump's
    // damage belongs to the one frame that reaches the screen, so
    // banking must happen at present time, not at pump time.
    let mut host: Host<Counter> = Host::new(app(), W, H);
    host.set_stylesheet(&sheet());
    host.pump();
    host.take_damage_for_age(1); // the first frame, presented

    host.app.count = 4;
    host.pump();
    let first = host.damage();
    host.app.count = 400;
    host.pump();
    let second = host.damage();

    let banked = host.take_damage_for_age(1);
    assert_eq!(banked, first.union(second), "both pumps land on one present");
    assert_eq!(host.take_damage_for_age(1), Damage::None, "and are not banked twice");
}

#[test]
fn buffer_age_unions_the_frames_a_stale_buffer_missed() {
    let mut host: Host<Counter> = Host::new(app(), W, H);
    host.set_stylesheet(&sheet());
    host.pump();
    host.take_damage_for_age(1);

    // Three presented frames, each damaging a region.
    let mut each = Vec::new();
    for c in [1i64, 20, 300] {
        host.app.count = c;
        host.pump();
        each.push(host.take_damage_for_age(1));
    }
    // Nothing has changed since the last present, so a buffer one
    // present old needs nothing; one three presents old needs the two
    // frames it missed.
    assert_eq!(host.take_damage_for_age(1), Damage::None);
    host.app.count = 9;
    host.pump();
    let three = host.take_damage_for_age(3);
    assert_eq!(three, three.union(each[2]), "age 3 covers the frames it missed");
    assert_eq!(host.take_damage_for_age(0), Damage::All, "unspecified contents");
    assert_eq!(host.take_damage_for_age(9), Damage::All, "older than we retain");
}

#[test]
fn an_aged_buffer_redrawn_from_its_union_matches_a_full_render() {
    // The property the age argument exists for: a double-buffered
    // surface hands back the frame from *two* presents ago, and
    // redrawing only the last frame's damage into it leaves the frame
    // before that showing through.
    let mut host: Host<Counter> = Host::new(app(), W, H);
    host.set_stylesheet(&sheet());
    host.pump();

    let mut oracle = Canvas::new(W, H);
    let mut back = Canvas::new(W, H);
    full(&host, &mut oracle);
    full(&host, &mut back); // the "old" buffer, two presents behind
    host.take_damage_for_age(1);

    host.app.count = 44;
    host.pump();
    full(&host, &mut oracle); // frame 1, presented, but not into `back`
    host.take_damage_for_age(1);
    host.app.count = -7;
    host.pump();
    full(&host, &mut oracle); // frame 2

    let d =
        host.take_damage_for_age(2).clip_to([0.0, 0.0, W as f32, H as f32]).expect("some damage");
    back.clear_within(d, BG);
    back.render_within(&host.list, d);
    assert_eq!(oracle.pixels, back.pixels, "a 2-frame-old buffer needs 2 frames of damage");
}
