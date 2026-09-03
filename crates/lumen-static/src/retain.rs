//! What the previous frame was, so the next one can draw only the
//! difference — plan M8b.
//!
//! # Why the display list and not the node table
//!
//! The agent's journal answers "which *node* changed", and pays for it:
//! it re-paints every leaf into a scratch list to sign each node
//! separately. The renderer asks a different question — "which
//! *pixels*" — and the display list answers it directly. Comparing this
//! frame's commands against last frame's needs no per-node bookkeeping,
//! no second paint pass, and no change to `Node::paint`. The two
//! mechanisms coexist because they answer different questions, and the
//! cheaper one is the one on the release path.
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

use crate::display::{Cmd, DisplayList, Rect};

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
/// Implemented twice: [`Rebuild`] keeps nothing and always reports
/// [`Damage::All`]; [`Incremental`] keeps the previous display list and
/// reports the region that differs.
pub trait Retention: Default {
    /// Diff the frame just painted against what is retained, remember
    /// it, and return what changed.
    fn record(&mut self, list: &DisplayList) -> Damage;

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
    fn record(&mut self, _: &DisplayList) -> Damage {
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

/// Retain the previous frame's display list and report what changed.
#[derive(Default, Debug)]
pub struct Incremental {
    prev: DisplayList,
    /// Everything that has changed since the last present. A frame may
    /// take several pumps; they all land on one screen.
    pending: Damage,
    /// Damage of the last [`AGES`] *presented* frames, newest at `head`.
    ring: [Damage; AGES],
    head: usize,
    seeded: bool,
}

impl Retention for Incremental {
    fn record(&mut self, list: &DisplayList) -> Damage {
        let d = if self.seeded { diff(&self.prev, list) } else { Damage::All };
        self.seeded = true;
        copy_list(&mut self.prev, list);
        self.pending = self.pending.union(d);
        d
    }

    fn take_damage_for_age(&mut self, age: u8) -> Damage {
        let banked = core::mem::replace(&mut self.pending, Damage::None);
        self.head = (self.head + 1) % AGES;
        let previous = core::mem::replace(&mut self.ring[self.head], banked);
        let _ = previous;
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
        self.pending = Damage::All;
        self.ring = [Damage::All; AGES];
    }
}

/// Overwrite `dst` with `src`, reusing `dst`'s allocations.
///
/// Field-by-field, spelled out rather than derived: the steady state
/// must not allocate, and cloning would build a new `Vec` and a new
/// `String` every frame (`docs/results-allocation.md`). Clearing and
/// extending reuses both once they have grown, which
/// `shell/tests/alloc_gate.rs` asserts.
fn copy_list(dst: &mut DisplayList, src: &DisplayList) {
    dst.cmds.clear();
    dst.cmds.extend_from_slice(&src.cmds);
    dst.text.clear();
    dst.text.push_str(&src.text);
}

/// The region in which two frames differ.
///
/// Commands are compared position by position: the tree is static and
/// paint is a deterministic pre-order walk, so the same node paints at
/// the same index frame after frame. A length change means the
/// structure changed (an arena grew, a branch switched) and is answered
/// with [`Damage::All`] rather than by aligning two lists — rare, and
/// the alignment would be guesswork.
fn diff(prev: &DisplayList, now: &DisplayList) -> Damage {
    if prev.cmds.len() != now.cmds.len() {
        return Damage::All;
    }
    let mut d = Damage::None;
    for (i, (a, b)) in prev.cmds.iter().zip(now.cmds.iter()).enumerate() {
        if a == b && !text_differs(prev, now, i, a) {
            continue;
        }
        d = d.union(Damage::Rect(ink(a))).union(Damage::Rect(ink(b)));
        if d == Damage::All {
            break;
        }
    }
    d
}

/// Whether two equal `Cmd::Text` at the same index actually say the
/// same thing.
///
/// `Cmd::Text` carries a *range* into the list's text buffer, not the
/// bytes. Two frames whose strings differ but sit at the same offset
/// and length — "12" becoming "13" — produce identical commands. Left
/// out, this is a label that never repaints.
fn text_differs(prev: &DisplayList, now: &DisplayList, i: usize, a: &Cmd) -> bool {
    let Cmd::Text { .. } = a else { return false };
    let (Cmd::Text { text: ra, .. }, Cmd::Text { text: rb, .. }) = (&prev.cmds[i], &now.cmds[i])
    else {
        return false;
    };
    prev.str_of(*ra) != now.str_of(*rb)
}

/// The region a command actually marks, which is not always its rect: a
/// shadow's falloff reaches `spread` past its own, and painting only
/// the rect would leave a ring of the old shadow behind.
fn ink(c: &Cmd) -> Rect {
    match c {
        Cmd::Shadow { rect, spread, .. } => {
            [rect[0] - spread, rect[1] - spread, rect[2] + spread, rect[3] + spread]
        }
        Cmd::Rect { rect, .. }
        | Cmd::RoundedRect { rect, .. }
        | Cmd::Text { rect, .. }
        | Cmd::PushClip { rect } => *rect,
        // A pop has no geometry of its own. It can only differ when a
        // push did, and that push is what carries the region.
        Cmd::PopClip => [0.0, 0.0, 0.0, 0.0],
    }
}
