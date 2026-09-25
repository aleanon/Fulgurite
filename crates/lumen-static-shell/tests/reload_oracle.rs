//! **A swapped-in application is drawn whole, under `Boundaries`** —
//! M41/T41.0, for R5-F6 (`docs/plan-boundaries-repair.md` §1.4).
//!
//! `Host::adopt` and `Host::restore` replace or graft the application
//! and then call `reasked`, which marks everything. Nothing tested that
//! mark: every host the reload tests build has a sheet source, and
//! `reasked` re-applies that sheet through `set_stylesheet`, which marks
//! everything too — so deleting `reasked`'s own mark left every gate
//! green. **A host built with `Host::new` has no sheet source**, and for
//! it `reasked`'s mark is the only one. This file has both arms.
//!
//! Each arm drives one session twice: a `Boundaries<256>` host presented
//! into a **persistent** canvas by its damage alone — what a window does
//! — and an `Everything` host rendered whole each frame. The session
//! hovers and presses a button and opens a popup, so the swap lands on
//! pointer state and an overlay; then it adopts a second application
//! whose label, sheet colour and handler all differ, releases the press
//! (so the *new* handler must run) and moves away. Every frame is
//! compared pixel for pixel, and the total is printed beside its bound.

use lumen_static::display::Color;
use lumen_static::event::EventCtx;
use lumen_static::invalidate::{Boundaries, Everything, Invalidation};
use lumen_static::prelude::*;
use lumen_static::retain::{Damage, Incremental};
use lumen_static::style::{Patch, Rule, Selector};
use lumen_static::{Cmd, Event};
use lumen_static_shell::program::application;
use lumen_static_shell::software::Canvas;
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

const W: u32 = 240;
const H: u32 = 180;
const BG: Color = Color(255, 255, 255, 255);
const V1: Color = Color(0xc8, 0x1e, 0x1e, 0xff);
const V2: Color = Color(0x1e, 0x8c, 0x3c, 0xff);

#[derive(StaticTree, Default)]
#[widget(column, spacing = 6.0)]
struct Ui {
    #[widget_id("title")]
    title: Label<String>,
    #[widget_id("go")]
    go: Button<Swap, String>,
    #[widget_id("menu")]
    menu: Popup<Label<String>>,
    #[widget_id("foot")]
    foot: Label<String>,
}

#[derive(DeriveApp, Default)]
struct Swap {
    #[tree]
    ui: Ui,
    clicks: u32,
    /// What the program's style source reads, for the sourced arm.
    sheet: Vec<Rule>,
}

fn sheet(bg: Color) -> Vec<Rule> {
    vec![
        Rule::new(Selector::Kind("button"), Patch::new().hover_dim(0.3).pressed_dim(0.5)),
        Rule::new(Selector::Id("go"), Patch::new().background(bg)),
    ]
}

fn v1() -> Swap {
    let mut a = Swap::default();
    a.ui.title.set("version one".into());
    a.ui.go.label = "one".into();
    a.ui.go.on_click = |s: &mut Swap, _: &EventCtx| s.clicks += 1;
    a.ui.menu.content.set("menu of the first build".into());
    a.ui.foot.set("foot".into());
    a.sheet = sheet(V1);
    a
}

/// The edited build: every visible thing differs, and so does the
/// handler, so a stale frame or a stale handler is visible.
fn v2() -> Swap {
    let mut a = Swap::default();
    a.ui.title.set("the second version".into());
    a.ui.go.label = "second".into();
    a.ui.go.on_click = |s: &mut Swap, _: &EventCtx| s.clicks += 10;
    a.ui.menu.content.set("menu two".into());
    a.ui.menu.open = true;
    a.ui.foot.set("a longer foot".into());
    a.sheet = sheet(V2);
    a
}

/// A host with a sheet source (M32's one construction path).
fn sourced<I: Invalidation>() -> Host<Swap, Incremental, I> {
    application(v1).size(W, H).style(|s: &Swap| s.sheet.clone()).host::<Incremental, I>()
}

/// A host with none: styled only by what its caller sets.
fn sourceless<I: Invalidation>() -> Host<Swap, Incremental, I> {
    let mut h: Host<Swap, Incremental, I> = Host::new(v1(), W, H);
    h.set_stylesheet(&sheet(V1));
    h
}

fn full<I: Invalidation>(h: &Host<Swap, Incremental, I>, c: &mut Canvas) {
    c.clear(BG);
    h.render(c);
}

