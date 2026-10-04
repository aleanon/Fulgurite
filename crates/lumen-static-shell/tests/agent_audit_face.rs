//! **A session's audit measures with the face its host lays out with** —
//! M60/T60.3 (`docs/plan-tally-slim.md`, "What building found").
//!
//! `ui.lint`'s W1005 compares a node's box with its content's natural
//! size, measured by the agent's own `TextMeasure`. `Local` built its
//! agent with the block stand-in while its host laid out with the system
//! face, so every `Program` window's lint was the block's opinion: the
//! tally's first run on the framework's window reported `#dec10` wanting
//! 72 px in a 66 px box. `examples/counter-window` had set the measure
//! itself, in the loop M60 deleted. Needs a system face (it is the case);
//! skips without one unless `LUMEN_REQUIRE_FONTS=1`.

use lumen_static::prelude::*;
use lumen_static::style::{Edges, Patch, Rule, Selector};
use lumen_static_shell::program::application;
use lumen_static_shell::session::{Crashing, Local, Offscreen, Session};
use lumen_static_shell::text::FontStack;
use lumen_static_widgets::prelude::*;
use serde_json::Value;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 8.0)]
struct Steps {
    #[widget_id("dec10")]
    dec10: Button<App>,
    #[widget_id("dec")]
    dec: Button<App>,
    #[widget_id("inc")]
    inc: Button<App>,
    #[widget_id("inc10")]
    inc10: Button<App>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Steps,
}

#[test]
fn ui_lint_measures_with_the_sessions_face() {
    if FontStack::system().is_none() {
        assert!(
            std::env::var("LUMEN_REQUIRE_FONTS").as_deref() != Ok("1"),
            "LUMEN_REQUIRE_FONTS=1 but no system face"
        );
        println!("no system face -- the block stand-in is both sides' measure; skipped");
        return;
    }
    let program = application(|| {
        let mut a = App::default();
        a.ui.dec10.label = "\u{2212}10";
        a.ui.dec.label = "\u{2212}1";
        a.ui.inc.label = "+1";
        a.ui.inc10.label = "+10";
        a
    })
    .size(440, 120)
    .style(|_: &App| {
        vec![Rule::new(
            Selector::Kind("button"),
            Patch::new().font_size(16.0).font_weight(600.0).padding(Edges { left: 16.0, right: 16.0, top: 11.0, bottom: 11.0 }),
        )]
    });
    let mut s = Local::from_program(program, Crashing);
    let mut screen = Offscreen::new(440, 120);
    s.frame(0.0, 440, 120);
    s.agent_frame_done(Default::default());
    let r: Value = serde_json::from_str(&s.agent(r#"{"jsonrpc":"2.0","id":1,"method":"ui.lint","params":{}}"#, &mut screen))
        .expect("JSON");
    let findings = r["result"]["findings"].as_array().cloned().unwrap_or_default();
    let w1005: Vec<&Value> = findings.iter().filter(|f| f["code"] == "W1005").collect();
    let dec10 = &s.host.states[lumen_static::style::index_of_wid(<Steps as Widget>::SUB_TABLE, "dec10").unwrap()];
    println!(
        "#dec10 laid out {}x{}; ui.lint: {} finding(s), {} W1005 (bound 0): {w1005:?}",
        dec10.size.w,
        dec10.size.h,
        findings.len(),
        w1005.len()
    );
    assert!(r["result"]["findings"].is_array(), "setup: ui.lint answered no findings list: {r}");
    assert!(dec10.size.w > 0.0, "setup: nothing was laid out");
    assert!(w1005.is_empty(), "the audit measured the buttons with another face than the host laid them out with");
}

/// **Under `Complex`, the audit shapes too** — M81/T81.4 (review C26, T8;
/// `docs/plan-shaped-paint.md`). An Arabic label is laid out at its
/// *joined* width by a `Complex` program's host; an audit measuring with
/// a `Basic` stack would see the wider isolated letters, find the box
/// too small, and report W1005 on a label nothing is wrong with. `Local`
/// builds the audit's measurer from a clone of the stack the host
/// measures with, so the two cannot disagree.
#[cfg(feature = "shaping")]
#[test]
fn ui_lint_shapes_with_the_sessions_stack() {
    use lumen_static::TextMeasure;
    use lumen_static_shell::text::{FontMetrics, Shaping};

    #[derive(StaticTree, Default)]
    #[widget(row, spacing = 0.0)]
    struct ArUi {
        #[widget_id("ar")]
        ar: Label<String>,
    }

    #[derive(DeriveApp, Default)]
    struct Ar {
        #[tree]
        ui: ArUi,
    }

    const ARABIC: &str = "مرحبا بالعالم";
    const SIZE: f32 = 24.0;
    let Some(stack) = FontStack::system() else {
        assert!(std::env::var("LUMEN_REQUIRE_FONTS").as_deref() != Ok("1"), "LUMEN_REQUIRE_FONTS=1 but no system face");
        println!("no system face -- skipped");
        return;
    };
    let joined = FontMetrics::new(stack.clone().with_shaping(Shaping::Complex)).measure(ARABIC, SIZE, f32::INFINITY);
    let isolated = FontMetrics::new(stack.clone()).measure(ARABIC, SIZE, f32::INFINITY);
    println!("setup: {ARABIC:?} at {SIZE} px: joined (Complex) {}x{}, isolated (Basic) {}x{}", joined.w, joined.h, isolated.w, isolated.h);
    assert!(isolated.w > joined.w + 1.0, "setup: the isolated letters must be wider than the joined run, or no W1005 can fire");

    let program = application(|| {
        let mut a = Ar::default();
        a.ui.ar.set(ARABIC.to_owned());
        a
    })
    .size(440, 120)
    .style(|_: &Ar| vec![Rule::new(Selector::Id("ar"), Patch::new().font_size(SIZE))])
    .shaping(Shaping::Complex);
    let mut s = Local::from_program(program, Crashing);
    let mut screen = Offscreen::with_shaping(440, 120, s.shaping());
    s.frame(0.0, 440, 120);
    s.agent_frame_done(Default::default());
    let r: Value = serde_json::from_str(&s.agent(r#"{"jsonrpc":"2.0","id":1,"method":"ui.lint","params":{}}"#, &mut screen))
        .expect("JSON");
    let findings = r["result"]["findings"].as_array().cloned().unwrap_or_default();
    let w1005: Vec<&Value> = findings.iter().filter(|f| f["code"] == "W1005").collect();
    let ar = &s.host.states[lumen_static::style::index_of_wid(<ArUi as Widget>::SUB_TABLE, "ar").unwrap()];
    println!(
        "#ar laid out {}x{} (Session::shaping {:?}); ui.lint: {} finding(s), {} W1005 (bound 0): {w1005:?}",
        ar.size.w,
        ar.size.h,
        s.shaping(),
        findings.len(),
        w1005.len()
    );
    assert!(r["result"]["findings"].is_array(), "setup: ui.lint answered no findings list: {r}");
    assert!((ar.size.w - joined.w).abs() <= 0.5, "setup: the host did not lay the label out at its joined width");
    assert!(w1005.is_empty(), "the audit measured the Arabic label with another stack than the host laid it out with");
}
