//! What a pump did, counted — M53/T53.0 (`docs/plan-frame-cost.md`).
//!
//! **Counts gate; clocks record** (that plan's rule 3). A clock moves
//! with machine load and a count does not, so the gates M53 builds —
//! a click that touches no arena must not re-run the arena cascade
//! (R6-F3), an idle pump must not place or paint (R6-F1) — assert on
//! these, and the timings are what decide whether a change ships.
//!
//! Instrumentation only: nothing here changes what a pump does.

/// How many times [`crate::Host::pump_with`] ran each pass since the
/// host was built. Monotonic; a test reads it before and after the
/// frames it cares about and subtracts.
///
/// `cascades` counts runs of `lumen_static::style::cascade_arenas_deep`
/// — the arena cascade a pump runs when an arena is owed one and a
/// sheet is set — not `Host::set_stylesheet`'s full cascade, which is
/// not part of a pump. `layouts`, `places` and `paints` count calls of
/// the pass, whether or not a bounded strategy then skipped every node
/// inside it: the question the idle gate asks is whether the pump
/// walked at all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PassCounts {
    pub pumps: u64,
    pub cascades: u64,
    pub layouts: u64,
    pub places: u64,
    pub paints: u64,
}

impl PassCounts {
    /// `self - earlier`, field by field: the passes run in between.
    pub fn since(self, earlier: PassCounts) -> PassCounts {
        PassCounts {
            pumps: self.pumps - earlier.pumps,
            cascades: self.cascades - earlier.cascades,
            layouts: self.layouts - earlier.layouts,
            places: self.places - earlier.places,
            paints: self.paints - earlier.paints,
        }
    }
}

/// What the last [`crate::Host::pump_with`] cost, stage by stage and
/// whole, in nanoseconds — M53/T53.1 (R6-F2, R6-F4).
///
/// **A published frame is the whole pump** (`docs/plan-frame-cost.md`
/// §2.1). Every probe used to add up `Host::timings3` — layout, place,
/// paint — and call it the frame, and each new step `pump_with` gained
/// (the arena cascade, the binding marks, the narrowing) had to be added
/// to every probe by hand, which is how the sum went stale twice. So the
/// host times itself: the stages below, and `pump` around all of them.
/// A stage nothing times shows up as [`StageTimes::gap`], which a probe
/// prints and bounds.
///
/// | field | what runs inside it |
/// |---|---|
/// | `input` | dispatch, the cancel, subscriptions, the outbox drain, the pointer-state marks, the deferred-result drain and the bindings |
/// | `cascade` | `cascade_arenas_deep`, when an arena is owed one and a sheet is set; 0 otherwise |
/// | `layout` | `layout` on the root |
/// | `place` | `place` on the root |
/// | `paint` | clearing the list and painting through `Scaled(Recording(list))` |
/// | `finish` | `Retention::finish` and spending the marks |
/// | `pump` | all of `pump_with`, measured around it |
///
/// Between `place` and `paint` the focus-ring fixup (and, with
/// `shadow-oracle`, the oracle's second layout) runs untimed: that is
/// what the gap holds by design.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StageTimes {
    pub input: u64,
    pub cascade: u64,
    pub layout: u64,
    pub place: u64,
    pub paint: u64,
    pub finish: u64,
    pub pump: u64,
}

impl StageTimes {
    /// The six stages added up.
    pub fn sum(&self) -> u64 {
        self.input + self.cascade + self.layout + self.place + self.paint + self.finish
    }

    /// `pump - sum`: what ran inside the pump and in no stage. Signed,
    /// because a printed negative gap is a finding about the timers.
    pub fn gap(&self) -> i64 {
        self.pump as i64 - self.sum() as i64
    }
}
