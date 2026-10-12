//! **A video's producer** — M84/T84.8 (`docs/plan-images-in-motion.md`,
//! `docs/results-images.md`).
//!
//! The `Video` widget (`fulgurite_widgets::video`) decodes nothing: it
//! shows `(id, frame)` from the application's [`ImageStore`] and reports
//! play, pause and seek as intent. What fills the store is a
//! [`VideoFeed`] — a source of [`VideoFrame`]s — run as one of the
//! application's **subscriptions**, through [`video_job`]:
//!
//! - the application declares `(kind, key)` while it plays, and its
//!   `job_for` answers `video_job::<MyFeed, MyMsg>` for that kind;
//! - the job opens the feed **by the key alone** ([`VideoFeed::open`]),
//!   paces each frame to its `pts_ms`, and sends it as the application's
//!   message; the application's `apply` hands the pixels to
//!   [`ImageStore::replace`] and writes the answer into the widget's
//!   `frame`;
//! - a pause un-declares, and a seek re-declares under a new key
//!   ([`VideoKey`]). The host then tells the old job to stop
//!   (`JobTrack::stop`, M84/T84.7): it returns on
//!   [`Sender::is_stopped`](fulgurite::inbox::Sender::is_stopped), and
//!   the drain drops whatever it sent after the stop, so **no frame from
//!   before a seek lands after it**.
//!
//! **Opened by data, never by a path or a `fn` in flight** (D1(d)): the
//! source and the start position are packed into the subscription's key,
//! so a hot swap re-opens the stream in whatever build is loaded, from
//! values. The trait is not `FrameSource` (what a renderer reads from a
//! frame), which is `Session`'s supertrait.
//!
//! `GifFeed`, behind the shell's `image` feature, is the reference feed:
//! a decoded GIF played at its own delays, so `Video` ships without a real
//! decoder crate — and it does: the GStreamer crate was deferred on K84b
//! (`docs/results-images.md` T84.9–T84.10, with the reopen condition).

use std::time::{Duration, Instant};

use fulgurite::inbox::{SendError, Sender};
use fulgurite::spawn::Descriptor;

#[cfg(doc)]
use crate::image::ImageStore;

/// One picture of a stream, as a feed produces it and a subscription
/// sends it to the application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoFrame {
    /// The subscription key the feed was opened with — so an application
    /// can tell which stream a frame came from. [`video_job`] sets it.
    pub key: u64,
    /// Which picture of the source this is (a GIF's frame index).
    pub index: u32,
    /// When to show it, in ms of stream time from the source's start:
    /// [`video_job`] paces sends to it. Increases across a loop.
    pub pts_ms: u64,
    /// Where it is in the source, `0..duration_ms`: what a time display
    /// and a seek track show.
    pub position_ms: u32,
    /// The source's length in ms; 0 for a stream with no end.
    pub duration_ms: u32,
    pub w: u32,
    pub h: u32,
    /// `w × h × 4` bytes of straight-alpha RGBA8, rows top to bottom —
    /// what [`ImageStore::replace`] takes. Allocated by the feed, on the
    /// worker; the frame thread copies it into the store's reused buffer
    /// and allocates nothing (`alloc_gate.rs`'s video arm).
    pub rgba: Vec<u8>,
}

/// **What a subscription key means to a video** — the source, a seek
/// counter, and where to start, packed into the `u64` a subscription is
/// identified by.
///
/// The counter is there because a key is an identity: a second seek to
/// the position the stream started from would otherwise be the same
/// subscription, still running, and the host would restart nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VideoKey {
    /// Which of the application's sources (its index, not a path).
    pub source: u16,
    /// Bumped by the application per seek, so every seek is a new key.
    pub seek: u16,
    /// Where in the source to start, in ms.
    pub start_ms: u32,
}

impl VideoKey {
    /// The `u64` a subscription is declared under.
    pub const fn pack(self) -> u64 {
        ((self.source as u64) << 48) | ((self.seek as u64) << 32) | self.start_ms as u64
    }

    /// The key a feed was opened with, read back.
    pub const fn unpack(key: u64) -> VideoKey {
        VideoKey { source: (key >> 48) as u16, seek: (key >> 32) as u16, start_ms: key as u32 }
    }
}

/// **A source of video frames**, opened from a subscription key.
///
/// `open` runs on the worker the subscription's job runs on, so a feed
/// needs no `Send`; it is created there and dropped there. `next_frame`
/// answers the next picture in order, or `None` at the end of the stream,
/// which ends the job.
pub trait VideoFeed: Sized {
    /// The feed `key` names ([`VideoKey`] for the usual packing), or
    /// `None` when it names nothing this build can play.
    fn open(key: u64) -> Option<Self>;

    /// The next frame, or `None` at the end.
    fn next_frame(&mut self) -> Option<VideoFrame>;
}

