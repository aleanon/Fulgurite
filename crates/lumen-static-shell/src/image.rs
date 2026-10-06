//! **Images an application owns, and the decoders that fill them** —
//! M84/T84.1 (`docs/plan-images-in-motion.md`).
//!
//! The core never decodes (`lumen_static::ImageSource`, and the
//! `Image` widget's module rustdoc): a renderer asks a source for an
//! [`ImageId`]'s RGBA8 pixels per command. This module is the desktop's
//! two halves of that source.
//!
//! - [`ImageStore`], an id → frames map that implements
//!   [`ImageSource`]. **Unconditional**: it needs no decoder, so an
//!   application that decodes its own pixels (or generates them) uses it
//!   with no feature at all. A stream (a video feed, M84b) overwrites an
//!   id's pixels with [`ImageStore::replace`].
//! - [`ImageChain`], the two-source lookup every renderer resolves a
//!   command through since M84/T84.6: the application's store, which a
//!   session hands over each present (`App::images`, through
//!   `FrameSource::images`), and then the window's (`Program::images`).
//! - Behind the shell's default-off `image` feature, `decode_png` and
//!   `decode_gif`, which answer a `DecodedImage`: full-canvas RGBA8
//!   frames and each frame's delay. A GIF's frames are **composited at
//!   decode time** — each one drawn over what its predecessor left, after
//!   that predecessor's disposal — so every frame is a picture on its
//!   own, and showing frame *n* never needs frame *n − 1*. Both are pure
//!   Rust (the `png` and `gif` crates).
//!
//! **The feature gates crates and compile time, not linked bytes.** A
//! build that never calls a decoder links none of it whatever the feature
//! says (M30's finding, `docs/results-widget-crate.md`), which is why
//! `image` is the recorded exception to K2's symbol check and its
//! falsifiable half is the crate count (`scripts/check_default_crates.sh`;
//! `docs/plan-iced-roadmap.md` K2). Its price is in
//! `docs/results-images.md` T84.1. It is not in the ABI stamp's feature
//! list (`session::abi_fields`): nothing here crosses the seam except as
//! `&dyn ImageSource`, a core trait the source hash already covers.
//!
//! **Which frame a store answers.** The frame is in the command since
//! M84/T84.2 (`lumen_static::Cmd::Image`), and [`ImageSource::image`]
//! takes it: a store answers `frames[frame % len]`, so an animated
//! image's tick writes the widget's `frame` and the next present draws
//! that frame, and a stream (one frame, overwritten by
//! [`ImageStore::replace`]) answers its latest pixels whatever counter
//! the command carries.

use std::collections::HashMap;

use lumen_static::display::{ImageId, ImageSource};

/// An id's pixels: one or more RGBA8 frames of one size.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    w: u32,
    h: u32,
    frames: Vec<Vec<u8>>,
    /// How many times [`ImageStore::replace`] has overwritten this id:
    /// the frame a stream's command paints next (M84/T84.6).
    version: u32,
}

/// **Images an application owns**, by [`ImageId`] — what a renderer reads
/// through [`ImageSource`].
///
/// Each id holds one or more RGBA8 frames of one size, straight (not
/// premultiplied) alpha, rows top to bottom, as `Canvas` and the GPU
/// atlas read them. An insert whose pixel count does not match `w × h`
/// is refused rather than stored short: a renderer treats a short slice
/// as missing, so storing one would turn an author's mistake into a
/// silently blank image.
#[derive(Clone, Debug, Default)]
pub struct ImageStore {
    map: HashMap<ImageId, Entry>,
}

