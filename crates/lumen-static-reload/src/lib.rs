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
        // A crate may lay a type out differently under debug assertions,
        // and a window's loader and its library are two cargo invocations
        // that could be given two profiles.
        put("debug_assertions", if cfg!(debug_assertions) { "on" } else { "off" });
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

/// **A window's hot library, from both sides** — M37/T37.1
/// (`docs/plan-window-reload.md`).
///
/// The library side is [`boot`](hot::boot), which [`export_program!`]
/// calls: the library's own `program()` is booted and the saved state
/// restored onto it, so a swap re-runs the **new** build's construction
/// (§2.2) and never goes through a pointer held from the old one — rule
/// 1, and M28/T28.7's defect in a new place.
///
/// The loader side is [`Build`](hot::Build): open a library, refuse it
/// unless its ABI stamp matches this image's, and boot sessions from it.
/// A session is borrowed in place rather than copied
/// (`docs/step4-hot-reload.md`, T37.0), which is what the stamp is for.
/// **No library is ever unloaded** (§2.5): each `Build` leaks its handle.
#[cfg(feature = "window")]
pub mod hot {
    use std::ffi::c_void;
    use std::path::{Path, PathBuf};

    use lumen_static::event::App;
    use lumen_static::reload::{symbols, Reloadable};
    use lumen_static_shell::program::Program;
    use lumen_static_shell::session::{abi_fields, FrameSource, Local, Recover, Screen, Session};

    /// This image's ABI stamp, with the shell's fields: what a window
    /// library exports and what its loader compares against.
    pub fn stamp() -> String {
        crate::abi::stamp(&abi_fields())
    }

