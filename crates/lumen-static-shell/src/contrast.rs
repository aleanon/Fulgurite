//! **Contrast, measured on what is painted** — M54/T54.0 and T54.5
//! (`docs/plan-a11y-platform.md` §0 rule 5, R16-F8).
//!
//! For each text command in a [`DisplayList`], the ground it sits on is
//! the topmost opaque fill painted **before** it whose rectangle holds
//! the run's centre, respecting the clips in force when that fill was
//! painted; with none, the page. The WCAG ratio of the text colour
//! against that ground is the run's contrast.
//!
//! Hand-written (text role, ground role) pairs are the "bound computed
//! from the wrong quantity" shape: they check pairs nobody paints and
//! miss pairs somebody does. This reads the list a renderer would draw.
//!
//! **A gradient is sampled, not guessed** (M82/T82.2). A run whose ground
//! is a gradient is measured against the gradient's colour at three
//! points -- the run's centre and both horizontal ends, on the centre's
//! row -- and the **minimum** ratio is the run's. The centre alone would
//! pass a run whose end sits on a failing stop. An end past the
//! gradient's rect is sampled at the rect's edge: the ground beyond it is
//! some other fill, which the centre walk did not reach. A gradient with
//! a translucent sample is still unmeasured, as `"translucent gradient"`.
//!
//! **A path is non-text ink** (M82/T82.5). Text over a [`Cmd::Path`]'s
//! ink is unmeasured, as `"path"`: a path is not one colour across its
//! box. The path itself is measured as a *shape*: its fill (its stroke,
//! when the fill is transparent) against the ground under its box's
//! centre, found by the same walk, into `Report::shapes`. WCAG 1.4.11
//! asks 3:1 of non-text ink (`NON_TEXT_RATIO`), not text's 4.5.
//!
//! **A shader bracket is unknown ground, and its fallback is none**
//! (M83/T83.3, `cfg(lumen_shader)`). Text over a `Cmd::Shader`'s rect is
//! unmeasured, as `"shader"`: its pixels are whatever the WGSL computes.
//! The commands inside the bracket are the fallback a backend that cannot
//! run the shader draws, and they are not ground for any run, inside the
//! bracket or after it; the bracket also clips like a `PushClip`.
//!
//! **What it cannot measure it counts, never skips.** A run whose ground
//! is translucent, an image or glass is `unmeasured`, with the reason; a
//! run whose centre is clipped away is `clipped` (nobody sees it). A
//! caller prints both beside the measured count, so "no text found"
//! cannot pass.

use lumen_static::display::{Cmd, Color, DisplayList, GradAxis, Rect};