/// Present by damage alone into a canvas that keeps its pixels.
fn present<I: Invalidation>(h: &mut Host<Swap, Incremental, I>, c: &mut Canvas) {
    match h.take_damage_for_age(1) {
        Damage::None => {}
        Damage::All => full(h, c),
        d @ Damage::Regions(_) => {
            for r in d.regions([0.0, 0.0, W as f32, H as f32]) {
                c.clear_within(r, BG);
                c.render_within(&h.list, r);
            }
        }
    }
}

fn differing(a: &Canvas, b: &Canvas) -> usize {
    a.pixels.iter().zip(&b.pixels).filter(|(x, y)| x != y).count()
}

fn has_text<I: Invalidation>(h: &Host<Swap, Incremental, I>, s: &str) -> bool {
    h.list.cmds.iter().any(|c| matches!(c, Cmd::Text { text, .. } if h.list.str_of(*text) == s))
}

fn node(wid: &str) -> usize {
    lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).unwrap()
}

/// The session, both ways. Returns the differing pixels summed over
/// every frame.
fn session(
    arm: &str,
    mut e: Host<Swap, Incremental, Everything>,
    mut b: Host<Swap, Incremental, Boundaries<256>>,
    swap: impl Fn(&mut dyn Swapper),
    sheet_after: Color,
) -> usize {
    let (mut ce, mut cb) = (Canvas::new(W, H), Canvas::new(W, H));
    let mut total = 0;
    let mut frame = |what: &str,
                     e: &mut Host<Swap, Incremental, Everything>,
                     b: &mut Host<Swap, Incremental, Boundaries<256>>| {
        full(e, &mut ce);
        present(b, &mut cb);
        let d = differing(&ce, &cb);
        println!("{arm} {what}: marks {:?}, differing px {d}", b.last_marks());
        total += d;
    };
    e.pump();
    b.pump();
    frame("first", &mut e, &mut b);

    let go = node("go");
    let s = e.states[go];
    let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);

    for h in [&mut e as &mut dyn Swapper, &mut b] {
        h.post(Event::PointerMove { x, y });
        assert!(!h.pump(), "setup: a hover must be unhandled");
        assert!(h.states()[go].hovered, "setup: the hover did not land on `go`");
    }
    frame("hover", &mut e, &mut b);

    for h in [&mut e as &mut dyn Swapper, &mut b] {
        h.post(Event::down(x, y));
        assert!(!h.pump(), "setup: a press activates on release, so it is unhandled here");
        assert!(h.states()[go].pressed, "setup: the press did not latch on `go`");
    }
    frame("press", &mut e, &mut b);

    for h in [&mut e as &mut dyn Swapper, &mut b] {
        h.open_menu();
        h.pump();
    }
    assert!(has_text(&e, "menu of the first build"), "setup: the popup did not open");
    frame("popup", &mut e, &mut b);

    swap(&mut e);
    swap(&mut b);
    e.pump();
    b.pump();
    assert!(has_text(&e, "the second version"), "setup: the swap did not reach Everything's frame");
    assert_eq!(
        (e.styles[go].background, b.styles[go].background),
        (sheet_after, sheet_after),
        "setup: the sheet in force after the swap is not the one this arm expects"
    );
    frame("swapped", &mut e, &mut b);

    for h in [&mut e as &mut dyn Swapper, &mut b] {
        h.post(Event::up(x, y));
        h.pump();
    }
    assert_eq!((e.app.clicks, b.app.clicks), (10, 10), "setup: the release did not run the new build's handler");
    frame("release", &mut e, &mut b);

    for h in [&mut e as &mut dyn Swapper, &mut b] {
        h.post(Event::PointerMove { x: 2.0, y: H as f32 - 2.0 });
        h.pump();
    }
    frame("away", &mut e, &mut b);
    total
}

/// What the session does to either host, object-safely.
trait Swapper {
    fn post(&mut self, ev: Event);
    fn pump(&mut self) -> bool;
    fn states(&self) -> &[lumen_static::NodeState];
    fn open_menu(&mut self);
    fn adopt(&mut self, app: Swap);
}

impl<I: Invalidation> Swapper for Host<Swap, Incremental, I> {
    fn post(&mut self, ev: Event) {
        Host::post(self, ev)
    }
    fn pump(&mut self) -> bool {
        Host::pump(self)
    }
    fn states(&self) -> &[lumen_static::NodeState] {
        &self.states
    }
    fn open_menu(&mut self) {
        self.app.ui.menu.open = true;
        assert!(self.invalidate("menu"), "the menu id must exist");
    }
    fn adopt(&mut self, app: Swap) {
        Host::adopt(self, app)
    }
}

