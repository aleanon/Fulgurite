//! What the previous frame was, so the next one can draw only the
//! difference — plan M8b.
//!
//! # Why the paint stream and not the node table
//!
//! The agent's journal answers "which *node* changed", and pays for it:
//! it re-paints every leaf into a scratch list to sign each node
//! separately. The renderer asks a different question — "which
//! *pixels*" — and the commands answer it directly. Comparing this
//! frame's commands against last frame's needs no per-node bookkeeping,
//! no second paint pass, and no change to `Node::paint`. The two
//! mechanisms coexist because they answer different questions, and the
//! cheaper one is the one on the release path.
//!
//! # Why a stream and not a stored list — M13/T13.8
//!
//! Retention used to keep the previous `DisplayList` and diff two lists.
//! Its recording method took a `&DisplayList`, which hard-wired the
//! vocabulary into the trait: even [`Rebuild`] was *typed* against a
//! list it does not want, and a strategy that computes damage without
//! storing commands could not express itself at all. That is what made
//! damage unavailable on a host with no list — the M13 device, where
//! damage is worth **more** than on a desktop, not less, because the
//! cost there is the SPI bus and a tenth of the screen is a tenth of
//! 31 ms (`docs/results-embedded.md`).
//!
//! So retention observes commands as they stream past
//! ([`Recording`], a [`Painter`] wrapper in the shape `Scaled` already
//! uses) and keeps `(hash, ink)` per command instead of the command.
//! No stored list, no text buffer, and it composes with the scaling
//! wrapper that is already there:
//!
//! ```text
//! paint → Recording(Scaled(sink))
//! ```
//!
//! Two consequences worth stating:
//!
//! - **Text compares better than it did.** `Cmd::Text` carries a
//!   *range* into a list's text buffer, so two frames whose strings
//!   differ but sit at the same offset and length — "12" becoming "13"
//!   — produced identical commands, and the old diff needed a special
//!   case to catch it. A stream hashes the bytes it is handed, so there
//!   is no special case to forget.
//! - **Equality becomes a 64-bit hash.** Two genuinely different
//!   commands that collide would leave stale pixels. At ~2 000 commands
//!   a frame that is a probability around 10⁻¹³ per frame, and
//!   `shell/tests/damage_oracle.rs` is the standing check that a
//!   partial redraw stays pixel-identical to a full one.
//!
//! # Why a buffer age, and not just this frame's damage
//!
//! The surface a shell presents into is multi-buffered: what
//! `softbuffer` hands back may be zeroed, or may hold the frame from
//! two presents ago. It says which through its buffer's `age`, and a
//! partial redraw that ignores that draws the difference against the
//! *wrong* base — the classic double-buffered smear. [`Retention`]
//! therefore keeps a short ring of recent damage and answers
//! [`Retention::take_damage_for_age`], which is what a shell must ask.
//!
//! # Why a type parameter and not a feature
//!
//! `docs/plan-architecture-v2.md` T8b.0. Briefly: [`Rebuild`] must stay
//! alive as the oracle [`Incremental`] is tested against — "same pixels
//! for the same input" is not a test a cargo feature lets you write,
//! because the two would never be in one binary. Memory is not the
//! argument; correctness surface is.

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::display::{fold, Color, Painter, Rect};

/// How many regions damage keeps apart before it starts merging them.
///
/// **Four, and the number is a cost ceiling rather than a correctness
/// one.** Overlapping regions are *correct* — each is cleared and
/// replayed independently, so a pixel in two of them is drawn twice and
/// lands on the same value — so merging can never make a frame wrong,
/// only slower. What it protects is the other direction: a frame with a
/// hundred separate changes should not become a hundred render calls.
///
/// A frame's damage is normally one region, or two once something reads
/// the backdrop beneath it (`docs/plan-glass.md` §5). Four leaves room
/// without making [`Damage`] large: it is stored per age in a ring of
/// `AGES`, so every rect here is paid five times over in the diffing
/// state.
pub const MAX_REGIONS: usize = 4;

/// A bounded set of damaged regions.
///
/// **The reason this is a set and not a bounding box** — M18. A box
/// union is not closed under anything useful: a scrollbar thumb moving
/// down the right edge, unioned with a toolbar across the top, is the
/// whole window. `docs/damage-amplification-decision.md` measured the
/// same failure from a different cause (a `Shrink` container's clip
/// change damaging a whole column, 1 562 px → 57 600 px, **37×**) and
/// recorded it as known and unfixed. This is the fix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Regions {
    rects: [Rect; MAX_REGIONS],
    len: u8,
}

