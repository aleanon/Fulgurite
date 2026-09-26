//! **Damage is in the surface's pixels at every scale** — M57/A2
//! (`docs/plan-widget-fixes.md`; R1-F1 of the M38 review).
//!
//! Retention observed the command stream *before* `Scaled` converted it,
//! so the damage it banked was in logical pixels and was applied to a
//! physical surface: at scale 2 a change in the right half of the window
//! repainted a region half its size, up and to the left. The widget audit
//! met it as a button whose count did not repaint until something else
//! did (`docs/widget-audit.md`, bug 1(b)). Partial against full repaint,
//! byte for byte, at 2 and at 1.5.

use lumen_static::retain::Damage;
use lumen_static::style::{Edges, Length, Patch, Rule, Selector};
use lumen_static::{App as DeriveApp, Color, StaticTree};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::Host;
use lumen_static_widgets::ops::{Fixed, Label};

const BG: Color = Color(240, 242, 246, 255);

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Ui {
    /// Pushes the label to the right half, where a logical rect read as
    /// physical misses it entirely.
    gap: Fixed,
    #[widget_id("count")]
    count: Label<String>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

fn run(scale: f32) -> (usize, usize) {
    let (lw, lh) = (200.0f32, 60.0f32);
    let (w, h) = ((lw * scale).round() as u32, (lh * scale).round() as u32);
    let mut app = App::default();
    app.ui.gap = Fixed { w: 120.0, h: 10.0 };
    app.ui.count.set("0".into());
    let mut host: Host<App> = Host::new(app, w, h);
    host.set_scale(scale);
    host.set_stylesheet(&[
        Rule::new(Selector::Kind("Ui"), Patch::new().background(BG).padding(Edges::all(10.0)).width(Length::FILL).height(Length::FILL)),
        Rule::new(Selector::Id("count"), Patch::new().color(Color(0, 0, 0, 255)).font_size(24.0)),
    ]);
    host.pump();
    let (mut full, mut part) = (Canvas::new(w, h), Canvas::new(w, h));
    for c in [&mut full, &mut part] {
        c.clear(BG);
        host.render(c);
    }
    host.take_damage_for_age(1);
    let (mut partial, mut worst) = (0, 0);
    for n in 1..6 {
        host.app.ui.count.set(format!("{}", n * 111));
        assert!(host.invalidate("count"));
        host.pump();
        full.clear(BG);
        host.render(&mut full);
        match host.take_damage_for_age(1) {
            Damage::None => {}
            Damage::All => {
                part.clear(BG);
                host.render(&mut part);
            }
            d @ Damage::Regions(_) => {
                partial += 1;
                for r in d.regions([0.0, 0.0, w as f32, h as f32]) {
                    part.clear_within(r, BG);
                    part.render_within(&host.list, r);
                }
            }
        }
        let differ = full.pixels.chunks(4).zip(part.pixels.chunks(4)).filter(|(a, b)| a != b).count();
        worst = worst.max(differ);
    }
    (partial, worst)
}

#[test]
fn a_partial_repaint_matches_a_full_one_at_every_scale() {
    for scale in [1.0, 1.5, 2.0] {
        let (partial, differ) = run(scale);
        println!("scale {scale}: {partial} partial repaints, worst {differ} pixels differ");
        assert!(partial >= 3, "setup: scale {scale} repainted partially only {partial} times");
        assert_eq!(differ, 0, "scale {scale}: the partial repaint left {differ} stale pixels");
    }
}
