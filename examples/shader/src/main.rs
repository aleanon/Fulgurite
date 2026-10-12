//! **An animated WGSL plasma behind a glass card** — M83/T83.7
//! (`docs/plan-gpu-extension.md`), what `ShaderView` is for.
//!
//! `cargo run -p shader --features gpu` runs the shader;
//! `cargo run -p shader --features shader` is the same window on the
//! software canvas, which draws the view's **fallback**. Every feature is
//! default-off and the binary requires `shader` — the manifest says why
//! (a workspace member's features unify into every `--workspace` build).
//! `--shot <file.ppm> [seconds]` renders one frame headlessly at that
//! time, which is the fallback too.
//!
//! # The three things this example is showing
//!
//! 1. **A shader is data on the program.** [`SHADERS`] is a table of
//!    `ShaderDesc`s — a name, WGSL with an `fs_main` and the size of its
//!    uniform block — registered with `Program::shaders`. A hot library
//!    re-sends its table on every swap, so an edited source reaches the
//!    GPU without a restart (T83.8 drives that). The view names its shader
//!    by index.
//! 2. **The time is state, and a binding puts it on screen.** `App::tick`
//!    advances [`Clock::t`]; the value form of `#[bind]` writes the
//!    uniform bytes through `ShaderView::set_uniforms`, whose answer is
//!    what marks the node. A tick that answers `true` marks the whole tree
//!    under `Boundaries` (`App::tick`'s rustdoc), so every animated frame
//!    re-lays the tree out: that price is recorded in
//!    `docs/results-shader.md` T83.7, and M84's narrow tick mark (its K84c) is
//!    where it is reopened.
//! 3. **The fallback is ordinary paint, clipped by the bracket.**
//!    [`fallback`] draws a dark field with four discs orbiting on the
//!    view's edge, half of each disc *outside* the box. The bracket is
//!    what clips them: a canvas that ignored `Cmd::Shader` would paint
//!    them over the page, which is what `just shader-drive`'s outside
//!    count exists to see. The centre of the box is never under a disc,
//!    so it is the field colour on every backend that cannot run the
//!    plasma, and a saturated plasma colour on one that does.
//!
//! The card is a `Glass` over the bottom of the view and the page below
//! it, declared after the view in a `stack` so it paints after it and
//! frosts it. Its button pauses the clock.

use fulgurite::display::{Color, Material, Painter, Rect, ShaderId};
use fulgurite::prelude::*;
use fulgurite_widgets::prelude::*;
use fulgurite_widgets::{Glass, ShaderView};
use fulgurite_winit::{application, Program, Run};

// ---- palette ---------------------------------------------------------

const PAGE: Color = Color(0x0b, 0x0e, 0x14, 0xff);
/// The fallback's field: what the centre of the view shows wherever the
/// plasma does not run. Dark and grey, so no plasma colour is near it
/// (the plasma's brightest channel is always at least 191; see
/// [`PLASMA`]).
pub const FIELD: Color = Color(0x1a, 0x1d, 0x2b, 0xff);
/// The orbiting discs, one per quarter turn.
const DISCS: [Color; 4] = [
    Color(0xff, 0x5f, 0x6d, 0xff),
    Color(0xff, 0xc3, 0x71, 0xff),
    Color(0x47, 0xcf, 0x73, 0xff),
    Color(0x4f, 0x9d, 0xff, 0xff),
];
const INK: Color = Color(0xf1, 0xf5, 0xfb, 0xff);
const SUB: Color = Color(0xc8, 0xd0, 0xdc, 0xff);
const GHOST: Color = Color(0x23, 0x2c, 0x3e, 0xff);
/// The card's tint: the page at 55%, so its white text keeps contrast
/// over the brightest plasma.
const TINT: Color = Color(0x0b, 0x0e, 0x14, 0x8c);

/// The disc radius in the fallback, logical px.
const DISC: f32 = 44.0;

// ---- the shader ------------------------------------------------------