impl Default for Regions {
    fn default() -> Self {
        Regions { rects: [[0.0; 4]; MAX_REGIONS], len: 0 }
    }
}

#[inline]
fn area_of(r: Rect) -> f32 {
    (r[2] - r[0]).max(0.0) * (r[3] - r[1]).max(0.0)
}

#[inline]
fn merge(a: Rect, b: Rect) -> Rect {
    [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]
}

#[inline]
fn overlaps(a: Rect, b: Rect) -> bool {
    a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
}

#[inline]
fn contains(outer: Rect, inner: Rect) -> bool {
    outer[0] <= inner[0] && outer[1] <= inner[1] && outer[2] >= inner[2] && outer[3] >= inner[3]
}

impl Regions {
    /// One region.
    pub fn one(r: Rect) -> Self {
        let mut out = Regions::default();
        out.push(r);
        out
    }

    pub fn as_slice(&self) -> &[Rect] {
        &self.rects[..self.len as usize]
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether any region overlaps `r`.
    ///
    /// The predicate a backdrop-reading command tests its source region
    /// against (`docs/plan-glass.md` §5). Strict on every edge: a region
    /// that merely *touches* `r` shares an edge and no pixels.
    pub fn intersects(&self, r: Rect) -> bool {
        self.as_slice().iter().any(|a| overlaps(*a, r))
    }

    /// The single box containing every region — what a caller that can
    /// only take one rect must use, and what M18 exists to stop being
    /// the only answer.
    pub fn bounds(&self) -> Option<Rect> {
        let mut it = self.as_slice().iter().copied();
        let first = it.next()?;
        Some(it.fold(first, merge))
    }

    /// Add a region, keeping the set within [`MAX_REGIONS`].
    ///
    /// Absorbed if an existing region already contains it; absorbing in
    /// turn if it contains one. Overlapping regions are merged, because
    /// keeping them apart would render their intersection twice for no
    /// benefit. When the set is full, `r` is first merged into the
    /// **last** slot, and then the two regions whose merge adds the least
    /// area are merged — so a full set loses precision twice per push
    /// (five disjoint squares end as three regions, not four), and never
    /// coverage: every rect ever pushed stays inside the union
    /// (`crates/lumen-static/tests/damage_regions.rs`,
    /// `every_pushed_rect_stays_covered`, M40/T40.0). Degrading this way
    /// is why a hundred scattered changes cost a few boxes instead of
    /// collapsing to the whole frame at the fifth; merging better would
    /// be a performance claim, and needs its own measurement.
    pub fn push(&mut self, r: Rect) {
        if area_of(r) <= 0.0 {
            return;
        }
        let mut r = r;
        // Absorb everything this rect already covers or touches, so the
        // set stays roughly disjoint without a separate pass.
        let mut k = 0;
        while k < self.len as usize {
            let e = self.rects[k];
            if contains(e, r) {
                return;
            }
            if contains(r, e) || overlaps(e, r) {
                r = merge(e, r);
                self.remove(k);
                continue;
            }
            k += 1;
        }
        if (self.len as usize) < MAX_REGIONS {
            self.rects[self.len as usize] = r;
            self.len += 1;
            return;
        }
        // Full: `r` plus the existing set is MAX_REGIONS + 1 boxes, and
        // one pair has to go. Pick the pair whose union wastes least.
        self.rects[MAX_REGIONS - 1] = merge(self.rects[MAX_REGIONS - 1], r);
        let mut best = (0usize, 1usize, f32::INFINITY);
        for i in 0..self.len as usize {
            for j in (i + 1)..self.len as usize {
                let cost = area_of(merge(self.rects[i], self.rects[j]))
                    - area_of(self.rects[i])
                    - area_of(self.rects[j]);
                if cost < best.2 {
                    best = (i, j, cost);
                }
            }
        }
        self.rects[best.0] = merge(self.rects[best.0], self.rects[best.1]);
        self.remove(best.1);
    }

    fn remove(&mut self, k: usize) {
        let last = self.len as usize - 1;
        self.rects[k] = self.rects[last];
        self.len -= 1;
    }
}

/// What differs between the frame just painted and the one before it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Damage {
    /// Nothing at all: the frame is identical. A shell may skip both
    /// the raster and the present.
    #[default]
    None,
    /// These regions differ, and nothing else does.
    Regions(Regions),
    /// Everything must be redrawn — the first frame, a resize, a
    /// structural change, or a host that does not retain.
    All,
}

impl Damage {
    /// Damage covering exactly one region.
    pub fn rect(r: Rect) -> Damage {
        if area_of(r) <= 0.0 {
            return Damage::None;
        }
        Damage::Regions(Regions::one(r))
    }

