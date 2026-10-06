//! **PNG and GIF into RGBA8, and into a store** — M84/T84.1
//! (`docs/plan-images-in-motion.md`).
//!
//! Every input here is encoded by the test with the decoders' own crates
//! (`png`'s and `gif`'s encoders), so the expected pixels are the ones
//! written, not ones read back from a decode.

use std::borrow::Cow;

use lumen_static::display::{ImageId, ImageSource};
use lumen_static_shell::image::{decode_gif, decode_png, ImageError, ImageStore};

/// A 7 × 5 RGBA pattern in which every channel of every pixel differs,
/// alpha included, so a dropped or reordered channel shows.
fn pattern() -> (u32, u32, Vec<u8>) {
    let (w, h) = (7u32, 5u32);
    let mut px = Vec::new();
    for y in 0..h {
        for x in 0..w {
            px.extend_from_slice(&[
                (x * 30) as u8,
                (y * 50) as u8,
                (x * 7 + y * 11) as u8,
                (40 + x * 20 + y * 3) as u8,
            ]);
        }
    }
    (w, h, px)
}

fn png_of(w: u32, h: u32, px: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().unwrap();
    wr.write_image_data(px).unwrap();
    wr.finish().unwrap();
    out
}

#[test]
fn a_png_round_trips_to_the_bytes_it_was_encoded_from() {
    let (w, h, px) = pattern();
    let d = decode_png(&png_of(w, h, &px)).expect("a well-formed PNG decodes");
    let len = d.frames.first().map_or(0, Vec::len);
    println!(
        "png: {}x{} ({} frame), {len} bytes (want {w}x{h}, {} bytes)",
        d.w,
        d.h,
        d.frames.len(),
        px.len()
    );
    assert_eq!((d.w, d.h, d.frames.len()), (w, h, 1), "setup: size or frame count");
    assert_eq!(len, px.len(), "length: the decoded frame is not w x h x 4 bytes");
    let differ = d.frames[0].iter().zip(&px).filter(|(a, b)| a != b).count();
    println!("png: {differ} of {} bytes differ from the encoded pattern (bound 0)", px.len());
    assert_eq!(differ, 0, "the decoded bytes differ from the encoded pattern");
}

const SCREEN: u16 = 8;
const PALETTE: [u8; 12] = [
    200, 40, 40, // 0: red
    40, 160, 60, // 1: green
    40, 60, 220, // 2: blue
    0, 0, 0, // 3: transparent
];

/// Four frames over an 8 × 8 screen, 100 ms each: red, green, then a blue
/// 2 × 2 at the top-left only (disposal `Keep`), then red again.
fn four_frame_gif() -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = gif::Encoder::new(&mut out, SCREEN, SCREEN, &PALETTE).unwrap();
        let full = |i: u8| vec![i; (SCREEN as usize).pow(2)];
        let frames: [(u16, u16, Vec<u8>); 4] = [
            (SCREEN, SCREEN, full(0)),
            (SCREEN, SCREEN, full(1)),
            (2, 2, vec![2; 4]),
            (SCREEN, SCREEN, full(0)),
        ];
        for (fw, fh, idx) in frames {
            let f = gif::Frame {
                width: fw,
                height: fh,
                delay: 10,
                dispose: gif::DisposalMethod::Keep,
                transparent: Some(3),
                buffer: Cow::Owned(idx),
                ..gif::Frame::default()
            };
            enc.write_frame(&f).unwrap();
        }
    }
    out
}

fn px(frame: &[u8], x: usize, y: usize) -> [u8; 4] {
    let i = (y * SCREEN as usize + x) * 4;
    frame[i..i + 4].try_into().unwrap()
}

#[test]
fn a_gif_frame_that_covers_part_of_the_screen_keeps_what_was_under_it() {
    let d = decode_gif(&four_frame_gif()).expect("a well-formed GIF decodes");
    println!(
        "gif: {}x{}, {} frames, delays {:?} (bound [100; 4])",
        d.w,
        d.h,
        d.frames.len(),
        d.delays_ms
    );
    assert_eq!((d.w, d.h, d.frames.len()), (8, 8, 4), "setup: screen size or frame count");
    assert!(d.frames.iter().all(|f| f.len() == 8 * 8 * 4), "setup: a frame is not the full screen");
    assert_eq!(d.delays_ms, [100; 4], "delays");
    let (corner, under, before) =
        (px(&d.frames[2], 0, 0), px(&d.frames[2], 4, 4), px(&d.frames[1], 4, 4));
    println!("gif: frame 2 at (0,0) {corner:?} (its own blue); at (4,4) {under:?}, frame 1's there {before:?}");
    assert_eq!(corner, [40, 60, 220, 255], "setup: frame 2 did not draw its own 2x2");
    assert_eq!(under, before, "frame 2 outside its 2x2 is not what frame 1 left (uncomposited?)");
    assert_eq!(under, [40, 160, 60, 255], "frame 1's green");
}