/// **The job a video subscription runs** — a `JobFn` for `job_for` to
/// answer: `video_job::<MyFeed, MyMsg>`.
///
/// Opens `F` from the descriptor's payload, then for each frame: returns
/// if the host has stopped it, sleeps until the frame's `pts_ms` (from
/// the first frame's), returns if it was stopped during the sleep, and
/// sends it as `M`. A `Disconnected` send — the host is gone, as after a
/// hot swap drops v1's inbox — ends it too. A `Full` send drops that
/// frame and carries on: the frame loop is behind, the inbox counted the
/// refusal (`Inbox::refused`), and the next frame is the one worth
/// showing.
///
/// **Paced to `pts_ms`**, never as fast as the feed can decode: a feed
/// with no sleep fills the inbox's 256 slots in a moment and the rest
/// are refused (`docs/results-images.md` T84.8's pacing gate).
pub fn video_job<F: VideoFeed, M: From<VideoFrame> + Send + 'static>(d: Descriptor, tx: &Sender<M>) {
    let Some(mut feed) = F::open(d.payload) else { return };
    let t0 = Instant::now();
    let mut first: Option<u64> = None;
    loop {
        if tx.is_stopped() {
            return;
        }
        let Some(mut f) = feed.next_frame() else { return };
        f.key = d.payload;
        let base = *first.get_or_insert(f.pts_ms);
        let due = t0 + Duration::from_millis(f.pts_ms.saturating_sub(base));
        let now = Instant::now();
        if due > now {
            std::thread::sleep(due - now);
        }
        if tx.is_stopped() {
            return;
        }
        if let Err(SendError::Disconnected(_)) = tx.send(M::from(f)) {
            return;
        }
    }
}

#[cfg(feature = "image")]
pub use gif_feed::{GifFeed, GifLibrary};

#[cfg(feature = "image")]
mod gif_feed {
    use super::{VideoFeed, VideoFrame, VideoKey};
    use crate::image::DecodedImage;

    /// **Where a [`GifFeed`] finds its GIFs**: the application's sources,
    /// by index — the `source` of a [`VideoKey`].
    ///
    /// An associated function, not a value, because a feed is opened on a
    /// worker from a key alone: whatever it reads must be reachable
    /// without anything carried in flight. Decode once and hand out the
    /// same `&'static` (a `OnceLock`), since every seek re-opens the feed.
    pub trait GifLibrary {
        /// Source `index`, decoded; `None` for an index this build lacks.
        fn gif(index: u16) -> Option<&'static DecodedImage>;
    }

    /// **The reference feed**: a decoded GIF, played at its own delays and
    /// looping, from the position its key names.
    ///
    /// A delay of 10 ms or less plays at 100 ms, the rule
    /// `fulgurite::Playback` and browsers apply. Each frame is a copy
    /// of the decoded picture (a GIF's frames are composited at decode
    /// time, so any one stands alone); the copy is made on the worker.
    pub struct GifFeed<L> {
        img: &'static DecodedImage,
        delays: Vec<u32>,
        duration_ms: u32,
        /// The frame `next_frame` answers next.
        next: usize,
        /// Its stream time.
        pts_ms: u64,
        /// Where in the source the first frame is shown from: the key's
        /// start, not its frame's, so a seek to 75% reads 75%.
        start_ms: Option<u32>,
        _lib: core::marker::PhantomData<fn() -> L>,
    }

    impl<L: GifLibrary> GifFeed<L> {
        /// The source's length in ms, at the delays it plays at.
        pub fn duration_ms(&self) -> u32 {
            self.duration_ms
        }
    }

    impl<L: GifLibrary> VideoFeed for GifFeed<L> {
        fn open(key: u64) -> Option<Self> {
            let k = VideoKey::unpack(key);
            let img = L::gif(k.source)?;
            if img.frames.is_empty() {
                return None;
            }
            let delays: Vec<u32> = (0..img.frames.len())
                .map(|i| match img.delays_ms.get(i).copied().unwrap_or(0) {
                    d if d <= 10 => fulgurite::Playback::SHORT_DELAY_MS as u32,
                    d => d,
                })
                .collect();
            let duration_ms: u32 = delays.iter().sum();
            let start = k.start_ms % duration_ms.max(1);
            // The frame showing at `start`: the last whose start is at or
            // before it.
            let (mut next, mut at) = (0, 0u32);
            while next + 1 < delays.len() && at + delays[next] <= start {
                at += delays[next];
                next += 1;
            }
            Some(GifFeed {
                img,
                delays,
                duration_ms,
                next,
                pts_ms: start as u64,
                start_ms: Some(start),
                _lib: core::marker::PhantomData,
            })
        }

        fn next_frame(&mut self) -> Option<VideoFrame> {
            let i = self.next;
            let pts = self.pts_ms;
            let position_ms = self.start_ms.take().unwrap_or((pts % self.duration_ms.max(1) as u64) as u32);
            // The next frame starts where this one's delay ends, measured
            // from this frame's own start, not from a mid-frame seek.
            let frame_start: u32 = self.delays[..i].iter().sum();
            let into = position_ms.saturating_sub(frame_start);
            self.pts_ms = pts + (self.delays[i].saturating_sub(into)) as u64;
            self.next = (i + 1) % self.delays.len();
            Some(VideoFrame {
                key: 0,
                index: i as u32,
                pts_ms: pts,
                position_ms,
                duration_ms: self.duration_ms,
                w: self.img.w,
                h: self.img.h,
                rgba: self.img.frames[i].clone(),
            })
        }
    }
}
