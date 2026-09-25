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