/// WCAG 2.x relative luminance of an sRGB colour (alpha ignored).
pub fn luminance(c: Color) -> f32 {
    fn lin(v: u8) -> f32 {
        let v = v as f32 / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * lin(c.0) + 0.7152 * lin(c.1) + 0.0722 * lin(c.2)
}

/// WCAG contrast ratio between two opaque colours, 1.0 to 21.0.
pub fn ratio(a: Color, b: Color) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// `fg` composited over the opaque `bg` by its alpha.
pub fn over(fg: Color, bg: Color) -> Color {
    let a = fg.3 as f32 / 255.0;
    let mix = |f: u8, b: u8| (f as f32 * a + b as f32 * (1.0 - a)).round() as u8;
    Color(mix(fg.0, bg.0), mix(fg.1, bg.1), mix(fg.2, bg.2), 255)
}

/// One text run, measured.
#[derive(Clone, Debug)]
pub struct Run {
    /// Index of the `Cmd::Text` in the list.
    pub index: usize,
    pub text: String,
    pub rect: Rect,
    /// The text colour as painted: its alpha composited over `ground`.
    pub fg: Color,
    /// The ground the ratio is against; over a gradient, the worst of the
    /// three samples (the module's doc).
    pub ground: Color,
    pub ratio: f32,
}

/// WCAG 1.4.11's minimum for non-text ink — what a [`Shape`] is held
/// to, against text's 4.5.
pub const NON_TEXT_RATIO: f32 = 3.0;

/// One path, measured as non-text ink — M82/T82.5.
#[derive(Clone, Debug)]
pub struct Shape {
    /// Index of the `Cmd::Path` in the list.
    pub index: usize,
    pub rect: Rect,
    /// The path's fill (or, with a transparent fill, its stroke) as
    /// painted: composited over `ground`.
    pub fg: Color,
    pub ground: Color,
    pub ratio: f32,
}

/// Everything [`text_contrast`] found in one list.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub measured: Vec<Run>,
    /// `(index, text, why)` for a run whose ground could not be named.
    pub unmeasured: Vec<(usize, String, &'static str)>,
    /// Runs whose centre lies outside the clip in force: never seen.
    pub clipped: usize,
    /// Every path whose ground could be named, measured as non-text ink
    /// (M82/T82.5; the module's doc).
    pub shapes: Vec<Shape>,
    /// `(index, why)` for a path whose ground could not be named.
    pub unmeasured_shapes: Vec<(usize, &'static str)>,
}

impl Report {
    /// The lowest-contrast measured run.
    pub fn min(&self) -> Option<&Run> {
        self.measured.iter().min_by(|a, b| a.ratio.total_cmp(&b.ratio))
    }

    /// Measured runs below `bound`.
    pub fn below(&self, bound: f32) -> impl Iterator<Item = &Run> {
        self.measured.iter().filter(move |r| r.ratio < bound)
    }

    /// Measured paths below `bound` ([`NON_TEXT_RATIO`] for WCAG 1.4.11).
    pub fn shapes_below(&self, bound: f32) -> impl Iterator<Item = &Shape> {
        self.shapes.iter().filter(move |s| s.ratio < bound)
    }
}

fn holds(r: &Rect, x: f32, y: f32) -> bool {
    x >= r[0] && x < r[2] && y >= r[1] && y < r[3]
}

/// Whether `(x, y)` is inside a rounded rectangle: outside the corner
/// squares it is the rectangle; inside one, it is the quarter circle.
fn holds_rounded(r: &Rect, radius: f32, x: f32, y: f32) -> bool {
    if !holds(r, x, y) {
        return false;
    }
    let rad = radius.max(0.0).min((r[2] - r[0]) / 2.0).min((r[3] - r[1]) / 2.0);
    let cx = if x < r[0] + rad { r[0] + rad } else if x > r[2] - rad { r[2] - rad } else { return true };
    let cy = if y < r[1] + rad { r[1] + rad } else if y > r[3] - rad { r[3] - rad } else { return true };
    (x - cx) * (x - cx) + (y - cy) * (y - cy) <= rad * rad
}

/// What a command painted at a point, as a ground.
enum Ground {
    /// Nothing here; look further down.
    Miss,
    Opaque(Color),
    /// A gradient, opaque at the run's centre and both ends: those three
    /// colours, of which the run's ratio is the worst.
    Sampled([Color; 3]),
    /// Something is here, and it is not one colour.
    Unknown(&'static str),
}

/// A gradient's colour at the run's centre `(x, y)` and at its two ends,
/// `x0` and `x1` on the same row, each clamped into the gradient's rect.
fn gradient_ground(c: &Cmd, x: f32, y: f32, x0: f32, x1: f32) -> Ground {
    let Cmd::Gradient { rect, stops, kind, geom, .. } = c else { return Ground::Miss };
    let axis = GradAxis::new(*rect, *kind, *geom);
    let at = |px: f32| stops.sample(axis.t(px.clamp(rect[0], rect[2]), y));
    let samples = [at(x), at(x0), at(x1)];
    if samples.iter().any(|s| s.3 != 255) {
        return Ground::Unknown("translucent gradient");
    }
    Ground::Sampled(samples)
}

fn ground_at(c: &Cmd, x: f32, y: f32) -> Ground {
    let solid = |color: Color| {
        if color.3 == 255 {
            Ground::Opaque(color)
        } else if color.3 == 0 {
            Ground::Miss
        } else {
            Ground::Unknown("translucent fill")
        }
    };
    match c {
        Cmd::Rect { rect, color } if holds(rect, x, y) => solid(*color),
        Cmd::RoundedRect { rect, color, radius } if holds_rounded(rect, *radius, x, y) => solid(*color),
        // The caller samples it (`gradient_ground`); here it only claims
        // the point, inside its rounded corners.
        Cmd::Gradient { rect, corner, .. } if holds_rounded(rect, *corner, x, y) => Ground::Sampled([Color(0, 0, 0, 0); 3]),
        Cmd::Image { rect, .. } if holds(rect, x, y) => Ground::Unknown("image"),
        Cmd::Glass { rect, .. } if holds(rect, x, y) => Ground::Unknown("glass"),
        Cmd::Shadow { rect, .. } if holds(rect, x, y) => Ground::Unknown("shadow"),
        // Not one colour across its box (M82/T82.5); the path is measured
        // itself, as a shape.
        Cmd::Path { rect, .. } if holds(rect, x, y) => Ground::Unknown("path"),
        // M83/T83.3: a shader's output is whatever its WGSL computes, so a
        // run over it is unmeasured. Its fallback is not the ground: the
        // walk skips every command inside a bracket (`text_contrast`), so
        // this arm is what a run over the bracket meets.
        #[cfg(lumen_shader)]
        Cmd::Shader { rect, .. } if holds(rect, x, y) => Ground::Unknown("shader"),
        _ => Ground::Miss,
    }
}

/// Measure every text run in `list` against the ground it is painted on,
/// with `page` behind everything.
pub fn text_contrast(list: &DisplayList, page: Color) -> Report {
    // The clip in force at each command: the intersection of every open
    // `PushClip`, or `None` for unclipped.
    let mut clips: Vec<Rect> = Vec::new();
    let mut at: Vec<Option<Rect>> = Vec::with_capacity(list.cmds.len());
    // Whether each command is inside a shader bracket (M83/T83.3): a
    // fallback, which a GPU that runs the shader never draws, so it is not
    // ground for anything. Only the `lumen_shader` build can set it.
    #[cfg_attr(not(lumen_shader), allow(unused_mut))]
    let mut depth = 0u32;
    let mut fallback: Vec<bool> = Vec::with_capacity(list.cmds.len());
    for c in &list.cmds {
        at.push(clips.last().copied());
        fallback.push(depth > 0);
        let push = |clips: &mut Vec<Rect>, rect: &Rect| {
            let r = match clips.last() {
                Some(o) => [rect[0].max(o[0]), rect[1].max(o[1]), rect[2].min(o[2]), rect[3].min(o[3])],
                None => *rect,
            };
            clips.push(r);
        };
        match c {
            Cmd::PushClip { rect } => push(&mut clips, rect),
            Cmd::PopClip => {
                clips.pop();
            }
            // A bracket clips its fallback as a `PushClip` would.
            #[cfg(lumen_shader)]
            Cmd::Shader { rect, .. } => {
                push(&mut clips, rect);
                depth += 1;
            }
            #[cfg(lumen_shader)]
            Cmd::ShaderEnd => {
                clips.pop();
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    let visible = |k: usize, x: f32, y: f32| at[k].is_none_or(|r| holds(&r, x, y));

    // The topmost ground painted before command `k` at `(x, y)`, a
    // gradient sampled at `x`, `x0` and `x1`; the page when nothing is.
    let ground_under = |k: usize, x: f32, y: f32, x0: f32, x1: f32| {
        for j in (0..k).rev() {
            if !visible(j, x, y) || fallback[j] {
                continue;
            }
            match ground_at(&list.cmds[j], x, y) {
                Ground::Miss => continue,
                Ground::Sampled(_) => return gradient_ground(&list.cmds[j], x, y, x0, x1),
                g => return g,
            }
        }
        Ground::Opaque(page)
    };

    let mut report = Report::default();
    for (k, c) in list.cmds.iter().enumerate() {
        // A path is non-text ink: its paint against the ground under its
        // box's centre (M82/T82.5).
        if let Cmd::Path { rect, fill, stroke, .. } = c {
            let paint = if fill.3 != 0 { *fill } else { *stroke };
            let (x, y) = ((rect[0] + rect[2]) / 2.0, (rect[1] + rect[3]) / 2.0);
            if paint.3 == 0 || !visible(k, x, y) {
                continue;
            }
            match ground_under(k, x, y, x, x) {
                Ground::Opaque(g) | Ground::Sampled([g, ..]) => {
                    let fg = over(paint, g);
                    report.shapes.push(Shape { index: k, rect: *rect, fg, ground: g, ratio: ratio(fg, g) });
                }
                Ground::Unknown(why) => report.unmeasured_shapes.push((k, why)),
                Ground::Miss => unreachable!(),
            }
            continue;
        }
        let Cmd::Text { rect, text, color, .. } = c else { continue };
        let s = list.str_of(*text);
        if s.trim().is_empty() || color.3 == 0 {
            continue;
        }
        let (x, y) = ((rect[0] + rect[2]) / 2.0, (rect[1] + rect[3]) / 2.0);
        if !visible(k, x, y) {
            report.clipped += 1;
            continue;
        }
        let ground = ground_under(k, x, y, rect[0], rect[2]);
        match ground {
            Ground::Opaque(g) => {
                let fg = over(*color, g);
                report.measured.push(Run { index: k, text: s.to_string(), rect: *rect, fg, ground: g, ratio: ratio(fg, g) });
            }
            Ground::Sampled(samples) => {
                // The worst of the three: a run is as legible as its least
                // legible part.
                let (fg, g, r) = samples
                    .iter()
                    .map(|g| {
                        let fg = over(*color, *g);
                        (fg, *g, ratio(fg, *g))
                    })
                    .min_by(|a, b| a.2.total_cmp(&b.2))
                    .expect("three samples");
                report.measured.push(Run { index: k, text: s.to_string(), rect: *rect, fg, ground: g, ratio: r });
            }
            Ground::Unknown(why) => report.unmeasured.push((k, s.to_string(), why)),
            Ground::Miss => unreachable!(),
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_on_white_is_21_and_a_colour_on_itself_is_1() {
        let (b, w) = (Color(0, 0, 0, 255), Color(255, 255, 255, 255));
        assert!((ratio(b, w) - 21.0).abs() < 0.01, "{}", ratio(b, w));
        assert!((ratio(w, w) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_ground_is_the_topmost_fill_under_the_centre_inside_its_clip() {
        let mut l = DisplayList::new();
        l.rect([0.0, 0.0, 100.0, 100.0], Color(0, 0, 0, 255));
        // Clipped away at the text's centre: must not count as its ground.
        l.push_clip([0.0, 0.0, 10.0, 10.0]);
        l.rect([0.0, 0.0, 100.0, 100.0], Color(255, 255, 255, 255));
        l.pop_clip();
        l.text([40.0, 40.0, 60.0, 60.0], "hi", 12.0, Color(255, 255, 255, 255));
        let r = text_contrast(&l, Color(128, 128, 128, 255));
        let run = &r.measured[0];
        println!("ground {:?} ratio {:.2}", run.ground, run.ratio);
        assert_eq!(run.ground, Color(0, 0, 0, 255), "the clipped white fill was taken as the ground");
    }

    #[test]
    fn a_translucent_ground_is_unmeasured_not_skipped() {
        let mut l = DisplayList::new();
        l.rect([0.0, 0.0, 100.0, 100.0], Color(0, 0, 0, 128));
        l.text([40.0, 40.0, 60.0, 60.0], "hi", 12.0, Color(255, 255, 255, 255));
        let r = text_contrast(&l, Color(255, 255, 255, 255));
        assert_eq!((r.measured.len(), r.unmeasured.len()), (0, 1));
    }
}
