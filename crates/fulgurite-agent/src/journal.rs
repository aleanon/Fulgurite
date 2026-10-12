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
//! **Cost, stated up front and measured** (Lumen's audit review found
//! its own lint path re-shaping text and deep-cloning trees per call —
//! the trap is real, and this file fell into a version of it):
//! recording a frame paints each *drawing* node once and folds each
//! container's signature from its children, over three buffers retained
//! across frames. **Zero allocations in steady state**; the numbers are
//! in `docs/results-agent-cost.md`. No AccessKit snapshot is built here;
//! that happens only when a query arrives. Dev-surface budget, per §9.
//!
//! This paragraph used to claim "one extra paint-op dispatch per node
//! plus two small arrays". It was three fresh collections per call, and
//! signing a node by calling `paint` on it meant a container repainted
//! its whole subtree — 2.1–2.8x N commands per frame, O(N.depth) work
//! (`docs/plan-review-fixes.md` F2.2).
//!
//! Records live in a bounded ring, so an agent that polls slowly is told
//! `truncated: true` rather than handed a silent half-answer.

use std::collections::VecDeque;

use fulgurite::desc::NodeDesc;
use fulgurite::{Point, Size};

use crate::TreeRef;

/// Frames of history kept: about four seconds at 60 fps. An agent
/// polling slower gets `truncated: true`.
///
/// **256 since M48/T48.4**, from 64. A focused field's caret, or any
/// animation, draws a frame every 16 ms whether or not anyone asks, so
/// one drive action of a dozen requests — focus, press, release, read
/// back — spanned 107 journal frames under `Boundaries`, and
/// `ui.lastDamage` answered from a truncated history with no region at
/// all. A frame that changed nothing costs a record with an empty,
/// unallocated `Vec`.
const DEFAULT_CAPACITY: usize = 256;

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
    /// The node drew something — in the frame before or in this one. A
    /// node that paints nothing (a spacer, a transparent container) can
    /// move without a pixel changing, so its move is a change but not
    /// damage (M56, `damage_regions`).
    pub paints: bool,
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
    /// See [`Change::paints`]; the union across the interval.
    pub paints: bool,
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
    /// Whether each node drew anything in the baseline frame.
    paints: Vec<bool>,
    records: VecDeque<FrameRecord>,
    seeded: bool,
    /// This frame's measurement, retained across frames. Three
    /// collections were allocated and dropped per call before
    /// 2026-09-02 — 172 KB per frame at 1k nodes, on the dev path
    /// (`docs/plan-review-fixes.md` F2.2).
    next_geom: Vec<Geom>,
    next_sums: Vec<u64>,
    next_paints: Vec<bool>,
    list: fulgurite::DisplayList,
}

/// Cheap structural identity of a table: shape only, no state. Detects a
/// swapped or reloaded tree, whose node indices mean something different.
fn fingerprint(table: &[NodeDesc]) -> u64 {
    let mut h = table.len() as u64;
    for d in table {
        h = h.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(
            d.first_child as u64 ^ ((d.parent as u64) << 16) ^ ((d.child_count as u64) << 48),
        );
    }
    h
}

