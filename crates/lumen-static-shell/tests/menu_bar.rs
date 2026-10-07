//! **`MenuBar`, `Program::menu` and accelerators** — M90/T90.5
//! (`docs/plan-desktop-integration.md`, "The menu bar is drawn by the
//! framework" and "Accelerators are the framework's").
//!
//! A key press no widget consumed is offered to the program's menu before
//! `Program::on_key`, so Ctrl+S with nothing focused runs `on_menu(SAVE)`
//! and not the key hook, a text field typing `s` keeps it, and a disabled
//! Save lets Ctrl+S through to the key hook. `Program::menu` refuses a
//! 65th entry with a sentence naming the count. A press on a title opens
//! its menu, Right moves to the next title's menu, and Escape closes it
//! with focus back on the bar.
//!
//! Built with `Program::host`, driven by `post` and `pump`, under
//! `Everything` and `Boundaries<256>`. Each gate asserts its setup landed
//! and prints its quantity beside its bound.

use lumen_static::event::PointerButton;
use lumen_static::invalidate::{Boundaries, Everything, Invalidation};
use lumen_static::menu::{MenuEntry, MenuMask, MenuSection};
use lumen_static::prelude::*;
use lumen_static::retain::Incremental;
use lumen_static::{Event, Key, Mods};
use lumen_static_shell::program::{application, KeyOutcome, Program};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

const W: u32 = 480;
const H: u32 = 360;

const SAVE: u32 = 2;
const PASTE: u32 = 4;

/// The program's menu: data, declared once, read by the bar and by the
/// host's accelerators alike.
const FILE: [MenuEntry; 2] = [
    MenuEntry::new("New", 1).accel(Key::Char('n'), Mods::CTRL),
    MenuEntry::new("Save", SAVE).accel(Key::Char('s'), Mods::CTRL),
];
const EDIT: [MenuEntry; 2] = [
    MenuEntry::new("Copy", 3).accel(Key::Char('c'), Mods::CTRL),
    MenuEntry::new("Paste", PASTE).accel(Key::Char('v'), Mods::CTRL),
];
const VIEW: [MenuEntry; 1] = [MenuEntry::new("Zoom", 5)];
const SECTIONS: [MenuSection; 3] = [
    MenuSection { title: "File", entries: &FILE },
    MenuSection { title: "Edit", entries: &EDIT },
    MenuSection { title: "View", entries: &VIEW },
];

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct Ui {
    #[widget_id("bar")]
    bar: MenuBar,
    #[widget_id("field")]
    field: TextInput<String>,
}

struct Ran {
    /// Every id `on_menu` was handed, in order.
    ids: Vec<u32>,
    /// Every key `on_key` was handed.
    keys: Vec<(Key, Mods)>,
    /// Whether Save (flattened entry 1) is enabled.
    save: bool,
}

impl Default for Ran {
    fn default() -> Self {
        Ran { ids: Vec::new(), keys: Vec::new(), save: true }
    }
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

fn on_key(r: &mut Ran, key: Key, mods: Mods) -> KeyOutcome {
    r.keys.push((key, mods));
    KeyOutcome::Handled
}

// ANCHOR: enabled
/// Which entries are enabled, from the state: Save is entry 1.
fn enabled(r: &Ran) -> MenuMask {
    MenuMask::ALL.with(1, r.save)
}
// ANCHOR_END: enabled

fn boot() -> App {
    let mut a = App::default();
    a.ui.bar = MenuBar::new(&SECTIONS);
    a
}

// ANCHOR: program
fn program() -> Program<App> {
    application(boot).size(W, H).menu(&SECTIONS, enabled).on_menu(on_menu).on_key(on_key)
}
// ANCHOR_END: program

type Hst<I> = Host<App, Incremental, I>;

fn index(wid: &str) -> usize {
    lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).unwrap_or_else(|| panic!("no #{wid}"))
}

fn chord<I: Invalidation>(h: &mut Hst<I>, key: Key, mods: Mods) -> bool {
    h.post(Event::Modifiers(mods));
    h.post(Event::Key { key, down: true });
    h.post(Event::Key { key, down: false });
    h.post(Event::Modifiers(Mods::NONE));
    h.pump()
}

fn click<I: Invalidation>(h: &mut Hst<I>, x: f32, y: f32) -> bool {
    h.post(Event::PointerDown { x, y, button: PointerButton::Left, clicks: 1 });
    h.post(Event::up(x, y));
    h.pump()
}

/// Title `k`'s centre, in window coordinates.
fn title_at<I: Invalidation>(h: &Hst<I>, k: usize) -> (f32, f32) {
    let s = h.states[index("bar")];
    let (a, b) = h.app.ui.bar.title_span(k).expect("a laid-out title");
    (s.pos.x + (a + b) / 2.0, s.pos.y + s.size.h / 2.0)
}

// ---- the accelerators ---------------------------------------------------

