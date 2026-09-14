//! `--form`: the M8/M9 widget set through the real rasteriser.
//!
//! Moved out of `main.rs` by M14/T14.0.

use lumen_static::prelude::*;
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::{FontMetrics, FontStack};
use lumen_static_shell::Host;

use crate::app::{ACCENT, INK, PAGE, SURFACE};


/// One row of the settings panel: a control and its label.
///
/// A checkbox is a box, not a box-with-text — composing it with a
/// `Label` in a row is the tree doing the work a `label:` field on every
/// control would otherwise duplicate.
#[derive(lumen_static::StaticTree, Default)]
#[widget(row, spacing = 8.0)]
struct CheckRow {
    #[widget_id("check")]
    check: lumen_static::toggle::CheckBox<Form>,
    #[widget_id("check_label")]
    label: Label,
}

#[derive(lumen_static::StaticTree, Default)]
#[widget(row, spacing = 8.0)]
struct SwitchRow {
    #[widget_id("switch")]
    switch: lumen_static::toggle::Switch<Form>,
    #[widget_id("switch_label")]
    label: Label,
}

/// One row: a slider and its label (T11.2).
#[derive(lumen_static::StaticTree, Default)]
#[widget(row, spacing = 8.0)]
struct SliderRow {
    #[widget_id("vol_label")]
    label: Label,
    #[widget_id("vol")]
    vol: lumen_static::slider::Slider,
}

/// The second tab's content — deliberately different, so switching is
/// visible and `ui.lastChange` has something to report.
#[derive(lumen_static::StaticTree, Default)]
#[widget(column, spacing = 6.0)]
struct AboutTab {
    #[widget_id("about")]
    about: Label,
}

/// A tab strip — T11.4, and it needed **no new widget type**.
///
/// Tabs are a *composition*: two buttons and an `Either`, wired by the
/// application's handlers. That is what "topology in the type" is for,
/// and T8.0 is what made it worth doing — branch nodes are ordinary
/// table nodes now, so a tab's content is addressable by selector,
/// styleable by `#id`, and visible to the audit. None of that was true
/// before 2026-09-04.
#[derive(lumen_static::StaticTree, Default)]
#[widget(row, spacing = 6.0)]
struct TabStrip {
    #[widget_id("tab_settings")]
    settings: lumen_static::ops::Button<Form, String>,
    #[widget_id("tab_about")]
    about: lumen_static::ops::Button<Form, String>,
}

/// The growing settings panel — M11's gate behind every task gate.
///
/// Each widget the milestone adds lands here as well as in its own
/// tests, because every M11 gate is otherwise agent-protocol-shaped:
/// `ui.lint` clean, the role present, `press` passing. A control can
/// satisfy all of that without ever being placed next to another one,
/// which is the fastest route to a vocabulary that is individually
/// correct and collectively unusable.
#[derive(lumen_static::StaticTree, Default)]
#[widget(column, spacing = 10.0)]
struct FormBody {
    tabs: TabStrip,
    #[widget_id("name")]
    name: lumen_static::input::TextInput,
    check_row: CheckRow,
    switch_row: SwitchRow,
    slider_row: SliderRow,
    #[widget_id("r1")]
    r1: Label,
    #[widget_id("r2")]
    r2: Label,
    #[widget_id("r3")]
    r3: Label,
    #[widget_id("r4")]
    r4: Label,
    #[widget_id("r5")]
    r5: Label,
}

#[derive(lumen_static::StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct FormUi {
    #[widget_id("scroll")]
    scroll: lumen_static::scroll::ScrollView<FormBody>,
}

#[derive(lumen_static::App, Default)]
struct Form {
    #[tree]
    ui: FormUi,
    /// The truth for the checkbox. The widget holds a copy, kept in step
    /// by the binding — the same relationship a `Label` has with the
    /// value it shows.
    #[bind(ui.scroll.content.check_row.check, with = set_on)]
    notify: bool,
    #[bind(ui.scroll.content.switch_row.switch, with = set_on)]
    dark: bool,
}

fn set_on<S: 'static, const K: u8>(t: &mut lumen_static::toggle::Toggle<S, K>, v: &bool) {
    t.on = *v;
}

