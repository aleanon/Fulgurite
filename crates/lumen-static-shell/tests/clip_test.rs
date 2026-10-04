//! The scoped containment invariant, proven at the pixel level: a
//! scrolling list must not draw where a click would not land.
//!
//! `crates/lumen-static/tests/scroll_containment.rs` pins the geometry
//! and the clip command; this pins what actually reaches the screen,
//! which is the claim that matters and the one only a renderer can
//! settle.
//!
//! Since M50/T50.8 (R8-F8) it pins the clip stack for **every** drawing
//! command, not the three `ink_of` could build: each is rendered inside
//! a `PushClip` and must ink nothing outside it, and a shadow inside a
//! `ScrollView` must stay in the viewport on a full redraw, which no
//! partial-versus-full oracle can see.

use lumen_static_widgets::ops::{Fixed, Spacer};
use lumen_static::repeat::Repeat;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::CLEAR;

/// Every row paints a solid 24px black bar the full width of the list.
fn ink_row(row: &mut Row, _: usize) {
    row.ink.w = W as f32;
    row.ink.h = 24.0;
}

/// A row that paints solid black across its whole width.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Row {
    ink: Fixed,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    /// Empty space above the list, so an escaping row has somewhere
    /// visibly wrong to land.
    #[widget_id("head")]
    head: Spacer,
    #[widget_id("list")]
    list: Repeat<Row>,
}

const W: u32 = 60;
const H: u32 = 160;

/// Render a list scrolled half an item, and report the inked fraction of
/// the band *above* the list — where a materialized row is placed but a
/// click is rejected.
fn ink_above_the_list() -> (f32, f32) {
    let mut ui = Ui { head: Spacer, list: Repeat::new(1000, 24.0, ink_row) };
    ui.list.scroll = 12.0;

    let mut styles = pass::default_styles(&ui);
    // A fixed 40px gap, then the list takes the rest.
    let sheet = [
        Rule::new(Selector::Id("head"), Patch::new().height(Length::Fixed(40.0))),
        Rule::new(Selector::Id("list"), Patch::new().height(Length::FILL)),
    ];
    lumen_static::style::cascade(
        <Ui as Widget>::SUB_TABLE,
        <Ui as Widget>::SEEDS,
        &sheet,
        &mut styles,
    );

    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    let view = Constraints { min_w: 0.0, max_w: W as f32, min_h: 0.0, max_h: H as f32 };
    pass::layout(&mut ui, &styles, &mut st, view);

    let list_top = st[2].pos.y;
    assert!(list_top >= 40.0, "the gap is above the list (got {list_top})");
    // The first row is placed 12px above the list top by `place`.
    assert!(ui.list.row_state(0).pos.y < list_top, "row 0 does overrun upward");

    let mut out = lumen_static::DisplayList::new();
    pass::paint(&ui, &st, &mut out);
    let mut c = Canvas::new(W, H);
    c.clear(CLEAR);
    <Canvas as lumen_static::Renderer>::render(&mut c, &out, W, H);

    // Inked fraction of the band above the list, and of the list itself.
    let band = |y0: u32, y1: u32| {
        let mut inked = 0usize;
        let mut total = 0usize;
        for y in y0..y1 {
            for x in 0..W {
                let p = ((y * W + x) * 4) as usize;
                total += 1;
                if c.pixels[p] != CLEAR.0 || c.pixels[p + 1] != CLEAR.1 {
                    inked += 1;
                }
            }
        }
        inked as f32 / total as f32
    };
    (band(0, list_top as u32), band(list_top as u32, H))
}

#[test]
fn a_scrolled_row_does_not_paint_where_a_click_would_miss() {
    let (above, inside) = ink_above_the_list();
    assert_eq!(above, 0.0, "nothing is drawn above the list — the clip holds");
    assert!(inside > 0.5, "and the list itself still renders ({inside:.2} inked)");
}

/// Render one command list to a fresh `w`×`h` canvas.
///
/// Every command kind has a case (M50/T50.8). Until then this sent six
/// of the nine drawing commands to `unreachable!()`, so the pixel pin
/// `clipped`'s rustdoc cited covered `Rect`, `RoundedRect` and
/// `StrokedRect` only. The three that have builders with guards worth
/// keeping still go through them; the rest are pushed verbatim, and a
/// `Text` command's range is backed by `H`s so the block-glyph path has
/// something to draw.
fn render_cmds(cmds: &[lumen_static::Cmd], w: u32, h: u32) -> Canvas {
    use lumen_static::Cmd;
    let mut list = lumen_static::DisplayList::new();
    for c in cmds {
        match *c {
            Cmd::Rect { rect, color } => list.rect(rect, color),
            Cmd::PushClip { rect } => list.push_clip(rect),
            Cmd::PopClip => list.pop_clip(),
            Cmd::RoundedRect { rect, color, radius } => list.rounded_rect(rect, color, radius),
            Cmd::StrokedRect { rect, color, radius, width } => {
                list.stroked_rect(rect, color, radius, width)
            }
            Cmd::Text { text: (start, len), .. } => {
                let end = (start + len) as usize;
                while list.text.len() < end {
                    list.text.push('H');
                }
                list.cmds.push(*c);
            }
            Cmd::Shadow { .. }
            | Cmd::Image { .. }
            | Cmd::Line { .. }
            | Cmd::Gradient { .. }
            | Cmd::Glass { .. } => list.cmds.push(*c),
        }
    }
    let mut c = Canvas::new(w, h);
    c.images = Some(Box::new(Checker));
    c.clear(CLEAR);
    <Canvas as lumen_static::Renderer>::render(&mut c, &list, w, h);
    c
}

