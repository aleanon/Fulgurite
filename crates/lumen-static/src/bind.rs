//! Derived presentation: tie a widget's displayed value to the state it
//! comes from, so the formatting step is *declared at the field* instead
//! of written out in an update method that can drift or be forgotten.
//!
//! The runtime here is deliberately small — two functions. The wiring is
//! `#[derive(App)]`, which turns `#[bind(...)]` attributes into the body
//! of [`App::update`](crate::event::App::update); the frame loop calls
//! that between input and layout. Nothing is registered, observed or
//! scheduled: a binding is generated straight-line code, which is why it
//! costs no runtime graph, survives a hot reload without migration (it
//! is code, re-supplied by the new dylib), and cannot capture anything —
//! the same properties that made handlers `fn` pointers rather than
//! closures (`docs/events-decision.md`).
//!
//! ## Change detection compares the *output*, not the source
//!
//! A binding must not re-measure text that did not change, since shaping
//! is the expensive part of a frame. The two obvious ways to know both
//! have holes:
//!
//! - a dirty bit on the source can be routed around — mutate the field
//!   directly and the screen lies, with nothing to detect it;
//! - a shadow copy of the source costs `size_of::<T>()` per binding and
//!   needs a field to live in, which a derive cannot add.
//!
//! So the comparison is against the formatted result. It is impossible
//! to defeat, because there is no setter to bypass and no bit to forget;
//! it stores nothing; and it keys on exactly the thing the dirty bit
//! protects, since equal text measures equally. The cost is running the
//! formatter every frame, which for the values that go in labels is
//! nanoseconds into a reused buffer.

// `alloc`, not the std prelude: this crate is `no_std` when the
// `std` feature is off (M13/T13.1), and absent entirely without the
// `alloc` feature (T13.4).
// The std scratch `thread_local!` is the only user.
#[cfg(feature = "std")]
use alloc::string::String;

use core::fmt::Write;
// The scratch `thread_local!` is the only user, and it is std-only.
#[cfg(feature = "std")]
use core::cell::RefCell;


/// Something a binding can write text into — M30.
///
/// `format_into` and `set_if_changed` took `&mut Label<S>` until the
/// widgets moved to `lumen-static-widgets`, at which point core could no
/// longer name the type its own `#[bind]` machinery writes to. Both
/// functions only ever called `set_str`, so the dependency was on one
/// method rather than on the widget, and this is that method.
///
/// **It returns whether the text changed**, which is the whole contract:
/// a binding runs every frame and must not mark a node for
/// re-measurement when nothing moved. An implementation that always
/// answered `true` would defeat `App::update`'s "a frame in which no
/// binding changed anything is still" — the property M16/T16.10 exists
/// for.
pub trait TextTarget {
    /// Store `text` if it differs from what is held, and say whether it
    /// did.
    fn set_str(&mut self, text: &str) -> bool;
}

#[cfg(feature = "std")]
std::thread_local! {
    /// One reusable formatting buffer per thread. It never escapes a
    /// call; it exists so that a binding running every frame allocates
    /// on no frame but the first.
    static SCRATCH: RefCell<String> = const { RefCell::new(String::new()) };
}

/// The `no_std` scratch buffer: `SCRATCH_CAP` bytes on the stack.
///
/// `thread_local!` is std-only, and the obvious `static` replacement is
/// not `Sync`, so *reusing* one buffer without `std` would need either
/// `critical-section` or `unsafe` — a dependency and an M0 violation
/// respectively. A stack buffer needs neither, and unlike the
/// `String::new()` this was between T13.1 and T13.4 it does not
/// allocate once per bound label per frame on the one target that can
/// least afford it.
///
/// The cost is a bound: a formatted label longer than `SCRATCH_CAP`
/// truncates at a `char` boundary, where `std` would grow. That is the
/// same rule `InlineText::set_from_str` follows for the same reason —
/// a binding is a presentation path, and a value too long for its field
/// should show its prefix.
///
/// Executed by `fixtures/embedded/tests/bind_nostd.rs` (M43/T43.8), which
/// `just thumb` runs with core built alone and without `std`: a 1,000-byte
/// value stores exactly this many bytes. Core's own tests build with `std`
/// and cannot reach this branch.
#[cfg(not(feature = "std"))]
const SCRATCH_CAP: usize = 256;