/// Render the form headlessly and assert the properties only a real
/// rasteriser can settle.
///
/// `ScrollView` and `TextInput` are covered by unit tests that count
/// display-list commands. That is not the same as knowing they *paint*:
/// this repository has twice shipped a construct whose commands were
/// right and whose pixels were not — a repeater that drew 44 px outside
/// its viewport for three milestones, and a renderer that ignored clip
/// scopes entirely. So the new widgets go through `Canvas` here, in
/// `just fixtures`, and the frame is asserted.
pub fn form(path: &str) {
    let (w, h) = (300u32, 200u32);
    let mut host: Host<Form> = Host::new(Form::default(), w, h);
    {
        let b = &mut host.app.ui.scroll.content;
        b.name.set_str("typed into a real field");
        b.check_row.label.set("notify me");
        b.check_row.check.on_toggle = |f: &mut Form, _| f.notify = !f.notify;
        b.switch_row.label.set("dark mode");
        b.switch_row.switch.on_toggle = |f: &mut Form, _| f.dark = !f.dark;
        b.slider_row.label.set("volume");
        b.slider_row.vol.set_value(0.65);
        b.tabs.settings.label = "Settings".into();
        b.tabs.about.label = "About".into();
        for (i, l) in [&mut b.r1, &mut b.r2, &mut b.r3, &mut b.r4, &mut b.r5].into_iter().enumerate()
        {
            l.set(match i {
                0 => "row one",
                1 => "row two",
                2 => "row three",
                3 => "row four",
                _ => "row five — below the fold",
            });
        }
    }
    host.set_stylesheet(&[
        Rule::new(Selector::Kind("FormUi"), Patch::new().background(PAGE).padding(Edges::all(12.0))),
        Rule::new(Selector::Id("name"), Patch::new().background(SURFACE).radius(8.0).width(Length::FILL)),
        Rule::new(Selector::Kind("label"), Patch::new().color(INK)),
        // Two colours, four states: the resting body and the on body are
        // each the other's mark (`crates/lumen-static/src/toggle.rs`).
        Rule::new(Selector::Kind("checkbox"), Patch::new().background(SURFACE).color(ACCENT)),
        Rule::new(Selector::Kind("switch"), Patch::new().background(SURFACE).color(ACCENT)),
        Rule::new(Selector::Kind("slider"), Patch::new().background(SURFACE).color(ACCENT)),
        Rule::new(
            Selector::Kind("button"),
            Patch::new().background(SURFACE).color(INK).radius(6.0).padding(Edges::all(6.0)),
        ),
        Rule::new(Selector::Id("name"), Patch::new().color(INK)),
    ]);
    // M25/T25.2: every rule in the form's sheet does something.
    {
        let sheet = host.stylesheet();
        let found: Vec<String> = host
            .sheet_findings()
            .iter()
            .map(|f| format!("rule {} {:?}: {:?}", f.rule, sheet[f.rule].selector, f.issue))
            .collect();
        assert!(found.is_empty(), "form stylesheet rules that change nothing:\n{}", found.join("\n"));
    }
    let mut metrics = FontStack::system().map(FontMetrics::new);
    let mut canvas = match FontStack::system() {
        Some(f) => Canvas::with_font(w, h, f),
        None => Canvas::new(w, h),
    };
    let pump = |host: &mut Host<Form>, m: &mut Option<FontMetrics>| match m.as_mut() {
        Some(m) => {
            host.pump_with(m);
        }
        None => {
            host.pump();
        }
    };
    pump(&mut host, &mut metrics);

    // Tabs are a composition, not a widget: two buttons and an
    // `Either`, wired here (T11.4).
    host.app.ui.scroll.content.tabs.settings.on_click =
        |_f: &mut Form, _: &lumen_static::EventCtx| {};
    host.app.ui.scroll.content.tabs.about.on_click =
        |_f: &mut Form, _: &lumen_static::EventCtx| {};

    // The panel's controls answer to the app's state through bindings.
    host.app.notify = true;
    host.app.dark = true;
    pump(&mut host, &mut metrics);
    assert!(
        host.app.ui.scroll.content.check_row.check.on,
        "the binding wrote the checkbox's copy"
    );
    assert!(host.app.ui.scroll.content.switch_row.switch.on, "and the switch's");

    // Resolve by author id, not by index: the panel grows every task,
    // and the first time it did, three hard-coded indices silently
    // became different nodes.
    let table = <FormUi as Widget>::SUB_TABLE;
    let at = |wid: &str| {
        lumen_static::style::index_of_wid(table, wid).unwrap_or_else(|| panic!("no #{wid}"))
    };
    let (i_scroll, i_body, i_name, i_last) = (at("scroll"), at("scroll") + 1, at("name"), at("r5"));

    let content_h = host.states[i_body].size.h;
    let viewport_h = host.states[i_scroll].size.h;
    assert!(content_h > viewport_h, "the form must overflow: {content_h} vs {viewport_h}");

    // Scroll to the bottom through the real event path, not by writing
    // the field: this asserts C3's `on_event` reaches the scroller.
    for _ in 0..20 {
        host.post(Event::Scroll { x: 150.0, y: 100.0, dy: -20.0 });
        pump(&mut host, &mut metrics);
    }
    let at_end = host.app.ui.scroll.scroll;
    assert!(at_end > 0.0, "the wheel scrolled it");
    host.post(Event::Scroll { x: 150.0, y: 100.0, dy: -20.0 });
    pump(&mut host, &mut metrics);
    assert_eq!(host.app.ui.scroll.scroll, at_end, "and it stops at the end");

    canvas.clear(PAGE);
    host.render(&mut canvas);
    let ink = canvas.ink_fraction(PAGE);
    assert!(ink > 0.01, "the form is blank ({ink:.3} inked)");

    // Scrolled to the bottom, the *first* row is above the viewport and
    // the last is inside it. The scroller's clip is what keeps the one
    // that has left from painting there anyway — the property a
    // repeater got wrong for three milestones, and only geometry plus
    // pixels together can show it.
    let top = host.states[i_scroll].pos.y;
    let field_y = host.states[i_name].pos.y;
    let last = &host.states[i_last];
    assert!(field_y < top, "the field has scrolled above the viewport (y={field_y}, top={top})");
    assert!(
        last.pos.y >= top && last.pos.y + last.size.h <= top + viewport_h + 0.001,
        "the last row is in view at the bottom (y={}, h={})",
        last.pos.y,
        last.size.h
    );

    // Back to the top for the artifact: the controls are what this
    // fixture is for, and they sit above the fold.
    host.app.ui.scroll.scroll = 0.0;
    pump(&mut host, &mut metrics);
    canvas.clear(PAGE);
    host.render(&mut canvas);
    // Back to the top for the artifact: the controls are what this
    // fixture is for, and they sit above the fold.
    host.app.ui.scroll.scroll = 0.0;
    pump(&mut host, &mut metrics);
    canvas.clear(PAGE);
    host.render(&mut canvas);

    std::fs::write(path, canvas.to_ppm()).expect("write frame");
    println!(
        "form: nodes={} cmds={} scroll={:.0}/{:.0} ink={:.1}% -> {path}",
        <FormUi as Widget>::NODE_COUNT,
        host.list.len(),
        host.app.ui.scroll.scroll,
        content_h - viewport_h,
        ink * 100.0,
    );
}
