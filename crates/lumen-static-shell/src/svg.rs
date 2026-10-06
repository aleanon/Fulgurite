//! A second [`Renderer`], and a useful one — `docs/plan-capabilities.md`
//! T15.12.
//!
//! # Why this exists
//!
//! It was written under T15.12 as an argument that a GPU backend was
//! not yet needed, on the ground that the software canvas renders a
//! live tally frame in 0.54 ms of a 16 ms budget
//! (`docs/results-competitive.md`).
//!
//! **That argument was wrong, and the GPU backend now exists**
//! (`lumen-static-wgpu`, `docs/results-gpu.md`). The 0.54 ms was
//! measured on a 0.23 megapixel window, and resolution is the one
//! variable a CPU rasteriser is most sensitive to: swept, it is ~2.7 ms
//! per megapixel and a 4K redraw is over budget
//! (`docs/results-raster-scaling.md`).
//!
//! What survives is the *other* thing this renderer was for, which
//! turned out to be the useful half: it proves [`Renderer`] admits a
//! second implementation — so the GPU backend was bounded work — and it
//! is useful on its own, for export, print, and documentation
//! screenshots that stay sharp. It handles the **whole** command
//! vocabulary, and since M51/T51.6 `tests/svg.rs` checks that
//! mechanically rather than by listing: its `kind` matches every `Cmd`
//! variant with no wildcard, so a new command does not compile there
//! until it has a sample, and each sample must emit its own element.
//! (Until then the test drew seven kinds, no border and no glass, and
//! both arms could emit nothing with the suite green — R9-F10.)
//!
//! It is also the first renderer here that is not a rasteriser at all,
//! which is a stronger test of the seam than a second rasteriser would
//! have been.

use core::fmt::Write;

use lumen_static::display::{Cmd, Color, DisplayList, GradAxis, GradKind, ImageId, PathEl, Rect, Renderer, Stops};

/// Renders a frame to SVG.
pub struct Svg {
    /// The document so far.
    out: String,
    /// How many clip scopes are open, so they can be closed.
    clips: usize,
    /// Gradient definitions, which SVG requires up front.
    defs: String,
    grads: usize,
}

impl Default for Svg {
    fn default() -> Self {
        Self::new()
    }
}

impl Svg {
    pub fn new() -> Self {
        Svg { out: String::new(), clips: 0, defs: String::new(), grads: 0 }
    }

    /// The finished document.
    pub fn finish(self) -> String {
        self.out
    }

    fn colour(c: Color) -> String {
        // `rgba()` rather than hex plus fill-opacity: one attribute, and
        // every SVG renderer since 2016 takes it.
        format!("rgba({},{},{},{:.3})", c.0, c.1, c.2, c.3 as f32 / 255.0)
    }

    /// A `<linearGradient>` or, since M82/T82.2, a `<radialGradient>`.
    ///
    /// A radial is written in **user space** (`cx`, `cy`, `r` in px, from
    /// [`GradAxis`], the canvas's own formula): in object-bounding-box
    /// units SVG scales `r` by the box's normalised diagonal, so a
    /// non-square rect would draw an ellipse where the canvas draws a
    /// circle reaching the farthest corner.
    fn gradient_def(&mut self, rect: Rect, stops: Stops, kind: GradKind, geom: [f32; 3]) -> String {
        let id = format!("g{}", self.grads);
        self.grads += 1;
        let close = match kind {
            GradKind::Linear => {
                // A unit vector along the gradient, which is how SVG
                // expresses direction — the canvas expresses the same
                // thing as an angle.
                let (dx, dy) = (geom[0].cos(), geom[0].sin());
                let (x1, y1) = (0.5 - dx / 2.0, 0.5 - dy / 2.0);
                let (x2, y2) = (0.5 + dx / 2.0, 0.5 + dy / 2.0);
                let _ = write!(
                    self.defs,
                    r#"<linearGradient id="{id}" x1="{x1:.3}" y1="{y1:.3}" x2="{x2:.3}" y2="{y2:.3}">"#
                );
                "</linearGradient>"
            }
            GradKind::Radial => {
                let [cx, cy, r, _] = GradAxis::new(rect, kind, geom).uv();
                let _ = write!(
                    self.defs,
                    r#"<radialGradient id="{id}" gradientUnits="userSpaceOnUse" cx="{cx:.2}" cy="{cy:.2}" r="{r:.2}">"#
                );
                "</radialGradient>"
            }
        };
        for k in 0..(stops.len as usize).min(lumen_static::display::MAX_STOPS) {
            let _ = write!(
                self.defs,
                r#"<stop offset="{:.3}" stop-color="{}"/>"#,
                // The offset, not the byte: `{:.3}` ignores precision on an
                // integer, so `at[k]` here would write `offset="51"` for 0.2
                // (M82/T82.1).
                stops.offset(k),
                Self::colour(stops.color[k])
            );
        }
        self.defs.push_str(close);
        id
    }