fn accelerators<I: Invalidation>(name: &str) {
    let mut h: Hst<I> = program().host();
    h.pump();
    assert_eq!(h.input.focus, None, "setup: something is focused");

    // ANCHOR: accel_gate
    let handled = chord(&mut h, Key::Char('s'), Mods::CTRL);
    let (menu, keys) = (h.app.ran.ids.clone(), h.app.ran.keys.len());
    println!("[{name}] Ctrl+S, nothing focused: handled {handled}, on_menu {menu:?} (want [{SAVE}]), on_key {keys} (want 0)");
    assert!(handled, "Ctrl+S was not handled");
    assert_eq!(menu, vec![SAVE], "Ctrl+S did not run on_menu(SAVE) once");
    assert_eq!(keys, 0, "Ctrl+S reached on_key as well: one key, handled twice");
    // ANCHOR_END: accel_gate

    // A disabled Save lets Ctrl+S through to the key hook.
    h.app.ran.save = false;
    let handled = chord(&mut h, Key::Char('s'), Mods::CTRL);
    let (menu, keys) = (h.app.ran.ids.clone(), h.app.ran.keys.clone());
    println!("[{name}] Ctrl+S, Save disabled: handled {handled}, on_menu {menu:?} (want [{SAVE}]), on_key {keys:?} (want one Ctrl+S)");
    assert_eq!(menu, vec![SAVE], "a disabled Save ran on_menu");
    assert_eq!(keys, vec![(Key::Char('s'), Mods::CTRL)], "a disabled Save's Ctrl+S did not reach on_key once");
    h.app.ran.save = true;
    h.app.ran.keys.clear();

    // Accept: a focused text field keeps what it types.
    let f = h.states[index("field")];
    click(&mut h, f.pos.x + 8.0, f.pos.y + f.size.h / 2.0);
    assert_eq!(h.input.focus, Some(index("field") as u32), "setup: the field did not take focus");
    h.post(Event::Key { key: Key::Char('s'), down: true });
    h.post(Event::Text { ch: 's' });
    h.post(Event::Key { key: Key::Char('s'), down: false });
    h.pump();
    let (text, menu, keys) = (h.app.ui.field.as_str().to_owned(), h.app.ran.ids.len(), h.app.ran.keys.len());
    println!("[{name}] `s` into the focused field: text {text:?} (want \"s\"), on_menu {menu} (want 1, unchanged), on_key {keys} (want 0)");
    assert_eq!(text, "s", "the field did not type `s`");
    assert_eq!(menu, 1, "typing `s` ran a menu entry");
    assert_eq!(keys, 0, "typing `s` into a field reached on_key");
}

#[test]
fn a_key_nobody_took_goes_to_the_menu_before_on_key_everything() {
    accelerators::<Everything>("Everything");
}

#[test]
fn a_key_nobody_took_goes_to_the_menu_before_on_key_boundaries() {
    accelerators::<Boundaries<256>>("Boundaries<256>");
}

// ---- the 64-entry limit -------------------------------------------------

const MANY: [MenuEntry; 65] = [MenuEntry::new("x", 0); 65];
const SIXTY_FIVE: [MenuSection; 2] =
    [MenuSection { title: "A", entries: &MANY_A }, MenuSection { title: "B", entries: &MANY_B }];
const MANY_A: [MenuEntry; 40] = [MenuEntry::new("a", 0); 40];
const MANY_B: [MenuEntry; 25] = [MenuEntry::new("b", 0); 25];
const SIXTY_FOUR: [MenuSection; 1] = [MenuSection { title: "A", entries: MANY.split_at(64).0 }];

#[test]
fn a_menu_of_65_entries_is_refused_by_name_and_64_is_accepted() {
    let refused = std::panic::catch_unwind(|| {
        let _ = application(boot).menu(&SIXTY_FIVE, enabled);
    });
    let msg = refused
        .as_ref()
        .err()
        .and_then(|p| p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())));
    println!("65 entries over 2 sections: {msg:?}");
    let msg = msg.expect("Program::menu accepted 65 entries");
    assert!(msg.contains("at most 64") && msg.contains("this one has 65"), "the refusal does not name the limit and the count: {msg}");
    let accepted = std::panic::catch_unwind(|| {
        let _ = application(boot).menu(&SIXTY_FOUR, enabled);
    });
    println!("64 entries: accepted {}", accepted.is_ok());
    assert!(accepted.is_ok(), "Program::menu refused 64 entries");
}

// ---- the bar ------------------------------------------------------------

