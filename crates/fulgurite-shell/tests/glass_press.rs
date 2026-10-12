//! M22/T22.4: a glass panel squashes under a press.
//!
//! Everything here is driven through `Host` — the real pump, the real
//! dispatch, the real `App::tick`. `docs/plan-liquid.md` §6 requires it:
//! a hand-rolled tick loop passes whether or not the frame loop asks
//! about animation at the right moment, which is half of what this task
//! fixes.

use fulgurite::display::{Color, Material};
use fulgurite::event::Event;
use fulgurite_shell::software::Canvas;
use fulgurite_shell::Host;

const W: u32 = 260;
const H: u32 = 200;
/// The `Glass` node, which `dispatch` never marks.
const PANEL: usize = 1;
/// The button inside it, which it does.
const BUTTON: usize = 3;
/// First column clear of the button's own press styling.
const CLEAR_OF_BUTTON: usize = 100;

#[derive(fulgurite::StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Inner {
    button: fulgurite_widgets::prelude::Button<A>,
}

#[derive(fulgurite::StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    panel: fulgurite_widgets::Glass<Inner>,
}

/// The application advances what it animates — there is no
/// framework-walked per-node tick, by a recorded decision (`App::tick`).
///
/// Reached through `#[app(tick = ...)]` rather than a hand-written
/// `impl App`, because the derive writes the *whole* impl: before
/// M22/T22.4 added the hook there was no way for a derived application
/// to advance anything, and `Glass`'s press would have retargeted and
/// then sat still forever.
fn advance(a: &mut A, dt: f32) -> bool {
    a.ui.panel.tick(dt)
}

#[derive(fulgurite::App, Default)]
#[app(tick = advance)]
struct A {
    #[tree]
    ui: Ui,
}

fn host() -> Host<A> {
    let mut a = A::default();
    a.ui.panel.material = Material::LIQUID;
    a.ui.panel.content.button.label = "press me";
    let mut h: Host<A> = Host::new(a, W, H);
    h.pump();
    h
}

fn shot(h: &Host<A>) -> Canvas {
    let mut c = Canvas::new(W, H);
    c.clear(Color(0x20, 0x28, 0x40, 0xff));
    fulgurite::Renderer::render(&mut c, &h.list, W, H);
    c
}

fn differing(a: &Canvas, b: &Canvas) -> usize {
    a.pixels.chunks(4).zip(b.pixels.chunks(4)).filter(|(p, q)| p[..3] != q[..3]).count()
}

/// Differing pixels **clear of the button**, which is the only honest
/// way to measure what the *panel* did.
///
/// The button dims on press all by itself, and it does so over
/// `92 x 34` px — 2416 of them. Counted frame-wide, that alone cleared a
/// bound of 400, so the deformation gate stayed green with the press
/// animation deleted. Both injections for this task reported a "peak"
/// of exactly 2416: the button's rect, and nothing else.
fn differing_right_of_the_button(a: &Canvas, b: &Canvas, x0: usize) -> usize {
    let mut n = 0;
    for y in 0..H as usize {
        for x in x0..W as usize {
            let i = (y * W as usize + x) * 4;
            if a.pixels[i..i + 3] != b.pixels[i..i + 3] {
                n += 1;
            }
        }
    }
    n
}

