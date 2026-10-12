//! M51/T51.1 (R9-F2): the software canvas draws `Cmd::Image` to the
//! exact pixel — the software twin of the GPU parity's image region.
//!
//! No pixel corpus held an image until M51, so a canvas that drew none
//! left every suite green (the review's `7415a9d`: 776 tests). This runs
//! in `just test`, so a machine with no wgpu build still sees it. The
//! expected value of every interior pixel is computed here from the
//! texel the placement's integer scale names and the blend `Canvas`
//! documents (straight alpha over, `(s·a + d·(255 − a)) / 255`, with the
//! tint multiplied in first) — not read back from a render.

use fulgurite::display::{Color, DisplayList, ImageId, ImageSource, Renderer};
use fulgurite_shell::software::Canvas;

const W: u32 = 200;
const H: u32 = 80;
const PAGE: Color = Color(250, 250, 250, 255);
const CARD: u32 = 16;

/// Four quadrants of distinct colour, one half transparent, inside a
/// one-texel dark frame — the same card `fulgurite-wgpu/tests/parity.rs`
/// draws.
fn texel(x: u32, y: u32) -> [u8; 4] {
    if x == 0 || y == 0 || x == CARD - 1 || y == CARD - 1 {
        [20, 20, 30, 255]
    } else {
        match (x < CARD / 2, y < CARD / 2) {
            (true, true) => [220, 40, 40, 255],
            (false, true) => [40, 180, 60, 255],
            (true, false) => [40, 80, 220, 255],
            (false, false) => [240, 200, 0, 128],
        }
    }
}

struct Card(Vec<u8>);

impl ImageSource for Card {
    fn image(&self, _: ImageId, _: u32) -> Option<(u32, u32, &[u8])> {
        Some((CARD, CARD, &self.0))
    }
}

fn card() -> Card {
    let mut px = Vec::new();
    for y in 0..CARD {
        for x in 0..CARD {
            px.extend_from_slice(&texel(x, y));
        }
    }
    Card(px)
}

/// `(name, origin, integer scale, tint)`.
const CASES: [(&str, (u32, u32), u32, Color); 4] = [
    ("1:1", (10, 10), 1, Color(255, 255, 255, 255)),
    ("2x", (40, 10), 2, Color(255, 255, 255, 255)),
    ("tinted 3x", (80, 10), 3, Color(255, 160, 60, 255)),
    ("alpha", (140, 10), 2, Color(255, 255, 255, 128)),
];

#[test]
fn every_interior_pixel_of_an_image_is_the_texel_it_names() {
    let mut l = DisplayList::new();
    for (_, (x0, y0), k, tint) in CASES {
        let s = (CARD * k) as f32;
        l.image([x0 as f32, y0 as f32, x0 as f32 + s, y0 as f32 + s], ImageId(1), tint, 0);
    }
    let mut c = Canvas::new(W, H);
    c.images = Some(Box::new(card()));
    c.clear(PAGE);
    c.render(&l, W, H);

    let m = |a: u8, b: u8| ((a as u32 * b as u32) / 255) as u8;
    for (name, (x0, y0), k, tint) in CASES {
        let side = CARD * k;
        let mut wrong = 0;
        let mut first = None;
        for dy in 0..side {
            for dx in 0..side {
                let t = texel(dx / k, dy / k);
                let src = [m(t[0], tint.0), m(t[1], tint.1), m(t[2], tint.2)];
                let a = m(t[3], tint.3) as u32;
                let page = [PAGE.0, PAGE.1, PAGE.2];
                let want: Vec<u8> =
                    (0..3).map(|i| ((src[i] as u32 * a + page[i] as u32 * (255 - a)) / 255) as u8).collect();
                let i = (((y0 + dy) * W + x0 + dx) * 4) as usize;
                if c.pixels[i..i + 3] != want[..] {
                    wrong += 1;
                    first.get_or_insert((dx, dy, c.pixels[i..i + 3].to_vec(), want.clone()));
                }
            }
        }
        println!("image {name}: {wrong} of {} px differ from their texel (bound 0)", side * side);
        assert_eq!(wrong, 0, "image {name}: first wrong pixel (dx, dy, got, want) {first:?}");
    }
}
