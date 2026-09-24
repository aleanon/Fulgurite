//! The hot-reload FFI surface, in its own crate.
//!
//! Separate from `lumen-static` for one reason: this is `unsafe` by
//! nature — raw pointers and lengths across a dylib boundary — and the
//! core crate's standing invariant is that it contains none at all,
//! enforced by `scripts/check_no_unsafe.sh` over `crates/*/src`. Putting
//! the shims in `lumen_static::reload` would have meant weakening that
//! check to allow a `cfg`-gated exception, which is how a gate stops
//! meaning anything (`docs/plan-review-fixes.md` F6.7).
//!
//! The symbol *names* stay in the core, in
//! [`lumen_static::reload::symbols`], because they are byte strings and
//! the loader needs them without taking on this crate.
//!
//! ```ignore
//! lumen_static_reload::export_reloadable_app!(Session);
//! ```

/// Emit the `extern "C"` shims a reloadable app must export.
///
/// ```ignore
/// export_reloadable_app!(Session);
/// ```
///
/// Replaces about 90 lines of hand-written FFI per app, and — more to
/// the point — makes the size-probe protocol one implementation rather
/// than three. The two fixtures already implemented it differently: the
/// host skipped the probe for one of the five calls, which is how an
/// oversized fallback log came to panic *after* the log had been drained
/// (`docs/plan-review-fixes.md` F6.7, F1.7).
///
/// Only the four lifecycle entry points and `hot_take_panic` are
/// generated, and each lifecycle shim catches a panic inside the library
/// ([`mod@panic`]). An app's own verbs — `hot_bump`, `hot_set_title` — are
/// app-specific and stay hand-written; there is nothing to share there,
/// except that a verb which runs application code wraps its body in
/// [`panic::guard`](crate::panic::guard), as `fixtures/hot-app`'s `hot_frame` does.
#[macro_export]
macro_rules! export_reloadable_app {
    ($ty:ty) => {
        /// `App::fresh()` from this dylib's code.
        ///
        /// # Safety
        /// The returned pointer must be passed only to this build's
        /// other entry points, and freed only via `hot_drop`.
        #[no_mangle]
        pub unsafe extern "C" fn hot_new() -> *mut ::core::ffi::c_void {
            // Null on a panic in `fresh()`; the report is in `hot_take_panic`.
            $crate::panic::guard(::core::ptr::null_mut(), || {
                ::std::boxed::Box::into_raw(::std::boxed::Box::new(
                    <$ty as ::lumen_static::reload::Reloadable>::fresh(),
                )) as *mut ::core::ffi::c_void
            })
        }

        /// Drop the tree while this dylib is still loaded — step 2 of
        /// §7's sequence.
        ///
        /// # Safety
        /// `p` must come from this build's `hot_new` and not be used
        /// afterwards.
        #[no_mangle]
        pub unsafe extern "C" fn hot_drop(p: *mut ::core::ffi::c_void) {
            let b = unsafe { ::std::boxed::Box::from_raw(p as *mut $ty) };
            $crate::panic::guard((), move || drop(b));
        }

        /// Serialize the keep half into a host-owned buffer.
        ///
        /// # Safety
        /// `p` must come from this build's `hot_new`; `(buf, cap)` must
        /// be a valid writable range or `(null, 0)` to size.
        #[no_mangle]
        pub unsafe extern "C" fn hot_save(
            p: *mut ::core::ffi::c_void,
            buf: *mut u8,
            cap: usize,
        ) -> usize {
            let app: &$ty = unsafe { &*(p as *const $ty) };
            // `usize::MAX` on a panic in `save()`: no buffer is that large.
            let Some(bytes) = $crate::panic::guard(None, || {
                Some(::lumen_static::reload::to_vec(&::lumen_static::reload::Reloadable::save(app)))
            }) else {
                return usize::MAX;
            };
            if bytes.len() <= cap && !buf.is_null() {
                unsafe {
                    ::core::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, bytes.len());
                }
            }
            bytes.len()
        }

        /// Graft saved bytes onto this build's fresh tree.
        ///
        /// # Safety
        /// `p` must come from this build's `hot_new`; `(ptr, len)` must
        /// be a valid readable range.
        #[no_mangle]
        pub unsafe extern "C" fn hot_restore(
            p: *mut ::core::ffi::c_void,
            ptr: *const u8,
            len: usize,
        ) -> i32 {
            let bytes = unsafe { ::core::slice::from_raw_parts(ptr, len) };
            match ::lumen_static::reload::from_slice(bytes) {
                ::core::option::Option::Some(v) => {
                    let app: &mut $ty = unsafe { &mut *(p as *mut $ty) };
                    // 2 on a panic in `restore()`.
                    $crate::panic::guard(2, || {
                        ::lumen_static::reload::Reloadable::restore(app, &v);
                        0
                    })
                }
                ::core::option::Option::None => 1,
            }
        }

        /// Drain the report of the last panic a shim caught — M36/T36.4.
        /// Two-call protocol, as `hot_save`; zero means none.
        ///
        /// # Safety
        /// `(buf, cap)` must be a valid writable range or `(null, 0)`.
        #[no_mangle]
        pub unsafe extern "C" fn hot_take_panic(buf: *mut u8, cap: usize) -> usize {
            unsafe { $crate::panic::take(buf, cap) }
        }
    };
}


