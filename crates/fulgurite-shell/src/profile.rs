//! **Per-stage frame cost, headless** — `FULGURITE_PROFILE`, beside
//! `FULGURITE_SHOT` (M60/T60.4, `docs/plan-tally-slim.md`).
//!
//! Until M60 this was `examples/counter-window`'s own `--profile`, fixed
//! to the tally; `just profile*` still measure the tally, and the numbers
//! belong in `docs/results-frame-cost.md`. Every `Program` gets it now:
//! the window crate's `FULGURITE_SHOT` path calls [`profile`] on the host it
//! has just rendered when `FULGURITE_PROFILE` is set.
//!
//! Each stage is timed 50 times and its median printed to stderr: the
//! pump, a clear, a whole render and the render per command class, the
//! blit to the window's pixel format. **The partial path** — what a
//! damaged frame costs — needs a change to damage, and a change is the
//! application's: with `FULGURITE_PROFILE_CLICK=<author id>` the node is
//! clicked twice and the second click's damage is measured. Twice,
//! because the first moves hover and focus onto the node, whose restyled
//! paint would otherwise be in the damage; the second changes only what
//! its handler changes (for the tally's `#inc`, the value and the pill —
//! the damage a count change makes, which the old `--profile` measured by
//! writing the count directly).

use std::time::Instant;

use fulgurite::display::{Cmd, Color, DisplayList};
use fulgurite::event::App;
use fulgurite::invalidate::Invalidation;
use fulgurite::retain::Retention;
use fulgurite::widget::Widget;
use fulgurite::TextMeasure;

use crate::software::Canvas;
use crate::Host;

/// Median of 50 timed runs of `f`, in µs.
fn median(mut f: impl FnMut()) -> u128 {
    let mut v: Vec<u128> = (0..50)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed().as_micros()
        })
        .collect();
    v.sort();
    v[25]
}

/// The list's commands of one class, with its text.
fn only(list: &DisplayList, keep: fn(&Cmd) -> bool) -> DisplayList {
    let mut sub = DisplayList::new();
    for c in &list.cmds {
        if keep(c) {
            sub.cmds.push(*c);
        }
    }
    sub.text = list.text.clone();
    sub
}

/// A command class: its name, and which commands belong to it.
type Class = (&'static str, fn(&Cmd) -> bool);

const CLASSES: [Class; 4] = [
    ("rect", |c| matches!(c, Cmd::Rect { .. })),
    ("rounded_rect", |c| matches!(c, Cmd::RoundedRect { .. })),
    // **Shadow was missing from this list until 2026-09-09**, so the
    // full-render rows summed to 86 of 345 us and the largest single
    // command class was unattributed (`docs/results-wide-stores.md`).
    ("shadow", |c| matches!(c, Cmd::Shadow { .. })),
    ("text", |c| matches!(c, Cmd::Text { .. })),
];

/// Print the per-stage cost of `host`'s frame to stderr. `click` is the
/// author id `FULGURITE_PROFILE_CLICK` named, for the partial path.
pub fn profile<A: App, R: Retention, I: Invalidation>(
    host: &mut Host<A, R, I>,
    canvas: &mut Canvas,
    metrics: &mut dyn TextMeasure,
    page: Color,
    click: Option<&str>,
) {
    let (w, h) = (canvas.width, canvas.height);
    // `invalidate_all` first: under a strategy that can skip, a pump
    // with nothing marked does nothing, and this row is the frame's cost.
    let us = median(|| {
        host.invalidate_all();
        host.pump_with(metrics);
    });
    eprintln!("{:<22} {us:>8} us", "pump(layout+paint)");
    eprintln!("{:<22} {:>8} us", "clear", median(|| canvas.clear(page)));
    eprintln!("{:<22} {:>8} us", "render(all)", median(|| host.render(canvas)));
    for (name, keep) in CLASSES {
        let sub = only(&host.list, keep);
        let us = median(|| fulgurite::Renderer::render(canvas, &sub, w, h));
        eprintln!("  {name:<20} {us:>8} us  ({} cmds)", sub.cmds.len());
    }
    let mut buf = vec![0u32; (w * h) as usize];
    eprintln!("{:<22} {:>8} us", "blit rgba->0rgb", median(|| canvas.blit_0rgb(&mut buf)));

    let Some(wid) = click else {
        eprintln!("{:<22} (set FULGURITE_PROFILE_CLICK=<author id> to measure a damaged frame)", "damage");
        return;
    };
    let table = <A::Tree as Widget>::SUB_TABLE;
    let Some(i) = fulgurite::style::index_of_wid(table, wid) else {
        eprintln!("FULGURITE_PROFILE_CLICK: no node carries #{wid}");
        return;
    };
    let s = &host.states[i];
    // Logical, as `NodeState` and the input queue both are.
    let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    // The first click moves hover and focus; bank its damage.
    host.click(x, y);
    host.pump_with(metrics);
    host.take_damage_for_age(1);
    host.click(x, y);
    host.pump_with(metrics);
    let Some(area) = host.take_damage_for_age(1).clip_to([0.0, 0.0, w as f32, h as f32]) else {
        eprintln!("{:<22} none -- a click on #{wid} changed nothing", "damage");
        return;
    };
    let px = ((area[2] - area[0]) * (area[3] - area[1])) as u64;
    eprintln!("{:<22} {px:>8} px  {area:?}  (#{wid} clicked)", "damage");
    eprintln!("{:<22} {:>8} us", "  clear_within", median(|| canvas.clear_within(area, page)));
    eprintln!("{:<22} {:>8} us", "  render_within", median(|| canvas.render_within(&host.list, area)));
    eprintln!("{:<22} {:>8} us", "  blit_within", median(|| canvas.blit_0rgb_within(&mut buf, area)));
    for (name, keep) in CLASSES.iter().skip(1) {
        let sub = only(&host.list, *keep);
        let us = median(|| canvas.render_within(&sub, area));
        eprintln!("    {name:<18} {us:>8} us  ({} cmds)", sub.cmds.len());
    }
}