/// Fill `geom` and `sums` for one frame, reusing all three buffers.
///
/// **Every node signs its own paint and nothing else — M27/T27.3.**
///
/// It used to fold: a container's signature was the mix of its
/// children's, so the table could be signed in one pass over the nodes
/// that actually draw instead of re-painting each subtree at every
/// depth. That solved the right problem and created two others, and both
/// were found by a drive script rather than by reading:
///
/// 1. **Damage amplified to the whole window.** A leaf's repaint flipped
///    every ancestor's folded signature, so every ancestor was reported
///    changed *with its own rect* — and the root's rect is the window.
///    A click on a 4 059 px² button reported 691 200 px² of damage,
///    **170x**, on a frame whose only real change was a 68x18 label.
/// 2. **A container's own paint was never signed at all.** The fold
///    contained its children and nothing of itself, so a container whose
///    background, shadow, radius — or, since M27/T27.1, whose border —
///    changed reported *nothing*. There was no test, because the fold
///    made every container look busy whenever anything under it moved.
///
/// So a container now signs exactly what the derive's `paint` draws
/// before it recurses: shadow, fill, radius, border. That is four state
/// reads, no painting, and no fold — cheaper than what it replaces, and
/// it makes both of the above impossible rather than smaller.
///
/// `child_count == 0` is the test for "paints arbitrary content", which
/// is deliberately true for the arena containers too: a `Repeat` has no
/// table children but does draw, and its window (and its clip) belong in
/// its own range.
fn measure_into(
    tree: &TreeRef,
    geom: &mut Vec<Geom>,
    sums: &mut Vec<u64>,
    paints: &mut Vec<bool>,
    list: &mut fulgurite::DisplayList,
    live: &[bool],
) {
    let n = tree.table.len();
    geom.clear();
    sums.clear();
    paints.clear();
    list.clear();
    // M82/T82.3: the frame's fill table, so a leaf's `ops::fill` paints
    // here what it paints on screen. `set_fills` reuses the buffer.
    list.set_fills(tree.fills);
    sums.resize(n, 0);
    paints.resize(n, false);

    for (i, (d, sum)) in tree.table.iter().zip(sums.iter_mut()).enumerate() {
        let st = tree.states[i];
        geom.push(Geom { size: st.size, pos: st.pos });
        // **Only nodes `node(i)` actually answers for.** For an index
        // inside a branch that is not laid out, it answers with the
        // enclosing container, and painting that container at `i` runs
        // off the end of `states` — see `crate::live_nodes`.
        if d.child_count == 0 && live[i] {
            let from = list.len();
            tree.root.node(i).paint(tree.states, i, list);
            *sum = list.hash_from(from);
            paints[i] = list.len() > from;
        }
    }

    // A container's *own* paint: what the derive draws before it
    // recurses. Signed from the resolved state rather than by painting,
    // because painting a container paints its whole subtree.
    for (i, d) in tree.table.iter().enumerate() {
        if d.child_count == 0 {
            continue;
        }
        // **A container that paints a part itself is signed by painting
        // it** — M48/T48.4. A `Picker`'s trigger is a table node its
        // `Picker` draws: `node(trigger)` answers with the picker, so the
        // trigger is a stand-in and is never signed, and the picker's
        // state-only signature has none of the trigger's text or swatch.
        // Choosing a colour changed the trigger and the journal reported
        // nothing (the drive's picker rows, once a repainting action with
        // no damage over its widget began to fail). A part is told from a
        // hidden branch by its area: a hidden subtree is zeroed by
        // `layout`, and a part is laid out to be seen. Costs one paint of
        // such a container's subtree per frame, only where one exists.
        //
        // It paints if that paint drew anything — the M56 `paints` rule
        // below, for a part. The M56 merge found the two rules crossing:
        // this branch `continue`d past the line that sets `paints`, so a
        // chosen date was a change with no damage anywhere.
        if live[i] && owns_a_visible_part(tree, live, i) {
            let from = list.len();
            tree.root.node(i).paint(tree.states, i, list);
            sums[i] = list.hash_from(from);
            paints[i] = list.len() > from;
            continue;
        }
        let st = &tree.states[i];
        let mut h = 0x9E37_79B9_7F4A_7C15u64;
        for v in [st.radius, st.shadow] {
            h = h.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(v.to_bits() as u64);
        }
        for c in [st.bg, st.border_color] {
            let bits = u32::from_be_bytes([c.0, c.1, c.2, c.3]) as u64;
            h = h.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(bits);
        }
        h = h.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(st.border_width as u64);
        // M82/T82.3: the fill, as its index *and* the resolved entry, so
        // a palette swap that recolours a role-named stop changes the
        // signature though the index did not move.
        let entry = (st.fill as usize).checked_sub(1).and_then(|k| tree.fills.get(k));
        h = h.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(st.fill as u64);
        if let Some(f) = entry {
            h = h.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(fill_hash(f));
        }
        sums[i] = h;
        // What the derive's `paint` draws before recursing, and whether
        // any of it is visible: a transparent container draws nothing.
        paints[i] = st.bg.3 > 0
            || st.shadow > 0.0
            || (st.border_width > 0 && st.border_color.3 > 0)
            || entry.is_some();
    }
}

/// A resolved fill entry's content, for a container's signature —
/// M82/T82.3: every stop's offset and colour and the shape's geometry.
fn fill_hash(f: &fulgurite::style::Fill) -> u64 {
    let mut h = 0xCBF2_9CE4_8422_2325u64;
    let mut mix = |v: u64| h = (h ^ v).wrapping_mul(0x0100_0000_01B3);
    mix(f.stops.len as u64);
    for k in 0..fulgurite::display::MAX_STOPS {
        let c = f.stops.color[k];
        mix(u64::from(f.stops.at[k]) << 32 | u32::from_be_bytes([c.0, c.1, c.2, c.3]) as u64);
    }
    mix(f.shape.kind() as u64);
    for g in f.shape.geom() {
        mix(g.to_bits() as u64);
    }
    h
}

/// Whether container `i` has a direct child it paints itself: a node
/// `node()` does not answer for (a stand-in, `live` false) that is laid
/// out with area — see the part-signing branch in `measure_into`.
fn owns_a_visible_part(tree: &TreeRef, live: &[bool], i: usize) -> bool {
    let d = &tree.table[i];
    let mut c = d.first_child as usize;
    for _ in 0..d.child_count {
        let s = &tree.states[c];
        if !live[c] && s.size.w > 0.0 && s.size.h > 0.0 {
            return true;
        }
        if tree.table[c].next_sibling == 0 {
            break;
        }
        c = tree.table[c].next_sibling as usize;
    }
    false
}

