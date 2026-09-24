//! A headless window loader: swap `fixtures/reload-app` from v1 to v2
//! and check the swapped session against v2 run from the start —
//! M37/T37.1 (`docs/plan-window-reload.md`).
//!
//! `reload-host <v1.so> <v2.so>`. The exit code is the verdict.
//!
//! **What is compared.** v1 is clicked once, saved, dropped; v2 boots
//! its own program with those bytes. A second v2 session boots with
//! nothing and is clicked once. The two frames must agree command for
//! command and byte for byte: the restore produced exactly the state the
//! input would have, under the new code.

use std::path::Path;

use lumen_static::display::Cmd;
use lumen_static::Event;
use lumen_static_reload::hot::{Build, Hot};
use lumen_static_shell::session::{FrameSource, Session};

const W: u32 = 240;
const H: u32 = 120;
/// The button fills the window's width and the top 60 px.
const AT: (f32, f32) = (120.0, 30.0);

fn text(s: &Hot) -> String {
    let l = s.list();
    let mut out = Vec::new();
    for c in &l.cmds {
        if let Cmd::Text { text, .. } = c {
            out.push(l.str_of(*text).to_string());
        }
    }
    out.join(" | ")
}

fn click(s: &mut Hot) {
    s.post(Event::down(AT.0, AT.1));
    s.post(Event::up(AT.0, AT.1));
    s.frame(16.0, W, H);
}

fn open(p: &str) -> Build {
    match Build::open(Path::new(p)) {
        Ok(b) => b,
        Err(e) => panic!("{p} refused: {e}"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let v1 = open(&args[0]);
    println!("stamp: {} agreed", args[0]);

    let mut a = v1.boot(None).expect("v1 boots");
    a.frame(0.0, W, H);
    let before = text(&a);
    println!("v1 fresh:   {before}");
    assert!(before.contains("count 0"), "v1's first frame: `{before}`");
    click(&mut a);
    let clicked = text(&a);
    println!("v1 clicked: {clicked}");
    // The setup must happen: a click that missed leaves `count 0`, and
    // everything after would compare two sessions nobody touched.
    assert!(clicked.contains("count 1"), "the click did not land: `{clicked}`");

    let bytes = a.save();
    println!("saved {} bytes: {}", bytes.len(), String::from_utf8_lossy(&bytes));
    drop(a); // dropped inside v1, which stays loaded (§2.5)

    let v2 = open(&args[1]);
    let mut b = v2.boot(Some(&bytes)).expect("v2 boots with v1's state");
    b.frame(0.0, W, H);
    let swapped = text(&b);

    let mut c = v2.boot(None).expect("v2 boots fresh");
    c.frame(0.0, W, H);
    click(&mut c);
    // The pointer is still over the button, which is hover-shaded; the
    // swapped session never had a pointer. Hover is not kept state (the
    // first run of this gate compared the two and found exactly that
    // shade), so the pointer leaves before the frames are compared.
    c.post(Event::PointerMove { x: 230.0, y: 115.0 });
    c.frame(16.0, W, H);
    let direct = text(&c);
    println!("v2 swapped: {swapped}");
    println!("v2 direct:  {direct}");

    assert!(
        swapped.contains("clicks: 1"),
        "after the swap the frame must show the NEW format with the OLD count; it shows `{swapped}`"
    );
    assert_eq!(swapped, direct, "the swapped frame's text differs from v2 run directly");
    assert_eq!(
        b.list().cmds,
        c.list().cmds,
        "the swapped frame's commands differ from v2 run directly"
    );
    assert_eq!(b.list().text, c.list().text);
    println!("  ok: {} commands, byte for byte", b.list().cmds.len());
    println!("\nWINDOW RELOAD (headless): PASS");
}
