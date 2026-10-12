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
//! **Data between the windows is a post** (M89/T89.4): when the list hears
//! `WindowOpened` for a row's click, it posts the row to the new window
//! (`Request::Post`), and the detail's `#item` shows it — "item 2" for row
//! 2. `#send` posts again to the last window opened, so a post can reach a
//! detail that is open and idle.
//!
//! **A request is answered by the window it came from** (M89/T89.5):
//! `#detail-shot` asks for a screenshot, which the window answers from
//! *this* window's renderer, and the detail puts the pixels' hash in its
//! **title** — the one place a drive can read that is not in the pixels
//! being hashed. `#field` is a text field, whose caret is where *this*
//! window's input method opens.
//!
//! **Each program has its own images** (M89b/T89.10): both trees show
//! `#swatch`, `ImageId(0)`, and each program's store answers id 0 with a
//! different solid colour — red for the list, blue for the detail. On the
//! GPU the two windows share one device and must not share an image atlas.
//!
//! The detail also lays its right-to-left label out `Shaping::Complex`
//! while the list stays `Basic`, so each window must render with its own
//! program's stack; and it sets an agent address that the window ignores,
//! because the primary program's port addresses every window.

use fulgurite::display::Color;
use fulgurite::prelude::*;
use fulgurite::request::{Answer, Request, Requests};
use fulgurite::spawn::Descriptor;
use fulgurite_shell::program::{application, KeyOutcome, OpenProgram, Program};
use fulgurite_widgets::prelude::*;
use fulgurite_winit::Run;

const PAGE: Color = Color(0xf4, 0xf5, 0xf7, 0xff);

// ---- the images: one store per program (M89b/T89.10) ------------------

/// **A program's images: `ImageId(0)` is one solid colour.** Each program
/// hands the window its own store (`Program::images`), and both use id 0:
/// the GPU windows share one device but not an image atlas, because two
/// programs' `ImageId(0)` are different images (P25). The drive reads the
/// pixel at each window's `#swatch` centre and wants its own colour.
pub struct Swatch {
    px: Vec<u8>,
}

/// The swatch's edge, in image pixels.
pub const SWATCH: u32 = 16;
/// The list's swatch colour and the detail's.
pub const LIST_SWATCH: [u8; 4] = [0xd0, 0x30, 0x30, 0xff];
pub const DETAIL_SWATCH: [u8; 4] = [0x20, 0x70, 0xd0, 0xff];

impl Swatch {
    pub fn new(rgba: [u8; 4]) -> Self {
        Swatch { px: rgba.repeat((SWATCH * SWATCH) as usize) }
    }
}

impl fulgurite::ImageSource for Swatch {
    fn image(&self, id: fulgurite::display::ImageId, _frame: u32) -> Option<(u32, u32, &[u8])> {
        (id == fulgurite::display::ImageId(0)).then_some((SWATCH, SWATCH, &self.px[..]))
    }
}

/// The `#swatch` widget both trees carry: id 0, drawn 48 × 48.
fn swatch() -> fulgurite_widgets::Image {
    fulgurite_widgets::Image::new(fulgurite::display::ImageId(0), fulgurite::Size { w: 48.0, h: 48.0 })
}

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
    #[widget_id("send")]
    pub send: Button<List>,
    /// `ImageId(0)` from the list's own `ImageSource` (M89b/T89.10).
    #[widget_id("swatch")]
    pub swatch: fulgurite_widgets::Image,
}

/// The list's state. Unsplit (`State = Self`), so an answer's `apply`
/// writes the labels itself and `ui.resolve` reads them with no frame.
#[derive(Default)]
pub struct List {
    pub ui: ListUi,
    requests: Requests,
    count: u32,
    /// The last window a row opened: where `#send` posts.
    last: u32,
    /// Posts `#send` has made, so each carries a new item.
    sent: u64,
}

/// What the window's answers become for the list.
pub enum ListMsg {
    /// A window opened, and the tag of the request that opened it — the
    /// row clicked.
    Opened(u32, u64),
    Closed(u32),
    NoSuch(u16),
}

