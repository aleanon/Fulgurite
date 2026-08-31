//! Frame-to-frame change journal — parity plan P2.
//!
//! Lumen answers "what changed" from a reactive dependency graph
//! (`ui.getDeps`, `ui.lastChange`, `ui.lastDamage`). This architecture has
//! no such graph and never will (§3: no rebuild, no signals), but it has
//! something the graph is a proxy for: two flat arrays that can simply be
//! compared. Per frame the journal records, for every node,
//!
//! - its geometry (`size`, `pos`) — "did it move or resize", and
//! - its paint checksum — "did its content change", which catches value
//!   edits that leave geometry identical (`"hello"` → `"world"`).
//!
//! Checksum inequality is a superset of geometry inequality (paint folds
//! geometry too), so the two flags classify a change as `geometry`,
//! `content`, or `both` — a distinction an agent can act on directly.
//!
//! **Cost, stated up front** (Lumen's audit review found its own lint
//! path re-shaping text and deep-cloning trees per call — the trap is
//! real): recording a frame is one extra paint-op dispatch per node plus
//! two small arrays. No AccessKit snapshot is built here; that happens
//! only when a query arrives. Dev-surface budget, per §9.
//!
//! Records live in a bounded ring, so an agent that polls slowly is told
//! `truncated: true` rather than handed a silent half-answer.

use std::collections::VecDeque;

use lumen_static::desc::{NodeDesc, NodeState, WidgetOps};
use lumen_static::{Point, Size};

use crate::TreeRef;

/// Frames of history kept. An agent polling once per second at 60fps
/// stays inside this; one polling slower gets `truncated: true`.
const DEFAULT_CAPACITY: usize = 64;

#[derive(Clone, Copy, PartialEq, Default)]
pub(crate) struct Geom {
    pub size: Size,
    pub pos: Point,
}

impl Geom {
    /// `[x0, y0, x1, y1]`, the node's absolute bounds.
    pub fn rect(&self) -> [f32; 4] {
        [self.pos.x, self.pos.y, self.pos.x + self.size.w, self.pos.y + self.size.h]
    }
}

/// One node's change within one frame.
pub(crate) struct Change {
    pub idx: u32,
    pub old: Geom,
    pub new: Geom,
    pub geometry: bool,
    pub content: bool,
}

/// A node's coalesced change across a queried interval: `old` is its
/// geometry at the start of the interval, `new` its geometry now, and the
/// flags are the union of everything observed in between. A node that
/// changed and reverted is still reported — it *did* change, and an agent
/// asking "what happened since frame 4" wants to know about the flicker.
pub(crate) struct Coalesced {
    pub idx: u32,
    pub old: Geom,
    pub new: Geom,
    pub geometry: bool,
    pub content: bool,
}

struct FrameRecord {
    frame: u64,
    /// The tree's structure changed under us (hot reload, a different
    /// tree): indices before this frame are not comparable to indices
    /// after it, and the baseline was re-seeded.
    reset: bool,
    changes: Vec<Change>,
}

/// What a `since` query found.
pub(crate) struct Report {
    pub changed: Vec<Coalesced>,
    /// History did not reach back to `since` — the answer is partial.
    pub truncated: bool,
    /// A baseline reset happened inside the interval.
    pub reset: bool,
    pub oldest_frame: u64,
}

#[derive(Default)]
pub(crate) struct Journal {
    /// Structural identity of the table the baseline was taken from.
    fingerprint: u64,
    geom: Vec<Geom>,
    sums: Vec<u64>,
    records: VecDeque<FrameRecord>,
    seeded: bool,
}

/// Cheap structural identity of a table: shape only, no state. Detects a
/// swapped or reloaded tree, whose node indices mean something different.
fn fingerprint(table: &[NodeDesc]) -> u64 {
    let mut h = table.len() as u64;
    for d in table {
        h = h.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(
            d.kind as u64 ^ ((d.offset as u64) << 16) ^ ((d.child_count as u64) << 48),
        );
    }
    h
}

