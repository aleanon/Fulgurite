//! Hot reload (handoff §7): throw the tree away, rebuild from new code,
//! graft old *values* back on.
//!
//! `save()` returns a JSON value holding only the keep-half of the state;
//! `fresh()` is the constructor from whichever dylib is currently loaded
//! (`Default::default()`); `restore()` grafts saved values onto a fresh
//! tree **per field, with fallback**: any incompatibility — missing field,
//! removed enum variant, changed type — keeps the fresh value for that
//! field alone and logs it (the §12.2 rule, generalized). A fn pointer or
//! vtable marked `#[reload(keep)]` fails to compile (`Serialize` is not
//! implemented), so the "Saved cannot contain code pointers" property is
//! structural, exactly as §7 demands.
//!
//! The reload *sequence* (save while old lib loaded → drop old tree →
//! load new lib, never dlclose → fresh → restore, bytes owned by the
//! host) lives in the host, not here — see `fixtures/hot-host`.

// `alloc`, not the std prelude: this crate is `no_std` when the
// `std` feature is off (M13/T13.1), and absent entirely without the
// `alloc` feature (T13.4).
#[cfg(feature = "alloc")]
use alloc::borrow::ToOwned;
#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use core::cell::RefCell;

pub use serde_json::{json, Map, Value};

use crate::ops::{Fixed, Label, Spacer};
use crate::repeat::Repeat;
use crate::widget::Widget;

pub fn to_value<T: serde::Serialize>(t: &T) -> Result<Value, serde_json::Error> {
    serde_json::to_value(t)
}

pub fn from_value<T: serde::de::DeserializeOwned>(v: Value) -> Result<T, serde_json::Error> {
    serde_json::from_value(v)
}

pub trait Reloadable {
    /// The keep-half of this value, as pointer-free data.
    fn save(&self) -> Value;
    /// A brand-new value built by the *currently loaded* code.
    fn fresh() -> Self
    where
        Self: Sized;
    /// Graft saved keep-values onto `self` (which came from [`fresh`](Reloadable::fresh)).
    /// Incompatible or missing fields keep their fresh value, logged.
    fn restore(&mut self, saved: &Value);
}

// ---- fallback log (silent fallback is indistinguishable from a bug) --------

std::thread_local! {
    static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Record (and print) a per-field fallback during restore.
pub fn log_fallback(ty: &str, field: &str, why: &str) {
    let line = format!("reload fallback: {ty}.{field}: {why} — keeping fresh value");
    eprintln!("{line}");
    LOG.with(|l| l.borrow_mut().push(line));
}

/// Drain the fallback log (host/tests inspect it after a reload).
pub fn take_log() -> Vec<String> {
    LOG.with(|l| core::mem::take(&mut *l.borrow_mut()))
}

// ---- framework impls --------------------------------------------------------

impl<S: crate::text::TextRestore> Reloadable for Label<S> {
    fn save(&self) -> Value {
        Value::String(self.as_str().to_owned())
    }
    fn fresh() -> Self {
        Self::default()
    }
    fn restore(&mut self, saved: &Value) {
        match saved.as_str() {
            // No leak for owning storage (`String`, `Cow`): M3 made this
            // the storage type's business. `&'static str` still leaks,
            // and says so in its `TextStorage` impl.
            Some(s) => self.set(<S as crate::text::TextRestore>::from_string(s.to_owned())),
            None => log_fallback("Label", "text", "saved value is not a string"),
        }
    }
}

/// `on_click` is code: `fresh()` supplies the new dylib's pointer and
/// `restore` never touches it — the whole point of
/// `docs/events-decision.md`. Written by hand because the derive's
/// untagged-field rule would demand `Reloadable` of an `fn` type.
impl<S: 'static, T: crate::text::TextRestore> Reloadable for crate::ops::Button<S, T> {
    fn save(&self) -> Value {
        Value::String(self.label.as_ref().to_owned())
    }
    fn fresh() -> Self {
        Self::default()
    }
    /// Through `T::from_string`, so `Button<S, String>` restores without
    /// leaking. The default storage is still `&'static str`, which can
    /// only accept a value by leaking — but that is now a *choice the
    /// type records*, documented at `TextStorage`, rather than the only
    /// path available. It used to `Box::leak` unconditionally, with no
    /// comment, while `Label`'s equivalent two impls above document
    /// theirs prominently (`docs/plan-review-fixes.md` F6.4).
    fn restore(&mut self, saved: &Value) {
        match saved.as_str() {
            Some(s) => self.label = <T as crate::text::TextRestore>::from_string(s.to_owned()),
            None => log_fallback("Button", "label", "saved value is not a string"),
        }
    }
}

/// `on` is kept; `look` and `on_toggle` are code, so `fresh()` supplies
/// the new build's and `restore` never touches them — M25/T25.5. The
/// resolved look is not saved either: `layout` recomputes it from the new
/// `look` on the first frame, which is what makes an edited look show
/// after a swap. Written by hand for the reason `Button`'s is: the
/// derive's untagged-field rule would demand `Reloadable` of an `fn`.
impl<S: 'static, const SHAPE: u8> Reloadable for crate::toggle::Toggle<S, SHAPE> {
    fn save(&self) -> Value {
        Value::Bool(self.on)
    }
    fn fresh() -> Self {
        Self::default()
    }
    fn restore(&mut self, saved: &Value) {
        match saved.as_bool() {
            Some(on) => self.on = on,
            None => log_fallback("Toggle", "on", "saved value is not a bool"),
        }
    }
}

