//! Bar, line and pie — three presentations of one data type.
//!
//! # The data is a fixed array, and that is the architecture
//!
//! [`Series`] holds [`MAX_POINTS`] values inline and a length, so a
//! chart allocates nothing and is `Copy`. That is the same call
//! `Stops` makes for gradients and `Regions` for damage, for the same
//! reason: this crate is `no_std`-clean and a chart on a microcontroller
//! is a chart whose points live in the widget.
//!
//! A series longer than the cap is **truncated, not refused**. A chart
//! is a read-only view of someone else's numbers and dropping a frame
//! over the thirty-third one would be a worse answer than showing
//! thirty-two.
//!
//! # One type, three shapes
//!
//! `Chart<KIND>` with a const parameter, exactly as `Toggle<SHAPE>` does
//! it — and for the identical reason. `kind_name` feeds the cascade and
//! the agent's selectors, and `defaults` is snapshotted once when a
//! stylesheet is set, so a shape held in a *field* would make both
//! state-dependent.

use lumen_static::desc::{NodeDesc, NodeState, NO_PARENT};
use lumen_static::display::{Color, Painter};
use lumen_static::ops::{record_painted, rect_of, resolve_self};
use lumen_static::style::{Length, Seed, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{leaf_common, Constraints, LayoutCx, Point, Size};

const LEAF: &[NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];

/// How many points a [`Series`] carries. Thirty-two is a screenful of
/// bars at a readable width and keeps `Series` at 132 bytes.
pub const MAX_POINTS: usize = 32;

/// A fixed-capacity run of values.
#[derive(Clone, Copy)]
pub struct Series {
    v: [f32; MAX_POINTS],
    len: u8,
}

impl Default for Series {
    fn default() -> Self {
        Series { v: [0.0; MAX_POINTS], len: 0 }
    }
}

impl Series {
    /// Take up to [`MAX_POINTS`] values. Longer input is truncated —
    /// see the module note on why that is not a refusal.
    pub fn new(values: &[f32]) -> Self {
        let mut s = Series::default();
        for (dst, src) in s.v.iter_mut().zip(values.iter().take(MAX_POINTS)) {
            *dst = *src;
        }
        s.len = values.len().min(MAX_POINTS) as u8;
        s
    }

    pub fn as_slice(&self) -> &[f32] {
        &self.v[..self.len as usize]
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The largest value, or `0` for an empty series.
    ///
    /// **Non-finite values are skipped rather than propagated.** A NaN
    /// in the input would make every later comparison false and the
    /// whole chart scale to nothing; a chart is a view of numbers it did
    /// not produce, and one bad sample should cost one bar.
    pub fn max(&self) -> f32 {
        let mut m = 0.0f32;
        for v in self.as_slice() {
            if v.is_finite() && *v > m {
                m = *v;
            }
        }
        m
    }

    /// The total, skipping non-finite values — what a pie divides by.
    pub fn sum(&self) -> f32 {
        let mut t = 0.0f32;
        for v in self.as_slice() {
            if v.is_finite() && *v > 0.0 {
                t += *v;
            }
        }
        t
    }
}

/// Vertical bars from a baseline of zero.
pub const BAR: u8 = 0;
/// A polyline through the points.
pub const LINE: u8 = 1;
/// Proportions of a whole, as a ring.
pub const PIE: u8 = 2;

/// A chart. See the module docs for why `KIND` is a const.
pub struct Chart<const KIND: u8 = BAR> {
    /// The values. A presentation copy — bind it.
    pub data: Series,
    /// The colours a pie's wedges take in turn — M25/T25.10. The built-in
    /// six by default; an empty slice falls back to them rather than
    /// painting nothing.
    pub series: &'static [Color],
}

pub type BarChart = Chart<BAR>;
pub type LineChart = Chart<LINE>;
pub type PieChart = Chart<PIE>;

impl<const KIND: u8> Default for Chart<KIND> {
    fn default() -> Self {
        Chart { data: Series::default(), series: &WEDGES }
    }
}

impl<const KIND: u8> Chart<KIND> {
    pub fn new(values: &[f32]) -> Self {
        Chart { data: Series::new(values), series: &WEDGES }
    }
    pub fn set(&mut self, values: &[f32]) {
        self.data = Series::new(values);
    }
}

impl<const KIND: u8> Widget for Chart<KIND> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    /// Its box is its style's; the data changes what is *drawn* in it,
    /// never how big it is.
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = LEAF;

    const KIND: &'static str = match KIND {
        LINE => "linechart",
        PIE => "piechart",
        _ => "barchart",
    };

    fn type_defaults() -> Style {
        Style {
            width: Length::Fixed(CHART.w),
            height: Length::Fixed(CHART.h),
            background: Color(0, 0, 0, 0),
            color: Color(0x2f, 0x6b, 0xff, 0xff),
            ..Style::default()
        }
    }

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, match KIND {
            PIE => lumen_static::style::Fields::of(&[
                lumen_static::style::Fields::WIDTH,
                lumen_static::style::Fields::HEIGHT,
                lumen_static::style::Fields::VISIBLE,
                lumen_static::style::Fields::BACKGROUND,
                lumen_static::style::Fields::BORDER,
                lumen_static::style::Fields::RADIUS,
            ]),
            _ => lumen_static::style::Fields::of(&[
                lumen_static::style::Fields::WIDTH,
                lumen_static::style::Fields::HEIGHT,
                lumen_static::style::Fields::VISIBLE,
                lumen_static::style::Fields::COLOR,
                lumen_static::style::Fields::BACKGROUND,
                lumen_static::style::Fields::BORDER,
                lumen_static::style::Fields::RADIUS,
            ]),
        })];
}

