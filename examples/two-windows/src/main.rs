//! **Two windows, one process, one agent port** — M89/T89.3
//! (`docs/plan-windows.md`).
//!
//! The list (program 0) has three rows; a click on one asks the window
//! for program 1, the detail, with `Request::OpenWindow`. The window
//! creates it and answers `WindowOpened` with its key inside the same
//! drain, and the list writes that key to `#opened`. The detail has its
//! own counter, `#detail-bump`, which no other tree has; it closes on
//! Escape (its key hook's `KeyOutcome::Quit` closes *its* window) or on
//! `#detail-close` (`Request::Close`), and the list hears
//! `WindowClosed`. Closing the list ends the process.
//!
//! The detail also lays its right-to-left label out `Shaping::Complex`
//! while the list stays `Basic`, so each window must render with its own
//! program's stack; and it sets an agent address that the window ignores,
//! because the primary program's port addresses every window.

use lumen_static::display::Color;
use lumen_static::prelude::*;
use lumen_static::request::{Answer, Request, Requests};
use lumen_static_shell::program::{application, KeyOutcome, OpenProgram, Program};
use lumen_static_widgets::prelude::*;
use lumen_static_winit::Run;

const PAGE: Color = Color(0xf4, 0xf5, 0xf7, 0xff);

// ---- the list: program 0 ----------------------------------------------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
pub struct ListUi {
    #[widget_id("list-title")]
    pub title: Label,
    #[widget_id("row1")]
    pub row1: Button<List>,
    #[widget_id("row2")]
    pub row2: Button<List>,
    #[widget_id("row3")]
    pub row3: Button<List>,
    #[widget_id("missing")]
    pub missing: Button<List>,
    #[widget_id("opened")]
    pub opened: Label<String>,
    #[widget_id("closed")]
    pub closed: Label<String>,
    #[widget_id("count")]
    pub count: Label<String>,
    #[widget_id("bump")]
    pub bump: Button<List>,
}

/// The list's state. Unsplit (`State = Self`), so an answer's `apply`
/// writes the labels itself and `ui.resolve` reads them with no frame.
#[derive(Default)]
pub struct List {
    pub ui: ListUi,
    requests: Requests,
    count: u32,
}

/// What the window's answers become for the list.
pub enum ListMsg {
    Opened(u32),
    Closed(u32),
    NoSuch(u16),
}

/// The detail's index in the window table.
pub const DETAIL: u16 = 1;

fn open(l: &mut List, row: u64) {
    l.requests.push(Request::OpenWindow { program: DETAIL, tag: row });
}

