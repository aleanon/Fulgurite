//! **A shadow's damage covers where the shadow was drawn** — M57/A14
//! (`docs/plan-widget-fixes.md`).
//!
//! A shadow is drawn dropped by a third of its spread, like light from
//! above, and retention damaged the *undropped* rect inflated by the
//! spread. So the lowest few rows of every shadow lay outside its damage,
//! and a shadowed box that shrank left them on screen: the audit's
//! Combobox, filtering 6 → 2 → 0 rows, kept grey lines where the list's
//! shadow had been (`docs/widget-audit.md`, Combobox).
//!
//! Partial repaint against full repaint, byte for byte, as
//! `tests/damage_oracle.rs` does for text.

use fulgurite::retain::Damage;
use fulgurite::style::{Edges, Length, Patch, Rule, Selector};
use fulgurite::{App as DeriveApp, Color, StaticTree};
use fulgurite_shell::software::Canvas;
use fulgurite_shell::Host;
use fulgurite_widgets::ops::Fixed;

const W: u32 = 240;
const H: u32 = 240;
const BG: Color = Color(236, 238, 242, 255);

/// A container draws the shadow: `Fixed` paints none at all, which the
/// second draft found by printing its damage — exactly the box.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Panel {
    #[widget_id("panel")]
    f: Fixed,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("box")]
    panel: Panel,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

/// Styled once: a stylesheet change resets retention and repaints the
/// whole window, which tests nothing about one shadow. The box shrinks by
/// its own field instead.
fn sheet() -> Vec<Rule> {
    vec![
        // **The container fills the window**, so its own background never
        // changes size. The first draft let it shrink with the panel, and
        // the container's repaint covered the whole old shadow: the
        // injection that restores the old damage left it green, with the
        // damage printed as the full width of the window.
        Rule::new(
            Selector::Kind("Ui"),
            Patch::new().background(BG).padding(Edges::all(30.0)).width(Length::FILL).height(Length::FILL),
        ),
        Rule::new(
            Selector::Id("box"),
            Patch::new()
                .background(Color(255, 255, 255, 255))
                .radius(10.0)
                .shadow(12.0),
        ),
    ]
}

fn full(h: &Host<App>, c: &mut Canvas) {
    c.clear(BG);
    h.render(c);
}

#[test]
fn a_shrinking_shadow_leaves_nothing_behind() {
    let mut host: Host<App> = Host::new(App::default(), W, H);
    host.app.ui.panel.f = Fixed { w: 150.0, h: 160.0 };
    host.set_stylesheet(&sheet());
    host.pump();
    let (mut oracle, mut incremental) = (Canvas::new(W, H), Canvas::new(W, H));
    full(&host, &mut oracle);
    full(&host, &mut incremental);
    host.take_damage_for_age(1);

    let mut partial = 0;
    let mut narrow = false;
    for h in [120.0, 80.0, 40.0] {
        host.app.ui.panel.f.h = h;
        assert!(host.invalidate("panel"));
        host.pump();
        full(&host, &mut oracle);
        match host.take_damage_for_age(1) {
            Damage::None => {}
            Damage::All => full(&host, &mut incremental),
            d @ Damage::Regions(_) => {
                partial += 1;
                let rs: Vec<_> = d.regions([0.0, 0.0, W as f32, H as f32]).collect();
                println!("  regions {rs:?}");
                narrow |= rs.iter().all(|r| (r[2] - r[0]) * (r[3] - r[1]) < 0.8 * (W * H) as f32);
                for r in d.regions([0.0, 0.0, W as f32, H as f32]) {
                    incremental.clear_within(r, BG);
                    incremental.render_within(&host.list, r);
                }
            }
        }
        let differ = oracle.pixels.chunks(4).zip(incremental.pixels.chunks(4)).filter(|(a, b)| a != b).count();
        println!("height {h}: {differ} pixels differ between the partial and the full repaint");
        assert_eq!(differ, 0, "height {h}: the old shadow was left behind");
    }
    assert!(partial >= 2, "setup: only {partial} steps repainted partially, so nothing was tested");
    assert!(narrow, "setup: every partial repaint covered the whole window, so the shadow's own damage was never tested");
}