fn measure(
    base: *const u8,
    table: &[NodeDesc],
    kinds: &[&'static WidgetOps],
    states: &[NodeState],
) -> (Vec<Geom>, Vec<u64>) {
    let mut geom = Vec::with_capacity(table.len());
    let mut sums = Vec::with_capacity(table.len());
    for (i, d) in table.iter().enumerate() {
        geom.push(Geom { size: states[i].size, pos: states[i].pos });
        let mut sum = 0u64;
        (kinds[d.kind as usize].paint)(base, d, &states[i], &mut sum);
        sums.push(sum);
    }
    (geom, sums)
}

impl Journal {
    /// Record one completed frame, diffing against the previous one.
    pub fn record(&mut self, frame: u64, tree: &TreeRef) {
        let (geom, sums) = measure(tree.base, tree.table, tree.kinds, tree.states);
        let fp = fingerprint(tree.table);

        // First frame, or the tree's structure changed: seed a baseline.
        // Nothing is reported as "changed" — there is no comparable prior
        // state — but a structural reset is flagged so the agent knows why.
        if !self.seeded || fp != self.fingerprint {
            let reset = self.seeded;
            self.push(FrameRecord { frame, reset, changes: Vec::new() });
            self.fingerprint = fp;
            self.geom = geom;
            self.sums = sums;
            self.seeded = true;
            return;
        }

        let mut changes = Vec::new();
        for i in 0..geom.len() {
            let moved = geom[i] != self.geom[i];
            let repainted = sums[i] != self.sums[i];
            if moved || repainted {
                changes.push(Change {
                    idx: i as u32,
                    old: self.geom[i],
                    new: geom[i],
                    geometry: moved,
                    // Paint folds geometry, so a pure move also flips the
                    // checksum; "content" means changed beyond geometry.
                    content: repainted && !moved,
                });
            }
        }
        self.push(FrameRecord { frame, reset: false, changes });
        self.geom = geom;
        self.sums = sums;
    }

    fn push(&mut self, rec: FrameRecord) {
        self.records.push_back(rec);
        while self.records.len() > DEFAULT_CAPACITY {
            self.records.pop_front();
        }
    }

    pub fn oldest_frame(&self) -> u64 {
        self.records.front().map_or(0, |r| r.frame)
    }

    /// Coalesce every change in `(since, now]`.
    pub fn since(&self, since: u64) -> Report {
        // Records for frames F_min..=F_max describe changes *arriving at*
        // those frames, so the interval is fully covered iff we still hold
        // the record for `since + 1`.
        let oldest = self.oldest_frame();
        let truncated = self.seeded && since + 1 < oldest;

        let mut order: Vec<u32> = Vec::new();
        let mut acc: std::collections::HashMap<u32, Coalesced> = std::collections::HashMap::new();
        let mut reset = false;

        for rec in self.records.iter().filter(|r| r.frame > since) {
            reset |= rec.reset;
            for c in &rec.changes {
                match acc.get_mut(&c.idx) {
                    Some(prev) => {
                        prev.new = c.new;
                        prev.geometry |= c.geometry;
                        prev.content |= c.content;
                    }
                    None => {
                        order.push(c.idx);
                        acc.insert(
                            c.idx,
                            Coalesced {
                                idx: c.idx,
                                old: c.old,
                                new: c.new,
                                geometry: c.geometry,
                                content: c.content,
                            },
                        );
                    }
                }
            }
        }

        let changed = order.into_iter().filter_map(|i| acc.remove(&i)).collect();
        Report { changed, truncated, reset, oldest_frame: oldest }
    }
}

/// Union of a coalesced report's old and new bounds — the damage rect.
pub(crate) fn damage(changed: &[Coalesced]) -> Option<[f32; 4]> {
    let mut acc: Option<[f32; 4]> = None;
    for c in changed {
        for r in [c.old.rect(), c.new.rect()] {
            // A zero-area node contributes nothing to damage, but a node
            // that *became* zero-area still damages where it used to be —
            // hence both rects, each filtered on its own.
            if r[2] <= r[0] || r[3] <= r[1] {
                continue;
            }
            acc = Some(match acc {
                None => r,
                Some(a) => [a[0].min(r[0]), a[1].min(r[1]), a[2].max(r[2]), a[3].max(r[3])],
            });
        }
    }
    acc
}