// ANCHOR: table
/// The plasma: three travelling sine fields and a moving radial one,
/// summed, and turned into a colour by three sines a third of a turn
/// apart — so the channels always sum to 1.5 and the brightest is at
/// least 0.75 (191 of 255). The block is the time in seconds, padded to
/// the 16 bytes a WGSL uniform struct rounds to.
pub const PLASMA: &str = "
struct U { t: f32, _p0: f32, _p1: f32, _p2: f32 }
@group(0) @binding(1) var<uniform> u: U;

@fragment fn fs_main(in: FulguriteVarying) -> @location(0) vec4<f32> {
    let p = in.local * vec2(8.0, 4.8);
    let t = u.t;
    var v = sin(p.x + t) + sin((p.y + t) * 0.5) + sin((p.x + p.y + t) * 0.5);
    let c = p + vec2(sin(t / 3.0), cos(t / 2.0)) * 2.0;
    v = v + sin(sqrt(c.x * c.x + c.y * c.y + 1.0) + t);
    let a = v * 3.14159;
    return vec4(0.5 + 0.5 * sin(a), 0.5 + 0.5 * sin(a + 2.094), 0.5 + 0.5 * sin(a + 4.189), 1.0);
}
";

/// **The program's shader table.** Entry 0 is the plasma; the view names
/// it as `ShaderId(0)`.
pub static SHADERS: [fulgurite_shell::shader::ShaderDesc; 1] =
    [fulgurite_shell::shader::ShaderDesc::new("plasma", PLASMA, 16)];
// ANCHOR_END: table

/// What the view draws where the plasma does not run (the software
/// canvas, `--shot`, a GPU that refused the WGSL): the field, then four
/// discs orbiting on the box's edge at the uniform block's time. Half of
/// each disc lies outside the box, and only the bracket's clip keeps it
/// there; the centre is never covered.
pub fn fallback(out: &mut dyn Painter, r: Rect, u: &[u8]) {
    let t = u.get(..4).map_or(0.0, |b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    out.rect(r, FIELD);
    let (cx, cy) = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
    let (ax, ay) = ((r[2] - r[0]) / 2.0, (r[3] - r[1]) / 2.0);
    for (k, colour) in DISCS.iter().enumerate() {
        let a = t * 0.8 + k as f32 * core::f32::consts::FRAC_PI_2;
        let (x, y) = (cx + ax * a.cos(), cy + ay * a.sin());
        out.rounded_rect([x - DISC, y - DISC, x + DISC, y + DISC], *colour, DISC);
    }
}

// ---- the tree --------------------------------------------------------

/// The card's contents, inside the `Glass` so they paint over the frost.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct CardBody {
    #[widget_id("title")]
    title: Label,
    #[widget_id("sub")]
    sub: Label,
    #[widget_id("pause")]
    pause: Button<Clock, String>,
}

/// The card's layer: a `FILL` spacer pushes the card to the stage's
/// bottom, over the view's lower edge and the page below it.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Layer {
    #[widget_id("push")]
    push: Spacer,
    #[widget_id("card")]
    card: Glass<CardBody>,
}

