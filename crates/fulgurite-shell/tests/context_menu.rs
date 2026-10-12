//! **`ContextMenu<W>`, `CommandList` and `Program::on_menu`** — M90/T90.4
//! (`docs/plan-desktop-integration.md`, "A menu is data, and every
//! activation is one hook" and "`ContextMenu<W>` and `CommandList`").
//!
//! A right press inside the wrapper opens its list with the top-left at
//! the press (`Anchor::At`), clamped inside the window near an edge; a
//! release on an enabled row runs `Program::on_menu` once with that
//! entry's id, taken from the list that consumed the release; a disabled
//! row runs nothing and leaves the list open; and an application without
//! a hook is never asked `Node::take_menu` at all (K90b). A native
//! activation posted as `Event::Menu` ends in the same hook. The frame
//! that opens the menu damages the list and the wrapper, no more.
//!
//! Built with `Program::host`, driven by `post` and `pump`, under
//! `Everything` and `Boundaries<256>`. Each gate asserts its setup landed
//! and prints its quantity beside its bound.

use fulgurite::desc::{NodeDesc, NodeState, NO_PARENT};
use fulgurite::display::Painter;
use fulgurite::event::{EventCtx, Handlers, NodeSel, PointerButton};
use fulgurite::invalidate::{Boundaries, Everything, Invalidation};
use fulgurite::menu::{MenuEntry, MenuMask};
use fulgurite::ops::{record_painted, resolve_self};
use fulgurite::prelude::*;
use fulgurite::retain::{Damage, Incremental};
use fulgurite::style::{Length, Seed, Style};
use fulgurite::{Event, Key, LayoutCx, Mods};
use fulgurite_shell::program::{application, KeyOutcome, Program};
use fulgurite_shell::Host;
use fulgurite_widgets::prelude::*;

const W: u32 = 360;
const H: u32 = 420;

// ANCHOR: entries
/// What the menu offers: data, with the ids the hook is handed.
const ENTRIES: [MenuEntry; 4] = [
    MenuEntry::new("Cut", 10).accel(Key::Char('x'), Mods::CTRL),
    MenuEntry::new("Copy", 20).accel(Key::Char('c'), Mods::CTRL),
    MenuEntry::new("Paste", 30).accel(Key::Char('v'), Mods::CTRL),
    MenuEntry::new("Delete", 40),
];
// ANCHOR_END: entries

/// A box the full width of the window that consumes a left release and
/// counts every time it is asked `take_menu` — the probe K90b needs: with
/// no hook installed it must read 0.
#[derive(Default)]
pub struct Area {
    asked: u32,
}

impl Widget for Area {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];
    const KIND: &'static str = "menu-area";
    const SEEDS: &'static [Seed] = &[Seed::unlinted(Self::KIND, Self::type_defaults)];
    fn type_defaults() -> Style {
        Style { width: Length::Fixed(W as f32), height: Length::Fixed(120.0), ..Style::default() }
    }
}

impl Node for Area {
    fn layout(&mut self, _cx: &mut LayoutCx, limits: Constraints, style: &[Style], st: &mut [NodeState], i: usize) -> Size {
        let size = resolve_self(limits, &style[i], Size { w: W as f32, h: 120.0 });
        record_painted(st, i, limits, size, &style[i])
    }
    fn paint(&self, _st: &[NodeState], _i: usize, _out: &mut dyn Painter) {}
    fn on_event(&mut self, ev: &Event, _cx: &EventCtx) -> bool {
        matches!(ev, Event::PointerUp { button: PointerButton::Left, .. })
    }
    fn take_menu(&mut self) -> Option<u32> {
        self.asked += 1;
        None
    }
    fulgurite::leaf_common!(Area, 0);
}

impl<S> Handlers<S> for Area {
    fn handler(&self, _sel: NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        None
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    /// Above the wrapper, so the wrapper's origin is not the window's:
    /// a menu that fell back to its parent's origin is visibly wrong.
    #[widget_id("head")]
    head: Label,
    #[widget_id("ctx")]
    ctx: ContextMenu<Area>,
}

#[derive(Default)]
struct Ran {
    /// Every id `on_menu` was handed, in order.
    ids: Vec<u32>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
    #[state]
    ran: Ran,
}

fn on_menu(r: &mut Ran, id: u32) -> KeyOutcome {
    r.ids.push(id);
    KeyOutcome::Handled
}

fn boot() -> App {
    let mut a = App::default();
    a.ui.head.set("head");
    a.ui.ctx = ContextMenu::new(Area::default(), &ENTRIES);
    a
}

// ANCHOR: program
fn program() -> Program<App> {
    application(boot).size(W, H).on_menu(on_menu)
}
// ANCHOR_END: program

type Hst<I> = Host<App, Incremental, I>;

fn index(wid: &str) -> usize {
    fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).unwrap_or_else(|| panic!("no #{wid}"))
}

