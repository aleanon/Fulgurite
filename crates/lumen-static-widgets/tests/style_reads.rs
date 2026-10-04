//! M25/T25.2: every kind's declared [`Seed::reads`] against what its
//! output actually does.
//!
//! The sheet lint reports a rule that sets a field no matched node
//! reads. That is only as true as the masks, and a mask written by hand
//! is a claim — so this measures one. For each kind, for each `Style`
//! field, it renders the kind twice under a stylesheet that differs
//! **only in that field**, in four pointer states, and records whether
//! the display list or any node's geometry changed. The declared mask
//! must equal the measured set in both directions:
//!
//! - a field declared and not measured is a mask that lies, and the lint
//!   would stay silent on a rule that does nothing;
//! - a field measured and not declared is a lint that fires on a rule
//!   that works.
//!
//! `cursor` changes no frame — the host reads it off the hovered node
//! (`Host::cursor`) — so it counts as read exactly when a hit at the
//! kind's centre lands on the kind itself.
//!
//! **Why every render sets a base background, colour and size.** A
//! transparent background hides `radius`, `shadow` and both dims; a
//! shrink-wrapped container hides `align`. The base makes each field
//! able to show, and the setup assertion below checks the probe rule
//! landed at all: every kind must honour `visible`, so a kind whose
//! measured set lacks it is a probe that never reached its node.

use lumen_static::display::DisplayList;
use lumen_static_widgets::ops::{Button, Fixed, Label, Spacer};
use lumen_static::repeat::Repeat;
use lumen_static_widgets::select::Select;
use lumen_static::style::{Cursor, Fields, Seed};
use lumen_static::{pass, Align, Color, Constraints, Dyn, Edges, Either, ImageId, Length, List, Node, NodeState, Patch, Point, Rule, Selector, Size, StaticTree, Widget};
use lumen_static_widgets::{CheckBox, Glass, Image, Popup, Radio, ScrollView, Slider, Switch, TextArea, TextInput};
use lumen_static_widgets::prelude::*;

const LIMITS: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 300.0 };

#[derive(Clone, Copy, Debug)]
enum Scenario {
    Plain,
    Hovered,
    Pressed,
    Focused,
}

const SCENARIOS: [Scenario; 4] =
    [Scenario::Plain, Scenario::Hovered, Scenario::Pressed, Scenario::Focused];

/// A base every render shares, so each field has something to act on.
const BASE: Patch = Patch::new()
    .background(Color(90, 90, 90, 255))
    .color(Color(30, 30, 30, 255))
    .width(Length::Fixed(200.0))
    .height(Length::Fixed(60.0));

/// The same base at the node's own size.
///
/// **Needed because a fixed size hides every field that only changes
/// intrinsic size.** Measured under `BASE` alone, `Button` read no
/// `padding`: its layout adds padding to the text's measured box, and a
/// fixed 200×60 swallows the difference, while its paint centres the
/// text in the box and never looks at the inset. A `button` padding rule
/// plainly works, so a mask declared from that measurement would have
/// made the lint fire on it. Each field is measured under both bases.
const BASE_SHRINK: Patch =
    Patch::new().background(Color(90, 90, 90, 255)).color(Color(30, 30, 30, 255));

