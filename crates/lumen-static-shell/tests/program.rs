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