impl ImageStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Hold `frames` (each `w × h × 4` bytes) under `id`, replacing what
    /// it held. `false`, and nothing stored, when there are no frames, a
    /// side is zero, or a frame's length is not `w × h × 4`.
    pub fn insert(&mut self, id: ImageId, w: u32, h: u32, frames: Vec<Vec<u8>>) -> bool {
        let want = w as usize * h as usize * 4;
        if w == 0 || h == 0 || frames.is_empty() || frames.iter().any(|f| f.len() != want) {
            return false;
        }
        self.map.insert(id, Entry { w, h, frames, version: 0 });
        true
    }

    /// **A stream's next picture** — overwrite `id` with one frame of
    /// `w × h`, as a video feed does per delivered frame (M84b), and
    /// answer **the frame to paint it at** (M84/T84.6).
    ///
    /// The answer goes into the widget's `frame`, which is what makes the
    /// next present repaint the image: damage is decided from the command
    /// (`Cmd::Image` carries the frame), and the GPU atlas caches by
    /// `(id, frame)`, so new pixels under an unchanged frame would stay
    /// stale on both. It counts this id's replacements from 1, wrapping;
    /// the stored entry holds one frame, so the store answers these
    /// pixels for any frame the command carries.
    ///
    /// The buffer is reused when the size is unchanged, so a steady stream
    /// allocates nothing here. `None`, and the store unchanged, when the
    /// length is not `w × h × 4` or a side is zero: a refused frame keeps
    /// the last picture rather than blanking the image.
    pub fn replace(&mut self, id: ImageId, w: u32, h: u32, pixels: &[u8]) -> Option<u32> {
        if w == 0 || h == 0 || pixels.len() != w as usize * h as usize * 4 {
            return None;
        }
        let version = match self.map.get_mut(&id) {
            Some(e) if e.w == w && e.h == h && e.frames.len() == 1 => {
                e.frames[0].copy_from_slice(pixels);
                e.version = e.version.wrapping_add(1);
                e.version
            }
            prev => {
                let version = prev.map_or(0, |e| e.version).wrapping_add(1);
                self.map.insert(id, Entry { w, h, frames: vec![pixels.to_vec()], version });
                version
            }
        };
        Some(version)
    }

    /// Hold a decoded image under `id`. Behind the shell's `image`
    /// feature, beside the decoders that produce one.
    #[cfg(feature = "image")]
    pub fn insert_decoded(&mut self, id: ImageId, img: DecodedImage) -> bool {
        self.insert(id, img.w, img.h, img.frames)
    }

    /// Stop holding `id`. Whether it was held.
    pub fn remove(&mut self, id: ImageId) -> bool {
        self.map.remove(&id).is_some()
    }

    /// `id`'s size and every frame it holds, in order.
    pub fn frames(&self, id: ImageId) -> Option<(u32, u32, &[Vec<u8>])> {
        self.map.get(&id).map(|e| (e.w, e.h, &e.frames[..]))
    }

    /// How many ids are held.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether no id is held.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl ImageSource for ImageStore {
    /// The id's frame `frame % len` (the module rustdoc says why), or
    /// `None` for an id the store does not hold. Never out of range: an
    /// entry holds at least one frame ([`ImageStore::insert`] refuses
    /// none).
    fn image(&self, id: ImageId, frame: u32) -> Option<(u32, u32, &[u8])> {
        self.map.get(&id).map(|e| (e.w, e.h, &e.frames[frame as usize % e.frames.len()][..]))
    }
}

/// Which store answered a command — the half of the GPU atlas's key that
/// keeps one id in two stores apart (M84/T84.6, second review A14).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ImageOrigin {
    /// The application's store: `App::images`, handed over by the frame.
    App,
    /// The window's store: `Program::images`, read once.
    Window,
}

/// What a store answers for one `(id, frame)`: width, height, RGBA8 —
/// [`ImageSource::image`]'s answer, named for [`ImageChain::resolve`].
pub type Pixels<'a> = (u32, u32, &'a [u8]);

/// **The two sources a renderer resolves an image through, in order** —
/// M84/T84.6 (`docs/plan-images-in-motion.md`).
///
/// The application's store first (`App::images`, which a session hands
/// over through `FrameSource::images` on every present, so a store the
/// application writes is the store the next frame reads), then the
/// window's (`Program::images`, read once into the backend). An id both
/// hold draws the application's pixels; an id only the window holds still
/// draws. `Canvas`, `Offscreen`, the window's software surface and the
/// GPU backend all take one; nothing holds it past a present.
///
/// A session showing M36's panic screen hands over `None`, so its ids
/// fall through to the window's store — which is why the GPU atlas keys a
/// slot on [`ImageOrigin`] as well as the id: a fallen-through id misses
/// instead of hitting the application's pixels.
#[derive(Clone, Copy, Default)]
pub struct ImageChain<'a> {
    /// The frame's store: the application's.
    pub app: Option<&'a dyn ImageSource>,
    /// The window's store.
    pub window: Option<&'a dyn ImageSource>,
}

