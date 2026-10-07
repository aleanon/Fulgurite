//! **The program hooks through the agent** — M60/T60.1 and T60.2
//! (`docs/plan-tally-slim.md` §0 rule 1).
//!
//! The same program `tests/program_hooks.rs` builds, driven the way a
//! window's agent drives it: `Local::from_program`, requests through
//! `Session::agent`, frames presented into an `Offscreen` — which is what
//! `ui.screenshot` reads back. An `input.key` must reach the key hook,
//! and an `input.click` that flips the sheet's key must be recoloured in
//! the frame that request presented. Since M90/T90.5 an `input.key` chord
//! answers `handled` for the key alone, so an accelerator that fired and
//! one no entry answers read differently.

use lumen_static::display::Color;
use lumen_static::event::EventCtx;
use lumen_static::prelude::*;
use lumen_static::style::{Patch, Rule, Selector};
use lumen_static::{Key, Mods};
use lumen_static_shell::program::{application, KeyOutcome, Program};
use lumen_static_shell::session::{Crashing, Local, Offscreen, Screen, Session};
use lumen_static_widgets::prelude::*;
use serde_json::Value;

const W: u32 = 320;
const H: u32 = 200;
const INK: Color = Color(0x20, 0x20, 0x20, 0xff);
const NEG: Color = Color(0xfb, 0x71, 0x85, 0xff);

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct Ui {
    #[widget_id("value")]
    value: Label<String>,
    #[widget_id("flip")]
    flip: Button<Count>,
}

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

/// One entry answering Ctrl+K (M90/T90.5).
const TOOLS: [lumen_static::menu::MenuEntry; 1] = [lumen_static::menu::MenuEntry::new("Bump", 7).accel(Key::Char('k'), Mods::CTRL)];
const MENU: [lumen_static::menu::MenuSection; 1] = [lumen_static::menu::MenuSection { title: "Tools", entries: &TOOLS }];

fn program() -> Program<Keys> {
    application(|| {
        let mut k = Keys::default();
        k.ui.flip.label = "flip";
        k.ui.flip.on_click = |c: &mut Count, _: &EventCtx| c.n -= 20;
        k
    })
    .size(W, H)
    .menu(&MENU, |_: &Count| lumen_static::menu::MenuMask::ALL)
    .on_menu(|c: &mut Count, _: u32| {
        c.n += 100;
        KeyOutcome::Handled
    })
    .on_key(|c: &mut Count, key: Key, _: Mods| {
        c.calls += 1;
        if key == Key::Up {
            c.n += 10;
            KeyOutcome::Handled
        } else {
            KeyOutcome::Ignored
        }
    })
    .style_by(
        |k: &Keys| k.count.n.signum(),
        |sign| vec![Rule::new(Selector::Id("value"), Patch::new().color(if sign < 0 { NEG } else { INK }))],
    )
}

fn ask(s: &mut Local<Keys, Crashing>, screen: &mut Offscreen, method: &str, params: &str) -> Value {
    let req = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"{method}","params":{params}}}"#);
    let r: Value = serde_json::from_str(&s.agent(&req, screen)).expect("the reply is JSON");
    assert!(r.get("error").is_none(), "{method} failed: {r}");
    r["result"].clone()
}