impl Journal {
    /// Record one completed frame, diffing against the previous one.
    /// Returns true when the tree's structure changed under us, so the
    /// audit can re-seed its finding set in step (indices either side of
    /// a reload address different nodes).
    /// `live` is `crate::live_nodes_with`'s answer for this frame, which
    /// `Agent` keeps and shares with the auditor (M48/T48.8).
    pub fn record(&mut self, frame: u64, tree: &TreeRef, live: &[bool]) -> bool {
        // Measured into the retained buffers, then swapped with the
        // baseline — so the two Vecs alternate roles instead of one pair
        // being allocated and the other dropped every frame.
        let (mut geom, mut sums, mut paints, mut list) = (
            core::mem::take(&mut self.next_geom),
            core::mem::take(&mut self.next_sums),
            core::mem::take(&mut self.next_paints),
            core::mem::take(&mut self.list),
        );
        measure_into(tree, &mut geom, &mut sums, &mut paints, &mut list, live);
        self.list = list;
        let fp = fingerprint(tree.table);

        // First frame, or the tree's structure changed: seed a baseline.
        // Nothing is reported as "changed" — there is no comparable prior
        // state — but a structural reset is flagged so the agent knows why.
        if !self.seeded || fp != self.fingerprint {
            let reset = self.seeded;
            self.push(FrameRecord { frame, reset, changes: Vec::new() });
            self.fingerprint = fp;
            core::mem::swap(&mut self.geom, &mut geom);
            core::mem::swap(&mut self.sums, &mut sums);
            core::mem::swap(&mut self.paints, &mut paints);
            self.next_geom = geom;
            self.next_sums = sums;
            self.next_paints = paints;
            self.seeded = true;
            return reset;
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
                    paints: paints[i] || self.paints[i],
                });
            }
        }
        self.push(FrameRecord { frame, reset: false, changes });
        core::mem::swap(&mut self.geom, &mut geom);
        core::mem::swap(&mut self.sums, &mut sums);
        core::mem::swap(&mut self.paints, &mut paints);
        self.next_geom = geom;
        self.next_sums = sums;
        self.next_paints = paints;
        false
    }

    fn push(&mut self, rec: FrameRecord) {
        // The ring's whole capacity at once, on the first record: grown
        // by doubling it reallocated at 128 and 256 frames, which
        // `tests/alloc_frame_done.rs` read as 0.04 per call (M48/T48.8).
        if self.records.capacity() == 0 {
            self.records.reserve_exact(DEFAULT_CAPACITY + 1);
        }
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
        let truncated = self.seeded && since.saturating_add(1) < oldest;

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
                        prev.paints |= c.paints;
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
                                paints: c.paints,
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
/// The damaged **region set**, not its bounding box — M27/T27.3.
///
/// [`damage`] below unions every changed rect into one box, and M18's own
/// doc says why that is the wrong answer: *"a box union is not closed
/// under anything useful: a scrollbar thumb moving down the right edge,
/// unioned with a toolbar across the top, is the whole window."* M18
/// replaced the union with a bounded set in the renderer and **this
/// surface was never updated**, so `ui.lastDamage` kept reporting the
/// pre-M18 quantity — and a drive script that bounded a widget's damage
/// by it read the whole window for a click on a button, because the
/// readout at the top and the control in the middle bound to the frame.
///
/// Capped at [`fulgurite::retain::MAX_REGIONS`], exactly as the
/// renderer caps it, so what an agent sees is what a frame would redraw
/// rather than a more optimistic parallel answer.
///
/// **And only nodes that draw** (M56). A spacer or a transparent
/// container that moved changed no pixel — the renderer's damage, taken
/// from commands, never saw it — but its box went into this set, and a
/// centring stage's four spacers and its row, all as wide as the stage,
/// merged into the stage's whole inner box: 7.47x an accordion whose own
/// damage was 1.00x (`tests/damage_paints.rs`). The move is still a
/// change, and `ui.lastChange` still reports it.
pub(crate) fn damage_regions(changed: &[Coalesced]) -> fulgurite::retain::Regions {
    let mut set = fulgurite::retain::Regions::default();
    for c in changed.iter().filter(|c| c.paints) {
        for r in [c.old.rect(), c.new.rect()] {
            if r[2] <= r[0] || r[3] <= r[1] {
                continue;
            }
            set.push(r);
        }
    }
    set
}

pub(crate) fn damage(changed: &[Coalesced]) -> Option<[f32; 4]> {
    let mut acc: Option<[f32; 4]> = None;
    for c in changed.iter().filter(|c| c.paints) {
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
