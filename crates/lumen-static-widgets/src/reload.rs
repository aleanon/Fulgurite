//! `Reloadable` for the widgets this crate owns — M30, M49/T49.5.
//!
//! The trait is `lumen-static`'s and the types are this crate's, so the
//! impls live here. Core keeps the impls for what core still owns —
//! `Repeat`, `Vec<W>` and `Option<W>`.
//!
//! # A widget saves its user's half, and only that (M49/T49.5)
//!
//! `docs/plan-reload-hardening.md` §2.3. A hot swap exists to show an
//! edit to the **code** while keeping what the **user** did. So each
//! widget saves exactly what a user can change and nothing the code set:
//!
//! | widget | saves |
//! |---|---|
//! | `TextInput`, `TextArea`, `SearchField`, `RichTextEditor` | text and caret |
//! | `Slider`, `RangeSlider`, `NumberInput` | value(s) |
//! | `ScrollView`, `Repeat`, `DataGrid` | scroll |
//! | `Popup`, `Veil`, `Collapsible` | open |
//! | `Strip`, `Chooser`, `Pagination`, `Picker` | the selection |
//! | `Tree` | which rows are expanded |
//! | `SplitPane`, `PaneGrid` | the divider fractions |
//! | `Toggle` | on |
//! | `Button`, `Label`, `Fixed`, `Spacer` | nothing |
//!
//! A container saves its children too, under their own keys, so a kept
//! subtree keeps every user value in it. `Button`'s label and a static
//! `Label`'s text are **code**: before T49.5 they were saved, so an edit
//! to either never appeared after a swap (R5-F1). A text that changes at
//! run time belongs in `#[bind]`, whose source is state and is kept. The
//! consequence, stated in the book: an **unsplit** application whose
//! handler writes a label directly gets the build's text back after a
//! swap. `crates/lumen-static-shell/tests/reload_widget_state.rs` has a
//! row per widget here, and `reload_code_data.rs` the code half.
//!
//! Every restore that cannot use a saved value keeps the fresh one and
//! logs it (rule 1 of the plan: a swap keeps a value or says it did not).

#![cfg(feature = "reload")]

use lumen_static::reload::{log_fallback, Reloadable};
use lumen_static::text::{TextBuffer, TextStorage};
use lumen_static::widget::Widget;
use serde_json::{json, Value};

use crate::ops::{Fixed, Label, Spacer};

/// A number field of `saved`, or a logged fallback.
fn num(saved: &Value, ty: &str, key: &str) -> Option<f32> {
    match saved.get(key).and_then(Value::as_f64) {
        Some(v) if (v as f32).is_finite() => Some(v as f32),
        Some(_) => {
            log_fallback(ty, key, "not finite");
            None
        }
        None => {
            log_fallback(ty, key, "missing or not a number");
            None
        }
    }
}

fn flag(saved: &Value, ty: &str, key: &str) -> Option<bool> {
    let v = saved.get(key).and_then(Value::as_bool);
    if v.is_none() {
        log_fallback(ty, key, "missing or not a bool");
    }
    v
}

fn index(saved: &Value, ty: &str, key: &str) -> Option<usize> {
    let v = saved.get(key).and_then(Value::as_u64).map(|v| v as usize);
    if v.is_none() {
        log_fallback(ty, key, "missing or not an index");
    }
    v
}

/// Restore a child from `saved[key]`, or log that it is missing.
fn nest<W: Reloadable>(saved: &Value, ty: &str, key: &str, child: &mut W) {
    match saved.get(key) {
        Some(v) => child.restore(v),
        None => log_fallback(ty, key, "missing from saved state"),
    }
}

/// A moved value that did not take — out of range, say — is logged too.
fn took(ok: bool, ty: &str, key: &str) {
    if !ok {
        log_fallback(ty, key, "the saved value is out of this build's range");
    }
}

// ---- code, not state -----------------------------------------------------

/// A label's text is code (R5-F1): the build says what it reads, and a
/// text that changes at run time is `#[bind]`'s, whose source is kept.
impl<S: TextStorage> Reloadable for Label<S> {
    fn save(&self) -> Value {
        Value::Null
    }
    fn restore(&mut self, _saved: &Value) {}
}

/// Nothing: `label` is code the build sets, and `on_click` is code the
/// build's boot wires (`docs/events-decision.md` rule 3). It saved its
/// label until T49.5, so an edited label never appeared after a swap.
impl<S: 'static, T: TextStorage> Reloadable for crate::ops::Button<S, T> {
    fn save(&self) -> Value {
        Value::Null
    }
    fn restore(&mut self, _saved: &Value) {}
}

