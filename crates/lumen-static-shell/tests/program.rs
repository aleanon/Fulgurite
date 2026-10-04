//! **One description, one path** — M32/T32.0 (`docs/plan-program.md`).
//!
//! `Program::host` is the function the window drives *and* the function
//! every headless frame is rendered from. These tests go through it, not
//! around it, and each assertion names the property it would lose.

use lumen_static::display::Color;
use lumen_static::event::App;
use lumen_static::prelude::*;
use lumen_static::spawn::{Descriptor, Immediate, Job};
use lumen_static::style::{Patch, Rule, Selector};
use lumen_static_shell::program::{application, Program};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("panel")]
    panel: Fixed,
}

#[derive(Default)]
struct Sheet1 {
    ui: Ui,
    /// What the boot was given — a captured argument.
    width: f32,
    landed: Vec<u64>,
}

impl App for Sheet1 {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self::State {
        self
    }
    type Tree = Ui;
    type Msg = u64;
    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
    fn apply(this: &mut Self, m: u64) -> bool {
        this.landed.push(m);
        true
    }
}

const MARK: Color = Color(0x12, 0x9a, 0x5e, 0xff);

/// A rule nothing else sets, sized by what the boot captured, so both the
/// sheet and the boot's argument are visible in one resolved node.
fn sheet(app: &Sheet1) -> Vec<Rule> {
    vec![Rule::new(
        Selector::Id("panel"),
        Patch::new().background(MARK).width(Length::Fixed(app.width)).height(Length::Fixed(20.0)),
    )]
}

fn panel(h: &Host<Sheet1>) -> (Color, f32) {
    let i = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "panel").expect("#panel");
    (h.states[i].bg, h.states[i].size.w)
}

fn program(width: f32) -> Program<Sheet1> {
    // A capturing boot: `width` comes from outside, as showcase's argv
    // entry does. The prototype refused this; boot is cold (plan §2.1).
    application(move || Sheet1 { width, ..Sheet1::default() }).size(200, 100).style(sheet)
}

#[test]
fn the_host_a_program_builds_is_styled_by_the_program() {
    let mut host: Host<Sheet1> = program(73.0).host();
    host.pump();
    let (bg, w) = panel(&host);
    println!("panel bg {bg:?} (rule {MARK:?}), width {w} (boot captured 73)");
    assert_eq!(
        bg, MARK,
        "M32: `Program::host` must install the program's sheet — it is the only place a \
         sheet comes from, so a host without it is unstyled everywhere"
    );
    assert_eq!(w, 73.0, "the captured boot argument must reach the application");
}

/// **The control**: the same application built around `host()`. Unstyled
/// — which is what every headless frame path was before M32, and why a
/// byte-identity gate over them could not see the window lose its sheet.
#[test]
fn a_host_built_around_the_program_is_not_styled_by_it() {
    let mut host: Host<Sheet1> = Host::new(Sheet1 { width: 73.0, ..Sheet1::default() }, 200, 100);
    host.pump();
    let (bg, _) = panel(&host);
    assert_ne!(bg, MARK, "if this matched, something other than `host()` installs the sheet");
}

/// `adopt` re-asks the program's sheet of the *new* application.
#[test]
fn adopting_an_application_re_asks_the_programs_sheet() {
    let mut host: Host<Sheet1> = program(73.0).host();
    host.pump();
    host.adopt(Sheet1 { width: 41.0, ..Sheet1::default() });
    host.pump();
    let (bg, w) = panel(&host);
    println!("after adopt: bg {bg:?}, width {w} (new application wants 41)");
    assert_eq!(w, 41.0, "the sheet was not re-asked of the adopted application");
    assert_eq!(bg, MARK);
}

fn double(d: Descriptor, tx: &lumen_static::inbox::Sender<u64>) {
    let _ = tx.send(d.payload * 2);
}

/// A boot may return jobs — iced's `(State, Task)`. They wait for a
/// spawner, because `host()` runs before a window installs one.
#[test]
fn a_boots_jobs_start_when_a_spawner_is_installed() {
    const JOB: Job<u64> =
        Job { desc: Descriptor { kind: 1, step: 0, payload: 21 }, job: double, on_fail: None };
    let p: Program<Sheet1> = application(|| (Sheet1::default(), vec![JOB]));
    let mut host: Host<Sheet1> = p.host();
    assert_eq!(host.held_jobs(), 1, "the boot's job must be held, not refused");
    assert_eq!(host.unspawned(), 0, "held is not refused");
    host.pump();
    assert!(host.app.landed.is_empty(), "nothing can run without a spawner");

    host.set_spawner(Box::new(Immediate));
    assert_eq!(host.held_jobs(), 0, "installing a spawner starts what was held");
    host.pump();
    assert_eq!(host.app.landed, vec![42], "the boot's job ran and its result was applied");
}

/// A title is a constant or a function of the state.
#[test]
fn a_title_may_be_a_function_of_the_state() {
    let p = program(10.0).title(|a: &Sheet1| format!("w={}", a.width));
    let host: Host<Sheet1> = p.host();
    assert_eq!(p.title_of(&host.app), "w=10");
    let q = program(10.0).title("fixed");
    assert_eq!(q.title_of(&host.app), "fixed");
}

/// **`Program::shaping` reaches the session and its canvas** — M81/T81.4
/// (`docs/plan-shaped-paint.md`). A program with `.shaping(Complex)` and a
/// Hebrew label, through `Local` and an `Offscreen` built from the
/// session's own `Session::shaping`: the label's band must equal a direct
/// `Canvas` render on a `Complex` stack and differ from a `Basic` one.
/// Needs a system face with Hebrew; skips without one unless
/// `LUMEN_REQUIRE_FONTS=1`.
#[cfg(all(feature = "truetype", feature = "shaping"))]
mod complex {
    use super::*;
    use lumen_static::display::Cmd;
    use lumen_static::{App as DeriveApp, StaticTree};
    use lumen_static_shell::session::{Crashing, Local, Offscreen, Screen, Session};
    use lumen_static_shell::software::Canvas;
    use lumen_static_shell::text::{FontMetrics, FontStack, Shaping};

