//! **The GPU image atlas under frames that change** — M84/T84.2
//! (`docs/plan-images-in-motion.md`, `docs/results-images.md`).
//!
//! Until T84.2 the atlas cached an image by its [`ImageId`] alone
//! (`image_slot`): the first draw uploaded it, and every later draw of
//! that id sampled the slot without asking the store again, so a store
//! answering new pixels kept drawing the old ones **even when the whole
//! surface was redrawn** (T84.0(b) pinned it: red on the second full
//! draw). The frame is in the command now, and the atlas keys an id's slot
//! on it: a new frame of the same size is written into the same slot in
//! place (`AtlasStats::image_uploads` counts each write), a size change
//! inserts again, and a second frame of one id in one draw takes a side
//! slot so both quads show their own frame.
//!
//! Every draw here is full damage, which isolates the atlas from
//! retention (the software half, `fulgurite-shell/tests/image_frame.rs`,
//! is the damage gate).
//!
//! Skips without an adapter under a plain `cargo test`; `just gpu` sets
//! FULGURITE_REQUIRE_GPU=1, under which a missing adapter fails.

use fulgurite::display::{Color, DisplayList, ImageId, ImageSource, Renderer};
use fulgurite_shell::software::Canvas;
use fulgurite_wgpu::Gpu;

const PAGE: Color = Color(250, 250, 250, 255);
const RED: [u8; 4] = [220, 30, 30, 255];
const BLUE: [u8; 4] = [30, 60, 220, 255];
const GREEN: [u8; 4] = [30, 160, 60, 255];
const AMBER: [u8; 4] = [230, 170, 20, 255];
const WHITE: Color = Color(255, 255, 255, 255);

fn gpu_required() -> bool {
    std::env::var("FULGURITE_REQUIRE_GPU").is_ok_and(|v| v == "1")
}

fn gpu(w: u32, h: u32) -> Option<Gpu> {
    match Gpu::headless(w, h) {
        Some(g) => {
            eprintln!("gpu: {}", g.adapter());
            Some(g)
        }
        None => {
            assert!(!gpu_required(), "FULGURITE_REQUIRE_GPU=1 but no adapter (FULGURITE_ALLOW_SKIP=1 on the recipe to skip)");
            eprintln!("gpu: no adapter — skipping (this is a supported configuration)");
            None
        }
    }
}

fn fill(side: u32, c: [u8; 4]) -> Vec<u8> {
    c.iter().copied().cycle().take((side * side * 4) as usize).collect()
}

/// One id whose frame `k` is a flat `side`-square of `colours[k % len]`.
struct Seq {
    side: u32,
    frames: Vec<Vec<u8>>,
}

impl Seq {
    fn new(side: u32, colours: &[[u8; 4]]) -> Self {
        Seq { side, frames: colours.iter().map(|&c| fill(side, c)).collect() }
    }
}

impl ImageSource for Seq {
    fn image(&self, id: ImageId, frame: u32) -> Option<(u32, u32, &[u8])> {
        (id == ImageId(7)).then(|| (self.side, self.side, &self.frames[frame as usize % self.frames.len()][..]))
    }
}

fn px(img: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    img[i..i + 4].try_into().unwrap()
}

fn one(rect: [f32; 4], frame: u32) -> DisplayList {
    let mut l = DisplayList::new();
    l.image(rect, ImageId(7), WHITE, frame);
    l
}

/// **T84.0(b), flipped.** Frame 0 then frame 1 of one id, both full
/// damage: the second draw is blue, and the atlas uploaded twice.
#[test]
fn a_new_frame_of_one_id_reaches_the_gpu() {
    const W: u32 = 64;
    const H: u32 = 48;
    let Some(mut g) = gpu(W, H) else { return };
    g.set_images(Box::new(Seq::new(8, &[RED, BLUE])));
    let rect = [16.0, 8.0, 48.0, 40.0];
    let full = [0.0, 0.0, W as f32, H as f32];

    g.draw(&one(rect, 0), None, Some((PAGE, full)));
    let first = px(&g.read_rgba(), W, 32, 24);
    println!("draw 1 (frame 0, full damage): centre {first:?}, atlas {:?}", g.atlas_stats());
    assert_eq!(first, RED, "setup: the first draw does not show frame 0's red at the image's centre");

    g.draw(&one(rect, 1), None, Some((PAGE, full)));
    let second = px(&g.read_rgba(), W, 32, 24);
    let s = g.atlas_stats();
    println!(
        "draw 2 (frame 1, full damage): centre {second:?} (want {BLUE:?}); image_uploads {} (want 2), \
         image_upload_bytes {}, resets {}",
        s.image_uploads, s.image_upload_bytes, s.resets
    );
    assert_eq!(second, BLUE, "frame 1 did not reach the GPU: the atlas served the slot it holds for frame 0");
    assert_eq!(s.image_uploads, 2, "a first sight and one frame change are two uploads");
}