/// The list's table index: the wrapper's popup is its last child but one.
fn list_index() -> usize {
    index("ctx") + <ContextMenu<Area> as Widget>::NODE_COUNT - 1
}

fn host_of<I: Invalidation>(p: Program<App>) -> Hst<I> {
    let mut h: Hst<I> = p.host();
    h.pump();
    h.pump();
    h
}

fn rect<I: Invalidation>(h: &Hst<I>, i: usize) -> [f32; 4] {
    let s = &h.states[i];
    [s.pos.x, s.pos.y, s.pos.x + s.size.w, s.pos.y + s.size.h]
}

fn right_click<I: Invalidation>(h: &mut Hst<I>, x: f32, y: f32) -> bool {
    h.post(Event::PointerMove { x, y });
    h.post(Event::PointerDown { x, y, button: PointerButton::Right, clicks: 1 });
    h.post(Event::PointerUp { x, y, button: PointerButton::Right });
    let handled = h.pump();
    // The overlay list the press opened is the one the next pump hit-tests
    // against, and that pump moves focus into it.
    h.pump();
    handled
}

fn left_click<I: Invalidation>(h: &mut Hst<I>, x: f32, y: f32) -> bool {
    h.post(Event::PointerMove { x, y });
    h.post(Event::down(x, y));
    h.post(Event::up(x, y));
    let handled = h.pump();
    h.pump();
    handled
}

/// The centre of row `k` of the open list.
fn row<I: Invalidation>(h: &Hst<I>, k: usize) -> (f32, f32) {
    let r = rect(h, list_index());
    ((r[0] + r[2]) / 2.0, r[1] + 6.0 + 30.0 * k as f32 + 15.0)
}

fn open_and_choose<I: Invalidation>(strategy: &str) {
    // Open at a point inside the wrapper, away from every edge.
    let mut h = host_of::<I>(program());
    let w = rect(&h, index("ctx"));
    let (x, y) = (w[0] + 40.0, w[1] + 30.0);
    let handled = right_click(&mut h, x, y);
    let l = rect(&h, list_index());
    println!(
        "[{strategy}] right press at ({x}, {y}) inside #ctx {w:?}: handled {handled}, open {}, list at ({}, {}) \
         (want the press), viewport {W}x{H}",
        h.app.ui.ctx.is_open(),
        l[0],
        l[1]
    );
    assert!(handled && h.app.ui.ctx.is_open(), "[{strategy}] setup: the right press opened no menu");
    assert_eq!((l[0], l[1]), (x, y), "[{strategy}] the list's top-left is not the press point");

    // Choose row 1 (Copy) by release: one hook call, with its id.
    let (rx, ry) = row(&h, 1);
    let handled = left_click(&mut h, rx, ry);
    println!(
        "[{strategy}] release on row 1 at ({rx}, {ry}): handled {handled}, on_menu {:?} (want [{}], once), open {}",
        h.app.ran.ids,
        ENTRIES[1].id,
        h.app.ui.ctx.is_open()
    );
    assert!(handled, "[{strategy}] setup: the release on the row was not handled");
    assert_eq!(h.app.ran.ids, vec![ENTRIES[1].id], "[{strategy}] on_menu did not run once with the row's id");
    assert!(!h.app.ui.ctx.is_open(), "[{strategy}] a choice left the menu open");

    // Near the right edge: clamped inside the viewport.
    let mut h = host_of::<I>(program());
    let (x, y) = (W as f32 - 10.0, w[1] + 30.0);
    right_click(&mut h, x, y);
    let l = rect(&h, list_index());
    println!(
        "[{strategy}] right press at ({x}, {y}): list [{}, {}, {}, {}] (want inside 0..{W} x 0..{H}, x = {} - {})",
        l[0],
        l[1],
        l[2],
        l[3],
        W,
        l[2] - l[0]
    );
    assert!(h.app.ui.ctx.is_open(), "[{strategy}] setup: no menu near the edge");
    assert!(l[0] >= 0.0 && l[2] <= W as f32 && l[3] <= H as f32, "[{strategy}] the list ran off the window: {l:?}");
    assert_eq!(l[2], W as f32, "[{strategy}] the clamp did not hold the list at the right edge");

    // Keyboard: the list has focus once open; Down then Enter chooses.
    let mut h = host_of::<I>(program());
    right_click(&mut h, w[0] + 40.0, w[1] + 30.0);
    let focused = h.input.focus;
    for k in [Key::Down, Key::Down, Key::Enter] {
        h.post(Event::Key { key: k, down: true });
        h.post(Event::Key { key: k, down: false });
    }
    h.pump();
    h.pump();
    println!(
        "[{strategy}] Down, Down, Enter with focus {focused:?} (the list is {}): on_menu {:?} (want [{}]), open {}",
        list_index(),
        h.app.ran.ids,
        ENTRIES[1].id,
        h.app.ui.ctx.is_open()
    );
    assert_eq!(focused, Some(list_index() as u32), "[{strategy}] the open list did not take focus");
    assert_eq!(h.app.ran.ids, vec![ENTRIES[1].id], "[{strategy}] the keyboard did not choose the second row");
}