    /// Everything both cover. `All` absorbs everything; `None` is the
    /// identity.
    ///
    /// **Exact, not a bounding box** — that is the whole of M18. The
    /// previous version merged two rects into the box spanning them,
    /// which is why a change in one corner of a window and a change in
    /// the other repainted all of it.
    pub fn union(self, other: Damage) -> Damage {
        match (self, other) {
            (Damage::All, _) | (_, Damage::All) => Damage::All,
            (Damage::None, d) | (d, Damage::None) => d,
            (Damage::Regions(a), Damage::Regions(b)) => {
                let mut out = a;
                for r in b.as_slice() {
                    out.push(*r);
                }
                Damage::Regions(out)
            }
        }
    }

    /// Whether this damage touches `r`. `All` touches everything;
    /// `None` touches nothing.
    pub fn intersects(self, r: Rect) -> bool {
        match self {
            Damage::None => false,
            Damage::All => area_of(r) > 0.0,
            Damage::Regions(g) => g.intersects(r),
        }
    }

    /// Each region to redraw, clipped to the viewport, in no particular
    /// order.
    ///
    /// Regions may overlap. That is deliberate and it is safe: a caller
    /// clears and replays each one independently, so a pixel covered
    /// twice is written the same value twice. Merging exists to bound
    /// the *work*, never to make the output correct.
    pub fn regions(self, viewport: Rect) -> RegionIter {
        let g = match self {
            Damage::None => Regions::default(),
            Damage::All => Regions::one(viewport),
            Damage::Regions(g) => g,
        };
        RegionIter { g, viewport, at: 0 }
    }

    /// The single rect to redraw within, given the surface being drawn
    /// into — the **bounding box** of every region.
    ///
    /// `None` means "draw nothing"; `Some(r)` is always clamped to the
    /// viewport, so a caller cannot be handed a rect off the surface.
    ///
    /// Kept for callers that can only take one rect. A caller that can
    /// take several should use [`Damage::regions`] and do less work.
    pub fn clip_to(self, viewport: Rect) -> Option<Rect> {
        let r = match self {
            Damage::None => return None,
            Damage::All => viewport,
            Damage::Regions(g) => g.bounds()?,
        };
        clip(r, viewport)
    }
}

#[inline]
fn clip(r: Rect, viewport: Rect) -> Option<Rect> {
    let out = [
        r[0].max(viewport[0]),
        r[1].max(viewport[1]),
        r[2].min(viewport[2]),
        r[3].min(viewport[3]),
    ];
    (out[0] < out[2] && out[1] < out[3]).then_some(out)
}

/// Each damaged region, clipped to a viewport. See [`Damage::regions`].
#[derive(Clone, Copy, Debug)]
pub struct RegionIter {
    g: Regions,
    viewport: Rect,
    at: u8,
}

impl Iterator for RegionIter {
    type Item = Rect;

    fn next(&mut self) -> Option<Rect> {
        while (self.at as usize) < self.g.len() {
            let r = self.g.rects[self.at as usize];
            self.at += 1;
            if let Some(c) = clip(r, self.viewport) {
                return Some(c);
            }
        }
        None
    }
}

/// How much of the previous frame a host keeps.
///
/// Implemented three times: [`Rebuild`] keeps nothing and always
/// reports [`Damage::All`]; [`Incremental`] keeps one `(hash, ink)` per
/// command in a `Vec`; [`Streaming`] keeps the same in a fixed array,
/// for a host with no allocator.
///
/// Nothing here names a display list. That is the point — see the
/// module note.
pub trait Retention: Default {
    /// Observe one command as it is painted: its content hash, and the
    /// region it actually inks.
    ///
    /// Called by [`Recording`] in paint order. Implementations must
    /// tolerate any number of calls, including more than the previous
    /// frame had.
    fn observe(&mut self, hash: u64, ink: Rect);

    /// Observe that a command's pixels are a **function of** the region
    /// `src`, not merely of its own inputs — M19/T19.2.
    ///
    /// [`crate::display::Cmd::Glass`] is the only command that reads
    /// what is already in the frame. Everything else composes forward,
    /// which is why a partial redraw can replay a command list and get
    /// the same pixels; a backdrop filter is a function of the frame
    /// beneath it, so if `src` changed, this command's output changed
    /// even though its own hash did not.
    ///
    /// Provided as a no-op, so [`Rebuild`] — which redraws everything
    /// anyway — and any host strategy are unaffected until they opt in.
    fn observe_dep(&mut self, src: Rect) {
        let _ = src;
    }

    /// End the frame: return what changed against what was retained,
    /// and retain this frame instead.
    fn finish(&mut self) -> Damage;

