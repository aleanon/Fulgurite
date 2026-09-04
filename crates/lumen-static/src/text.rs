//! Text storage — plan M3 (`docs/layout-and-storage-decisions.md` §1).
//!
//! Text lives in a *widget field*, not in a sibling with a getter,
//! because shaping is the expensive part of a real frame and only owned
//! storage can know when to invalidate: mutation goes through the
//! widget, so the widget can set a dirty bit. A getter cannot observe
//! that application state changed and would re-measure every frame.
//!
//! Storage is generic so each profile can choose: `&'static str` or an
//! inline buffer for `no_std`, `Cow<'static, str>` for desktop (literals
//! cost nothing, computed values allocate only when they must), `String`
//! for simplicity.

// `alloc`, not the std prelude: this crate is `no_std` when the
// `std` feature is off (M13/T13.1), and absent entirely without the
// `alloc` feature (T13.4).
#[cfg(feature = "alloc")]
use alloc::borrow::Cow;
#[cfg(feature = "alloc")]
use alloc::borrow::ToOwned;
#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::desc::{Constraints, Size};

/// A value plus a dirty bit. Reads are transparent; every mutable access
/// marks the node for re-measurement — §3's original `Dirty<String>`.
#[derive(Debug, Default, Clone)]
pub struct Dirty<T> {
    value: T,
    dirty: bool,
}

impl<T> Dirty<T> {
    pub const fn new(value: T) -> Self {
        Dirty { value, dirty: true }
    }

    #[inline]
    pub fn get(&self) -> &T {
        &self.value
    }

    /// Mutable access; marks dirty because the caller may change it.
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.dirty = true;
        &mut self.value
    }

    #[inline]
    pub fn set(&mut self, value: T) {
        self.value = value;
        self.dirty = true;
    }

    #[inline]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    #[inline]
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
}

impl<T> core::ops::Deref for Dirty<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}

/// A memoized measurement, valid for the limits it was taken under.
///
/// Keyed on the limits as well as the dirty bit, because the same text
/// measures differently in a narrower box once shaping wraps it (M5).
#[derive(Debug, Default, Clone)]
pub struct Measured {
    cache: Option<(Constraints, Size)>,
}

impl Measured {
    pub fn new() -> Self {
        Measured { cache: None }
    }

    /// Return the cached size, or compute and store one.
    #[inline]
    /// Takes `&mut self`.
    ///
    /// This was `Cell<Option<..>>` with a `&self` signature — interior
    /// mutability inherited from the flat-pass design, where ops were
    /// handed a shared base pointer. `layout` takes `&mut self`, and
    /// `get_or` has exactly one caller, inside it. The `Cell` bought
    /// nothing and cost `Sync`: every tree holding a `Label` was
    /// `!Sync`, which the repeater's own docs record removing for the
    /// same reason (`docs/plan-review-fixes.md` F6.3).
    pub fn get_or(&mut self, limits: Constraints, dirty: bool, f: impl FnOnce() -> Size) -> Size {
        if !dirty {
            if let Some((c, s)) = self.cache {
                if c == limits {
                    return s;
                }
            }
        }
        let s = f();
        self.cache = Some((limits, s));
        s
    }

    /// Drop the memo (a reload restores values without going through
    /// `Dirty`, so the cache must be invalidated explicitly).
    pub fn invalidate(&mut self) {
        self.cache = None;
    }

    /// Whether the last `get_or` was served from the memo — for tests.
    pub fn is_warm(&self, limits: Constraints) -> bool {
        matches!(self.cache, Some((c, _)) if c == limits)
    }
}

/// What `Label<S>` accepts.
///
/// Nothing but "can be read as a string, and has an empty value" (B11).
/// Rebuilding from an owned `String` used to be here, which quietly
/// required every storage to depend on `alloc`: a fixed-capacity inline
/// buffer — the shape a `no_std` profile needs — had to pull in a heap
/// allocator solely to satisfy a method only the hot-reload path calls.
/// That method is [`TextRestore`] now, gated behind `reload`, so the
/// no-allocation profile stays reachable
/// (`docs/text-and-scrolling-decisions.md` B11).
pub trait TextStorage: AsRef<str> + Default {}

impl<T: AsRef<str> + Default> TextStorage for T {}

/// Where a shaped run's cluster positions are kept (B9).
///
/// An associated type of the *storage*, not a fixed `Vec`, so a
/// fixed-capacity inline buffer can pair with a fixed-capacity cluster
/// array and the no-allocation profile stays reachable. `String` pairs
/// with `Vec<Cluster>`.
pub trait ClusterBuf: crate::cx::GlyphSink + Default {
    fn clear(&mut self);
    fn as_slice(&self) -> &[crate::cx::Cluster];
}

