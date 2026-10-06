//! **A program with no window of its own** — M89/T89.6
//! (`docs/plan-windows.md`, "A daemon is `daemon(boot)`").
//!
//! `daemon(boot)` describes a program as `application(boot)` does, and the
//! window crate opens no window for it: its session runs **windowless**, as
//! window 0, pumped by its own deadline and drained after every frame. This
//! one counts 120 ms ticks into `#ticks`, a label on a tree nobody sees; at
//! tick 10 it opens its panel (program 1 of its window table), at tick 20 it
//! closes it, and at tick 40 it ends the process with `Request::Quit`.
//! Closing its last window ends nothing: a daemon's windows are all table
//! entries.
//!
//! **Its timer is its own deadline**, `App::wake_in_ms` and `App::tick`,
//! not `Timers::every`: a `Timers` firing is delivered by a job, so it costs
//! two frames a period — the deadline's, which spawns the job, and the wake's,
//! which applies its result (`docs/results-frame-cost.md`, "Idle") — and a
//! daemon that only counts needs one.
//!
//! Escape — sent by the agent as `input.key` on window 0 — reaches its key
//! hook, which answers `KeyOutcome::Quit`. A windowless session has no
//! window for that to close, so the window crate ignores it, with one line
//! on stderr naming `Request::Quit` (second review A21).
//!
//! `daemon --steady` ticks and never opens, closes or quits;
//! `daemon --still` does not tick at all — the two legs of `just
//! idle-count`'s daemon phase (K89d).

use lumen_static::prelude::*;
use lumen_static::request::{Answer, Request, Requests};
use lumen_static_shell::program::{application, daemon, KeyOutcome, OpenProgram, Program};
use lumen_static_widgets::prelude::*;

/// The tick period, in milliseconds.
pub const PERIOD_MS: f64 = 120.0;
/// The tick at which the panel opens, closes, and the daemon quits.
pub const OPEN_AT: u64 = 10;
pub const CLOSE_AT: u64 = 20;
pub const QUIT_AT: u64 = 40;
/// The panel's index in the window table.
pub const PANEL: u16 = 1;

/// What the daemon does with its ticks: `argv[1]` — `--steady` and
/// `--still` for the idle legs, the schedule otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    /// Open at 10, close at 20, quit at 40.
    #[default]
    Scripted,
    /// Tick, and nothing else.
    Steady,
    /// No deadline at all.
    Still,
}

impl Mode {
    pub fn from_args() -> Mode {
        match std::env::args().nth(1).as_deref() {
            Some("--steady") => Mode::Steady,
            Some("--still") => Mode::Still,
            _ => Mode::Scripted,
        }
    }
}

// ---- the daemon: program 0, windowless ---------------------------------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
pub struct DaemonUi {
    /// The ticks counted so far.
    #[widget_id("ticks")]
    pub ticks: Label<String>,
    /// The panel's state: "no panel", "panel N" or "closed N".
    #[widget_id("panel")]
    pub panel: Label<String>,
}

/// The daemon's state. Unsplit (`State = Self`), so its tick writes its
/// labels itself.
#[derive(Default)]
pub struct Daemon {
    pub ui: DaemonUi,
    requests: Requests,
    mode: Mode,
    /// Milliseconds since boot, as the ticks saw them.
    elapsed_ms: f64,
    ticks: u64,
    /// The panel's window key while it is open.
    open: Option<u32>,
}

/// What the window's answers become for the daemon.
pub enum DaemonMsg {
    Opened(u32),
    Closed(u32),
}

impl Daemon {
    /// Tick `n` came due: the schedule's request for it, if any.
    fn due(&mut self, n: u64) {
        if self.mode != Mode::Scripted {
            return;
        }
        match n {
            OPEN_AT => {
                self.requests.push(Request::OpenWindow { program: PANEL, tag: OPEN_AT });
            }
            CLOSE_AT => {
                if let Some(window) = self.open {
                    self.requests.push(Request::CloseWindow { window });
                }
            }
            QUIT_AT => {
                self.requests.push(Request::Quit);
            }
            _ => {}
        }
    }
}

