//! M46/T46.0 item 1, R2-F2: a label changed to a string whose text hash
//! collides with the old one still reports damage, and a partial redraw
//! still matches a full one.
//!
//! `damage_oracle.rs` is the same comparison, but its inputs are integer
//! counts, and no two short decimal strings collide under any fold worth
//! the name. Retention compares a text command by a 64-bit hash of its
//! bytes (`retain.rs`, "Equality becomes a 64-bit hash"), so a pair that
//! hashes equal is a label whose change is invisible to it: no damage,
//! and the old pixels stay on screen.
//!
//! Three transitions, all from [`A`], through the default
//! `Host<_, Incremental>` with a bound `Label<String>`:
//!
//! - **[`CONTROL`]**: same length, not colliding. Must give a damage
//!   region and zero stale pixels, before and after any fix — the gate
//!   accepting good input.
//! - **[`SOLVED`]**: same length, solved (by lattice reduction) to be
//!   equal under the linear `h * K + b` fold `display::fold` is. It is
//!   the one input guaranteed to exercise the defect in that fold, and it
//!   would pass for any other function, which is why it is not alone.
//! - **[`NUL_PREFIXED`]**: `"\0"` in front of a one-byte string. Under a
//!   fold that starts from 0 and folds no length a leading zero byte
//!   changes nothing, so any function seeded with the length must
//!   separate the two *for a structural reason*. Retention's fold starts
//!   from the command's rect rather than 0, so this pair separates
//!   there today (see the test). The NUL is measured
//!   with no advance, and the gate checks that the label's rect did not
//!   move, since a rect change would be damage whatever the text hash
//!   did.
//!
//! Each step asserts that it landed (the label holds the new text and
//! the list is not empty), so a frame in which nothing happened cannot
//! pass. The stale count is printed beside its bound.

use lumen_static::retain::Damage;
use lumen_static::cx::{BlockMetrics, TextMeasure};
use lumen_static::{App as DeriveApp, Cmd, Color, Incremental, Size, StaticTree};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::Host;
use lumen_static_widgets::ops::Label;

const W: u32 = 200;
const H: u32 = 60;
const BG: Color = Color(255, 255, 255, 255);

/// 12 bytes. [`SOLVED`] differs from it by
/// `d = [4,-10,3,-8,-7,4,13,-11,-18,-8,-14,8]`, and
/// `Σ dᵢ·K^(11-i) ≡ 0 (mod 2⁶⁴)` for `display::fold`'s `K` (M38/R2, probe
/// ae9125b).
const A: &str = " k ih  lsio ";
const SOLVED: &str = "$a#aa$-aaaa(";
/// Same length as [`A`], last byte one off [`SOLVED`]'s: not colliding.
const CONTROL: &str = "$a#aa$-aaaa)";

/// The one-byte string [`NUL_PREFIXED`] extends.
const ONE: &str = "x";
/// `"\0x"`. Measured by [`NulFree`], a NUL has no advance, so the label's
/// rect is the same as `"x"`'s: the text hash is the only thing that can
/// tell the two commands apart, which is the point.
const NUL_PREFIXED: &str = "\u{0}x";

/// `BlockMetrics` with a zero-width NUL — identical to it for every other
/// string in this file, which carries no NUL.
struct NulFree;
impl TextMeasure for NulFree {
    fn measure(&mut self, text: &str, size: f32, max_w: f32) -> Size {
        let visible: String = text.chars().filter(|&c| c != '\0').collect();
        BlockMetrics.measure(&visible, size, max_w)
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("v")]
    v: Label<String>,
}

#[derive(DeriveApp, Default)]
struct Probe {
    #[tree]
    ui: Ui,
    #[bind(ui.v, "{}")]
    text: String,
}

struct Step {
    landed: bool,
    damage: String,
    damage_regions: usize,
    stale_pixels: usize,
    rect_before: Option<[f32; 4]>,
    rect_after: Option<[f32; 4]>,
}

fn text_rect(host: &Host<Probe, Incremental>) -> Option<[f32; 4]> {
    host.list.cmds.iter().find_map(|c| match c {
        Cmd::Text { rect, .. } => Some(*rect),
        _ => None,
    })
}

fn step(from: &str, to: &str) -> Step {
    let app = Probe { text: from.into(), ..Probe::default() };
    let mut host: Host<Probe, Incremental> = Host::new(app, W, H);
    host.pump_with(&mut NulFree);
    let rect_before = text_rect(&host);
    let mut oracle = Canvas::new(W, H);
    let mut inc = Canvas::new(W, H);
    oracle.clear(BG);
    host.render(&mut oracle);
    inc.clear(BG);
    host.render(&mut inc);
    let _ = host.take_damage_for_age(1);

    host.app.text = to.into();
    host.pump_with(&mut NulFree);
    let landed = host.app.ui.v.text.as_str() == to && !host.list.cmds.is_empty();
    let rect_after = text_rect(&host);
    oracle.clear(BG);
    host.render(&mut oracle);
    let dmg = host.take_damage_for_age(1);
    let mut damage_regions = 0;
    match &dmg {
        Damage::None => {}
        Damage::All => {
            damage_regions = 1;
            inc.clear(BG);
            host.render(&mut inc);
        }
        d @ Damage::Regions(_) => {
            for r in d.regions([0.0, 0.0, W as f32, H as f32]) {
                damage_regions += 1;
                inc.clear_within(r, BG);
                inc.render_within(&host.list, r);
            }
        }
    }
    let stale_pixels = oracle.pixels.iter().zip(inc.pixels.iter()).filter(|(a, b)| a != b).count();
    Step { landed, damage: format!("{dmg:?}"), damage_regions, stale_pixels, rect_before, rect_after }
}

fn check(from: &str, to: &str, needs_ink: bool) {
    let s = step(from, to);
    println!(
        "{from:?} -> {to:?}: landed={} damage={} regions={} stale_pixels {} / bound 0 \
         (text rect {:?} -> {:?})",
        s.landed, s.damage, s.damage_regions, s.stale_pixels, s.rect_before, s.rect_after
    );
    assert!(s.landed, "setup: the label must hold {to:?} and the list must be non-empty");
    assert!(s.rect_before.is_some(), "setup: the frame must carry a text command");
    assert_eq!(
        s.rect_before, s.rect_after,
        "setup: the text rect moved, so damage would come from the rect and not the text hash"
    );
    assert!(s.damage_regions > 0, "{from:?} -> {to:?}: the text changed and retention reported no damage");
    if needs_ink {
        assert_eq!(s.stale_pixels, 0, "{from:?} -> {to:?}: a partial redraw differs from a full one");
    }
}

/// Accepts good input: a non-colliding change of the same length.
#[test]
fn a_non_colliding_change_damages_and_leaves_nothing_stale() {
    check(A, CONTROL, true);
}

#[test]
fn a_solved_colliding_change_still_damages() {
    check(A, SOLVED, true);
}

/// The pair a length seed separates. The claim is damage only: the NUL
/// has no advance, so which pixels a NUL paints is the rasteriser's
/// business, not the hash's.
///
/// **Green today, and not by luck of the input.** `FoldedText` starts its
/// fold from 0, where a leading zero byte is invisible; retention starts
/// from the command's tag and rect, where `h·K + 0 ≠ h`. So this pair is
/// not a collision in retention at HEAD (T46.0) and stays here as the
/// guard that a new function keeps separating it.
#[test]
fn a_nul_prefixed_change_still_damages() {
    check(ONE, NUL_PREFIXED, false);
}
