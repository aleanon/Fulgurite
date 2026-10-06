//! A counter that reloads itself inside its window — M37/T37.4
//! (`docs/plan-window-reload.md`).
//!
//! The drive edits the lines marked `// EDIT ...` in a copy of this
//! file: the bound label's format (`EDIT format`), and a line in the
//! click handler that becomes a type error or a panic (`EDIT handler`).
//! `#field` is focused after the swaps, for the IME area (M80/T80.5).
//! `#hebrew` is a right-to-left label whose pixels depend on how the
//! program lays text out: the drive edits `EDIT shaping` to
//! `Shaping::Complex` and the window must follow the swap (M81/T81.4), and
//! its `Everything` leg edits `EDIT strategy` before the build.
//! `#star` is a paint box whose callback fills a path (M82/T82.5): while
//! a failed build's error bar is up, the window draws a copy of the
//! session's list, and the star's region must keep its `ui.regionHash` —
//! a copy that dropped the list's path store would lose it. It is first
//! in the column and the window is 400 px tall, so the bar (at most ten
//! lines at the foot) never reaches it.
//! `#swatch` (M83/T83.8), second, is a `ShaderView` with the `shader`
//! feature: the drive's GPU legs edit `EDIT colour` in its WGSL from red
//! to blue and the swap must re-send the new build's table to the backend.
//! Without the feature it is an empty spacer in the same slot, so the tree
//! has one shape in every build.

use lumen_static::event::EventCtx;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{App as DeriveApp, Reloadable as DeriveReloadable, StaticTree};
use lumen_static_shell::program::{application, Program};
use lumen_static_shell::text::Shaping;
use lumen_static_widgets::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(StaticTree, Default, DeriveReloadable)]
#[widget(column, spacing = 8.0)]
pub struct Ui {
    /// A filled star, painted with `Painter::path` (M82/T82.5).
    #[widget_id("star")]
    pub star: PaintBox,
    /// A box the program's shader draws, with `shader` ([`Swatch`]).
    #[widget_id("swatch")]
    pub swatch: Swatch,
    #[widget_id("inc")]
    pub inc: Button<Tally>,
    #[widget_id("value")]
    pub value: Label<String>,
    /// A field to focus after a swap: the window must place its IME area
    /// from the new build's session (M80/T80.5), whose `caret_rect` is
    /// reached through the reload crate's forward.
    #[widget_id("field")]
    pub field: TextInput<String>,
    /// Hebrew: drawn right to left, joined and ordered, only under
    /// `Shaping::Complex` (M81/T81.4).
    #[widget_id("hebrew")]
    pub hebrew: Label<String>,
}

/// `#swatch`: a `ShaderView` running [`SHADERS`]'s one entry with the
/// `shader` feature (M83/T83.8)…
#[cfg(feature = "shader")]
pub type Swatch = lumen_static_widgets::ShaderView<16>;
/// …and an empty spacer in its slot without it.
#[cfg(not(feature = "shader"))]
pub type Swatch = Spacer;

/// The swatch's WGSL: a flat colour times the uniform block's gain (all
/// ones). The colour is on its own line, which the drive's GPU legs edit
/// in the copy; the gain is there so the bracket carries uniform bytes,
/// which the failed-build step's error bar must keep (`hot::Barred`
/// clones the list, uniforms included).
#[cfg(feature = "shader")]
const SWATCH: &str = "
struct U { gain: vec4<f32> }
@group(0) @binding(1) var<uniform> u: U;
@fragment fn fs_main(in: LumenVarying) -> @location(0) vec4<f32> {
    let c = vec3(0.86, 0.12, 0.16); // EDIT colour
    return vec4(c * u.gain.rgb, 1.0);
}
";

/// The program's shader table: `#swatch` names entry 0.
#[cfg(feature = "shader")]
pub static SHADERS: [lumen_static_shell::shader::ShaderDesc; 1] =
    [lumen_static_shell::shader::ShaderDesc::new("swatch", SWATCH, 16)];

/// What `#swatch` draws where the shader does not run: grey, neither of
/// the colours the drive edits between.
#[cfg(feature = "shader")]
fn swatch_fallback(out: &mut dyn lumen_static::Painter, r: [f32; 4], _: &[u8]) {
    out.rect(r, lumen_static::Color(0x80, 0x80, 0x80, 0xff));
}

