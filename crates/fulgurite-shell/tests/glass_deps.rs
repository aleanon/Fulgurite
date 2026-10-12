//! **The dependency cap's `Damage::All` escape, fired and checked —
//! M51/T51.3 (R9-F3).**
//!
//! A backdrop-reading command (`Cmd::Glass`) registers the region it
//! reads, and damage that touches that region is dilated to all of it
//! (`Diffing::dilate` in `crates/fulgurite/src/retain.rs`). The table
//! of those regions holds [`MAX_DEPS`] entries. A frame with more
//! panels than that cannot record them all, so `dilate` answers
//! `Damage::All` — correct, and merely slow.
//!
//! Until this file, nothing fired that branch: `dilate`'s rustdoc said
//! there was "no cap and no fallback", and with the branch deleted every
//! suite stayed green, including M34's shadow oracle, which compares
//! strategies rather than a partial render with a full one. The review
//! measured 1,822 px wrong with five frosted panels
//! (`docs/results-deep-review.md`, R9-F3).
//!
//! So: `MAX_DEPS` panels, then `MAX_DEPS + 1`, a bar moving under the
//! last one, and every frame rendered twice — partially over the damage
//! the retention reports, and in full — and compared. The setup is
//! asserted before the pixels: the first arm must go partial (the table
//! held every panel) and the second must go `Damage::All` (it did not).
//!
//! Software only. It runs no wgpu code, so nothing about it depends on
//! the adapter: on this project's llvmpipe-only machine and on a real
//! GPU it proves the same thing, which is the core retention's rule.

use fulgurite::display::{Color, DisplayList, Material, Painter, Renderer};
use fulgurite::retain::{Damage, Incremental, Recording, Retention, MAX_DEPS};
use fulgurite_shell::software::Canvas;

const H: u32 = 120;
const BG: Color = Color(18, 22, 32, 255);

fn width(n: usize) -> u32 {
    20 + n as u32 * 75
}

/// `n` frosted panels in a row over static stripes; the bar under the
/// last panel shifts with `step`.
fn frame(ret: &mut Incremental, n: usize, step: u32) -> DisplayList {
    let mut l = DisplayList::new();
    {
        let mut p = Recording::new(&mut l, ret);
        let p: &mut dyn Painter = &mut p;
        for k in 0..n {
            let x = 10.0 + k as f32 * 75.0;
            p.rect([x, 20.0, x + 60.0, 30.0], Color(200, 60, 60, 255));
            let dx = if k == n - 1 { (step % 5) as f32 * 6.0 } else { 0.0 };
            p.rect([x + dx, 50.0, x + dx + 12.0, 70.0], Color(60, 200, 90, 255));
        }
        for k in 0..n {
            let x = 10.0 + k as f32 * 75.0;
            let m = Material { blur: 6.0, tint: Color(255, 255, 255, 40), ..Material::FROSTED };
            p.glass([x + 2.0, 40.0, x + 58.0, 80.0], 8.0, m);
        }
    }
    l
}

/// Frames that went partial, frames that went `All`, and the worst
/// pixel count by which a partial-over-damage render differed from a
/// full one.
fn run(n: usize) -> (usize, usize, usize) {
    let w = width(n);
    let mut ret = Incremental::default();
    let mut full = Canvas::new(w, H);
    let mut inc = Canvas::new(w, H);
    let l = frame(&mut ret, n, 0);
    let _ = ret.finish();
    let _ = ret.take_damage_for_age(1);
    for c in [&mut full, &mut inc] {
        c.clear(BG);
        c.render(&l, w, H);
    }
    let (mut partial, mut all, mut worst) = (0, 0, 0);
    for step in 1..6 {
        let l = frame(&mut ret, n, step);
        let _ = ret.finish();
        full.clear(BG);
        full.render(&l, w, H);
        match ret.take_damage_for_age(1) {
            Damage::None => {}
            Damage::All => {
                all += 1;
                inc.clear(BG);
                inc.render(&l, w, H);
            }
            d @ Damage::Regions(_) => {
                partial += 1;
                for r in d.regions([0.0, 0.0, w as f32, H as f32]) {
                    inc.clear_within(r, BG);
                    inc.render_within(&l, r);
                }
            }
        }
        let diff = full
            .pixels
            .chunks_exact(4)
            .zip(inc.pixels.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count();
        worst = worst.max(diff);
    }
    (partial, all, worst)
}

/// At the cap, every panel is in the table: frames go partial, and a
/// partial render equals a full one.
#[test]
fn at_max_deps_panels_frames_go_partial_and_match_a_full_render() {
    let n = MAX_DEPS;
    let (partial, all, worst) = run(n);
    println!("{n} panels (MAX_DEPS): {partial} partial, {all} All, worst {worst} px differ (bound 0)");
    assert_eq!((partial, all), (5, 0), "setup: {n} panels must take the partial path every frame");
    assert_eq!(worst, 0, "{worst} px differ between partial and full with {n} panels");
}

/// One past the cap, the table overflows: every frame must take the
/// `Damage::All` escape, and the result must still equal a full render.
/// Deleting the escape leaves these frames partial over a table missing
/// a panel, and the pixels under that panel go stale.
#[test]
fn past_max_deps_panels_frames_fall_back_to_all_and_stay_correct() {
    let n = MAX_DEPS + 1;
    let (partial, all, worst) = run(n);
    println!("{n} panels (MAX_DEPS + 1): {partial} partial, {all} All, worst {worst} px differ (bound 0)");
    assert_eq!(worst, 0, "{worst} px differ between partial and full with {n} panels");
    assert_eq!((partial, all), (0, 5), "setup: {n} panels must take the Damage::All escape every frame");
}
