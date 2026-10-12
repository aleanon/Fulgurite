//! **Every widget's own colours follow the palette, and read** — M57/A8
//! (`docs/plan-widget-fixes.md`).
//!
//! The palette reached a node only through a stylesheet rule's
//! `color_role`, and every widget's `type_defaults` were light-palette
//! literals. So an application had to restyle every kind for dark mode,
//! and the showcase did it for four: a label read 1.30:1, a card stayed
//! white on a dark page, a modal's title was black on its dark panel
//! (`docs/widget-audit.md` X6). A kind now declares its colours as
//! [`Seed::roles`](fulgurite::style::Seed), and the cascade resolves
//! them against the host's palette with no rule at all.
//!
//! Two claims per kind, under **both** palettes, with an **empty** sheet:
//! 1. every colour field the kind paints with is a role, not a literal —
//!    so no kind can quietly be left behind;
//! 2. each pair a user has to tell apart meets its bound: 4.5:1 for text
//!    (WCAG 1.4.3), 3:1 for a control's body and edge (WCAG 1.4.11).
//!
//! Colours that are **data** stay literals and are not checked here: a
//! chart's series and a colour picker's swatches.

use fulgurite::display::Color;
use fulgurite::style::{Fields, Palette, Seed};
use fulgurite::Widget;
use fulgurite_widgets::ops::{Button, Label};
use fulgurite_widgets::*;

/// Which colour a check reads.
#[derive(Clone, Copy, Debug)]
enum F {
    Fg,
    Bg,
    Border,
    Page,
}

/// A kind and the pairs it must keep apart: (a, b, bound, why).
struct Kind {
    seed: Seed,
    pairs: &'static [(F, F, f32, &'static str)],
}

const TEXT: f32 = 4.5;
const UI: f32 = 3.0;

fn kinds() -> Vec<Kind> {
    let k = |seeds: &'static [Seed], pairs| Kind { seed: seeds[0], pairs };
    vec![
        k(<Label as Widget>::SEEDS, &[(F::Fg, F::Page, TEXT, "text on the page")]),
        k(<Button<()> as Widget>::SEEDS, &[(F::Fg, F::Bg, TEXT, "label on its body")]),
        k(<TextInput as Widget>::SEEDS, &[(F::Fg, F::Page, TEXT, "text")]),
        k(<TextArea as Widget>::SEEDS, &[
            (F::Fg, F::Bg, TEXT, "text on its ground"),
            (F::Border, F::Page, UI, "the field's edge (M57/B8)"),
        ]),
        k(<RichText as Widget>::SEEDS, &[(F::Fg, F::Page, TEXT, "text")]),
        k(<Markdown as Widget>::SEEDS, &[(F::Fg, F::Page, TEXT, "text")]),
        k(<CheckBox<()> as Widget>::SEEDS, &[
            (F::Bg, F::Page, UI, "an unchecked box is seen"),
            (F::Fg, F::Page, UI, "a checked box is seen"),
        ]),
        k(<Switch<()> as Widget>::SEEDS, &[(F::Bg, F::Page, UI, "an off track is seen")]),
        k(<Radio<()> as Widget>::SEEDS, &[(F::Bg, F::Page, UI, "an unselected ring is seen")]),
        k(<Slider as Widget>::SEEDS, &[(F::Fg, F::Page, UI, "the filled part carries the value")]),
        k(<RangeSlider as Widget>::SEEDS, &[(F::Fg, F::Page, UI, "handles and range")]),
        k(<ProgressBar as Widget>::SEEDS, &[(F::Fg, F::Page, UI, "the fill carries the value")]),
        k(<Spinner as Widget>::SEEDS, &[(F::Fg, F::Page, UI, "the ring is seen")]),
        k(<NumberInput as Widget>::SEEDS, &[
            (F::Fg, F::Bg, TEXT, "the number"),
            (F::Border, F::Page, UI, "the field's edge"),
        ]),
        k(<SearchField as Widget>::SEEDS, &[(F::Border, F::Page, UI, "the pill's edge")]),
        k(<Pagination as Widget>::SEEDS, &[(F::Fg, F::Bg, TEXT, "page numbers")]),
        k(<Tree as Widget>::SEEDS, &[(F::Fg, F::Bg, TEXT, "labels")]),
        k(<Toast as Widget>::SEEDS, &[(F::Fg, F::Bg, TEXT, "the message on its pill")]),
        k(<Badge as Widget>::SEEDS, &[(F::Fg, F::Bg, TEXT, "the count")]),
        k(<Chip as Widget>::SEEDS, &[(F::Fg, F::Bg, TEXT, "the tag")]),
        k(<Avatar as Widget>::SEEDS, &[(F::Fg, F::Bg, TEXT, "the initials")]),
        k(<Icon as Widget>::SEEDS, &[(F::Fg, F::Page, UI, "the mark")]),
        k(<AppBar as Widget>::SEEDS, &[(F::Fg, F::Bg, TEXT, "the title")]),
        k(<Tabs as Widget>::SEEDS, &[(F::Fg, F::Page, TEXT, "tab labels")]),
        k(<BottomNav as Widget>::SEEDS, &[(F::Fg, F::Bg, TEXT, "item labels")]),
        k(<NavigationRail as Widget>::SEEDS, &[(F::Fg, F::Bg, TEXT, "item labels")]),
        k(<Collapsible<Label> as Widget>::SEEDS, &[(F::Fg, F::Bg, TEXT, "the header")]),
        k(<SplitPane<Label, Label> as Widget>::SEEDS, &[(F::Fg, F::Page, UI, "the divider")]),
        k(<PaneGrid<Label, Label, Label, Label> as Widget>::SEEDS, &[(F::Fg, F::Bg, UI, "the dividers on the panes")]),
        k(<Card<Label> as Widget>::SEEDS, &[]),
        k(<Modal<Label> as Widget>::SEEDS, &[]),
        k(<Divider as Widget>::SEEDS, &[]),
        k(<Skeleton as Widget>::SEEDS, &[]),
        k(<PaintBox as Widget>::SEEDS, &[]),
        // M82/T82.8: its run is text, whether drawn as outlines or not.
        k(<VectorText as Widget>::SEEDS, &[(F::Fg, F::Page, TEXT, "the run on the page")]),
        k(<BarChart as Widget>::SEEDS, &[
            (F::Fg, F::Page, UI, "the default series colour"),
            (F::Border, F::Page, TEXT, "the axes' labels (M57/B30)"),
        ]),
        k(<PullToRefresh<Label> as Widget>::SEEDS, &[(F::Fg, F::Page, UI, "the pull indicator")]),
    ]
}

