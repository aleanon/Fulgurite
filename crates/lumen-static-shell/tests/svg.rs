//! T15.12: that `Renderer` admits a second implementation.
//!
//! The architectural claim M10a made and nothing had tested from the
//! other side: a renderer is a consumer of the command list, not the
//! definition of one. This is the first non-rasterising one, which is a
//! stronger test than a second rasteriser would have been.
//!
//! Since M51/T51.6 the vocabulary test is exhaustive **by construction**:
//! `kind` matches every `Cmd` variant with no wildcard, so a new command
//! does not compile here until it is placed, and each command must emit
//! its own element.

use lumen_static::display::{Cmd, Color, GradKind, GradShape, ImageId, Material, Renderer, Stops};
use lumen_static::{DisplayList, Point};
use lumen_static_shell::svg::Svg;

const RED: Color = Color(255, 0, 0, 255);

fn render(f: impl FnOnce(&mut DisplayList)) -> String {
    let mut l = DisplayList::new();
    f(&mut l);
    let mut s = Svg::new();
    s.render(&l, 100, 50);
    s.finish()
}

#[test]
fn it_produces_a_document_with_the_right_viewport() {
    let out = render(|l| l.rect([0.0, 0.0, 10.0, 10.0], RED));
    assert!(out.starts_with("<svg xmlns="));
    assert!(out.contains(r#"viewBox="0 0 100 50""#));
    assert!(out.ends_with("</svg>"));
}

/// How many `Cmd` variants there are. Bumped by hand, which is the point:
/// a new variant does not compile in [`kind`] until it has an index, and
/// once it has one [`the_whole_command_vocabulary_renders`] refuses to
/// pass until [`one_of_each`] draws it.
#[cfg(not(lumen_shader))]
const KINDS: usize = 12;
/// M83/T83.3: the shader bracket's two variants, in a `lumen_shader` build.
#[cfg(lumen_shader)]
const KINDS: usize = 14;

/// A stable index per command kind, from an **exhaustive** match with no
/// wildcard arm — M51/T51.6. "The whole vocabulary" is made true by the
/// compiler rather than by a sentence that goes stale: the first version
/// of this test drew seven kinds and said "every", and missed the border
/// and glass.
fn kind(c: &Cmd) -> (usize, &'static str) {
    match c {
        Cmd::Rect { .. } => (0, "Rect"),
        Cmd::RoundedRect { .. } => (1, "RoundedRect"),
        Cmd::StrokedRect { .. } => (2, "StrokedRect"),
        Cmd::Shadow { .. } => (3, "Shadow"),
        Cmd::Text { .. } => (4, "Text"),
        Cmd::Line { .. } => (5, "Line"),
        Cmd::Gradient { .. } => (6, "Gradient"),
        Cmd::Image { .. } => (7, "Image"),
        Cmd::Glass { .. } => (8, "Glass"),
        Cmd::PushClip { .. } => (9, "PushClip"),
        Cmd::PopClip => (10, "PopClip"),
        Cmd::Path { .. } => (11, "Path"),
        #[cfg(lumen_shader)]
        Cmd::Shader { .. } => (12, "Shader"),
        #[cfg(lumen_shader)]
        Cmd::ShaderEnd => (13, "ShaderEnd"),
    }
}

/// One of every command: the list to render, the index of the command
/// being judged in it, and the element its SVG must contain.
///
/// `PopClip` is judged in context: a pop with nothing open emits nothing
/// by design, so its sample opens a clip first and its evidence is that
/// the group closes *before* the rect drawn after it — a pop that
/// emitted nothing would leave the rect inside the group, closed only at
/// the end of the document.
#[cfg(lumen_shader)]
fn shader(rect: [f32; 4]) -> Cmd {
    Cmd::Shader { rect, id: lumen_static::display::ShaderId(0), uniforms: (0, 0) }
}

fn one_of_each() -> Vec<(Vec<Cmd>, usize, &'static str)> {
    let r = [0.0, 0.0, 10.0, 10.0];
    let clip = Cmd::PushClip { rect: r };
    vec![
        (vec![Cmd::Rect { rect: r, color: RED }], 0, r#"<rect x="0.00" y="0.00" width="10.00" height="10.00" fill="#),
        (vec![Cmd::RoundedRect { rect: r, color: RED, radius: 4.0 }], 0, r#"rx="4.00" fill="#),
        (vec![Cmd::StrokedRect { rect: r, color: RED, radius: 4.0, width: 2.0 }], 0, r#"stroke-width="2.00""#),
        (vec![Cmd::Shadow { rect: r, color: RED, radius: 4.0, spread: 3.0 }], 0, r#"opacity="0.35""#),
        (vec![Cmd::Text { rect: [0.0, 0.0, 50.0, 16.0], text: (0, 2), size: 12.0, weight: 400.0, line_height: 0, color: RED }], 0, ">hi</text>"),
        (vec![Cmd::Line { a: Point { x: 0.0, y: 0.0 }, b: Point { x: 9.0, y: 9.0 }, width: 2.0, color: RED }], 0, "<line"),
        (vec![Cmd::Gradient { rect: r, stops: Stops::two(RED, Color(0, 0, 255, 255)), kind: GradKind::Linear, geom: [0.0; 3], corner: 0.0 }], 0, "<linearGradient"),
        (vec![Cmd::Image { rect: r, id: ImageId(3), tint: RED, frame: 2 }], 0, r#"data-image="3" data-frame="2""#),
        (vec![Cmd::Glass { rect: r, radius: 4.0, material: Material::FROSTED }], 0, r#"data-blur="12.00""#),
        (vec![clip], 0, r#"<g clip-path="url(#c0)">"#),
        (vec![clip, Cmd::PopClip, Cmd::Rect { rect: r, color: RED }], 1, r#"</g><rect"#),
        // M82/T82.5: its two elements are the `Move` and `Line` the test
        // puts in the list's store.
        (
            vec![Cmd::Path { rect: r, els: (0, 2), fill: RED, stroke: Color(0, 0, 0, 0), width: 0.0 }],
            0,
            r#"<path d="M0.00 0.00 L9.00 9.00" fill="rgba(255,0,0,1.000)" fill-rule="nonzero" stroke="none""#,
        ),
        // M83/T83.3: a bracket is a `clipPath` group around its fallback,
        // and its end closes the group, judged in context like `PopClip`.
        #[cfg(lumen_shader)]
        (vec![shader(r)], 0, r#"<g clip-path="url(#c0)">"#),
        #[cfg(lumen_shader)]
        (vec![shader(r), Cmd::ShaderEnd, Cmd::Rect { rect: r, color: RED }], 1, r#"</g><rect"#),
    ]
}

/// **Every command in the vocabulary emits its element** — M51/T51.6
/// (R9-F10).
///
/// The test this replaces drew seven kinds and matched six substrings
/// against the whole document; it had no border and no glass, so with
/// both SVG arms emitting nothing (scratch commit c75d6c4) the suite was
/// green. Each command is now rendered on its own and must produce its
/// own element, and the kinds drawn must be all of them.
#[test]
fn the_whole_command_vocabulary_renders() {
    let mut covered = [false; KINDS];
    let mut missing = Vec::new();
    for (cmds, judged, marker) in one_of_each() {
        let (k, name) = kind(&cmds[judged]);
        covered[k] = true;
        let mut l = DisplayList::new();
        l.text.push_str("hi");
        l.paths.extend([
            lumen_static::display::PathEl::Move(Point { x: 0.0, y: 0.0 }),
            lumen_static::display::PathEl::Line(Point { x: 9.0, y: 9.0 }),
        ]);
        l.cmds.extend(cmds.iter().copied());
        let mut s = Svg::new();
        s.render(&l, 100, 50);
        let out = s.finish();
        if !out.contains(marker) {
            missing.push(name);
            println!("{name}: {marker} missing from: {out}");
        }
    }
    let undrawn: Vec<usize> = (0..KINDS).filter(|k| !covered[*k]).collect();
    println!("{KINDS} command kinds, {} drawn; missing elements: {missing:?}", KINDS - undrawn.len());
    assert!(undrawn.is_empty(), "command kinds with no sample in one_of_each: {undrawn:?}");
    assert!(missing.is_empty(), "commands whose SVG element is missing: {missing:?}");
}

/// The failure that shows up in someone else's tool, days later.
#[test]
fn text_is_xml_escaped() {
    let out = render(|l| l.text([0.0, 0.0, 50.0, 16.0], "a < b & \"c\"", 12.0, RED));
    assert!(out.contains("a &lt; b &amp; &quot;c&quot;"), "{out}");
    assert!(!out.contains("a < b &"), "a raw ampersand makes the document unparseable");
}

#[test]
fn clips_open_and_close_as_groups() {
    let out = render(|l| {
        l.push_clip([0.0, 0.0, 10.0, 10.0]);
        l.rect([0.0, 0.0, 5.0, 5.0], RED);
        l.pop_clip();
    });
    assert_eq!(out.matches("<g clip-path=").count(), 1);
    assert_eq!(out.matches("</g>").count(), 1);
    assert!(out.contains("<clipPath"));
}

/// A renderer must not trust the list to be balanced, exactly as the
/// canvas does not — an unbalanced one must still parse.
#[test]
fn an_unbalanced_clip_still_closes() {
    let out = render(|l| {
        l.push_clip([0.0, 0.0, 10.0, 10.0]);
        l.rect([0.0, 0.0, 5.0, 5.0], RED);
        // no pop
    });
    assert_eq!(out.matches("<g clip-path=").count(), 1);
    assert_eq!(out.matches("</g>").count(), 1, "closed anyway");
    assert!(out.ends_with("</svg>"));
}

#[test]
fn rendering_twice_does_not_accumulate() {
    // `defs` and the clip depth are per-frame state; a renderer reused
    // across frames must not grow.
    let mut l = DisplayList::new();
    l.gradient([0.0, 0.0, 10.0, 10.0], Stops::two(RED, RED), GradShape::Linear { angle: 0.0 }, 0.0);
    let mut s = Svg::new();
    s.render(&l, 100, 50);
    let first = s.finish();
    let mut s = Svg::new();
    s.render(&l, 100, 50);
    s.render(&l, 100, 50);
    assert_eq!(s.finish().len(), first.len(), "a second frame is the same size");
}

/// M82/T82.1: **a stop's offset is written as a fraction, not its
/// byte.** `Stops::at` is in 1/255 steps; `{:.3}` ignores precision on an
/// integer, so writing the raw byte compiles and emits `offset="51"`.
#[test]
fn a_stop_offset_is_written_as_a_fraction() {
    let stops = Stops {
        at: [0, 51, 255, 0],
        color: [RED, Color(0, 255, 0, 255), Color(0, 0, 255, 255), RED],
        len: 3,
    };
    let out = render(|l| l.gradient([0.0, 0.0, 10.0, 10.0], stops, GradShape::Linear { angle: 0.0 }, 0.0));
    let offsets: Vec<&str> = out.match_indices("offset=\"").map(|(i, m)| {
        let rest = &out[i + m.len()..];
        &rest[..rest.find('"').unwrap()]
    }).collect();
    println!("stop offsets written: {offsets:?} (want the middle one 0.200)");
    assert_eq!(offsets.len(), 3, "setup: three stops must be written");
    assert_eq!(offsets[1], "0.200", "the middle stop at 51/255 was not written as 0.200");
}

/// M82/T82.2: **a radial, rounded gradient writes a `<radialGradient>`
/// in user space and the corner as `rx`.** A radial written as the
/// linear arm would still render something, so the element is asserted
/// by name, with its resolved centre and radius printed.
#[test]
fn a_radial_rounded_gradient_writes_radial_gradient_and_rx() {
    let out = render(|l| {
        l.gradient(
            [0.0, 0.0, 40.0, 20.0],
            Stops::two(RED, Color(0, 0, 255, 255)),
            GradShape::Radial { center: [0.25, 0.5], radius: 1.0 },
            6.0,
        )
    });
    let radial = out.find("<radialGradient").map(|i| &out[i..i + out[i..].find('>').unwrap() + 1]);
    println!("radial: {radial:?}; linear present: {}; rx=\"6.00\" present: {}", out.contains("<linearGradient"), out.contains(r#"rx="6.00""#));
    // Centre (10, 10); the farthest corner is (40, 0) or (40, 20), at
    // sqrt(30² + 10²) = 31.62.
    assert_eq!(
        radial,
        Some(r#"<radialGradient id="g0" gradientUnits="userSpaceOnUse" cx="10.00" cy="10.00" r="31.62">"#),
        "a radial gradient was not written as a <radialGradient> at its resolved centre and radius"
    );
    assert!(!out.contains("<linearGradient"), "a radial also wrote a linear gradient");
    assert!(out.contains(r#"rx="6.00""#), "the gradient's corner was not written as rx");
}
