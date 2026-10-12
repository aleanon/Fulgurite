//! Values that change over time — `docs/plan-capabilities.md` T15.4.
//!
//! # Why this is `no_std` and needs no clock
//!
//! The core has no clock and cannot have one: `std::time` is not
//! available and an embedded target's timer is a peripheral. So it does
//! not ask what time it is — the **host** says, once a frame, and
//! everything here is arithmetic on the number it gave.
//!
//! That inverts the usual shape, in which a framework owns a timer and
//! widgets subscribe to it, and it is what lets animation cost nothing
//! on a target with no allocator and no operating system.
//!
//! # What it costs when nothing animates
//!
//! Nothing. There is no per-frame pass and no registry: an
//! [`Animated<T>`] is a field on a widget, so a tree with none has no
//! animation state at all. The gating policy's second mechanism — a
//! type, not a feature.
//!
//! A frame loop asks [`Animated::settled`] whether to schedule another
//! frame. A UI that is not animating idles, which is the property
//! `docs/results-frame-cost.md`'s idle numbers depend on.

use crate::num::clamp;

/// How a value moves between two points.
///
/// `const fn`s over a `dyn Fn`: an easing is arithmetic, and a function
/// pointer per animated value would be eight bytes and an indirect call
/// for something the optimizer can fold.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Easing {
    Linear,
    /// Slow to start. The default for something entering.
    EaseIn,
    /// Slow to stop. The default for something leaving.
    EaseOut,
    /// Slow at both ends — the one to reach for when unsure.
    #[default]
    EaseInOut,
}

impl Easing {
    /// Map linear progress `t` in 0..=1 to eased progress.
    ///
    /// Cubic, because it is the cheapest curve that reads as smooth and
    /// needs no transcendental function — which matters on a target
    /// whose FPU is single-precision and whose `libm` is absent.
    pub fn apply(self, t: f32) -> f32 {
        let t = clamp(t, 0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t * t,
            Easing::EaseOut => {
                let u = 1.0 - t;
                1.0 - u * u * u
            }
            Easing::EaseInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    let u = -2.0 * t + 2.0;
                    1.0 - u * u * u / 2.0
                }
            }
        }
    }
}

/// A value that interpolates.
///
/// Implemented for `f32` and for [`crate::Color`], which is what a UI
/// animates: positions, sizes, opacities and colours.
pub trait Lerp: Copy {
    fn lerp(self, to: Self, t: f32) -> Self;
}

impl Lerp for f32 {
    fn lerp(self, to: Self, t: f32) -> Self {
        self + (to - self) * t
    }
}

impl Lerp for crate::display::Color {
    fn lerp(self, to: Self, t: f32) -> Self {
        let c = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
        crate::display::Color(
            c(self.0, to.0),
            c(self.1, to.1),
            c(self.2, to.2),
            c(self.3, to.3),
        )
    }
}

/// A value moving from one point to another over a duration.
///
/// Time is in **milliseconds**, as an `f32`, supplied by the host. Not
/// `Duration`: it is `std`, and a frame loop already has an elapsed
/// number to hand.
#[derive(Clone, Copy, Debug)]
pub struct Animated<T: Lerp> {
    from: T,
    to: T,
    /// How long the whole move takes, in ms.
    pub duration: f32,
    pub easing: Easing,
    /// How far through, in ms. At `>= duration` the value is `to`.
    elapsed: f32,
}

impl<T: Lerp + Default> Default for Animated<T> {
    fn default() -> Self {
        Animated::new(T::default())
    }
}

impl<T: Lerp> Animated<T> {
    /// A value at rest.
    pub fn new(at: T) -> Self {
        Animated { from: at, to: at, duration: 200.0, easing: Easing::default(), elapsed: 200.0 }
    }

    /// The value now.
    pub fn get(&self) -> T {
        if self.settled() {
            return self.to;
        }
        self.from.lerp(self.to, self.easing.apply(self.elapsed / self.duration))
    }

    /// Whether it has arrived. A frame loop that sees `true` everywhere
    /// can stop asking for frames.
    pub fn settled(&self) -> bool {
        // Spelled positively so NaN lands in the "settled" arm: an
        // animation with a NaN duration must stop, not run forever. A NaN
        // *elapsed* cannot arise: `tick` treats a NaN `dt` as 0 (M47/T47.5).
        !self.duration.is_finite() || self.duration <= 0.0 || self.elapsed >= self.duration
    }

    /// Move towards `to`, **retargeting from where it is now** rather
    /// than from where the last move started.
    ///
    /// That is what makes an interrupted animation look right: a panel
    /// told to close halfway through opening should close from halfway,
    /// not jump to open and then close. Retargeting to the value it is
    /// already heading for does nothing, so a binding may call this
    /// every frame.
    pub fn go(&mut self, to: T)
    where
        T: PartialEq,
    {
        if self.to == to {
            return;
        }
        self.from = self.get();
        self.to = to;
        self.elapsed = 0.0;
    }