    /// **The library side**: boot `program`, then graft `saved` onto it.
    /// Recovery is always on — a hot library's panic must be caught in
    /// the library (M36/T36.4) — and the session can save, for the next
    /// swap.
    pub fn boot<A: App + Reloadable + 'static>(program: Program<A>, saved: Option<&[u8]>) -> Box<dyn Session> {
        let mut s = Local::from_program(program, Recover::On)
            .with_saver(|a| lumen_static::reload::to_vec(&Reloadable::save(a)));
        if let Some(v) = saved.and_then(lumen_static::reload::from_slice) {
            s.host.restore(&v);
        }
        Box::new(s)
    }

    type BootFn = unsafe extern "C" fn(*const u8, usize) -> *mut c_void;
    type DropFn = unsafe extern "C" fn(*mut c_void);
    type TakeFn = unsafe extern "C" fn(*mut u8, usize) -> usize;

    /// Why a library was not opened.
    #[derive(Debug)]
    pub enum Refused {
        /// The dynamic loader could not open it.
        Load(String),
        /// It was built differently (the stamp names how).
        Stamp(crate::abi::Mismatch),
        /// It does not export an entry point a window library must.
        Missing(&'static str),
    }

    impl core::fmt::Display for Refused {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                Refused::Load(e) => write!(f, "could not load the library: {e}"),
                Refused::Stamp(m) => write!(f, "{m}"),
                Refused::Missing(s) => write!(f, "the library exports no `{s}` — is it built with `export_program!`?"),
            }
        }
    }

    /// One loaded build of a window library. Leaked, never unloaded.
    pub struct Build {
        path: PathBuf,
        boot: BootFn,
        drop: DropFn,
        take_panic: TakeFn,
    }

    fn sym<T: Copy>(lib: &'static libloading::Library, name: &'static [u8]) -> Result<T, Refused> {
        let shown = core::str::from_utf8(name).unwrap_or("?");
        // SAFETY: `T` is the fn type `symbols` documents for `name`; the
        // stamp was checked before any symbol but the stamp's is read.
        unsafe { lib.get::<T>(name) }.map(|s| *s).map_err(|_| Refused::Missing(shown))
    }

    impl Build {
        /// Open `path`, and refuse it unless it was built as this image
        /// was. Nothing but the stamp is called before the check.
        pub fn open(path: &Path) -> Result<Build, Refused> {
            // SAFETY: loading runs the library's initialisers; a window
            // library is built from this workspace's code.
            let lib = unsafe { libloading::Library::new(path) }.map_err(|e| Refused::Load(e.to_string()))?;
            let lib: &'static libloading::Library = Box::leak(Box::new(lib));
            let read: TakeFn = sym(lib, symbols::ABI_STAMP)?;
            // SAFETY: `lumen_abi_stamp` writes at most `cap` bytes.
            let theirs = unsafe { crate::abi::read(read) };
            crate::abi::check(&stamp(), &theirs).map_err(Refused::Stamp)?;
            Ok(Build {
                path: path.to_path_buf(),
                boot: sym(lib, symbols::BOOT)?,
                drop: sym(lib, symbols::DROP_SESSION)?,
                take_panic: sym(lib, symbols::TAKE_PANIC)?,
            })
        }

        pub fn path(&self) -> &Path {
            &self.path
        }

        /// Boot this build's program, with `saved` restored onto it. `Err`
        /// with the library's panic report if its boot panicked.
        pub fn boot(&self, saved: Option<&[u8]>) -> Result<Hot, String> {
            let (ptr, len) = saved.map_or((core::ptr::null(), 0), |b| (b.as_ptr(), b.len()));
            // SAFETY: `(ptr, len)` is a live slice or null; the library
            // copies what it needs before returning.
            let p = unsafe { (self.boot)(ptr, len) };
            if p.is_null() {
                return Err(self.take_panic().unwrap_or_else(|| "the library's boot returned nothing".into()));
            }
            // SAFETY: a non-null `lumen_boot` result is a thin pointer to a
            // `Box<dyn Session>` the library allocated, and the stamp says
            // both sides lay one out alike.
            let session = unsafe { *Box::from_raw(p as *mut Box<dyn Session>) };
            Ok(Hot { session: Some(session), drop: self.drop })
        }

        /// The report of the last panic the library caught outside a
        /// session's own guard — in `boot` or a drop.
        pub fn take_panic(&self) -> Option<String> {
            // SAFETY: `hot_take_panic` writes at most `cap` bytes.
            let s = unsafe { crate::abi::read(self.take_panic) };
            (!s.is_empty()).then_some(s)
        }
    }

    /// What one build of the application's library produced.
    #[derive(Debug, Clone)]
    pub enum Outcome {
        /// The library, at cargo's output path.
        Built(PathBuf),
        /// The compiler's diagnostics. `output` is cargo's output path,
        /// which may still hold an **earlier** build — the reason a
        /// failed build must never be swapped to (T37.2's injection).
        Failed { log: String, output: PathBuf },
    }

    /// The application crate a window rebuilds as a library (§2.6):
    /// `cargo rustc -p <name> --lib --crate-type cdylib` into its own
    /// target directory, so the running binary's artifacts are left alone.
    #[derive(Debug, Clone)]
    pub struct Crate {
        pub manifest: PathBuf,
        pub name: String,
        pub features: String,
        pub release: bool,
        pub target_dir: PathBuf,
    }

    impl Crate {
        /// The package at `dir` (its `Cargo.toml`'s `[package] name`).
        pub fn at(dir: &Path, features: &str, release: bool, target_dir: PathBuf) -> Result<Crate, String> {
            let manifest = dir.join("Cargo.toml");
            let text = std::fs::read_to_string(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
            let name = text
                .lines()
                .skip_while(|l| l.trim() != "[package]")
                .find_map(|l| {
                    let (k, v) = l.split_once('=')?;
                    (k.trim() == "name").then(|| v.trim().trim_matches('"').to_string())
                })
                .ok_or_else(|| format!("{}: no [package] name", manifest.display()))?;
            Ok(Crate { manifest, name, features: features.to_string(), release, target_dir })
        }

        /// Where cargo writes the library.
        pub fn output(&self) -> PathBuf {
            let lib = format!(
                "{}{}{}",
                std::env::consts::DLL_PREFIX,
                self.name.replace('-', "_"),
                std::env::consts::DLL_SUFFIX
            );
            self.target_dir.join(if self.release { "release" } else { "debug" }).join(lib)
        }

        /// Build it once, synchronously. The diagnostics are cargo's
        /// stderr, in `--message-format short`.
        pub fn build(&self) -> Outcome {
            let mut cmd = std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
            cmd.arg("rustc")
                .arg("--manifest-path")
                .arg(&self.manifest)
                .args(["-p", &self.name, "--lib", "--crate-type", "cdylib", "--message-format", "short"])
                .arg("--target-dir")
                .arg(&self.target_dir);
            if self.release {
                cmd.arg("--release");
            }
            if !self.features.is_empty() {
                cmd.args(["--features", &self.features]);
            }
            match cmd.output() {
                Ok(o) if o.status.success() => Outcome::Built(self.output()),
                Ok(o) => Outcome::Failed { log: String::from_utf8_lossy(&o.stderr).into_owned(), output: self.output() },
                Err(e) => Outcome::Failed { log: format!("could not run cargo: {e}"), output: self.output() },
            }
        }
    }

    /// **When a window swaps, and to what** — M37/T37.2 (§2.6, rule 2).
    ///
    /// A build's outcome arrives from the builder thread; the swap happens
    /// later, **between frames**, when the window calls [`Reloader::swap`].
    /// A failed build, a refused library and a boot that panicked all
    /// leave the running session in place and say why in [`error`].
    ///
    /// [`error`]: Reloader::error
    #[derive(Debug, Default)]
    pub struct Reloader {
        /// The content hash of the last library accepted, so a build that
        /// changed nothing does not swap.
        hash: Option<u64>,
        copies: u32,
        pending: Option<PathBuf>,
        /// Why the last build or swap did not take: the compiler's log, a
        /// stamp refusal, a boot's panic. Cleared by a successful swap.
        pub error: Option<String>,
        /// Swaps completed.
        pub reloads: u32,
        /// A build is running.
        pub building: bool,
    }

    fn hash(bytes: &[u8]) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in bytes {
            h = (h ^ *b as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }

    impl Reloader {
        pub fn new() -> Self {
            Self::default()
        }

        /// A library waiting for the next frame boundary.
        pub fn pending(&self) -> Option<&Path> {
            self.pending.as_deref()
        }

        /// Take a build's outcome. A success whose bytes differ from the
        /// last accepted library is copied to a path of its own — the
        /// dynamic loader caches by path, so reopening one name would hand
        /// back the old image — and waits for [`Reloader::swap`].
        pub fn outcome(&mut self, o: Outcome) {
            self.building = false;
            match o {
                Outcome::Built(path) => {
                    let bytes = match std::fs::read(&path) {
                        Ok(b) => b,
                        Err(e) => {
                            self.error = Some(format!("{}: {e}", path.display()));
                            return;
                        }
                    };
                    let h = hash(&bytes);
                    if self.hash == Some(h) {
                        return;
                    }
                    self.copies += 1;
                    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("lib");
                    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("so");
                    let copy = path.with_file_name(format!("{stem}-hot-{}.{ext}", self.copies));
                    if let Err(e) = std::fs::write(&copy, &bytes) {
                        self.error = Some(format!("{}: {e}", copy.display()));
                        return;
                    }
                    self.hash = Some(h);
                    self.pending = Some(copy);
                }
                Outcome::Failed { log, .. } => {
                    self.error = Some(log);
                }
            }
        }

        /// **At a frame boundary**: if a library is waiting, open it, boot
        /// its program with `current`'s saved state, let `adopt` set it up
        /// (waker, scale, size), and put it in `current`'s place. Whether
        /// the session changed.
        ///
        /// The new session is booted **before** the old one is dropped,
        /// so a refused or panicking build leaves the old one running —
        /// rule 2. Both libraries stay loaded either way (§2.5).
        pub fn swap(&mut self, current: &mut Box<dyn Session>, adopt: impl FnOnce(&mut dyn Session)) -> bool {
            let Some(path) = self.pending.take() else { return false };
            let build = match Build::open(&path) {
                Ok(b) => b,
                Err(e) => {
                    self.error = Some(format!("{}: {e}", path.display()));
                    return false;
                }
            };
            let saved = current.save();
            match build.boot(Some(&saved)) {
                Ok(hot) => {
                    let mut next: Box<dyn Session> = Box::new(hot);
                    adopt(&mut *next);
                    *current = next;
                    self.reloads += 1;
                    self.error = None;
                    true
                }
                Err(report) => {
                    self.error = Some(format!("the new build panicked while booting: {report}"));
                    false
                }
            }
        }
    }

    /// A session a library built, returned to that library to be dropped.
    pub struct Hot {
        session: Option<Box<dyn Session>>,
        drop: DropFn,
    }

    impl Hot {
        fn s(&self) -> &dyn Session {
            self.session.as_deref().expect("a live session")
        }
        fn m(&mut self) -> &mut dyn Session {
            self.session.as_deref_mut().expect("a live session")
        }
    }

    impl Drop for Hot {
        fn drop(&mut self) {
            if let Some(s) = self.session.take() {
                let p = Box::into_raw(Box::new(s)) as *mut c_void;
                // SAFETY: `p` is the box `Build::boot` took from this
                // library, handed back once.
                unsafe { (self.drop)(p) };
            }
        }
    }

    impl FrameSource for Hot {
        fn list(&self) -> &lumen_static::DisplayList {
            self.s().list()
        }
        fn take_damage_for_age(&mut self, age: u8) -> lumen_static::retain::Damage {
            self.m().take_damage_for_age(age)
        }
        fn timings(&self) -> (u64, u64) {
            self.s().timings()
        }
    }

    impl Session for Hot {
        fn post(&mut self, ev: lumen_static::Event) {
            self.m().post(ev)
        }
        fn copy(&self) -> Option<String> {
            self.s().copy()
        }
        fn paste(&mut self, text: &str) {
            self.m().paste(text)
        }
        fn compose(&mut self, c: lumen_static::event::Composition<'_>) -> bool {
            self.m().compose(c)
        }
        fn set_scale(&mut self, scale: f32) {
            self.m().set_scale(scale)
        }
        fn scale(&self) -> f32 {
            self.s().scale()
        }
        fn to_logical(&self, x: f32, y: f32) -> (f32, f32) {
            self.s().to_logical(x, y)
        }
        fn resize(&mut self, width: u32, height: u32) {
            self.m().resize(width, height)
        }
        fn frame(&mut self, dt_ms: f32, width: u32, height: u32) -> bool {
            self.m().frame(dt_ms, width, height)
        }
        fn failed(&self) -> bool {
            self.s().failed()
        }
        fn retry(&mut self) -> bool {
            self.m().retry()
        }
        fn title(&self) -> String {
            self.s().title()
        }
        fn cursor(&self) -> lumen_static::style::Cursor {
            self.s().cursor()
        }
        fn next_deadline_ms(&mut self) -> Option<f32> {
            self.m().next_deadline_ms()
        }
        fn more_deferred(&self) -> bool {
            self.s().more_deferred()
        }
        fn set_waker(&mut self, waker: std::sync::Arc<dyn lumen_static::inbox::Waker>) {
            self.m().set_waker(waker)
        }
        fn page(&self) -> lumen_static::Color {
            self.s().page()
        }
        fn agent(&mut self, req: &str, screen: &mut dyn Screen) -> String {
            self.m().agent(req, screen)
        }
        fn agent_frame_done(&mut self) {
            self.m().agent_frame_done()
        }
        fn save(&self) -> Vec<u8> {
            self.s().save()
        }
    }
}

/// Export a window library's entry points — M37/T37.1.
///
/// ```ignore
/// pub fn program() -> Program<Counter> { application(Counter::build) }
/// lumen_static_reload::export_program!(program);
/// ```
///
/// The stamp (with the shell's fields), `lumen_boot`, `lumen_drop` and
/// `hot_take_panic`. Every one catches a panic inside the library
/// (M36/T36.4); a session's own methods catch theirs in the session.
#[cfg(feature = "window")]
#[macro_export]
macro_rules! export_program {
    ($program:path) => {
        $crate::export_abi_stamp!(&::lumen_static_shell::session::abi_fields());

        /// Boot this build's program with the saved state (null for none).
        ///
        /// # Safety
        /// `(saved, len)` must be a valid readable range, or null.
        #[no_mangle]
        pub unsafe extern "C" fn lumen_boot(saved: *const u8, len: usize) -> *mut ::core::ffi::c_void {
            let saved = if saved.is_null() {
                ::core::option::Option::None
            } else {
                ::core::option::Option::Some(unsafe { ::core::slice::from_raw_parts(saved, len) })
            };
            $crate::panic::guard(::core::ptr::null_mut(), || {
                let s = $crate::hot::boot($program(), saved);
                ::std::boxed::Box::into_raw(::std::boxed::Box::new(s)) as *mut ::core::ffi::c_void
            })
        }

        /// Drop a session this library booted.
        ///
        /// # Safety
        /// `p` must come from this build's `lumen_boot`, handed back once.
        #[no_mangle]
        pub unsafe extern "C" fn lumen_drop(p: *mut ::core::ffi::c_void) {
            let b = unsafe {
                ::std::boxed::Box::from_raw(p as *mut ::std::boxed::Box<dyn ::lumen_static_shell::session::Session>)
            };
            $crate::panic::guard((), move || drop(b));
        }

        /// Drain the last caught panic's report.
        ///
        /// # Safety
        /// `(buf, cap)` must be a valid writable range or `(null, 0)`.
        #[no_mangle]
        pub unsafe extern "C" fn hot_take_panic(buf: *mut u8, cap: usize) -> usize {
            unsafe { $crate::panic::take(buf, cap) }
        }
    };
}