    /// What a surface buffer `age` frames old needs redrawn, *and*
    /// mark the accumulated damage as presented.
    ///
    /// `age` counts presents, not pumps: a shell may pump several times
    /// per frame — the tally re-cascades and re-lays-out when the sign
    /// flips — and every one of those steps' damage belongs to the one
    /// frame that reaches the screen. So damage accumulates in `record`
    /// and is banked here.
    ///
    /// `age == 0` means the buffer's contents are unspecified, so the
    /// answer is [`Damage::All`]; `age == 1` is everything since the
    /// last present; `age == n` also unions the `n - 1` presents before
    /// that. An age past what is retained is [`Damage::All`] — not the
    /// last few frames' union, which would leave whatever changed
    /// before those on screen.
    fn take_damage_for_age(&mut self, age: u8) -> Damage;

    /// Forget everything: the next frame is a full redraw. A resize or
    /// a tree swap calls this.
    fn reset(&mut self);
}

/// Retain nothing; redraw everything, every frame.
///
/// The shipped behaviour before M8b, kept as the oracle every
/// incremental gate compares against — and the right choice for an app
/// small enough that "redraw it all" is cheaper than being clever, or
/// one that would rather not trust the clever path at all.
///
/// Zero-sized: a `Host<A, Rebuild>` carries no per-frame history.
#[derive(Default, Debug, Clone, Copy)]
pub struct Rebuild;

impl Retention for Rebuild {
    /// Keeps nothing, so there is nothing to observe. This is the body
    /// that makes `Rebuild` free: an empty call the optimizer removes,
    /// not a branch taken per command.
    fn observe(&mut self, _: u64, _: Rect) {}

    fn finish(&mut self) -> Damage {
        Damage::All
    }
    fn take_damage_for_age(&mut self, _: u8) -> Damage {
        Damage::All
    }
    fn reset(&mut self) {}
}

/// How many frames of damage are kept for [`Retention::take_damage_for_age`].
///
/// Triple buffering is the deepest thing in normal use, so an age above
/// 3 is either a compositor doing something unusual or a buffer that
/// has been out of rotation; both are answered honestly with
/// [`Damage::All`].
const AGES: usize = 4;

/// The comparison itself, shared by [`Incremental`] and [`Streaming`].
///
/// Split out because the two differ only in where the retained slots
/// live — a `Vec` or a fixed array. Writing the ring, the pending
/// union and the seeding rule twice is how they would drift.
#[derive(Debug, Default)]
struct Diffing {
    /// How many commands the previous frame had.
    len: usize,
    /// How many this frame has produced so far.
    n: usize,
    /// Damage accumulated within the frame being observed.
    frame: Damage,
    /// Whether a previous frame exists to compare against.
    seeded: bool,
    /// Set when a command arrived with no slot to compare it against —
    /// a frame longer than the retained capacity. Answered with
    /// [`Damage::All`], never by comparing against a slot that is not
    /// this command's.
    overflow: bool,
    /// Everything changed since the last present. A frame may take
    /// several pumps; they all land on one screen.
    pending: Damage,
    /// Damage of the last [`AGES`] *presented* frames, newest at `head`.
    ring: [Damage; AGES],
    head: usize,
    /// Source regions observed this frame by commands that read the
    /// frame beneath them, and how many. Bounded: a scene with more
    /// backdrop filters than this reports `Damage::All`, which is
    /// correct and merely slow, the same answer a slot overflow gets.
    deps: [Rect; MAX_DEPS],
    n_deps: usize,
    dep_overflow: bool,
}

/// How many backdrop-reading commands one frame may have before damage
/// degrades to [`Damage::All`].
///
/// `docs/plan-glass.md` §5.1: a glass panel serialises the rasteriser
/// and cannot be batched, so a frame with more than a handful is a
/// design problem rather than a case to optimise. Four matches
/// [`MAX_REGIONS`], which is not a coincidence — each panel contributes
/// at most one region.
pub const MAX_DEPS: usize = 4;

impl Diffing {
    /// Compare one command against its retained slot and overwrite it.
    ///
    /// `slot` is `None` when the frame has run past the retained
    /// capacity. In place: there is no "current frame" buffer, because
    /// a command is compared the moment it arrives and then replaces
    /// what it was compared against.
    fn step(&mut self, slot: Option<&mut (u64, Rect)>, hash: u64, ink: Rect) {
        if self.n == 0 {
            // First command of a frame: last frame's dependencies are
            // gone. Cleared here rather than in `finish`, because
            // `take_damage_for_age` runs *after* `finish` and needs
            // them still alive.
            self.n_deps = 0;
            self.dep_overflow = false;
        }
        self.n += 1;
        let Some(slot) = slot else {
            self.overflow = true;
            return;
        };
        if self.seeded && slot.0 != hash {
            self.frame = self.frame.union(Damage::rect(slot.1)).union(Damage::rect(ink));
        }
        *slot = (hash, ink);
    }