/// Nothing: a fixed box's size is the code's.
impl Reloadable for Fixed {
    fn save(&self) -> Value {
        Value::Null
    }
    fn restore(&mut self, _saved: &Value) {}
}

impl Reloadable for Spacer {
    fn save(&self) -> Value {
        Value::Null
    }
    fn restore(&mut self, _saved: &Value) {}
}

// ---- what a user changes -------------------------------------------------

/// `on` is kept — M25/T25.5. `on_toggle` is code, so the new build's boot
/// supplies its own; `mark` is not saved either, so an edited default
/// colour shows after a swap (`fixtures/hot-app` asserts both).
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

/// The caret, clamped onto the restored text's `char` boundaries: a
/// caret saved inside a character a new build would reject is moved to
/// the end rather than trusted.
fn caret_in(text: &str, c: usize) -> usize {
    if c <= text.len() && text.is_char_boundary(c) {
        c
    } else {
        text.len()
    }
}

impl<S: TextBuffer> Reloadable for crate::input::TextInput<S> {
    fn save(&self) -> Value {
        json!({ "text": self.as_str(), "caret": self.caret() })
    }
    fn restore(&mut self, saved: &Value) {
        match saved.get("text").and_then(Value::as_str) {
            Some(t) => {
                self.set_str(t);
                if let Some(c) = index(saved, "TextInput", "caret") {
                    self.caret = caret_in(self.as_str(), c);
                }
            }
            None => log_fallback("TextInput", "text", "missing or not a string"),
        }
    }
}

impl<S: TextBuffer> Reloadable for crate::textarea::TextArea<S> {
    fn save(&self) -> Value {
        json!({ "text": self.as_str(), "caret": self.caret() })
    }
    fn restore(&mut self, saved: &Value) {
        match saved.get("text").and_then(Value::as_str) {
            Some(t) => {
                self.set_str(t);
                if let Some(c) = index(saved, "TextArea", "caret") {
                    self.caret = caret_in(self.as_str(), c);
                }
            }
            None => log_fallback("TextArea", "text", "missing or not a string"),
        }
    }
}

/// The query; the icon is code.
impl Reloadable for crate::field::SearchField {
    fn save(&self) -> Value {
        json!({ "field": self.field.save() })
    }
    fn restore(&mut self, saved: &Value) {
        nest(saved, "SearchField", "field", &mut self.field);
    }
}

impl Reloadable for crate::slider::Slider {
    fn save(&self) -> Value {
        json!({ "value": self.value() })
    }
    fn restore(&mut self, saved: &Value) {
        if let Some(v) = num(saved, "Slider", "value") {
            self.set_value(v);
        }
    }
}

impl Reloadable for crate::field::RangeSlider {
    fn save(&self) -> Value {
        json!({ "low": self.low(), "high": self.high() })
    }
    fn restore(&mut self, saved: &Value) {
        if let (Some(lo), Some(hi)) = (num(saved, "RangeSlider", "low"), num(saved, "RangeSlider", "high")) {
            // High first when it grows, so the low bound never has to
            // pass it; low first when it shrinks.
            if hi >= self.high() {
                self.set_high(hi);
                self.set_low(lo);
            } else {
                self.set_low(lo);
                self.set_high(hi);
            }
        }
    }
}

/// The value; `min`, `max`, `step` and `decimals` are code.
impl Reloadable for crate::field::NumberInput {
    fn save(&self) -> Value {
        json!({ "value": self.value() })
    }
    fn restore(&mut self, saved: &Value) {
        if let Some(v) = num(saved, "NumberInput", "value") {
            self.set_value(v);
        }
    }
}

impl<W: Widget + Reloadable> Reloadable for crate::scroll::ScrollView<W> {
    fn save(&self) -> Value {
        json!({ "scroll": self.scroll, "content": self.content.save() })
    }
    fn restore(&mut self, saved: &Value) {
        if let Some(s) = num(saved, "ScrollView", "scroll") {
            self.scroll = s.max(0.0);
        }
        nest(saved, "ScrollView", "content", &mut self.content);
    }
}

/// The scroll, through `Repeat`'s own impl; the columns are code.
impl<R: Widget + Default + 'static> Reloadable for crate::data::DataGrid<R> {
    fn save(&self) -> Value {
        json!({ "rows": self.rows.save() })
    }
    fn restore(&mut self, saved: &Value) {
        nest(saved, "DataGrid", "rows", &mut self.rows);
    }
}

/// Open, and its content; the anchor is code.
impl<W: Widget + Reloadable> Reloadable for crate::popup::Popup<W> {
    fn save(&self) -> Value {
        json!({ "open": self.open, "content": self.content.save() })
    }
    fn restore(&mut self, saved: &Value) {
        if let Some(o) = flag(saved, "Popup", "open") {
            self.open = o;
        }
        nest(saved, "Popup", "content", &mut self.content);
    }
}

