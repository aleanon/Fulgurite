//! **M12/T12.2: golden images, with a diff you can look at.**
//!
//! The *comparison* already existed — `tests/damage_oracle.rs` compares
//! whole pixel buffers. What it did on a mismatch was `assert_eq!` two
//! 264 KB `Vec<u8>`s: no count of differing pixels, let alone a picture.
//! What was missing is the harness around it — storing a reference,
//! updating it deliberately, and producing something to look at when it
//! fails.
//!
//! # Three rules, written down with it
//!
//! **Goldens are for the software renderer only**, and it is
//! deterministic: pure arithmetic plus `sqrt`, which IEEE-754 requires
//! correctly rounded, and no GPU path to diverge from.
//!
//! **Deterministic is not portable, and the difference is the whole
//! trap.** Under the `truetype` feature — what a window and the live
//! gate actually run — `FontStack::system` takes *the first of five
//! candidate faces that exists on the machine*, silently serves both
//! weights from the regular face when no bold is found, and depends on
//! a caret-ranged `fontdue`. A golden recorded that way records which
//! fonts the recording machine had installed. So goldens are recorded
//! and compared with `truetype` **off**, and [`compare`](crate::golden::compare) refuses either
//! with it on — otherwise `LUMEN_UPDATE_GOLDENS=1` becomes a button
//! that bakes in the author's font install.
//!
//! **A golden is not a substitute for a property.**
//! `tests/paint_containment.rs` and `damage_oracle.rs` catch classes of
//! bug across a corpus; a golden catches one frame changing. The bugs
//! this project actually shipped — a repeater painting 44 px outside
//! its viewport, a renderer ignoring clips, a skipped present — were
//! caught by properties and by the live gate, and would have been
//! *masked* by a re-recorded golden.
//!
//! # Why PPM
//!
//! [`Canvas::to_ppm`](crate::software::Canvas::to_ppm) already writes
//! it and it needs no dependency at all. Not the PNG encoder in
//! `lumen-static-agent`: that crate depends on `lumen-static` with
//! `features = ["observe"]`, not optional, and Cargo unifies features
//! across a selection — so a dev-dependency on it from
//! `lumen-static-shell/tests` would switch `observe` on for
//! `alloc_gate.rs`'s process and grow `NodeDesc` from 48 B to 72 B.
//! That is the same unification hazard M13's feature polarity is argued
//! from.

use std::path::{Path, PathBuf};

use crate::software::Canvas;

/// What a comparison found.
#[derive(Debug)]
pub enum Verdict {
    /// Byte-identical to the stored reference.
    Match,
    /// No reference existed, and one was written. A first run is not a
    /// pass: it is a recording, and it says so.
    Recorded(PathBuf),
    /// Re-recorded because `LUMEN_UPDATE_GOLDENS=1`.
    Updated { path: PathBuf, differing: usize },
    /// Different, with artifacts written beside the reference.
    Differs {
        /// How many pixels differ, of how many.
        differing: usize,
        total: usize,
        /// The bounding box of the difference, `[x0, y0, x1, y1]` —
        /// **the part a number cannot tell you**. "1,482 pixels differ"
        /// is a number; "they are all in the button's top-left corner"
        /// is a diagnosis.
        region: [u32; 4],
        actual: PathBuf,
        diff: PathBuf,
    },
}

impl Verdict {
    /// Whether a test should pass on this.
    ///
    /// `Recorded` is **not** a pass. A first run that silently went
    /// green would make an unreviewed reference indistinguishable from
    /// a checked one, which is how a golden suite stops meaning
    /// anything.
    pub fn is_match(&self) -> bool {
        matches!(self, Verdict::Match)
    }
}

impl core::fmt::Display for Verdict {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Verdict::Match => write!(f, "matches"),
            Verdict::Recorded(p) => {
                write!(f, "no reference existed; recorded {}. Review it and re-run", p.display())
            }
            Verdict::Updated { path, differing } => {
                write!(f, "re-recorded {} ({differing} px changed)", path.display())
            }
            Verdict::Differs { differing, total, region, actual, diff } => write!(
                f,
                "{differing} of {total} pixels differ, inside [{}, {}, {}, {}] \
                 — see {} and {}",
                region[0],
                region[1],
                region[2],
                region[3],
                actual.display(),
                diff.display()
            ),
        }
    }
}

