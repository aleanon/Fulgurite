//! **A tray application** — M90/T90.7 (`docs/plan-desktop-integration.md`,
//! "The Linux tray is D-Bus, not GTK").
//!
//! A daemon (`daemon(boot)`, M89/T89.6): no window of its own, its session
//! the windowless one, key 0, for the whole process. Its only surface is a
//! tray item declared as data with `Program::tray` — a 16 × 16 icon, a
//! tooltip and three entries:
//!
//! - **Open** pushes `Request::OpenWindow { program: 1, tag }`, and the
//!   window's key comes back in `Answer::WindowOpened`. Disabled while a
//!   window is open and while locked.
//! - **Lock** toggles `locked`. Locking closes the open window with
//!   `Request::CloseWindow { window }`, the key from that answer — the
//!   window's own session is never told its key (`WindowOpened` goes to
//!   the opener, `docs/plan-windows.md`). Its state is what `enabled`
//!   reads, so toggling it flips Open's dbusmenu `enabled` property.
//! - **Quit** pushes `Request::Quit`, which ends the process. **Not
//!   `KeyOutcome::Quit`**: from the windowless session that closes nothing
//!   (second review A21), and the window crate says so on stderr.
//!
//! The window (program 1) has a **Close** button that pushes
//! `Request::Close`, which closes the window it came from; the daemon hears
//! `Answer::WindowClosed` and runs on. Closing its last window ends
//! nothing.
//!
//! A chosen entry reaches [`on_menu`] as its id: `ksni`'s thread sends it
//! through the event loop's proxy, and the window posts it as
//! `Event::Menu` to the windowless session — **the OS path**, which `just
//! tray-drive` drives by sending the item's dbusmenu `Event(id,
//! "clicked")`. `#status` and `app.tray` show the result.

use fulgurite::menu::{MenuEntry, MenuMask};
use fulgurite::prelude::*;
use fulgurite::request::{Answer, Request, Requests};
use fulgurite_shell::program::{application, daemon, KeyOutcome, OpenProgram, Program};
use fulgurite_shell::tray::TraySpec;
use fulgurite_widgets::prelude::*;

/// The entries' ids, as `on_menu` hears them.
pub const OPEN: u32 = 1;
pub const LOCK: u32 = 2;
pub const QUIT: u32 = 3;

/// The tray's menu: bit *n* of the enabled mask is entry *n*.
pub const MENU: &[MenuEntry] = &[MenuEntry::new("Open", OPEN), MenuEntry::new("Lock", LOCK), MenuEntry::new("Quit", QUIT)];

/// The window's index in the window table.
pub const WINDOW: u16 = 1;

/// The icon: 16 × 16 straight RGBA, a filled circle on transparency.
pub const ICON_SIZE: u32 = 16;
pub static ICON: [u8; (ICON_SIZE * ICON_SIZE * 4) as usize] = icon();

const fn icon() -> [u8; (ICON_SIZE * ICON_SIZE * 4) as usize] {
    let mut px = [0u8; (ICON_SIZE * ICON_SIZE * 4) as usize];
    let mut i = 0;
    while i < (ICON_SIZE * ICON_SIZE) as usize {
        let (x, y) = ((i % 16) as i32 * 2 - 15, (i / 16) as i32 * 2 - 15);
        if x * x + y * y <= 14 * 14 {
            px[i * 4] = 0x2f;
            px[i * 4 + 1] = 0x6f;
            px[i * 4 + 2] = 0xeb;
            px[i * 4 + 3] = 0xff;
        }
        i += 1;
    }
    px
}

// ---- the daemon: program 0, windowless ---------------------------------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
pub struct TrayUi {
    /// "closed", "window N" or "locked".
    #[widget_id("status")]
    pub status: Label<String>,
}

