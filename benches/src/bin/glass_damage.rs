//! **What a small change under a glass panel costs, through `Host`** —
//! M50/T50.0, for R8-F6 (`docs/plan-raster-allocation.md` §2.5).
//!
//! The review measured a small damage rect under a frosted panel with the
//! host bypassed, and the verifier found that number is not what a window
//! pays: through the real `Host`, retention's `dilate` grows any damage
//! that touches a panel's source region into the whole source before the
//! renderer sees it. So this pumps a real tree — a stack with a label
//! under a `Glass` bar, the composition `examples/glass` ships — changes
//! the label, and prints what the renderer receives and what drawing it
//! costs.
//!
//! The label is a 24 px square of text that alternates between two
//! characters, so the change is the size the plan names. Three panel
//! heights at 1920 wide: the toolbar `examples/glass` uses (64 px), a
//! sheet (360 px) and a full-window panel. `FROSTED` and `LIQUID` each.
//!
//! §5's kill rule for T50.9 reads the `ms` column: build the
//! damage-bounded variant only if a small change under a frosted panel
//! costs more than a quarter of a 16.7 ms frame. Timings are the median
//! of eight, on the software canvas with the pinned face; run it under
//! `just bench-lock` and mark it LOADED when the machine was busy.
//!
//! Run: `just bench-lock cargo run --release -p benches --bin glass_damage`

use std::time::Instant;

use fulgurite::display::{Color, Material};
use fulgurite::style::{Length, Patch, Rule, Selector};
use fulgurite::{App as DeriveApp, StaticTree};
use fulgurite_shell::software::Canvas;
use fulgurite_shell::text::{FontMetrics, FontStack};
use fulgurite_shell::Host;
use fulgurite_widgets::ops::{Label, Spacer};
use fulgurite_widgets::Glass;

const W: u32 = 1920;
const H: u32 = 1080;
const PAGE: Color = Color(0x30, 0x50, 0x90, 0xff);

/// What sits under the panel: the label that changes, in a column.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Under {
    #[widget_id("mark")]
    mark: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(stack)]
struct Screen {
    under: Under,
    #[widget_id("bar")]
    bar: Glass<Spacer>,
}

#[derive(DeriveApp, Default)]
struct Demo {
    #[tree]
    ui: Screen,
}

fn sheet(bar_h: f32) -> Vec<Rule> {
    vec![
        Rule::new(Selector::Kind("Screen"), Patch::new().background(PAGE)),
        Rule::new(Selector::Id("bar"), Patch::new().height(Length::Fixed(bar_h))),
        Rule::new(
            Selector::Id("mark"),
            Patch::new()
                .width(Length::Fixed(24.0))
                .height(Length::Fixed(24.0))
                .font_size(18.0)
                .color(Color(0xff, 0xff, 0xff, 0xff))
                .background(Color(0xe0, 0x40, 0x40, 0xff)),
        ),
    ]
}

fn face() -> FontStack {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/fulgurite-shell/tests/fonts/DejaVuSans.ttf");
    let bytes = std::fs::read(path).expect("the pinned face");
    FontStack::from_bytes(&bytes, None).expect("the pinned face parses")
}

struct Row {
    material: &'static str,
    bar: [f32; 4],
    damage: Vec<[f32; 4]>,
    ms: f64,
}

fn measure(material: (&'static str, Material), bar_h: f32) -> Row {
    let font = face();
    let mut m = FontMetrics::new(font.clone());
    let mut app = Demo::default();
    app.ui.bar.material = material.1;
    app.ui.under.mark.set("A".to_string());
    let mut host: Host<Demo> = Host::new(app, W, H);
    host.set_stylesheet(&sheet(bar_h));
    let mut canvas = Canvas::with_font(W, H, font);
    let vp = [0.0, 0.0, W as f32, H as f32];
    host.pump_with(&mut m);
    host.pump_with(&mut m);
    canvas.clear(PAGE);
    host.render(&mut canvas);
    let _ = host.take_damage_for_age(1);

    let bar_node = <Screen as fulgurite::Widget>::SUB_TABLE.iter().position(|d| d.wid == "bar").expect("the bar");
    let s = host.states[bar_node];
    let bar = [s.pos.x, s.pos.y, s.pos.x + s.size.w, s.pos.y + s.size.h];

    let mut samples = Vec::new();
    let mut damage = Vec::new();
    for k in 0..24u32 {
        host.app.ui.under.mark.set(if k.is_multiple_of(2) { "B" } else { "A" }.to_string());
        host.pump_with(&mut m);
        let regions: Vec<[f32; 4]> = host.take_damage_for_age(1).regions(vp).collect();
        assert!(!regions.is_empty(), "setup: the label changed and nothing was damaged");
        let t = Instant::now();
        for r in &regions {
            canvas.clear_within(*r, PAGE);
            canvas.render_within(&host.list, *r);
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        if k >= 8 {
            samples.push(ms);
        }
        damage = regions;
    }
    samples.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    let ms = (samples[samples.len() / 2 - 1] + samples[samples.len() / 2]) / 2.0;
    Row { material: material.0, bar, damage, ms }
}

fn main() {
    println!("glass_damage: a 24x24 label change under a Glass bar, {W}x{H}, through Host; median of 16\n");
    println!("{:>8} {:>22} {:>10} {:>40} {:>10} {:>8}", "material", "bar", "bar px", "damage the renderer receives", "damage px", "ms");
    for bar_h in [64.0, 360.0, H as f32] {
        for mat in [("FROSTED", Material::FROSTED), ("LIQUID", Material::LIQUID)] {
            let r = measure(mat, bar_h);
            let bar_px = (r.bar[2] - r.bar[0]) * (r.bar[3] - r.bar[1]);
            let dmg_px: f32 = r.damage.iter().map(|d| (d[2] - d[0]) * (d[3] - d[1])).sum();
            let dmg = r
                .damage
                .iter()
                .map(|d| format!("[{:.0},{:.0},{:.0},{:.0}]", d[0], d[1], d[2], d[3]))
                .collect::<Vec<_>>()
                .join(" ");
            println!(
                "{:>8} {:>22} {:>10.0} {:>40} {:>10.0} {:>8.2}",
                r.material,
                format!("[{:.0},{:.0},{:.0},{:.0}]", r.bar[0], r.bar[1], r.bar[2], r.bar[3]),
                bar_px,
                dmg,
                dmg_px,
                r.ms
            );
        }
    }
    println!("\nkill rule (T50.9): build only if a FROSTED row costs more than 16.7 / 4 = 4.17 ms");
}
