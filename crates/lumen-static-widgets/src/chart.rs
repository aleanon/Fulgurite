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
use lumen_static::{Constraints, LayoutCx, Point, Size};

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
///
/// **A non-finite sample breaks the line, not the chart.** The scale
/// skips it (`Series::max`) and the segments that touch it are not
/// drawn, so one bad value costs a gap rather than the whole plot — a
/// chart is a view of numbers it did not produce.
pub const LINE: u8 = 1;
/// Proportions of a whole, as a ring.
pub const PIE: u8 = 2;

/// A chart. See the module docs for why `KIND` is a const.
///
/// # Axes and inspection — M57/B30, B31
///
/// A bar or line chart was a shape with no scale: nothing said what a
/// bar's height meant, and nothing answered "what is this one?"
/// (`docs/widget-audit.md`, BarChart, LineChart). With `axes`, the plot
/// is inset for **y ticks** (a nice step, labelled) and **x labels**
/// (from `labels`), over a baseline, all in the style's `border_color`
/// (role `Muted`); the chart's own `border_width` stays 0, so that colour
/// is ink, not an edge. **A hover** marks the bar or point under the
/// pointer and shows its value in a pill. A line joins its segments with
/// round dots and is inset by half its stroke, so neither its ends nor
/// its peaks are cut by the box.
pub struct Chart<const KIND: u8 = BAR> {
    /// The values. A presentation copy — bind it.
    pub data: Series,
    /// The colours a pie's wedges take in turn — M25/T25.10. The built-in
    /// six by default; an empty slice falls back to them rather than
    /// painting nothing.
    pub series: &'static [Color],
    /// Draw a y scale, a baseline and x labels (bar and line only).
    pub axes: bool,
    /// One label per value, under its bar or point, when `axes` is on.
    pub labels: &'static [&'static str],
    /// The value under the pointer.
    hot: Option<usize>,
    /// From layout: the tick step and count, the gutters the axes take,
    /// and the hovered value's label width.
    step: f32,
    ticks: usize,
    gutter: f32,
    foot: f32,
    tip_w: f32,
    /// Its laid-out size, for turning a hover's local x into a value.
    size: Size,
}

pub type BarChart = Chart<BAR>;
pub type LineChart = Chart<LINE>;
pub type PieChart = Chart<PIE>;

impl<const KIND: u8> Default for Chart<KIND> {
    fn default() -> Self {
        Chart::new(&[])
    }
}

impl<const KIND: u8> Chart<KIND> {
    pub fn new(values: &[f32]) -> Self {
        Chart {
            data: Series::new(values),
            series: &WEDGES,
            axes: false,
            labels: &[],
            hot: None,
            step: 0.0,
            ticks: 0,
            gutter: 0.0,
            foot: 0.0,
            tip_w: 0.0,
            size: Size { w: 0.0, h: 0.0 },
        }
    }

    /// The value under the pointer, if any.
    pub fn hot(&self) -> Option<usize> {
        self.hot
    }

    /// The plot area within `r`: inset for the axes and by half a line's
    /// stroke, so nothing drawn at a value's extreme is cut.
    fn plot(&self, r: [f32; 4]) -> [f32; 4] {
        let half = if KIND == LINE { Self::stroke(r[3] - r[1]) / 2.0 + 1.0 } else { 0.0 };
        let (g, f) = if self.axes && KIND != PIE { (self.gutter, self.foot) } else { (0.0, 0.0) };
        [r[0] + g + half, r[1] + half + if self.axes { 6.0 } else { 0.0 }, r[2] - half, r[3] - f - half]
    }

    fn stroke(h: f32) -> f32 {
        (h * 0.018).max(1.5)
    }

    /// The top of the scale: the ticks' top when there are axes, the
    /// data's maximum otherwise.
    fn top(&self) -> f32 {
        if self.axes && self.ticks > 0 { self.step * self.ticks as f32 } else { self.data.max() }
    }

