//! `Reloadable` for the widgets this crate owns — M30.
//!
//! The trait is `lumen-static`'s and the types are this crate's, so the
//! impls live here: they moved with `Label`, `Button`, `Toggle`,
//! `Fixed` and `Spacer` when M30 moved the widget vocabulary out of the
//! core. Core keeps the impls for what core still owns — `Repeat`,
//! `Vec<W>` and `Option<W>`.
//!
//! Nothing about the hot-reload contract changed. `fixtures/hot-app`
//! still asserts the same per-field fallbacks, and it is the gate that
//! says this move preserved them.

#![cfg(feature = "reload")]

use lumen_static::reload::{log_fallback, Reloadable};
use serde_json::{json, Value};

use crate::ops::{Fixed, Label, Spacer};

impl<S: lumen_static::text::TextRestore> Reloadable for Label<S> {
    fn save(&self) -> Value {
        Value::String(self.as_str().to_owned())
    }
    fn restore(&mut self, saved: &Value) {
        match saved.as_str() {
            // No leak for owning storage (`String`, `Cow`): M3 made this
            // the storage type's business. `&'static str` still leaks,
            // and says so in its `TextStorage` impl.
            Some(s) => self.set(<S as lumen_static::text::TextRestore>::from_string(s.to_owned())),
            None => log_fallback("Label", "text", "saved value is not a string"),
        }
    }
}

/// `on_click` is code: the new build's boot supplies its pointer and
/// `restore` never touches it — the whole point of
/// `docs/events-decision.md`. Written by hand because the derive's
/// untagged-field rule would demand `Reloadable` of an `fn` type.
impl<S: 'static, T: lumen_static::text::TextRestore> Reloadable for crate::ops::Button<S, T> {
    fn save(&self) -> Value {
        Value::String(self.label.as_ref().to_owned())
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
            Some(s) => self.label = <T as lumen_static::text::TextRestore>::from_string(s.to_owned()),
            None => log_fallback("Button", "label", "saved value is not a string"),
        }
    }
}

/// `on` is kept — M25/T25.5. `on_toggle` is code, so the new build's boot
/// supplies its own and `restore` never touches it; `mark` is not saved
/// either, so an edited default colour shows after a swap
/// (`fixtures/hot-app` asserts both). Written by hand for the reason
/// `Button`'s is: the derive's untagged-field rule would demand
/// `Reloadable` of an `fn`.
impl<S: 'static, const SHAPE: u8> Reloadable for crate::toggle::Toggle<S, SHAPE> {
    fn save(&self) -> Value {
        Value::Bool(self.on)
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
    fn restore(&mut self, _saved: &Value) {}
}