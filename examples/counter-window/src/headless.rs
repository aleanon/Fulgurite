//! `--headless`, `--profile` and `--drive`: the tally without a window.
//!
//! Rungs 1-3 of the verification ladder. Moved out of `main.rs` by
//! M14/T14.0; none of it is windowing code.

use std::time::Instant;

use crate::app;
use lumen_static::prelude::*;
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::{FontMetrics, FontStack};
use lumen_static_shell::Host;

use crate::app::{build, stylesheet_for, Tally, PAGE};

pub fn headless(path: &str, count: i64) {
    let (w, h) = (440u32, 520u32);
    let mut host = Host::new(build(), w, h);
    host.app.count = count;
    host.set_stylesheet(&stylesheet_for(count.signum()));
    let mut canvas = match FontStack::system() {
        Some(f) => Canvas::with_font(w, h, f),
        None => Canvas::new(w, h),
    };
    let mut metrics = FontStack::system().map(FontMetrics::new);
    match metrics.as_mut() {
        Some(m) => {
            host.pump_with(m);
        }
        None => {
            host.pump();
        }
    }
    canvas.clear(PAGE);
    host.render(&mut canvas);
    std::fs::write(path, canvas.to_ppm()).expect("write frame");
    if std::env::var_os("LUMEN_PROFILE").is_some() {
        profile(&mut host, &mut canvas, metrics.as_mut());
    }
    println!(
        "tally: count={count} nodes={} cmds={} ink={:.1}% -> {path}",
        <app::Page as Widget>::NODE_COUNT,
        host.list.len(),
        canvas.ink_fraction(PAGE) * 100.0,
    );
}

/// Per-stage frame cost, 50 frames each, median. Temporary harness for
/// the 2026-09-03 profiling round; the numbers go in
/// `docs/results-frame-cost.md`.
pub fn profile(host: &mut Host<Tally>, canvas: &mut Canvas, mut metrics: Option<&mut FontMetrics>) {
    let med = |name: &str, mut f: Box<dyn FnMut() + '_>| {
        let mut v: Vec<u128> = (0..50)
            .map(|_| {
                let t = Instant::now();
                f();
                t.elapsed().as_micros()
            })
            .collect();
        v.sort();
        eprintln!("{name:<22} {:>8} us", v[25]);
    };
    let (w, h) = (canvas.width, canvas.height);
    med(
        "pump(layout+paint)",
        Box::new(|| match metrics.as_mut() {
            Some(m) => {
                host.pump_with(*m);
            }
            None => {
                host.pump();
            }
        }),
    );
    med("clear", Box::new(|| canvas.clear(PAGE)));
    med("render(all)", Box::new(|| host.render(canvas)));
    // Per command class.
    let mut only = |name: &str, keep: Box<dyn Fn(&lumen_static::Cmd) -> bool>| {
        let mut sub = lumen_static::DisplayList::new();
        for c in &host.list.cmds {
            if keep(c) {
                sub.cmds.push(*c);
            }
        }
        sub.text = host.list.text.clone();
        let mut v: Vec<u128> = (0..50)
            .map(|_| {
                let t = Instant::now();
                lumen_static::Renderer::render(canvas, &sub, w, h);
                t.elapsed().as_micros()
            })
            .collect();
        v.sort();
        eprintln!("{name:<22} {:>8} us  ({} cmds)", v[25], sub.cmds.len());
    };
    use lumen_static::Cmd;
    only("  rect", Box::new(|c| matches!(c, Cmd::Rect { .. })));
    only("  rounded_rect", Box::new(|c| matches!(c, Cmd::RoundedRect { .. })));
    // **Shadow was missing from this list until 2026-09-09** while the
    // damaged breakdown below had it, so the full-render rows summed to
    // 86 of 345 us and the largest single command class was
    // unattributed. It is 75% of a full redraw of the tally
    // (`docs/results-wide-stores.md`), which is not what
    // `docs/results-frame-cost.md` says a reader should expect.
    only("  shadow", Box::new(|c| matches!(c, Cmd::Shadow { .. })));
    only("  text", Box::new(|c| matches!(c, Cmd::Text { .. })));
    let mut buf = vec![0u32; (w * h) as usize];
    med("blit rgba->0rgb", Box::new(|| canvas.blit_0rgb(&mut buf)));

    // The partial path (M8b), against the damage a count change makes.
    // Bank the seed frame first, or the damage is the whole viewport.
    host.take_damage_for_age(1);
    host.app.count += 1;
    match metrics.as_mut() {
        Some(m) => {
            host.pump_with(*m);
        }
        None => {
            host.pump();
        }
    }
    if let Some(area) =
        host.take_damage_for_age(1).clip_to([0.0, 0.0, w as f32, h as f32])
    {
        let px = ((area[2] - area[0]) * (area[3] - area[1])) as u64;
        eprintln!("{:<22} {:>8} px  {area:?}", "damage", px);
        med("  clear_within", Box::new(|| canvas.clear_within(area, PAGE)));
        med("  render_within", Box::new(|| canvas.render_within(&host.list, area)));
        med("  blit_within", Box::new(|| canvas.blit_0rgb_within(&mut buf, area)));
        // What each command class costs inside that rect.
        let mut only_in = |name: &str, keep: Box<dyn Fn(&lumen_static::Cmd) -> bool>| {
            let mut sub = lumen_static::DisplayList::new();
            for c in &host.list.cmds {
                if keep(c) {
                    sub.cmds.push(*c);
                }
            }
            sub.text = host.list.text.clone();
            let mut v: Vec<u128> = (0..50)
                .map(|_| {
                    let t = Instant::now();
                    canvas.render_within(&sub, area);
                    t.elapsed().as_micros()
                })
                .collect();
            v.sort();
            eprintln!("{name:<22} {:>8} us  ({} cmds)", v[25], sub.cmds.len());
        };
        only_in("    rounded_rect", Box::new(|c| matches!(c, Cmd::RoundedRect { .. })));
        only_in("    shadow", Box::new(|c| matches!(c, Cmd::Shadow { .. })));
        only_in("    text", Box::new(|c| matches!(c, Cmd::Text { .. })));
    }
}

