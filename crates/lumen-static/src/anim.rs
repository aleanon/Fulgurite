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