    const W: u32 = 360;
    const H: u32 = 80;
    const HEBREW: &str = "שלום עולם";

    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct HeUi {
        #[widget_id("he")]
        he: Label<String>,
    }

    #[derive(DeriveApp, Default)]
    struct He {
        #[tree]
        ui: HeUi,
    }

    fn boot() -> He {
        let mut a = He::default();
        a.ui.he.set(HEBREW.to_owned());
        a
    }

    fn described(shaping: Shaping) -> Program<He> {
        application(boot)
            .size(W, H)
            .style(|_: &He| vec![Rule::new(Selector::Id("he"), Patch::new().font_size(32.0))])
            .shaping(shaping)
    }

    fn label(states: &[lumen_static::NodeState]) -> [f32; 4] {
        let i = lumen_static::style::index_of_wid(<HeUi as Widget>::SUB_TABLE, "he").expect("#he");
        let s = &states[i];
        [s.pos.x, s.pos.y, s.pos.x + s.size.w, s.pos.y + s.size.h]
    }

    /// The label's band: every row its box covers, across the canvas.
    fn band(pixels: &[u8], r: [f32; 4]) -> Vec<u8> {
        let (y0, y1) = (r[1].floor().max(0.0) as usize, (r[3].ceil() as usize).min(H as usize));
        pixels[y0 * W as usize * 4..y1 * W as usize * 4].to_vec()
    }

    fn differing(a: &[u8], b: &[u8]) -> usize {
        assert_eq!(a.len(), b.len(), "bands of different heights");
        a.chunks_exact(4).zip(b.chunks_exact(4)).filter(|(p, q)| p != q).count()
    }

    /// Through the session, as a window or a headless drive renders.
    fn through_session(shaping: Shaping) -> (Shaping, [f32; 4], Vec<u8>, bool) {
        let mut s = Local::from_program(described(shaping), Crashing);
        let mut screen = Offscreen::with_shaping(W, H, s.shaping());
        s.frame(0.0, W, H);
        s.frame(0.0, W, H);
        let page = s.page();
        screen.present(&mut s, page);
        let list = &s.host.list;
        let has_text = list.cmds.iter().any(|c| matches!(c, Cmd::Text { text, .. } if list.str_of(*text) == HEBREW));
        let r = label(&s.host.states);
        (s.shaping(), r, screen.canvas.pixels.clone(), has_text)
    }

    /// By hand: one stack in `shaping`, a measurer and a canvas from it.
    fn direct(stack: &FontStack, shaping: Shaping) -> ([f32; 4], Vec<u8>) {
        let stack = stack.clone().with_shaping(shaping);
        let p = described(Shaping::Basic);
        let page = p.page;
        let mut host: Host<He> = p.host();
        let mut m = FontMetrics::new(stack.clone());
        host.pump_with(&mut m);
        host.pump_with(&mut m);
        let mut c = Canvas::with_font(W, H, stack);
        c.clear(page);
        host.render(&mut c);
        (label(&host.states), c.pixels)
    }

    #[test]
    fn a_complex_program_is_measured_and_drawn_shaped() {
        let Some(stack) = FontStack::system() else {
            assert!(std::env::var("LUMEN_REQUIRE_FONTS").as_deref() != Ok("1"), "LUMEN_REQUIRE_FONTS=1 but no system face");
            println!("no system face -- skipped");
            return;
        };
        let (mode, r, session_px, has_text) = through_session(Shaping::Complex);
        let (rc, complex_px) = direct(&stack, Shaping::Complex);
        let (rb, basic_px) = direct(&stack, Shaping::Basic);
        println!("session: Session::shaping {mode:?}, label {r:?}; direct Complex label {rc:?}, Basic label {rb:?}");
        assert!(has_text, "setup: the session's list holds no Cmd::Text with {HEBREW:?}");
        let band_s = band(&session_px, r);
        let to_complex = differing(&band_s, &band(&complex_px, r));
        let to_basic = differing(&band_s, &band(&basic_px, r));
        let ink = band_s.chunks_exact(4).filter(|p| p[0] < 128).count();
        println!(
            "label band {} px, {ink} inked: session vs direct Complex {to_complex} differing (bound 0), \
             vs direct Basic {to_basic} differing (bound > 0)",
            band_s.len() / 4
        );
        assert!(ink > 0, "setup: the label inked nothing");
        assert!(to_basic > 0, "the session's frame equals a Basic render: the program's mode never reached it");
        assert_eq!(to_complex, 0, "the session's frame is not a Complex stack's render of the label");
        assert_eq!(r, rc, "the session laid the label out in another box than a Complex stack measures");
        assert_eq!(mode, Shaping::Complex, "the session does not report its program's `Shaping::Complex`");
    }

    /// The accept direction: a program that does not ask for `Complex`
    /// renders exactly as a `Basic` stack does.
    #[test]
    fn a_basic_program_is_drawn_as_before() {
        let Some(stack) = FontStack::system() else { return };
        let (mode, r, session_px, _) = through_session(Shaping::Basic);
        let (_, basic_px) = direct(&stack, Shaping::Basic);
        let d = differing(&band(&session_px, r), &band(&basic_px, r));
        println!("Basic program: Session::shaping {mode:?}; session vs direct Basic {d} differing (bound 0)");
        assert_eq!(mode, Shaping::Basic);
        assert_eq!(d, 0);
    }
}
