//! **The two program hooks M60 adds** — `Program::on_key` (T60.1) and
//! `Program::style_by` (T60.2), `docs/plan-tally-slim.md` §2.
//!
//! Every gate here builds its host with `Program::host`, the function the
//! window's session boots through, and drives it with `post` and `pump` —
//! the queue and the pump the window and the agent use (rule 1). Each one
//! asserts that its action *landed* (the hook's own call count, `pump`'s
//! `handled`, a click reaching its handler) before it compares a frame,
//! and prints the quantity it bounds.

use lumen_static::display::Color;
use lumen_static::event::EventCtx;
use lumen_static::invalidate::{Boundaries, Everything, Invalidation};
use lumen_static::prelude::*;
use lumen_static::retain::Incremental;
use lumen_static::style::{Patch, Rule, Selector};
use lumen_static::{Event, Key, Mods};
use lumen_static_shell::program::{application, KeyOutcome, Program};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

const W: u32 = 320;
const H: u32 = 200;
const PAGE: Color = Color(255, 255, 255, 255);
const INK: Color = Color(0x20, 0x20, 0x20, 0xff);
const NEG: Color = Color(0xfb, 0x71, 0x85, 0xff);

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct Ui {
    #[widget_id("value")]
    value: Label<String>,
    #[widget_id("field")]
    field: TextInput,
    #[widget_id("flip")]
    flip: Button<Count>,
}

/// The state: the count, and how many times the key hook was called.
#[derive(Default)]
struct Count {
    n: i64,
    calls: u32,
}

#[derive(DeriveApp, Default)]
struct Keys {
    #[tree]
    ui: Ui,
    #[state]
    #[bind(ui.value, n, "{}")]
    count: Count,
}

/// Up adds ten — so "0" becomes "10" and the label's box grows, which
/// the block glyphs of a font-less canvas can show — Down takes ten,
/// Escape quits, anything else is not the application's.
fn keys(c: &mut Count, key: Key, _mods: Mods) -> KeyOutcome {
    c.calls += 1;
    match key {
        Key::Up => {
            c.n += 10;
            KeyOutcome::Handled
        }
        Key::Down => {
            c.n -= 10;
            KeyOutcome::Handled
        }
        Key::Escape => KeyOutcome::Quit,
        _ => KeyOutcome::Ignored,
    }
}

/// The value is drawn in `NEG` while the count is negative.
fn sheet(sign: i64) -> Vec<Rule> {
    vec![Rule::new(Selector::Id("value"), Patch::new().color(if sign < 0 { NEG } else { INK }))]
}

fn boot() -> Keys {
    let mut k = Keys::default();
    k.ui.flip.label = "flip";
    k.ui.flip.on_click = |c: &mut Count, _: &EventCtx| c.n -= 20;
    k
}

fn program() -> Program<Keys> {
    application(boot).size(W, H).on_key(keys).style_by(|k: &Keys| k.count.n.signum(), sheet)
}

fn frame<A: lumen_static::event::App, I: Invalidation>(h: &Host<A, Incremental, I>) -> Vec<u8> {
    let mut c = Canvas::new(W, H);
    c.clear(PAGE);
    h.render(&mut c);
    c.pixels
}

fn differing(a: &[u8], b: &[u8]) -> usize {
    a.chunks(4).zip(b.chunks(4)).filter(|(x, y)| x != y).count()
}

fn key(h: &mut Host<Keys, Incremental, impl Invalidation>, k: Key) {
    h.post(Event::Key { key: k, down: true });
    h.post(Event::Key { key: k, down: false });
}

fn index(wid: &str) -> usize {
    lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).unwrap_or_else(|| panic!("no #{wid}"))
}

