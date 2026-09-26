//! Recognizers an application owns — `docs/plan-capabilities.md` T15.5.
//!
//! # Why these are types and not a framework pass
//!
//! Recognition is stateful: a pinch is two touches over time, a long
//! press is one touch and a clock. A pass that recognised them would
//! run for every tree, including the overwhelming majority that want
//! none — which the M15 gating policy forbids. So they are structs a
//! widget or an application holds, fed the same `Event::Touch` stream
//! everything else sees, and a tree that holds none carries nothing.
//!
//! All of it is `no_std`: fixed arrays, no clock of its own. [`LongPress`]
//! takes elapsed time the way [`crate::Animated`] does, because the core
//! still has no timer and still should not.

use crate::desc::Point;
use crate::event::{Event, TouchPhase};
use crate::num::clamp;

/// How many simultaneous touches a recognizer tracks.
///
/// Two, because every gesture here is one- or two-fingered and a third
/// finger is not part of any of them. A recognizer that wants more is a
/// different recognizer, and this array is what keeps these `no_std`.
pub const MAX_TOUCHES: usize = 2;

/// The live touches, in order of arrival.
#[derive(Clone, Copy, Debug, Default)]
pub struct Touches {
    ids: [u32; MAX_TOUCHES],
    at: [Point; MAX_TOUCHES],
    len: usize,
}

impl Touches {
    /// Feed one event. Returns whether the set changed.
    pub fn feed(&mut self, ev: &Event) -> bool {
        let Event::Touch { id, x, y, phase } = *ev else { return false };
        let found = (0..self.len).find(|&k| self.ids[k] == id);
        match phase {
            TouchPhase::Begin => {
                if found.is_some() || self.len == MAX_TOUCHES {
                    // A duplicate begin, or a third finger this
                    // recognizer does not track. Ignored rather than
                    // evicting one that a gesture in flight depends on.
                    return false;
                }
                self.ids[self.len] = id;
                self.at[self.len] = Point { x, y };
                self.len += 1;
                true
            }
            TouchPhase::Move => match found {
                Some(k) => {
                    self.at[k] = Point { x, y };
                    true
                }
                None => false,
            },
            // End and Cancel both remove the finger. What differs is
            // what the *recognizer* concludes, not the bookkeeping.
            TouchPhase::End | TouchPhase::Cancel => match found {
                Some(k) => {
                    for j in k..self.len - 1 {
                        self.ids[j] = self.ids[j + 1];
                        self.at[j] = self.at[j + 1];
                    }
                    self.len -= 1;
                    true
                }
                None => false,
            },
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn at(&self, k: usize) -> Option<Point> {
        (k < self.len).then(|| self.at[k])
    }

    /// Distance between the first two touches, or `None` with fewer.
    fn spread(&self) -> Option<f32> {
        let (a, b) = (self.at(0)?, self.at(1)?);
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        Some(crate::num::sqrt(dx * dx + dy * dy))
    }

    /// Midpoint of the first two touches.
    pub fn centre(&self) -> Option<Point> {
        let (a, b) = (self.at(0)?, self.at(1)?);
        Some(Point { x: (a.x + b.x) / 2.0, y: (a.y + b.y) / 2.0 })
    }
}

/// Two fingers moving apart or together.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pinch {
    touches: Touches,
    /// The spread when the second finger landed, which is what a scale
    /// is relative to.
    start: Option<f32>,
}

impl Pinch {
    /// Feed one event; returns the scale when it changed.
    ///
    /// `1.0` is the spread the gesture started at, `2.0` twice as far
    /// apart. `None` means this event was not part of a pinch — which
    /// includes a *third* finger arriving, because a two-finger
    /// recognizer should stop rather than guess.
    pub fn feed(&mut self, ev: &Event) -> Option<f32> {
        if !self.touches.feed(ev) {
            return None;
        }
        match (self.touches.len(), self.start) {
            (2, None) => {
                // The gesture just became a pinch.
                self.start = self.touches.spread();
                Some(1.0)
            }
            (2, Some(s)) if s > 0.0 => self.touches.spread().map(|now| now / s),
            _ => {
                self.start = None;
                None
            }
        }
    }

    /// Where the pinch is centred, for zooming about a point rather
    /// than about the origin.
    pub fn centre(&self) -> Option<Point> {
        self.touches.centre()
    }
}

/// One finger held still for long enough.
///
/// Takes elapsed time rather than reading a clock, exactly as
/// [`crate::Animated`] does and for the same reason.
#[derive(Clone, Copy, Debug)]
pub struct LongPress {
    /// How long the finger must be down, in ms.
    pub hold: f32,
    /// How far it may drift and still count, in logical px.
    pub slop: f32,
    down: Option<(u32, Point)>,
    held: f32,
    fired: bool,
}

impl Default for LongPress {
    fn default() -> Self {
        // 500 ms and 10 px are the platform-conventional values, and
        // the same 500 ms `MULTI_CLICK_TIME` uses.
        LongPress { hold: 500.0, slop: 10.0, down: None, held: 0.0, fired: false }
    }
}

impl LongPress {
    /// Feed one event.
    pub fn feed(&mut self, ev: &Event) {
        let Event::Touch { id, x, y, phase } = *ev else { return };
        match phase {
            TouchPhase::Begin if self.down.is_none() => {
                self.down = Some((id, Point { x, y }));
                self.held = 0.0;
                self.fired = false;
            }
            TouchPhase::Move => {
                if let Some((d, from)) = self.down {
                    if d == id && ((x - from.x).abs() > self.slop || (y - from.y).abs() > self.slop)
                    {
                        // Drifted: it was a drag, not a hold.
                        self.down = None;
                    }
                }
            }
            TouchPhase::End | TouchPhase::Cancel => {
                if self.down.map(|(d, _)| d) == Some(id) {
                    self.down = None;
                }
            }
            TouchPhase::Begin => {}
        }
    }

    /// Advance by `dt` ms; returns the press's position **once**, on
    /// the tick it completes.
    ///
    /// Once, not every tick: a long press is an event, and a recognizer
    /// that kept reporting it would fire a context menu per frame for
    /// as long as the finger stayed down.
    ///
    /// A negative or NaN `dt` holds nothing (M47/T47.5: a NaN used to
    /// stall the press for ever); a huge one completes it.
    pub fn tick(&mut self, dt: f32) -> Option<Point> {
        let (_, at) = self.down?;
        if self.fired {
            return None;
        }
        // A NaN `dt` advances nothing (M47/T47.5, R7-F8): `clamp` returns
        // it unchanged, and a NaN `held` is never `>= hold`, so the press
        // never fired however long the finger stayed down.
        let dt = if dt.is_nan() { 0.0 } else { dt };
        self.held += clamp(dt, 0.0, self.hold);
        if self.held >= self.hold {
            self.fired = true;
            return Some(at);
        }
        None
    }
}
