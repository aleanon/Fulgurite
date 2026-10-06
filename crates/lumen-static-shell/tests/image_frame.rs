//! **A new frame of one image repaints its rect, and only its rect** —
//! M84/T84.2 (`docs/plan-images-in-motion.md`), the software half.
//!
//! Damage is decided from the command stream. Until T84.2,
//! `Recording::image` folded the rect, the id and the tint and nothing
//! else, so a source that started answering new pixels under one
//! [`ImageId`] produced an identical list, retention reported nothing
//! stale, and the canvas kept the old picture — even after a tick that
//! answered `true` (T84.0(a) pinned exactly that: red, 0 px damaged).
//! The version is in the command now (`Cmd::Image.frame`): the tick
//! writes `Image::frame`, `Recording` and `Cmd::hash_into` fold it, and
//! the source answers for it ([`ImageSource::image`]).

use lumen_static::display::{Color, DisplayList, ImageId, ImageSource};
use lumen_static::prelude::*;
use lumen_static::widget::Node;
use lumen_static_shell::session::{Offscreen, Screen};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

const W: u32 = 120;
const H: u32 = 100;
const SIDE: u32 = 8;
const PAGE: Color = Color(250, 250, 250, 255);
const RED: [u8; 4] = [220, 30, 30, 255];
const BLUE: [u8; 4] = [30, 60, 220, 255];

/// One id, red at even frames and blue at odd — a two-frame sequence, the
/// store a decoder would fill, minus the decoder.
struct Frames {
    red: Vec<u8>,
    blu: Vec<u8>,
}

impl Frames {
    fn new() -> Self {
        let fill =
            |c: [u8; 4]| c.iter().copied().cycle().take((SIDE * SIDE * 4) as usize).collect();
        Frames { red: fill(RED), blu: fill(BLUE) }
    }
}

impl ImageSource for Frames {
    fn image(&self, id: ImageId, frame: u32) -> Option<(u32, u32, &[u8])> {
        (id == ImageId(7)).then(|| (SIDE, SIDE, if frame % 2 == 1 { &self.blu[..] } else { &self.red[..] }))
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("img")]
    img: Image,
    #[widget_id("still")]
    still: Image,
}

#[derive(DeriveApp, Default)]
#[app(tick = advance)]
struct Anim {
    #[tree]
    ui: Ui,
    ticks: u32,
}

/// Answers `true` on a positive `dt`, as an animating tick does, after
/// advancing the first image's frame — the line T84.0(a) could not write.
/// The second image is never written.
fn advance(a: &mut Anim, dt: f32) -> bool {
    if dt > 0.0 {
        a.ticks += 1;
        a.ui.img.frame += 1;
        return true;
    }
    false
}

/// The centre of node `i`'s box, as RGBA.
fn centre(screen: &Offscreen, host: &Host<Anim>, i: usize) -> [u8; 4] {
    let st = &host.states[i];
    let (x, y) = ((st.pos.x + st.size.w / 2.0) as u32, (st.pos.y + st.size.h / 2.0) as u32);
    let p = ((y * W + x) * 4) as usize;
    screen.canvas.pixels[p..p + 4].try_into().unwrap()
}

/// Each leaf's own paint, hashed — the journal's per-node signature
/// (`lumen_static_agent`'s `measure_into`, and the shell's shadow oracle's
/// `sign_leaves`): `DisplayList::hash_from` over what that node paints.
fn signatures(host: &Host<Anim>) -> [u64; 2] {
    let mut l = DisplayList::new();
    [1, 2].map(|i| {
        let from = l.len();
        host.app.ui.node(i).paint(&host.states, i, &mut l);
        l.hash_from(from)
    })
}

fn boot() -> (Host<Anim>, Offscreen) {
    let mut a = Anim::default();
    a.ui.img = Image::new(ImageId(7), Size { w: 40.0, h: 40.0 });
    a.ui.still = Image::new(ImageId(7), Size { w: 40.0, h: 40.0 });
    let host: Host<Anim> = Host::new(a, W, H);
    let mut screen = Offscreen::new(W, H);
    screen.canvas.images = Some(Box::new(Frames::new()));
    (host, screen)
}

/// **T84.0(a), flipped.** The tick advances the first image to frame 1;
/// the next present shows blue at its centre and damages exactly its box
/// (1.00×). And the neighbour, one id at an unchanged frame, keeps its
/// pixels, is not damaged, and its journal signature does not move.
#[test]
fn a_new_frame_repaints_its_image_and_not_its_neighbour() {
    let (mut host, mut screen) = boot();
    host.pump();
    host.pump();
    let first = screen.present(&mut host, PAGE);
    let (a0, b0) = (centre(&screen, &host, 1), centre(&screen, &host, 2));
    let sig0 = signatures(&host);
    println!("frame 0: centres {a0:?} {b0:?}, damaged_px {:?}", first.damaged_px);
    assert_eq!((a0, b0), (RED, RED), "setup: frame 0 does not show the store's red at both images' centres");

    let ticked = host.tick(16.0);
    host.pump();
    let t = screen.present(&mut host, PAGE);
    let (a1, b1) = (centre(&screen, &host, 1), centre(&screen, &host, 2));
    let sig1 = signatures(&host);
    let px = t.damaged_px.unwrap_or(u64::MAX);
    let rect = (host.states[1].size.w * host.states[1].size.h) as u64;
    let ratio = px as f64 / rect as f64;
    println!(
        "after the tick: tick answered {ticked} ({} tick), img.frame {}, still.frame {}; centres {a1:?} {b1:?} \
         (want {BLUE:?} {RED:?}); damaged_px {px} against the image's box {rect} px: {ratio:.2}x (bound 1.00x)",
        host.app.ticks, host.app.ui.img.frame, host.app.ui.still.frame,
    );
    println!(
        "journal: img signature {:016x} -> {:016x} (moved {}), still {:016x} -> {:016x} (listed {}; bound: absent)",
        sig0[0],
        sig1[0],
        sig0[0] != sig1[0],
        sig0[1],
        sig1[1],
        sig0[1] != sig1[1]
    );
    assert!(ticked && host.app.ticks == 1, "setup: the tick did not run or answered false");
    assert_eq!(host.app.ui.img.frame, 1, "setup: the tick did not advance the image to frame 1");
    assert_eq!(a1, BLUE, "frame 1 of img (id 7) did not reach the canvas: the frame is not in the command's fold");
    assert_eq!(ratio, 1.0, "a new frame damaged {px} px, not the image's box of {rect} px");
    assert_eq!(b1, RED, "the neighbour at frame 0 changed pixels");
    assert_ne!(sig0[0], sig1[0], "node 1 (img): its journal signature did not move with its frame");
    assert_eq!(sig0[1], sig1[1], "node 2 (still) is in the changed-signature list, and its frame never moved");

    // Accept: a frame that writes nothing new is no damage.
    host.pump();
    let idle = screen.present(&mut host, PAGE);
    println!("idle: damaged_px {:?} (want 0)", idle.damaged_px);
    assert_eq!(idle.damaged_px.unwrap_or(u64::MAX), 0, "an unchanged frame damaged pixels");
}