/// Render one command list to a fresh 64×64 canvas and return the inked
/// fraction.
fn ink_of(cmds: &[lumen_static::Cmd]) -> f32 {
    render_cmds(cmds, 64, 64).ink_fraction(CLEAR)
}

/// A 4×4 opaque image, so `Cmd::Image` has pixels to blit.
struct Checker;
static CHECKER: [u8; 64] = {
    let mut px = [0u8; 64];
    let mut i = 0;
    while i < 16 {
        px[i * 4] = if i % 2 == 0 { 200 } else { 20 };
        px[i * 4 + 1] = 40;
        px[i * 4 + 2] = 90;
        px[i * 4 + 3] = 255;
        i += 1;
    }
    px
};
impl lumen_static::ImageSource for Checker {
    fn image(&self, _: lumen_static::display::ImageId) -> Option<(u32, u32, &[u8])> {
        Some((4, 4, &CHECKER))
    }
}

/// The per-command pin's canvas and clip — `docs/plan-raster-allocation.md`
/// T50.8.
const PW: u32 = 200;
const PH: u32 = 120;
const CLIP: [f32; 4] = [40.0, 40.0, 120.0, 80.0];

/// Pixels where `c` differs from `base`, outside `clip` and inside it.
///
/// Against a reference render rather than against `CLEAR`, because glass
/// draws nothing over a flat backdrop: a blur of one colour is that
/// colour. Its rows carry a striped backdrop, and ink is what the
/// command changed.
fn ink_split(c: &Canvas, base: &Canvas, clip: [f32; 4]) -> (usize, usize) {
    let (mut outside, mut inside) = (0, 0);
    for y in 0..c.height {
        for x in 0..c.width {
            let p = ((y * c.width + x) * 4) as usize;
            if c.pixels[p..p + 3] == base.pixels[p..p + 3] {
                continue;
            }
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            if fx >= clip[0] && fx < clip[2] && fy >= clip[1] && fy < clip[3] {
                inside += 1;
            } else {
                outside += 1;
            }
        }
    }
    (outside, inside)
}

/// Vertical stripes over the whole canvas, drawn *before* any clip: the
/// backdrop a glass panel needs to show that it did anything.
fn stripes() -> Vec<lumen_static::Cmd> {
    use lumen_static::display::Color;
    (0..PW / 10)
        .map(|k| {
            let x = (k * 10) as f32;
            let color = if k % 2 == 0 { Color(20, 20, 20, 255) } else { Color(230, 230, 230, 255) };
            lumen_static::Cmd::Rect { rect: [x, 0.0, x + 10.0, PH as f32], color }
        })
        .collect()
}

