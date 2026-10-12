//! **Every raw-pointer operation the shims perform, in one
//! dependency-free file** — M49/T49.2 and T49.3
//! (`docs/plan-reload-hardening.md`, R4-F6 and R4-F8).
//!
//! The generated `extern "C"` shims, the panic report's drain and the
//! loader's session round trip all go through these functions and do no
//! pointer arithmetic of their own. Two reasons:
//!
//! - **The null guards live once.** `(null, 0)` is how the two-call
//!   protocol spells "nothing", and `SAVE` and the stamp always accepted
//!   it, while `hot_restore` handed it to `slice::from_raw_parts` —
//!   undefined behaviour even for an empty slice — and `hot_drop` and
//!   `fulgurite_drop` handed null to `Box::from_raw`.
//! - **miri can reach them.** `cargo miri` cannot build this crate here
//!   (every build script fails; `docs/plan-reload-hardening.md`, T49.0),
//!   and cannot run dlopen anywhere. This file uses nothing but `core`
//!   and `alloc`, so `fixtures/miri-reload` (`just miri-reload`)
//!   includes it by path — the same file, not a transcription (T49.3) —
//!   and drives every call shape the shims make. Keep it that way: an
//!   import from outside `core`/`alloc` breaks that crate.

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ffi::c_void;

/// `(ptr, len)` as a byte slice; **null reads as empty**, whatever `len`.
///
/// # Safety
/// A non-null `ptr` must be valid for reads of `len` bytes for `'a`.
pub unsafe fn bytes<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if ptr.is_null() {
        return &[];
    }
    // SAFETY: non-null, and the caller vouches for the range.
    unsafe { core::slice::from_raw_parts(ptr, len) }
}

/// The two-call protocol's writing half: copy `src` into `(buf, cap)`
/// when it fits and `buf` is not null, and return `src.len()` either way
/// — so `(null, 0)` sizes and a too-small buffer is left untouched.
///
/// # Safety
/// A non-null `buf` must be valid for writes of `cap` bytes.
pub unsafe fn copy_out(src: &[u8], buf: *mut u8, cap: usize) -> usize {
    if src.len() <= cap && !buf.is_null() {
        // SAFETY: `buf` holds `cap >= src.len()` writable bytes, and a
        // host-owned buffer cannot overlap a library-owned `src`.
        unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), buf, src.len()) };
    }
    src.len()
}

/// Hand `value` across the boundary as a thin pointer.
pub fn give<T>(value: T) -> *mut c_void {
    Box::into_raw(Box::new(value)) as *mut c_void
}

/// Take back what [`give`] handed out; **null is `None`**.
///
/// # Safety
/// A non-null `p` must come from `give::<T>` and not be used again.
pub unsafe fn take<T>(p: *mut c_void) -> Option<Box<T>> {
    if p.is_null() {
        return None;
    }
    // SAFETY: the caller vouches `p` is `give::<T>`'s, unused since.
    Some(unsafe { Box::from_raw(p as *mut T) })
}

/// Borrow what [`give`] handed out; **null is `None`**.
///
/// # Safety
/// A non-null `p` must come from `give::<T>`, still live, and not be
/// borrowed mutably for `'a`.
pub unsafe fn get<'a, T>(p: *const c_void) -> Option<&'a T> {
    // SAFETY: as the caller vouches.
    unsafe { (p as *const T).as_ref() }
}

/// Borrow what [`give`] handed out, mutably; **null is `None`**.
///
/// # Safety
/// A non-null `p` must come from `give::<T>`, still live, and not be
/// borrowed otherwise for `'a`.
pub unsafe fn get_mut<'a, T>(p: *mut c_void) -> Option<&'a mut T> {
    // SAFETY: as the caller vouches.
    unsafe { (p as *mut T).as_mut() }
}

/// The two-call protocol's reading half: size with `(null, 0)`, allocate,
/// call again, and keep what was written — never more than was sized.
///
/// # Safety
/// `f` must follow the protocol: write at most `cap` bytes to `buf`.
pub unsafe fn read(f: unsafe extern "C" fn(*mut u8, usize) -> usize) -> Vec<u8> {
    // SAFETY: `(null, 0)` asks for the size only.
    let need = unsafe { f(core::ptr::null_mut(), 0) };
    let mut buf = alloc::vec![0u8; need];
    // SAFETY: `buf` holds `need` writable bytes.
    let wrote = unsafe { f(buf.as_mut_ptr(), buf.len()) };
    buf.truncate(wrote.min(need));
    buf
}
