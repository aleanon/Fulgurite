//! **The application's store on the GPU** — M84/T84.6
//! (`docs/plan-images-in-motion.md`, `docs/results-images.md`).
//!
//! Since T84.6 a renderer resolves each `Cmd::Image` through the frame's
//! store first (`FrameSource::images`, the application's) and the
//! window's second (`Gpu::set_images`). Two things the GPU atlas has to
//! get right that the software canvas, which resolves every command
//! afresh, cannot get wrong:
//!
//! - **a swap**: the backend lives on the loader's side, and a swapped-in
//!   build's store may answer an unchanged `(id, frame)` with new pixels;
//!   the atlas asks a store only when a command's frame moves, so the
//!   window calls `Gpu::invalidate_images()` on the swap (second review
//!   A14);
//! - **the origin**: one id in both stores is two slots, so an id that
//!   falls through to the window's store — the application's is not
//!   handed over while M36's screen is up — misses instead of hitting the
//!   application's pixels.
//!
//! Every draw is full damage, isolating the atlas from retention. Skips
//! without an adapter under a plain `cargo test`; `just gpu` sets
//! FULGURITE_REQUIRE_GPU=1, under which a missing adapter fails.

use fulgurite::display::{Color, DisplayList, ImageId, ImageSource};
use fulgurite_wgpu::Gpu;

const W: u32 = 64;
const H: u32 = 48;
const PAGE: Color = Color(250, 250, 250, 255);
const RED: [u8; 4] = [220, 30, 30, 255];
const BLUE: [u8; 4] = [30, 60, 220, 255];
const WHITE: Color = Color(255, 255, 255, 255);
const RECT: [f32; 4] = [16.0, 8.0, 48.0, 40.0];
const FULL: [f32; 4] = [0.0, 0.0, W as f32, H as f32];

fn gpu() -> Option<Gpu> {
    match Gpu::headless(W, H) {
        Some(g) => {
            eprintln!("gpu: {}", g.adapter());
            Some(g)
        }
        None => {
            let required = std::env::var("FULGURITE_REQUIRE_GPU").is_ok_and(|v| v == "1");
            assert!(!required, "FULGURITE_REQUIRE_GPU=1 but no adapter (FULGURITE_ALLOW_SKIP=1 on the recipe to skip)");
            eprintln!("gpu: no adapter — skipping (this is a supported configuration)");
            None
        }
    }
}

/// Id 1, one flat 8-square frame of one colour, whatever the frame asks.
struct Flat(Vec<u8>);

impl Flat {
    fn new(c: [u8; 4]) -> Self {
        Flat(c.iter().copied().cycle().take(8 * 8 * 4).collect())
    }
}

impl ImageSource for Flat {
    fn image(&self, id: ImageId, _frame: u32) -> Option<(u32, u32, &[u8])> {
        (id == ImageId(1)).then_some((8, 8, &self.0[..]))
    }
}

fn list() -> DisplayList {
    let mut l = DisplayList::new();
    l.image(RECT, ImageId(1), WHITE, 0);
    l
}

fn centre(g: &mut Gpu) -> [u8; 4] {
    let img = g.read_rgba();
    let i = ((24 * W + 32) * 4) as usize;
    img[i..i + 4].try_into().unwrap()
}

/// **Across a swap** (A14). Store A answers id 1 frame 0 red; store B, a
/// swapped-in build's, answers the same pair blue. The test invalidates
/// as the window's swap does (`Backend::invalidate_images`), and the next
/// full draw shows B's blue. Without the call the atlas hits and keeps
/// A's red — the injection.
#[test]
fn a_swapped_in_store_reaches_the_atlas_after_invalidation() {
    let Some(mut g) = gpu() else { return };
    let (a, b) = (Flat::new(RED), Flat::new(BLUE));
    g.draw_regions_from(&list(), &[FULL], Some(PAGE), Some(&a));
    let first = centre(&mut g);
    println!("v1's store, (id 1, frame 0): centre {first:?}, image_uploads {}", g.atlas_stats().image_uploads);
    assert_eq!(first, RED, "setup: the first draw does not show store A's red");

    // The swap: the window calls this beside `reloaded`.
    g.invalidate_images();
    g.draw_regions_from(&list(), &[FULL], Some(PAGE), Some(&b));
    let second = centre(&mut g);
    let s = g.atlas_stats();
    println!(
        "v2's store, the same (id 1, frame 0), after invalidate_images(): centre {second:?} (want {BLUE:?}); \
         image_uploads {} (want 2), resets {}",
        s.image_uploads, s.resets
    );
    assert_eq!(second, BLUE, "after a swap the atlas kept v1's pixels for an unchanged (id, frame)");
    assert_eq!(s.image_uploads, 2, "the swapped-in store's pixels were not uploaded");
}

/// **Origin** (A14, T84.2's gate carried here). Id 1 is red in the
/// application's store and blue in the window's. Drawn with the frame's
/// store it is red; drawn again with the frame's store gone — what a
/// session showing M36's screen hands over — it falls through to the
/// window's and must be blue. Keyed on the id alone the second draw would
/// hit the application's slot and stay red — the injection.
#[test]
fn an_id_that_falls_through_misses_the_other_stores_slot() {
    let Some(mut g) = gpu() else { return };
    g.set_images(Box::new(Flat::new(BLUE)));
    let app = Flat::new(RED);
    g.draw_regions_from(&list(), &[FULL], Some(PAGE), Some(&app));
    let first = centre(&mut g);
    println!("frame's store present: centre {first:?} (want the application's {RED:?})");
    assert_eq!(first, RED, "setup: the application's store did not win for an id both hold");

    g.draw_regions_from(&list(), &[FULL], Some(PAGE), None);
    let second = centre(&mut g);
    let s = g.atlas_stats();
    println!(
        "frame's store gone (M36's screen): centre {second:?} (want the window's {BLUE:?}); image_uploads {} \
         (want 2: one slot per origin)",
        s.image_uploads
    );
    assert_eq!(second, BLUE, "an id that fell through to the window's store drew the application's slot");
    assert_eq!(s.image_uploads, 2, "one id in two stores did not take two slots");
}
