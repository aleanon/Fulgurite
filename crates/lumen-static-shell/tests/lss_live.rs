//! **A live `.lss` is re-read when the file changes** — M61/T61.5
//! (`docs/plan-lss.md` §2.4).
//!
//! `Program::live_lss` is what the window calls in debug and `reload`
//! builds. These go through `Program::host` and `Host::pump`, the path
//! every window and headless frame takes, and read the resolved colour of
//! one node — not whether a function was called.

use lumen_static::display::Color;
use lumen_static::event::App;
use lumen_static::prelude::*;
use lumen_static::style::{Patch, Rule, Selector};
use lumen_static_shell::program::application;
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("title")]
    title: Label,
}

#[derive(Default)]
struct Page {
    ui: Ui,
}

impl App for Page {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self::State {
        self
    }
    type Tree = Ui;
    type Msg = ();
    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
    fn apply(_: &mut Self, _: ()) -> bool {
        false
    }
}

/// What this "build compiled in": the release answer, and the fallback.
const COMPILED_INK: Color = Color(1, 1, 1, 255);
const COMPILED: &[Rule] = &[Rule::new(Selector::Id("title"), Patch::new().color(COMPILED_INK))];

fn title(h: &Host<Page>) -> Color {
    let i = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "title").expect("#title");
    h.styles[i].color
}

/// Write the sheet with an mtime `ahead` seconds from now, so two writes in
/// one second are still two modification times.
fn write(path: &str, css: &str, ahead: i64) {
    std::fs::write(path, css).expect("write the sheet");
    let t = std::time::SystemTime::now() + std::time::Duration::from_secs(ahead as u64);
    std::fs::File::options().write(true).open(path).and_then(|f| f.set_modified(t)).expect("set mtime");
}

fn scratch(name: &str) -> &'static str {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("lss_live");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    Box::leak(dir.join(name).to_string_lossy().into_owned().into_boxed_str())
}

#[test]
fn a_live_sheet_follows_its_file_and_falls_back_to_the_compiled_one() {
    let path = scratch("follows.lss");
    write(path, "#title { color: #102030ff; }", 0);
    let mut h: Host<Page> = application(Page::default).style_lss(COMPILED, path).live_lss().host();
    h.pump();
    let first = title(&h);
    println!("read at build: {first:?} (want #102030)");
    assert_eq!(first, Color(0x10, 0x20, 0x30, 0xff), "the live program drew the compiled sheet, not the file");

    // Saved: the next pump draws it. Nothing else changed.
    write(path, "#title { color: #405060ff; }", 2);
    h.pump();
    let saved = title(&h);
    println!("after a save: {saved:?} (want #405060)");
    assert_eq!(saved, Color(0x40, 0x50, 0x60, 0xff), "the file changed and the frame kept the old sheet");

    // Broken: the compiled sheet, not the last good one and not nothing.
    write(path, "#title { colour: #708090ff; }", 4);
    h.pump();
    let broken = title(&h);
    println!("after a bad save: {broken:?} (want the compiled {COMPILED_INK:?})");
    assert_eq!(broken, COMPILED_INK, "a sheet that does not lower must fall back to the compiled one");
}

/// The other direction: without `live_lss` — what a release window
/// without `reload` runs — the file is never read, however it changes.
#[test]
fn without_live_lss_the_file_is_never_read() {
    let path = scratch("never.lss");
    write(path, "#title { color: #102030ff; }", 0);
    let mut h: Host<Page> = application(Page::default).style_lss(COMPILED, path).host();
    h.pump();
    write(path, "#title { color: #405060ff; }", 2);
    h.pump();
    println!("not live: {:?} (want the compiled {COMPILED_INK:?})", title(&h));
    assert_eq!(title(&h), COMPILED_INK);
}