/// The detail's index in the window table.
pub const DETAIL: u16 = 1;

/// A post's `kind`: "show this item"; its `payload` is the item's number.
pub const ITEM: u64 = 1;

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
            Answer::WindowOpened { window } => Some(ListMsg::Opened(window, _tag)),
            Answer::WindowClosed { window } => Some(ListMsg::Closed(window)),
            Answer::NoSuchProgram { program } => Some(ListMsg::NoSuch(program)),
            _ => None,
        }
    }
    fn apply(this: &mut Self, m: ListMsg) -> bool {
        match m {
            ListMsg::Opened(window, row) => {
                this.ui.opened.set(format!("{window}"));
                this.last = window;
                post_item(this, window, row);
            }
            ListMsg::Closed(window) => this.ui.closed.set(format!("closed {window}")),
            ListMsg::NoSuch(program) => this.ui.opened.set(format!("no program {program}")),
        }
        true
    }
}

// ANCHOR: post
/// **Tell window `window` which item to show** — a post: data, addressed
/// by the window's key, delivered to the detail as `Answer::Posted` and
/// turned into the detail's own message by its `App::answered`, in the
/// build loaded when it arrives. No state is shared between the windows.
fn post_item(l: &mut List, window: u32, item: u64) {
    l.requests.push(Request::Post { window, desc: Descriptor::new(ITEM, item) });
}
// ANCHOR_END: post

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
    l.ui.swatch = swatch();
    l.ui.send.label = "send";
    l.ui.send.on_click = |l, _| {
        l.sent += 1;
        let (window, item) = (l.last, 100 + l.sent);
        post_item(l, window, item);
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
    /// The item the list posted here, or "no item".
    #[widget_id("item")]
    pub item: Label<String>,
    /// Which window posted it.
    #[widget_id("from")]
    pub from: Label<String>,
    /// A text field: its caret is this window's IME area (T89.5).
    #[widget_id("field")]
    pub field: fulgurite_widgets::input::TextInput,
    /// Asks the window for a screenshot of this window (T89.5).
    #[widget_id("detail-shot")]
    pub shot: Button<Detail>,
    #[widget_id("detail-bump")]
    pub bump: Button<Detail>,
    #[widget_id("detail-close")]
    pub close: Button<Detail>,
    #[widget_id("detail-panic")]
    pub panic: Button<Detail>,
    /// Arabic, which joins: shaped and unshaped runs differ in width.
    #[widget_id("rtl")]
    pub rtl: Label,
    /// `ImageId(0)` from the detail's own `ImageSource` — the same id as
    /// the list's, a different image (M89b/T89.10).
    #[widget_id("swatch")]
    pub swatch: fulgurite_widgets::Image,
}

/// `Reloadable` because a window program is one in a `reload` build
/// (`WindowApp`, M89/T89.7): nothing is kept, everything is the new
/// build's. Expands to nothing without the core's `reload`.
#[derive(Default, fulgurite::Reloadable)]
pub struct Detail {
    #[reload(fresh)]
    pub ui: DetailUi,
    #[reload(fresh)]
    requests: Requests,
    #[reload(fresh)]
    count: u32,
    /// The last screenshot's answer, as the title shows it: its width by height and the
    /// pixels' FNV-1a hash, or "refused".
    #[reload(fresh)]
    shot: Option<String>,
}

/// The screenshot request's tag.
pub const SHOT: u64 = 1;

/// FNV-1a over a screenshot's RGBA bytes — what the title shows, so the
/// drive compares it with `ui.screenshot`'s pixels hashed the same way.
fn pixel_hash(rgba: &[u8]) -> u64 {
    rgba.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3))
}