/// **One id at two frames in one list** (P1). Every `write_texture`
/// lands before the frame's one submit, so rewriting one slot in place
/// for the second command would leave both quads sampling it. The GPU
/// must agree with the software canvas, which resolves each command.
#[test]
fn one_id_at_two_frames_in_one_list_draws_both() {
    const W: u32 = 96;
    const H: u32 = 48;
    let Some(mut g) = gpu(W, H) else { return };
    let colours = [GREEN, RED, BLUE, AMBER];
    g.set_images(Box::new(Seq::new(8, &colours)));
    let mut c = Canvas::new(W, H);
    c.images = Some(Box::new(Seq::new(8, &colours)));

    let (a, b) = ([8.0, 8.0, 40.0, 40.0], [56.0, 8.0, 88.0, 40.0]);
    let mut l = DisplayList::new();
    l.image(a, ImageId(7), WHITE, 1);
    l.image(b, ImageId(7), WHITE, 2);
    let frames: Vec<(ImageId, u32)> = l
        .cmds
        .iter()
        .filter_map(|c| match c {
            fulgurite::Cmd::Image { id, frame, .. } => Some((*id, *frame)),
            _ => None,
        })
        .collect();
    assert_eq!(frames, [(ImageId(7), 1), (ImageId(7), 2)], "setup: the list must hold one id at frames 1 and 2");

    let full = [0.0, 0.0, W as f32, H as f32];
    g.draw(&l, None, Some((PAGE, full)));
    let gp = g.read_rgba();
    c.clear(PAGE);
    c.render(&l, W, H);
    let (ga, gb) = (px(&gp, W, 24, 24), px(&gp, W, 72, 24));
    let (sa, sb) = (px(&c.pixels, W, 24, 24), px(&c.pixels, W, 72, 24));
    let s = g.atlas_stats();
    println!(
        "frame 1 at A, frame 2 at B: gpu {ga:?} {gb:?}, software {sa:?} {sb:?} (want {RED:?} {BLUE:?}); \
         image_uploads {}",
        s.image_uploads
    );
    assert_eq!((sa, sb), (RED, BLUE), "setup: the software canvas did not draw frames 1 and 2");
    assert_eq!(ga, sa, "A (id 7, frame 1): the GPU differs from software -- the second command rewrote its slot");
    assert_eq!(gb, sb, "B (id 7, frame 2): the GPU differs from software");

    // And a second draw of the same list keeps both, with no new upload.
    g.draw(&l, None, Some((PAGE, full)));
    let gp = g.read_rgba();
    let again = g.atlas_stats().image_uploads;
    println!("redraw: gpu {:?} {:?}, image_uploads {again} (want {})", px(&gp, W, 24, 24), px(&gp, W, 72, 24), s.image_uploads);
    assert_eq!((px(&gp, W, 24, 24), px(&gp, W, 72, 24)), (RED, BLUE), "a redraw lost one of the two frames");
    assert_eq!(again, s.image_uploads, "an unchanged list uploaded again");
}