/// `#star`'s callback: one filled path, placed by its transform.
fn star(out: &mut dyn lumen_static::Painter, r: [f32; 4]) {
    use lumen_static::display::{PathEl, PathPaint};
    use lumen_static::Point;
    let mut els = [PathEl::Close; 11];
    for (i, e) in els.iter_mut().take(10).enumerate() {
        let rr = if i % 2 == 0 { 1.0 } else { 0.4 };
        let a = -core::f32::consts::FRAC_PI_2 + i as f32 * core::f32::consts::PI / 5.0;
        let p = Point { x: rr * a.cos(), y: rr * a.sin() };
        *e = if i == 0 { PathEl::Move(p) } else { PathEl::Line(p) };
    }
    let k = 0.45 * (r[3] - r[1]);
    let paint = PathPaint { fill: STAR_INK, stroke: lumen_static::Color(0, 0, 0, 0), width: 0.0 };
    out.path(&els, [k, 0.0, 0.0, k, (r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0], paint);
}

/// `#star`'s colour, which the drive looks for at the box's centre.
pub const STAR_INK: lumen_static::Color = lumen_static::Color(0xc2, 0x41, 0x0c, 0xff);

/// What `#hebrew` says.
pub const HEBREW: &str = "שלום עולם";

/// What a handler may touch (M33), and what a reload keeps.
#[derive(Default, Serialize, Deserialize)]
pub struct Tally {
    pub count: i64,
}

#[derive(DeriveApp, DeriveReloadable)]
pub struct Counter {
    #[tree]
    pub ui: Ui,
    #[state]
    #[reload(keep)]
    #[bind(ui.value, count, "count {}")] // EDIT format
    pub tally: Tally,
}

fn on_inc(t: &mut Tally, _: &EventCtx) {
    let step = 1; // EDIT handler
    t.count += step;
}

/// The boot: the one place a `Counter` is built — by `main`, by the
/// library a reload loads, and by `Host::reload` (M49/T49.4).
fn boot() -> Counter {
    let mut c = Counter { ui: Ui::default(), tally: Tally::default() };
    c.ui.inc.label = "+";
    c.ui.inc.on_click = on_inc;
    c.ui.hebrew.set(HEBREW.to_owned());
    c.ui.star.draw = star;
    #[cfg(feature = "shader")]
    {
        let mut gain = [0u8; 16];
        for k in 0..4 {
            gain[k * 4..k * 4 + 4].copy_from_slice(&1.0f32.to_le_bytes());
        }
        let mut v = lumen_static_widgets::ShaderView::new(lumen_static::display::ShaderId(0), swatch_fallback);
        v.set_uniforms(&gain);
        c.ui.swatch = v;
    }
    c
}

// ANCHOR: program
/// The strategy the window runs: the drive's `Everything` leg edits this
/// line before it builds (M81/T81.4), so both strategies see a swap that
/// changes the program's shaping.
pub type Strategy = lumen_static_shell::program::DefaultInval; // EDIT strategy

/// The program, described once: the binary runs it, the library exports
/// it, and a reload boots the new build's copy of it (§2.2).
pub fn program() -> Program<Counter, Strategy> {
    with_shaders(
        application(boot)
            .invalidation::<Strategy>()
            .title("hot-window")
            .size(320, 400)
            .style(|_: &Counter| {
                #[allow(unused_mut)]
                let mut rules = vec![
                    Rule::new(Selector::Id("inc"), Patch::new().width(Length::FILL)),
                    Rule::new(Selector::Id("hebrew"), Patch::new().font_size(20.0)),
                    Rule::new(Selector::Id("star"), Patch::new().width(Length::FILL).height(Length::Fixed(48.0))),
                ];
                #[cfg(feature = "shader")]
                rules.push(Rule::new(Selector::Id("swatch"), Patch::new().width(Length::FILL).height(Length::Fixed(40.0))));
                rules
            })
            .shaping(Shaping::Basic), // EDIT shaping
    )
}

/// The program's shader table with `shader` (M83/T83.8): re-asked from the
/// new session on every swap and re-sent to the backend
/// (`Backend::set_shaders` in the window's swap branch). Kept out of the
/// chain above, whose last line the drive edits whole.
fn with_shaders(p: Program<Counter, Strategy>) -> Program<Counter, Strategy> {
    #[cfg(feature = "shader")]
    let p = p.shaders(&SHADERS);
    p
}

// Only a hot build is a library a window loads (M49/T49.9).
#[cfg(feature = "hot")]
lumen_static_reload::export_program!(program);
// ANCHOR_END: program