/// **A panic inside a loaded library is caught there, with the library's
/// own hook** — M36/T36.4 (`docs/plan-panic-recovery.md` §2.5).
///
/// A `cdylib` links its own copy of std, so it has its own panic hook: a
/// hook the loader installs never sees a panic raised in library code,
/// and the location — which exists only inside a hook — is lost. So the
/// hook, the record and the catch all live here, in the crate the
/// **library** links, and every loaded build gets fresh copies of these
/// statics. [`guard`](crate::panic::guard) installs the hook on the first shim call of each
/// build.
///
/// The report crosses the boundary as plain bytes, drained by the
/// generated `hot_take_panic`. Not `extern "C-unwind"`: the payload is a
/// `Box<dyn Any>` from another build, and a `TypeId` is not stable
/// across builds. Without a catch, a panic reaching an `extern "C"`
/// boundary aborts the loader (Rust 1.81).
pub mod panic {
    use std::cell::RefCell;
    use std::sync::{Mutex, Once};

    thread_local! {
        static LAST: RefCell<Option<String>> = const { RefCell::new(None) };
    }

    static HOOK: Once = Once::new();
    static REPORT: Mutex<Option<String>> = Mutex::new(None);

    /// Install this build's recording hook. It chains the previous hook,
    /// so stderr still gets std's report. Once per loaded build.
    pub fn install() {
        HOOK.call_once(|| {
            let prev = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                let msg = match info.payload().downcast_ref::<&str>() {
                    Some(s) => (*s).to_string(),
                    None => info.payload().downcast_ref::<String>().cloned().unwrap_or_default(),
                };
                let line = match info.location() {
                    Some(l) => format!("panicked at {}:{}:{}: {msg}", l.file(), l.line(), l.column()),
                    None => format!("panicked (no location): {msg}"),
                };
                let _ = LAST.try_with(|c| *c.borrow_mut() = Some(line));
                prev(info);
            }));
        });
    }

    /// Run a shim's body, catching a panic. On a panic the report is
    /// kept for `hot_take_panic` and `fallback` is returned.
    pub fn guard<R>(fallback: R, f: impl FnOnce() -> R) -> R {
        install();
        let _ = LAST.try_with(|c| c.borrow_mut().take());
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
            Ok(r) => r,
            Err(payload) => {
                let recorded = LAST.try_with(|c| c.borrow_mut().take()).ok().flatten();
                let line = recorded.unwrap_or_else(|| {
                    let msg = match payload.downcast_ref::<&str>() {
                        Some(s) => (*s).to_string(),
                        None => payload.downcast_ref::<String>().cloned().unwrap_or_default(),
                    };
                    format!("panicked (location unavailable — this build's hook did not record): {msg}")
                });
                if let Ok(mut r) = REPORT.lock() {
                    *r = Some(line);
                }
                fallback
            }
        }
    }

    /// Drain the last report into `(buf, cap)` — the two-call protocol:
    /// returns the length, and keeps the report when `cap` is too small.
    /// Zero means no panic since the last drain.
    ///
    /// # Safety
    /// `(buf, cap)` must be a valid writable range, or `(null, 0)`.
    pub unsafe fn take(buf: *mut u8, cap: usize) -> usize {
        let Ok(mut r) = REPORT.lock() else { return 0 };
        let Some(s) = r.as_ref() else { return 0 };
        let n = s.len();
        if n <= cap && !buf.is_null() {
            unsafe { core::ptr::copy_nonoverlapping(s.as_ptr(), buf, n) };
            *r = None;
        }
        n
    }
}