fn bar<I: Invalidation>(name: &str) {
    let mut h: Hst<I> = program().host();
    h.pump();
    let bar = index("bar");
    let list = bar + 2;

    // ANCHOR: bar_gate
    let (x, y) = title_at(&h, 0);
    let handled = click(&mut h, x, y);
    h.pump();
    let open = h.app.ui.bar.open_title();
    let ls = h.states[list];
    println!(
        "[{name}] click on title 0 at ({x:.0}, {y:.0}): handled {handled}, open {open:?} (want Some(0)), list {:?} at ({:.0}, {:.0}), focus {:?} (list {list})",
        h.app.ui.bar.menu.content.entries().iter().map(|e| e.label).collect::<Vec<_>>(),
        ls.pos.x,
        ls.pos.y,
        h.input.focus
    );
    assert!(handled, "the click on a title was not handled");
    assert_eq!(open, Some(0), "a click on title 0 did not open its menu");
    assert!(ls.size.h > 0.0, "the open menu has no area");

    h.post(Event::Key { key: Key::Right, down: true });
    let handled = h.pump();
    h.pump();
    let open = h.app.ui.bar.open_title();
    // Which menu is showing is what the list holds and where it hangs —
    // not only the bar's roving title, which a Right that only moved the
    // title would also have advanced.
    let shown: Vec<_> = h.app.ui.bar.menu.content.entries().iter().map(|e| e.label).collect();
    let (lx, tx) = (h.states[list].pos.x, h.states[bar].pos.x + h.app.ui.bar.title_span(1).unwrap().0);
    println!(
        "[{name}] Right with the menu open: handled {handled}, open {open:?} (want Some(1)), entries {shown:?} (want [\"Copy\", \"Paste\"]), list at x {lx:.0} (title 1 at {tx:.0})"
    );
    assert!(handled, "Right was not handled");
    assert_eq!(open, Some(1), "Right did not move to the next title's menu");
    assert_eq!(shown, ["Copy", "Paste"], "Right moved the title but the menu still shows another section");
    assert!((lx - tx).abs() < 0.5, "the menu does not hang below title 1: list x {lx}, title x {tx}");

    h.post(Event::Key { key: Key::Escape, down: true });
    let handled = h.pump();
    h.pump();
    let open = h.app.ui.bar.open_title();
    println!("[{name}] Escape: handled {handled}, open {open:?} (want None), focus {:?} (want the bar, {bar})", h.input.focus);
    assert!(handled, "Escape was not handled");
    assert_eq!(open, None, "Escape did not close the menu");
    assert_eq!(h.input.focus, Some(bar as u32), "focus did not return to the bar");
    // ANCHOR_END: bar_gate

    // With a menu open, hovering another title switches to its menu.
    let (x0, y0) = title_at(&h, 0);
    click(&mut h, x0, y0);
    h.pump();
    let (x2, y2) = title_at(&h, 2);
    h.post(Event::PointerMove { x: x2, y: y2 });
    h.pump();
    let open = h.app.ui.bar.open_title();
    println!("[{name}] hover title 2 with title 0 open: open {open:?} (want Some(2))");
    assert_eq!(open, Some(2), "hovering another title did not switch menus");

    // And a release on a row chooses it, by id, through on_menu.
    let (x1, y1) = title_at(&h, 0);
    h.post(Event::Key { key: Key::Escape, down: true });
    h.pump();
    click(&mut h, x1, y1);
    h.pump();
    let ls = h.states[list];
    let row1 = (ls.pos.x + 40.0, ls.pos.y + 6.0 + 30.0 * 1.5);
    let before = h.app.ran.ids.len();
    let handled = click(&mut h, row1.0, row1.1);
    h.pump();
    let got = h.app.ran.ids[before..].to_vec();
    println!("[{name}] release on File > Save: handled {handled}, on_menu {got:?} (want [{SAVE}]), open {:?}", h.app.ui.bar.open_title());
    assert_eq!(got, vec![SAVE], "a release on Save did not run on_menu(SAVE) once");
    assert_eq!(h.app.ui.bar.open_title(), None, "a choice did not close the menu");

    // The keyboard route: the focused bar, Down opens with the first row
    // highlighted, Enter chooses it.
    assert_eq!(h.input.focus, Some(bar as u32), "setup: the bar is not focused after the choice");
    let before = h.app.ran.ids.len();
    h.post(Event::Key { key: Key::Down, down: true });
    h.pump();
    h.pump();
    let opened = h.app.ui.bar.open_title();
    h.post(Event::Key { key: Key::Enter, down: true });
    h.pump();
    h.pump();
    let got = h.app.ran.ids[before..].to_vec();
    println!("[{name}] Down then Enter on the bar: opened {opened:?} (want Some(0)), on_menu {got:?} (want [1], New)");
    assert_eq!(opened, Some(0), "Down on the focused bar did not open its title's menu");
    assert_eq!(got, vec![1], "Enter did not choose the first row");
}

#[test]
fn a_title_opens_its_menu_right_moves_and_escape_returns_focus_everything() {
    bar::<Everything>("Everything");
}

#[test]
fn a_title_opens_its_menu_right_moves_and_escape_returns_focus_boundaries() {
    bar::<Boundaries<256>>("Boundaries<256>");
}