/// **A right press opens the menu at the press, kept on the window; a
/// release on a row runs `on_menu` once with the row's id**, and so does
/// the keyboard.
// ANCHOR: open_and_choose
#[test]
fn a_right_press_opens_the_menu_there_and_a_row_runs_the_hook() {
    open_and_choose::<Everything>("Everything");
    open_and_choose::<Boundaries<256>>("Boundaries");
}
// ANCHOR_END: open_and_choose

fn disabled<I: Invalidation>(strategy: &str) {
    let mut h = host_of::<I>(program());
    h.app.ui.ctx.enabled = MenuMask::ALL.with(2, false);
    assert!(h.invalidate("ctx"), "setup: no #ctx to invalidate");
    h.pump();
    let w = rect(&h, index("ctx"));
    right_click(&mut h, w[0] + 40.0, w[1] + 30.0);
    let (rx, ry) = row(&h, 2);
    let handled = left_click(&mut h, rx, ry);
    println!(
        "[{strategy}] release on disabled row 2 (Paste): handled {handled}, on_menu {:?} (want []), open {} (want true)",
        h.app.ran.ids,
        h.app.ui.ctx.is_open()
    );
    assert!(h.app.ran.ids.is_empty(), "[{strategy}] a disabled entry ran the hook");
    assert!(h.app.ui.ctx.is_open(), "[{strategy}] a release on a disabled entry closed the menu");
    // Accept: the enabled row beside it still runs.
    let (rx, ry) = row(&h, 3);
    left_click(&mut h, rx, ry);
    println!("[{strategy}] then row 3 (Delete): on_menu {:?} (want [{}])", h.app.ran.ids, ENTRIES[3].id);
    assert_eq!(h.app.ran.ids, vec![ENTRIES[3].id], "[{strategy}] setup: the enabled row did not run");
}

/// **A disabled entry runs nothing and leaves the list open**.
#[test]
fn a_disabled_entry_runs_nothing() {
    disabled::<Everything>("Everything");
    disabled::<Boundaries<256>>("Boundaries");
}

fn no_hook<I: Invalidation>(strategy: &str) {
    // A left click on the area: consumed, and with no hook never asked.
    let mut h = host_of::<I>(application(boot).size(W, H));
    let w = rect(&h, index("ctx"));
    let handled = left_click(&mut h, w[0] + 100.0, w[1] + 100.0);
    let asked = h.app.ui.ctx.content.asked;
    println!("[{strategy}] no on_menu: click on the area handled {handled}, take_menu asked {asked} time(s) (bound 0)");
    assert!(handled, "[{strategy}] setup: the area did not consume the click");
    assert_eq!(asked, 0, "[{strategy}] take_menu was asked with no hook installed");
    // Accept: with a hook, the consuming node is asked.
    let mut h = host_of::<I>(program());
    left_click(&mut h, w[0] + 100.0, w[1] + 100.0);
    let asked = h.app.ui.ctx.content.asked;
    println!("[{strategy}] with on_menu: take_menu asked {asked} time(s) (want > 0)");
    assert!(asked > 0, "[{strategy}] setup: with a hook the consuming node was not asked");
}

/// **No hook, no cost**: an application without `on_menu` is never asked
/// `take_menu` (K90b), and one with it asks the consuming node.
#[test]
fn without_a_hook_take_menu_is_never_asked() {
    no_hook::<Everything>("Everything");
    no_hook::<Boundaries<256>>("Boundaries");
}

