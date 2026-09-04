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

/// What differs between the frame just painted and the one before it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Damage {
    /// Nothing at all: the frame is identical. A shell may skip both
    /// the raster and the present.
    #[default]
    None,
    /// Only this region differs.
    Rect(Rect),
    /// Everything must be redrawn — the first frame, a resize, a
    /// structural change, or a host that does not retain.
    All,
}

impl Damage {
    /// The smallest damage covering both. `All` absorbs everything;
    /// `None` is the identity.
    pub fn union(self, other: Damage) -> Damage {
        match (self, other) {
            (Damage::All, _) | (_, Damage::All) => Damage::All,
            (Damage::None, d) | (d, Damage::None) => d,
            (Damage::Rect(a), Damage::Rect(b)) => Damage::Rect([
                a[0].min(b[0]),
                a[1].min(b[1]),
                a[2].max(b[2]),
                a[3].max(b[3]),
            ]),
        }
    }

    /// The rect to redraw within, given the surface being drawn into.
    /// `None` means "draw nothing"; `Some(r)` is always clamped to the
    /// viewport, so a caller cannot be handed a rect off the surface.
    pub fn clip_to(self, viewport: Rect) -> Option<Rect> {
        let r = match self {
            Damage::None => return None,
            Damage::All => viewport,
            Damage::Rect(r) => r,
        };
        let out = [
            r[0].max(viewport[0]),
            r[1].max(viewport[1]),
            r[2].min(viewport[2]),
            r[3].min(viewport[3]),
        ];
        (out[0] < out[2] && out[1] < out[3]).then_some(out)
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
}

impl Diffing {
    /// Compare one command against its retained slot and overwrite it.
    ///
    /// `slot` is `None` when the frame has run past the retained
    /// capacity. In place: there is no "current frame" buffer, because
    /// a command is compared the moment it arrives and then replaces
    /// what it was compared against.
    fn step(&mut self, slot: Option<&mut (u64, Rect)>, hash: u64, ink: Rect) {
        self.n += 1;
        let Some(slot) = slot else {
            self.overflow = true;
            return;
        };
        if self.seeded && slot.0 != hash {
            self.frame = self.frame.union(Damage::Rect(slot.1)).union(Damage::Rect(ink));
        }
        *slot = (hash, ink);
    }

    /// Close the frame and return its damage.
    ///
    /// A length change is [`Damage::All`] rather than an attempt to
    /// align two streams: it means the structure changed (an arena
    /// grew, a branch switched), it is rare, and the alignment would be
    /// guesswork.
    fn finish(&mut self) -> Damage {
        let d = if !self.seeded || self.overflow || self.n != self.len {
            Damage::All
        } else {
            self.frame
        };
        self.len = self.n;
        self.n = 0;
        self.frame = Damage::None;
        self.overflow = false;
        self.seeded = true;
        self.pending = self.pending.union(d);
        d
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
        d
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
    slots: Vec<(u64, Rect)>,
    d: Diffing,
}

#[cfg(feature = "alloc")]
impl Retention for Incremental {
    fn observe(&mut self, hash: u64, ink: Rect) {
        let i = self.d.n;
        if i < self.slots.len() {
            let slot = self.slots.get_mut(i);
            self.d.step(slot, hash, ink);
        } else {
            // The frame is longer than any before it. There is no slot
            // to compare against, but the length change `finish` sees
            // answers it — so this is deliberately *not* `overflow`,
            // which means "a fixed capacity that cannot grow".
            self.slots.push((hash, ink));
            self.d.n += 1;
        }
    }

    fn finish(&mut self) -> Damage {
        let d = self.d.finish();
        // Drop slots the frame no longer reaches, so a shrink does not
        // leave a stale tail for the next comparison to trust.
        self.slots.truncate(self.d.len);
        d
    }

    fn take_damage_for_age(&mut self, age: u8) -> Damage {
        self.d.take_damage_for_age(age)
    }

    fn reset(&mut self) {
        self.slots.clear();
        self.d.reset();
    }
}

/// Retain `(hash, ink)` for up to `N` commands in a fixed array — the
/// allocator-free half of [`Incremental`] (M13/T13.8).
///
/// `N` commands cost `N * 20` bytes and no heap at all: `Streaming<256>`
/// is 5 KB, against the 128 KB the M13 device has
/// (`docs/results-embedded.md`). A frame longer than `N` reports
/// [`Damage::All`], which is correct but wasteful — size `N` above the
/// busiest frame, not at the typical one.
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
}