impl<W: Widget + Reloadable, const KIND: u8> Reloadable for crate::veil::Veil<W, KIND> {
    fn save(&self) -> Value {
        json!({ "open": self.open, "panel": self.panel.save() })
    }
    fn restore(&mut self, saved: &Value) {
        if let Some(o) = flag(saved, "Veil", "open") {
            if o {
                self.open();
            } else {
                self.close();
            }
        }
        nest(saved, "Veil", "panel", &mut self.panel);
    }
}

/// Open, and its body; the title is code.
impl<W: Widget + Reloadable, S: TextStorage> Reloadable for crate::split::Collapsible<W, S> {
    fn save(&self) -> Value {
        json!({ "open": self.is_open(), "body": self.body.save() })
    }
    fn restore(&mut self, saved: &Value) {
        if let Some(o) = flag(saved, "Collapsible", "open") {
            self.set_open(o);
        }
        nest(saved, "Collapsible", "body", &mut self.body);
    }
}

/// The selected item; the items are code.
impl<const KIND: u8> Reloadable for crate::nav::Strip<KIND> {
    fn save(&self) -> Value {
        json!({ "selected": self.selected() })
    }
    fn restore(&mut self, saved: &Value) {
        if let Some(i) = index(saved, "Strip", "selected") {
            let ok = i == self.selected() || self.select(i);
            took(ok, "Strip", "selected");
        }
    }
}

/// The selection, the trigger's text and whether the list is open; the
/// options are code.
impl<const KIND: u8> Reloadable for crate::choose::Chooser<KIND> {
    fn save(&self) -> Value {
        json!({ "selected": self.selected(), "trigger": self.trigger.save(), "open": self.is_open() })
    }
    fn restore(&mut self, saved: &Value) {
        match saved.get("selected") {
            Some(Value::Null) => nest(saved, "Chooser", "trigger", &mut self.trigger),
            Some(v) => match v.as_u64() {
                Some(i) => {
                    let i = i as usize;
                    let ok = self.selected() == Some(i) || self.select(i);
                    took(ok, "Chooser", "selected");
                }
                None => log_fallback("Chooser", "selected", "not an index"),
            },
            None => log_fallback("Chooser", "selected", "missing from saved state"),
        }
        if let Some(o) = flag(saved, "Chooser", "open") {
            if o {
                self.open();
            } else {
                self.close();
            }
        }
    }
}

/// Which rows are expanded, by position; the rows are code. A build with
/// fewer rows keeps what fits and logs the rest.
impl Reloadable for crate::data::Tree {
    fn save(&self) -> Value {
        Value::Array(self.open[..self.count].iter().map(|o| Value::Bool(*o)).collect())
    }
    fn restore(&mut self, saved: &Value) {
        let Some(arr) = saved.as_array() else {
            log_fallback("Tree", "open", "saved value is not an array");
            return;
        };
        for (i, v) in arr.iter().enumerate().take(self.count) {
            if let Some(o) = v.as_bool() {
                self.open[i] = o;
            }
        }
        if arr.len() != self.count {
            log_fallback(
                "Tree",
                "open",
                &format!("saved {} rows, this build has {}; the rest keep their fresh state", arr.len(), self.count),
            );
        }
    }
}

impl<A: Widget + Reloadable, B: Widget + Reloadable> Reloadable for crate::split::SplitPane<A, B> {
    fn save(&self) -> Value {
        json!({ "split": self.split(), "first": self.first.save(), "second": self.second.save() })
    }
    fn restore(&mut self, saved: &Value) {
        if let Some(f) = num(saved, "SplitPane", "split") {
            self.set_split(f);
        }
        nest(saved, "SplitPane", "first", &mut self.first);
        nest(saved, "SplitPane", "second", &mut self.second);
    }
}

impl<A, B, C, D> Reloadable for crate::split::PaneGrid<A, B, C, D>
where
    A: Widget + Reloadable,
    B: Widget + Reloadable,
    C: Widget + Reloadable,
    D: Widget + Reloadable,
{
    fn save(&self) -> Value {
        json!({
            "vertical": self.vertical(),
            "left": self.horizontal(0),
            "right": self.horizontal(1),
            "top_left": self.top_left.save(),
            "top_right": self.top_right.save(),
            "bottom_left": self.bottom_left.save(),
            "bottom_right": self.bottom_right.save(),
        })
    }
    fn restore(&mut self, saved: &Value) {
        for (which, key) in [(0u8, "vertical"), (1, "left"), (2, "right")] {
            if let Some(f) = num(saved, "PaneGrid", key) {
                self.set_divider(which, f);
            }
        }
        nest(saved, "PaneGrid", "top_left", &mut self.top_left);
        nest(saved, "PaneGrid", "top_right", &mut self.top_right);
        nest(saved, "PaneGrid", "bottom_left", &mut self.bottom_left);
        nest(saved, "PaneGrid", "bottom_right", &mut self.bottom_right);
    }
}