    /// Open a `<g>` clipped to `rect`, for a `PushClip` and for a shader
    /// bracket (M83/T83.3), which draw their contents the same way here.
    fn open_clip(&mut self, body: &mut String, rect: Rect) {
        let id = format!("c{}", self.clips);
        let _ = write!(
            self.defs,
            r#"<clipPath id="{id}"><rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}"/></clipPath>"#,
            rect[0],
            rect[1],
            rect[2] - rect[0],
            rect[3] - rect[1]
        );
        let _ = write!(body, r#"<g clip-path="url(#{id})">"#);
        self.clips += 1;
    }

    /// Close the innermost group, never below zero.
    fn close_clip(&mut self, body: &mut String) {
        if self.clips > 0 {
            body.push_str("</g>");
            self.clips -= 1;
        }
    }

    /// XML-escape text. Without this a label containing `&` or `<`
    /// produces a document no parser will open — which is the kind of
    /// failure that shows up in someone else's tool, days later.
    fn escape(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '&' => out.push_str("&amp;"),
                '"' => out.push_str("&quot;"),
                _ => out.push(c),
            }
        }
        out
    }
}

impl Renderer for Svg {
    fn render(&mut self, list: &DisplayList, width: u32, height: u32) {
        let mut body = String::new();
        self.defs.clear();
        self.clips = 0;

        for c in &list.cmds {
            match c {
                Cmd::Rect { rect, color } => {
                    let _ = write!(
                        body,
                        r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}"/>"#,
                        rect[0],
                        rect[1],
                        rect[2] - rect[0],
                        rect[3] - rect[1],
                        Self::colour(*color)
                    );
                }
                Cmd::RoundedRect { rect, color, radius } => {
                    let _ = write!(
                        body,
                        r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="{radius:.2}" fill="{}"/>"#,
                        rect[0],
                        rect[1],
                        rect[2] - rect[0],
                        rect[3] - rect[1],
                        Self::colour(*color)
                    );
                }
                Cmd::StrokedRect { rect, color, radius, width } => {
                    // SVG strokes are *centred* on the path and ours is
                    // inset, so the path is inset by half the width and
                    // the radius follows it. Painting the box edge at
                    // full width instead would put half the border
                    // outside the node, which is the difference between
                    // a border and a focus ring.
                    let h = width / 2.0;
                    let _ = write!(
                        body,
                        r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="{:.2}" fill="none" stroke="{}" stroke-width="{width:.2}"/>"#,
                        rect[0] + h,
                        rect[1] + h,
                        (rect[2] - rect[0] - width).max(0.0),
                        (rect[3] - rect[1] - width).max(0.0),
                        (radius - h).max(0.0),
                        Self::colour(*color)
                    );
                }
                Cmd::Shadow { rect, color, radius, spread } => {
                    // No blur filter: a filter per shadow is expensive
                    // to render and the canvas's own shadow is one SDF
                    // pass. A soft edge as a translucent outset rect is
                    // close enough for an export and honest about being
                    // an approximation.
                    let _ = write!(
                        body,
                        r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="{:.2}" fill="{}" opacity="0.35"/>"#,
                        rect[0] - spread,
                        rect[1] - spread,
                        (rect[2] - rect[0]) + spread * 2.0,
                        (rect[3] - rect[1]) + spread * 2.0,
                        radius + spread,
                        Self::colour(*color)
                    );
                }
                // `line_height` is named and ignored: an SVG `<text>` is
                // one line, so there is no second line to pitch (M80/T80.1).
                Cmd::Text { rect, text, size, weight, line_height: _, color } => {
                    let _ = write!(
                        body,
                        r#"<text x="{:.2}" y="{:.2}" font-size="{size:.2}" font-weight="{weight:.0}" fill="{}" xml:space="preserve">{}</text>"#,
                        rect[0],
                        // SVG positions text on its baseline; the
                        // command's rect is its box, so the baseline is
                        // approximated at 0.8 of the size down.
                        rect[1] + size * 0.8,
                        Self::colour(*color),
                        Self::escape(list.str_of(*text))
                    );
                }
                Cmd::Line { a, b, width: w, color } => {
                    let _ = write!(
                        body,
                        r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="{w:.2}" stroke-linecap="round"/>"#,
                        a.x,
                        a.y,
                        b.x,
                        b.y,
                        Self::colour(*color)
                    );
                }
                Cmd::Gradient { rect, stops, kind, geom, corner } => {
                    let id = self.gradient_def(*rect, *stops, *kind, *geom);
                    // The corner as `rx`, as a `RoundedRect` writes it, and
                    // only when there is one, so a square gradient's
                    // element is what it was before M82/T82.2.
                    let rx = if *corner > 0.0 { format!(r#" rx="{corner:.2}""#) } else { String::new() };
                    let _ = write!(
                        body,
                        r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}"{rx} fill="url(#{id})"/>"#,
                        rect[0],
                        rect[1],
                        rect[2] - rect[0],
                        rect[3] - rect[1]
                    );
                }
                Cmd::Image { rect, id, tint, frame } => {
                    // A placeholder with the id and the frame, not the
                    // pixels: this renderer has no `ImageSource` and
                    // inventing one would embed a base64 payload nobody
                    // asked for. An exporter that wants images resolves
                    // them itself, at `data-frame` (M84/T84.2).
                    let ImageId(n) = id;
                    let _ = write!(
                        body,
                        r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}" data-image="{n}" data-frame="{frame}"/>"#,
                        rect[0],
                        rect[1],
                        rect[2] - rect[0],
                        rect[3] - rect[1],
                        Self::colour(*tint)
                    );
                }
                Cmd::Glass { rect, radius, material } => {
                    // **SVG has a real Gaussian blur and this does not
                    // use it.** `feGaussianBlur` with `BackgroundImage`
                    // as its input was removed from the spec and no
                    // browser implements it, so a faithful export would
                    // need the backdrop rasterised into the document —
                    // pixels this renderer deliberately does not have
                    // (see the image arm above). The degraded panel is
                    // the honest answer, and it carries `data-blur` so
                    // an exporter that *can* resolve a backdrop knows
                    // what was asked for.
                    let (blur, tint) = (material.blur, material.tint);
                    let a = tint.3.max(lumen_static::display::GLASS_FALLBACK_ALPHA);
                    let _ = write!(
                        body,
                        r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="{:.2}" fill="{}" data-blur="{:.2}"/>"#,
                        rect[0],
                        rect[1],
                        rect[2] - rect[0],
                        rect[3] - rect[1],
                        radius,
                        Self::colour(Color(tint.0, tint.1, tint.2, a)),
                        blur
                    );
                }
                Cmd::Path { els, fill, stroke, width, .. } => {
                    // The elements are already through the path's
                    // transform (`DisplayList::path`), so `d` is in user
                    // space with no transform attribute (M82/T82.5).
                    // Round joins and caps, non-zero: what both
                    // rasterisers draw.
                    let (at, n) = (els.0 as usize, els.1 as usize);
                    let Some(els) = list.paths.get(at..at + n) else { continue };
                    let mut d = String::new();
                    for e in els {
                        let (k, ps) = e.points();
                        let op = match e {
                            PathEl::Move(_) => "M",
                            PathEl::Line(_) => "L",
                            PathEl::Quad(..) => "Q",
                            PathEl::Cubic(..) => "C",
                            PathEl::Close => "Z",
                        };
                        d.push_str(op);
                        for p in &ps[..k] {
                            let _ = write!(d, "{:.2} {:.2} ", p.x, p.y);
                        }
                    }
                    let paint = |c: Color| if c.3 == 0 { "none".to_string() } else { Self::colour(c) };
                    let _ = write!(
                        body,
                        r#"<path d="{}" fill="{}" fill-rule="nonzero" stroke="{}" stroke-width="{width:.2}" stroke-linejoin="round" stroke-linecap="round"/>"#,
                        d.trim_end(),
                        paint(*fill),
                        paint(*stroke)
                    );
                }
                Cmd::PushClip { rect } => self.open_clip(&mut body, *rect),
                Cmd::PopClip => self.close_clip(&mut body),
                // **A shader bracket is a `clipPath` group** (M83/T83.3):
                // SVG runs no WGSL, so the document carries the fallback
                // recorded inside the bracket, clipped to the shader's rect,
                // as the software canvas draws it.
                #[cfg(lumen_shader)]
                Cmd::Shader { rect, .. } => self.open_clip(&mut body, *rect),
                #[cfg(lumen_shader)]
                Cmd::ShaderEnd => self.close_clip(&mut body),
            }
        }
        // An unbalanced list must still produce a parseable document —
        // `tests/paint_containment.rs` forbids one, but a renderer must
        // not trust that, exactly as the canvas does not.
        for _ in 0..self.clips {
            body.push_str("</g>");
        }
        self.clips = 0;

        self.out = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}"><defs>{}</defs>{body}</svg>"#,
            self.defs
        );
    }
}