const CHART: Size = Size { w: 280.0, h: 160.0 };

/// The series palette a chart has unless it sets [`Chart::series`], used
/// when a chart has more than one colour to give — which is the pie, and
/// the pie only.
const WEDGES: [Color; 6] = [
    Color(0x2f, 0x6b, 0xff, 0xff),
    Color(0x21, 0xc0, 0xa8, 0xff),
    Color(0xff, 0x9f, 0x43, 0xff),
    Color(0xee, 0x5a, 0x6f, 0xff),
    Color(0x9b, 0x7a, 0xf0, 0xff),
    Color(0x48, 0xb0, 0xf7, 0xff),
];

impl<const KIND: u8> Node for Chart<KIND> {
    fn layout(
        &mut self,
        _cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = style[i];
        let size = resolve_self(limits, &me, CHART);
        record_painted(st, i, limits, size, &me)
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = rect_of(s);
        let (w, h) = (r[2] - r[0], r[3] - r[1]);
        if w <= 0.0 || h <= 0.0 || self.data.is_empty() {
            return;
        }
        if s.bg.3 > 0 {
            out.rounded_rect(r, s.bg, s.radius);
            lumen_static::ops::border(st, i, out);
        }
        let n = self.data.len();
        let vals = self.data.as_slice();
        match KIND {
            LINE => {
                let max = self.data.max();
                if max <= 0.0 {
                    return;
                }
                let step = if n > 1 { w / (n - 1) as f32 } else { 0.0 };
                let y_of = |v: f32| r[3] - (v.max(0.0) / max) * h;
                let stroke = (h * 0.018).max(1.5);
                for k in 1..n {
                    let (a, b) = (vals[k - 1], vals[k]);
                    if !a.is_finite() || !b.is_finite() {
                        continue;
                    }
                    out.line(
                        Point { x: r[0] + step * (k - 1) as f32, y: y_of(a) },
                        Point { x: r[0] + step * k as f32, y: y_of(b) },
                        stroke,
                        s.fg,
                    );
                }
            }
            PIE => {
                // **Wedges as rings of segments**, because the renderer
                // has no arc and no polygon: each wedge is a run of thin
                // radial lines. At 96 segments over a whole turn the
                // joins are under a pixel at any size this widget is
                // used at, and the cost is bounded by a const rather
                // than by the data.
                let total = self.data.sum();
                if total <= 0.0 {
                    return;
                }
                let d = w.min(h);
                let (cx, cy) = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
                let outer = d * 0.46;
                let inner = d * 0.24;
                const STEPS: usize = 96;
                let mut at = 0.0f32;
                for (k, v) in vals.iter().enumerate() {
                    if !v.is_finite() || *v <= 0.0 {
                        continue;
                    }
                    let frac = v / total;
                    let series = if self.series.is_empty() { &WEDGES[..] } else { self.series };
                    let colour = series[k % series.len()];
                    let from = (at * STEPS as f32) as usize;
                    let to = ((at + frac) * STEPS as f32) as usize;
                    for seg in from..to.min(STEPS) {
                        let (cs, sn) = unit(seg);
                        out.line(
                            Point { x: cx + cs * inner, y: cy + sn * inner },
                            Point { x: cx + cs * outer, y: cy + sn * outer },
                            // A hair over the chord between two
                            // segments, so neighbours overlap instead of
                            // leaving a gap.
                            (outer * 0.075).max(1.0),
                            colour,
                        );
                    }
                    at += frac;
                }
            }
            _ => {
                let max = self.data.max();
                if max <= 0.0 {
                    return;
                }
                let slot = w / n as f32;
                let bw = (slot * 0.68).max(1.0);
                for (k, v) in vals.iter().enumerate() {
                    if !v.is_finite() || *v <= 0.0 {
                        continue;
                    }
                    let bh = (v / max) * h;
                    let x = r[0] + slot * k as f32 + (slot - bw) / 2.0;
                    out.rounded_rect([x, r[3] - bh, x + bw, r[3]], s.fg, (bw * 0.18).min(bh / 2.0));
                }
            }
        }
    }