/// The sub-seeds of composite widgets whose colours are their own: an
/// option list, a picker's trigger and pad.
fn parts() -> Vec<Kind> {
    let find = |seeds: &'static [Seed], kind: &str| *seeds.iter().find(|s| s.kind == kind).unwrap();
    vec![
        Kind { seed: find(<PickList as Widget>::SEEDS, "optionlist"), pairs: &[(F::Fg, F::Bg, TEXT, "options")] },
        Kind {
            seed: find(<ColorPicker as Widget>::SEEDS, "picker-trigger"),
            pairs: &[(F::Fg, F::Bg, TEXT, "the value"), (F::Border, F::Page, UI, "the field's edge")],
        },
    ]
}

fn pick(f: F, s: &fulgurite::Style, p: &Palette) -> Color {
    match f {
        F::Fg => s.color,
        F::Bg => s.background,
        F::Border => s.border_color,
        F::Page => p.page,
    }
}

fn contrast(a: Color, b: Color) -> f32 {
    fn lum(c: Color) -> f32 {
        let f = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * f(c.0) + 0.7152 * f(c.1) + 0.0722 * f(c.2)
    }
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

#[test]
fn every_colour_a_widget_paints_with_is_a_role() {
    let mut literal = Vec::new();
    for k in kinds().into_iter().chain(parts()) {
        let d = (k.seed.defaults)();
        let r = k.seed.roles;
        let reads = k.seed.reads;
        // A colour the kind actually shows: one its seed says it reads,
        // opaque, or a border it draws.
        if reads.contains(Fields::COLOR) && d.color.3 > 0 && r.color.is_none() {
            literal.push(format!("{}: color {:?}", k.seed.kind, d.color));
        }
        if reads.contains(Fields::BACKGROUND) && d.background.3 > 0 && r.background.is_none() {
            literal.push(format!("{}: background {:?}", k.seed.kind, d.background));
        }
        if d.border_width > 0 && r.border.is_none() {
            literal.push(format!("{}: border {:?}", k.seed.kind, d.border_color));
        }
    }
    assert!(literal.is_empty(), "colours that ignore the palette:\n{}", literal.join("\n"));
}

#[test]
fn every_widget_reads_in_both_palettes() {
    let mut fails = Vec::new();
    for (name, p) in [("light", Palette::LIGHT), ("dark", Palette::DARK)] {
        for k in kinds().into_iter().chain(parts()) {
            let s = k.seed.style(&p);
            for &(a, b, bound, why) in k.pairs {
                let (ca, cb) = (pick(a, &s, &p), pick(b, &s, &p));
                let r = contrast(ca, cb);
                println!("{name:5} {:14} {why:34} {r:5.2}:1 (bound {bound:.1})", k.seed.kind);
                if r < bound {
                    fails.push(format!("{name} {}: {why} is {r:.2}:1 ({ca:?} on {cb:?}), under {bound}", k.seed.kind));
                }
            }
        }
    }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}