/// One of every drawing command, each reaching well past `CLIP` on all
/// sides, with the area of the box it would ink unclipped.
fn every_command() -> Vec<(&'static str, lumen_static::Cmd, f32, bool)> {
    use lumen_static::display::{Color, ImageId, Material, Stops};
    use lumen_static::{Cmd, Point};
    let area = |r: [f32; 4]| (r[2] - r[0]) * (r[3] - r[1]);
    let big = [10.0, 10.0, 190.0, 110.0];
    let ink = Color(200, 30, 30, 255);
    vec![
        ("Rect", Cmd::Rect { rect: big, color: ink }, area(big), false),
        ("RoundedRect", Cmd::RoundedRect { rect: big, color: ink, radius: 12.0 }, area(big), false),
        (
            "StrokedRect",
            Cmd::StrokedRect { rect: big, color: ink, radius: 6.0, width: 45.0 },
            area(big),
            false,
        ),
        (
            // The shadow's reach is its rect grown by `spread`. A shadow
            // inks only *around* its rect, so the rect sits inside the
            // clip and the ring crosses it on every side.
            "Shadow",
            Cmd::Shadow { rect: [55.0, 50.0, 105.0, 70.0], color: Color(0, 0, 0, 80), radius: 8.0, spread: 30.0 },
            area([25.0, 20.0, 135.0, 100.0]),
            false,
        ),
        (
            "Gradient",
            Cmd::Gradient { rect: big, stops: Stops::two(ink, Color(30, 30, 200, 255)), angle: 0.0 },
            area(big),
            false,
        ),
        (
            "Line",
            Cmd::Line { a: Point { x: 10.0, y: 10.0 }, b: Point { x: 190.0, y: 110.0 }, width: 8.0, color: ink },
            area([6.0, 6.0, 194.0, 114.0]),
            false,
        ),
        (
            // Block glyphs: `Canvas::new` has no font. 30 `H`s at 16 px.
            "Text",
            Cmd::Text { rect: [10.0, 50.0, 190.0, 70.0], text: (0, 30), size: 16.0, weight: 400.0, line_height: 0, color: ink },
            area([10.0, 50.0, 190.0, 70.0]),
            false,
        ),
        ("Image", Cmd::Image { rect: big, id: ImageId(1), tint: Color(255, 255, 255, 255) }, area(big), false),
        ("Glass FROSTED", Cmd::Glass { rect: big, radius: 12.0, material: Material::FROSTED }, area(big), true),
        ("Glass LIQUID", Cmd::Glass { rect: big, radius: 12.0, material: Material::LIQUID }, area(big), true),
    ]
}

/// **The clip stack is pinned for every command, not three — M50/T50.8
/// (R8-F8).**
///
/// Each drawing command, reaching past `CLIP` on every side, is rendered
/// inside `PushClip(CLIP)`: nothing may be inked outside it. Three
/// setup checks come first, so a command that draws nothing cannot pass
/// by drawing nothing (`AGENT.md`'s first vacuous shape): the same
/// command **without** the clip inks outside the rect, and with the clip
/// it still inks inside. The count is printed beside the command's
/// unclipped reach, so a zero reads as a zero out of something.
///
/// Injection, recorded in the commit: scratch commit c472f80 (the shadow arm
/// draws over its unclipped reach) turns the `Shadow` row red by name.
#[test]
fn every_command_stays_inside_a_push_clip() {
    use lumen_static::Cmd;
    let mut escaped = Vec::new();
    for (name, cmd, reach, backdrop) in every_command() {
        let under = if backdrop { stripes() } else { Vec::new() };
        let base = render_cmds(&under, PW, PH);
        let bare = render_cmds(&[&under[..], &[cmd]].concat(), PW, PH);
        let (bare_out, _) = ink_split(&bare, &base, CLIP);
        let scoped = [&under[..], &[Cmd::PushClip { rect: CLIP }, cmd, Cmd::PopClip]].concat();
        let (out, inside) = ink_split(&render_cmds(&scoped, PW, PH), &base, CLIP);
        println!(
            "{name:>13}: {out} px inked outside the clip (bound 0; unclipped reach {reach:.0} px², \
             unclipped ink outside {bare_out} px, ink inside {inside} px)"
        );
        assert!(bare_out > 0, "{name}: setup — unclipped, it must ink outside the clip (got 0)");
        assert!(inside > 0, "{name}: setup — clipped, it must still ink inside the clip (got 0)");
        if out != 0 {
            escaped.push((name, out));
        }
    }
    assert!(escaped.is_empty(), "commands inked outside a PushClip: {escaped:?}");
}

