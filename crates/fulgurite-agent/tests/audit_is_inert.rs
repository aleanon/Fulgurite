//! Observing a frame must not change it.
//!
//! `audit.rs`'s module doc names, as a scar adopted from Lumen's own
//! review rather than re-earned, that "Lumen's `lint()` was re-shaping
//! every text node per call". The intrinsic probe then measured every
//! leaf twice per frame *through `layout`*, which writes `Label`'s
//! measurement memo — so the memo ended each frame keyed on the probe's
//! constraints and missed on the next one. Measured before the fix: 0
//! text measurements per idle frame with no agent, 400 of 400 with the
//! agent attached. `docs/plan-review-fixes.md` F2.1.

use fulgurite::cx::TextMeasure;
use fulgurite_widgets::ops::{Fixed, Label};
use fulgurite::widget::Node;
use fulgurite::{pass, Constraints, LayoutCx, NodeState, Size, StaticTree, Widget};
use fulgurite_agent::{Agent, TreeMut};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("a")]
    a: Label,
    #[widget_id("b")]
    b: Label,
    mark: Fixed,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 400.0 };

/// Counts every call that reaches the text stack.
#[derive(Default)]
struct Counting {
    calls: std::cell::Cell<usize>,
}

impl TextMeasure for Counting {
    fn measure(&mut self, text: &str, size: f32, max_w: f32) -> Size {
        self.calls.set(self.calls.get() + 1);
        fulgurite::BlockMetrics.measure(text, size, max_w)
    }
}

struct Fixture {
    ui: Ui,
    st: [NodeState; Ui::NODE_COUNT],
    styles: Vec<fulgurite::Style>,
    agent: Agent,
    metrics: Counting,
}

impl Fixture {
    fn new() -> Self {
        let mut ui = Ui::default();
        ui.a.set("first label");
        ui.b.set("second label");
        ui.mark = Fixed { w: 10.0, h: 10.0 };
        let styles = pass::default_styles(&ui);
        Fixture {
            ui,
            st: [NodeState::default(); Ui::NODE_COUNT],
            styles,
            agent: Agent::new(),
            metrics: Counting::default(),
        }
    }

    /// One frame's layout, reporting how many measurements it needed.
    fn layout(&mut self) -> usize {
        let before = self.metrics.calls.get();
        let mut cx = LayoutCx::new(&mut self.metrics);
        self.ui.layout(&mut cx, VIEW, &self.styles, &mut self.st, 0);
        self.metrics.calls.get() - before
    }

    fn observe(&mut self) {
        let mut tree = TreeMut::of_root(&mut self.ui, &self.styles, &self.st);
        self.agent.frame_done(&mut tree, fulgurite_agent::FrameTiming::passes(1, 1));
    }
}

#[test]
fn observing_a_frame_does_not_force_a_re_measure_on_the_next_one() {
    let mut f = Fixture::new();
    assert_eq!(f.layout(), 2, "first frame measures both labels");
    assert_eq!(f.layout(), 0, "an idle frame measures nothing — the memo holds");

    f.observe();
    assert_eq!(
        f.layout(),
        0,
        "and it still measures nothing after the agent has looked at it"
    );

    // Repeatedly, since the failure was per-frame and unbounded.
    for _ in 0..10 {
        f.observe();
        assert_eq!(f.layout(), 0, "every subsequent frame too");
    }
}

#[test]
fn the_memo_stays_warm_across_frame_done() {
    let mut f = Fixture::new();
    f.layout();
    assert!(f.ui.a.measure_is_warm(VIEW, &f.styles[1], 1.0, 0), "warm before");
    f.observe();
    assert!(f.ui.a.measure_is_warm(VIEW, &f.styles[1], 1.0, 0), "and warm after — the probe left no trace");
}

#[test]
fn a_real_change_still_re_measures_exactly_one_label() {
    let mut f = Fixture::new();
    f.layout();
    f.observe();
    f.ui.b.set("a different second label");
    assert_eq!(f.layout(), 1, "only the label that changed");
}

