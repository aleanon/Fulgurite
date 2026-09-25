//! M40/T40.0, R1-F1 (S0): a partial redraw at a display scale other than
//! 1.0 is pixel-identical to a full one.
//!
//! `damage_oracle.rs` is the same comparison at 1.0 only, and 1.0 is the
//! one scale at which logical and physical pixels coincide — so it could
//! not see that retention observed the paint stream **before** `Scaled`
//! multiplied it. Every region in `Damage` was then logical, and both
//! surfaces applied it to a physical buffer: at 1.5 a hovered button's
//! damage said `[42,0,78,34]` while the pixels that changed were at
//! `[63,0,117,51]`, and the difference stayed on screen
//! (`docs/plan-physical-damage.md` §1.1).
//!
//! The default `Host` (`Incremental`, `Boundaries<256>`) is driven against
//! a `Rebuild` + `Everything` host at 1.0, 1.25, 1.5 and 2.0. Each step is
//! partially rendered from `take_damage_for_age(1)` onto a canvas that
//! persists across the session, and compared with a full render of the
//! oracle. **The 1.0 arm is the control**: it must print 0 whatever the
//! fix, or the gate measures something other than scale.
//!
//! Three steps per scale (a hover, a bound label, an `adopt`), because a
//! single step can pass by choosing one whose ink happens to stay inside
//! the logical rect. Each step asserts that it landed and that its damage
//! is a non-empty set of regions, so a no-op step cannot pass. The bound
//! label sits against the window's right edge, where `Damage::regions`
//! clamps against the physical viewport.
//!
//! Under `truetype` the canvases draw real glyphs from a pinned face, so
//! a glyph blit that stops short of a fractional region edge (R1-F8) is
//! visible here too at a device scale.

use lumen_static::invalidate::Everything;
use lumen_static::retain::Damage;
use lumen_static::style::{Edges, Length, Patch, Rule, Selector};
use lumen_static::{App as DeriveApp, Color, Event, Rebuild, StaticTree};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::Host;
use lumen_static_widgets::ops::{Button, Label, Spacer};

/// Logical size. Physical is this times the scale, rounded, the way a
/// window's inner size is.
const LW: f32 = 300.0;
const LH: f32 = 120.0;
const BG: Color = Color(18, 22, 32, 255);

#[derive(StaticTree, Default)]
#[widget(row, spacing = 8.0)]
struct Bar {
    #[widget_id("go")]
    go: Button<Tally>,
    gap: Spacer,
    #[widget_id("value")]
    value: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 6.0)]
struct Ui {
    #[widget_id("title")]
    title: Label<String>,
    bar: Bar,
}

#[derive(DeriveApp, Default)]
struct Tally {
    #[tree]
    ui: Ui,
    #[bind(ui.bar.value, "{}")]
    count: i64,
}

fn sheet() -> Vec<Rule> {
    vec![
        // No right padding: the value label ends on the window's edge.
        Rule::new(
            Selector::Kind("Ui"),
            Patch::new()
                .background(BG)
                .padding(Edges { left: 7.0, right: 0.0, top: 7.0, bottom: 7.0 }),
        ),
        Rule::new(
            Selector::Kind("Button"),
            Patch::new().background(Color(70, 100, 220, 255)).radius(6.0).padding(Edges::all(7.0)),
        ),
        Rule::new(Selector::Id("value"), Patch::new().font_size(21.0)),
        Rule::new(Selector::Kind("Bar"), Patch::new().width(Length::FILL)),
    ]
}

fn app(title: &str) -> Tally {
    let mut t = Tally::default();
    t.ui.title.set(title.to_owned());
    t.ui.bar.go.label = "go";
    t
}

fn index(wid: &str) -> usize {
    let table = <Ui as lumen_static::Widget>::SUB_TABLE;
    lumen_static::style::index_of_wid(table, wid).expect("id exists")
}

/// The pinned face, when the rasteriser can draw glyphs. `None` is the
/// block stand-in, measured by `BlockMetrics`.
#[cfg(feature = "truetype")]
type Face = Option<lumen_static_shell::text::FontStack>;
#[cfg(not(feature = "truetype"))]
type Face = Option<()>;

