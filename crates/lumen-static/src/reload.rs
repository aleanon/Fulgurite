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

impl<S: crate::text::TextStorage> Reloadable for Label<S> {
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
            Some(s) => self.set(S::from_string(s.to_owned())),
            None => log_fallback("Label", "text", "saved value is not a string"),
        }
    }
}

/// `on_click` is code: `fresh()` supplies the new dylib's pointer and
/// `restore` never touches it — the whole point of
/// `docs/events-decision.md`. Written by hand because the derive's
/// untagged-field rule would demand `Reloadable` of an `fn` type.
impl<S: 'static> Reloadable for crate::ops::Button<S> {
    fn save(&self) -> Value {
        Value::String(self.label.to_owned())
    }
    fn fresh() -> Self {
        Self::default()
    }
    fn restore(&mut self, saved: &Value) {
        match saved.as_str() {
            Some(s) => self.label = Box::leak(s.to_owned().into_boxed_str()),
            None => log_fallback("Button", "label", "saved value is not a string"),
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
            Some(s) => self.scroll = s as f32,
            None => log_fallback("Repeat", "scroll", "missing or not a number"),
        }
        match saved.get("item_extent").and_then(Value::as_f64) {
            Some(e) => self.item_extent = e as f32,
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