/// **The ABI stamp** — M37/T37.0 (`docs/plan-window-reload.md` §2.3).
///
/// A window loader borrows a hot library's values in place — its display
/// list, its trait objects — rather than copying them as bytes, because
/// a stable encoding cost more than the frame's own noise
/// (`docs/step4-hot-reload.md`, T37.0). That is sound only while both
/// images lay out every crossing type identically, which Rust promises
/// for one compiler, one source and one feature set, and nothing else.
/// So each image describes its build as `name=value` lines, and the
/// loader refuses a library whose lines differ from its own, **naming
/// the first field that differs**.
///
/// It cannot see everything: a `#[global_allocator]` in the library is
/// invisible to it, and a hot library must not set one, because values
/// allocated on one side are freed on the other.
pub mod abi {
    /// This image's stamp: the compiler, the target, the panic strategy,
    /// the core's build ([`lumen_static::reload::build_fields`]), and
    /// whatever the caller adds for the crates above the core.
    pub fn stamp(extra: &[(&'static str, String)]) -> String {
        let mut out = String::new();
        let mut put = |k: &str, v: &str| {
            out.push_str(k);
            out.push('=');
            out.push_str(v);
            out.push('\n');
        };
        put("rustc", env!("LUMEN_RUSTC"));
        put("target", env!("LUMEN_TARGET"));
        put("panic", if cfg!(panic = "unwind") { "unwind" } else { "abort" });
        for (k, v) in lumen_static::reload::build_fields() {
            put(k, &v);
        }
        for (k, v) in extra {
            put(k, v);
        }
        out
    }

    /// Why a library was refused: the first field whose value differs,
    /// or that one side has and the other does not.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Mismatch {
        pub field: String,
        pub loader: Option<String>,
        pub library: Option<String>,
    }

    impl core::fmt::Display for Mismatch {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            let show = |v: &Option<String>| v.clone().unwrap_or_else(|| "(absent)".into());
            write!(
                f,
                "the library was built differently: `{}` is `{}` here and `{}` in the library",
                self.field,
                show(&self.loader),
                show(&self.library)
            )
        }
    }

    /// Compare the loader's stamp with a library's, field by field in the
    /// loader's order, then any field only the library has.
    pub fn check(loader: &str, library: &str) -> Result<(), Mismatch> {
        let parse = |s: &str| -> Vec<(String, String)> {
            s.lines()
                .filter_map(|l| l.split_once('='))
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        };
        let (a, b) = (parse(loader), parse(library));
        let get = |v: &[(String, String)], k: &str| v.iter().find(|(x, _)| x == k).map(|(_, y)| y.clone());
        for (k, v) in &a {
            let theirs = get(&b, k);
            if theirs.as_deref() != Some(v.as_str()) {
                return Err(Mismatch { field: k.clone(), loader: Some(v.clone()), library: theirs });
            }
        }
        for (k, v) in &b {
            if get(&a, k).is_none() {
                return Err(Mismatch { field: k.clone(), loader: None, library: Some(v.clone()) });
            }
        }
        Ok(())
    }

    /// Read a loaded library's stamp through its `lumen_abi_stamp`.
    ///
    /// # Safety
    /// `f` must be a library's `lumen_abi_stamp` export.
    pub unsafe fn read(f: unsafe extern "C" fn(*mut u8, usize) -> usize) -> String {
        let need = unsafe { f(core::ptr::null_mut(), 0) };
        let mut buf = vec![0u8; need];
        let wrote = unsafe { f(buf.as_mut_ptr(), buf.len()) };
        buf.truncate(wrote.min(need));
        String::from_utf8_lossy(&buf).into_owned()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn the_first_differing_field_is_named_and_an_identical_stamp_passes() {
            let here = stamp(&[("extra", "1".into())]);
            assert_eq!(check(&here, &here), Ok(()), "an identical build must be accepted");
            let other = here.replace("\nextra=1", "\nextra=2");
            let m = check(&here, &other).unwrap_err();
            assert_eq!(m.field, "extra");
            let missing = here.replace("extra=1\n", "");
            assert_eq!(check(&here, &missing).unwrap_err().library, None);
            let more = format!("{here}new=1\n");
            assert_eq!(check(&here, &more).unwrap_err().field, "new");
        }
    }
}

/// Export this image's ABI stamp as `lumen_abi_stamp` (M37/T37.0). The
/// argument is the extra fields, as for [`abi::stamp`].
#[macro_export]
macro_rules! export_abi_stamp {
    () => {
        $crate::export_abi_stamp!(&[]);
    };
    ($extra:expr) => {
        /// This library's ABI stamp. Two-call protocol.
        ///
        /// # Safety
        /// `(buf, cap)` must be a valid writable range or `(null, 0)`.
        #[no_mangle]
        pub unsafe extern "C" fn lumen_abi_stamp(buf: *mut u8, cap: usize) -> usize {
            let s = $crate::abi::stamp($extra);
            if s.len() <= cap && !buf.is_null() {
                unsafe { ::core::ptr::copy_nonoverlapping(s.as_ptr(), buf, s.len()) };
            }
            s.len()
        }
    };
}