/// Compare `canvas` against `tests/golden/<name>.ppm` under `dir`.
///
/// Set `LUMEN_UPDATE_GOLDENS=1` to re-record instead of comparing.
///
/// # Panics
///
/// With the `truetype` feature on — see the module docs. This is a
/// panic rather than a silent skip because a golden suite that quietly
/// did nothing under the feature a window uses is worse than one that
/// refuses.
pub fn compare(dir: &Path, name: &str, canvas: &Canvas) -> std::io::Result<Verdict> {
    #[cfg(feature = "truetype")]
    {
        let _ = (dir, canvas);
        panic!(
            "golden `{name}`: refusing to record or compare with the `truetype` feature \
             on. `FontStack::system` picks whichever of five candidate faces the machine \
             happens to have, so a golden recorded here would record this machine's font \
             install."
        );
    }

    #[cfg(not(feature = "truetype"))]
    {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(format!("{name}.ppm"));
        let got = canvas.to_ppm();

        let updating = std::env::var("LUMEN_UPDATE_GOLDENS").is_ok_and(|v| v == "1");
        let stored = std::fs::read(&path).ok();

        match (stored, updating) {
            (None, _) => {
                std::fs::write(&path, &got)?;
                Ok(Verdict::Recorded(path))
            }
            (Some(want), true) => {
                let (differing, _) = diff_stats(&want, &got);
                std::fs::write(&path, &got)?;
                Ok(Verdict::Updated { path, differing })
            }
            (Some(want), false) if want == got => Ok(Verdict::Match),
            (Some(want), false) => {
                let (differing, region) = diff_stats(&want, &got);
                let total = (canvas.width * canvas.height) as usize;
                let actual = dir.join(format!("{name}.actual.ppm"));
                let diff = dir.join(format!("{name}.diff.ppm"));
                std::fs::write(&actual, &got)?;
                std::fs::write(&diff, diff_image(&want, &got))?;
                Ok(Verdict::Differs { differing, total, region, actual, diff })
            }
        }
    }
}

#[cfg(not(feature = "truetype"))]
/// Where the header ends and pixels begin in a P6 PPM.
///
/// Three newline-terminated fields: magic, dimensions, maxval. Written
/// by `Canvas::to_ppm`, so the shape is known rather than guessed —
/// this is not a general PPM parser and does not pretend to be.
fn body(ppm: &[u8]) -> &[u8] {
    let mut seen = 0;
    for (i, b) in ppm.iter().enumerate() {
        if *b == b'\n' {
            seen += 1;
            if seen == 3 {
                return &ppm[i + 1..];
            }
        }
    }
    &[]
}

#[cfg(not(feature = "truetype"))]
/// Differing pixel count, and the bounding box they fall in.
fn diff_stats(want: &[u8], got: &[u8]) -> (usize, [u32; 4]) {
    let (a, b) = (body(want), body(got));
    // A size change is a total difference: comparing the overlap would
    // report a small number for a frame that is entirely the wrong
    // shape.
    if a.len() != b.len() {
        return (a.len().max(b.len()) / 3, [0, 0, 0, 0]);
    }
    let w = width_of(want).max(1);
    let (mut n, mut x0, mut y0, mut x1, mut y1) = (0usize, u32::MAX, u32::MAX, 0u32, 0u32);
    for (i, (pa, pb)) in a.chunks_exact(3).zip(b.chunks_exact(3)).enumerate() {
        if pa != pb {
            n += 1;
            let (x, y) = (i as u32 % w, i as u32 / w);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x + 1);
            y1 = y1.max(y + 1);
        }
    }
    if n == 0 {
        (0, [0, 0, 0, 0])
    } else {
        (n, [x0, y0, x1, y1])
    }
}

#[cfg(not(feature = "truetype"))]
fn width_of(ppm: &[u8]) -> u32 {
    let text = String::from_utf8_lossy(&ppm[..ppm.len().min(64)]);
    text.lines()
        .nth(1)
        .and_then(|l| l.split_whitespace().next())
        .and_then(|w| w.parse().ok())
        .unwrap_or(1)
}

#[cfg(not(feature = "truetype"))]
/// The reference, dimmed, with every differing pixel in red.
///
/// **This is the part that makes a golden usable.** A bare "1,482
/// pixels differ" is a number, not a diagnosis; the picture says *where*
/// — and the dimmed base is what makes "where" mean something, because
/// red on black tells you nothing about which control moved.
fn diff_image(want: &[u8], got: &[u8]) -> Vec<u8> {
    let (a, b) = (body(want), body(got));
    let header_end = want.len() - a.len();
    let mut out = want[..header_end].to_vec();
    for (pa, pb) in a.chunks_exact(3).zip(b.chunks_exact(3)) {
        if pa == pb {
            out.extend_from_slice(&[pa[0] / 4, pa[1] / 4, pa[2] / 4]);
        } else {
            out.extend_from_slice(&[255, 0, 0]);
        }
    }
    // A size mismatch leaves the tail unpaired; mark it all.
    for _ in a.chunks_exact(3).len()..b.chunks_exact(3).len() {
        out.extend_from_slice(&[255, 0, 0]);
    }
    out
}