impl Reloadable for Fixed {
    fn save(&self) -> Value {
        json!({ "w": self.w, "h": self.h })
    }
    fn fresh() -> Self {
        Self::default()
    }
    fn restore(&mut self, saved: &Value) {
        match (saved.get("w").and_then(Value::as_f64), saved.get("h").and_then(Value::as_f64)) {
            (Some(w), Some(h)) => {
                self.w = w as f32;
                self.h = h as f32;
            }
            _ => log_fallback("Fixed", "w/h", "saved value is not {w, h}"),
        }
    }
}

impl Reloadable for Spacer {
    fn save(&self) -> Value {
        Value::Null
    }
    fn fresh() -> Self {
        Spacer
    }
    fn restore(&mut self, _saved: &Value) {}
}

/// §12.4, decided for now: the materialized window is disposable — it
/// re-binds from data every frame, so only the *logical* state (count,
/// scroll position, extent) is worth keeping. `bind` is code and comes
/// from the fresh dylib untouched.
impl<T: Widget + Default + 'static> Reloadable for Repeat<T> {
    fn save(&self) -> Value {
        json!({ "count": self.count, "scroll": self.scroll, "item_extent": self.item_extent })
    }
    fn fresh() -> Self {
        Self::default()
    }
    fn restore(&mut self, saved: &Value) {
        match saved.get("count").and_then(Value::as_u64) {
            Some(c) => self.count = c as usize,
            None => log_fallback("Repeat", "count", "missing or not an integer"),
        }
        match saved.get("scroll").and_then(Value::as_f64) {
            Some(s) if (s as f32).is_finite() => self.scroll = (s as f32).max(0.0),
            Some(_) => log_fallback("Repeat", "scroll", "not finite"),
            None => log_fallback("Repeat", "scroll", "missing or not a number"),
        }
        // Saved state is the one input the core does not write itself, so
        // it is the one place a value has to be checked rather than
        // trusted. A subnormal extent overflowed `visible_range`'s cast;
        // a small one asked for hundreds of millions of rows from 40
        // bytes of JSON (`docs/plan-review-fixes.md` F1.4).
        match saved.get("item_extent").and_then(Value::as_f64) {
            Some(e) if (e as f32) >= Repeat::<T>::MIN_ITEM_EXTENT => self.item_extent = e as f32,
            Some(e) => log_fallback(
                "Repeat",
                "item_extent",
                &format!("{e} is below MIN_ITEM_EXTENT; keeping the fresh value"),
            ),
            None => log_fallback("Repeat", "item_extent", "missing or not a number"),
        }
    }
}

/// §7's container rule: saved data drives arity. `fresh()` gives an empty
/// list; restore rebuilds every element with fresh *code* and saved *data*.
impl<W: Reloadable> Reloadable for Vec<W> {
    fn save(&self) -> Value {
        Value::Array(self.iter().map(Reloadable::save).collect())
    }
    fn fresh() -> Self {
        Vec::new()
    }
    fn restore(&mut self, saved: &Value) {
        let Some(arr) = saved.as_array() else {
            log_fallback("Vec", "<self>", "saved value is not an array");
            return;
        };
        self.clear();
        self.extend(arr.iter().map(|v| {
            let mut w = W::fresh();
            w.restore(v);
            w
        }));
    }
}

impl<W: Reloadable> Reloadable for Option<W> {
    fn save(&self) -> Value {
        match self {
            Some(w) => w.save(),
            None => Value::Null,
        }
    }
    fn fresh() -> Self {
        None
    }
    fn restore(&mut self, saved: &Value) {
        if saved.is_null() {
            *self = None;
        } else {
            let mut w = W::fresh();
            w.restore(saved);
            *self = Some(w);
        }
    }
}

// ---------------------------------------------------------------------
// The hot-reload ABI, declared once
// ---------------------------------------------------------------------

/// The symbol names a reloadable app exports and a loader looks up.
///
/// Both sides referenced these as byte-string literals — twelve of them
/// in `fixtures/hot-app` and twelve more in `fixtures/hot-host`, with
/// nothing checking they agreed. A rename is a runtime `unwrap` on a
/// `None`, at load time, in a dylib swap. Naming them once makes the
/// handshake a compile-time fact on the loader side and a single edit on
/// the app side (`docs/plan-review-fixes.md` F6.7).
pub mod symbols {
    /// `fn() -> *mut c_void` — `App::fresh()` from this build's code.
    pub const NEW: &[u8] = b"hot_new";
    /// `fn(*mut c_void)` — drop the tree while this library is still
    /// loaded. Step 2 of §7's sequence.
    pub const DROP: &[u8] = b"hot_drop";
    /// `fn(*mut c_void, *mut u8, usize) -> usize` — serialize the keep
    /// half into a host-owned buffer. Two-call protocol: pass a null
    /// pointer and zero capacity to size, then allocate and call again.
    pub const SAVE: &[u8] = b"hot_save";
    /// `fn(*mut c_void, *const u8, usize) -> i32` — graft saved bytes
    /// onto this build's fresh tree. Returns 0 on success.
    pub const RESTORE: &[u8] = b"hot_restore";
    /// `fn(*mut u8, usize) -> usize` — drain the fallback log. Same
    /// two-call protocol as [`SAVE`], and it must not discard the log
    /// when the buffer is too small (F1.7).
    pub const TAKE_LOG: &[u8] = b"hot_take_log";
}

/// Serialize a saved value. Infallible in practice — a `Value` built by
/// `save()` always encodes — and returns an empty buffer rather than
/// panicking if that ever stops being true.
pub fn to_vec(v: &Value) -> Vec<u8> {
    serde_json::to_vec(v).unwrap_or_default()
}

/// Parse saved bytes, or `None` if they are not the shape `save()` emits.
pub fn from_slice(bytes: &[u8]) -> Option<Value> {
    serde_json::from_slice(bytes).ok()
}