    /// Which value an x in the plot is over.
    fn index_at(&self, plot: [f32; 4], x: f32) -> Option<usize> {
        let n = self.data.len();
        let w = plot[2] - plot[0];
        if n == 0 || w <= 0.0 || x < plot[0] || x > plot[2] {
            return None;
        }
        let k = if KIND == LINE {
            if n == 1 { 0.0 } else { lumen_static::num::round((x - plot[0]) / (w / (n - 1) as f32)) }
        } else {
            lumen_static::num::floor((x - plot[0]) / (w / n as f32))
        };
        Some((k as usize).min(n - 1))
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
            border_color: Color(0x5b, 0x66, 0x7a, 0xff),
            font_size: 11.0,
            ..Style::default()
        }
    }

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, match KIND {
            // Hittable since M57/B30 (for hover), so the pointer's cursor
            // and shades are read like any hittable leaf's.
            PIE => lumen_static::style::Fields::of(&[
                lumen_static::style::Fields::WIDTH,
                lumen_static::style::Fields::HEIGHT,
                lumen_static::style::Fields::VISIBLE,
                lumen_static::style::Fields::BACKGROUND,
                lumen_static::style::Fields::BORDER,
                lumen_static::style::Fields::BACKGROUND_FILL,
                lumen_static::style::Fields::RADIUS,
                lumen_static::style::Fields::CURSOR,
                lumen_static::style::Fields::PRESSED_DIM,
                lumen_static::style::Fields::HOVER_DIM,
            ]),
            _ => lumen_static::style::Fields::of(&[
                lumen_static::style::Fields::WIDTH,
                lumen_static::style::Fields::HEIGHT,
                lumen_static::style::Fields::VISIBLE,
                lumen_static::style::Fields::COLOR,
                lumen_static::style::Fields::BACKGROUND,
                lumen_static::style::Fields::BORDER,
                lumen_static::style::Fields::BACKGROUND_FILL,
                lumen_static::style::Fields::RADIUS,
                lumen_static::style::Fields::FONT_SIZE,
                lumen_static::style::Fields::CURSOR,
                lumen_static::style::Fields::PRESSED_DIM,
                lumen_static::style::Fields::HOVER_DIM,
            ]),
        })
        // `border` is the axes' ink (M57/B30): muted text, at 4.5:1.
        .with_roles(lumen_static::style::SeedRoles::NONE.color(lumen_static::style::Role::Accent).border(lumen_static::style::Role::Muted))];
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
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = style[i];
        let size = resolve_self(limits, &me, CHART);
        // The scale and the room its labels take, measured here because
        // `Painter` cannot measure.
        if self.axes && KIND != PIE {
            let (step, ticks) = nice_scale(self.data.max());
            self.step = step;
            self.ticks = ticks;
            let px = me.font_size;
            let mut widest = 0.0f32;
            for t in 0..=ticks {
                let mut buf = [0u8; 16];
                let label = fmt_num(step * t as f32, step, &mut buf);
                widest = widest.max(cx.text.measure(label, px, f32::INFINITY).w);
            }
            self.gutter = widest + 8.0;
            self.foot = if self.labels.is_empty() { 4.0 } else { px * 1.6 };
        } else {
            self.gutter = 0.0;
            self.foot = 0.0;
        }
        self.tip_w = match self.hot.and_then(|k| self.data.as_slice().get(k)) {
            Some(v) => {
                let mut buf = [0u8; 16];
                let label = fmt_num(*v, if self.step > 0.0 { self.step / 10.0 } else { 0.01 }, &mut buf);
                cx.text.measure(label, me.font_size, f32::INFINITY).w
            }
            None => 0.0,
        };
        self.size = size;
        record_painted(st, i, limits, size, &me)
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = rect_of(s);
        let (w, h) = (r[2] - r[0], r[3] - r[1]);
        if w <= 0.0 || h <= 0.0 || self.data.is_empty() {
            return;
        }
        lumen_static::ops::fill(st, i, out);
        if s.bg.3 > 0 {
            out.rounded_rect(r, s.bg, s.radius);
            lumen_static::ops::border(st, i, out);
        }
        let n = self.data.len();
        let vals = self.data.as_slice();
        let plot = self.plot(r);
        let (pw, ph) = (plot[2] - plot[0], plot[3] - plot[1]);
        let top = self.top();
        if KIND != PIE && self.axes {
            self.paint_axes(s, r, plot, out);
        }
        match KIND {
            LINE => {
                if top <= 0.0 || pw <= 0.0 || ph <= 0.0 {
                    return;
                }
                let step = if n > 1 { pw / (n - 1) as f32 } else { 0.0 };
                let y_of = |v: f32| plot[3] - (v.max(0.0) / top) * ph;
                let stroke = Self::stroke(h);
                for k in 1..n {
                    let (a, b) = (vals[k - 1], vals[k]);
                    if !a.is_finite() || !b.is_finite() {
                        continue;
                    }
                    out.line(
                        Point { x: plot[0] + step * (k - 1) as f32, y: y_of(a) },
                        Point { x: plot[0] + step * k as f32, y: y_of(b) },
                        stroke,
                        s.fg,
                    );
                }
                // **Round joins** (M57/B31): a dot of the stroke's width
                // at every point, so segments meet without a notch.
                for (k, v) in vals.iter().enumerate() {
                    if !v.is_finite() {
                        continue;
                    }
                    let (x, y) = (plot[0] + step * k as f32, y_of(*v));
                    let d = if self.hot == Some(k) { stroke * 3.0 } else { stroke };
                    out.rounded_rect([x - d / 2.0, y - d / 2.0, x + d / 2.0, y + d / 2.0], s.fg, d / 2.0);
                }
                if let Some(k) = self.hot.filter(|&k| k < n && vals[k].is_finite()) {
                    self.paint_tip(s, plot[0] + step * k as f32, y_of(vals[k]), vals[k], out);
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
                // The last wedge that draws anything: it ends at the full
                // turn exactly (M57/B32).
                let last = vals.iter().rposition(|v| v.is_finite() && *v > 0.0);
                for (k, v) in vals.iter().enumerate() {
                    if !v.is_finite() || *v <= 0.0 {
                        continue;
                    }
                    let frac = v / total;
                    let series = if self.series.is_empty() { &WEDGES[..] } else { self.series };
                    let colour = series[k % series.len()];
                    // **Rounded, not truncated, and the last one closes the
                    // ring** — M57/B32. Truncating each boundary and summing
                    // fractions in f32, which ends just short of 1.0, left
                    // the final segment undrawn: a 1.35% gap at 12 o'clock
                    // (`docs/widget-audit.md`, PieChart). Rounded, adjacent
                    // wedges share each boundary exactly.
                    let seg = |t: f32| lumen_static::num::round(t * STEPS as f32) as usize;
                    let from = seg(at);
                    let to = if Some(k) == last { STEPS } else { seg(at + frac) };
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
                if top <= 0.0 || pw <= 0.0 || ph <= 0.0 {
                    return;
                }
                let slot = pw / n as f32;
                let bw = (slot * 0.68).max(1.0);
                for (k, v) in vals.iter().enumerate() {
                    if !v.is_finite() || *v <= 0.0 {
                        continue;
                    }
                    let bh = (v / top) * ph;
                    let x = plot[0] + slot * k as f32 + (slot - bw) / 2.0;
                    // The hovered bar a shade darker.
                    let c = if self.hot == Some(k) { Color(s.fg.0 / 5 * 4, s.fg.1 / 5 * 4, s.fg.2 / 5 * 4, s.fg.3) } else { s.fg };
                    out.rounded_rect([x, plot[3] - bh, x + bw, plot[3]], c, (bw * 0.18).min(bh / 2.0));
                }
                if let Some(k) = self.hot.filter(|&k| k < n && vals[k].is_finite()) {
                    let bh = (vals[k].max(0.0) / top) * ph;
                    self.paint_tip(s, plot[0] + slot * (k as f32 + 0.5), plot[3] - bh, vals[k], out);
                }
            }
        }
    }

    /// A figure, its values as its text — M54/T54.3 (R15-F3): a chart
    /// reported `GenericContainer`, so a screen reader announced nothing
    /// where a sighted user reads the data.
    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut alloc::vec::Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        use core::fmt::Write;
        node.set_role(lumen_static::observe::accesskit::Role::Figure);
        let mut v = alloc::string::String::new();
        for (k, x) in self.data.as_slice().iter().enumerate() {
            let _ = write!(v, "{}{x}", if k > 0 { ", " } else { "" });
        }
        node.set_value(v);
    }

    /// A hover picks the bar or point under the pointer (M57/B30, B31);
    /// leaving clears it. A pie has no inspection.
    fn on_event(&mut self, ev: &lumen_static::Event, cx: &lumen_static::EventCtx) -> bool {
        if KIND == PIE {
            return false;
        }
        match ev {
            lumen_static::Event::Hover { .. } => {
                let r = [0.0, 0.0, self.size.w, self.size.h];
                let k = self.index_at(self.plot(r), cx.local.x);
                core::mem::replace(&mut self.hot, k) != k
            }
            lumen_static::Event::Leave => self.hot.take().is_some(),
            _ => false,
        }
    }

    lumen_static::leaf_common!(Chart<KIND>, 0);
}

