//! M25/T25.3: `Host::set_palette` reaches `layout`, under the strategy
//! that can skip it.
//!
//! **Every gate here runs on `Boundaries<16>`.** Under the default
//! `Everything` every node is laid out every frame, so a `set_palette`
//! that marked nothing would still reach layout on the next pump and none
//! of these could fail — which is why the plan's first draft, which said
//! the setter "damages" the frame, would have passed with the mark
//! deleted (`docs/plan-appearance.md` §5).
//!
//! The probe is a leaf that records the palette its `layout` saw and
//! counts how many times it was laid out, so "the palette reached layout"
//! is asserted as a layout that ran, not inferred from a frame.

use std::cell::Cell;

use lumen_static::desc::{NodeDesc, NodeState, NO_PARENT};
use lumen_static::display::{Cmd, Color, Painter};
use lumen_static::event::{EventCtx, Handlers, NodeSel};
use lumen_static::invalidate::Boundaries;
use lumen_static::ops::{record_painted, resolve_self};
use lumen_static_widgets::ops::{Label};
use lumen_static::style::{Palette, Patch, Role, Rule, Seed, Selector, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{leaf_common, App as DeriveApp, Constraints, LayoutCx, Size, StaticTree};
use lumen_static_shell::Host;

#[derive(Default)]
struct Probe {
    seen: Cell<Option<Color>>,
    layouts: Cell<u32>,
}

impl Widget for Probe {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];
    const KIND: &'static str = "palette-probe";
    const SEEDS: &'static [Seed] = &[Seed::unlinted(Self::KIND, Self::type_defaults)];
}

impl Node for Probe {
    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        self.seen.set(Some(cx.palette.text));
        self.layouts.set(self.layouts.get() + 1);
        let size = resolve_self(limits, &style[i], Size { w: 10.0, h: 10.0 });
        record_painted(st, i, limits, size, &style[i])
    }

    fn paint(&self, _st: &[NodeState], _i: usize, _out: &mut dyn Painter) {}

    leaf_common!(Probe, 0, nohit);
}

impl<S> Handlers<S> for Probe {
    fn handler(&self, _sel: NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        None
    }
}

/// The probe inside a container, so a skipped layout is the derive's
/// skip — the one `Boundaries` performs.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Panel {
    probe: Probe,
    #[widget_id("note")]
    note: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    panel: Panel,
}

#[derive(DeriveApp, Default)]
struct Themed {
    #[tree]
    ui: Ui,
}

type Bounded = Host<Themed, lumen_static::Incremental, Boundaries<16>>;

/// A host after two pumps, so the second was a still frame and the set is
/// empty — the state in which a frame can skip.
fn settled() -> Bounded {
    let mut h: Bounded = Host::new(Themed::default(), 200, 100);
    h.pump();
    h.pump();
    assert!(h.changed().can_skip(), "setup: a still frame leaves nothing marked");
    h
}

fn layouts(h: &Bounded) -> u32 {
    h.app.ui.panel.probe.layouts.get()
}

/// The colour the label's text was painted in last frame — read off the
/// display list, so a style that was cascaded but never laid out and
/// painted does not count.
fn note_colour(h: &Bounded) -> Option<Color> {
    let colours: Vec<Color> = h
        .list
        .cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::Text { color, .. } => Some(*color),
            _ => None,
        })
        .collect();
    assert_eq!(colours.len(), 1, "setup: the label paints exactly one run of text: {colours:?}");
    colours.first().copied()
}

/// `settled`, with text in the label so it paints something to read.
fn settled_with_text() -> Bounded {
    let mut app = Themed::default();
    app.ui.panel.note = Label::new("note");
    let mut h: Bounded = Host::new(app, 200, 100);
    h.pump();
    h.pump();
    assert!(h.changed().can_skip(), "setup: a still frame leaves nothing marked");
    h
}

/// **`Accent`, not `Text`**: since M57/A8 a label's own default *is* the
/// `Text` role, so a rule naming `Text` would change nothing a test could
/// see. The claim is about the rule's role, so it names one no label
/// defaults to.
fn text_role() -> [Rule; 1] {
    [Rule::new(Selector::Id("note"), Patch::new().color_role(Role::Accent))]
}

