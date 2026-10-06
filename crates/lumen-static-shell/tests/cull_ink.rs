//! **Culling by ink, not by rect** — M85/T85.6's gate for the plan's X2
//! (`docs/plan-layout-input.md`, Decisions, "The ink, not the rect").
//!
//! Two subjects whose pixels are on the surface while their rect is not:
//!
//! - a shadowed `Card` whose rect starts exactly at the surface's bottom
//!   edge, under another card that fills the surface: its shadow drops
//!   0.35 × its spread and grows its spread on every side
//!   (`display::shadow_ink`), so its top few rows land on the card above;
//! - an open `Select` whose trigger is below the surface: its menu is a
//!   `Popup`, which registers for the overlay pass from inside its own
//!   paint and is kept on the window, so it draws on the surface — but
//!   only if the paint that defers it runs.
//!
//! Each is laid out by a `Host` in a 300 × 300 window, inside a
//! `ScrollView` the size of the window (so the column below the fold is
//! laid out, not squeezed), and painted twice with `pass::paint_overlaid`
//! through `Scaled(list)`, the host's order — once with the surface as
//! `set_bounds`, as `Host::frame` paints, once with `None` — and both
//! lists are rendered on the software canvas.
//!
//! - **Derived containers**: byte-identical. They do not cull at T85.6,
//!   and the cull T85.8 adds (ops::cull_child) must keep them identical
//!   by culling on ink and never culling a subtree flagged
//!   `NodeDesc::INKS_OUTSIDE`.
//! - **`List`**, which culls by `rect_of` today (T16.12): the two renders
//!   of the card case **differ** — the shadow vanishes when culled. Pinned
//!   green (index §3.1); T85.8 moves `List` onto that cull and flips
//!   it.
//!
//! **There is no `List` variant of the menu case**, because a `Popup`
//! inside a `List` draws no menu even unculled: it defers its arena-local
//! index, and the overlay pass repaints the *main-table* node at that
//! index, so the menu lands inside the list's own clip, off the surface,
//! and the two renders agree for the wrong reason
//! (`docs/plan-layout-input.md`, "What building found", T85.6).
//!
//! Each case prints the subject's rect, its ink and the surface, and
//! asserts that the setup landed: the rect is off the surface, the ink is
//! on it, the menu is open (`content_absent()` false) and the uncull
//! render actually holds the ink. The figures are recorded once, in
//! `docs/results-layout-input.md` (T85.6).

use lumen_static::display::shadow_ink;
use lumen_static::event::App as AppTrait;
use lumen_static::hatch::List;
use lumen_static::overlay::Overlays;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::widget::Node;
use lumen_static::{pass, App as DeriveApp, DisplayList, Renderer, Scaled, StaticTree, Widget};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::{Host, CLEAR};
use lumen_static_widgets::ops::Fixed;
use lumen_static_widgets::select::Row;
use lumen_static_widgets::{Card, ScrollView, Select};

const SIDE: u32 = 300;
const S: f32 = SIDE as f32;
const SURFACE: [f32; 4] = [0.0, 0.0, S, S];
/// The card above: its 16 px padding on each side plus this fills the
/// surface exactly, so the card below starts at the surface's edge.
const TOP_CONTENT: f32 = S - 32.0;

type Rect = [f32; 4];

fn meets(a: Rect, b: Rect) -> bool {
    a[0].max(b[0]) < a[2].min(b[2]) && a[1].max(b[1]) < a[3].min(b[3])
}

// ---- the trees ------------------------------------------------------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Cards {
    #[widget_id("above")]
    above: Card<Fixed>,
    #[widget_id("below")]
    below: Card<Fixed>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct CardUi {
    #[widget_id("scroll")]
    scroll: ScrollView<Cards>,
}

