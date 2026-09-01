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

use core::cell::Cell;

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
    cache: Cell<Option<(Constraints, Size)>>,
}

impl Measured {
    pub fn new() -> Self {
        Measured { cache: Cell::new(None) }
    }

    /// Return the cached size, or compute and store one.
    #[inline]
    pub fn get_or(&self, limits: Constraints, dirty: bool, f: impl FnOnce() -> Size) -> Size {
        if !dirty {
            if let Some((c, s)) = self.cache.get() {
                if c == limits {
                    return s;
                }
            }
        }
        let s = f();
        self.cache.set(Some((limits, s)));
        s
    }

    /// Drop the memo (a reload restores values without going through
    /// `Dirty`, so the cache must be invalidated explicitly).
    pub fn invalidate(&self) {
        self.cache.set(None);
    }

    /// Whether the last `get_or` was served from the memo — for tests.
    pub fn is_warm(&self, limits: Constraints) -> bool {
        matches!(self.cache.get(), Some((c, _)) if c == limits)
    }
}

/// What `Label<S>` accepts. Implemented for the storage types each
/// profile uses; the blanket `AsRef<str>` bound would conflict with the
/// `From<String>` reload path, so it is spelled out.
pub trait TextStorage: AsRef<str> + Default {
    /// Rebuild from an owned string — the hot-reload restore path.
    /// `&'static str` cannot, and says so by leaking (a dev-only path).
    fn from_string(s: String) -> Self;

    /// Replace the contents from a borrowed string, reusing the existing
    /// allocation where the storage can.
    ///
    /// This is the binding path's hot loop (`crate::bind`): a derived
    /// label is re-formatted on frames where its source changed, so the
    /// storage that desktop apps actually use must not allocate to
    /// accept a new value. The default cannot do better than a rebuild.
    fn set_from_str(&mut self, s: &str) {
        *self = Self::from_string(s.to_owned());
    }
}

impl TextStorage for String {
    fn from_string(s: String) -> Self {
        s
    }

    /// Keeps the capacity: steady-state binding updates allocate on no
    /// frame but the first for any given length.
    fn set_from_str(&mut self, s: &str) {
        self.clear();
        self.push_str(s);
    }
}

impl TextStorage for std::borrow::Cow<'static, str> {
    fn from_string(s: String) -> Self {
        std::borrow::Cow::Owned(s)
    }

    /// Reuses the buffer when this `Cow` already owns one; a `Borrowed`
    /// literal has no buffer to reuse and becomes `Owned`.
    fn set_from_str(&mut self, s: &str) {
        match self {
            std::borrow::Cow::Owned(buf) => {
                buf.clear();
                buf.push_str(s);
            }
            std::borrow::Cow::Borrowed(_) => *self = std::borrow::Cow::Owned(s.to_owned()),
        }
    }
}

impl TextStorage for &'static str {
    /// The documented dev-mode leak: this storage cannot own, and hot
    /// reload is a development path (§7's dlopen-without-dlclose spirit).
    fn from_string(s: String) -> Self {
        Box::leak(s.into_boxed_str())
    }
}