impl<'a> ImageChain<'a> {
    /// Whether neither source is present: a renderer may then skip
    /// image commands outright (and the software canvas may thread).
    pub fn is_empty(&self) -> bool {
        self.app.is_none() && self.window.is_none()
    }

    /// `id` at `frame`, and which store answered it.
    pub fn resolve(&self, id: ImageId, frame: u32) -> Option<(ImageOrigin, Pixels<'a>)> {
        if let Some(px) = self.app.and_then(|s| s.image(id, frame)) {
            return Some((ImageOrigin::App, px));
        }
        self.window.and_then(|s| s.image(id, frame)).map(|px| (ImageOrigin::Window, px))
    }
}

impl ImageSource for ImageChain<'_> {
    /// [`ImageChain::resolve`] without the origin.
    fn image(&self, id: ImageId, frame: u32) -> Option<(u32, u32, &[u8])> {
        self.resolve(id, frame).map(|(_, px)| px)
    }
}

#[cfg(feature = "image")]
pub use decode::{decode_gif, decode_png, DecodedImage, ImageError, MAX_DECODED_BYTES};

#[cfg(feature = "image")]
mod decode {
    use std::fmt;

    /// The most bytes one decode may produce across all of its frames:
    /// 256 MiB. A GIF's frames are composited to full canvases, so a
    /// small file of many frames over a large screen asks for
    /// `frames × w × h × 4`; past this it answers
    /// [`ImageError::TooLarge`] instead of allocating it.
    pub const MAX_DECODED_BYTES: usize = 256 << 20;