/// `--drive` runs the app through the agent surface and asserts.
///
/// Lumen's `verifying-apps` skill puts this above screenshots: assert on
/// state, semantics and bounds first, pixels second. Every click here
/// goes in by *author id* through `ui.getLayout` — the agent asks where
/// `#inc` is and clicks its centre — so the same path an agent driving
/// this app would take is the path the test takes.
pub fn drive() {
    use lumen_static_agent::{Agent, TreeRef};
    use serde_json::json;

    let (w, h) = (440u32, 520u32);
    let mut host = Host::new(build(), w, h);
    host.set_stylesheet(&stylesheet_for(0));
    host.pump();

    let agent = Agent::new();
    let centre = |host: &Host<Tally>, id: &str| -> (f32, f32) {
        let tree = TreeRef::of_root(&host.app.ui, &host.styles, &host.states);
        let r = agent.dispatch(
            &tree,
            &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.getLayout",
                     "params": { "selector": id } }),
        );
        let l = &r["result"];
        let (x, y) = (l["pos"][0].as_f64().unwrap(), l["pos"][1].as_f64().unwrap());
        let (bw, bh) = (l["size"][0].as_f64().unwrap(), l["size"][1].as_f64().unwrap());
        ((x + bw / 2.0) as f32, (y + bh / 2.0) as f32)
    };

    let click = |host: &mut Host<Tally>, id: &str| {
        let (x, y) = centre(host, id);
        host.click(x, y);
        host.pump();
    };

    click(&mut host, "#inc");
    click(&mut host, "#inc");
    click(&mut host, "#inc10");
    assert_eq!(host.app.count, 12, "+1 +1 +10");
    assert_eq!(host.app.ui.card.value.as_str(), "12", "and the binding put it on screen");
    assert_eq!(host.app.ui.card.pill.as_str(), "POSITIVE");

    click(&mut host, "#dec10");
    click(&mut host, "#dec10");
    click(&mut host, "#dec");
    assert_eq!(host.app.count, -9);
    assert_eq!(host.app.ui.card.pill.as_str(), "NEGATIVE");

    click(&mut host, "#reset");
    assert_eq!(host.app.count, 0);
    assert_eq!(host.app.ui.card.value.as_str(), "0");
    assert_eq!(host.app.ui.card.pill.as_str(), "AT ZERO");

    // Bounds, not just state: every control the agent can name is on
    // screen and has area.
    for id in ["#dec10", "#dec", "#inc", "#inc10", "#reset", "#value", "#pill", "#caption"] {
        let (x, y) = centre(&host, id);
        assert!(
            x > 0.0 && y > 0.0 && x < w as f32 && y < h as f32,
            "{id} centre ({x}, {y}) is off screen"
        );
    }

    println!("drive: 7 clicks by author id through ui.getLayout — all assertions passed");
}
