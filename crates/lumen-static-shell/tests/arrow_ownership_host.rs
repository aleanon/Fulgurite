//! **An arrow belongs to the widget that uses it, through a `Host`** —
//! M44/T44.1 (`docs/plan-input-semantics.md` §2.1), the `Host` legs of
//! `crates/lumen-static/tests/arrow_ownership.rs`.
//!
//! The same shape — a `TextInput` holding `abc` beside a `Button`, End,
//! one Left — for an unsplit and a split application, each under
//! `Everything` and `Boundaries<256>`. The split leg is the one that
//! matters: since M29/T29.6 a split application redraws only what an
//! event *consumed*, so a widget that took the arrow without the pass
//! recording it paints last frame's caret under `Boundaries` while
//! `Everything` paints the new one (§0 rule 1). Each leg prints
//! `(focus, caret)` beside the expected pair, and the frame hashes and
//! damage beside each other.

use lumen_static::cx::{Cluster, GlyphSink, TextMeasure};
use lumen_static::event::EventCtx;
use lumen_static::invalidate::{Boundaries, Everything, Invalidation};
use lumen_static::prelude::*;
use lumen_static::retain::{Damage, Incremental};
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{Event, Key, Size};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

/// A measurer **with clusters**. The host's stand-in (`BlockMetrics`)
/// reports none, and a field with no clusters draws its caret at the end
/// of the run wherever it is — so under the stand-in a caret moved by
/// Left paints exactly where it was, and a stale caret is invisible. The
/// first version of this test used the stand-in and its injection (c),
/// the prototype without `consumed.push`, left it green.
struct Prop;

impl TextMeasure for Prop {
    fn measure(&mut self, t: &str, size: f32, _max_w: f32) -> Size {
        Size { w: t.chars().count() as f32 * size * 0.5, h: size }
    }