impl<const KIND: u8> Chart<KIND> {
    fn paint_axes(&self, s: &NodeState, r: [f32; 4], plot: [f32; 4], out: &mut dyn Painter) {
        let ink = s.border_color;
        let faint = Color(ink.0, ink.1, ink.2, 0x40);
        let ph = plot[3] - plot[1];
        // Ticks: a faint rule across the plot, and the value at the left.
        for t in 0..=self.ticks {
            let v = self.step * t as f32;
            let y = plot[3] - if self.top() > 0.0 { v / self.top() * ph } else { 0.0 };
            let c = if t == 0 { ink } else { faint };
            out.rect([plot[0], y - 0.5, plot[2], y + 0.5], c);
            let mut buf = [0u8; 16];
            let label = fmt_num(v, self.step, &mut buf);
            out.text([r[0], y - s.font_size * 0.7, plot[0] - 6.0, y + s.font_size * 0.7], label, s.font_size, ink);
        }
        // x labels under their bar or point.
        let n = self.data.len();
        if n > 0 && !self.labels.is_empty() {
            let pw = plot[2] - plot[0];
            for (k, l) in self.labels.iter().take(n).enumerate() {
                let x = if KIND == LINE {
                    plot[0] + if n > 1 { pw / (n - 1) as f32 * k as f32 } else { 0.0 }
                } else {
                    plot[0] + pw / n as f32 * (k as f32 + 0.5)
                };
                let half = s.font_size * 0.3 * l.len() as f32;
                out.text([x - half, plot[3] + 3.0, x + half + 8.0, r[3]], l, s.font_size, ink);
            }
        }
    }

