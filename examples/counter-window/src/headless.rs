//! `--drive` and `--drive2`: the tally without a window, through the
//! session the window drives.
//!
//! Rung 2 of the verification ladder. Since M60/T60.4
//! (`docs/plan-tally-slim.md`) both go through
//! `lumen_static_shell::session::Local`, booted from the same
//! [`program`](crate::app::program) the window runs: `--drive` sends the
//! agent protocol through `Session::agent`, as the window's endpoint
//! does, and `--drive2` runs M12's harness over the same session's
//! `Local::live`. The example keeps no adapter of its own. One frame to a
//! file is `LUMEN_SHOT` and the profiler is `LUMEN_PROFILE`, both the
//! window crate's.

use lumen_static_shell::session::{Crashing, Local, Offscreen, Session};
use serde_json::{json, Value};

use crate::app::{program, Tally};

const W: u32 = 440;
const H: u32 = 520;

fn session() -> (Local<Tally, Crashing>, Offscreen) {
    let mut s = Local::from_program(program(0, W, H), Crashing);
    let mut screen = Offscreen::new(W, H);
    // The window's first redraw, which a drive starts from.
    s.frame(0.0, W, H);
    lumen_static_shell::session::Screen::present(&mut screen, &mut s, crate::app::PAGE);
    (s, screen)
}

/// One request through `Session::agent`, as the window's endpoint sends
/// it. An error reply fails the drive by name.
fn ask(s: &mut Local<Tally, Crashing>, screen: &mut Offscreen, method: &str, params: Value) -> Value {
    let req = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    let r: Value = serde_json::from_str(&s.agent(&req.to_string(), screen)).expect("the reply is JSON");
    if let Some(e) = r.get("error") {
        panic!("drive: {method} {params} failed: {e}");
    }
    r["result"].clone()
}

/// `--drive` runs the app through the agent protocol and asserts.
///
/// Lumen's `verifying-apps` skill puts this above screenshots: assert on
/// state, semantics and bounds first, pixels second. Every click goes in
/// by *author id* through `input.click`, and must answer `handled` — a
/// click that reached no handler fails here, not three assertions later
/// on an unchanged count (the M16/T16.7 shape).
pub fn drive() {
    let (mut s, mut screen) = session();
    let mut clicks = 0;
    let mut click = |s: &mut Local<Tally, Crashing>, screen: &mut Offscreen, id: &str| {
        let r = ask(s, screen, "input.click", json!({ "selector": id }));
        assert_eq!(r["handled"], true, "drive: the click on {id} reached no handler: {r}");
        clicks += 1;
    };
    let text = |s: &mut Local<Tally, Crashing>, screen: &mut Offscreen, id: &str| -> String {
        ask(s, screen, "ui.resolve", json!({ "selector": id }))["value"].as_str().unwrap_or("").to_string()
    };

    click(&mut s, &mut screen, "#inc");
    click(&mut s, &mut screen, "#inc");
    click(&mut s, &mut screen, "#inc10");
    assert_eq!(s.host.app.count.n, 12, "+1 +1 +10");
    assert_eq!(text(&mut s, &mut screen, "#value"), "12", "and the binding put it on screen");
    assert_eq!(text(&mut s, &mut screen, "#pill"), "POSITIVE");

    click(&mut s, &mut screen, "#dec10");
    click(&mut s, &mut screen, "#dec10");
    click(&mut s, &mut screen, "#dec");
    assert_eq!(s.host.app.count.n, -9);
    assert_eq!(text(&mut s, &mut screen, "#pill"), "NEGATIVE");

    click(&mut s, &mut screen, "#reset");
    assert_eq!(s.host.app.count.n, 0);
    assert_eq!(text(&mut s, &mut screen, "#value"), "0");
    assert_eq!(text(&mut s, &mut screen, "#pill"), "AT ZERO");

    // Bounds, not just state: every control the agent can name is on
    // screen and has area.
    for id in ["#dec10", "#dec", "#inc", "#inc10", "#reset", "#value", "#pill", "#caption"] {
        let l = ask(&mut s, &mut screen, "ui.getLayout", json!({ "selector": id }));
        let f = |v: &Value| v.as_f64().unwrap_or(-1.0) as f32;
        let (x, y) = (f(&l["pos"][0]) + f(&l["size"][0]) / 2.0, f(&l["pos"][1]) + f(&l["size"][1]) / 2.0);
        assert!(
            f(&l["size"][0]) > 0.0 && x > 0.0 && y > 0.0 && x < W as f32 && y < H as f32,
            "{id} centre ({x}, {y}) is off screen, or it has no area: {l}"
        );
    }

    println!("drive: {clicks} clicks by author id through Session::agent -- all assertions passed");
}

/// `--drive2`: the same session, through **M12/T12.1's harness**.
///
/// Both are kept and both run, because the comparison is the evidence —
/// a harness that were *only* shorter would be a convenience, and the
/// claim is that it says more:
///
/// * a failure carries the protocol's own words, including the nearest
///   candidates for a missed selector;
/// * a failure lists the steps that preceded it.
///
/// The harness drives the session's own `Live` (`Local::live`) — the
/// adapter `Session::agent` uses, so there is one.
pub fn drive_harness() {
    use lumen_static_agent::harness::{Expect, TestApp};
    use lumen_static_agent::AgentHost;

    fn script<H: AgentHost>(app: &mut TestApp<'_, H>, w: f32, h: f32) -> Expect {
        app.click("#inc")?;
        app.click("#inc")?;
        app.click("#inc10")?;
        app.expect_text("#value", "12")?;
        app.expect_text("#pill", "POSITIVE")?;

        app.click("#dec10")?;
        app.click("#dec10")?;
        app.click("#dec")?;
        app.expect_text("#pill", "NEGATIVE")?;

        app.click("#reset")?;
        app.expect_text("#value", "0")?;
        app.expect_text("#pill", "AT ZERO")?;

        // Bounds, not just state: every control the agent can name is on
        // screen and has area.
        for id in ["#dec10", "#dec", "#inc", "#inc10", "#reset", "#value", "#pill", "#caption"] {
            app.expect_visible(id, (w, h))?;
        }

        // And the accepting direction for the locator itself: a
        // selector that names nothing must say so. Without this, a
        // resolver that silently matched the root would pass every
        // assertion above against the wrong node.
        app.expect_absent("#no-such-control")?;
        Ok(())
    }

    let (mut s, mut screen) = session();
    let mut live = s.live(&mut screen);
    let mut app = TestApp::new(&mut live);
    match script(&mut app, W as f32, H as f32) {
        Ok(()) => println!("drive2: {} harness steps, all assertions passed", app.steps().len()),
        Err(e) => panic!("drive2: {e}"),
    }
}