#[test]
fn an_adopted_application_is_drawn_whole_with_a_sheet_source() {
    let total = session("sourced", sourced(), sourced(), |h| h.adopt(v2()), V2);
    println!("sourced: differing px: {total} / 0");
    assert_eq!(total, 0, "a host with a sheet source drew a stale frame after adopt");
}

#[test]
fn an_adopted_application_is_drawn_whole_without_a_sheet_source() {
    let total = session("sourceless", sourceless(), sourceless(), |h| h.adopt(v2()), V1);
    println!("sourceless: differing px: {total} / 0");
    assert_eq!(total, 0, "a host built with Host::new drew a stale frame after adopt");
}

/// **The `restore` arm** — a window's swap boots the new build and
/// grafts the saved state onto it through `Host::restore`
/// (M37/T37.1), which ends in the same `reasked`. Behind `reload`,
/// because `restore` is. Its own application, because `Popup` is not
/// `Reloadable`; a sourceless host, so `reasked`'s mark is the only one.
#[cfg(feature = "reload")]
mod restore {
    use super::*;
    use lumen_static::event::Application;
    use lumen_static::reload::Reloadable as _;
    use lumen_static::Reloadable as DeriveReloadable;

    #[derive(StaticTree, Default, DeriveReloadable)]
    #[widget(column, spacing = 6.0)]
    struct RUi {
        #[widget_id("title")]
        title: Label<String>,
        #[widget_id("go")]
        go: Button<Kept, String>,
        #[widget_id("foot")]
        foot: Label<String>,
    }

    #[derive(DeriveApp, DeriveReloadable)]
    struct Kept {
        #[tree]
        ui: RUi,
        #[reload(keep)]
        clicks: u32,
    }

    impl Application for Kept {
        fn build() -> Self {
            let mut a = Kept { ui: RUi::default(), clicks: 0 };
            a.ui.title.set("before the restore".into());
            a.ui.go.label = "go".into();
            a.ui.go.on_click = |s: &mut Kept, _: &EventCtx| s.clicks += 1;
            a.ui.foot.set("foot".into());
            a
        }
    }

    fn host<I: Invalidation>() -> Host<Kept, Incremental, I> {
        let mut h: Host<Kept, Incremental, I> = Host::new(Kept::build(), W, H);
        h.set_stylesheet(&sheet(V1));
        h
    }

    fn full<I: Invalidation>(h: &Host<Kept, Incremental, I>, c: &mut Canvas) {
        c.clear(BG);
        h.render(c);
    }

    fn present<I: Invalidation>(h: &mut Host<Kept, Incremental, I>, c: &mut Canvas) {
        match h.take_damage_for_age(1) {
            Damage::None => {}
            Damage::All => full(h, c),
            d @ Damage::Regions(_) => {
                for r in d.regions([0.0, 0.0, W as f32, H as f32]) {
                    c.clear_within(r, BG);
                    c.render_within(&h.list, r);
                }
            }
        }
    }

    #[test]
    fn a_restored_state_is_drawn_whole_without_a_sheet_source() {
        let mut e: Host<Kept, Incremental, Everything> = host();
        let mut b: Host<Kept, Incremental, Boundaries<256>> = host();
        let (mut ce, mut cb) = (Canvas::new(W, H), Canvas::new(W, H));
        e.pump();
        b.pump();
        full(&e, &mut ce);
        present(&mut b, &mut cb);
        let go = lumen_static::style::index_of_wid(<RUi as Widget>::SUB_TABLE, "go").unwrap();
        let s = e.states[go];
        let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
        e.post(Event::PointerMove { x, y });
        b.post(Event::PointerMove { x, y });
        e.pump();
        b.pump();
        assert!(b.states[go].hovered, "setup: the hover did not land");
        full(&e, &mut ce);
        present(&mut b, &mut cb);

        // What a swap carries across: the kept state of a session whose
        // tree says something else.
        let mut other = Kept::build();
        other.ui.title.set("a title the saved session had".into());
        other.clicks = 4;
        let saved = other.save();
        e.restore(&saved);
        b.restore(&saved);
        let mut total = 0;
        for f in 0..3 {
            e.pump();
            b.pump();
            full(&e, &mut ce);
            present(&mut b, &mut cb);
            let d = differing(&ce, &cb);
            println!("restore frame {f}: marks {:?}, differing px {d}", b.last_marks());
            total += d;
        }
        assert_eq!(e.app.ui.title.as_str(), "a title the saved session had", "setup: the restore did not happen");
        assert_eq!(b.app.clicks, 4, "setup: the kept field was not restored");
        println!("restore: differing px: {total} / 0");
        assert_eq!(total, 0, "a restored session drew a stale frame");
    }
}