impl App for List {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self {
        self
    }
    type Tree = ListUi;
    type Msg = ListMsg;
    fn tree(&self) -> &ListUi {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut ListUi {
        &mut self.ui
    }
    fn requests(&mut self) -> Option<&mut Requests> {
        Some(&mut self.requests)
    }
    fn answered(_tag: u64, a: Answer<'_>) -> Option<ListMsg> {
        match a {
            Answer::WindowOpened { window } => Some(ListMsg::Opened(window)),
            Answer::WindowClosed { window } => Some(ListMsg::Closed(window)),
            Answer::NoSuchProgram { program } => Some(ListMsg::NoSuch(program)),
            _ => None,
        }
    }
    fn apply(this: &mut Self, m: ListMsg) -> bool {
        match m {
            ListMsg::Opened(window) => this.ui.opened.set(format!("{window}")),
            ListMsg::Closed(window) => this.ui.closed.set(format!("closed {window}")),
            ListMsg::NoSuch(program) => this.ui.opened.set(format!("no program {program}")),
        }
        true
    }
}

fn list() -> List {
    let mut l = List::default();
    l.ui.title.set("Items");
    l.ui.row1.label = "item 1";
    l.ui.row1.on_click = |l, _| open(l, 1);
    l.ui.row2.label = "item 2";
    l.ui.row2.on_click = |l, _| open(l, 2);
    l.ui.row3.label = "item 3";
    l.ui.row3.on_click = |l, _| open(l, 3);
    l.ui.missing.label = "program 9";
    l.ui.missing.on_click = |l, _| {
        l.requests.push(Request::OpenWindow { program: 9, tag: 9 });
    };
    l.ui.opened.set("none opened".into());
    l.ui.closed.set("none closed".into());
    l.ui.count.set("0".into());
    l.ui.bump.label = "+1";
    l.ui.bump.on_click = |l, _| {
        l.count += 1;
        l.ui.count.set(format!("{}", l.count));
    };
    l
}

// ---- the detail: program 1 --------------------------------------------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
pub struct DetailUi {
    #[widget_id("detail-title")]
    pub title: Label,
    #[widget_id("detail-count")]
    pub count: Label<String>,
    #[widget_id("detail-bump")]
    pub bump: Button<Detail>,
    #[widget_id("detail-close")]
    pub close: Button<Detail>,
    #[widget_id("detail-panic")]
    pub panic: Button<Detail>,
    /// Arabic, which joins: shaped and unshaped runs differ in width.
    #[widget_id("rtl")]
    pub rtl: Label,
}

#[derive(Default)]
pub struct Detail {
    pub ui: DetailUi,
    requests: Requests,
    count: u32,
}

impl App for Detail {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self {
        self
    }
    type Tree = DetailUi;
    type Msg = ();
    fn tree(&self) -> &DetailUi {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut DetailUi {
        &mut self.ui
    }
    fn requests(&mut self) -> Option<&mut Requests> {
        Some(&mut self.requests)
    }
    fn apply(_: &mut Self, _: ()) -> bool {
        false
    }
}

fn detail_state() -> Detail {
    let mut d = Detail::default();
    d.ui.title.set("Detail");
    d.ui.count.set("0".into());
    d.ui.bump.label = "bump";
    d.ui.bump.on_click = |d, _| {
        d.count += 1;
        d.ui.count.set(format!("{}", d.count));
    };
    d.ui.close.label = "close";
    d.ui.close.on_click = |d, _| {
        d.requests.push(Request::Close);
    };
    d.ui.panic.label = "panic";
    d.ui.panic.on_click = |_, _| panic!("the detail window panicked on purpose");
    d.ui.rtl.set("مرحبا بالعالم");
    d
}

/// Escape closes the detail: `KeyOutcome::Quit` closes the window it came
/// from, and only the primary's ends the process.
fn detail_keys(_: &mut Detail, key: lumen_static::Key, _: lumen_static::Mods) -> KeyOutcome {
    match key {
        lumen_static::Key::Escape => KeyOutcome::Quit,
        _ => KeyOutcome::Ignored,
    }
}

// ANCHOR: windows
/// **The detail, described** — a `Program` like any other, opened by the
/// list's window table. Its agent address is ignored on purpose: one port
/// addresses every window.
pub fn detail() -> Program<Detail> {
    application(detail_state)
        .title("two-windows — detail")
        .size(300, 260)
        .page(PAGE)
        .on_key(detail_keys)
        .shaping(lumen_static_shell::text::Shaping::Complex)
        .agent("127.0.0.1:7999")
}

/// **The window table**: entry `n` is the program `Request::OpenWindow {
/// program: n, .. }` opens. An ordinary `fn` the list's program holds;
/// requests carry only the index.
pub fn windows(index: u16) -> Option<Box<dyn OpenProgram>> {
    match index {
        DETAIL => Some(boxed(detail())),
        _ => None,
    }
}

/// **The list, described**, with its window table.
pub fn program() -> Program<List> {
    application(list)
        .title("two-windows — list")
        .size(340, 380)
        .page(PAGE)
        .accessible(cfg!(feature = "a11y"))
        .windows(windows)
}
// ANCHOR_END: windows

/// Each program under the build's strategy: `Everything` with the
/// `everything` feature, so the drive runs both.
fn boxed(p: Program<Detail>) -> Box<dyn OpenProgram> {
    #[cfg(feature = "everything")]
    return Box::new(p.invalidation::<lumen_static::Everything>());
    #[cfg(not(feature = "everything"))]
    Box::new(p)
}

fn main() {
    #[cfg(feature = "everything")]
    program().invalidation::<lumen_static::Everything>().run();
    #[cfg(not(feature = "everything"))]
    program().run();
}