    /// What a decoder answers: full-size RGBA8 frames, straight alpha,
    /// and how long each is shown.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct DecodedImage {
        pub w: u32,
        pub h: u32,
        /// One `w × h × 4` buffer per frame. A PNG has one; a GIF's are
        /// composited (the module rustdoc says how).
        pub frames: Vec<Vec<u8>>,
        /// Each frame's delay in milliseconds, as encoded (a GIF stores
        /// hundredths of a second; a PNG's single frame has 0). Not
        /// clamped: a browser shows a 0 or 10 ms GIF frame for 100 ms,
        /// and that policy belongs to whatever plays the frames.
        pub delays_ms: Vec<u32>,
    }

    /// Why a decode failed. A malformed or truncated file is an error,
    /// never a panic.
    #[derive(Clone, Debug, PartialEq, Eq)]
    #[non_exhaustive]
    pub enum ImageError {
        /// The bytes are not a well-formed file of the format asked for,
        /// or end early. The decoder's own message.
        Malformed(String),
        /// Well-formed, in a form this decoder does not turn into RGBA8.
        Unsupported(&'static str),
        /// The decoded frames would exceed [`MAX_DECODED_BYTES`].
        TooLarge { w: u32, h: u32, frames: usize },
    }

    impl fmt::Display for ImageError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                ImageError::Malformed(m) => write!(f, "malformed image: {m}"),
                ImageError::Unsupported(m) => write!(f, "unsupported image: {m}"),
                ImageError::TooLarge { w, h, frames } => {
                    write!(f, "image too large: {frames} frame(s) of {w}x{h} exceed {MAX_DECODED_BYTES} bytes")
                }
            }
        }
    }

    impl std::error::Error for ImageError {}

    fn too_large(w: u32, h: u32, frames: usize) -> bool {
        (w as usize).saturating_mul(h as usize).saturating_mul(4).saturating_mul(frames)
            > MAX_DECODED_BYTES
    }

    /// Decode a PNG to one RGBA8 frame.
    ///
    /// Palette, grey and 16-bit images are normalised to eight bits a
    /// channel (`png::Transformations::normalize_to_color8`), then grey,
    /// grey-alpha and RGB are widened to RGBA with an opaque alpha. An
    /// animated PNG decodes to its default image only.
    pub fn decode_png(bytes: &[u8]) -> Result<DecodedImage, ImageError> {
        let mal = |e: png::DecodingError| ImageError::Malformed(e.to_string());
        let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
        dec.set_transformations(png::Transformations::normalize_to_color8());
        let mut reader = dec.read_info().map_err(mal)?;
        let (w, h) = {
            let i = reader.info();
            (i.width, i.height)
        };
        if too_large(w, h, 1) {
            return Err(ImageError::TooLarge { w, h, frames: 1 });
        }
        let size = reader.output_buffer_size().ok_or(ImageError::TooLarge { w, h, frames: 1 })?;
        let mut buf = vec![0; size];
        let out = reader.next_frame(&mut buf).map_err(mal)?;
        if out.bit_depth != png::BitDepth::Eight {
            return Err(ImageError::Unsupported("a bit depth other than 8 after normalisation"));
        }
        let src = &buf[..out.buffer_size()];
        let n = w as usize * h as usize;
        let rgba: Vec<u8> = match out.color_type {
            png::ColorType::Rgba => src.to_vec(),
            png::ColorType::Rgb => {
                src.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect()
            }
            png::ColorType::GrayscaleAlpha => {
                src.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect()
            }
            png::ColorType::Grayscale => src.iter().flat_map(|&g| [g, g, g, 255]).collect(),
            png::ColorType::Indexed => {
                return Err(ImageError::Unsupported("an indexed image after normalisation"))
            }
        };
        if rgba.len() != n * 4 {
            return Err(ImageError::Malformed(format!(
                "{} bytes of pixels for {w}x{h}",
                rgba.len()
            )));
        }
        Ok(DecodedImage { w, h, frames: vec![rgba], delays_ms: vec![0] })
    }

    /// Decode a GIF to every frame, **composited** to the logical
    /// screen's size.
    ///
    /// The canvas starts transparent. Each frame is drawn over it at its
    /// offset, skipping its transparent index, and the canvas is copied
    /// out as that frame's picture; then the frame's disposal is applied
    /// before the next is drawn: `Keep` (and the unspecified `Any`)
    /// leaves it, `Background` clears the frame's rect to transparent,
    /// `Previous` restores the canvas as it was before the frame. A frame
    /// partly outside the screen is clipped to it.
    pub fn decode_gif(bytes: &[u8]) -> Result<DecodedImage, ImageError> {
        let mal = |e: gif::DecodingError| ImageError::Malformed(e.to_string());
        let mut opts = gif::DecodeOptions::new();
        opts.set_color_output(gif::ColorOutput::RGBA);
        let mut dec = opts.read_info(bytes).map_err(mal)?;
        let (w, h) = (dec.width() as u32, dec.height() as u32);
        if w == 0 || h == 0 {
            return Err(ImageError::Malformed(format!("a {w}x{h} logical screen")));
        }
        let mut canvas = vec![0u8; w as usize * h as usize * 4];
        let mut frames = Vec::new();
        let mut delays_ms = Vec::new();
        // ANCHOR: composite
        while let Some(f) = dec.read_next_frame().map_err(mal)? {
            if too_large(w, h, frames.len() + 1) {
                return Err(ImageError::TooLarge { w, h, frames: frames.len() + 1 });
            }
            let saved = (f.dispose == gif::DisposalMethod::Previous).then(|| canvas.clone());
            let (fx, fy, fw, fh) = (f.left as u32, f.top as u32, f.width as u32, f.height as u32);
            if f.buffer.len() < fw as usize * fh as usize * 4 {
                return Err(ImageError::Malformed(format!(
                    "a frame of {} bytes for {fw}x{fh}",
                    f.buffer.len()
                )));
            }
            let visible = |x: u32, y: u32| x < w && y < h;
            for y in 0..fh {
                for x in 0..fw {
                    let (cx, cy) = (fx + x, fy + y);
                    let s = ((y * fw + x) * 4) as usize;
                    if visible(cx, cy) && f.buffer[s + 3] != 0 {
                        let d = ((cy * w + cx) * 4) as usize;
                        canvas[d..d + 4].copy_from_slice(&f.buffer[s..s + 4]);
                    }
                }
            }
            frames.push(canvas.clone());
            delays_ms.push(f.delay as u32 * 10);
            match f.dispose {
                gif::DisposalMethod::Background => {
                    for y in fy..(fy + fh).min(h) {
                        for x in fx..(fx + fw).min(w) {
                            let d = ((y * w + x) * 4) as usize;
                            canvas[d..d + 4].fill(0);
                        }
                    }
                }
                gif::DisposalMethod::Previous => canvas = saved.unwrap_or(canvas),
                gif::DisposalMethod::Keep | gif::DisposalMethod::Any => {}
            }
        }
        // ANCHOR_END: composite
        if frames.is_empty() {
            return Err(ImageError::Malformed("a GIF with no frames".into()));
        }
        Ok(DecodedImage { w, h, frames, delays_ms })
    }
}