    /// Jump, with no animation.
    pub fn set(&mut self, to: T) {
        self.from = to;
        self.to = to;
        self.elapsed = self.duration;
    }

    /// Advance by `dt` milliseconds. Returns whether the value changed,
    /// so a caller knows whether the frame needs repainting.
    ///
    /// `dt` is guarded rather than trusted: a stall's huge `dt` lands on
    /// the target, a negative one moves nothing, and **a NaN one moves
    /// nothing** (M47/T47.5) — it used to leave the value unsettled for
    /// ever, since a NaN `elapsed` is never past the duration.
    pub fn tick(&mut self, dt: f32) -> bool {
        if self.settled() {
            return false;
        }
        // Guarded rather than trusted: a host that stalls hands back a
        // huge `dt`, and a negative one is a clock that went backwards.
        // Both must land the value somewhere sane rather than off the
        // end of the curve. **A NaN advances nothing** (M47/T47.5,
        // R7-F8): `clamp` hands a NaN back unchanged, and a NaN
        // `elapsed` is never `>= duration`, so the value never settled
        // and a loop asking for frames while anything animates never
        // went idle. Treated as 0, as `Timers::advance` refuses a
        // non-finite `dt`.
        let dt = if dt.is_nan() { 0.0 } else { dt };
        self.elapsed += clamp(dt, 0.0, self.duration);
        true
    }

    /// Where it is heading.
    pub fn target(&self) -> T {
        self.to
    }
}

/// **An animated image's clock** — M84/T84.3
/// (`docs/plan-images-in-motion.md`, `docs/results-images.md`).
///
/// It holds each frame's delay, how far into the current frame the clock
/// is, and whether it is playing; the frame it answers is what an
/// application writes into an `Image`'s `frame` field from its `App::tick`.
/// The widget holds only that index — an animated image is an `Image`
/// whose `frame` moves, not a second widget type — and the pixels for
/// every frame are the host's (`ImageSource::image(id, frame)`).
///
/// **It answers a frame only when the frame changes**, so a tick that
/// forwards [`Playback::tick`] satisfies `App::tick`'s contract without a
/// thought: a tick that wrote the tree answers `true`, and a zero `dt`
/// moves nothing and answers `None`. Answering on every positive `dt`
/// instead would mark the whole tree at the display's rate for an image
/// that changes ten times a second.
///
/// **And it says when the next change is due** ([`Playback::next_change_ms`]),
/// which is what `App::wake_in_ms` answers: the window sleeps until that
/// instant instead of ticking at vsync, and a paused or single-frame
/// image asks for no frame at all.
///
/// Behind `alloc`: the delays are a `Vec`, sized once at construction.
/// Nothing here allocates afterwards.
// ANCHOR: playback
#[cfg(feature = "alloc")]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Playback {
    /// Each frame's delay in ms, after [`Playback::new`]'s policy.
    delays: alloc::vec::Vec<f32>,
    /// The frame shown now.
    frame: u32,
    /// How far into `frame` the clock is, in ms.
    elapsed: f32,
    /// Whether the clock runs. A paused playback holds its frame and
    /// answers no deadline. Written directly — a pause button flips it.
    pub playing: bool,
}

#[cfg(feature = "alloc")]
impl Playback {
    /// A frame's delay, in ms, when the image asks for 10 ms or less —
    /// what browsers show a 0 or 10 ms GIF frame for. A decoder reports
    /// delays as encoded (`DecodedImage::delays_ms`), so the policy lives
    /// here, with what plays the frames; and a zero delay played as zero
    /// would never leave its frame.
    pub const SHORT_DELAY_MS: f32 = 100.0;

    /// A playback of `delays_ms.len()` frames, at frame 0, **playing**.
    /// A delay of 10 ms or less plays as [`Playback::SHORT_DELAY_MS`].
    pub fn new(delays_ms: &[u32]) -> Self {
        let delays =
            delays_ms.iter().map(|&d| if d <= 10 { Self::SHORT_DELAY_MS } else { d as f32 }).collect();
        Playback { delays, frame: 0, elapsed: 0.0, playing: true }
    }

    /// The frame shown now: what the application writes into the image.
    pub fn frame(&self) -> u32 {
        self.frame
    }

    /// How many frames it cycles through.
    pub fn len(&self) -> usize {
        self.delays.len()
    }

    /// Whether it has no frames at all.
    pub fn is_empty(&self) -> bool {
        self.delays.is_empty()
    }