fn centre<A: lumen_static::event::App, I: Invalidation>(h: &Host<A, Incremental, I>, wid: &str) -> (f32, f32) {
    let s = &h.states[index(wid)];
    (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
}

/// **An unconsumed key reaches the hook, and its change is drawn under
/// `Boundaries`** — the default strategy, compared pixel for pixel with
/// the `Everything` diagnostic (§0 rule 2).
#[test]
#[ignore = "red until T60.1: Host::pump_with never calls the key hook"]
fn an_unconsumed_key_reaches_the_hook_and_is_painted_under_boundaries() {
    let mut b: Host<Keys, Incremental, Boundaries<256>> = program().host();
    let mut e: Host<Keys, Incremental, Everything> = program().host();
    b.pump();
    e.pump();
    let before = frame(&b);

    key(&mut b, Key::Up);
    key(&mut e, Key::Up);
    let handled = b.pump();
    e.pump();
    println!(
        "Up: hook calls {} (want 1: the release is not a press), count {} (want 10), pump handled {handled}",
        b.app.count.calls, b.app.count.n
    );
    assert_eq!(b.app.count.calls, 1, "the hook never ran for an unconsumed Up, or ran for its release too");
    assert_eq!(b.app.count.n, 10, "the hook's change did not land");
    assert!(handled, "a key the hook handled must count as handled");
    assert_eq!(b.app.ui.value.as_str(), "10", "the binding did not present the hook's change");

    let (after_b, after_e) = (frame(&b), frame(&e));
    let moved = differing(&before, &after_b);
    let stale = differing(&after_b, &after_e);
    println!("the key's frame: {moved} px changed (bound > 0); Boundaries vs Everything differ by {stale} px (bound 0)");
    assert!(moved > 0, "the frame after the key is the frame before it");
    assert_eq!(stale, 0, "Boundaries drew a stale frame for a key the hook handled");
}

/// **A key a focused widget owns never reaches the hook** — navigation
/// and editing keys in a `TextInput`, which owns its arrows (M44/T44.1),
/// and Tab, which dispatch consumes to move focus.
/// And the accept direction, in the same session: with nothing focused
/// the same Up does reach it.
#[test]
#[ignore = "red until T60.1: Host::pump_with never calls the key hook"]
fn a_key_a_focused_widget_consumes_never_reaches_the_hook() {
    let mut h: Host<Keys, Incremental, Boundaries<256>> = program().host();
    h.pump();
    let (x, y) = centre(&h, "field");
    h.click(x, y);
    h.pump();
    let field = index("field") as u32;
    println!("setup: focus after clicking #field is {:?} (want Some({field}))", h.input.focus);
    assert_eq!(h.input.focus, Some(field), "setup: the click did not focus the field");

    // The field is empty, so each of these is a no-op at its edge and
    // dispatch reports it unconsumed (`handled` false) — which is the
    // case the rule exists for: a field owns its navigation and editing
    // keys whether or not it moved, or an arrow at the end of a field
    // would nudge the application (§2.1).
    for k in [Key::Left, Key::Right, Key::Up, Key::Down, Key::Home, Key::End, Key::Backspace, Key::Delete] {
        key(&mut h, k);
        let handled = h.pump();
        println!("{k:?} into the focused field: handled {handled}, hook calls {}", h.app.count.calls);
        assert_eq!(h.input.focus, Some(field), "setup: {k:?} moved focus off the field");
        assert_eq!(h.app.count.calls, 0, "{k:?} belongs to the focused field and still reached the hook");
    }
    key(&mut h, Key::Tab);
    h.pump();
    println!("Tab: focus {:?}, hook calls {}", h.input.focus, h.app.count.calls);
    assert_ne!(h.input.focus, Some(field), "setup: Tab did not move focus");
    assert_eq!(h.app.count.calls, 0, "Tab moved focus and still reached the hook");

    // The accept direction: nothing focused, and Up is the application's.
    h.input.focus = None;
    key(&mut h, Key::Up);
    h.pump();
    println!("Up with nothing focused: hook calls {} (want 1), count {} (want 10)", h.app.count.calls, h.app.count.n);
    assert_eq!(h.app.count.calls, 1, "an unconsumed Up did not reach the hook");
    assert_eq!(h.app.count.n, 10);
}

/// `Quit` asks to close and `Ignored` is not handled.
#[test]
#[ignore = "red until T60.1: Host::pump_with never calls the key hook"]
fn quit_is_recorded_and_ignored_is_not_handled() {
    let mut h: Host<Keys, Incremental, Boundaries<256>> = program().host();
    h.pump();
    key(&mut h, Key::Char('x'));
    let handled = h.pump();
    println!("x: hook calls {}, handled {handled} (want false), quit {}", h.app.count.calls, h.quit_requested());
    assert_eq!(h.app.count.calls, 1, "the hook never saw an unconsumed x");
    assert!(!handled, "an Ignored key counted as handled");
    assert!(!h.quit_requested());
    key(&mut h, Key::Escape);
    h.pump();
    println!("Escape: hook calls {}, quit {} (want true)", h.app.count.calls, h.quit_requested());
    assert_eq!(h.app.count.calls, 2, "the hook never saw an unconsumed Escape");
    assert!(h.quit_requested(), "KeyOutcome::Quit was not recorded");
}

// ---- an unsplit application ----------------------------------------------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct LooseUi {
    #[widget_id("note")]
    note: Label,
    #[widget_id("other")]
    other: Label,
}

#[derive(DeriveApp, Default)]
struct Loose {
    #[tree]
    ui: LooseUi,
}