fn pinned_face() -> Face {
    #[cfg(feature = "truetype")]
    {
        let regular = std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf").ok();
        let bold = std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf").ok();
        let f = regular
            .and_then(|r| lumen_static_shell::text::FontStack::from_bytes(&r, bold.as_deref()));
        if f.is_none() {
            eprintln!("pinned face missing; drawing block glyphs");
        }
        f
    }
    #[cfg(not(feature = "truetype"))]
    None
}

/// A canvas that draws what the window would: real glyphs when there is
/// a face, the block stand-in otherwise.
fn canvas(face: &Face, w: u32, h: u32) -> Canvas {
    #[cfg(feature = "truetype")]
    if let Some(f) = face {
        return Canvas::with_font(w, h, f.clone());
    }
    let _ = face;
    Canvas::new(w, h)
}

/// The measurer a window would lay out with, so a glyph's ink sits in
/// the box its run was given — a block-measured box under real glyphs
/// puts ink outside every region and fails at 1.0 for a reason that has
/// nothing to do with scale.
fn measurer(face: &Face, scale: f32) -> Box<dyn lumen_static::TextMeasure> {
    #[cfg(feature = "truetype")]
    if let Some(f) = face {
        return Box::new(lumen_static_shell::text::FontMetrics::with_scale(f.clone(), scale));
    }
    let _ = (face, scale);
    Box::new(lumen_static::BlockMetrics)
}

/// How many pixels differ, and the box around them.
fn diff(a: &Canvas, b: &Canvas) -> (usize, Option<[u32; 4]>) {
    let (mut n, mut bb) = (0, None::<[u32; 4]>);
    for y in 0..a.height {
        for x in 0..a.width {
            let i = ((y * a.width + x) * 4) as usize;
            if a.pixels[i..i + 4] != b.pixels[i..i + 4] {
                n += 1;
                bb = Some(match bb {
                    None => [x, y, x + 1, y + 1],
                    Some(r) => [r[0].min(x), r[1].min(y), r[2].max(x + 1), r[3].max(y + 1)],
                });
            }
        }
    }
    (n, bb)
}

type Oracle = Host<Tally, Rebuild, Everything>;

struct Session {
    scale: f32,
    face: Face,
    w: u32,
    h: u32,
    host: Host<Tally>,
    oracle: Oracle,
    partial: Canvas,
    full: Canvas,
}

impl Session {
    fn new(scale: f32, face: Face) -> Self {
        let (w, h) = ((LW * scale).round() as u32, (LH * scale).round() as u32);
        let mut host: Host<Tally> = Host::new(app("tally"), w, h);
        let mut oracle: Oracle = Host::new(app("tally"), w, h);
        for hh in [&mut host as &mut dyn Scalable, &mut oracle] {
            hh.setup(scale, &mut *measurer(&face, scale));
        }
        let (mut partial, mut full) = (canvas(&face, w, h), canvas(&face, w, h));
        partial.clear(BG);
        host.render(&mut partial);
        full.clear(BG);
        oracle.render(&mut full);
        host.take_damage_for_age(1);
        Session { scale, face, w, h, host, oracle, partial, full }
    }

    /// Pump both, render the oracle fully and the host from its damage,
    /// and report. Returns the differing pixel count.
    fn step(&mut self, what: &str, node: usize) -> usize {
        let mut m = measurer(&self.face, self.scale);
        self.host.pump_with(&mut *m);
        self.oracle.pump_with(&mut *m);
        self.full.clear(BG);
        self.oracle.render(&mut self.full);

        let d = self.host.take_damage_for_age(1);
        let viewport = [0.0, 0.0, self.w as f32, self.h as f32];
        let regions: Vec<[f32; 4]> = d.regions(viewport).collect();
        assert!(
            matches!(d, Damage::Regions(_)) && !regions.is_empty(),
            "{what} @ {}: the step must damage a set of regions to test anything, got {d:?}",
            self.scale
        );
        for r in &regions {
            self.partial.clear_within(*r, BG);
            self.partial.render_within(&self.host.list, *r);
        }

        let st = &self.host.states[node];
        let k = self.scale;
        let phys = [st.pos.x * k, st.pos.y * k, (st.pos.x + st.size.w) * k, (st.pos.y + st.size.h) * k];
        let (n, bb) = diff(&self.full, &self.partial);
        println!(
            "scale {k:<4} {what:<6} regions {regions:?}; node physical {phys:?}; \
             px differing {n} / bound 0; bbox {bb:?}"
        );
        n
    }
}

