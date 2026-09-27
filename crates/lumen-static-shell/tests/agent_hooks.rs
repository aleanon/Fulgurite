//! **The program hooks through the agent** — M60/T60.1 and T60.2
//! (`docs/plan-tally-slim.md` §0 rule 1).
//!
//! The same program `tests/program_hooks.rs` builds, driven the way a
//! window's agent drives it: `Local::from_program`, requests through
//! `Session::agent`, frames presented into an `Offscreen` — which is what
//! `ui.screenshot` reads back. An `input.key` must reach the key hook,
//! and an `input.click` that flips the sheet's key must be recoloured in
//! the frame that request presented.

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

fn program() -> Program<Keys> {
    application(|| {
        let mut k = Keys::default();
        k.ui.flip.label = "flip";
        k.ui.flip.on_click = |c: &mut Count, _: &EventCtx| c.n -= 20;
        k
    })
    .size(W, H)
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
#[ignore = "red until T60.1: Host::pump_with never calls the key hook"]
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
#[ignore = "red until T60.2: Host::pump_with never compares the sheet key"]
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