/// The page; the page count is code.
impl Reloadable for crate::data::Pagination {
    fn save(&self) -> Value {
        json!({ "page": self.page() })
    }
    fn restore(&mut self, saved: &Value) {
        if let Some(p) = index(saved, "Pagination", "page") {
            self.go(p);
            took(self.page() == p, "Pagination", "page");
        }
    }
}

/// What was picked — the swatch, the date, the time, the file — and
/// whether the pad is open. The file names are code.
impl<const KIND: u8> Reloadable for crate::pick::Picker<KIND> {
    fn save(&self) -> Value {
        let p = &self.pad.content;
        json!({
            "open": self.pad.open,
            "color": p.color, "year": p.year, "month": p.month, "day": p.day,
            "hour": p.hour, "minute": p.minute, "file": p.file,
        })
    }
    fn restore(&mut self, saved: &Value) {
        if let Some(o) = flag(saved, "Picker", "open") {
            self.pad.open = o;
        }
        let p = &mut self.pad.content;
        let int = |key: &str| saved.get(key).and_then(Value::as_i64);
        match (int("color"), int("year"), int("month"), int("day"), int("hour"), int("minute"), int("file")) {
            (Some(c), Some(y), Some(mo), Some(d), Some(h), Some(mi), Some(f)) => {
                p.color = c.max(0) as usize;
                p.year = y as i32;
                p.month = mo.clamp(1, 12) as u8;
                p.day = d.clamp(1, 31) as u8;
                p.hour = h.clamp(0, 23) as u8;
                p.minute = mi.clamp(0, 59) as u8;
                p.file = f.max(0) as usize;
            }
            _ => log_fallback("Picker", "value", "a field is missing or not an integer"),
        }
    }
}

/// The source text; the preview is derived from it.
impl Reloadable for crate::rich::RichTextEditor {
    fn save(&self) -> Value {
        json!({ "input": self.input.save() })
    }
    fn restore(&mut self, saved: &Value) {
        nest(saved, "RichTextEditor", "input", &mut self.input);
    }
}

// ---- widgets with nothing a user changes (M49/T49.8) ---------------------
//
// So a kept subtree can hold any widget of this crate. Each is code: what
// it shows is the build's, or a binding's whose source is kept state.
// `Select` is here because it is controlled (M33): the choice and whether
// it is open live in the application's state.

macro_rules! stateless {
    ($([$($g:tt)*] $ty:ty),* $(,)?) => {$(
        impl<$($g)*> Reloadable for $ty {
            fn save(&self) -> Value {
                Value::Null
            }
            fn restore(&mut self, _saved: &Value) {}
        }
    )*};
}

stateless! {
    [] crate::trim::Divider,
    [] crate::trim::Icon,
    [S: TextStorage] crate::trim::Badge<S>,
    [S: TextStorage] crate::trim::Chip<S>,
    [S: TextStorage] crate::trim::Avatar<S>,
    [] crate::indicator::ProgressBar,
    [] crate::indicator::Skeleton,
    [] crate::indicator::Spinner,
    [const KIND: u8] crate::chart::Chart<KIND>,
    [S: TextStorage] crate::nav::AppBar<S>,
    [S: TextStorage] crate::surface::Toast<S>,
    [] crate::rich::Markdown,
    [] crate::rich::RichText,
    [] crate::rich::PaintBox,
    [] crate::choose::OptionList,
    [] crate::image::Image,
    [S: 'static] crate::select::Select<S>,
}

/// A container with nothing of its own to keep: its content's half.
macro_rules! wraps {
    ($([$($g:tt)*] $ty:ty, $field:ident, $name:literal),* $(,)?) => {$(
        impl<$($g)*> Reloadable for $ty {
            fn save(&self) -> Value {
                json!({ stringify!($field): self.$field.save() })
            }
            fn restore(&mut self, saved: &Value) {
                nest(saved, $name, stringify!($field), &mut self.$field);
            }
        }
    )*};
}

wraps! {
    [W: Widget + Reloadable] crate::surface::Card<W>, content, "Card",
    [W: Widget + Reloadable] crate::glass::Glass<W>, content, "Glass",
    [W: Widget + Reloadable] crate::nav::PullToRefresh<W>, content, "PullToRefresh",
    [W: Widget + Reloadable, S: TextStorage] crate::surface::Tooltip<W, S>, content, "Tooltip",
}