    /// Record a region a backdrop-reading command depends on.
    fn dep(&mut self, src: Rect) {
        if self.n_deps < MAX_DEPS {
            self.deps[self.n_deps] = src;
            self.n_deps += 1;
        } else {
            self.dep_overflow = true;
        }
    }

    /// Grow `d` until every dependent region that damage touches is
    /// itself fully damaged — M19/T19.2.
    ///
    /// **`D ∪= src`, not `D ∪= dst`.** The panel writes only its own
    /// rect but *reads* a wider one, and the ring between them is what a
    /// partial redraw would leave holding the previous frame's final
    /// pixels — including commands painted *after* the panel, and
    /// including the panel's own previous output, which re-blurs and
    /// smears a little more every frame. Damaging only where it draws
    /// fixes neither.
    ///
    /// **Terminates in at most `n_deps + 1` rounds, provably**, so there
    /// is no cap and no fallback: `d` only grows, the regions are fixed
    /// for the frame, and re-adding a contained region is a no-op — so
    /// every round that changes `d` consumes one of a finite set. A cap
    /// with a `Damage::All` escape would be a branch that cannot fire,
    /// and `AGENT.md` says such a branch cannot be shown to fail.
    fn dilate(&self, mut d: Damage) -> Damage {
        if self.dep_overflow && self.n_deps > 0 {
            return Damage::All;
        }
        if matches!(d, Damage::None | Damage::All) {
            return d;
        }
        for _ in 0..=self.n_deps {
            let mut grew = false;
            for k in 0..self.n_deps {
                let src = self.deps[k];
                if d.intersects(src) {
                    let next = d.union(Damage::rect(src));
                    if next != d {
                        d = next;
                        grew = true;
                    }
                }
            }
            if !grew {
                break;
            }
        }
        d
    }

    /// Add damage computed outside the per-command walk.
    ///
    /// [`Incremental`] compares two whole streams in `finish` rather
    /// than slot-by-slot as it goes, because aligning them needs both
    /// (M26/T26.11). This is how it hands the result back.
    ///
    /// `alloc`-gated with its only caller: `Streaming` keeps one buffer
    /// and has nothing to hand back, so on a no-alloc build this is
    /// dead code and `-D warnings` says so.
    #[cfg(feature = "alloc")]
    fn add(&mut self, d: Damage) {
        self.frame = self.frame.union(d);
    }

    /// Close the frame and return its damage.
    ///
    /// **A length change is `Damage::All` only where the strategy
    /// cannot align two streams.** [`Streaming`] keeps one fixed array
    /// and answers `All`; [`Incremental`] keeps the previous frame as
    /// well and aligns, so it passes `lengths_align = true` and this
    /// takes the damage it computed instead. Before M26/T26.11 nothing
    /// could align, so every popup open and close repainted the window
    /// on the strategy the desktop shell defaults to.
    fn finish_aligned(&mut self, lengths_align: bool) -> Damage {
        let d = if !self.seeded || self.overflow || (!lengths_align && self.n != self.len) {
            Damage::All
        } else {
            self.dilate(self.frame)
        };
        self.len = self.n;
        self.n = 0;
        self.frame = Damage::None;
        self.overflow = false;
        self.seeded = true;
        self.pending = self.pending.union(d);
        d
    }

    /// The un-aligned form: a length change is [`Damage::All`].
    fn finish(&mut self) -> Damage {
        self.finish_aligned(false)
    }

    fn take_damage_for_age(&mut self, age: u8) -> Damage {
        let banked = core::mem::replace(&mut self.pending, Damage::None);
        self.head = (self.head + 1) % AGES;
        self.ring[self.head] = banked;
        if age == 0 || age as usize > AGES || !self.seeded {
            return Damage::All;
        }
        // `banked` is what age 1 needs; each older age adds one more
        // presented frame, walking back from the entry before this one.
        let mut d = banked;
        for k in 1..age as usize {
            d = d.union(self.ring[(self.head + AGES - k) % AGES]);
        }
        // **Dilated again here, not only in `finish`.** Each banked
        // entry was closed under the dependency rule when it was
        // written, but their *union* need not be: merging two regions
        // that each missed a panel can produce one that clips it. The
        // panel would then be correct at buffer age 1 and stale at 3,
        // which is the worst shape a bug can have.
        self.dilate(d)
    }