/// A value for `name` that differs from every kind's default and from
/// `BASE`.
fn patch_for(name: &str) -> Patch {
    let p = Patch::new();
    match name {
        "width" => p.width(Length::Fixed(173.0)),
        "height" => p.height(Length::Fixed(71.0)),
        "padding" => p.padding(Edges::all(9.5)),
        "spacing" => p.spacing(13.0),
        "align" => p.align(Align::Center),
        "cursor" => p.cursor(Cursor::Grab),
        "focus_ring" => p.focus_ring(5),
        "visible" => p.visible(false),
        "color" => p.color(Color(11, 200, 33, 255)),
        "background" => p.background(Color(200, 40, 90, 255)),
        "radius" => p.radius(7.5),
        "font_size" => p.font_size(23.0),
        "font_weight" => p.font_weight(800.0),
        "shadow" => p.shadow(6.0),
        "pressed_dim" => p.pressed_dim(0.5),
        "hover_dim" => p.hover_dim(0.5),
        // One probe for the pair: a width with no colour draws
        // nothing and a colour with no width is invisible, which is
        // why `Fields::BORDER` is one bit (M27/T27.11).
        "border" => p.border_width(3).border_color(Color(250, 10, 10, 255)),
        // M80/T80.2: 1.5x changes even a single line's height (CSS's
        // rule, "a single line included"), and the paint byte either way.
        "line_height" => p.line_height(1.5),
        other => panic!("no probe value for field {other}"),
    }
}

type Frame = (u64, usize, Vec<(Size, Point)>, bool);

fn probe_index<R: Widget>() -> usize {
    lumen_static::style::index_of_wid(R::SUB_TABLE, "probe").expect("the probe root carries #probe")
}

fn render<R: Widget + Node + Default>(setup: fn(&mut R), sheet: &[Rule], sc: Scenario) -> Frame {
    let mut root = R::default();
    setup(&mut root);
    let mut styles = pass::default_styles(&root);
    lumen_static::style::cascade_deep(&mut root, sheet, &mut styles);
    let i = probe_index::<R>();
    let mut st = vec![NodeState::default(); R::NODE_COUNT];
    match sc {
        Scenario::Plain => {}
        Scenario::Hovered => st[i].hovered = true,
        Scenario::Pressed => {
            st[i].hovered = true;
            st[i].pressed = true;
        }
        Scenario::Focused => st[i].focused = true,
    }
    pass::layout(&mut root, &styles, &mut st, LIMITS);
    let mut list = DisplayList::new();
    pass::paint(&root, &st, &mut list);
    // **The host's ring, as the host draws it** — M57/A6. No widget draws
    // its own ring any more; the host does, once, around the keyboard-
    // focused node, reading that node's `focus_ring` and `radius`. So a
    // focused kind's output is its paint *plus* that ring, or every
    // focusable kind would measure as not reading `focus_ring`.
    if matches!(sc, Scenario::Focused) {
        lumen_static::ops::focus_ring(&st[i], Color(0, 0, 255, 255), &mut list);
    }
    let geometry = st.iter().map(|s| (s.size, s.pos)).collect();
    let c = st[i];
    let (x, y) = (c.pos.x + c.size.w / 2.0, c.pos.y + c.size.h / 2.0);
    let hits_itself = pass::hit_test(&root, &st, x, y).is_some_and(|h| h.node as usize == i);
    (list.hash_from(0), list.len(), geometry, hits_itself)
}

/// The fields `R`'s probed node is measured to read, in one instance
/// state.
///
/// **Only the pointer states the node can actually be in.** The first
/// version set `hovered` and `pressed` on every kind, so a badge — which
/// no hit ever lands on — measured as reading `hover_dim`, and a rule
/// giving it one would have gone unreported while doing nothing on
/// screen. Hovered and pressed are probed only where a hit lands on the
/// node itself; focused only where the table marks it focusable.
fn measured<R: Widget + Node + Default>(setup: fn(&mut R)) -> Fields {
    let i = probe_index::<R>();
    let focusable = R::SUB_TABLE[i].is_focusable();
    let mut read = Fields::NONE;
    for base_patch in [BASE, BASE_SHRINK] {
        let base = Rule::new(Selector::Id("probe"), base_patch);
        let hittable = render(setup, &[base], Scenario::Plain).3;
        for (field, name) in Fields::EACH {
            if name == "cursor" {
                if hittable {
                    read = read.union(field);
                }
                continue;
            }
            let with = [base, Rule::new(Selector::Id("probe"), patch_for(name))];
            for sc in SCENARIOS {
                let reachable = match sc {
                    Scenario::Plain => true,
                    Scenario::Hovered | Scenario::Pressed => hittable,
                    Scenario::Focused => focusable,
                };
                if !reachable {
                    continue;
                }
                let (a, b) = (render(setup, &[base], sc), render(setup, &with, sc));
                if (a.0, a.1, &a.2) != (b.0, b.1, &b.2) {
                    read = read.union(field);
                    break;
                }
            }
        }
    }
    read
}

