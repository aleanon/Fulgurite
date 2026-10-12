//! **A still frame reshapes nothing** — M80/T80.4 (`docs/plan-text-fit.md`).
//!
//! A `Selectable` shapes its text into a cluster list so a press can find
//! a byte and the tint a row. That shape is keyed exactly as the label's
//! memo is (the room, the style, the scale, the measurer's generation) and
//! on the shown text's length, so a frame that changes none of them shapes
//! nothing — counted by `text_counters().measured`, which is process-wide,
//! so this is alone in its binary with one `#[test]`.
//!
//! The idle frames are **forced to lay out** (`Host::invalidate_all`): a
//! pump with nothing marked skips layout altogether, and would read 0
//! whatever the widget did on a layout. That they did is asserted by
//! zeroing the node's box before each and seeing layout write it back.

use fulgurite::display::Color;
use fulgurite::event::{Event, PointerButton};
use fulgurite::style::{Patch, Rule, Selector};
use fulgurite::{App as DeriveApp, StaticTree};
use fulgurite_shell::text::{text_counters, FontMetrics, FontStack};
use fulgurite_shell::Host;
use fulgurite_widgets::Selectable;

#[path = "../../fulgurite-shell/tests/fonts/pinned.rs"]
mod pinned;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("sel")]
    sel: Selectable<String>,
}

#[derive(DeriveApp, Default)]
struct Page {
    #[tree]
    ui: Ui,
}

#[test]
fn an_idle_frame_shapes_nothing() {
    let font = FontStack::from_bytes(pinned::bytes(), None).expect("the pinned face parses");
    let mut a = Page::default();
    a.ui.sel.label.set("Hold the quick brown fox\nLid jumps over the lazy dog".to_owned());
    let mut h: Host<Page> = Host::new(a, 480, 120);
    h.set_stylesheet(&[Rule::new(Selector::Id("sel"), Patch::new().color(Color(0, 0, 0, 255)))]);
    let mut m = FontMetrics::new(font);
    let i = fulgurite::style::index_of_wid(<Ui as fulgurite::Widget>::SUB_TABLE, "sel").expect("#sel");

    let c0 = text_counters().measured;
    h.pump_with(&mut m);
    // A press and a drag, so there is a selection to keep and paint.
    let st = h.states[i];
    let (x, y) = (st.pos.x + 2.0, st.pos.y + 8.0);
    h.post(Event::PointerDown { x, y, button: PointerButton::Left, clicks: 1 });
    h.pump_with(&mut m);
    h.post(Event::PointerMove { x: x + 120.0, y });
    h.pump_with(&mut m);
    let first = text_counters().measured - c0;
    let copied = h.copy().map(str::to_owned);
    println!("setup: the first frames measured or shaped {first} run(s) (> 0); selection {copied:?}");
    assert!(first > 0, "setup: nothing was shaped -- the measurer is not the one counted");
    assert!(copied.is_some(), "setup: the drag selected nothing");

    let before = text_counters().measured;
    let mut laid = 0;
    let w = h.states[i].size.w;
    for _ in 0..2 {
        // The box is zeroed and must come back: only a layout writes it.
        h.states[i].size.w = 0.0;
        h.invalidate_all();
        h.pump_with(&mut m);
        laid += (h.states[i].size.w == w) as u32;
    }
    let idle = text_counters().measured - before;
    println!("two idle frames, both laid out ({laid} of 2 rewrote the Selectable's zeroed box): {idle} run(s) measured or shaped (bound 0)");
    assert_eq!(laid, 2, "setup: the idle frames did not lay the Selectable out");
    assert_eq!(idle, 0, "an idle frame reshaped the text");
    assert_eq!(h.copy().map(str::to_owned), copied, "the idle frames changed the selection");
}
