//! M26/T26.17 Phase D #12: a label's measurement memo must miss when the
//! **display scale** changes, not only when the text, limits, size or
//! weight do.
//!
//! [`MeasureKey`] carries the constraints, the font size and the weight
//! (M26/T26.3). It does not carry the scale — and the measurer's answer
//! depends on it. `FontMetrics` says why in its own rustdoc: advances
//! are *not* linear in size, so it measures at `size * scale` and
//! divides back down, and 16 px doubled is not 32 px.
//!
//! **The ordering that reaches it is the ordinary one**, which is why
//! the review could only call this plausible: a scale change alone moves
//! the logical viewport, so the limits move with it and the key changes.
//! `winit`'s `ScaleFactorChanged` delivers a new scale *and* a new
//! physical size together, and the normal case — dragging a window from
//! a 1x monitor to a 2x one — keeps the **logical** size constant while
//! doubling the physical one. Limits unchanged, scale doubled, memo hit,
//! stale box.
//!
//! The failure is the one `docs/results-frame-cost.md` already records
//! under another cause: text laid out to a box measured for a different
//! rasterisation drifts out of it, and "POSITIVE" renders as "POSITIV".

use lumen_static_widgets::ops::Label;
use lumen_static::{Constraints, LayoutCx, Node, NodeState, Style};

/// The non-linear measurer, shared with the shell's `scale_memo.rs`,
/// which drives the same memo through a `Host` (M40/T40.0).
#[path = "support/hinted.rs"]
mod hinted;
use hinted::Hinted;

const LIMITS: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 400.0 };

/// One layout at the measurer's current scale.
///
/// The two scales move together, exactly as they do in the shell: the
/// winit layer calls `FontMetrics::set_scale` and `Host::set_scale` from
/// the same `ScaleFactorChanged`, and `Host::pump_with` hands its own
/// scale to the context. A test that moved only one of them would be
/// testing a configuration no host produces.
fn width(l: &mut Label<String>, m: &mut Hinted) -> f32 {
    let scale = m.scale;
    let mut cx = LayoutCx::new(m).with_scale(scale);
    let (sty, mut st) = ([Style::default()], [NodeState::default()]);
    l.layout(&mut cx, LIMITS, &sty, &mut st, 0).w
}

#[test]
fn a_scale_change_under_the_same_limits_re_measures() {
    let mut m = Hinted::at(1.0);
    let mut l: Label<String> = Label::new("POSITIVE".into());

    let at_1x = width(&mut l, &mut m);
    let calls_1x = m.calls.get();
    assert_eq!(calls_1x, 1, "setup: the first layout measured");

    // The ScaleFactorChanged ordering: the scale doubles, the *logical*
    // limits do not move, because the physical surface doubled with it.
    m.scale = 2.0;
    let at_2x = width(&mut l, &mut m);

    println!(
        "1x: {at_1x}; 2x under identical limits: {at_2x} (measurer calls {} -> {})",
        calls_1x,
        m.calls.get()
    );
    assert_ne!(
        at_1x, at_2x,
        "the memo served the 1x box at 2x: MeasureKey does not carry the scale, so a \
         ScaleFactorChanged that keeps the logical size constant hits the cache"
    );
    assert_eq!(m.calls.get(), 2, "the scale change must re-measure, not re-use");
}

/// The other direction, which is what keeps this from becoming a memo
/// that never hits: an unchanged scale must still be served from cache.
#[test]
fn an_unchanged_scale_still_hits_the_memo() {
    let mut m = Hinted::at(2.0);
    let mut l: Label<String> = Label::new("POSITIVE".into());

    let first = width(&mut l, &mut m);
    let second = width(&mut l, &mut m);
    println!("same scale twice: {first} then {second}, measurer calls {}", m.calls.get());
    assert_eq!(first, second);
    assert_eq!(m.calls.get(), 1, "an unchanged scale must not re-measure");
}