/// Measured over every instance state a kind is given — a toggle paints
/// `background` only when it is off, so probing it on alone under-reads.
fn check<R: Widget + Node + Default>(setups: &[fn(&mut R)]) -> (&'static str, Fields, Fields) {
    let mut read = Fields::NONE;
    for setup in setups {
        read = read.union(check_one::<R>(*setup));
    }
    let seed: Seed = R::SEEDS[probe_index::<R>()];
    (seed.kind, seed.reads, read)
}

fn check_one<R: Widget + Node + Default>(setup: fn(&mut R)) -> Fields {
    // Setup: the probe rule reaches the node's style row. Asserted on the
    // *cascade's* answer, not on the frame — a kind may legitimately read
    // nothing (an `Either` forwards to its showing child), and a setup
    // check that assumed every kind reads `visible` fired on exactly the
    // kind this lint exists to find.
    let mut root = R::default();
    setup(&mut root);
    let mut styles = pass::default_styles(&root);
    let rule = Rule::new(Selector::Id("probe"), BASE.visible(false));
    lumen_static::style::cascade_deep(&mut root, &[rule], &mut styles);
    let i = probe_index::<R>();
    assert!(
        !styles[i].visible && styles[i].width == Length::Fixed(200.0),
        "{}: the probe rule did not reach the node's style row",
        R::SEEDS[i].kind
    );
    measured(setup)
}

/// A derived enum, whose kind is its own ident like a struct's.
#[derive(StaticTree)]
enum Stage {
    Words(Label<String>),
    Block(Fixed),
}

impl Default for Stage {
    fn default() -> Self {
        Stage::Words(label("stage"))
    }
}

/// A two-child derived container, so `spacing` and `align` can show.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 2.0)]
struct Pair {
    a: Label<String>,
    b: Label<String>,
}

fn label(text: &str) -> Label<String> {
    let mut l = Label::<String>::default();
    l.set(text.to_owned());
    l
}

fn bind_label(l: &mut Label<String>, row: usize) {
    l.set(format!("row {row}"));
}

fn bind_row(b: &mut lumen_static_widgets::select::Row<()>, row: usize) {
    b.label = format!("option {row}");
}

macro_rules! kinds {
    ($( $root:ident : $ty:ty = [ $($setup:expr),+ $(,)? ] ;)*) => {
        $(
            #[derive(StaticTree, Default)]
            #[widget(column, spacing = 3.0)]
            struct $root {
                #[widget_id("probe")]
                w: $ty,
                tail: Label,
            }
        )*

        /// Every kind in both crates, measured.
        fn all_kinds() -> Vec<(&'static str, Fields, Fields)> {
            vec![$( check::<$root>(&[$($setup),+]) ),*]
        }
    };
}

