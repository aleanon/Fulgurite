//! **A keystroke in a `TextArea` costs what its text costs, not its text
//! squared** — M45/T45.0 item 13, for R16-F7
//! (`docs/plan-text-correctness.md` §1.6).
//!
//! Two quadratics in `FontStack::shape_into`: `line_of` searched every
//! line for each glyph (glyphs × lines), and when substitution ran,
//! `remap` did a `position` and an `nth` over the character indices for
//! every glyph (glyphs × chars). Reshaping the whole document per edit is
//! real too, and linear.
//!
//! A timing, so a ratio of minima: five keystroke-plus-layout timings at
//! each size, the minimum of each, and the ratio of the two. Ten times
//! the text is about 10 when linear and about 100 when quadratic; the
//! bound is 25. **The self-ratio** — the small size against itself — is
//! printed beside it, so noise on a loaded machine is visible next to the
//! number it could spoil.
//!
//! The substituted arm is at a tenth of the size: at 200 KB its
//! quadratic runs for minutes, and a gate that takes an hour to go red
//! is not run. The same ratio of ten.
#![cfg(feature = "truetype")]

use std::time::{Duration, Instant};

use fulgurite::prelude::*;
use fulgurite::{Event, Key};
use fulgurite_shell::text::{FontMetrics, FontStack};
use fulgurite_shell::Host;
use fulgurite_widgets::TextArea;

#[path = "fonts/pinned.rs"]
mod pinned;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("area")]
    area: TextArea,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

/// `bytes` of wrapping prose, with `extra` every 97 characters when given.
fn text(bytes: usize, extra: Option<char>) -> String {
    let words = "lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor ";
    let mut s = String::with_capacity(bytes + 8);
    let mut n = 0;
    while s.len() < bytes {
        for c in words.chars() {
            s.push(c);
            n += 1;
            if let (Some(e), 0) = (extra, n % 97) {
                s.push(e);
            }
        }
    }
    s
}

/// The minimum of five keystroke-plus-layout timings.
fn keystroke(font: &FontStack, body: &str) -> Duration {
    let mut a = App::default();
    a.ui.area.set_str(body);
    let mut m = FontMetrics::new(font.clone());
    let mut h: Host<App> = Host::new(a, 480, 400);
    h.pump_with(&mut m);
    h.post(Event::Key { key: Key::Tab, down: true });
    h.pump_with(&mut m);
    assert_eq!(h.input.focus, Some(1), "setup: Tab did not focus the area");
    let len = h.app.ui.area.as_str().len();
    let mut best = Duration::MAX;
    for _ in 0..5 {
        h.post(Event::Text { ch: 'x' });
        let t = Instant::now();
        h.pump_with(&mut m);
        best = best.min(t.elapsed());
    }
    assert_eq!(h.app.ui.area.as_str().len(), len + 5, "setup: the keystrokes did not land");
    best
}

fn ratio(font: &FontStack, name: &str, small: usize, extra: Option<char>) -> f64 {
    let (a, b) = (text(small, extra), text(small * 10, extra));
    let self_a = keystroke(font, &a).as_secs_f64();
    let ta = keystroke(font, &a).as_secs_f64();
    let tb = keystroke(font, &b).as_secs_f64();
    let r = tb / ta;
    println!(
        "{name}: keystroke {}KB={:.2} ms {}KB={:.2} ms ratio={r:.1} (bound 25), self-ratio {:.2}",
        small / 1024,
        ta * 1e3,
        small * 10 / 1024,
        tb * 1e3,
        ta / self_a
    );
    r
}

#[test]
fn a_keystroke_scales_linearly_with_the_document() {
    let font = FontStack::from_bytes(pinned::bytes(), None).expect("the pinned face parses");
    // Substitution must actually run in the second arm, or it measures
    // the first arm twice: count the '?'s the painter lays out.
    let (q, _) = font.place_run("?", 16.0, 400.0, f32::INFINITY);
    let (sub, _) = font.place_run(&text(2048, Some('\u{4f60}')), 16.0, 400.0, f32::INFINITY);
    let substituted = sub.iter().filter(|p| p.id == q[0].id).count();
    println!("substituted arm: {substituted} glyphs substituted in 2 KB (bound > 0)");
    assert!(substituted > 0, "setup: nothing was substituted, so the second arm cannot reach remap");

    let plain = ratio(&font, "plain", 20 * 1024, None);
    let subst = ratio(&font, "substituted", 2 * 1024, Some('\u{4f60}'));
    assert!(plain < 25.0 && subst < 25.0, "keystroke cost grows faster than the text: plain {plain:.1}, substituted {subst:.1}");
}