/// The setup both hosts share, so the two cannot drift apart.
trait Scalable {
    fn setup(&mut self, scale: f32, m: &mut dyn lumen_static::TextMeasure);
}

impl<R: lumen_static::Retention, I: lumen_static::invalidate::Invalidation> Scalable
    for Host<Tally, R, I>
{
    fn setup(&mut self, scale: f32, m: &mut dyn lumen_static::TextMeasure) {
        self.set_scale(scale);
        self.set_stylesheet(&sheet());
        self.pump_with(m);
        // A still frame, so the first step starts from an empty set.
        self.pump_with(m);
    }
}

/// One session at `scale`: returns the differing pixels per step.
fn drive(scale: f32, face: Face) -> [usize; 3] {
    let mut s = Session::new(scale, face);
    let (go, value, title) = (index("go"), index("value"), index("title"));

    // 1. A hover: the button's shade, which no handler writes.
    let st = s.host.states[go];
    let (x, y) = (st.pos.x + st.size.w / 2.0, st.pos.y + st.size.h / 2.0);
    s.host.post(Event::PointerMove { x, y });
    s.oracle.post(Event::PointerMove { x, y });
    let hover = s.step("hover", go);
    assert!(s.host.states[go].hovered, "setup @ {scale}: the move did not land on the button");

    // 2. A bound label against the right edge.
    let before = s.host.app.ui.bar.value.text.get().clone();
    s.host.app.count = 7_777;
    s.oracle.app.count = 7_777;
    let label = s.step("label", value);
    let after = s.host.app.ui.bar.value.text.get().clone();
    assert_ne!(before, after, "setup @ {scale}: the bound label did not change");
    let right = s.host.states[value].pos.x + s.host.states[value].size.w;
    assert!((right - LW).abs() < 0.01, "setup @ {scale}: the label ends at {right}, not the edge {LW}");

    // 3. `adopt`: a new application, with a different title.
    s.host.adopt(app("adopted"));
    s.oracle.adopt(app("adopted"));
    let adopt = s.step("adopt", title);
    assert_eq!(s.host.app.ui.title.text.get(), "adopted", "setup @ {scale}: adopt did not land");

    [hover, label, adopt]
}

/// Every device scale, and what each step left stale.
fn device_scales(face: impl Fn() -> Face) -> Vec<(f32, [usize; 3])> {
    let mut bad = Vec::new();
    for scale in [1.25, 1.5, 2.0] {
        let px = drive(scale, face());
        if px != [0, 0, 0] {
            bad.push((scale, px));
        }
    }
    bad
}

#[test]
fn block_glyphs_a_partial_redraw_at_scale_one_matches_a_full_one() {
    // The control. Logical and physical coincide, so this is green with
    // and without the fix; if it ever differs, the gate is not measuring
    // scale.
    assert_eq!(drive(1.0, None), [0, 0, 0], "hover, label, adopt at 1.0");
}

#[test]
#[ignore = "red until T40.1: R1-F1, retention observes logical rects"]
fn block_glyphs_a_partial_redraw_at_device_scales_matches_a_full_one() {
    let bad = device_scales(|| None);
    assert!(bad.is_empty(), "stale pixels (scale, [hover, label, adopt]): {bad:?}");
}

/// The same session with real glyphs from the pinned face, laid out by
/// the face's own metrics. A glyph blit that truncates a fractional far
/// edge (R1-F8) leaves one row here once the regions are physical.
#[cfg(feature = "truetype")]
#[test]
fn real_glyphs_a_partial_redraw_at_scale_one_matches_a_full_one() {
    assert_eq!(drive(1.0, pinned_face()), [0, 0, 0], "hover, label, adopt at 1.0");
}

#[cfg(feature = "truetype")]
#[test]
#[ignore = "red until T40.2: R1-F1 and R1-F8, the glyph blit truncates a fractional far edge"]
fn real_glyphs_a_partial_redraw_at_device_scales_matches_a_full_one() {
    let bad = device_scales(pinned_face);
    assert!(bad.is_empty(), "stale pixels (scale, [hover, label, adopt]): {bad:?}");
}