// ---------------------------------------------------------------------
// M41/T41.0 (`docs/plan-boundaries-repair.md` §1.2, R1-F5): the audit's
// probe under the `Boundaries` default, in the window's order.
//
// `Node::probe`'s provided body is `self.layout(..)`, and `TextInput`
// and `TextArea` did not override it until M41/T41.4. Their `layout` writes the scroll
// offset from the limits it is handed, and the audit probes every leaf
// at 1e6 and 2e6 px — so after `frame_done` a scrolled field holds a
// scroll of zero. Under `Everything` the next frame's layout puts it
// back and nothing shows. Under `Boundaries` an unrelated hover marks
// only the hovered node, the field is skipped, and paint draws the
// probe's scroll.
//
// Built without `shadow-oracle` (rule 2): the oracle's second layout
// would repair the field before paint and hide the defect.
// ---------------------------------------------------------------------

mod bounded {
    use fulgurite::invalidate::{Everything, Invalidation};
    use fulgurite::prelude::*;
    use fulgurite::retain::Incremental;
    use fulgurite::style::{Length, Patch, Rule, Selector};
    use fulgurite::{Cmd, DisplayList, Event};
    use fulgurite_agent::{Agent, FrameTiming, TreeMut};
    use fulgurite_shell::Host;
    use fulgurite_widgets::prelude::*;

