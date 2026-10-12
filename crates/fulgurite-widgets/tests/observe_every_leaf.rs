//! **Every leaf widget tells a reader what it is** — M57/A17
//! (`docs/plan-widget-fixes.md`).
//!
//! Badge, Chip, Avatar, Icon, Toast, the three charts, ProgressBar,
//! Spinner and Skeleton published `GenericContainer` with no value
//! (`docs/widget-audit.md` X9): the agent could not read a badge's count,
//! and a screen reader met a progress bar as an empty box. Every leaf in
//! this tree must publish a role other than `GenericContainer`, or a
//! value, or be hidden deliberately (a skeleton is busy and hidden).
#![cfg(feature = "observe")]

use fulgurite::observe::{emit_root, AkNode, NodeId};
use fulgurite::prelude::*;
use fulgurite::{pass, Constraints};

use fulgurite_widgets::*;
use fulgurite_widgets::ops::{Button, Label};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 2.0)]
struct Ui {
    label: Label<String>,
    button: Button<()>,
    input: TextInput,
    check: CheckBox<()>,
    slider: Slider,
    badge: Badge,
    chip: Chip,
    avatar: Avatar,
    icon: Icon,
    bars: BarChart,
    line: LineChart,
    pie: PieChart,
    progress: ProgressBar,
    spinner: Spinner,
    skeleton: Skeleton,
    toast: Toast,
    rich: RichText,
    pages: Pagination,
    number: NumberInput,
    range: RangeSlider,
}

fn entries() -> Vec<(NodeId, AkNode, &'static str)> {
    let mut ui = Ui::default();
    ui.label.set("text".into());
    ui.button.label = "Go";
    ui.badge = Badge::new("3");
    ui.chip = Chip::new("tag");
    ui.avatar.set("AB");
    ui.bars.data = Series::new(&[1.0, 2.0]);
    ui.progress = ProgressBar::new(0.4);
    ui.toast = Toast::new("Saved");
    ui.rich = RichText::new("some words");
    let styles = pass::default_styles(&ui);
    let mut st = vec![fulgurite::NodeState::default(); <Ui as Widget>::NODE_COUNT];
    pass::layout(&mut ui, &styles, &mut st, Constraints { min_w: 0.0, max_w: 600.0, min_h: 0.0, max_h: 2000.0 });
    let table = <Ui as Widget>::SUB_TABLE;
    let mut out = Vec::new();
    emit_root(table, &ui, &st, &|i: usize| (i + 1) as u64, &mut out);
    out.into_iter()
        .map(|(id, n)| {
            let i = id.0 as usize - 1;
            let kind = if i < table.len() { <Ui as Widget>::SEEDS[i].kind } else { "?" };
            (id, n, kind)
        })
        .collect()
}

#[test]
fn every_leaf_publishes_a_role_or_a_value() {
    let table = <Ui as Widget>::SUB_TABLE;
    let mut silent = Vec::new();
    let mut leaves = 0;
    for (id, n, kind) in entries() {
        let i = id.0 as usize - 1;
        if i >= table.len() || table[i].child_count > 0 {
            continue;
        }
        leaves += 1;
        let says_something = n.role() != fulgurite::observe::accesskit::Role::GenericContainer
            || n.value().is_some()
            || n.label().is_some()
            || n.is_hidden();
        if !says_something {
            silent.push(kind);
        }
    }
    println!("{leaves} leaves; silent: {silent:?}");
    assert!(leaves >= 20, "setup: only {leaves} leaves were emitted");
    assert!(silent.is_empty(), "leaves that tell a reader nothing: {silent:?}");
}

/// **What a reader hears is what the layout spaces** — M57/B6. The join
/// put a space between every pair of spans, so "**bold**, `code`." was
/// published as "bold , code ." while the screen showed "bold, code.".
#[test]
fn rich_text_publishes_its_own_spacing() {
    use fulgurite::observe::accesskit as ak;
    let m = Markdown::new("**bold**, `code`. plain words");
    let st = [fulgurite::NodeState::default()];
    let mut n = ak::Node::new(ak::Role::Unknown);
    let mut extra = Vec::new();
    fulgurite::widget::Node::observe(&m, &st, 0, 1, &mut n, &mut extra);
    let v = n.value().unwrap_or("").to_string();
    println!("spans {:?} -> {v:?}", (0..m.rich().spans().len()).map(|k| m.rich().span_str(k)).collect::<Vec<_>>());
    assert_eq!(v, "bold, code. plain words");
}