/// **A shadow inside a `ScrollView`'s clip, on a full redraw** — the case
/// no oracle can see (R8-F8).
///
/// `tests/damage_oracle.rs` compares a partial render with a full one; a shadow
/// that escapes a scroll viewport escapes identically in both, so the
/// two agree. Only a pixel count against the viewport's own box can
/// tell. A card with a 14 px shadow is scrolled so it straddles the
/// viewport's top edge; the whole frame is rendered, and nothing may be
/// inked above or below the viewport.
///
/// Setup, asserted first: the same display list with its clips stripped
/// *does* ink outside the viewport, so the scene puts shadow where the
/// clip must stop it.
#[test]
fn a_shadow_inside_a_scroll_view_stays_in_its_viewport_on_a_full_redraw() {
    use lumen_static::style::{Length, Patch, Rule, Selector};
    use lumen_static::Cmd;
    use lumen_static_widgets::ScrollView;

    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct Shaded {
        body: Fixed,
    }

    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 30.0)]
    struct Cards {
        #[widget_id("card-a")]
        a: Shaded,
        #[widget_id("card-b")]
        b: Shaded,
        #[widget_id("card-c")]
        c: Shaded,
    }

    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct View {
        #[widget_id("top")]
        top: Spacer,
        #[widget_id("view")]
        view: ScrollView<Cards>,
        #[widget_id("bottom")]
        bottom: Spacer,
    }

    let mut ui = View::default();
    for card in [&mut ui.view.content.a, &mut ui.view.content.b, &mut ui.view.content.c] {
        card.body = Fixed { w: 120.0, h: 40.0 };
    }
    // Scroll so card a straddles the viewport's top edge.
    ui.view.scroll = 20.0;

    let mut styles = pass::default_styles(&ui);
    let shaded = Patch::new().shadow(14.0).background(lumen_static::display::Color(240, 240, 240, 255));
    let sheet = [
        Rule::new(Selector::Id("top"), Patch::new().height(Length::Fixed(30.0))),
        Rule::new(Selector::Id("view"), Patch::new().height(Length::Fixed(60.0))),
        Rule::new(Selector::Id("bottom"), Patch::new().height(Length::FILL)),
        Rule::new(Selector::Id("card-a"), shaded),
        Rule::new(Selector::Id("card-b"), shaded),
        Rule::new(Selector::Id("card-c"), shaded),
    ];
    lumen_static::style::cascade(<View as Widget>::SUB_TABLE, <View as Widget>::SEEDS, &sheet, &mut styles);

    let mut st = [NodeState::default(); View::NODE_COUNT];
    let view = Constraints { min_w: 0.0, max_w: PW as f32, min_h: 0.0, max_h: PH as f32 };
    pass::layout(&mut ui, &styles, &mut st, view);
    let mut out = lumen_static::DisplayList::new();
    pass::paint(&ui, &st, &mut out);

    // The viewport is the ScrollView's own clip: the first PushClip.
    let viewport = out
        .cmds
        .iter()
        .find_map(|c| if let Cmd::PushClip { rect } = c { Some(*rect) } else { None })
        .expect("the ScrollView pushes its viewport as a clip");
    let shadows = out.cmds.iter().filter(|c| matches!(c, Cmd::Shadow { .. })).count();
    assert!(shadows >= 1, "setup: the cards paint a shadow (got {shadows} Shadow commands)");

    let full = |list: &lumen_static::DisplayList| {
        let mut c = Canvas::new(PW, PH);
        c.clear(CLEAR);
        <Canvas as lumen_static::Renderer>::render(&mut c, list, PW, PH);
        c
    };
    let mut unclipped = lumen_static::DisplayList::new();
    unclipped.text = out.text.clone();
    unclipped.cmds =
        out.cmds.iter().copied().filter(|c| !matches!(c, Cmd::PushClip { .. } | Cmd::PopClip)).collect();
    let base = full(&lumen_static::DisplayList::new());
    let (bare_out, _) = ink_split(&full(&unclipped), &base, viewport);
    let (escaped, inside) = ink_split(&full(&out), &base, viewport);
    println!(
        "viewport {viewport:?}: {escaped} px inked outside it on a full redraw (bound 0; \
         {bare_out} px with the clips stripped, {inside} px inside)"
    );
    assert!(bare_out > 0, "setup: without its clip the shadow must reach outside the viewport (got 0)");
    assert!(inside > 0, "setup: the viewport itself renders (got 0 px)");
    assert_eq!(escaped, 0, "a shadow inside a ScrollView inked {escaped} px outside its viewport");
}

#[test]
fn a_nan_rect_draws_nothing_inside_or_outside_a_clip() {
    use lumen_static::display::Color;
    use lumen_static::Cmd;
    let nan = [f32::NAN, 0.0, 10.0, 10.0];
    assert_eq!(ink_of(&[Cmd::Rect { rect: nan, color: Color::BLACK }]), 0.0, "outside a clip");
    assert_eq!(
        ink_of(&[
            Cmd::PushClip { rect: [0.0, 0.0, 32.0, 32.0] },
            Cmd::Rect { rect: nan, color: Color::BLACK },
            Cmd::PopClip,
        ]),
        0.0,
        "inside a clip — previously filled the whole clip solid"
    );
}

#[test]
fn a_huge_rect_terminates_and_covers_the_surface() {
    use lumen_static::display::Color;
    use lumen_static::Cmd;
    // Previously ~10^16 loop iterations. If this test finishes, it passed.
    let huge = [-1e8, -1e8, 1e8, 1e8];
    assert_eq!(ink_of(&[Cmd::Rect { rect: huge, color: Color::BLACK }]), 1.0);
}

#[test]
fn an_inverted_rect_draws_nothing() {
    use lumen_static::display::Color;
    use lumen_static::Cmd;
    let inverted = [30.0, 30.0, 10.0, 10.0];
    assert_eq!(ink_of(&[Cmd::Rect { rect: inverted, color: Color::BLACK }]), 0.0);
    assert_eq!(
        ink_of(&[
            Cmd::PushClip { rect: [0.0, 0.0, 64.0, 64.0] },
            Cmd::Rect { rect: inverted, color: Color::BLACK },
            Cmd::PopClip,
        ]),
        0.0
    );
}
