//! M19/T19.1: `Glass<W>`.
//!
//! The composition the feature exists for is the last test: a stack with
//! content underneath and a glass panel over it, where the panel's
//! command is emitted *after* everything it blurs.

use fulgurite::display::Color;
use fulgurite_widgets::ops::{Fixed, Label};
use fulgurite::{pass, Cmd, Constraints, DisplayList, Length, NodeState, StaticTree, Widget};
use fulgurite_widgets::{Glass, ScrollView};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Bar {
    #[widget_id("title")]
    title: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Rows {
    a: Fixed,
    b: Fixed,
}

/// The motivating scene: content and a glass bar sharing one rect.
#[derive(StaticTree, Default)]
#[widget(stack)]
struct Screen {
    #[widget_id("content")]
    content: ScrollView<Rows>,
    #[widget_id("bar")]
    bar: Glass<Bar>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Panel {
    #[widget_id("glass")]
    glass: Glass<Bar>,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 300.0 };

fn build_panel() -> (Panel, Vec<fulgurite::Style>, [NodeState; Panel::NODE_COUNT]) {
    let mut ui = Panel::default();
    ui.glass.content.title.set("Files");
    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Panel::NODE_COUNT];
    pass::layout(&mut ui, &sty, &mut st, VIEW);
    (ui, sty, st)
}

fn glass_cmds(list: &DisplayList) -> Vec<(usize, [f32; 4], f32, Color)> {
    list.cmds
        .iter()
        .enumerate()
        .filter_map(|(k, c)| match c {
            Cmd::Glass { rect, material, .. } => Some((k, *rect, material.blur, material.tint)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_glass_panel_emits_one_command_over_its_own_rect() {
    let (ui, _sty, st) = build_panel();
    let mut list = DisplayList::new();
    pass::paint(&ui, &st, &mut list);
    let g = glass_cmds(&list);
    assert_eq!(g.len(), 1, "one panel, one command: {:?}", list.cmds);
    assert_eq!(g[0].1, fulgurite::ops::rect_of(&st[1]), "over the panel's own rect");
    assert_eq!(
        g[0].2,
        fulgurite::display::Material::FROSTED.blur,
        "and the widget's blur reaches the command"
    );
}

/// The content paints **after** the panel, so a button on a toolbar is
/// on top of the frost rather than under it.
#[test]
fn content_paints_on_top_of_the_panel() {
    let (ui, _sty, st) = build_panel();
    let mut list = DisplayList::new();
    pass::paint(&ui, &st, &mut list);
    let g = glass_cmds(&list)[0].0;
    let text = list
        .cmds
        .iter()
        .position(|c| matches!(c, Cmd::Text { .. }))
        .expect("the title must paint");
    assert!(g < text, "glass at {g} must precede its content at {text}");
}

/// `Style::background` is ignored, not silently composited under the
/// glass — see the module note. A fill underneath is what the blur would
/// read, and an opaque one blurs to itself.
#[test]
fn a_background_is_not_painted_under_the_glass() {
    let (mut ui, mut sty, mut st) = build_panel();
    sty[1].background = Color(255, 0, 0, 255);
    pass::layout(&mut ui, &sty, &mut st, VIEW);
    let mut list = DisplayList::new();
    pass::paint(&ui, &st, &mut list);
    let fills: Vec<&Cmd> = list
        .cmds
        .iter()
        .filter(|c| matches!(c, Cmd::RoundedRect { color, .. } if *color == Color(255, 0, 0, 255)))
        .collect();
    assert!(fills.is_empty(), "the background was painted under the glass: {fills:?}");
    assert_eq!(glass_cmds(&list).len(), 1, "and the panel still drew");
}

/// A hidden panel draws nothing **and neither does its content** — the
/// `Style::visible` rule every other widget follows.
///
/// The second half is the half with teeth. The panel's own command
/// disappears for free: `resolve_self` zeroes a hidden node's size and
/// `DisplayList::glass` drops a zero-area rect, so removing this
/// widget's `hide_subtree` call leaves that assertion green. What
/// `hide_subtree` is actually for is the **descendants** — F1.6 found a
/// hidden panel still on screen because only its own node was zeroed —
/// so the content's geometry is what this has to check.
#[test]
fn a_hidden_panel_paints_nothing_and_neither_does_its_content() {
    let (mut ui, mut sty, mut st) = build_panel();
    sty[1].visible = false;
    pass::layout(&mut ui, &sty, &mut st, VIEW);
    let mut list = DisplayList::new();
    pass::paint(&ui, &st, &mut list);
    assert!(glass_cmds(&list).is_empty(), "a hidden panel emitted glass");
    let content = st[2].size;
    assert_eq!(
        (content.w, content.h),
        (0.0, 0.0),
        "the content kept last frame's geometry: {content:?}"
    );
}

/// The panel takes its corner radius from its own style, the way every
/// other container does.
#[test]
fn the_radius_comes_from_style() {
    let (mut ui, mut sty, mut st) = build_panel();
    sty[1].radius = 14.0;
    pass::layout(&mut ui, &sty, &mut st, VIEW);
    let mut list = DisplayList::new();
    pass::paint(&ui, &st, &mut list);
    let r = list
        .cmds
        .iter()
        .find_map(|c| match c {
            Cmd::Glass { radius, .. } => Some(*radius),
            _ => None,
        })
        .expect("a glass command");
    assert_eq!(r, 14.0);
}

/// **The composition the milestone exists for.** In a stack, the panel
/// shares the content's rect and is emitted after it — so what it reads
/// is the content.
#[test]
fn in_a_stack_the_panel_covers_and_follows_the_content() {
    let mut ui = Screen::default();
    ui.content.content.a.w = 300.0;
    ui.content.content.a.h = 120.0;
    ui.content.content.b.w = 300.0;
    ui.content.content.b.h = 120.0;
    ui.bar.content.title.set("Files");
    let mut sty = pass::default_styles(&ui);
    // The bar is a top strip, not the whole screen.
    let bar = 1 + ScrollView::<Rows>::NODE_COUNT;
    sty[bar].height = Length::Fixed(48.0);
    let mut st = [NodeState::default(); Screen::NODE_COUNT];
    pass::layout(&mut ui, &sty, &mut st, VIEW);

    let content_r = fulgurite::ops::rect_of(&st[1]);
    let bar_r = fulgurite::ops::rect_of(&st[bar]);
    // Assert the setup: they must actually overlap, or this proves
    // nothing about a backdrop.
    let over = (bar_r[2].min(content_r[2]) - bar_r[0].max(content_r[0])).max(0.0)
        * (bar_r[3].min(content_r[3]) - bar_r[1].max(content_r[1])).max(0.0);
    println!("bar {bar_r:?} over content {content_r:?}, overlap {over} px");
    assert!(over > 1000.0, "the bar must cover content to have anything to blur");

    let mut list = DisplayList::new();
    pass::paint(&ui, &st, &mut list);
    let g = glass_cmds(&list);
    assert_eq!(g.len(), 1, "one panel");
    // Something the panel blurs must have been emitted before it.
    let before = list.cmds[..g[0].0]
        .iter()
        .filter(|c| !matches!(c, Cmd::PushClip { .. } | Cmd::PopClip))
        .count();
    assert!(before > 0, "the panel is first in the list, so it reads nothing");
}
