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
//! `#pic` (M84/T84.6) is an `Image` whose pixels come from the
//! application's own store (`#[app(images = ..)]`, `App::images`), which
//! the renderer reads through the frame: the drive edits `EDIT pic`, the
//! store's colour, and after the swap the region must show the new build's
//! pixels — the loader never holds the store, it re-asks the session each
//! present. Under a failed build's bar the region keeps its pixels: the
//! bar's frame forwards the session's images. Like `#star` it sits above
//! the bar's reach.
//! `#open-panel` (M89/T89.7) opens the **panel**, program 1 of the window
//! table: a second window of a different size, with its own kept count
//! (`#panel-count`, formatted by the line marked `EDIT panel`), a bump
//! button and a 150 ms `Timers` tick. The drive opens it before the first
//! swap and after one, and plants a panic in its boot (`EDIT panel-boot`)
//! to see a refused swap leave both windows on the build they run.
//! `MENU` (M90/T90.5) is the program's one-entry menu, installed with
//! `Program::menu`: Bump answers Ctrl+K, and the drive edits `EDIT accel`
//! to Ctrl+J. After the swap Ctrl+J must reach `on_menu` and Ctrl+K must
//! not — the accelerators are the new build's, installed by
//! `Program::host`, never a table the loader kept.

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
    /// An image from the application's store (M84/T84.6).
    #[widget_id("pic")]
    pub pic: Image,
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
    /// Opens the panel (M89/T89.7): `Request::OpenWindow { program: 1 }`.
    #[widget_id("open-panel")]
    pub open: Button<Tally>,
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
    /// Where `#open-panel` asks for the panel (M89/T89.7). Not kept: a
    /// request is answered inside the drain that took it, so none is
    /// pending at a swap.
    #[serde(skip)]
    pub requests: lumen_static::request::Requests,
}

/// `#pic`'s id in the application's store.
pub const PIC: lumen_static::ImageId = lumen_static::ImageId(1);

#[derive(DeriveApp, DeriveReloadable)]
#[app(images = images, requests = requests)]
pub struct Counter {
    #[tree]
    pub ui: Ui,
    #[state]
    #[reload(keep)]
    #[bind(ui.value, count, "count {}")] // EDIT format
    pub tally: Tally,
    /// The images this build owns: `#pic`'s pixels. Fresh on a swap — the
    /// new build's boot fills it, which is how an edited asset arrives.
    #[reload(fresh)]
    pub store: lumen_static_shell::image::ImageStore,
}

/// The store the renderer reads through the frame (`App::images`).
fn images(c: &Counter) -> Option<&dyn lumen_static::ImageSource> {
    Some(&c.store)
}

/// The queue `#open-panel` pushes onto (`App::requests`, M89/T89.7).
fn requests(c: &mut Counter) -> Option<&mut lumen_static::request::Requests> {
    Some(&mut c.tally.requests)
}

fn on_open(t: &mut Tally, _: &EventCtx) {
    t.requests.push(lumen_static::request::Request::OpenWindow { program: PANEL, tag: 1 });
}

/// `#pic`'s colour in this build: the line the drive edits.
pub fn pic_ink() -> [u8; 4] {
    [0x16, 0x7a, 0x3c, 0xff] // EDIT pic
}

/// The program's menu (M90/T90.5): one entry, its accelerator on the line
/// the drive edits.
const TOOLS: [lumen_static::menu::MenuEntry; 1] = [
    lumen_static::menu::MenuEntry::new("Bump", 1).accel(lumen_static::Key::Char('k'), lumen_static::Mods::CTRL), // EDIT accel
];
pub const MENU: [lumen_static::menu::MenuSection; 1] = [lumen_static::menu::MenuSection { title: "Tools", entries: &TOOLS }];

/// Every entry enabled.
fn menu_enabled(_: &Tally) -> lumen_static::menu::MenuMask {
    lumen_static::menu::MenuMask::ALL
}

/// A chosen entry: handled, so the drive reads `handled: true` for the
/// accelerator and `false` for a key no entry answers.
fn on_menu(_: &mut Tally, _id: u32) -> lumen_static_shell::program::KeyOutcome {
    lumen_static_shell::program::KeyOutcome::Handled
}

fn on_inc(t: &mut Tally, _: &EventCtx) {
    let step = 1; // EDIT handler
    t.count += step;
}

/// The boot: the one place a `Counter` is built — by `main`, by the
/// library a reload loads, and by `Host::reload` (M49/T49.4).
fn boot() -> Counter {
    let mut c = Counter { ui: Ui::default(), tally: Tally::default(), store: Default::default() };
    let ink = pic_ink();
    c.store.insert(PIC, 8, 8, vec![ink.iter().copied().cycle().take(8 * 8 * 4).collect()]);
    c.ui.pic = Image::new(PIC, lumen_static::desc::Size { w: 8.0, h: 8.0 });
    c.ui.pic.fit = lumen_static_widgets::Fit::Stretch;
    c.ui.pic.tint = lumen_static::Color(0xff, 0xff, 0xff, 0xff);
    c.ui.inc.label = "+";
    c.ui.inc.on_click = on_inc;
    c.ui.hebrew.set(HEBREW.to_owned());
    c.ui.open.label = "panel";
    c.ui.open.on_click = on_open;
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

// ---- the panel: program 1 of the window table (M89/T89.7) ---------------

/// The panel's index in the window table.
pub const PANEL: u16 = 1;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 6.0)]
pub struct PanelUi {
    /// The kept count, in the format the `EDIT panel` line writes.
    #[widget_id("panel-count")]
    pub count: Label<String>,
    #[widget_id("panel-bump")]
    pub bump: Button<Panel>,
    /// The timer's ticks: it keeps the panel drawing on its own.
    #[widget_id("panel-ticks")]
    pub ticks: Label<String>,
    /// Fills the rest of the window, so the root's box is the window's
    /// logical size — what the drive compares with the window it is in.
    #[widget_id("panel-fill")]
    pub fill: Spacer,
}