impl App for Daemon {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self {
        self
    }
    type Tree = DaemonUi;
    type Msg = DaemonMsg;
    fn tree(&self) -> &DaemonUi {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut DaemonUi {
        &mut self.ui
    }
    fn requests(&mut self) -> Option<&mut Requests> {
        Some(&mut self.requests)
    }
    // ANCHOR: tick
    /// **Count the periods that passed**, catching up if a frame came late,
    /// and make each one's request. Writes the tree, so it answers `true`
    /// on a change and only then (`App::tick`'s contract under
    /// `Boundaries`).
    fn tick(&mut self, dt_ms: f32) -> bool {
        if self.mode == Mode::Still || dt_ms <= 0.0 {
            return false;
        }
        self.elapsed_ms += dt_ms as f64;
        let now = (self.elapsed_ms / PERIOD_MS) as u64;
        if now == self.ticks {
            return false;
        }
        for n in self.ticks + 1..=now {
            self.due(n);
        }
        self.ticks = now;
        self.ui.ticks.set(format!("{now}"));
        true
    }

    /// The next period's instant: the deadline the windowless session is
    /// pumped at.
    fn wake_in_ms(&self) -> Option<f32> {
        (self.mode != Mode::Still).then(|| (PERIOD_MS - self.elapsed_ms % PERIOD_MS) as f32)
    }
    // ANCHOR_END: tick
    fn answered(_tag: u64, a: Answer<'_>) -> Option<DaemonMsg> {
        match a {
            Answer::WindowOpened { window } => Some(DaemonMsg::Opened(window)),
            Answer::WindowClosed { window } => Some(DaemonMsg::Closed(window)),
            _ => None,
        }
    }
    fn apply(this: &mut Self, m: DaemonMsg) -> bool {
        match m {
            DaemonMsg::Opened(window) => {
                this.open = Some(window);
                this.ui.panel.set(format!("panel {window}"));
            }
            DaemonMsg::Closed(window) => {
                this.open = None;
                this.ui.panel.set(format!("closed {window}"));
            }
        }
        true
    }
}

fn boot() -> Daemon {
    let mut d = Daemon { mode: Mode::from_args(), ..Default::default() };
    d.ui.ticks.set("0".into());
    d.ui.panel.set("no panel".into());
    d
}

/// Escape asks to close: `KeyOutcome::Quit`. A windowless session has no
/// window to close, so the window ignores it and says so (A21).
fn keys(_: &mut Daemon, key: lumen_static::Key, _: lumen_static::Mods) -> KeyOutcome {
    match key {
        lumen_static::Key::Escape => KeyOutcome::Quit,
        _ => KeyOutcome::Ignored,
    }
}

// ---- the panel: program 1, a window ------------------------------------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
pub struct PanelUi {
    #[widget_id("panel-title")]
    pub title: Label,
}

#[derive(Default)]
pub struct Panel {
    pub ui: PanelUi,
}

impl App for Panel {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self {
        self
    }
    type Tree = PanelUi;
    type Msg = ();
    fn tree(&self) -> &PanelUi {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut PanelUi {
        &mut self.ui
    }
    fn apply(_: &mut Self, _: ()) -> bool {
        false
    }
}

fn panel_state() -> Panel {
    let mut p = Panel::default();
    p.ui.title.set("the daemon's panel");
    p
}

/// The panel, described: a window like any other, opened by the daemon's
/// window table.
pub fn panel() -> Program<Panel> {
    application(panel_state).title("daemon — panel").size(240, 100)
}

// ANCHOR: daemon
/// **The window table**: entry 1 is the panel.
pub fn windows(index: u16) -> Option<Box<dyn OpenProgram>> {
    match index {
        PANEL => Some(boxed(panel())),
        _ => None,
    }
}

/// **The daemon, described**: `daemon(boot)` where a windowed program says
/// `application(boot)`. No size, no page: it has no window.
pub fn program() -> Program<Daemon> {
    daemon(boot).title("daemon").on_key(keys).windows(windows)
}
// ANCHOR_END: daemon

/// The panel under the build's strategy: `Everything` with the
/// `everything` feature, so the drive runs both.
fn boxed(p: Program<Panel>) -> Box<dyn OpenProgram> {
    #[cfg(feature = "everything")]
    return Box::new(p.invalidation::<lumen_static::Everything>());
    #[cfg(not(feature = "everything"))]
    Box::new(p)
}