    leaf_common!(Chart<KIND>, 0, nohit);
}

/// One of [`STEPS_ROUND`] unit vectors around the circle, starting at
/// twelve o'clock and going clockwise — the direction every pie chart in
/// every toolkit reads, and the direction a spinner turns.
pub(crate) const STEPS_ROUND: usize = 96;

pub(crate) fn unit(seg: usize) -> (f32, f32) {
    // Quarter-turn table; the other three quadrants are sign flips of
    // it. Same argument as the spinner's ring: no trig in a `no_std`
    // crate that has needed none.
    const Q: usize = 24;
    const QUARTER: [(f32, f32); Q] = [
        (0.000_000, -1.000_000),
        (0.065_403, -0.997_859),
        (0.130_526, -0.991_445),
        (0.195_090, -0.980_785),
        (0.258_819, -0.965_926),
        (0.321_439, -0.946_930),
        (0.382_683, -0.923_880),
        (0.442_289, -0.896_873),
        (0.500_000, -0.866_025),
        (0.555_570, -0.831_470),
        (0.608_761, -0.793_353),
        (0.659_346, -0.751_840),
        // Clippy names the 45° entry as an approximation of
        // `FRAC_1_SQRT_2`, which it is — the table is a quarter turn in
        // equal steps, so one of its twenty-four entries is bound to be
        // a named constant. Written as the constant so the lint is
        // answered rather than silenced.
        (core::f32::consts::FRAC_1_SQRT_2, -core::f32::consts::FRAC_1_SQRT_2),
        (0.751_840, -0.659_346),
        (0.793_353, -0.608_761),
        (0.831_470, -0.555_570),
        (0.866_025, -0.500_000),
        (0.896_873, -0.442_289),
        (0.923_880, -0.382_683),
        (0.946_930, -0.321_439),
        (0.965_926, -0.258_819),
        (0.980_785, -0.195_090),
        (0.991_445, -0.130_526),
        (0.997_859, -0.065_403),
    ];
    let s = seg % (Q * 4);
    let (i, quad) = (s % Q, s / Q);
    let (x, y) = QUARTER[i];
    match quad {
        0 => (x, y),
        1 => (-y, x),
        2 => (-x, -y),
        _ => (y, -x),
    }
}

/// **No handlers.** Every widget in a tree must answer `Handlers`, and a
/// widget that carries no `on_*` field answers `None` — dispatch then
/// walks past it. This is one impl per type rather than a blanket one
/// because a blanket `impl<W, S> Handlers<S> for W` would collide with
/// every widget that *does* have handlers.
impl<S, const KIND: u8> lumen_static::event::Handlers<S> for Chart<KIND> {
    fn handler(
        &self,
        _sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &lumen_static::EventCtx)> {
        None
    }
}