/// A target a value binding (`#[bind(target, source)]`) can write —
/// M33/T33.1.
///
/// **Compares before it writes and says whether it wrote**, exactly as
/// [`format_into`] does, so a value binding reports precisely and a
/// second update that changed nothing answers `false`. That is the
/// property `with =` gives up by returning nothing, and the reason
/// `docs/plan-damage-precision.md` §2.4 lists `with =` as unnarrowable.
///
/// Every `PartialEq + Clone` field is a target: a material's blur, a
/// popup's `open`. A widget whose value feeds a memo implements it for
/// itself so the memo sees the change.
///
/// ```
/// use lumen_static::bind::BindValue;
/// let mut blur = 4.0_f32;
/// assert!(blur.bind_value(&8.0), "changed");
/// assert!(!blur.bind_value(&8.0), "the same value writes nothing");
/// ```
pub trait BindValue<T: ?Sized> {
    fn bind_value(&mut self, value: &T) -> bool;
}

impl<T: PartialEq + Clone> BindValue<T> for T {
    #[inline]
    fn bind_value(&mut self, value: &T) -> bool {
        if *self == *value {
            return false;
        }
        self.clone_from(value);
        true
    }
}

/// Format `args` into `dst`, but only actually store the result — and
/// only mark the label for re-measurement — if it differs from the text
/// the label already holds. Returns whether the label changed.
///
/// This is what `#[bind(target, "…{}…")]` expands to.
///
/// The target is anything implementing [`TextTarget`] — in practice
/// `lumen_static_widgets::Label`, whose impl is the one this was written
/// against. Core cannot name it since M30 moved the widgets out, and the
/// dependency was only ever on `set_str`.
///
/// ```
/// use lumen_static::bind::{format_into, TextTarget};
///
/// // A minimal target, standing in for a `Label` — which lives in
/// // `lumen-static-widgets` and cannot be named from this crate's own
/// // doctest.
/// #[derive(Default)]
/// struct Readout(String);
/// impl TextTarget for Readout {
///     fn set_str(&mut self, text: &str) -> bool {
///         let changed = self.0 != text;
///         if changed { self.0.clear(); self.0.push_str(text); }
///         changed
///     }
/// }
///
/// let mut l = Readout("count: 1".into());
/// assert!(!format_into(&mut l, format_args!("count: {}", 1)), "same text, no write");
/// assert!(format_into(&mut l, format_args!("count: {}", 2)), "changed");
/// assert_eq!(l.0, "count: 2");
/// ```
pub fn format_into<D: TextTarget + ?Sized>(dst: &mut D, args: core::fmt::Arguments<'_>) -> bool {
    // Taken rather than borrowed: a `Display` impl that itself runs a
    // binding would double-borrow a `RefCell` and panic. Taking hands
    // the inner call an empty buffer instead, which is merely slower.
    #[cfg(feature = "std")]
    let mut buf: String = SCRATCH.with(|c| c.take());
    #[cfg(not(feature = "std"))]
    let mut buf: crate::inline::InlineText<SCRATCH_CAP> = Default::default();

    buf.clear();
    let _ = buf.write_fmt(args); // writing to a String cannot fail
    let changed = dst.set_str(buf.as_ref());

    #[cfg(feature = "std")]
    SCRATCH.with(|c| c.replace(buf));
    changed
}

/// Store `text` in `dst` only if it differs. The non-formatting form,
/// for a binding whose source is already a string.
pub fn set_if_changed<D: TextTarget + ?Sized>(dst: &mut D, text: &str) -> bool {
    // `Label::set_str` compares as of F4.6; this is kept as the named
    // spelling for a binding whose source is already a string.
    dst.set_str(text)
}

