//! **The blend in `ops::shaded`** — M86/T86.1 (`docs/plan-transitions.md`).
//!
//! While a transition runs, `NodeState::pointer_mix` carries a hover level
//! and a press level out of `ops::MIX_LEVELS`, and `shaded` blends the
//! background between rest, hover and press by them. Three claims, each
//! printed with its count:
//!
//! - **the endpoints are the bit path, byte for byte**: at the four level
//!   pairs (each 0 or 127) the blend gives exactly what `shaded` gives on
//!   the `pressed`/`hovered` bits with `pointer_mix == 0`, including
//!   "pressed with no `pressed_dim` shows the hover shade";
//! - **a level between moves the colour between**: at `h = 64` a channel
//!   lies strictly between its rest and hover values;
//! - **the interior rounds to nearest**: at `h = p = 64` the blend equals
//!   a reference computed here in `f32`, `a + ((b - a)·t + 63.5) / 127`
//!   floored, with 0 differing channels.
//!
//! The endpoint check cannot see a truncating `lerp` — truncation is exact
//! at `t = 0` and `t = 127` — which is why the interior check exists.
//!
//! In the shell's tests because the shell is what enables the core
//! `transitions` feature; the firmware does not, and links no blend
//! (`docs/results-transitions.md`, K86a).

use fulgurite::display::Color;
use fulgurite::ops::{encode_mix, shaded, MIX_LEVELS};
use fulgurite::{NodeState, Style};

const HOVER_DIMS: [f32; 3] = [0.0, 0.07, 0.12];
const PRESSED_DIMS: [f32; 2] = [0.0, 0.15];

/// 64 colours, extremes included, spread over the cube.
fn colours() -> Vec<Color> {
    let mut v = vec![Color(0, 0, 0, 255), Color(255, 255, 255, 255), Color(255, 0, 0, 128), Color(1, 2, 3, 0)];
    let mut x: u32 = 0x9e37_79b9;
    while v.len() < 64 {
        x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let b = x.to_le_bytes();
        v.push(Color(b[0], b[1], b[2], b[3] | 0x80));
    }
    v
}

/// Every (colour, hover_dim, pressed_dim) case.
fn cases() -> Vec<(Color, Style)> {
    let mut out = Vec::new();
    for c in colours() {
        for hd in HOVER_DIMS {
            for pd in PRESSED_DIMS {
                out.push((c, Style { hover_dim: hd, pressed_dim: pd, ..Style::default() }));
            }
        }
    }
    out
}

fn state(hovered: bool, pressed: bool, mix: u32) -> NodeState {
    NodeState { hovered, pressed, pointer_mix: mix, ..NodeState::default() }
}

fn channels(c: Color) -> [u8; 4] {
    [c.0, c.1, c.2, c.3]
}

#[test]
fn at_the_four_endpoints_the_blend_is_the_bit_path() {
    let cases = cases();
    let full = MIX_LEVELS;
    let (mut compared, mut differ) = (0usize, Vec::new());
    for (c, style) in &cases {
        for (hovered, pressed) in [(false, false), (true, false), (false, true), (true, true)] {
            let mix = encode_mix(if hovered { full } else { 0 }, if pressed { full } else { 0 });
            let blended_state = state(hovered, pressed, mix);
            // **Setup landed**: the level written is an active one.
            assert_ne!(blended_state.pointer_mix & 0x8000, 0, "encode_mix must set bit 15");
            let bits = shaded(*c, &state(hovered, pressed, 0), style);
            let blend = shaded(*c, &blended_state, style);
            compared += 1;
            if bits != blend {
                differ.push((*c, style.hover_dim, style.pressed_dim, hovered, pressed, bits, blend));
            }
        }
    }
    println!("endpoints: {} of {compared} differ ({} cases x 4 level pairs; bound 0)", differ.len(), cases.len());
    assert!(
        differ.is_empty(),
        "the blend at an endpoint is not the bit path: {} of {compared}, first {:?}",
        differ.len(),
        differ[0]
    );
}

#[test]
fn a_level_between_puts_the_shade_between() {
    let c = Color(200, 120, 60, 255);
    let style = Style { hover_dim: 0.12, ..Style::default() };
    let rest = shaded(c, &state(false, false, 0), &style);
    let hover = shaded(c, &state(true, false, 0), &style);
    let mid = shaded(c, &state(true, false, encode_mix(64, 0)), &style);
    println!("h = 64/127, red channel: rest {}, mid {}, hover {}", rest.0, mid.0, hover.0);
    assert!(hover.0 < mid.0 && mid.0 < rest.0, "a level between must give a channel strictly between");
}

#[test]
fn the_interior_rounds_to_nearest() {
    let cases = cases();
    let t = 64u32;
    // `a + ((b - a)·t + 63.5) / 127`, floored, in f32.
    let reference = |a: u8, b: u8| -> u8 {
        let (a, b) = (a as f32, b as f32);
        (a + ((b - a) * t as f32 + 63.5) / MIX_LEVELS as f32).floor() as u8
    };
    let (mut channels_compared, mut differ) = (0usize, Vec::new());
    for (c, style) in &cases {
        // The two dimmed colours, from the bit path: `shaded` is the only
        // public way to `dim`.
        let hover = shaded(*c, &state(true, false, 0), style);
        let press = if style.pressed_dim > 0.0 { Some(shaded(*c, &state(false, true, 0), style)) } else { None };
        let got = shaded(*c, &state(true, true, encode_mix(t, t)), style);
        let (cc, hh, gg) = (channels(*c), channels(hover), channels(got));
        let pp = press.map(channels);
        for k in 0..3 {
            let base = reference(cc[k], hh[k]);
            let want = match pp {
                Some(p) => reference(base, p[k]),
                None => base,
            };
            channels_compared += 1;
            if gg[k] != want {
                differ.push((*c, style.hover_dim, style.pressed_dim, k, gg[k], want));
            }
        }
    }
    println!(
        "interior h = p = {t}: {} of {channels_compared} channels differ from the rounding reference ({} cases; bound 0)",
        differ.len(),
        cases.len()
    );
    assert!(
        differ.is_empty(),
        "the interior does not round to nearest: {} of {channels_compared}, first (colour, hover_dim, pressed_dim, channel, got, want) {:?}",
        differ.len(),
        differ[0]
    );
}