/// What the panel keeps across a swap.
#[derive(Default, Serialize, Deserialize)]
pub struct PanelKept {
    pub count: i64,
}

/// The panel. Unsplit, and its `App` written by hand, because its timer
/// (`App::timers`) is not something the derive offers.
#[derive(DeriveReloadable)]
pub struct Panel {
    #[reload(fresh)]
    pub ui: PanelUi,
    #[reload(keep)]
    pub kept: PanelKept,
    #[reload(fresh)]
    ticks: u64,
    #[reload(fresh)]
    outbox: lumen_static::spawn::Outbox,
    #[reload(fresh)]
    timers: lumen_static::spawn::Timers,
}

/// The panel's timer: a `Timers` firing is a job whose result wakes the
/// panel's window through **its** keyed waker.
pub const PANEL_TICK: u64 = 7;

/// The panel's message: a tick arrived.
pub struct PanelTick;

fn panel_tick(_: lumen_static::spawn::Descriptor, tx: &lumen_static::inbox::Sender<PanelTick>) {
    let _ = tx.send(PanelTick);
}

fn panel_text(p: &Panel) -> String {
    format!("panel {}", p.kept.count) // EDIT panel
}

impl lumen_static::event::App for Panel {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self {
        self
    }
    type Tree = PanelUi;
    type Msg = PanelTick;
    fn tree(&self) -> &PanelUi {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut PanelUi {
        &mut self.ui
    }
    fn outbox(&mut self) -> Option<&mut lumen_static::spawn::Outbox> {
        Some(&mut self.outbox)
    }
    fn timers(&mut self) -> Option<&mut lumen_static::spawn::Timers> {
        Some(&mut self.timers)
    }
    fn job_for(d: lumen_static::spawn::Descriptor) -> Option<lumen_static::spawn::JobFn<PanelTick>> {
        (d.kind == PANEL_TICK).then_some(panel_tick as lumen_static::spawn::JobFn<PanelTick>)
    }
    fn apply(this: &mut Self, _: PanelTick) -> bool {
        this.ticks += 1;
        this.ui.ticks.set(format!("tick {}", this.ticks));
        true
    }
    /// The count's label, from the kept count, in this build's format —
    /// after a swap, the restored count in the new build's words.
    fn update(&mut self) -> bool {
        let text = panel_text(self);
        if self.ui.count.as_str() == text {
            return false;
        }
        self.ui.count.set(text);
        true
    }
}

fn panel_boot() -> Panel {
    let _ = (); // EDIT panel-boot
    let mut p = Panel {
        ui: PanelUi::default(),
        kept: PanelKept::default(),
        ticks: 0,
        outbox: Default::default(),
        timers: Default::default(),
    };
    p.ui.bump.label = "+";
    p.ui.bump.on_click = |p, _| p.kept.count += 1;
    p.ui.ticks.set("tick 0".into());
    p.timers.every(PANEL_TICK, 1, 150.0);
    p
}

/// The panel, described: a window program of its own size.
pub fn panel() -> Program<Panel, Strategy> {
    application(panel_boot)
        .invalidation::<Strategy>()
        .title("hot-window — panel")
        .size(260, 180)
        .style(|_: &Panel| vec![Rule::new(Selector::Id("panel-fill"), Patch::new().width(Length::FILL).height(Length::FILL))])
}

/// The window table: entry 1 is the panel.
pub fn windows(index: u16) -> Option<Box<dyn lumen_static_shell::program::OpenProgram>> {
    match index {
        PANEL => Some(Box::new(panel())),
        _ => None,
    }
}

// ANCHOR: program
/// The strategy the window runs: the drive's `Everything` leg edits this
/// line before it builds (M81/T81.4), so both strategies see a swap that
/// changes the program's shaping.
#[cfg(not(feature = "everything"))]
pub type Strategy = lumen_static_shell::program::DefaultInval; // EDIT strategy
/// …and `Everything` with the `everything` feature (M89/T89.7), for the
/// drive's windows leg under both strategies.
#[cfg(feature = "everything")]
pub type Strategy = lumen_static::Everything;

/// The program, described once: the binary runs it, the library exports
/// it, and a reload boots the new build's copy of it (§2.2).
pub fn program() -> Program<Counter, Strategy> {
    with_shaders(
        application(boot)
            .invalidation::<Strategy>()
            .title("hot-window")
            .size(320, 400)
            .windows(windows)
            .menu(&MENU, menu_enabled)
            .on_menu(on_menu)
            .style(|_: &Counter| {
                #[allow(unused_mut)]
                let mut rules = vec![
                    Rule::new(Selector::Id("inc"), Patch::new().width(Length::FILL)),
                    Rule::new(Selector::Id("hebrew"), Patch::new().font_size(20.0)),
                    Rule::new(Selector::Id("star"), Patch::new().width(Length::FILL).height(Length::Fixed(48.0))),
                    Rule::new(Selector::Id("pic"), Patch::new().width(Length::FILL).height(Length::Fixed(32.0))),
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