    fn reset(&mut self) {
        self.seeded = false;
        self.n = 0;
        self.len = 0;
        self.frame = Damage::None;
        self.overflow = false;
        self.pending = Damage::All;
        self.ring = [Damage::All; AGES];
    }
}

/// Retain one `(hash, ink)` per command in a growable buffer, and
/// report what changed.
///
/// The desktop default. It allocates once, as the buffer grows to the
/// largest frame seen, and never again — `shell/tests/alloc_gate.rs`
/// asserts the steady state at zero.
///
/// **Requires the `alloc` feature** (default-on). [`Streaming`] is the
/// same strategy over a fixed array, for a host without one.
#[cfg(feature = "alloc")]
#[derive(Default, Debug)]
pub struct Incremental {
    /// The frame being observed.
    slots: Vec<(u64, Rect)>,
    /// The frame before it, kept whole (M26/T26.11).
    ///
    /// **Two buffers, because aligning needs both.** The comparison used
    /// to happen slot-by-slot *during* the walk, overwriting each slot
    /// as it went — which is why a length change could only answer
    /// `Damage::All`: by the time `finish` knew the lengths differed,
    /// the previous frame was gone. A popup opening therefore repainted
    /// the window, on the strategy the desktop shell defaults to.
    prev: Vec<(u64, Rect)>,
    d: Diffing,
}

#[cfg(feature = "alloc")]
impl Incremental {
    /// Damage between two frames whose lengths differ.
    ///
    /// **Longest common prefix and suffix by hash; damage the middle.**
    /// A popup opens by appending commands in the middle of a stream
    /// whose head and tail are untouched, and that is exactly what this
    /// finds. Nothing here guesses: a prefix matches only where the
    /// hashes are equal, so a stream that genuinely changed everywhere
    /// reports every rect in both frames — which covers the viewport,
    /// the same answer `Damage::All` gave, arrived at by measurement.
    fn align(prev: &[(u64, Rect)], cur: &[(u64, Rect)]) -> Damage {
        let mut head = 0usize;
        while head < prev.len() && head < cur.len() && prev[head].0 == cur[head].0 {
            head += 1;
        }
        let mut tail = 0usize;
        while tail < prev.len() - head
            && tail < cur.len() - head
            && prev[prev.len() - 1 - tail].0 == cur[cur.len() - 1 - tail].0
        {
            tail += 1;
        }
        let mut d = Damage::None;
        for slot in &prev[head..prev.len() - tail] {
            d = d.union(Damage::rect(slot.1));
        }
        for slot in &cur[head..cur.len() - tail] {
            d = d.union(Damage::rect(slot.1));
        }
        d
    }
}

#[cfg(feature = "alloc")]
impl Retention for Incremental {
    fn observe(&mut self, hash: u64, ink: Rect) {
        // **Recorded, not compared.** The comparison moved to `finish`
        // so that both whole streams are in hand when the lengths
        // differ; `Diffing::step` still runs, with no slot, for the
        // bookkeeping it owns (the per-frame dependency reset and the
        // command count).
        self.d.step(None, hash, ink);
        self.d.overflow = false;
        self.slots.push((hash, ink));
    }

    fn observe_dep(&mut self, src: Rect) {
        self.d.dep(src);
    }

    fn finish(&mut self) -> Damage {
        if self.d.seeded {
            let d = if self.prev.len() == self.slots.len() {
                // Equal lengths: slot for slot, which is the common case
                // and the cheap one.
                let mut d = Damage::None;
                for (a, b) in self.prev.iter().zip(self.slots.iter()) {
                    if a.0 != b.0 {
                        d = d.union(Damage::rect(a.1)).union(Damage::rect(b.1));
                    }
                }
                d
            } else {
                Self::align(&self.prev, &self.slots)
            };
            self.d.add(d);
        }
        let d = self.d.finish_aligned(true);
        core::mem::swap(&mut self.prev, &mut self.slots);
        self.slots.clear();
        d
    }

    fn take_damage_for_age(&mut self, age: u8) -> Damage {
        self.d.take_damage_for_age(age)
    }

    fn reset(&mut self) {
        self.slots.clear();
        self.prev.clear();
        self.d.reset();
    }
}