impl ClusterBuf for Vec<crate::cx::Cluster> {
    fn clear(&mut self) {
        Vec::clear(self);
    }
    fn as_slice(&self) -> &[crate::cx::Cluster] {
        self
    }
}

/// Storage that a hot reload can rebuild from saved bytes.
///
/// Behind `reload` because that is the only caller: restoring is a
/// dev-surface operation (§7), and requiring it of every storage is what
/// B11 objected to. `&'static str` satisfies it by leaking, which is
/// acceptable exactly once per reload and is why [`TextBuffer`] is a
/// separate trait for the repeated case.
#[cfg(feature = "reload")]
pub trait TextRestore: TextStorage {
    /// Rebuild from an owned string — the hot-reload restore path.
    fn from_string(s: String) -> Self;
}

/// Storage that can accept a new value *repeatedly* without leaking.
///
/// Split out of [`TextStorage`] deliberately. `&'static str` can satisfy
/// `from_string` only by leaking, which is acceptable exactly once per
/// hot reload (§7's dlopen-without-dlclose spirit) and catastrophic in a
/// binding, which runs every frame. Keeping the repeated-write method in
/// a separate trait turns that misuse into a **compile error** at the
/// binding site instead of an unbounded leak nobody sees:
///
/// ```compile_fail
/// # use lumen_static::ops::Label;
/// # use lumen_static::bind::format_into;
/// let mut l: Label = Label::new("start");      // default storage
/// format_into(&mut l, format_args!("{}", 1));  // no TextBuffer impl
/// ```
///
/// Measured before the split: 1000 bound updates to a default `Label`
/// leaked 9,906 bytes and never released them. `Label<String>` allocated
/// twice and then stayed flat.
pub trait TextBuffer: TextStorage {
    /// Where this storage's shaped cluster positions live (B9).
    type Clusters: ClusterBuf;

    /// Replace the contents from a borrowed string, reusing the existing
    /// allocation where the storage can.
    fn set_from_str(&mut self, s: &str);

    /// Insert `s` at byte offset `at` (B3: methods, never a raw
    /// `&mut String`).
    ///
    /// `at` must be a `char` boundary — every caller here derives it
    /// from a caret, and a caret only ever sits on one.
    fn insert_str(&mut self, at: usize, s: &str);

    /// Remove `range`, whose ends must be `char` boundaries.
    fn remove_range(&mut self, range: core::ops::Range<usize>);
}

#[cfg(feature = "reload")]
impl TextRestore for String {
    fn from_string(s: String) -> Self {
        s
    }
}

impl TextBuffer for String {
    type Clusters = Vec<crate::cx::Cluster>;

    /// Keeps the capacity: steady-state binding updates allocate on no
    /// frame but the first for any given length.
    fn set_from_str(&mut self, s: &str) {
        self.clear();
        self.push_str(s);
    }

    fn insert_str(&mut self, at: usize, s: &str) {
        String::insert_str(self, at, s);
    }

    fn remove_range(&mut self, range: core::ops::Range<usize>) {
        self.replace_range(range, "");
    }
}

#[cfg(feature = "reload")]
impl TextRestore for Cow<'static, str> {
    fn from_string(s: String) -> Self {
        Cow::Owned(s)
    }
}

impl TextBuffer for Cow<'static, str> {
    type Clusters = Vec<crate::cx::Cluster>;

    fn insert_str(&mut self, at: usize, s: &str) {
        self.to_mut().insert_str(at, s);
    }

    fn remove_range(&mut self, range: core::ops::Range<usize>) {
        self.to_mut().replace_range(range, "");
    }

    /// Reuses the buffer when this `Cow` already owns one; a `Borrowed`
    /// literal has no buffer to reuse and becomes `Owned`.
    fn set_from_str(&mut self, s: &str) {
        match self {
            Cow::Owned(buf) => {
                buf.clear();
                buf.push_str(s);
            }
            Cow::Borrowed(_) => *self = Cow::Owned(s.to_owned()),
        }
    }
}

#[cfg(feature = "reload")]
impl TextRestore for &'static str {
    /// The documented dev-mode leak: this storage cannot own, and hot
    /// reload is a development path (§7's dlopen-without-dlclose spirit).
    /// It happens once per swap, not once per frame — which is why there
    /// is deliberately no [`TextBuffer`] impl here.
    fn from_string(s: String) -> Self {
        Box::leak(s.into_boxed_str())
    }
}