/// **Which table node a binding writes** — M29/T29.2
/// (`docs/plan-damage-precision.md` §2.3).
///
/// The deepest node whose memory contains `target`. A binding's target
/// is a field of the tree — a `Label`, or a field *inside* a widget, such
/// as a popup's `open` — so its address lies inside exactly one chain of
/// nested nodes, and the deepest of them is the widget that owns it.
///
/// **Why an address, not a const.** The plan's first route was a
/// per-field const offset composed along the binding's path; its second,
/// `wid_index!`. The first cannot cross a hand-written or generic
/// container (`Glass<W>` has no derived per-field items, and glass binds
/// through one), and the second needs an author id on every target. An
/// address crosses any container, and [`BindSite`] computes it once per
/// binding, so the per-frame cost after the first write is one atomic
/// load.
///
/// Indices an enum's inactive variant owns, and a `None`'s, resolve to
/// the container itself (`Node::node` returns `self`); they are skipped
/// by resolving to the same address, size and kind as their parent.
/// `None` when no table node contains `target` — an address in an arena
/// row, which the caller answers conservatively.
pub fn node_of<T: crate::widget::Widget>(tree: &T, target: *const u8) -> Option<usize> {
    let table = T::SUB_TABLE;
    let span = |i: usize| {
        let n = crate::widget::Node::node(tree, i);
        let p = n as *const dyn crate::widget::Node as *const u8;
        (p, core::mem::size_of_val(n), n.kind_name())
    };
    let t = target as usize;
    let mut best = None;
    for (i, d) in table.iter().enumerate() {
        let (p, size, kind) = span(i);
        if i > 0 && d.parent != crate::desc::NO_PARENT {
            let (pp, ps, pk) = span(d.parent as usize);
            if p == pp && size == ps && kind == pk {
                continue;
            }
        }
        let lo = p as usize;
        if t >= lo && t < lo + size {
            best = Some(i);
        }
    }
    best
}

/// One binding's node index, computed on first use — M29/T29.2.
///
/// The derive emits one per `#[bind]` of a non-generic application, as a
/// `static`, so [`node_of`]'s walk runs once for the life of the build
/// that owns it (a hot reload loads a new build with fresh statics, and
/// a new layout). A generic application gets none, and walks per write.
///
/// **All-zero when new, so it claims no flash** — M52/T52.3
/// (`docs/plan-embedded-footprint.md` §2.1, F-NEW-1). It stores the index
/// **plus two**: `0` is "not looked up yet" and `1` is "no table node
/// holds the target". Until M52 "unknown" was `usize::MAX`, a non-zero
/// initialiser, so each binding's static landed in `.data` — stored in
/// flash and copied to RAM before `main`. Now it is `.bss`: 4 B of zeroed
/// RAM per binding on a 32-bit target, and no image. That is still a
/// framework static, which the embedded profile otherwise has none of,
/// so `scripts/bss.allow` exempts it **by pattern**
/// (`*::update_marked::__ln_site`) with that argument, and
/// `scripts/check_bss.sh` counts the exempted caches and their bytes. It
/// is bounded by the source text: one per `#[bind]` line, never per
/// instance or per frame.
pub struct BindSite(core::sync::atomic::AtomicUsize);

impl BindSite {
    const UNKNOWN: usize = 0;
    const NOWHERE: usize = 1;
    /// What a stored index is offset by, so `0` and `1` stay free.
    const OFFSET: usize = 2;

    pub const fn new() -> BindSite {
        BindSite(core::sync::atomic::AtomicUsize::new(Self::UNKNOWN))
    }

    /// The cached index, or `find`'s answer, cached.
    pub fn get(&self, find: impl FnOnce() -> Option<usize>) -> Option<usize> {
        use core::sync::atomic::Ordering::Relaxed;
        match self.0.load(Relaxed) {
            Self::UNKNOWN => {
                let found = find();
                self.0.store(found.map_or(Self::NOWHERE, |i| i + Self::OFFSET), Relaxed);
                found
            }
            Self::NOWHERE => None,
            i => Some(i - Self::OFFSET),
        }
    }
}

impl Default for BindSite {
    fn default() -> Self {
        Self::new()
    }
}