    fn shape(&mut self, t: &str, size: f32, weight: f32, max_w: f32, sink: &mut dyn GlyphSink) -> Size {
        let mut x = 0.0;
        for (b, _) in t.char_indices() {
            sink.cluster(Cluster { byte: b as u32, x, line: 0, rtl: false });
            x += size * 0.5;
        }
        sink.cluster(Cluster { byte: t.len() as u32, x, line: 0, rtl: false });
        self.measure_weighted(t, size, weight, max_w)
    }
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
struct URow {
    #[widget_id("field")]
    field: TextInput,
    #[widget_id("ok")]
    ok: Button<Unsplit>,
}

#[derive(DeriveApp, Default)]
struct Unsplit {
    #[tree]
    ui: URow,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
struct SRow {
    #[widget_id("field")]
    field: TextInput,
    #[widget_id("ok")]
    ok: Button<Store>,
}

#[derive(Default)]
struct Store {
    clicks: u32,
}

#[derive(DeriveApp, Default)]
struct Split {
    #[tree]
    ui: SRow,
    #[state]
    store: Store,
}

fn sheet() -> Vec<Rule> {
    vec![
        Rule::new(Selector::Id("field"), Patch::new().width(Length::Fixed(160.0)).height(Length::Fixed(24.0))),
        Rule::new(Selector::Id("ok"), Patch::new().width(Length::Fixed(60.0)).height(Length::Fixed(24.0))),
    ]
}

fn host<A: lumen_static::event::App, I: Invalidation>(app: A) -> Host<A, Incremental, I> {
    let mut h: Host<A, Incremental, I> = Host::new(app, 320, 80);
    h.set_stylesheet(&sheet());
    h.pump_with(&mut Prop);
    h.pump_with(&mut Prop);
    h
}

fn index<A: lumen_static::event::App>(wid: &str) -> u32 {
    lumen_static::style::index_of_wid(<A::Tree as Widget>::SUB_TABLE, wid).expect(wid) as u32
}

const TEXT: &str = "abcdefghijklmnopqrstuvwx";

/// Drive both hosts in lockstep: focus the field by clicking it, End,
/// then Left until the caret is near the start, comparing the two frames
/// after every Left.
///
/// **Why so many Lefts.** A caret move alone repaints correctly even when
/// nothing marked the field, because `paint` reads the caret directly.
/// What a skipped layout leaves stale is the field's *scroll*: the text
/// is wider than the field, End scrolls it, and walking the caret back
/// past the left edge must scroll it again — which `layout` computes. So
/// the frames are compared across that crossing. The first version
/// compared one Left, and the review prototype's shape (ask the widget
/// first, never `consumed.push`) left it green: its injection (c) proved
/// only that the caret is painted from state.
///
/// The first Left also turns the focus ring on, and T25.0 marks the
/// focused node for that, so the frames after it would agree even without
/// a consumed mark; every later Left changes only what `Consumed` marks.
fn drive<A: lumen_static::event::App>(
    what: &str,
    e: &mut Host<A, Incremental, Everything>,
    b: &mut Host<A, Incremental, Boundaries<256>>,
    caret: impl Fn(&A) -> usize,
) {
    let field = index::<A>("field");
    let s = b.states[field as usize];
    let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    for ev in [Event::down(x, y), Event::up(x, y), Event::Key { key: Key::End, down: true }] {
        e.post(ev);
        b.post(ev);
        e.pump_with(&mut Prop);
        b.pump_with(&mut Prop);
    }
    assert_eq!(b.input.focus, Some(field), "setup: the click did not focus the field");
    assert_eq!(caret(&b.app), TEXT.len(), "setup: End did not reach the field");

    let mut damaged = 0;
    for k in 0..20 {
        e.post(Event::Key { key: Key::Left, down: true });
        b.post(Event::Key { key: Key::Left, down: true });
        e.pump_with(&mut Prop);
        b.pump_with(&mut Prop);
        let (fe, fb) = ((e.input.focus, caret(&e.app)), (b.input.focus, caret(&b.app)));
        let want = (Some(field), TEXT.len() - 1 - k);
        let (he, hb) = (e.list.hash_from(0), b.list.hash_from(0));
        println!(
            "{what}: Left #{}: Everything (focus, caret) = {fe:?}  Boundaries = {fb:?}  expected {want:?}; \
             hashes {he:#018x} / {hb:#018x}; marks {:?}",
            k + 1,
            b.last_marks()
        );
        assert_eq!(fb, want, "{what}: Left #{} left the field under Boundaries", k + 1);
        assert_eq!(fe, want, "{what}: Left #{} left the field under Everything", k + 1);
        if !matches!(b.damage(), Damage::None) {
            damaged += 1;
        }
        // An unsplit application's handled event marks everything by
        // design (T16.7); only a split one narrows, so only its leg can
        // see a missing mark, and it must not fall back.
        if A::STATE_IS_SPLIT {
            assert!(!b.last_marks().all, "{what}: a Left marked everything, so this leg cannot see a missing mark");
        }
        assert_eq!(he, hb, "{what}: after Left #{} the narrowed frame differs from the whole-tree frame", k + 1);
    }
    println!("{what}: {damaged} of 20 Lefts damaged the frame (expected 20)");
    assert_eq!(damaged, 20, "{what}: a caret that moved damaged nothing");
}

#[test]
fn an_unsplit_application_keeps_the_arrow_in_the_field() {
    let make = || {
        let mut u = Unsplit::default();
        u.ui.field.set_str(TEXT);
        u.ui.ok.label = "ok";
        u
    };
    let mut e: Host<Unsplit, Incremental, Everything> = host(make());
    let mut b: Host<Unsplit, Incremental, Boundaries<256>> = host(make());
    drive("unsplit", &mut e, &mut b, |a: &Unsplit| a.ui.field.caret());
}

#[test]
fn a_split_application_keeps_the_arrow_in_the_field_and_redraws_the_caret() {
    let make = || {
        let mut s = Split::default();
        s.ui.field.set_str(TEXT);
        s.ui.ok.label = "ok";
        s.ui.ok.on_click = |s: &mut Store, _: &EventCtx| s.clicks += 1;
        s
    };
    let mut e: Host<Split, Incremental, Everything> = host(make());
    let mut b: Host<Split, Incremental, Boundaries<256>> = host(make());
    drive("split", &mut e, &mut b, |a: &Split| a.ui.field.caret());
}