    /// The fields in a container of their own. A derived container
    /// skips at the top of its own `layout`, and a leaf is laid out
    /// whenever its parent is — so fields beside the hovered button, in
    /// one column, would be laid out again by the hover's mark and the
    /// case could not fail.
    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 4.0)]
    struct Fields {
        #[widget_id("field")]
        field: TextInput,
        #[widget_id("area")]
        area: TextArea,
    }

    #[derive(StaticTree, Default)]
    #[widget(row, spacing = 4.0)]
    struct Buttons {
        #[widget_id("hover")]
        hover: Button<Form>,
        #[widget_id("other")]
        other: Button<Form>,
    }

    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 4.0)]
    struct Ui {
        fields: Fields,
        buttons: Buttons,
    }

    #[derive(DeriveApp, Default)]
    struct Form {
        #[tree]
        ui: Ui,
    }

    /// `BlockMetrics`' advance and wrap, **with clusters**: a text area
    /// places its caret — and so its scroll — from the clusters, and the
    /// stand-in reports none, so under it no area ever scrolls and the
    /// area's half of this case could not fail.
    struct Blocks;

    impl fulgurite::TextMeasure for Blocks {
        fn measure(&mut self, text: &str, size: f32, max_w: f32) -> fulgurite::Size {
            fulgurite::BlockMetrics.measure(text, size, max_w)
        }

        fn shape(
            &mut self,
            text: &str,
            size: f32,
            weight: f32,
            max_w: f32,
            sink: &mut dyn fulgurite::cx::GlyphSink,
        ) -> fulgurite::Size {
            let adv = size * 0.5;
            let per_line = if max_w.is_finite() && max_w > 0.0 { (max_w / adv).floor().max(1.0) as usize } else { usize::MAX };
            for b in 0..=text.len() {
                let (line, col) = (b / per_line, b % per_line);
                sink.cluster(fulgurite::cx::Cluster { byte: b as u32, x: col as f32 * adv, line: line as u16, rtl: false });
            }
            self.measure_weighted(text, size, weight, max_w)
        }
    }

    /// A measurer unlike `Blocks`: a wider advance, so anything a probe
    /// measures differs from what the frame's layout measured.
    struct Wide;

    impl fulgurite::TextMeasure for Wide {
        fn measure(&mut self, text: &str, size: f32, max_w: f32) -> fulgurite::Size {
            fulgurite::BlockMetrics.measure(text, size * 1.5, max_w)
        }
    }

    const LINE: &str = "a line of text long enough to scroll the field sideways";
    const PARAGRAPH: &str = "a paragraph long enough to wrap onto several lines in a two hundred pixel window";

    /// The x of the field's run, and the y and text of the area's first
    /// painted line.
    ///
    /// **A line, not the value** (M45/T45.11): a text area paints one run
    /// per visible line, so its scroll shows as which line comes first and
    /// where. A probe that pinned the scroll to zero paints the
    /// paragraph's first line at the padding edge.
    fn text_at(list: &DisplayList, field: &str, area: &str) -> (Option<f32>, Option<(f32, String)>) {
        let mut out = (None, None);
        for c in &list.cmds {
            if let Cmd::Text { rect, text, .. } = c {
                let s = list.str_of(*text);
                if s == field {
                    out.0 = Some(rect[0]);
                } else if out.1.is_none() && !s.is_empty() && area.contains(s) {
                    out.1 = Some((rect[1], s.to_owned()));
                }
            }
        }
        out
    }

    fn centre<I: Invalidation>(h: &Host<Form, Incremental, I>, wid: &str) -> (f32, f32) {
        let i = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).unwrap();
        let s = h.states[i];
        (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
    }

    fn typed<I: Invalidation>(h: &mut Host<Form, Incremental, I>, wid: &str, text: &str) {
        let (x, y) = centre(h, wid);
        h.click(x, y);
        h.pump_with(&mut Blocks);
        for ch in text.chars() {
            // A newline is Enter, as a window delivers it.
            h.post(match ch {
                '\n' => Event::Key { key: fulgurite::Key::Enter, down: true },
                ch => Event::Text { ch },
            });
            h.pump_with(&mut Blocks);
        }
    }

    /// A session with a scrolled field and a scrolled area, focus parked
    /// on neither, so the next frames change nothing about them.
    fn session<I: Invalidation>() -> Host<Form, Incremental, I> {
        let mut h: Host<Form, Incremental, I> = Host::new(Form::default(), 200, 240);
        // A fixed height and a paragraph that wraps at the window's
        // width: the probe's 1e6 px width unwraps it to one line, and the
        // scroll clamp — computed from the wrapped height — then pins the
        // scroll to zero.
        h.set_stylesheet(&[Rule::new(
            Selector::Id("area"),
            Patch::new().width(Length::FILL).height(Length::Fixed(40.0)),
        )]);
        h.pump_with(&mut Blocks);
        typed(&mut h, "field", LINE);
        typed(&mut h, "area", PARAGRAPH);
        h.pump_with(&mut Blocks);
        h
    }

    /// Three arms, one session: `Everything` with the agent,
    /// `Boundaries<256>` with the agent, and `Boundaries<256>` without.
    #[test]
    fn the_audit_leaves_no_trace_under_boundaries() {
        let mut e: Host<Form, Incremental, Everything> = session();
        let mut b: Host<Form> = session();
        let mut plain: Host<Form> = session();
        let (mut ae, mut ab) = (Agent::new(), Agent::new());
        let (field, area) = (e.app.ui.fields.field.as_str().to_owned(), e.app.ui.fields.area.as_str().to_owned());
        let hover = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "hover").unwrap();
        let other = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "other").unwrap();
        let fs = e.states[fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "field").unwrap()];
        let edge = fs.pos.x + fs.pad.0;
        let as_ = e.states[fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "area").unwrap()];
        let top = as_.pos.y + as_.pad.1;
        let (ex, ey) = text_at(&e.list, &field, &area);
        println!("settled: Everything field x {ex:?} (pad edge {edge}), area y {ey:?} (pad edge {top})");
        assert!(ex.is_some_and(|x| x < edge), "Everything's field did not scroll, so this case cannot fail");
        assert!(
            ey.as_ref().is_some_and(|(y, s)| *y < top || !area.starts_with(s.as_str())),
            "Everything's area did not scroll, so its half cannot fail"
        );
        let mut differing = 0;
        for f in 0..3 {
            // The window's order: frame, then the agent, then the next
            // input, then the next frame.
            {
                let mut t = TreeMut::of_root(e.app.tree_mut(), &e.styles, &e.states);
                ae.frame_done(&mut t, FrameTiming::passes(1, 1));
            }
            {
                let mut t = TreeMut::of_root(b.app.tree_mut(), &b.styles, &b.states);
                ab.frame_done(&mut t, FrameTiming::passes(1, 1));
            }
            // An unrelated hover, alternating so each frame moves it.
            let (x, y) = centre(&e, if f % 2 == 0 { "hover" } else { "other" });
            let target = if f % 2 == 0 { hover } else { other };
            for h in [&mut e as &mut dyn Pumped, &mut b, &mut plain] {
                h.hover(x, y);
            }
            assert!(e.states[target].hovered && b.states[target].hovered, "setup: frame {f}'s hover did not land");
            let (xe, ye) = text_at(&e.list, &field, &area);
            let (xb, yb) = text_at(&b.list, &field, &area);
            let (xp, yp) = text_at(&plain.list, &field, &area);
            let (e_, b_, p_) = ((xe, ye.clone()), (xb, yb.clone()), (xp, yp.clone()));
            println!(
                "frame {f}: field x — Everything+agent {xe:?}, Boundaries+agent {xb:?}, Boundaries alone {xp:?} | \
                 area first line — {ye:?}, {yb:?}, {yp:?} | bound: all equal -> {}",
                e_ == b_ && e_ == p_
            );
            if e_ != b_ || e_ != p_ {
                differing += 1;
            }
        }
        assert_eq!(differing, 0, "{differing} of 3 frames painted the audit's probe instead of the fields' scroll");
    }

    /// Object-safe pumping for the three arms.
    trait Pumped {
        fn hover(&mut self, x: f32, y: f32);
    }

    impl<I: Invalidation> Pumped for Host<Form, Incremental, I> {
        fn hover(&mut self, x: f32, y: f32) {
            self.post(Event::PointerMove { x, y });
            let handled = self.pump_with(&mut Blocks);
            assert!(!handled, "setup: a hover must be unhandled, or it marks everything");
        }
    }

    // -----------------------------------------------------------------
    // M41/T41.4: the sweep. The named case covers two widgets; this is
    // what catches the *next* widget whose `layout` writes state that
    // paint reads, since `Node::probe`'s provided body is `layout`.
    // -----------------------------------------------------------------

    /// A leaf of most kinds the widget crate ships, each holding state
    /// a probe could disturb where it has any.
    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 2.0)]
    struct Kinds {
        label: Label<String>,
        button: Button<Sweep>,
        #[widget_id("field")]
        field: TextInput,
        #[widget_id("area")]
        area: TextArea,
        slider: Slider,
        check: CheckBox<Sweep>,
        switch: Switch<Sweep>,
        radio: Radio<Sweep>,
        badge: Badge,
        chip: Chip<&'static str, Sweep>,
        progress: ProgressBar,
        divider: Divider,
        spinner: Spinner,
        skeleton: Skeleton,
        avatar: Avatar,
        #[widget_id("number")]
        number: NumberInput,
        range: RangeSlider,
        search: SearchField,
        rich: RichText,
        markdown: Markdown,
        editor: RichTextEditor,
        // M41/T41.7: the leaves the first sweep left out. Its claim was
        // "every leaf kind the widget crate ships", and the live oracle
        // then found `Pagination` — not in this list — painting the
        // probe's measurements in the showcase.
        pages: Pagination,
        tree: Tree,
        tabs: Tabs,
        bottom: BottomNav,
        rail: NavigationRail,
        options: OptionList,
        colours: ColorPicker,
        days: DatePicker,
        picklist: PickList,
        appbar: AppBar<String>,
        paint: PaintBox,
        bars: BarChart,
    }

    #[derive(DeriveApp, Default)]
    struct Sweep {
        #[tree]
        ui: Kinds,
    }

    /// For every live leaf: paint it, probe it at the audit's two limits
    /// into scratch state, paint it again from the frame's state, and
    /// compare. A probe that writes anything paint reads moves the hash.
    ///
    /// **The probe measures with a different measurer than the layout
    /// did** (M41/T41.7). That is the window's case: it lays out with
    /// `FontMetrics`, and the agent's audit probes with the block
    /// stand-in unless the host installs its own. Probing with the same
    /// measurer hid every widget that caches a measured width, because
    /// the probe wrote back the value that was already there.
    #[test]
    fn a_probe_changes_nothing_any_leaf_paints() {
        let mut app = Sweep::default();
        // Content, so a widget that measures its items has something to
        // measure: a strip with no items and a pagination of one page
        // cannot show a probe writing their measured widths.
        app.ui.pages = Pagination::new(30);
        app.ui.tree = Tree::new(&[Branch::new("root", 0), Branch::new("leaf", 1)]);
        app.ui.tabs = Tabs::new(&["One", "Two", "Three"]);
        app.ui.bottom = BottomNav::new(&["Home", "Search"]);
        app.ui.rail = NavigationRail::new(&["Mail", "Chat"]);
        app.ui.rich = RichText::new(PARAGRAPH);
        app.ui.markdown = Markdown::new(PARAGRAPH);
        let mut h: Host<Sweep, Incremental, Everything> = Host::new(app, 200, 1400);
        h.set_stylesheet(&[Rule::new(
            Selector::Id("area"),
            Patch::new().width(Length::FILL).height(Length::Fixed(40.0)),
        )]);
        h.pump_with(&mut Blocks);
        // The two widgets whose probe the named case covers, scrolled —
        // a field at rest has no scroll for a probe to reset.
        let centre = |h: &Host<Sweep, Incremental, Everything>, wid: &str| {
            let i = fulgurite::style::index_of_wid(<Kinds as Widget>::SUB_TABLE, wid).unwrap();
            let s = h.states[i];
            (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
        };
        // The number field last, so it stays focused: a focused field is
        // what the showcase's drive leaves behind, and its caret is drawn
        // from what `layout` shaped (M41/T41.7).
        for (wid, text) in [("field", LINE), ("area", PARAGRAPH), ("number", "12345678901234567890")] {
            let (x, y) = centre(&h, wid);
            h.click(x, y);
            h.pump_with(&mut Blocks);
            for ch in text.chars() {
                h.post(Event::Text { ch });
                h.pump_with(&mut Blocks);
            }
        }
        h.pump_with(&mut Blocks);

        const A: fulgurite::Constraints = fulgurite::Constraints { min_w: 0.0, max_w: 1.0e6, min_h: 0.0, max_h: 1.0e6 };
        const B: fulgurite::Constraints = fulgurite::Constraints { min_w: 0.0, max_w: 2.0e6, min_h: 0.0, max_h: 2.0e6 };
        let table = <Kinds as Widget>::SUB_TABLE;
        let (mut swept, mut moved) = (0, Vec::new());
        for (i, d) in table.iter().enumerate() {
            if d.child_count != 0 {
                continue;
            }
            let paint = |h: &Host<Sweep, Incremental, Everything>| {
                let mut l = DisplayList::new();
                h.app.tree().node(i).paint(&h.states, i, &mut l);
                l.hash_from(0)
            };
            let before = paint(&h);
            let mut scratch = h.states.clone();
            let styles = h.styles.clone();
            let mut m = Wide;
            let mut cx = fulgurite::LayoutCx::new(&mut m);
            let node = h.app.tree_mut().node_mut(i);
            node.probe(&mut cx, A, &styles, &mut scratch, i);
            node.probe(&mut cx, B, &styles, &mut scratch, i);
            swept += 1;
            if paint(&h) != before {
                moved.push((i, h.app.tree().node(i).kind_name()));
            }
        }
        println!("swept {swept} leaves; paint moved under a probe: {moved:?} (bound: none)");
        assert!(swept >= 20, "setup: only {swept} leaves swept");
        assert!(moved.is_empty(), "a probe changed what these leaves paint: {moved:?}");
    }
}