/// Retain `(hash, ink)` for up to `N` commands in a fixed array — the
/// allocator-free half of [`Incremental`] (M13/T13.8).
///
/// `N` commands cost `N * 24` bytes and no heap at all: `Streaming<256>`
/// is **6,680 B**, against the 128 KB the M13 device has
/// (`docs/results-embedded.md`). A frame longer than `N` reports
/// [`Damage::All`], which is correct but wasteful — size `N` above the
/// busiest frame, not at the typical one.
///
/// **Both numbers were wrong until M26/T26.11**: this said `N * 20` and
/// "5 KB", and a slot is 24 B — a `u64` and four `f32`s. Measured with
/// `size_of`, which is also how the 6,680 above is known rather than
/// derived.
///
/// **And this strategy keeps *one* buffer, deliberately.**
/// [`Incremental`] gained a second in T26.11 so it can align two
/// streams whose lengths differ, which is what stops a popup repainting
/// the window. Doing the same here would take `Streaming<256>` to
/// 13 KB — a tenth of the M13 device's RAM — for a strategy whose whole
/// purpose is a bounded, allocation-free footprint. A length change
/// still answers [`Damage::All`] here, and an embedded frame is both
/// simpler and rarer to restructure.
#[derive(Debug)]
pub struct Streaming<const N: usize> {
    slots: [(u64, Rect); N],
    d: Diffing,
}

impl<const N: usize> Default for Streaming<N> {
    fn default() -> Self {
        Streaming { slots: [(0, [0.0; 4]); N], d: Diffing::default() }
    }
}

impl<const N: usize> Retention for Streaming<N> {
    fn observe(&mut self, hash: u64, ink: Rect) {
        let i = self.d.n;
        let slot = self.slots.get_mut(i);
        self.d.step(slot, hash, ink);
    }

    fn observe_dep(&mut self, src: Rect) {
        self.d.dep(src);
    }

    fn finish(&mut self) -> Damage {
        self.d.finish()
    }

    fn take_damage_for_age(&mut self, age: u8) -> Damage {
        self.d.take_damage_for_age(age)
    }

    fn reset(&mut self) {
        self.d.reset();
    }
}

/// A [`Painter`] that hands every command to a [`Retention`] on its way
/// through — M13/T13.8.
///
/// The same shape as [`crate::Scaled`], and it composes with it:
///
/// ```text
/// paint → Recording(Scaled(sink))
/// ```
///
/// Retention sees content hashes and inked rects, never commands, which
/// is what lets a host compute damage without keeping a display list.
pub struct Recording<'a, R: Retention> {
    inner: &'a mut dyn Painter,
    ret: &'a mut R,
}

impl<'a, R: Retention> Recording<'a, R> {
    pub fn new(inner: &'a mut dyn Painter, ret: &'a mut R) -> Self {
        Recording { inner, ret }
    }
}

fn h_rect(h: u64, r: Rect) -> u64 {
    let mut h = h;
    for v in r {
        h = fold(h, v.to_bits() as u64);
    }
    h
}

fn h_color(h: u64, c: Color) -> u64 {
    fold(h, u32::from_be_bytes([c.0, c.1, c.2, c.3]) as u64)
}