    /// The hovered value, in a pill above its bar or point.
    fn paint_tip(&self, s: &NodeState, x: f32, y: f32, v: f32, out: &mut dyn Painter) {
        let mut buf = [0u8; 16];
        let label = fmt_num(v, if self.step > 0.0 { self.step / 10.0 } else { 0.01 }, &mut buf);
        let (w, h) = (self.tip_w + 12.0, s.font_size + 8.0);
        let top = (y - h - 6.0).max(s.pos.y);
        let left = (x - w / 2.0).clamp(s.pos.x, s.pos.x + s.size.w - w);
        let pill = s.border_color;
        // Black or white, whichever reads on the pill: its colour is the
        // palette's muted text, light on a dark page and dark on a light.
        let lum = 0.2126 * pill.0 as f32 + 0.7152 * pill.1 as f32 + 0.0722 * pill.2 as f32;
        let text = if lum > 140.0 { Color(0, 0, 0, 0xff) } else { Color(0xff, 0xff, 0xff, 0xff) };
        out.rounded_rect([left, top, left + w, top + h], pill, h / 2.0);
        out.text([left + 6.0, top + 4.0, left + w, top + h], label, s.font_size, text);
    }
}

/// A tick step of 1, 2 or 5 times a power of ten that splits `max` into
/// at most five ticks, and how many. No `libm`: the powers are found by
/// multiplying, which is exact enough for a label.
pub fn nice_scale(max: f32) -> (f32, usize) {
    if !max.is_finite() || max <= 0.0 {
        return (1.0, 1);
    }
    let raw = max / 4.0;
    let mut mag = 1.0f32;
    while mag * 10.0 <= raw {
        mag *= 10.0;
    }
    while mag > raw {
        mag /= 10.0;
    }
    let step = [1.0, 2.0, 5.0, 10.0].iter().map(|m| m * mag).find(|s| *s >= raw).unwrap_or(10.0 * mag);
    let ticks = lumen_static::num::ceil(max / step - 1e-4) as usize;
    (step, ticks.max(1))
}

/// `v` as text, with as many decimals as `step` needs (0, 1 or 2), into
/// `buf`. No `format!`: this runs in `paint` on a crate that has no
/// allocator in its smallest profile.
pub fn fmt_num(v: f32, step: f32, buf: &mut [u8; 16]) -> &str {
    let decimals = if step >= 1.0 { 0 } else if step >= 0.1 { 1 } else { 2 };
    let scale = [1.0, 10.0, 100.0][decimals];
    let neg = v < 0.0;
    let mut n = lumen_static::num::round(v.abs() * scale) as u64;
    let mut i = buf.len();
    let mut digits = 0;
    loop {
        if digits == decimals && decimals > 0 {
            i -= 1;
            buf[i] = b'.';
        }
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        digits += 1;
        if (n == 0 && digits > decimals) || i < 2 {
            break;
        }
    }
    if neg {
        i -= 1;
        buf[i] = b'-';
    }
    core::str::from_utf8(&buf[i..]).unwrap_or("?")
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