/// M25/T25.11: a rule that names a role follows the host's palette — at
/// `set_stylesheet` and again at every `set_palette`.
///
/// **Dark first**, so the sheet is applied under a palette that is not
/// the default: `set_stylesheet` resolving against `Palette::LIGHT`
/// instead of the host's would otherwise go unseen.
#[test]
fn a_role_in_the_sheet_follows_set_palette() {
    let mut h = settled_with_text();
    let default = note_colour(&h);
    assert_ne!(default, Some(Palette::DARK.accent), "setup: the label's default is not dark text");
    assert_ne!(default, Some(Palette::LIGHT.accent), "setup: the label's default is not light text");
    assert_ne!(Palette::DARK.accent, Palette::LIGHT.accent, "setup: the palettes differ where probed");

    h.set_palette(Palette::DARK);
    h.set_stylesheet(&text_role());
    h.pump();
    let under_dark = note_colour(&h);
    println!("default {default:?}; after the sheet, under dark: {under_dark:?}");
    assert_eq!(
        under_dark,
        Some(Palette::DARK.accent),
        "set_stylesheet resolved the role against a palette other than the host's"
    );

    h.pump();
    assert!(h.changed().can_skip(), "setup: the frame settled before the swap");
    h.set_palette(Palette::LIGHT);
    h.pump();
    let under_light = note_colour(&h);
    println!("after set_palette(LIGHT): {under_light:?}");
    assert_eq!(
        under_light,
        Some(Palette::LIGHT.accent),
        "set_palette did not re-resolve the sheet's role: the label kept last palette's colour"
    );
}

/// A scoped restyle resolves against the host's palette too — otherwise a
/// class toggle under a dark theme would paint that subtree light.
#[test]
fn restyle_subtree_resolves_roles_against_the_host_palette() {
    let mut h = settled_with_text();
    h.set_palette(Palette::DARK);
    h.pump();
    assert!(h.restyle_subtree(&text_role(), "note"), "setup: the id matched");
    h.pump();
    let painted = note_colour(&h);
    println!("restyled under dark: {painted:?}");
    assert_eq!(
        painted,
        Some(Palette::DARK.accent),
        "restyle_subtree resolved the role against a palette other than the host's"
    );
}

/// A rule that names a colour, not a role, keeps it under every palette —
/// the half of theming a role must not take over.
#[test]
fn a_plain_colour_does_not_follow_the_palette() {
    const INK: Color = Color(90, 10, 10, 255);
    let mut h = settled_with_text();
    h.set_stylesheet(&[Rule::new(Selector::Id("note"), Patch::new().color(INK))]);
    h.pump();
    h.set_palette(Palette::DARK);
    h.pump();
    assert_eq!(note_colour(&h), Some(INK));
}

#[test]
fn a_still_frame_does_not_lay_the_probe_out() {
    // The premise every other test rests on: without a mark, the probe is
    // skipped. If this failed the other tests would pass for the wrong
    // reason — layout running every frame.
    let mut h = settled();
    let before = layouts(&h);
    h.pump();
    println!("layouts: {before} -> {}", layouts(&h));
    assert_eq!(layouts(&h), before, "a still frame laid the probe out: the skip never fires");
}

#[test]
fn a_new_palette_reaches_layout_on_the_next_frame() {
    let mut h = settled();
    assert_eq!(*h.palette(), Palette::LIGHT);
    assert_ne!(Palette::DARK.text, Palette::LIGHT.text, "setup: the palettes differ where probed");
    let before = layouts(&h);

    h.set_palette(Palette::DARK);
    h.pump();

    let seen = h.app.ui.panel.probe.seen.get();
    println!("layouts: {before} -> {}; seen {seen:?}, dark {:?}", layouts(&h), Palette::DARK.text);
    assert!(layouts(&h) > before, "set_palette did not cause a layout: the probe kept last frame");
    assert_eq!(
        seen,
        Some(Palette::DARK.text),
        "layout ran against a palette other than the host's"
    );
}

#[test]
fn an_equal_palette_marks_nothing() {
    let mut h = settled();
    let before = layouts(&h);
    h.set_palette(Palette::LIGHT);
    assert!(h.changed().can_skip(), "setting the palette the host already has marked the tree");
    h.pump();
    println!("layouts: {before} -> {}", layouts(&h));
    assert_eq!(layouts(&h), before, "an unchanged palette still laid the probe out");
}