/// The daemon's state. Unsplit (`State = Self`), so its hook writes the
/// label itself. `Reloadable` because a window program is in a `reload`
/// build (as examples/daemon's are): nothing is kept across a swap.
/// Expands to nothing without the core's `reload`.
#[derive(Default, fulgurite::Reloadable)]
pub struct TrayApp {
    #[reload(fresh)]
    pub ui: TrayUi,
    #[reload(fresh)]
    requests: Requests,
    /// The window's key while it is open.
    #[reload(fresh)]
    pub open: Option<u32>,
    #[reload(fresh)]
    pub locked: bool,
}

/// What the window's answers become for the daemon.
pub enum TrayMsg {
    Opened(u32),
    Closed(u32),
}

impl TrayApp {
    fn show(&mut self) {
        let s = match (self.locked, self.open) {
            (true, _) => "locked".to_string(),
            (false, Some(w)) => format!("window {w}"),
            (false, None) => "closed".to_string(),
        };
        self.ui.status.set(s);
    }
}

impl App for TrayApp {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self {
        self
    }
    type Tree = TrayUi;
    type Msg = TrayMsg;
    fn tree(&self) -> &TrayUi {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut TrayUi {
        &mut self.ui
    }
    fn requests(&mut self) -> Option<&mut Requests> {
        Some(&mut self.requests)
    }
    fn answered(_tag: u64, a: Answer<'_>) -> Option<TrayMsg> {
        match a {
            Answer::WindowOpened { window } => Some(TrayMsg::Opened(window)),
            Answer::WindowClosed { window } => Some(TrayMsg::Closed(window)),
            _ => None,
        }
    }
    fn apply(this: &mut Self, m: TrayMsg) -> bool {
        match m {
            TrayMsg::Opened(window) => this.open = Some(window),
            TrayMsg::Closed(window) => {
                if this.open == Some(window) {
                    this.open = None;
                }
            }
        }
        this.show();
        true
    }
}

// ANCHOR: on_menu
/// **A chosen tray entry** — posted as `Event::Menu` to the windowless
/// session, which runs this. Every exit is a request: `KeyOutcome::Quit`
/// from key 0 would close nothing.
pub fn on_menu(app: &mut TrayApp, id: u32) -> KeyOutcome {
    match id {
        OPEN if !app.locked && app.open.is_none() => {
            app.requests.push(Request::OpenWindow { program: WINDOW, tag: OPEN as u64 });
        }
        LOCK => {
            app.locked = !app.locked;
            if let (true, Some(window)) = (app.locked, app.open) {
                app.requests.push(Request::CloseWindow { window });
            }
            app.show();
        }
        QUIT => {
            app.requests.push(Request::Quit);
        }
        _ => return KeyOutcome::Ignored,
    }
    KeyOutcome::Handled
}

/// Which entries are enabled, from the state: Open only while no window is
/// open and nothing is locked. Called in the host, never in the window.
pub fn enabled(app: &TrayApp) -> MenuMask {
    MenuMask::ALL.with(0, !app.locked && app.open.is_none())
}
// ANCHOR_END: on_menu

fn boot() -> TrayApp {
    let mut t = TrayApp::default();
    t.show();
    t
}

// ---- the window: program 1 ---------------------------------------------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
pub struct WindowUi {
    #[widget_id("window-title")]
    pub title: Label,
    /// Pushes `Request::Close`: the window's session is never told its key.
    #[widget_id("close")]
    pub close: Button<TrayWindow>,
}

/// `Reloadable`, as `TrayApp` is.
#[derive(Default, fulgurite::Reloadable)]
pub struct TrayWindow {
    #[reload(fresh)]
    pub ui: WindowUi,
    #[reload(fresh)]
    requests: Requests,
}

impl App for TrayWindow {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self {
        self
    }
    type Tree = WindowUi;
    type Msg = ();
    fn tree(&self) -> &WindowUi {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut WindowUi {
        &mut self.ui
    }
    fn requests(&mut self) -> Option<&mut Requests> {
        Some(&mut self.requests)
    }
    fn apply(_: &mut Self, _: ()) -> bool {
        false
    }
}

fn window_state() -> TrayWindow {
    let mut w = TrayWindow::default();
    w.ui.title.set("opened from the tray");
    w.ui.close.label = "Close";
    w.ui.close.on_click = |w, _| {
        w.requests.push(Request::Close);
    };
    w
}

/// The window, described: opened by the daemon's window table.
pub fn window() -> Program<TrayWindow> {
    application(window_state).title("tray — window").size(240, 100)
}

// ANCHOR: program
/// **The window table**: entry 1 is the window.
pub fn windows(index: u16) -> Option<Box<dyn OpenProgram>> {
    match index {
        WINDOW => Some(boxed(window())),
        _ => None,
    }
}

/// **The tray application, described**: a daemon whose tray is data.
pub fn program() -> Program<TrayApp> {
    daemon(boot)
        .title("tray")
        .windows(windows)
        .on_menu(on_menu)
        .tray(TraySpec { icon: (ICON_SIZE, ICON_SIZE, &ICON), tooltip: "fulgurite tray", menu: MENU, enabled })
}
// ANCHOR_END: program

/// The window under the build's strategy: `Everything` with the
/// `everything` feature, so the drive runs both.
fn boxed(p: Program<TrayWindow>) -> Box<dyn OpenProgram> {
    #[cfg(feature = "everything")]
    return Box::new(p.invalidation::<fulgurite::Everything>());
    #[cfg(not(feature = "everything"))]
    Box::new(p)
}

/// The hooks without a window or a bus (M90/T90.7): what each entry asks
/// for, and which entries the state enables. `just tray-drive` drives the
/// same hooks through the OS path; this holds them where no tray can run.
#[cfg(test)]
mod tests {
    use super::*;

