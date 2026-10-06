//! **The GPU image atlas under pixels that change** — M84/T84.0(b)
//! (`docs/plan-images-in-motion.md`).
//!
//! The atlas caches an image by its [`ImageId`] alone (`image_slot`): the
//! first draw uploads it, and every later draw of that id samples the
//! slot without asking the store again. So a store that switches its
//! pixels from red to blue under one id keeps drawing red on this
//! backend **even when the whole surface is redrawn** — both draws here
//! are full damage, which isolates the cache from retention (the software
//! half, `lumen-static-shell/tests/image_frame.rs`, is the damage gap).
//!
//! **Pinned green** (`docs/plan-iced-roadmap.md` §3.1): the second draw's
//! centre is asserted red and printed beside `atlas_stats()`. T84.2 keys
//! the slot on the command's frame, writes the new pixels in place, and
//! flips the assertion to blue.
//!
//! Skips without an adapter under a plain `cargo test`; `just gpu` sets
//! LUMEN_REQUIRE_GPU=1, under which a missing adapter fails.

use std::cell::Cell;
use std::rc::Rc;

use lumen_static::display::{Color, DisplayList, ImageId, ImageSource};
use lumen_static_wgpu::Gpu;

const W: u32 = 64;
const H: u32 = 48;
const SIDE: u32 = 8;
const PAGE: Color = Color(250, 250, 250, 255);
const RED: [u8; 4] = [220, 30, 30, 255];
const BLUE: [u8; 4] = [30, 60, 220, 255];

/// One id, whose pixels the shared flag switches from red to blue.
struct Switching {
    blue: Rc<Cell<bool>>,
    red: Vec<u8>,
    blu: Vec<u8>,
}

impl ImageSource for Switching {
    fn image(&self, id: ImageId) -> Option<(u32, u32, &[u8])> {
        (id == ImageId(7))
            .then(|| (SIDE, SIDE, if self.blue.get() { &self.blu[..] } else { &self.red[..] }))
    }
}

fn gpu_required() -> bool {
    std::env::var("LUMEN_REQUIRE_GPU").is_ok_and(|v| v == "1")
}

/// The centre of the 32 × 32 image at (16, 8).
fn centre(px: &[u8]) -> [u8; 4] {
    let (x, y) = (16 + 16, 8 + 16);
    let i = ((y * W + x) * 4) as usize;
    px[i..i + 4].try_into().unwrap()
}

#[test]
fn the_atlas_keeps_an_ids_first_pixels_today() {
    let Some(mut g) = Gpu::headless(W, H) else {
        assert!(
            !gpu_required(),
            "LUMEN_REQUIRE_GPU=1 but no adapter (LUMEN_ALLOW_SKIP=1 on the recipe to skip)"
        );
        eprintln!("gpu: no adapter — skipping (this is a supported configuration)");
        return;
    };
    eprintln!("gpu: {}", g.adapter());
    let blue = Rc::new(Cell::new(false));
    let fill = |c: [u8; 4]| -> Vec<u8> {
        c.iter().copied().cycle().take((SIDE * SIDE * 4) as usize).collect()
    };
    g.set_images(Box::new(Switching { blue: blue.clone(), red: fill(RED), blu: fill(BLUE) }));

    let mut l = DisplayList::new();
    l.image([16.0, 8.0, 48.0, 40.0], ImageId(7), Color(255, 255, 255, 255));
    let full = [0.0, 0.0, W as f32, H as f32];

    g.draw(&l, None, Some((PAGE, full)));
    let first = centre(&g.read_rgba());
    let s1 = g.atlas_stats();
    println!("draw 1 (full damage): centre {first:?}, atlas {s1:?}");
    assert_eq!(
        first, RED,
        "setup: the first draw does not show the store's red at the image's centre"
    );

    blue.set(true);
    g.draw(&l, None, Some((PAGE, full)));
    let second = centre(&g.read_rgba());
    let s2 = g.atlas_stats();
    println!(
        "draw 2 (full damage, store now answers {BLUE:?}): centre {second:?}, atlas {s2:?} \
         (pinned red; T84.2 flips this to blue with image_uploads = 2)"
    );
    assert_eq!(
        second, RED,
        "pinned (T84.0b): the atlas caches by ImageId, so a full redraw still samples the first upload -- \
         if this moved, the slot is keyed on more than the id (T84.2 flips this to blue)"
    );
}