/// A stack, so the card shares the view's rect and paints after it —
/// which is what lets the glass read, and frost, the plasma.
#[derive(StaticTree, Default)]
#[widget(stack)]
struct Stage {
    #[widget_id("plasma")]
    plasma: ShaderView<16>,
    layer: Layer,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Screen {
    #[widget_id("stage")]
    stage: Stage,
}

/// Advance the clock — `App::tick`. It writes state only; the binding
/// on [`Plasma::clock`] writes the bytes. Paused, it answers `false`, so
/// the window idles; a zero `dt` writes nothing (the tick contract).
fn advance(app: &mut Plasma, dt_ms: f32) -> bool {
    if app.clock.paused {
        return false;
    }
    if dt_ms > 0.0 {
        app.clock.t += dt_ms / 1000.0;
    }
    true
}

#[derive(DeriveApp, Default)]
#[app(tick = advance)]
struct Plasma {
    #[tree]
    ui: Screen,
    // ANCHOR: bind
    /// The time, and whether it runs. The value form of `#[bind]` writes
    /// [`Clock::uniforms`] through `ShaderView::set_uniforms`, which marks
    /// the view only when the bytes changed.
    #[state]
    #[bind(ui.stage.plasma, uniforms())]
    clock: Clock,
    // ANCHOR_END: bind
}

/// What a handler may touch: the clock.
#[derive(Default)]
pub struct Clock {
    /// Seconds since start, advanced by `App::tick`.
    pub t: f32,
    /// Set by the card's button; a paused clock does not tick.
    pub paused: bool,
}

impl Clock {
    /// The uniform block: `t` as a little-endian `f32`, then 12 bytes of
    /// pad (the WGSL struct's size).
    pub fn uniforms(&self) -> [u8; 16] {
        let mut b = [0; 16];
        b[..4].copy_from_slice(&self.t.to_le_bytes());
        b
    }
}

// ---- style -----------------------------------------------------------

fn stylesheet() -> Vec<Rule> {
    vec![
        Rule::new(
            Selector::Kind("Screen"),
            Patch::new().background(PAGE).padding(Edges { left: 40.0, right: 40.0, top: 40.0, bottom: 40.0 }),
        ),
        Rule::new(Selector::Id("stage"), Patch::new().height(Length::FILL)),
        Rule::new(Selector::Id("plasma"), Patch::new().width(Length::FILL).height(Length::Fixed(240.0))),
        Rule::new(
            Selector::Kind("Layer"),
            Patch::new().padding(Edges { left: 24.0, right: 24.0, top: 0.0, bottom: 0.0 }),
        ),
        Rule::new(Selector::Id("push"), Patch::new().height(Length::FILL)),
        Rule::new(
            Selector::Id("card"),
            Patch::new()
                .height(Length::Fixed(120.0))
                .radius(16.0)
                .padding(Edges { left: 18.0, right: 18.0, top: 14.0, bottom: 14.0 }),
        ),
        Rule::new(Selector::Id("title"), Patch::new().color(INK).font_size(17.0).font_weight(700.0)),
        Rule::new(Selector::Id("sub"), Patch::new().color(SUB).font_size(13.0)),
        Rule::new(
            Selector::Id("pause"),
            Patch::new()
                .background(GHOST)
                .color(INK)
                .radius(8.0)
                .font_size(14.0)
                .padding(Edges { left: 12.0, right: 12.0, top: 4.0, bottom: 4.0 }),
        ),
    ]
}

/// The window's logical size.
pub const W: u32 = 480;
/// The window's logical height.
pub const H: u32 = 400;

/// **The program, described once** (M32): the window, `--shot` and the
/// tests are all built from it, so all of them have the table.
fn program() -> Program<Plasma> {
    application(build).title("shader").size(W, H).page(PAGE).style(|_: &Plasma| stylesheet()).shaders(&SHADERS)
}

fn build() -> Plasma {
    let mut app = Plasma::default();
    let mut view = ShaderView::new(ShaderId(0), fallback);
    view.set_label("An animated plasma");
    app.ui.stage.plasma = view;
    app.ui.stage.layer.card = Glass::with(CardBody::default(), Material { blur: 14.0, tint: TINT, ..Material::FROSTED });
    let body = &mut app.ui.stage.layer.card.content;
    body.title.set("A WGSL plasma, under glass");
    body.sub.set("GPU: the shader. Software: its fallback.");
    body.pause.label = "Pause / play".into();
    body.pause.on_click = |c: &mut Clock, _: &EventCtx| c.paused = !c.paused;
    app
}

/// `--shot <file.ppm> [seconds]`: one frame, headless, at that time —
/// the fallback, as every host without the GPU draws it.
fn shot(path: &str, t: f32) {
    use fulgurite_shell::software::Canvas;
    use fulgurite_shell::text::{FontMetrics, FontStack};
    use fulgurite_shell::Host;

    let mut host: Host<Plasma> = program().host();
    host.app.clock.t = t;
    host.app.clock.paused = true;
    let mut canvas = match FontStack::system() {
        Some(f) => Canvas::with_font(W, H, f),
        None => Canvas::new(W, H),
    };
    match FontStack::system().map(FontMetrics::new).as_mut() {
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
}

fn main() {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("--shot") {
        let path = args.next().unwrap_or_else(|| "shader.ppm".into());
        let t = args.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
        shot(&path, t);
        return;
    }
    // Built `--features agent`, the endpoint is `FULGURITE_AGENT_ADDR`, read by
    // the window; inert otherwise.
    #[cfg(feature = "everything")]
    program().invalidation::<fulgurite::Everything>().run();
    #[cfg(not(feature = "everything"))]
    program().run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use fulgurite::display::{Cmd, DisplayList};
    use fulgurite_shell::software::Canvas;
    use fulgurite_shell::Host;

    fn pumped(t: f32) -> Host<Plasma> {
        let mut host: Host<Plasma> = program().host();
        host.app.clock.t = t;
        host.pump();
        host
    }

    /// M25/T25.2: no rule in the sheet is a no-op.
    #[test]
    fn every_stylesheet_rule_changes_something() {
        let host = pumped(0.0);
        let sheet = host.stylesheet();
        let found: Vec<String> = host
            .sheet_findings()
            .iter()
            .map(|f| format!("rule {} {:?}: {:?}", f.rule, sheet[f.rule].selector, f.issue))
            .collect();
        println!("{} rules, {} findings", sheet.len(), found.len());
        assert!(!sheet.is_empty(), "setup: the sheet was applied");
        assert!(found.is_empty(), "stylesheet rules that change nothing:\n{}", found.join("\n"));
    }

    /// The program carries the table, and the view's bracket carries the
    /// clock's bytes through the binding: a tick moves them.
    #[test]
    fn a_tick_reaches_the_bracket_through_the_binding() {
        let mut host = pumped(0.0);
        assert_eq!(host.shaders().descs().iter().map(|d| d.name()).collect::<Vec<_>>(), ["plasma"]);
        let bytes = |h: &Host<Plasma>| -> Vec<u8> {
            let l: &DisplayList = &h.list;
            let u = l
                .cmds
                .iter()
                .find_map(|c| match c {
                    Cmd::Shader { id, uniforms, .. } if *id == ShaderId(0) => Some(*uniforms),
                    _ => None,
                })
                .expect("the view paints a bracket");
            l.uniforms_of(u).to_vec()
        };
        let before = bytes(&host);
        assert!(<Plasma as fulgurite::event::App>::tick(&mut host.app, 500.0), "an unpaused clock animates");
        host.pump();
        let after = bytes(&host);
        println!("uniforms before {before:?}, after {after:?}");
        assert_eq!(&after[..4], &0.5f32.to_le_bytes(), "the bracket carries t = 0.5 s");
        assert_ne!(before, after);
        host.app.clock.paused = true;
        assert!(!<Plasma as fulgurite::event::App>::tick(&mut host.app, 500.0), "a paused clock idles");
    }

    /// The fallback's discs overflow the box, and the bracket clips them:
    /// the canvas shows no disc colour outside the view, and the centre is
    /// the field.
    #[test]
    fn the_fallback_is_clipped_to_the_view() {
        // At t = 0 the discs sit at the four edge midpoints, so one
        // overflows each side.
        let host = pumped(0.0);
        let mut c = Canvas::new(W, H);
        c.clear(PAGE);
        host.render(&mut c);
        let ppm = c.to_ppm();
        let rgb = &ppm[ppm.len() - (W * H * 3) as usize..];
        let px = |x: u32, y: u32| {
            let i = ((y * W + x) * 3) as usize;
            Color(rgb[i], rgb[i + 1], rgb[i + 2], 0xff)
        };
        let at = Screen::SUB_TABLE.iter().position(|d| d.wid == "plasma").expect("the view");
        let r = host.states[at].pos;
        println!("view at ({}, {}); centre {:?}", r.x, r.y, px(240, 160));
        assert_eq!(px(240, 160), FIELD, "the view's centre is the fallback's field");
        // Strips of page left of and above the view, where the discs on
        // those edges would overflow unclipped.
        let left = (2..38).flat_map(|x| (60..260).map(move |y| (x, y)));
        let top = (60..420).flat_map(|x| (2..38).map(move |y| (x, y)));
        let outside = left.chain(top).filter(|&(x, y)| px(x, y) != PAGE).count();
        println!("non-page pixels left of the view: {outside} (bound 0)");
        assert_eq!(outside, 0, "a disc was painted outside the view: the bracket did not clip");
    }
}