impl App for Detail {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self {
        self
    }
    type Tree = DetailUi;
    type Msg = DetailMsg;
    fn tree(&self) -> &DetailUi {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut DetailUi {
        &mut self.ui
    }
    fn requests(&mut self) -> Option<&mut Requests> {
        Some(&mut self.requests)
    }
    // ANCHOR: posted
    /// A post arrives under the unsolicited tag; the variant and the
    /// descriptor's `kind` say what it is.
    fn answered(_tag: u64, a: Answer<'_>) -> Option<DetailMsg> {
        match a {
            Answer::Posted { from, desc } if desc.kind == ITEM => Some(DetailMsg::Item { from, item: desc.payload }),
            Answer::Image { width, height, rgba, .. } if _tag == SHOT => {
                Some(DetailMsg::Shot(format!("{width}x{height} {:016x}", pixel_hash(rgba))))
            }
            Answer::Refused(_) if _tag == SHOT => Some(DetailMsg::Shot("refused".into())),
            _ => None,
        }
    }
    // ANCHOR_END: posted
    fn apply(this: &mut Self, m: DetailMsg) -> bool {
        match m {
            DetailMsg::Item { from, item } => {
                this.ui.item.set(format!("item {item}"));
                this.ui.from.set(format!("from {from}"));
            }
            // The title only: a label would change the pixels the next
            // screenshot hashes.
            DetailMsg::Shot(shot) => this.shot = Some(shot),
        }
        true
    }
}

/// What the window's answers become for the detail.
pub enum DetailMsg {
    /// Window `from` posted item `item` here.
    Item { from: u32, item: u64 },
    /// A screenshot answered: `WxH hash`, or "refused".
    Shot(String),
}

fn detail_state() -> Detail {
    let mut d = Detail::default();
    d.ui.title.set("Detail");
    d.ui.count.set("0".into());
    d.ui.item.set("no item".into());
    d.ui.from.set("from none".into());
    d.ui.bump.label = "bump";
    d.ui.bump.on_click = |d, _| {
        d.count += 1;
        d.ui.count.set(format!("{}", d.count));
    };
    d.ui.shot.label = "screenshot";
    d.ui.shot.on_click = |d, _| {
        d.requests.push(Request::Screenshot { tag: SHOT });
    };
    d.ui.close.label = "close";
    d.ui.close.on_click = |d, _| {
        d.requests.push(Request::Close);
    };
    d.ui.panic.label = "panic";
    d.ui.panic.on_click = |_, _| panic!("the detail window panicked on purpose");
    d.ui.rtl.set("مرحبا بالعالم");
    d.ui.swatch = swatch();
    d
}

/// The detail's title: its name, and the last screenshot's answer once
/// there is one (T89.5).
fn detail_title(d: &Detail) -> String {
    match &d.shot {
        None => "two-windows — detail".into(),
        Some(shot) => format!("two-windows — detail · shot {shot}"),
    }
}

/// Escape closes the detail: `KeyOutcome::Quit` closes the window it came
/// from, and only the primary's ends the process.
fn detail_keys(_: &mut Detail, key: fulgurite::Key, _: fulgurite::Mods) -> KeyOutcome {
    match key {
        fulgurite::Key::Escape => KeyOutcome::Quit,
        _ => KeyOutcome::Ignored,
    }
}

// ANCHOR: windows
/// **The detail, described** — a `Program` like any other, opened by the
/// list's window table. Its agent address is ignored on purpose: one port
/// addresses every window.
pub fn detail() -> Program<Detail> {
    application(detail_state)
        .title(detail_title)
        .size(300, 400)
        .page(PAGE)
        .on_key(detail_keys)
        .shaping(fulgurite_shell::text::Shaping::Complex)
        .images(Swatch::new(DETAIL_SWATCH))
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
        .size(340, 440)
        .page(PAGE)
        .accessible(cfg!(feature = "a11y"))
        .images(Swatch::new(LIST_SWATCH))
        .windows(windows)
}
// ANCHOR_END: windows

/// Each program under the build's strategy: `Everything` with the
/// `everything` feature, so the drive runs both.
fn boxed(p: Program<Detail>) -> Box<dyn OpenProgram> {
    #[cfg(feature = "everything")]
    return Box::new(p.invalidation::<fulgurite::Everything>());
    #[cfg(not(feature = "everything"))]
    Box::new(p)
}

fn main() {
    #[cfg(feature = "everything")]
    program().invalidation::<fulgurite::Everything>().run();
    #[cfg(not(feature = "everything"))]
    program().run();
}