/// Pixels exactly `c` inside the node `sel` resolves to, on the screen.
fn pixels_of(s: &mut Local<Keys, Crashing>, screen: &mut Offscreen, sel: &str, c: Color) -> usize {
    let l = ask(s, screen, "ui.getLayout", &format!(r#"{{"selector":"{sel}"}}"#));
    let f = |v: &Value| v.as_f64().expect("a number") as u32;
    let (x0, y0) = (f(&l["pos"][0]), f(&l["pos"][1]));
    let (x1, y1) = ((x0 + f(&l["size"][0])).min(W), (y0 + f(&l["size"][1])).min(H));
    let (w, _, px) = screen.frame_rgba_owned();
    let mut n = 0;
    for y in y0..y1 {
        for x in x0..x1 {
            let i = ((y * w + x) * 4) as usize;
            if px[i..i + 3] == [c.0, c.1, c.2] {
                n += 1;
            }
        }
    }
    n
}

trait Owned {
    fn frame_rgba_owned(&self) -> (u32, u32, Vec<u8>);
}
impl Owned for Offscreen {
    fn frame_rgba_owned(&self) -> (u32, u32, Vec<u8>) {
        (self.canvas.width, self.canvas.height, self.canvas.pixels.clone())
    }
}

#[test]
fn input_key_reaches_the_key_hook() {
    let mut s = Local::from_program(program(), Crashing);
    let mut screen = Offscreen::new(W, H);
    s.frame(0.0, W, H);
    let r = ask(&mut s, &mut screen, "input.key", r#"{"key":"Up"}"#);
    println!(
        "input.key Up: reply handled {}, hook calls {} (want 1), count {} (want 10)",
        r["handled"], s.host.app.count.calls, s.host.app.count.n
    );
    assert_eq!(s.host.app.count.calls, 1, "the agent's input.key never reached the key hook");
    assert_eq!(s.host.app.count.n, 10);
    assert_eq!(r["handled"], true, "the reply must say the hook handled the key");
    let v = ask(&mut s, &mut screen, "ui.resolve", r##"{"selector":"#value"}"##);
    println!("#value after the key: {}", v["value"]);
    assert_eq!(v["value"], "10", "the agent reads a stale value after the hook's change");
}

#[test]
fn input_click_that_flips_the_key_is_recoloured_in_its_own_frame() {
    let mut s = Local::from_program(program(), Crashing);
    let mut screen = Offscreen::new(W, H);
    // The first frame, as the window's redraw draws it, so the screen
    // holds the frame before the click.
    s.frame(0.0, W, H);
    screen.present(&mut s, Color(255, 255, 255, 255));
    let before = pixels_of(&mut s, &mut screen, "#value", NEG);
    let r = ask(&mut s, &mut screen, "input.click", r##"{"selector":"#flip"}"##);
    let after = pixels_of(&mut s, &mut screen, "#value", NEG);
    println!(
        "input.click #flip: handled {}, count {} (want -20); #value in NEG {before} px before (want 0), \
         {after} px in the frame the click presented (bound > 0)",
        r["handled"], s.host.app.count.n
    );
    assert_eq!(r["handled"], true, "setup: the click did not land");
    assert_eq!(s.host.app.count.n, -20, "setup: the click did not reach its handler");
    assert_eq!(before, 0, "setup: the value was already negative-coloured");
    assert!(after > 0, "the agent's click flipped the sign and its frame kept the old sheet");
}

#[test]
fn a_chord_answers_handled_for_its_key_alone() {
    let mut s = Local::from_program(program(), Crashing);
    let mut screen = Offscreen::new(W, H);
    s.frame(0.0, W, H);
    let k = ask(&mut s, &mut screen, "input.key", r#"{"key":"k","mods":["ctrl"]}"#);
    let after_k = s.host.app.count.n;
    let j = ask(&mut s, &mut screen, "input.key", r#"{"key":"j","mods":["ctrl"]}"#);
    let after_j = s.host.app.count.n;
    println!(
        "input.key Ctrl+K: handled {} (want true), count {after_k} (want 100); Ctrl+J: handled {} (want false), \
         count {after_j} (want 100), key hook calls {} (want 1, the J)",
        k["handled"], j["handled"], s.host.app.count.calls
    );
    assert_eq!(after_k, 100, "setup: Ctrl+K did not reach the accelerator");
    assert_eq!(k["handled"], true, "the accelerator's chord answered handled false");
    assert_eq!(after_j, 100, "Ctrl+J ran a menu entry");
    assert_eq!(j["handled"], false, "a chord nothing answered reported handled: the modifier event's answer leaked into the key's");
}