/// **A press thickens the rim, and releasing puts it back exactly.**
///
/// Two vacuity traps `docs/plan-liquid.md` §11 names, both avoided here:
///
/// - `FROSTED` has no rim, so the bevel is skipped entirely and every
///   press test would render a panel compared with itself. The material
///   is `LIQUID` and the rim is asserted non-zero first.
/// - The press is *temporal*, and the obvious test is one frame.
///   Nothing advances an animation unless the test advances it, so a
///   test that presses and renders sees the value at rest: the flag
///   changed, the pixels matched, the injection was green. This is a
///   sequence, and the animated value is printed at each step.
#[test]
fn a_press_thickens_the_rim_and_releasing_restores_it() {
    let mut h = host();
    assert!(h.app.ui.panel.material.rim > 0.0, "a material with no rim has no bevel to deform");

    // The press lands on the *button*, which is where `dispatch` marks
    // it — the panel's own node never carries the flag.
    // **The centre of the button, which is not the centre of the
    // panel.** The panel is 260 px wide and the button 92, so a click at
    // the panel's centre lands on the panel itself — which is pressable,
    // so the test would still go green while proving only the trivial
    // case. That is the `Fixed(220)` failure in `AGENT.md`, reached from
    // the other direction.
    let b = &h.states[BUTTON];
    let (cx, cy) = (b.pos.x + b.size.w / 2.0, b.pos.y + b.size.h / 2.0);

    // **Hover first, and take the resting frame with the pointer already
    // there.** The button dims on hover as well as on press, and the
    // pointer does not leave when the button is released — so a resting
    // frame captured before any pointer event differs from the settled
    // one by the *hover*, and 2416 px of it were charged to the press.
    // Held still, the only thing that changes across this test is the
    // press.
    h.post(Event::PointerMove { x: cx, y: cy });
    h.pump();
    let rest = shot(&h);

    h.post(Event::down(cx, cy));
    h.pump();
    let hit: Vec<usize> =
        h.states.iter().enumerate().filter(|(_, n)| n.pressed).map(|(i, _)| i).collect();
    println!("  pressed nodes: {hit:?} (panel is {PANEL}, button is {BUTTON})");
    assert_eq!(
        hit,
        vec![BUTTON],
        "the press did not land on the button alone — the subtree scan is untested"
    );
    assert!(!h.states[PANEL].pressed, "the panel carries the flag; nothing here needs a subtree scan");

    let mut peak = 0;
    for step in 0..6 {
        h.tick(30.0);
        h.pump();
        let d = differing_right_of_the_button(&rest, &shot(&h), CLEAR_OF_BUTTON);
        println!("  press step {step}: {d} px differ from rest");
        peak = peak.max(d);
    }
    println!("peak deformation {peak} px");
    assert!(peak > 400, "the press barely deformed the panel: {peak} px");
    // And the whole frame still has to come back, button included.

    // Release, and run the animation out.
    h.post(Event::up(cx, cy));
    h.pump();
    for _ in 0..12 {
        h.tick(30.0);
        h.pump();
    }
    assert!(!h.tick(0.0), "the animation never settled");
    let settled = differing(&rest, &shot(&h));
    println!("settled vs pre-press: {settled} px differ");
    assert_eq!(settled, 0, "the settled frame is not the pre-press frame ({settled} px)");
}

/// **The frame loop must ask about animation *after* the pump.**
///
/// This is the reorder in `fulgurite-winit`, stated as the property
/// that makes it necessary. The tick has to stay *before* the pump —
/// the bindings the pump runs are what put a ticked value on screen —
/// but its answer is stale by the time the pump returns: a press
/// dispatched during the pump starts an animation the tick could not
/// have seen. Asked in the old order, the loop is told nothing is
/// moving, requests no redraw, and the rim jumps to its next value on
/// the following unrelated event.
///
/// A zero `dt` is a pure query, which is what lets one call answer it.
#[test]
fn an_animation_started_by_the_pump_is_reported_after_it() {
    let mut h = host();
    // Asked before the pump — the order the loop used until T22.4 —
    // the answer is about the previous frame.
    let before = h.tick(0.0);
    let b = &h.states[BUTTON];
    h.post(Event::down(b.pos.x + b.size.w / 2.0, b.pos.y + b.size.h / 2.0));
    h.pump();
    let after = h.tick(0.0);
    println!("animating: before the pump {before}, after it {after}");
    assert!(!before, "the fixture was already animating; it proves nothing");
    assert!(after, "the press started nothing the loop could see — it would not request a redraw");
}
