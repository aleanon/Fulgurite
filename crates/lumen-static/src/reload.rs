//! Hot reload (handoff §7): throw the tree away, rebuild from new code,
//! graft old *values* back on.
//!
//! `save()` returns a JSON value holding only the keep-half of the state;
//! `restore()` grafts saved values onto a value the **currently loaded**
//! code built — an application's through its program's boot, a row's
//! through `Default` — **per field, with fallback**: any incompatibility — missing field,
//! removed enum variant, changed type — keeps the fresh value for that
//! field alone and logs it (the §12.2 rule, generalized). A fn pointer or
//! vtable marked `#[reload(keep)]` fails to compile (`Serialize` is not
//! implemented), so the "Saved cannot contain code pointers" property is
//! structural, exactly as §7 demands.
//!
//! The reload *sequence* (save while the old lib is loaded → load the new
//! lib, never dlclose → boot the new build → restore → only then drop the
//! old tree; bytes owned by the host) lives in the host, not here — see
//! `lumen_static_reload::hot::Reloader` and `fixtures/hot-host`.

// `alloc`, not the std prelude: this crate is `no_std` when the
// `std` feature is off (M13/T13.1), and absent entirely without the
// `alloc` feature (T13.4).
#[cfg(feature = "alloc")]
#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use core::cell::RefCell;

pub use serde_json::{json, Map, Value};

use crate::repeat::Repeat;
use crate::widget::Widget;

pub fn to_value<T: serde::Serialize>(t: &T) -> Result<Value, serde_json::Error> {
    serde_json::to_value(t)
}

pub fn from_value<T: serde::de::DeserializeOwned>(v: Value) -> Result<T, serde_json::Error> {
    serde_json::from_value(v)
}

/// A value that survives a hot swap: what it keeps, and how that is
/// grafted onto a new build's value.
///
/// **There is no constructor here** (M49/T49.4). It had one, `fresh()`,
/// which the derive routed to `Application::build` for an application —
/// a second construction path beside the program's boot, which is what a
/// window swap and `Host::reload` actually run, and which nothing
/// checked against it (R5-F10, `docs/plan-reload-hardening.md` §2.2). An
/// application is built by its program's boot, always (`Host::reload`
/// in the shell re-runs it); a container that
/// needs a fresh row (`Vec`, `Option`, `Repeat`) takes `Default`, which
/// is the only thing a row's `fresh()` ever was.
///
pub trait Reloadable {
    /// The keep-half of this value, as pointer-free data.
    fn save(&self) -> Value;
    /// Graft saved keep-values onto `self`, which the currently loaded
    /// code built. Incompatible or missing fields keep their fresh value,
    /// logged.
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

// ---- the derive's keep arm ------------------------------------------------

/// Save one `#[reload(keep)]` field into `out` — what the derive's `save`
/// calls, per field.
///
/// **A value that does not serialize is left out, and logged** (M49/T49.6,
/// R5-F3). It used to be saved as `null`, and `null` deserializes into an
/// `Option` as `None` — so a kept `Option<HashMap<(u8, u8), _>>` (serde_json
/// refuses non-string keys) came back `None` over the fresh value, with
/// no line: the save's log is the *old* library's, and the swap drains it
/// before the restore. Left out, the restore logs "missing from saved
/// state" in the new library, where the window's bar reads it.
pub fn save_keep<T: serde::Serialize>(out: &mut Map<String, Value>, value: &T, ty: &str, field: &str) {
    match to_value(value) {
        Ok(v) => {
            out.insert(String::from(field), v);
        }
        Err(e) => log_fallback(ty, field, &format!("save failed, not saved: {e}")),
    }
}

/// Restore one `#[reload(keep)]` field — what the derive's `restore`
/// calls, per field. Keeps the fresh value, and logs, when the saved
/// state has no such key, when it cannot be read as this build's type,
/// and (M49/T49.6) when it is `null` where the fresh value is not: serde
/// writes `NaN` as `null`, so a kept `Some(f32::NAN)` would otherwise come
/// back `None` over a fresh `Some(x)`, silently. That covers NaN without
/// a per-type walk; the cost is that a user's deliberate `None` over a
/// build's `Some` default is kept as the fresh value too, and says so.
pub fn restore_keep<T: serde::Serialize + serde::de::DeserializeOwned>(
    slot: &mut T,
    saved: Option<&Value>,
    ty: &str,
    field: &str,
) {
    let Some(v) = saved else {
        log_fallback(ty, field, "missing from saved state");
        return;
    };
    if v.is_null() && to_value(&*slot).is_ok_and(|f| !f.is_null()) {
        log_fallback(ty, field, "serialized as null (a NaN, or a value that did not save)");
        return;
    }
    match from_value(v.clone()) {
        Ok(x) => *slot = x,
        Err(e) => log_fallback(ty, field, &format!("incompatible saved value: {e}")),
    }
}

// ---- framework impls --------------------------------------------------------


/// **The scroll, and only the scroll** — M49/T49.5
/// (`docs/plan-reload-hardening.md` §2.3).
///
/// §12.4 decided the materialized window is disposable: it re-binds from
/// data every frame. It also kept `count` and `item_extent`, and both
/// are the **code's**: the build says how tall a row is, and the next
/// update's binding sets the count from the application's data. Kept,
/// an edited row height never appeared after a swap (R5-F1). `bind` is
/// code and comes from the new build untouched.
impl<T: Widget + Default + 'static> Reloadable for Repeat<T> {
    fn save(&self) -> Value {
        json!({ "scroll": self.scroll })
    }
    fn restore(&mut self, saved: &Value) {
        // Saved state is the one input the core does not write itself, so
        // it is the one place a value has to be checked rather than
        // trusted (`docs/plan-review-fixes.md` F1.4).
        match saved.get("scroll").and_then(Value::as_f64) {
            Some(s) if (s as f32).is_finite() => self.scroll = (s as f32).max(0.0),
            Some(_) => log_fallback("Repeat", "scroll", "not finite"),
            None => log_fallback("Repeat", "scroll", "missing or not a number"),
        }
    }
}