    fn drained(app: &mut TrayApp) -> Vec<Request> {
        let mut out = [Request::Quit; Requests::CAPACITY];
        let n = app.requests.drain_into(&mut out);
        out[..n].to_vec()
    }

    #[test]
    fn open_asks_for_the_window_once_and_not_while_open_or_locked() {
        let mut app = boot();
        assert_eq!(on_menu(&mut app, OPEN), KeyOutcome::Handled);
        let got = drained(&mut app);
        println!("Open: {got:?} (want one OpenWindow {{ program: {WINDOW} }})");
        assert_eq!(got, vec![Request::OpenWindow { program: WINDOW, tag: OPEN as u64 }]);
        assert!(enabled(&app).enabled(0), "Open is disabled before any window opened");
        TrayApp::apply(&mut app, TrayMsg::Opened(7));
        assert!(!enabled(&app).enabled(0), "Open stays enabled while its window is open");
        on_menu(&mut app, OPEN);
        assert!(drained(&mut app).is_empty(), "a second Open asked for a second window");
    }

    #[test]
    fn lock_closes_the_open_window_by_its_key_and_disables_open() {
        let mut app = boot();
        TrayApp::apply(&mut app, TrayMsg::Opened(7));
        on_menu(&mut app, LOCK);
        let got = drained(&mut app);
        println!("Lock with window 7 open: {got:?}, status {:?}", app.ui.status.text.as_str());
        assert_eq!(got, vec![Request::CloseWindow { window: 7 }]);
        assert_eq!(app.ui.status.text.as_str(), "locked");
        TrayApp::apply(&mut app, TrayMsg::Closed(7));
        assert!(!enabled(&app).enabled(0), "Open is enabled while locked");
        on_menu(&mut app, LOCK);
        assert!(enabled(&app).enabled(0), "unlocking left Open disabled");
    }

    /// `KeyOutcome::Quit` from the windowless session closes nothing
    /// (M89, second review A21): the tray's Quit must be a request.
    #[test]
    fn quit_is_a_request_not_a_key_outcome() {
        let mut app = boot();
        let outcome = on_menu(&mut app, QUIT);
        let got = drained(&mut app);
        println!("Quit: outcome {outcome:?}, requests {got:?} (want Handled, [Quit])");
        assert_eq!(outcome, KeyOutcome::Handled, "Quit answered a KeyOutcome a daemon ignores");
        assert_eq!(got, vec![Request::Quit]);
        assert_eq!(on_menu(&mut app, 99), KeyOutcome::Ignored, "an unknown id was handled");
    }
}