impl<R: Retention> Painter for Recording<'_, R> {
    fn rect(&mut self, rect: Rect, color: Color) {
        self.ret.observe(h_color(h_rect(fold(0, 1), rect), color), rect);
        self.inner.rect(rect, color);
    }

    /// Hashes the **text bytes**, which is the improvement T13.8 got for
    /// free. Comparing stored `Cmd::Text` needed a special case, because
    /// a command holds a *range* into a text buffer: "12" becoming "13"
    /// is the same offset and the same length, so the two commands were
    /// equal and the label never repainted. A stream is handed the
    /// `&str` itself, so there is no case to forget.
    fn text_weighted(&mut self, rect: Rect, s: &str, size: f32, weight: f32, color: Color) {
        let mut h = h_rect(fold(0, 2), rect);
        h = fold(h, size.to_bits() as u64);
        h = fold(h, weight.to_bits() as u64);
        h = h_color(h, color);
        for b in s.as_bytes() {
            h = fold(h, *b as u64);
        }
        self.ret.observe(h, rect);
        self.inner.text_weighted(rect, s, size, weight, color);
    }

    fn rounded_rect(&mut self, rect: Rect, color: Color, radius: f32) {
        let mut h = h_rect(fold(0, 3), rect);
        h = h_color(h, color);
        h = fold(h, radius.to_bits() as u64);
        self.ret.observe(h, rect);
        self.inner.rounded_rect(rect, color, radius);
    }

    /// A border inks only its own rect: it is drawn *inside* it.
    fn stroked_rect(&mut self, rect: Rect, color: Color, radius: f32, width: f32) {
        let mut h = h_rect(fold(0, 11), rect);
        h = h_color(h, color);
        h = fold(h, radius.to_bits() as u64);
        h = fold(h, width.to_bits() as u64);
        self.ret.observe(h, rect);
        self.inner.stroked_rect(rect, color, radius, width);
    }

    /// A shadow inks past its own rect: the falloff reaches `spread`
    /// beyond it, and repainting only the rect would leave a ring of the
    /// old shadow behind.
    fn shadow(&mut self, rect: Rect, radius: f32, spread: f32) {
        let mut h = h_rect(fold(0, 4), rect);
        h = fold(h, radius.to_bits() as u64);
        h = fold(h, spread.to_bits() as u64);
        let ink = [rect[0] - spread, rect[1] - spread, rect[2] + spread, rect[3] + spread];
        self.ret.observe(h, ink);
        self.inner.shadow(rect, radius, spread);
    }

    fn line(
        &mut self,
        a: crate::desc::Point,
        b: crate::desc::Point,
        width: f32,
        color: Color,
    ) {
        let mut h = fold(0, 8);
        for v in [a.x, a.y, b.x, b.y, width] {
            h = fold(h, v.to_bits() as u64);
        }
        h = h_color(h, color);
        // A line's ink is its bounding box, widened by half the stroke
        // each way — a damage rect that used only the endpoints would
        // leave the thick part of a diagonal behind.
        let w = width / 2.0;
        let ink = [
            a.x.min(b.x) - w,
            a.y.min(b.y) - w,
            a.x.max(b.x) + w,
            a.y.max(b.y) + w,
        ];
        self.ret.observe(h, ink);
        self.inner.line(a, b, width, color);
    }

    fn gradient(&mut self, rect: Rect, stops: crate::display::Stops, angle: f32) {
        let mut h = h_rect(fold(0, 9), rect);
        h = fold(h, angle.to_bits() as u64);
        h = fold(h, stops.len as u64);
        for k in 0..(stops.len as usize).min(crate::display::MAX_STOPS) {
            h = fold(h, stops.at[k].to_bits() as u64);
            h = h_color(h, stops.color[k]);
        }
        self.ret.observe(h, rect);
        self.inner.gradient(rect, stops, angle);
    }

    fn image(&mut self, rect: Rect, id: crate::display::ImageId, tint: Color) {
        let mut h = h_rect(fold(0, 7), rect);
        h = fold(h, id.0 as u64);
        h = h_color(h, tint);
        self.ret.observe(h, rect);
        self.inner.image(rect, id, tint);
    }

    /// A backdrop filter inks exactly its own rect and **reads** a wider
    /// one, and both are reported: `observe` for the ink, `observe_dep`
    /// for the source.
    ///
    /// Both, not either. Skipping `observe` would desynchronise the
    /// command count against the retained slots and make `finish` answer
    /// `Damage::All` for the rest of the frame; skipping `observe_dep`
    /// would let the panel go stale whenever its backdrop changed
    /// without this command's own hash changing — which is every scroll.
    fn glass(&mut self, rect: Rect, radius: f32, material: crate::display::Material) {
        let mut h = h_rect(fold(0, 10), rect);
        h = fold(h, radius.to_bits() as u64);
        for v in [material.blur, material.rim, material.refraction, material.specular, material.light] {
            h = fold(h, v.to_bits() as u64);
        }
        h = h_color(h, material.tint);
        // The `u8` strengths. **This list is the one that decides
        // whether pixels are stale** -- `Cmd::hash_into`'s parallel list
        // only affects consumers of that function, while an omission
        // here means the frame reports no damage and the shell skips
        // the raster entirely.
        h = fold(
            h,
            ((material.shadow as u64) << 24)
                | ((material.saturation as u64) << 16)
                | ((material.dispersion as u64) << 8)
                | material.adapt as u64,
        );
        h = h_color(h, material.tint_over_light);
        h = fold(h, ((material.morph_x as u16 as u64) << 16) | material.morph_y as u16 as u64);
        self.ret.observe(h, rect);
        self.ret.observe_dep(crate::display::glass_source(rect, &material));
        self.inner.glass(rect, radius, material);
    }

    fn push_clip(&mut self, rect: Rect) {
        self.ret.observe(h_rect(fold(0, 5), rect), rect);
        self.inner.push_clip(rect);
    }

    /// A pop has no geometry of its own. It can only differ when a push
    /// did, and that push is what carries the region.
    fn pop_clip(&mut self) {
        self.ret.observe(fold(0, 6), [0.0; 4]);
        self.inner.pop_clip();
    }

    fn defer(&mut self, i: u32) -> bool {
        self.inner.defer(i)
    }
    /// Forwarded unchanged: recording observes a stream, it does not
    /// move anything (T16.12).
    fn clip_bounds(&self) -> Option<Rect> {
        self.inner.clip_bounds()
    }
}