#[derive(DeriveApp, Default)]
struct CardApp {
    #[tree]
    ui: CardUi,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct CardRows {
    #[widget_id("list")]
    list: List<Card<Fixed>>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct CardListUi {
    #[widget_id("scroll")]
    scroll: ScrollView<CardRows>,
}

#[derive(DeriveApp, Default)]
struct CardListApp {
    #[tree]
    ui: CardListUi,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Pick {
    #[widget_id("gap")]
    gap: Fixed,
    #[widget_id("sel")]
    sel: Select<SelApp>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct SelUi {
    #[widget_id("scroll")]
    scroll: ScrollView<Pick>,
}

#[derive(DeriveApp, Default)]
struct SelApp {
    #[tree]
    ui: SelUi,
}

const OPTS: [&str; 3] = ["Apple", "Banana", "Cherry"];

fn bind<S: 'static>(r: &mut Row<S>, i: usize) {
    r.label.clear();
    r.label.push_str(OPTS[i]);
}

fn select<S: 'static>() -> Select<S> {
    let mut s = Select::new(OPTS.len(), 20.0, bind::<S>);
    s.open();
    s
}

fn card(h: f32) -> Card<Fixed> {
    Card::new(Fixed { w: 100.0, h })
}

// ---- the paint ------------------------------------------------------

/// Lay `app` out in a `SIDE` × `SIDE` host, then paint it with the
/// surface as the bound and with `None`, and render both.
fn renders<A: AppTrait>(app: A) -> (Host<A>, Vec<u8>, Vec<u8>, usize, usize) {
    let mut h: Host<A> = Host::new(app, SIDE, SIDE);
    h.set_stylesheet(&[
        Rule::new(Selector::Id("scroll"), Patch::new().width(Length::Fixed(S)).height(Length::Fixed(S))),
        // A menu panel taller than its three 20 px rows, and opaque, so
        // the menu has ink a render can see without a font. (The rows'
        // own kind is not styled: a rule on them does not reach a
        // `Select` inside a `List`'s arena, where the panel's does.)
        Rule::new(
            Selector::Kind("repeat"),
            Patch::new().height(Length::Fixed(100.0)).background(lumen_static::display::Color(40, 60, 120, 255)),
        ),
    ]);
    h.pump();
    let paint = |bounds: Option<Rect>| {
        let mut list = DisplayList::new();
        list.set_bounds(bounds);
        let mut ov = Overlays::<16>::default();
        {
            let mut out = Scaled::new(&mut list, 1.0);
            pass::paint_overlaid(h.app.tree(), &h.states, &mut out, &mut ov);
        }
        let mut c = Canvas::new(SIDE, SIDE);
        c.clear(CLEAR);
        c.render(&list, SIDE, SIDE);
        (c.pixels, list.cmds.len())
    };
    let (some, n_some) = paint(Some(SURFACE));
    let (none, n_none) = paint(None);
    (h, some, none, n_some, n_none)
}

fn differing(a: &[u8], b: &[u8]) -> usize {
    a.chunks(4).zip(b.chunks(4)).filter(|(p, q)| p != q).count()
}

/// Pixels in `r` (clipped to the surface) that are not `CLEAR`.
fn inked(px: &[u8], r: Rect) -> usize {
    let (x0, y0) = (r[0].max(0.0) as usize, r[1].max(0.0) as usize);
    let (x1, y1) = (r[2].min(S) as usize, r[3].min(S) as usize);
    let mut n = 0;
    for y in y0..y1 {
        for x in x0..x1 {
            let p = (y * SIDE as usize + x) * 4;
            if px[p..p + 3] != [CLEAR.0, CLEAR.1, CLEAR.2] {
                n += 1;
            }
        }
    }
    n
}

fn rect_at<A: AppTrait>(h: &Host<A>, i: usize) -> Rect {
    let s = h.states[i];
    [s.pos.x, s.pos.y, s.pos.x + s.size.w, s.pos.y + s.size.h]
}

/// The card case's setup: `rect` off the surface, its shadow's ink on
/// it, and the band of the card above that the ink covers. Answers the
/// band.
fn card_setup(what: &str, rect: Rect, shadow: f32) -> Rect {
    let ink = shadow_ink(rect, shadow);
    println!("{what}: card rect {rect:?}, shadow {shadow}, ink {ink:?}, surface {SURFACE:?}");
    assert!(shadow > 0.0, "setup ({what}): the card casts no shadow");
    assert!(!meets(rect, SURFACE), "setup ({what}): the card's rect {rect:?} is on the surface");
    assert!(meets(ink, SURFACE), "setup ({what}): the card's ink {ink:?} misses the surface");
    // Only the band above the card's top edge and inside its width:
    // the card above is white there, so a shadow is the only ink.
    [rect[0], ink[1], rect[2], S]
}

/// The select case's setup: the popup open, the trigger off the surface,
/// the menu on it. Answers the menu's rect.
fn select_setup(what: &str, root: usize, trigger: Rect, menu: Rect, absent: bool) -> Rect {
    println!(
        "{what}: trigger rect {trigger:?}, menu rect {menu:?} (ink), surface {SURFACE:?}, content_absent() {absent}; select at node {root}"
    );
    assert!(!absent, "setup ({what}): the menu is closed (content_absent() true)");
    assert!(!meets(trigger, SURFACE) && trigger[1] >= S, "setup ({what}): the trigger {trigger:?} is on the surface");
    assert!(meets(menu, SURFACE), "setup ({what}): the menu {menu:?} is not on the surface");
    menu
}

// ---- the gates ------------------------------------------------------

#[test]
fn a_derived_column_paints_a_shadow_from_below_the_surface() {
    let mut app = CardApp::default();
    app.ui.scroll.content.above = card(TOP_CONTENT);
    app.ui.scroll.content.below = card(40.0);
    let (h, some, none, n_some, n_none) = renders(app);
    let below = lumen_static::style::index_of_wid(<CardUi as Widget>::SUB_TABLE, "below").unwrap();
    let band = card_setup("derived", rect_at(&h, below), h.states[below].shadow);
    let ink = inked(&none, band);
    let diff = differing(&some, &none);
    println!("derived: commands {n_some} / {n_none}; shadow pixels on the surface (None render) {ink}; Some vs None differ in {diff} pixels (bound 0)");
    assert!(ink > 0, "setup (derived): the uncull render shows no shadow in {band:?}");
    assert_eq!(diff, 0, "a derived column culled a card whose shadow is on the surface: {diff} pixels differ");
}

#[test]
fn a_derived_column_paints_an_open_menu_whose_trigger_is_below_the_surface() {
    let mut app = SelApp::default();
    app.ui.scroll.content.gap = Fixed { w: 10.0, h: S };
    app.ui.scroll.content.sel = select();
    let (h, some, none, n_some, n_none) = renders(app);
    let table = <SelUi as Widget>::SUB_TABLE;
    let sel = lumen_static::style::index_of_wid(table, "sel").unwrap();
    let (trigger, menu, absent) = parts(&h, sel);
    let menu = select_setup("derived", sel, trigger, menu, absent);
    let ink = inked(&none, menu);
    let diff = differing(&some, &none);
    println!("derived: commands {n_some} / {n_none}; menu pixels on the surface (None render) {ink}; Some vs None differ in {diff} pixels (bound 0)");
    assert!(ink > 0, "setup (derived): the uncull render shows no menu in {menu:?}");
    assert_eq!(diff, 0, "a derived column culled an open menu whose trigger is below the surface: {diff} pixels differ");
}

/// The trigger's rect, the menu's rect and the popup's `content_absent`,
/// for the `Select` whose subtree starts at `sel` in the host's table.
fn parts<A: AppTrait>(h: &Host<A>, sel: usize) -> (Rect, Rect, bool) {
    let table = <A::Tree as Widget>::SUB_TABLE;
    let tree = h.app.tree();
    let end = lumen_static::style::subtree_range(table, sel).end;
    let popup = (sel..end).find(|&i| tree.node(i).floats_children()).expect("the Select holds a popup");
    let trigger = rect_at(h, sel + 1);
    let menu = rect_at(h, popup + 1);
    (trigger, menu, tree.node(popup).content_absent())
}

#[test]
fn a_list_culls_a_shadow_from_below_the_surface_today() {
    let mut app = CardListApp::default();
    app.ui.scroll.content.list = List::new(vec![card(TOP_CONTENT), card(40.0)]);
    let (h, some, none, n_some, n_none) = renders(app);
    let below = h.app.ui.scroll.content.list.item_state(1, 0);
    let rect = [below.pos.x, below.pos.y, below.pos.x + below.size.w, below.pos.y + below.size.h];
    let band = card_setup("List", rect, below.shadow);
    let ink = inked(&none, band);
    let diff = differing(&some, &none);
    println!("List: commands {n_some} / {n_none}; shadow pixels on the surface (None render) {ink}; Some vs None differ in {diff} pixels");
    assert!(ink > 0, "setup (List): the uncull render shows no shadow in {band:?}");
    // Pinned (T85.6): `List` culls by `rect_of`, so the shadow vanishes.
    // T85.8 flips this to byte-identical.
    assert!(diff > 0, "a List no longer culls the card below the surface ({diff} pixels differ) -- the gap this pins has closed; flip the gate (T85.8)");
}