kinds! {
    PLabel: Label<String> = [|r| r.w.set("probe text".to_owned())];
    PFixed: Fixed = [|r| { r.w.w = 20.0; r.w.h = 10.0; }];
    PSpacer: Spacer = [|_| {}];
    PButton: Button<(), String> = [|r| r.w.label = "go".to_owned()];
    PCheck: CheckBox<()> = [|r| r.w.on = true, |r| r.w.on = false];
    PRadio: Radio<()> = [|r| r.w.on = true, |r| r.w.on = false];
    PSwitch: Switch<()> = [|r| r.w.on = true, |r| r.w.on = false];
    PSlider: Slider = [|r| r.w = Slider::new(0.5)];
    PInput: TextInput = [|r| r.w = TextInput::new("abc"), |r| r.w = TextInput::new("")];
    PArea: TextArea = [|r| r.w = TextArea::new("ab\ncd"), |r| r.w = TextArea::new("")];
    PScroll: ScrollView<Label<String>> = [|r| r.w.content.set("scrolled".to_owned())];
    PSelect: Select<()> = [
        |r| { r.w = Select::new(3, 20.0, bind_row); r.w.open(); },
        |r| r.w = Select::new(3, 20.0, bind_row),
    ];
    PRepeat: Repeat<Label<String>> = [|r| r.w = Repeat::new(3, 20.0, bind_label)];
    PImage: Image = [|r| r.w = Image::new(ImageId(1), Size { w: 40.0, h: 30.0 })];
    PGlass: Glass<Label<String>> = [|r| r.w.content.set("glass".to_owned())];
    PPopup: Popup<Label<String>> = [
        |r| { r.w.open = true; r.w.content.set("pop".to_owned()); },
        |r| r.w.content.set("pop".to_owned()),
    ];
    PEither: Either<Label<String>, Fixed> = [|r| r.w.a.set("a".to_owned())];
    PList: List<Label<String>> = [|r| r.w = List::new(vec![label("one"), label("two")])];
    PDyn: Dyn<()> = [|r| r.w = Dyn::new(label("dyn"))];
    PPair: Pair = [|r| { r.w.a.set("a".to_owned()); r.w.b.set("b".to_owned()); }];
    PStage: Stage = [|_| {}, |r| r.w = Stage::Block(Fixed { w: 20.0, h: 10.0 })];
    PDivider: Divider = [|_| {}];
    PBadge: Badge<String> = [|r| r.w.set("new".to_owned())];
    PChip: Chip<String> = [|r| r.w.set("tag".to_owned())];
    PAvatar: Avatar<String> = [|r| r.w.set("AB".to_owned())];
    PIcon: Icon = [|r| r.w = Icon::new(Symbol::Play)];
    PProgress: ProgressBar = [|r| r.w.value = 0.5];
    PSpinner: Spinner = [|r| r.w.phase = 0.3];
    PSkeleton: Skeleton = [|r| r.w.phase = 0.3];
    // With axes and labels: a chart's font is read only there (M57/B30),
    // and a fixture without them measured `font_size` as unread.
    PBar: BarChart = [|r| { r.w = BarChart::new(&[1.0, 3.0, 2.0]); r.w.axes = true; r.w.labels = &["a", "b", "c"]; }];
    PLine: LineChart = [|r| { r.w = LineChart::new(&[1.0, 3.0, 2.0]); r.w.axes = true; r.w.labels = &["a", "b", "c"]; }];
    PPie: PieChart = [|r| r.w = PieChart::new(&[1.0, 3.0, 2.0])];
    // M80/T80.2: one of the three kinds that read `line_height`, and not
    // measured before it.
    PRich: RichText = [|r| r.w = RichText::new("probe words wrap")];
}

fn names(f: Fields) -> String {
    f.names().collect::<Vec<_>>().join(", ")
}

#[test]
fn every_kind_declares_exactly_the_fields_its_output_reads() {
    let rows = all_kinds();
    let mut wrong = Vec::new();
    for (kind, declared, measured) in &rows {
        println!("{kind:<12} measured: {}", names(*measured));
        let lies = declared.minus(*measured);
        let omits = measured.minus(*declared);
        if !lies.is_empty() || !omits.is_empty() {
            wrong.push(format!(
                "{kind}: declares unread [{}]; omits read [{}]",
                names(lies),
                names(omits)
            ));
        }
    }
    println!("{} kinds measured", rows.len());
    assert!(wrong.is_empty(), "Seed::reads disagrees with the output:\n{}", wrong.join("\n"));
}
