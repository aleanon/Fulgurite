//! **Every command states what it is as a ground** — M83/T83.0
//! (`docs/plan-gpu-extension.md`), a gate that compiles today (S3).
//!
//! `contrast::text_contrast` names a run's ground by asking each command
//! beneath it what it painted at the run's centre, and that question ends
//! in a wildcard: a command it has no arm for answers "nothing here",
//! and the walk measures the run against whatever lies under it. So a new
//! `Cmd` variant is silently judged against the wrong colour, and nothing
//! fails to compile.
//!
//! This test puts one of each command under a text run's centre and
//! asserts the ground the report names. Its table is an exhaustive match
//! with no wildcard, so a new variant does not compile here until it
//! states its row: M83/T83.3 adds the shader bracket's two variants under
//! `cfg(lumen_shader)`. The rows are:
//!
//! - **opaque**: the run is measured against the command's colour;
//! - **sampled**: a gradient, measured against its colour at the run;
//! - **unknown**: something is there and it is not one colour, so the run
//!   is unmeasured, with the reason;
//! - **miss**: the command fills nothing (`Text`, `Line`, `StrokedRect`,
//!   the clips), and the run is measured against the page beneath it.

use lumen_static::desc::Point;
use lumen_static::display::{Cmd, Color, DisplayList, GradShape, ImageId, Material, PathEl, PathPaint, Stops, AFFINE_IDENTITY};
use lumen_static_shell::contrast::text_contrast;

/// Distinct from every colour below, so a run measured against the page
/// is told apart from one measured against a fill.
const PAGE: Color = Color(200, 180, 160, 255);
const INK: Color = Color(0, 0, 0, 255);
/// Where every command is painted; the probe run's centre is its middle.
const AREA: [f32; 4] = [0.0, 0.0, 100.0, 100.0];
const PROBE: [f32; 4] = [40.0, 40.0, 60.0, 60.0];

/// The number of `Cmd` variants this test builds. A new variant must be
/// added to [`samples`] as well as to [`row`]; `row` alone compiles only
/// because a variant nobody builds never reaches it, so this count is
/// asserted against the distinct kinds the samples produce.
const KINDS: usize = 12;

#[derive(Debug, PartialEq)]
enum Want {
    /// Measured against this colour (a fill, or a gradient's colour at
    /// the run).
    Ground(Color),
    /// Unmeasured, for this reason.
    Unknown(&'static str),
    /// Fills nothing: measured against the page beneath it.
    Miss,
}

/// Each command's name and the ground it must report. **Exhaustive, no
/// wildcard**: this match is the reason the test exists.
fn row(c: &Cmd) -> (&'static str, Want) {
    match c {
        Cmd::Rect { color, .. } => ("Rect", Want::Ground(*color)),
        Cmd::Text { .. } => ("Text", Want::Miss),
        Cmd::RoundedRect { color, .. } => ("RoundedRect", Want::Ground(*color)),
        Cmd::StrokedRect { .. } => ("StrokedRect", Want::Miss),
        Cmd::Shadow { .. } => ("Shadow", Want::Unknown("shadow")),
        Cmd::Line { .. } => ("Line", Want::Miss),
        // Both stops one colour, so every sample is that colour.
        Cmd::Gradient { stops, .. } => ("Gradient", Want::Ground(stops.sample(0.5))),
        Cmd::Image { .. } => ("Image", Want::Unknown("image")),
        Cmd::Glass { .. } => ("Glass", Want::Unknown("glass")),
        Cmd::PushClip { .. } => ("PushClip", Want::Miss),
        Cmd::PopClip => ("PopClip", Want::Miss),
        Cmd::Path { .. } => ("Path", Want::Unknown("path")),
    }
}

/// One painter per kind, each painting over [`AREA`]. `PopClip` is
/// preceded by the `PushClip` it closes; the command under test is always
/// the last one before the probe.
fn samples() -> Vec<fn(&mut DisplayList)> {
    vec![
        |l| l.rect(AREA, Color(255, 255, 255, 255)),
        |l| l.text(AREA, "under", 14.0, Color(30, 30, 200, 255)),
        |l| l.rounded_rect(AREA, Color(20, 60, 140, 255), 8.0),
        |l| l.stroked_rect(AREA, Color(10, 10, 10, 255), 4.0, 2.0),
        |l| l.shadow(AREA, 8.0, 6.0),
        |l| l.line(Point { x: 0.0, y: 50.0 }, Point { x: 100.0, y: 50.0 }, 4.0, Color(10, 10, 10, 255)),
        |l| {
            let c = Color(10, 120, 10, 255);
            l.gradient(AREA, Stops::two(c, c), GradShape::Linear { angle: 0.0 }, 0.0)
        },
        |l| l.image(AREA, ImageId(0), Color(255, 255, 255, 255)),
        |l| l.glass(AREA, 12.0, Material::default()),
        |l| l.push_clip(AREA),
        |l| {
            l.push_clip(AREA);
            l.pop_clip()
        },
        |l| {
            let p = |x, y| Point { x, y };
            let square = [
                PathEl::Move(p(0.0, 0.0)),
                PathEl::Line(p(100.0, 0.0)),
                PathEl::Line(p(100.0, 100.0)),
                PathEl::Line(p(0.0, 100.0)),
                PathEl::Close,
            ];
            let paint = PathPaint { fill: Color(140, 20, 20, 255), stroke: Color(0, 0, 0, 0), width: 0.0 };
            l.path(&square, AFFINE_IDENTITY, paint)
        },
    ]
}

#[test]
fn contrast_names_every_command() {
    let mut names: Vec<&'static str> = Vec::new();
    let (mut measured, mut unmeasured) = (0, 0);
    let mut wrong: Vec<String> = Vec::new();

    for paint in samples() {
        let mut list = DisplayList::new();
        paint(&mut list);
        let under = list.cmds.len().checked_sub(1).expect("the sample painted nothing");
        list.text(PROBE, "probe", 14.0, INK);
        let probe = list.cmds.len() - 1;
        assert!(matches!(list.cmds[probe], Cmd::Text { .. }), "the probe is the last command");

        let (name, want) = row(&list.cmds[under]);
        assert!(!names.contains(&name), "{name} sampled twice");
        names.push(name);

        let r = text_contrast(&list, PAGE);
        assert_eq!(r.clipped, 0, "{name}: the probe's centre is inside every clip; it must be seen");
        let got = if let Some(run) = r.measured.iter().find(|m| m.index == probe) {
            measured += 1;
            if run.ground == PAGE {
                Want::Miss
            } else {
                Want::Ground(run.ground)
            }
        } else if let Some((_, _, why)) = r.unmeasured.iter().find(|u| u.0 == probe) {
            unmeasured += 1;
            Want::Unknown(why)
        } else {
            panic!("{name}: the probe run is in neither measured nor unmeasured");
        };
        println!("{name:<12} want {want:?}, got {got:?}");
        if got != want {
            wrong.push(format!("{name}: want {want:?}, got {got:?}"));
        }
    }

    // The setup landed: one probe reported per kind, and every kind built.
    println!(
        "kinds {} built of {KINDS}; probes reported {} (measured {measured}, unmeasured {unmeasured})",
        names.len(),
        measured + unmeasured
    );
    assert_eq!(names.len(), KINDS, "a Cmd kind has a row and no sample (or KINDS is stale): {names:?}");
    assert_eq!(measured + unmeasured, KINDS, "every probe must be reported, measured or unmeasured");
    assert!(wrong.is_empty(), "a command reports the wrong ground: {wrong:?}");
}