/// **An unsplit application's hook may write the tree**, with no binding
/// to mark it — so `Handled` must mark everything, or `Boundaries` skips
/// the note and draws last frame's (§2.1).
#[test]
#[ignore = "red until T60.1: Host::pump_with never calls the key hook"]
fn an_unsplit_hook_that_writes_the_tree_is_painted_under_boundaries() {
    fn program() -> Program<Loose> {
        application(|| {
            let mut l = Loose::default();
            l.ui.other.set("other");
            l
        })
        .size(W, H)
        .on_key(|l: &mut Loose, k: Key, _: Mods| {
            if k != Key::Up {
                return KeyOutcome::Ignored;
            }
            l.ui.note.set("a long note the hook wrote");
            KeyOutcome::Handled
        })
    }
    let mut b: Host<Loose, Incremental, Boundaries<256>> = program().host();
    let mut e: Host<Loose, Incremental, Everything> = program().host();
    b.pump();
    e.pump();
    let before = frame(&b);
    for h in [&mut b as &mut dyn FnPost, &mut e] {
        h.up();
    }
    let handled = b.pump();
    e.pump();
    let (after_b, after_e) = (frame(&b), frame(&e));
    let (moved_e, stale) = (differing(&before, &after_e), differing(&after_b, &after_e));
    println!(
        "Up: note {:?}, handled {handled}; Everything's frame moved {moved_e} px (bound > 0), Boundaries differs by {stale} px (bound 0)",
        b.app.ui.note.as_str()
    );
    assert_eq!(b.app.ui.note.as_str(), "a long note the hook wrote", "the hook never ran");
    assert!(moved_e > 0, "setup: the note's change is not visible even under Everything");
    assert_eq!(stale, 0, "Boundaries drew a stale frame for an unsplit hook's write");
}

/// Posting Up to hosts of two strategies through one loop.
trait FnPost {
    fn up(&mut self);
}
impl<I: Invalidation> FnPost for Host<Loose, Incremental, I> {
    fn up(&mut self) {
        self.post(Event::Key { key: Key::Up, down: true });
    }
}

// ---- style keyed on state -------------------------------------------------

/// Pixels exactly `c` inside `wid`'s box.
fn pixels_of<A: lumen_static::event::App, I: Invalidation>(h: &Host<A, Incremental, I>, px: &[u8], wid: &str, c: Color) -> usize {
    let s = &h.states[index(wid)];
    let (x0, y0) = (s.pos.x.max(0.0) as u32, s.pos.y.max(0.0) as u32);
    let (x1, y1) = (((s.pos.x + s.size.w) as u32).min(W), ((s.pos.y + s.size.h) as u32).min(H));
    let mut n = 0;
    for y in y0..y1 {
        for x in x0..x1 {
            let i = ((y * W + x) * 4) as usize;
            if px[i..i + 3] == [c.0, c.1, c.2] {
                n += 1;
            }
        }
    }
    n
}

/// **A click that flips the key is drawn in the new sheet on the frame
/// the click lands** — one pump, no second frame to catch up (§2.2).
#[test]
#[ignore = "red until T60.2: Host::pump_with never compares the sheet key"]
fn a_click_that_flips_the_key_is_drawn_in_the_new_sheet_on_its_own_frame() {
    let mut b: Host<Keys, Incremental, Boundaries<256>> = program().host();
    let mut e: Host<Keys, Incremental, Everything> = program().host();
    b.pump();
    e.pump();
    let neg_before = pixels_of(&b, &frame(&b), "value", NEG);
    let (x, y) = centre(&b, "flip");
    b.click(x, y);
    e.click(x, y);
    let handled = b.pump();
    e.pump();
    let (fb, fe) = (frame(&b), frame(&e));
    let neg_after = pixels_of(&b, &fb, "value", NEG);
    let stale = differing(&fb, &fe);
    println!(
        "flip: handled {handled}, count {} (want -20); #value in NEG: {neg_before} px before (want 0), \
         {neg_after} px on the click's frame (bound > 0); Boundaries vs Everything {stale} px (bound 0)",
        b.app.count.n
    );
    assert!(handled && b.app.count.n == -20, "setup: the click did not reach its handler");
    assert_eq!(neg_before, 0, "setup: the value was already negative-coloured");
    assert!(neg_after > 0, "the sign flipped and the value was not drawn in the negative sheet on that frame");
    assert_eq!(stale, 0, "Boundaries drew the re-styled frame differently from Everything");

    // And back: Up twice crosses zero, and the sheet follows again.
    for _ in 0..3 {
        key(&mut b, Key::Up);
    }
    b.pump();
    let fb = frame(&b);
    println!("three Ups: count {} (want 10); #value in NEG {} px (want 0)", b.app.count.n, pixels_of(&b, &fb, "value", NEG));
    assert_eq!(b.app.count.n, 10);
    assert_eq!(pixels_of(&b, &fb, "value", NEG), 0, "the value kept the negative sheet after the count went positive");
}
