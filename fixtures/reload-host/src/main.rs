//! A headless window loader: swap `fixtures/reload-app` from v1 to v2
//! and check the swapped session against v2 run from the start —
//! M37/T37.1 (`docs/plan-window-reload.md`).
//!
//! `reload-host <v1.so> <v2.so> <v3.so>`. The exit code is the verdict.
//!
//! **What is compared.** v1 is clicked once, saved, dropped; v2 boots
//! its own program with those bytes. A second v2 session boots with
//! nothing and is clicked once. The two frames must agree command for
//! command and byte for byte: the restore produced exactly the state the
//! input would have, under the new code.

use std::path::Path;

use lumen_static::display::Cmd;
use lumen_static::Event;
use lumen_static_reload::hot::{Build, Hot, Outcome, Reloader};
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

fn text_of(s: &dyn Session) -> String {
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
    // ---- T37.2: when the window swaps, and when it must not ---------
    //
    // `Reloader` is the window's swap logic without the window. A failed
    // build must leave the running library in place — its `output` names
    // v2's library, as a stale file from an earlier build would — and
    // the next good build must swap.
    let mut r = Reloader::new();
    let mut cur: Box<dyn Session> = Box::new(v1.boot(None).expect("v1 boots"));
    cur.frame(0.0, W, H);
    cur.post(Event::down(AT.0, AT.1));
    cur.post(Event::up(AT.0, AT.1));
    cur.frame(16.0, W, H);
    assert!(text_of(&*cur).contains("count 1"), "the click did not land: `{}`", text_of(&*cur));
    let adopt = |s: &mut dyn Session| {
        s.frame(0.0, W, H);
    };

    r.outcome(Outcome::Failed {
        log: "error[E0308]: mismatched types\n --> src/lib.rs:40:9".into(),
        output: std::path::PathBuf::from(&args[1]),
    });
    let swapped = r.swap(&mut cur, adopt);
    cur.frame(16.0, W, H);
    let after_failure = text_of(&*cur);
    println!("after a failed build: {after_failure} (swapped: {swapped}, reloads {})", r.reloads);
    assert!(
        after_failure.contains("count 1") && !swapped,
        "a failed build swapped the running library: the frame is `{after_failure}`, not v1's"
    );
    assert!(r.error.as_deref().is_some_and(|e| e.contains("E0308")), "the build log must be kept to show");

    r.outcome(Outcome::Built(std::path::PathBuf::from(&args[1])));
    let pending = r.pending().map(|p| p.display().to_string());
    assert!(r.swap(&mut cur, adopt), "the next good build must swap");
    let after_good = text_of(&*cur);
    println!("after a good build:   {after_good} (from {}, reloads {})", pending.unwrap_or_default(), r.reloads);
    assert!(after_good.contains("clicks: 1"), "the good build's frame: `{after_good}`");
    assert_eq!(r.error, None, "a successful swap clears the error");
    assert_eq!(r.reloads, 1);

    // The same bytes again change nothing, so nothing swaps.
    r.outcome(Outcome::Built(std::path::PathBuf::from(&args[1])));
    assert!(r.pending().is_none(), "an unchanged library was queued to swap");
    // A failure, then the source fixed back to what last built: the
    // rebuilt library is byte-identical to the running one, so nothing
    // swaps -- but the failure must not stay on screen.
    r.outcome(Outcome::Failed { log: "error: expected `;`".into(), output: std::path::PathBuf::from(&args[1]) });
    r.outcome(Outcome::Built(std::path::PathBuf::from(&args[1])));
    assert!(r.pending().is_none() && r.error.is_none(), "a fixed build left the error up: {:?}", r.error);
    println!("  ok: a failed build kept v1; the good one swapped; an unchanged one did not");

    // ---- T37.3: kept state whose type changed -------------------------
    //
    // v3 changes the kept `name` from a `String` to a `u32`. The swap
    // must complete; `name` must hold v3's boot value; the warning must
    // name it; and `tally` must survive — per field, not all or nothing.
    let saved = String::from_utf8_lossy(&cur.save()).into_owned();
    r.outcome(Outcome::Built(std::path::PathBuf::from(&args[2])));
    assert!(r.swap(&mut cur, adopt), "a changed field's type must not refuse the swap");
    cur.frame(16.0, W, H);
    let after = text_of(&*cur);
    let warning = r.warning.clone().unwrap_or_default();
    println!("saved before v3: {saved}");
    println!("after v3:        {after}");
    println!("warning:         {warning}");
    assert!(warning.contains("name"), "the warning must name the field that reset: `{warning}`");
    assert!(after.contains("name 7"), "`name` must hold v3's boot value: `{after}`");
    assert!(after.contains("clicks: 1"), "every other kept field survives -- `tally` did not: `{after}`");
    assert!(r.error.is_none(), "a reset field is a warning, not a failure");
    println!("  ok: `name` reset to its boot value and was named; `tally` kept count 1");

    println!("\nWINDOW RELOAD (headless): PASS");
}