/// §7's container rule: saved data drives arity. Restore rebuilds every
/// element with fresh *code* (`Default`, from the build loaded now) and
/// saved *data*.
impl<W: Reloadable + Default> Reloadable for Vec<W> {
    fn save(&self) -> Value {
        Value::Array(self.iter().map(Reloadable::save).collect())
    }
    fn restore(&mut self, saved: &Value) {
        let Some(arr) = saved.as_array() else {
            log_fallback("Vec", "<self>", "saved value is not an array");
            return;
        };
        self.clear();
        self.extend(arr.iter().map(|v| {
            let mut w = W::default();
            w.restore(v);
            w
        }));
    }
}

impl<W: Reloadable + Default> Reloadable for Option<W> {
    fn save(&self) -> Value {
        match self {
            Some(w) => w.save(),
            None => Value::Null,
        }
    }
    fn restore(&mut self, saved: &Value) {
        if saved.is_null() {
            *self = None;
        } else {
            let mut w = W::default();
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
/// **This build of the core, as `name=value` fields** — the core's part
/// of a hot library's ABI stamp (M37/T37.0, `docs/plan-window-reload.md`
/// §2.3).
///
/// A loader borrows a library's values in place, so the two must lay out
/// every type that crosses identically. The core's features change
/// layouts (`observe` adds methods to every widget's vtable), and cargo
/// unifies features per invocation, so a loader and a library built by
/// two invocations can disagree without either noticing. The sizes are
/// here because a field added to a crossing type changes them; a
/// reordering changes none, and is caught by the source hash
/// `lumen_static_reload::abi::stamp` carries beside these (M49/T49.1).
pub fn build_fields() -> alloc::vec::Vec<(&'static str, alloc::string::String)> {
    use alloc::format;
    let mut features = alloc::vec::Vec::new();
    for (on, name) in [
        (cfg!(feature = "std"), "std"),
        (cfg!(feature = "alloc"), "alloc"),
        (cfg!(feature = "observe"), "observe"),
        (cfg!(feature = "reload"), "reload"),
        (cfg!(feature = "rdyn_opaque"), "rdyn_opaque"),
    ] {
        if on {
            features.push(name);
        }
    }
    let mut out = alloc::vec![
        ("lumen-static", alloc::string::String::from(env!("CARGO_PKG_VERSION"))),
        ("lumen-static features", features.join(",")),
    ];
    out.extend(crossing_types().iter().map(|c| (c.name, format!("{}/{}", c.size, c.align))));
    out
}

/// One type a hot library's values cross the boundary as.
pub struct Crossing {
    /// The stamp's field name.
    pub name: &'static str,
    /// [`core::any::type_name`]'s path: which crate defines it.
    pub path: &'static str,
    pub size: usize,
    pub align: usize,
}

fn crossing<T>(name: &'static str) -> Crossing {
    Crossing {
        name,
        path: core::any::type_name::<T>(),
        size: core::mem::size_of::<T>(),
        align: core::mem::align_of::<T>(),
    }
}

/// **The core's types that cross a hot library's boundary**, once:
/// [`build_fields`] stamps their sizes, and `lumen-static-reload`'s tests
/// check each is defined in a crate its source hash covers (M49/T49.1) —
/// a crossing type defined anywhere else could change with no line of
/// the stamp moving.
pub fn crossing_types() -> [Crossing; 6] {
    [
        crossing::<crate::DisplayList>("DisplayList"),
        crossing::<crate::display::Cmd>("Cmd"),
        crossing::<crate::Event>("Event"),
        crossing::<crate::NodeState>("NodeState"),
        crossing::<crate::style::Style>("Style"),
        crossing::<crate::retain::Damage>("Damage"),
    ]
}

pub mod symbols {
    /// `fn() -> *mut c_void` — the application, built by this build's
    /// boot function (M49/T49.4: `export_reloadable_app!` takes it).
    pub const NEW: &[u8] = b"hot_new";
    /// `fn(*mut c_void)` — drop the tree while this library is still
    /// loaded. Step 2 of §7's sequence. Null is accepted and does nothing
    /// (M49/T49.2).
    pub const DROP: &[u8] = b"hot_drop";
    /// `fn(*mut c_void, *mut u8, usize) -> usize` — serialize the keep
    /// half into a host-owned buffer. Two-call protocol: pass a null
    /// pointer and zero capacity to size, then allocate and call again.
    /// A null tree answers `usize::MAX` (M49/T49.2).
    pub const SAVE: &[u8] = b"hot_save";
    /// `fn(*mut c_void, *const u8, usize) -> i32` — graft saved bytes
    /// onto this build's fresh tree. Returns 0 on success; `(null, 0)` is
    /// accepted and answers 1, "not a saved state" (M49/T49.2).
    pub const RESTORE: &[u8] = b"hot_restore";
    /// `fn(*mut u8, usize) -> usize` — drain the fallback log. Same
    /// two-call protocol as [`SAVE`], and it must not discard the log
    /// when the buffer is too small (F1.7).
    pub const TAKE_LOG: &[u8] = b"hot_take_log";
    /// `fn(*mut u8, usize) -> usize` — drain the report of the last panic
    /// a shim caught inside the library, as `panicked at file:line:col:
    /// message`. Same two-call protocol; zero means none (M36/T36.4).
    pub const TAKE_PANIC: &[u8] = b"hot_take_panic";
    /// `fn(*mut u8, usize) -> usize` — this library's ABI stamp
    /// (M37/T37.0), `name=value` lines. Same two-call protocol. A loader
    /// reads it **before** calling anything that returns a Rust value.
    pub const ABI_STAMP: &[u8] = b"lumen_abi_stamp";
    /// `fn(*const u8, usize) -> *mut c_void` — a window library's session
    /// (M37/T37.1): its own `program()` booted, the saved bytes (null for
    /// none) restored onto it, as a thin pointer to a
    /// `Box<dyn lumen_static_shell::session::Session>`. Null if it
    /// panicked; the report is in [`TAKE_PANIC`].
    pub const BOOT: &[u8] = b"lumen_boot";
    /// `fn(*mut c_void)` — drop a session inside the library that built
    /// it, so its application's `Drop` impls run there, caught. Null is
    /// accepted and does nothing (M49/T49.2).
    pub const DROP_SESSION: &[u8] = b"lumen_drop";
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
