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
/// Only the four generic entry points are generated. An app's own
/// verbs — `hot_bump`, `hot_set_title` — are app-specific and stay
/// hand-written; there is nothing to share there.
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
            ::std::boxed::Box::into_raw(::std::boxed::Box::new(
                <$ty as ::lumen_static::reload::Reloadable>::fresh(),
            )) as *mut ::core::ffi::c_void
        }

        /// Drop the tree while this dylib is still loaded — step 2 of
        /// §7's sequence.
        ///
        /// # Safety
        /// `p` must come from this build's `hot_new` and not be used
        /// afterwards.
        #[no_mangle]
        pub unsafe extern "C" fn hot_drop(p: *mut ::core::ffi::c_void) {
            drop(unsafe { ::std::boxed::Box::from_raw(p as *mut $ty) });
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
            let bytes = ::lumen_static::reload::to_vec(&::lumen_static::reload::Reloadable::save(app));
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
                    ::lumen_static::reload::Reloadable::restore(app, &v);
                    0
                }
                ::core::option::Option::None => 1,
            }
        }
    };
}