fn native<I: Invalidation>(strategy: &str) {
    let mut h = host_of::<I>(program());
    h.post(Event::Menu { id: 77 });
    let handled = h.pump();
    println!("[{strategy}] Event::Menu {{ id: 77 }}: handled {handled}, on_menu {:?} (want [77])", h.app.ran.ids);
    assert!(handled && h.app.ran.ids == vec![77], "[{strategy}] a native activation did not reach on_menu");
    let mut h = host_of::<I>(application(boot).size(W, H));
    h.post(Event::Menu { id: 77 });
    let handled = h.pump();
    println!("[{strategy}] the same with no hook: handled {handled} (want false)");
    assert!(!handled, "[{strategy}] a native activation with no hook was handled");
}

/// **A native activation ends in the same hook**: `Event::Menu` through
/// the one input queue.
#[test]
fn a_native_activation_reaches_the_same_hook() {
    native::<Everything>("Everything");
    native::<Boundaries<256>>("Boundaries");
}

fn area(r: [f32; 4]) -> f32 {
    (r[2] - r[0]).max(0.0) * (r[3] - r[1]).max(0.0)
}

fn meet(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[0].max(b[0]), a[1].max(b[1]), a[2].min(b[2]), a[3].min(b[3])]
}

/// How much of `r` neither `a` nor `b` covers, in px².
fn uncovered(r: [f32; 4], a: [f32; 4], b: [f32; 4]) -> f32 {
    area(r) - area(meet(r, a)) - area(meet(r, b)) + area(meet(meet(r, a), b))
}

fn opening_damage<I: Invalidation>(strategy: &str) {
    let mut h = host_of::<I>(program());
    let w = rect(&h, index("ctx"));
    // Near the wrapper's bottom, so the list hangs out of it: damage the
    // wrapper's box alone could not cover is then the list's or a fault.
    let (x, y) = (w[0] + 40.0, w[1] + 100.0);
    // Rest the pointer first, so the frame measured is the press's alone.
    h.post(Event::PointerMove { x, y });
    h.pump();
    h.pump();
    h.post(Event::PointerDown { x, y, button: PointerButton::Right, clicks: 1 });
    h.post(Event::PointerUp { x, y, button: PointerButton::Right });
    h.pump();
    let l = rect(&h, list_index());
    // The list's ink: its box and its shadow, as retention damages it.
    let ink = fulgurite::display::shadow_ink(l, h.states[list_index()].shadow);
    let regions: Vec<[f32; 4]> = match h.damage() {
        Damage::Regions(r) => r.as_slice().to_vec(),
        Damage::None => Vec::new(),
        Damage::All => vec![[0.0, 0.0, W as f32, H as f32]],
    };
    // The host's scale is 1 here, so damage's physical px are logical.
    let total: f32 = regions.iter().map(|r| area(*r)).sum();
    let outside: f32 = regions.iter().map(|r| uncovered(*r, ink, w)).sum();
    let both = area(ink) + area(w) - area(meet(ink, w));
    let ratio = total / both;
    println!(
        "[{strategy}] the opening frame: {} region(s) {regions:?}; list {l:?} ink {ink:?}, wrapper {w:?}; \
         damaged {total:.0} px² against list ink + wrapper {both:.0} px² = {ratio:.2}x (bound 1.00), outside \
         both {outside:.0} px² (bound 0)",
        regions.len()
    );
    assert!(h.app.ui.ctx.is_open(), "[{strategy}] setup: the press opened no menu");
    assert!(l[3] > w[3], "[{strategy}] setup: the list does not hang out of the wrapper ({l:?} in {w:?})");
    assert!(!regions.is_empty(), "[{strategy}] setup: the opening frame damaged nothing");
    assert!(
        outside == 0.0 && ratio <= 1.0,
        "[{strategy}] the opening frame damaged more than the list and the wrapper: {outside:.0} px² outside, {ratio:.2}x"
    );
}

/// **The frame that opens the menu damages the list and the wrapper, no
/// more** — each region inside one of the two, at most 1.00x its area
/// (M27/T27.3's per-widget bound).
#[test]
fn opening_the_menu_damages_the_list_and_the_wrapper() {
    opening_damage::<Everything>("Everything");
    opening_damage::<Boundaries<256>>("Boundaries");
}

/// `Event` stays 16 B with `Menu { id }` (K1).
#[test]
fn event_stays_sixteen_bytes() {
    let n = core::mem::size_of::<Event>();
    println!("size_of::<Event>() = {n} (bound 16)");
    assert_eq!(n, 16);
}