/// **No atlas churn** (K84d). A 300 × 300, four-frame loop over 100 frame
/// changes after a first draw: every change is written in place, so the
/// atlas never resets, each change is one upload of exactly one frame,
/// and the last draw shows the frame it asked for.
#[test]
fn an_animating_image_never_resets_the_atlas() {
    const S: u32 = 300;
    const W: u32 = 320;
    let Some(mut g) = gpu(W, W) else { return };
    let colours = [RED, BLUE, GREEN, AMBER];
    g.set_images(Box::new(Seq::new(S, &colours)));
    let rect = [10.0, 10.0, 310.0, 310.0];
    let full = [0.0, 0.0, W as f32, W as f32];

    g.draw(&one(rect, 0), None, Some((PAGE, full)));
    let first = px(&g.read_rgba(), W, 160, 160);
    let s0 = g.atlas_stats();
    assert_eq!(first, RED, "setup: the first draw does not show frame 0");
    assert_eq!(s0.image_uploads, 1, "setup: the first draw is one upload");

    // Accept direction: 100 draws at the unchanged frame upload nothing.
    for _ in 0..100 {
        g.draw(&one(rect, 0), None, Some((PAGE, full)));
    }
    let idle = g.atlas_stats();
    println!("unchanged frame, 100 draws: image_uploads {} (want 1), resets {}", idle.image_uploads, idle.resets);
    assert_eq!(idle.image_uploads, 1, "draws at an unchanged frame uploaded again");

    let changes = 100u32;
    let mut wrong = 0;
    for f in 1..=changes {
        g.draw(&one(rect, f), None, Some((PAGE, full)));
        if px(&g.read_rgba(), W, 160, 160) != colours[f as usize % colours.len()] {
            wrong += 1;
        }
    }
    let s = g.atlas_stats();
    let per = s.image_upload_bytes / s.image_uploads.max(1);
    println!(
        "{changes} frame changes: resets {} (bound 0), image_uploads {} (bound 1 + changes = {}), \
         image_upload_bytes / image_uploads {per} (bound {}), draws not showing their frame {wrong} (bound 0)",
        s.resets,
        s.image_uploads,
        1 + changes,
        S * S * 4
    );
    assert_eq!(wrong, 0, "{wrong} of {changes} draws did not show the frame they asked for");
    assert_eq!(s.resets, 0, "the atlas reset {} times under animation: frames are not written in place", s.resets);
    assert_eq!(s.image_uploads, 1 + changes as u64, "uploads are not one per frame change");
    assert_eq!(per, (S * S * 4) as u64, "an upload carried other than one frame");
}

/// **A size change inserts again.** A 300 × 300 frame, then a 200 × 200
/// one, then 300 × 300 again: each draws whole at its own size, and the
/// orphaned space costs at most one reset.
#[test]
fn a_frame_of_another_size_draws_at_its_size() {
    const W: u32 = 320;
    /// Frame `k` is `sides[k]` square, its quadrants TL, TR, BL, BR in
    /// four colours, rotated by `k` so each frame differs everywhere.
    struct Sized {
        frames: Vec<(u32, Vec<u8>)>,
    }
    let quad = [RED, BLUE, GREEN, AMBER];
    let sides = [300u32, 200, 300];
    let frames = sides
        .iter()
        .enumerate()
        .map(|(k, &s)| {
            let mut v = Vec::with_capacity((s * s * 4) as usize);
            for y in 0..s {
                for x in 0..s {
                    let q = (y >= s / 2) as usize * 2 + (x >= s / 2) as usize;
                    v.extend_from_slice(&quad[(q + k) % 4]);
                }
            }
            (s, v)
        })
        .collect();
    impl ImageSource for Sized {
        fn image(&self, id: ImageId, frame: u32) -> Option<(u32, u32, &[u8])> {
            let (s, v) = self.frames.get(frame as usize)?;
            (id == ImageId(7)).then_some((*s, *s, &v[..]))
        }
    }
    let Some(mut g) = gpu(W, W) else { return };
    g.set_images(Box::new(Sized { frames }));
    let rect = [10.0, 10.0, 310.0, 310.0];
    let full = [0.0, 0.0, W as f32, W as f32];
    // Quadrant centres of the drawn rect.
    let at = [(85, 85), (235, 85), (85, 235), (235, 235)];
    for (k, side) in sides.iter().enumerate() {
        g.draw(&one(rect, k as u32), None, Some((PAGE, full)));
        let img = g.read_rgba();
        let got: Vec<[u8; 4]> = at.iter().map(|&(x, y)| px(&img, W, x, y)).collect();
        let want: Vec<[u8; 4]> = (0..4).map(|q| quad[(q + k) % 4]).collect();
        let s = g.atlas_stats();
        println!(
            "frame {k} ({side}x{side}): quadrants {got:?} (want {want:?}); resets {} (bound <= 1), image_uploads {}",
            s.resets, s.image_uploads
        );
        assert_eq!(got, want, "frame {k} ({side}x{side}) did not draw whole at its own size");
        assert!(s.resets <= 1, "a size change reset the atlas {} times", s.resets);
    }
}