/// **The plan's gate is the GIF** (T84.1): every prefix answers an error.
/// The PNG is held to "never panics", and to "an answer from a prefix is
/// the whole picture": `png` accepts a file whose only missing bytes are
/// the end chunk's tail, after every pixel has been read, and decodes it
/// exactly — lenient, not wrong (`docs/plan-images-in-motion.md`, "What
/// building found").
#[test]
fn every_truncation_is_an_error_and_never_a_panic() {
    type Dec = fn(&[u8]) -> Result<Vec<Vec<u8>>, ImageError>;
    let gif = four_frame_gif();
    let (w, h, p) = pattern();
    let png = png_of(w, h, &p);
    let cases: [(&str, &[u8], Dec, bool); 2] = [
        ("gif", &gif, |b| decode_gif(b).map(|d| d.frames), true),
        ("png", &png, |b| decode_png(b).map(|d| d.frames), false),
    ];
    for (name, bytes, dec, strict) in cases {
        let whole = dec(bytes).expect("setup: the whole file does not decode");
        let mut panics = Vec::new();
        let mut oks = Vec::new();
        let mut wrong = Vec::new();
        for n in 0..bytes.len() {
            match std::panic::catch_unwind(|| dec(&bytes[..n])) {
                Err(_) => panics.push(n),
                Ok(Ok(f)) => {
                    oks.push(n);
                    if f != whole {
                        wrong.push(n);
                    }
                }
                Ok(Err(_)) => {}
            }
        }
        println!(
            "{name}: {} prefixes tried (0..{}), panics {} (bound 0), decoded anyway {oks:?} (bound {}), \
             of those not the whole picture {wrong:?} (bound 0)",
            bytes.len(),
            bytes.len(),
            panics.len(),
            if strict { "none" } else { "the end chunk's tail only" },
        );
        assert!(
            panics.is_empty(),
            "{name}: a truncated file panicked at prefix length(s) {panics:?}"
        );
        assert!(
            wrong.is_empty(),
            "{name}: a truncated file decoded to a different picture at {wrong:?}"
        );
        if strict {
            assert!(oks.is_empty(), "{name}: a truncated file decoded at prefix length(s) {oks:?}");
        } else {
            assert!(
                oks.iter().all(|&n| n + 12 > bytes.len()),
                "{name}: decoded before the end chunk at {oks:?}"
            );
        }
    }
}

#[test]
fn a_store_holds_what_it_was_given_and_answers_the_frame_asked_for() {
    let d = decode_gif(&four_frame_gif()).unwrap();
    let mut s = ImageStore::new();
    assert!(s.insert_decoded(ImageId(3), d.clone()), "a decoded GIF is refused");
    let (w, h, first) = s.image(ImageId(3), 0).expect("the id is held");
    let held = s.frames(ImageId(3)).map_or(0, |(_, _, f)| f.len());
    println!("store: {w}x{h}, answers {} bytes, holds {held} frames", first.len());
    assert_eq!((w, h, first, held), (8, 8, &d.frames[0][..], 4));
    // M84/T84.2: `frames[frame % len]`. Frame 2 is its own picture (the
    // compositing test above keeps frames 1 and 2 distinct), and frame 5
    // wraps to 1.
    assert_ne!(d.frames[2], d.frames[0], "setup: frames 0 and 2 must differ for this to see anything");
    let at = |f: u32| s.image(ImageId(3), f).map(|(_, _, p)| p.to_vec());
    let which = |f: u32| (0..4).find(|&k| at(f).as_deref() == Some(&d.frames[k][..]));
    println!("store: frame 2 answers frame {:?}, frame 5 answers frame {:?} (want 2 and 1)", which(2), which(5));
    assert_eq!(at(2).as_deref(), Some(&d.frames[2][..]), "frame 2 did not answer the id's third frame");
    assert_eq!(at(5).as_deref(), Some(&d.frames[1][..]), "frame 5 did not wrap to frame 1 (5 % 4)");
    assert!(s.image(ImageId(4), 0).is_none(), "an id the store does not hold answered");
    // Refusals: a short frame, a zero side, no frames.
    assert!(!s.insert(ImageId(5), 2, 2, vec![vec![0; 15]]), "a short frame was stored");
    assert!(!s.insert(ImageId(5), 0, 2, vec![vec![]]), "a zero-width image was stored");
    assert!(!s.insert(ImageId(5), 2, 2, vec![]), "an image with no frames was stored");
    assert!(s.image(ImageId(5), 0).is_none(), "a refused insert left something behind");
    // A stream's next picture replaces in place, and answers the frame to
    // paint it at (M84/T84.6): 1, then 2, and a refused one moves nothing.
    assert_eq!(s.replace(ImageId(6), 1, 1, &[1, 2, 3, 4]), Some(1));
    assert_eq!(s.replace(ImageId(6), 1, 1, &[5, 6, 7, 8]), Some(2), "the second picture did not move the frame");
    assert_eq!(s.replace(ImageId(6), 1, 1, &[9, 9]), None, "a short stream frame was taken");
    assert_eq!(s.image(ImageId(6), 0), Some((1, 1, &[5u8, 6, 7, 8][..])));
    // A stream's counter is not an index: one frame answers at any count.
    assert_eq!(s.image(ImageId(6), 9), Some((1, 1, &[5u8, 6, 7, 8][..])), "a stream did not answer its latest at frame 9");
    assert_eq!(s.len(), 2);
}