    /// Advance by `dt` ms; **the new frame, only when it changed**.
    ///
    /// `None` while paused, for fewer than two frames, for a zero,
    /// negative or NaN `dt` (guarded as [`Animated::tick`] guards it), and
    /// when the clock lands back on the frame it left. A stall's huge `dt`
    /// is reduced by whole cycles first, so it costs one pass over the
    /// delays, not one per cycle missed.
    pub fn tick(&mut self, dt: f32) -> Option<u32> {
        if !self.playing || self.delays.len() < 2 || dt.is_nan() || dt <= 0.0 {
            return None;
        }
        let was = self.frame;
        let cycle: f32 = self.delays.iter().sum();
        let mut left = self.elapsed + dt;
        if left >= cycle {
            left %= cycle;
        }
        // `left` is now under one cycle, so this walks at most once round.
        let mut k = self.frame as usize;
        while left >= self.delays[k] {
            left -= self.delays[k];
            k = (k + 1) % self.delays.len();
        }
        self.frame = k as u32;
        self.elapsed = left;
        (self.frame != was).then_some(self.frame)
    }

    /// How long until the next frame change, in ms, measured from the last
    /// [`Playback::tick`] — what `App::wake_in_ms` answers. `None` while
    /// paused or for fewer than two frames: nothing will change, so
    /// nothing needs a frame.
    pub fn next_change_ms(&self) -> Option<f32> {
        if !self.playing || self.delays.len() < 2 {
            return None;
        }
        Some((self.delays[self.frame as usize] - self.elapsed).max(0.0))
    }
}
// ANCHOR_END: playback

#[cfg(all(test, feature = "alloc"))]
mod playback_tests {
    use super::Playback;

    /// **Timing** — M84/T84.3's first gate. A `[100; 4]` playback starts
    /// at frame 0; `tick(0)` answers `None` and moves nothing (the
    /// `App::tick` contract); four `tick(100)` answer 1, 2, 3, 0 (P5).
    /// At 50 ms steps every change is answered exactly once: the
    /// injection that answers `Some` on every positive `dt` prints
    /// duplicates there.
    #[test]
    fn a_frame_is_answered_once_when_it_changes() {
        let mut p = Playback::new(&[100; 4]);
        let start = p.frame();
        let zero = p.tick(0.0);
        let steps: [Option<u32>; 4] = core::array::from_fn(|_| p.tick(100.0));
        println!("start {start}; tick(0) -> {zero:?}; four tick(100) -> {steps:?} (want 0; None; [1, 2, 3, 0])");
        assert_eq!(start, 0, "setup: a playback starts at frame 0");
        assert_eq!(zero, None, "tick(0) answered a frame: a zero dt must move nothing (App::tick's contract)");
        assert_eq!(p.frame(), 0, "after a whole cycle the playback is back at frame 0");
        assert_eq!(steps, [Some(1), Some(2), Some(3), Some(0)], "four 100 ms ticks over [100; 4]");

        let mut q = Playback::new(&[100; 4]);
        let halves: alloc::vec::Vec<Option<u32>> = (0..8).map(|_| q.tick(50.0)).collect();
        let answered: alloc::vec::Vec<u32> = halves.iter().flatten().copied().collect();
        println!("eight tick(50) -> {halves:?}; answered {answered:?} (want [1, 2, 3, 0], no duplicates)");
        assert_eq!(answered, [1, 2, 3, 0], "at 50 ms steps a frame was answered twice, or missed");
    }

    /// A paused or single-frame playback answers no frame and no deadline;
    /// a deadline while playing is what is left of the current frame.
    #[test]
    fn the_deadline_is_what_is_left_of_the_frame() {
        let mut p = Playback::new(&[100, 40, 0]);
        assert_eq!(p.next_change_ms(), Some(100.0));
        assert_eq!(p.tick(30.0), None);
        let left = p.next_change_ms();
        println!("after 30 ms of a 100 ms frame: next change in {left:?} ms (want 70)");
        assert_eq!(left, Some(70.0));
        assert_eq!(p.tick(70.0), Some(1));
        assert_eq!(p.next_change_ms(), Some(40.0));
        // A zero delay plays as the short-delay policy, not as zero.
        assert_eq!(p.tick(40.0), Some(2));
        assert_eq!(p.next_change_ms(), Some(Playback::SHORT_DELAY_MS));
        p.playing = false;
        assert_eq!((p.tick(500.0), p.next_change_ms()), (None, None), "paused: no frame and no deadline");
        let mut one = Playback::new(&[100]);
        assert_eq!((one.tick(500.0), one.next_change_ms()), (None, None), "one frame never changes");
        // A stall: 10 000.5 cycles of 240 ms lands mid-way, in one pass.
        let mut s = Playback::new(&[100, 40, 100]);
        assert_eq!(s.tick(240.0 * 10_000.0 + 120.0), Some(1));
        assert!(s.tick(f32::NAN).is_none() && s.tick(-5.0).is_none(), "NaN and negative dt move nothing");
    }
}
