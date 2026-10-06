//! **An image whose pixels change under the same id** — M84/T84.0(a)
//! (`docs/plan-images-in-motion.md`), the software half.
//!
//! Damage is decided from the command stream: `Recording::image` folds
//! the rect, the id and the tint, and nothing else. So when the store
//! behind an [`ImageId`] starts answering new pixels, the next frame's
//! list is identical to the last, retention reports nothing stale, and
//! the canvas keeps the old picture — even after a tick that answers
//! `true`, which marks the whole tree and re-runs layout.
//!
//! **This pins that gap, green** (`docs/plan-iced-roadmap.md` §3.1): it
//! asserts today's stale pixel and its 0 px of damage, and prints both.
//! T84.2 puts the version in the command (`Cmd::Image.frame`), adds the
//! one line the tick below cannot write yet (`img.frame += 1`), and flips
//! the assertion to blue.

use std::cell::Cell;
use std::rc::Rc;

use lumen_static::display::{Color, ImageId, ImageSource};
use lumen_static::prelude::*;
use lumen_static_shell::session::{Offscreen, Screen};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

const W: u32 = 120;
const H: u32 = 80;
const SIDE: u32 = 8;
const PAGE: Color = Color(250, 250, 250, 255);
const RED: [u8; 4] = [220, 30, 30, 255];
const BLUE: [u8; 4] = [30, 60, 220, 255];

/// One id, whose pixels the shared flag switches from red to blue — the
/// store a decoder or a video feed would be, minus the decoder.
struct Switching {
    blue: Rc<Cell<bool>>,
    red: Vec<u8>,
    blu: Vec<u8>,
}

impl Switching {
    fn new(blue: Rc<Cell<bool>>) -> Self {
        let fill =
            |c: [u8; 4]| c.iter().copied().cycle().take((SIDE * SIDE * 4) as usize).collect();
        Switching { blue, red: fill(RED), blu: fill(BLUE) }
    }
}

impl ImageSource for Switching {
    fn image(&self, id: ImageId) -> Option<(u32, u32, &[u8])> {
        (id == ImageId(7))
            .then(|| (SIDE, SIDE, if self.blue.get() { &self.blu[..] } else { &self.red[..] }))
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("img")]
    img: Image,
}

#[derive(DeriveApp, Default)]
#[app(tick = advance)]
struct Anim {
    #[tree]
    ui: Ui,
    ticks: u32,
}

/// Answers `true` on a positive `dt`, as an animating tick does — so the
/// host marks the whole tree. It writes nothing to the image, because
/// nothing it could write reaches the command today; T84.2 adds
/// `a.ui.img.frame += 1` here.
fn advance(a: &mut Anim, dt: f32) -> bool {
    if dt > 0.0 {
        a.ticks += 1;
        return true;
    }
    false
}

/// The centre of the image's box, as RGBA.
fn centre(screen: &Offscreen, host: &Host<Anim>) -> [u8; 4] {
    let st = &host.states[1];
    let (x, y) = ((st.pos.x + st.size.w / 2.0) as u32, (st.pos.y + st.size.h / 2.0) as u32);
    let i = ((y * W + x) * 4) as usize;
    screen.canvas.pixels[i..i + 4].try_into().unwrap()
}

#[test]
fn a_store_that_changes_pixels_under_one_id_repaints_nothing_today() {
    let blue = Rc::new(Cell::new(false));
    let mut a = Anim::default();
    a.ui.img = Image::new(ImageId(7), Size { w: 40.0, h: 40.0 });
    let mut host: Host<Anim> = Host::new(a, W, H);
    let mut screen = Offscreen::new(W, H);
    screen.canvas.images = Some(Box::new(Switching::new(blue.clone())));

    host.pump();
    host.pump();
    let first = screen.present(&mut host, PAGE);
    let before = centre(&screen, &host);
    println!("frame 0: centre {before:?}, damaged_px {:?}", first.damaged_px);
    assert_eq!(before, RED, "setup: frame 0 does not show the store's red at the image's centre");

    // The store switches; the application ticks and pumps; the frame is
    // presented.
    blue.set(true);
    let ticked = host.tick(16.0);
    host.pump();
    let t = screen.present(&mut host, PAGE);
    let after = centre(&screen, &host);
    let px = t.damaged_px.unwrap_or(u64::MAX);
    println!(
        "after the switch: tick answered {ticked} ({} tick), centre {after:?} (store now answers {BLUE:?}), \
         damaged_px {px} (pinned 0; T84.2 bounds it at the image's box, {} px)",
        host.app.ticks,
        (host.states[1].size.w * host.states[1].size.h) as u64
    );
    assert!(ticked && host.app.ticks == 1, "setup: the tick did not run or answered false");
    assert_eq!(
        (after, px),
        (RED, 0),
        "pinned (T84.0a): a store that changes pixels under one id stays stale with 0 px damaged -- \
         if this moved, the version reached the command (T84.2 flips this to blue)"
    );
}
