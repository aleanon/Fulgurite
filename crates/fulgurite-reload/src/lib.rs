//! The hot-reload FFI surface, in its own crate.
//!
//! Separate from `fulgurite` for one reason: this is `unsafe` by
//! nature — raw pointers and lengths across a dylib boundary — and the
//! core crate's standing invariant is that it contains none at all,
//! enforced by `scripts/check_no_unsafe.sh` over `crates/*/src`. Putting
//! the shims in `fulgurite::reload` would have meant weakening that
//! check to allow a `cfg`-gated exception, which is how a gate stops
//! meaning anything (`docs/plan-review-fixes.md` F6.7).
//!
//! The symbol *names* stay in the core, in
//! [`fulgurite::reload::symbols`], because they are byte strings and
//! the loader needs them without taking on this crate.
//!
//! ```ignore
//! fulgurite_reload::export_reloadable_app!(Session, Session::build);
//! ```

/// Emit the `extern "C"` shims a reloadable app must export.
///
/// ```ignore
/// export_reloadable_app!(Session, Session::build);
/// ```
///
/// The second argument is the **boot function**, `fn() -> Session`, from
/// this build's code (M49/T49.4): `hot_new` calls it. It used to call
/// `Reloadable::fresh`, which the derive routed to `Application::build` —
/// a constructor trait nothing else used.
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
    ($ty:ty, $boot:expr) => {
        /// The application, built by this dylib's boot function.
        ///
        /// # Safety
        /// The returned pointer must be passed only to this build's
        /// other entry points, and freed only via `hot_drop`.
        #[no_mangle]
        pub unsafe extern "C" fn hot_new() -> *mut ::core::ffi::c_void {
            // Null on a panic in the boot; the report is in `hot_take_panic`.
            $crate::panic::guard(::core::ptr::null_mut(), || {
                let boot: fn() -> $ty = $boot;
                $crate::raw::give(boot())
            })
        }

        /// Drop the tree while this dylib is still loaded — step 2 of
        /// §7's sequence.
        ///
        /// # Safety
        /// `p` must come from this build's `hot_new` and not be used
        /// afterwards, or be null, which does nothing (M49/T49.2).
        #[no_mangle]
        pub unsafe extern "C" fn hot_drop(p: *mut ::core::ffi::c_void) {
            if let ::core::option::Option::Some(b) = unsafe { $crate::raw::take::<$ty>(p) } {
                $crate::panic::guard((), move || drop(b));
            }
        }

        /// Serialize the keep half into a host-owned buffer.
        ///
        /// # Safety
        /// `p` must come from this build's `hot_new`, or be null, which
        /// answers `usize::MAX` (M49/T49.2); `(buf, cap)` must be a valid
        /// writable range or `(null, 0)` to size.
        #[no_mangle]
        pub unsafe extern "C" fn hot_save(
            p: *mut ::core::ffi::c_void,
            buf: *mut u8,
            cap: usize,
        ) -> usize {
            let ::core::option::Option::Some(app) = (unsafe { $crate::raw::get::<$ty>(p) }) else {
                return usize::MAX;
            };
            // `usize::MAX` on a panic in `save()`: no buffer is that large.
            let Some(bytes) = $crate::panic::guard(None, || {
                Some(::fulgurite::reload::to_vec(&::fulgurite::reload::Reloadable::save(app)))
            }) else {
                return usize::MAX;
            };
            unsafe { $crate::raw::copy_out(&bytes, buf, cap) }
        }

        /// Graft saved bytes onto this build's fresh tree.
        ///
        /// Returns 0 on success, 1 when the bytes are not a saved state
        /// (`(null, 0)` included — M49/T49.2) or `p` is null, and 2 on a
        /// panic in `restore()`.
        ///
        /// # Safety
        /// `p` must come from this build's `hot_new`, or be null;
        /// `(ptr, len)` must be a valid readable range, or have a null
        /// `ptr`, which reads as empty.
        #[no_mangle]
        pub unsafe extern "C" fn hot_restore(
            p: *mut ::core::ffi::c_void,
            ptr: *const u8,
            len: usize,
        ) -> i32 {
            let bytes = unsafe { $crate::raw::bytes(ptr, len) };
            let ::core::option::Option::Some(app) = (unsafe { $crate::raw::get_mut::<$ty>(p) }) else {
                return 1;
            };
            match ::fulgurite::reload::from_slice(bytes) {
                ::core::option::Option::Some(v) => {
                    // 2 on a panic in `restore()`.
                    $crate::panic::guard(2, || {
                        ::fulgurite::reload::Reloadable::restore(app, &v);
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


#[doc(hidden)]
pub mod raw;

#[cfg(test)]
mod shim_tests;

pub mod panic;

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
/// "One source" is checked, not assumed (M49/T49.1): sizes cannot see a
/// reordered enum, so [`source_fields`](abi::source_fields) carries a
/// hash of each crate whose types cross, taken by this crate's build
/// script. Any edit to those crates refuses every later library until
/// the window restarts, and the refusal says so; `fulgurite-widgets`
/// is not hashed, because no widget type crosses.
///
/// It cannot see everything: transitive dependency versions are not in
/// it (`docs/step4-hot-reload.md`, the stamp), a `#[global_allocator]`
/// in the library is invisible to it, and a hot library must not set one, because values
/// allocated on one side are freed on the other.
pub mod abi {
    /// This image's stamp: the compiler, the target, the panic strategy,
    /// the framework's source, the core's build ([`fulgurite::reload::build_fields`]), and
    /// whatever the caller adds for the crates above the core.
    pub fn stamp(extra: &[(&'static str, String)]) -> String {
        let mut out = String::new();
        let mut put = |k: &str, v: &str| {
            out.push_str(k);
            out.push('=');
            out.push_str(v);
            out.push('\n');
        };
        put("rustc", env!("FULGURITE_RUSTC"));
        put("target", env!("FULGURITE_TARGET"));
        put("panic", if cfg!(panic = "unwind") { "unwind" } else { "abort" });
        // A crate may lay a type out differently under debug assertions,
        // and a window's loader and its library are two cargo invocations
        // that could be given two profiles.
        put("debug_assertions", if cfg!(debug_assertions) { "on" } else { "off" });
        // **The source, not only the sizes** (M49/T49.1, R4-F5): a
        // reordered enum keeps every size below and changes every tag.
        // `build.rs` hashes each crate whose types cross, the lockfile
        // that pins their dependencies, and the rustflags.
        for (k, v) in source_fields() {
            put(k, v);
        }
        for (k, v) in fulgurite::reload::build_fields() {
            put(k, &v);
        }
        for (k, v) in extra {
            put(k, v);
        }
        out
    }

    /// The source half of the stamp: one line per crate whose types
    /// cross, then the lockfile and the rustflags (see `build.rs`).
    /// `fulgurite-widgets` is not here: no widget type crosses.
    pub fn source_fields() -> [(&'static str, &'static str); 5] {
        [
            ("source fulgurite", env!("FULGURITE_SRC_CORE")),
            ("source fulgurite-shell", env!("FULGURITE_SRC_SHELL")),
            ("source fulgurite-reload", env!("FULGURITE_SRC_RELOAD")),
            ("source Cargo.lock", env!("FULGURITE_SRC_LOCK")),
            ("rustflags", env!("FULGURITE_RUSTFLAGS")),
        ]
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
            // The source moved under a running window: nothing is wrong
            // with the library, but the loader still runs the old
            // framework, and only a restart changes that.
            if let Some(what) = self.field.strip_prefix("source ") {
                return write!(
                    f,
                    "the framework was edited since this window started: `{what}` hashes `{}` here and `{}` \
                     in the library; restart the window to pick up the edit",
                    show(&self.loader),
                    show(&self.library)
                );
            }
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

    /// Read a loaded library's stamp through its `fulgurite_abi_stamp`.
    ///
    /// # Safety
    /// `f` must be a library's `fulgurite_abi_stamp` export.
    pub unsafe fn read(f: unsafe extern "C" fn(*mut u8, usize) -> usize) -> String {
        // SAFETY: as the caller vouches.
        String::from_utf8_lossy(&unsafe { crate::raw::read(f) }).into_owned()
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

        /// §5's risk, checked: a type that crosses must be defined in a
        /// crate `build.rs` hashes, or it could change under a stamp that
        /// did not. Prints each type's path beside the crates allowed.
        #[test]
        fn every_crossing_type_is_defined_in_a_hashed_crate() {
            let hashed: Vec<String> = source_fields()
                .iter()
                .filter_map(|(k, _)| k.strip_prefix("source fulgurite"))
                .map(|rest| format!("fulgurite{}::", rest.replace('-', "_")))
                .collect();
            let mut paths: Vec<String> =
                fulgurite::reload::crossing_types().iter().map(|c| c.path.to_string()).collect();
            #[cfg(feature = "window")]
            paths.push(core::any::type_name::<Box<dyn fulgurite_shell::session::Session>>().to_string());
            println!("hashed crates: {hashed:?}");
            for p in &paths {
                let ok = hashed.iter().any(|h| p.contains(h.as_str()));
                println!("  {p}: {}", if ok { "hashed" } else { "NOT HASHED" });
                assert!(ok, "`{p}` crosses the boundary from a crate the stamp does not hash");
            }
            assert!(paths.len() >= 6, "the list of crossing types is shorter than the stamp's: {}", paths.len());
        }

        #[test]
        fn a_source_edit_names_the_crate_and_says_to_restart() {
            let here = stamp(&[]);
            let (k, v) = source_fields()[0];
            let edited = here.replace(&format!("{k}={v}"), &format!("{k}=0"));
            let m = check(&here, &edited).unwrap_err();
            let shown = m.to_string();
            println!("{shown}");
            assert_eq!(m.field, "source fulgurite");
            assert!(shown.contains("`fulgurite`") && shown.contains("restart"), "{shown}");
        }
    }
}

/// Export this image's ABI stamp as `fulgurite_abi_stamp` (M37/T37.0). The
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
        pub unsafe extern "C" fn fulgurite_abi_stamp(buf: *mut u8, cap: usize) -> usize {
            let s = $crate::abi::stamp($extra);
            unsafe { $crate::raw::copy_out(s.as_bytes(), buf, cap) }
        }
    };
}

/// **A window's hot library, from both sides** — M37/T37.1
/// (`docs/plan-window-reload.md`).
///
/// The library side is [`boot`](hot::boot) (in the shell since M89/T89.7,
/// re-exported here), which [`export_program!`]
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

    use fulgurite::reload::symbols;
    use fulgurite_shell::session::{abi_fields, FrameSource, Screen, Session};

    /// This image's ABI stamp, with the shell's fields: what a window
    /// library exports and what its loader compares against.
    pub fn stamp() -> String {
        crate::abi::stamp(&abi_fields())
    }

    /// **Moved into the shell by M89/T89.7** (`docs/plan-windows.md`, hot
    /// reload): `OpenProgram::hot` boots a window program from a library,
    /// and the shell cannot depend on this crate. Re-exported here so every
    /// caller — `export_program!`, the window, the fixtures — keeps its path.
    pub use fulgurite_shell::session::hot::{boot, save, RESERVED};

    /// `fulgurite_boot(index, saved, len)` — indexed since M89/T89.7: 0 is the
    /// library's `program()`, `n` its window table's entry `n`.
    type BootFn = unsafe extern "C" fn(u16, *const u8, usize) -> *mut c_void;
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
    #[derive(Debug)]
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
            // SAFETY: `fulgurite_abi_stamp` writes at most `cap` bytes.
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

        /// Boot this build's program `index` — 0 is its `program()`, `n`
        /// its window table's entry `n` (M89/T89.7) — with `saved` restored
        /// onto it. `Err` with the library's report if its boot panicked or
        /// it has no window program `index`.
        pub fn boot(&self, index: u16, saved: Option<&[u8]>) -> Result<Hot, String> {
            let (ptr, len) = saved.map_or((core::ptr::null(), 0), |b| (b.as_ptr(), b.len()));
            // SAFETY: `(ptr, len)` is a live slice or null; the library
            // copies what it needs before returning.
            let p = unsafe { (self.boot)(index, ptr, len) };
            if p.is_null() {
                return Err(self.take_panic().unwrap_or_else(|| "the library's boot returned nothing".into()));
            }
            // SAFETY: a non-null `fulgurite_boot` result is a thin pointer to a
            // `Box<dyn Session>` the library allocated, and the stamp says
            // both sides lay one out alike.
            let Some(session) = (unsafe { crate::raw::take::<Box<dyn Session>>(p) }) else {
                return Err(self.take_panic().unwrap_or_else(|| "the library's boot returned nothing".into()));
            };
            Ok(Hot { session: Some(*session), drop: self.drop })
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
        /// The content hash of the library **running**, so a build that
        /// changed nothing does not swap. Set by a successful swap, not by
        /// queueing one: a library that was refused or panicked while
        /// booting is retried when it is built again.
        hash: Option<u64>,
        pending: Option<(PathBuf, u64)>,
        /// **The build running** — the one the last successful swap put in
        /// — kept so a window opened after it boots from it
        /// ([`Reloader::open`], M89/T89.7). `None` until the first swap.
        build: Option<Build>,
        /// Why the last build or swap did not take: the compiler's log, a
        /// stamp refusal, a boot's panic. Cleared by a successful swap.
        pub error: Option<String>,
        /// What the last swap **did** take but could not keep all of: one
        /// line per kept field whose type no longer fits, which holds its
        /// boot value (§2.4 — nothing is refused, because refusing would
        /// trap the user on the old code). Replaced by the next swap.
        pub warning: Option<String>,
        /// Swaps completed.
        pub reloads: u32,
        /// Whether the last swap **changed the tree's shape** — M49/T49.11
        /// (R4-F7). A node's identity is its table offset, so an agent
        /// holding node indices must resolve them again; the author id and
        /// a `#a > #b` path survive. `false` for an edit that kept the shape.
        pub shape_changed: bool,
        /// What the last swap did with the old build's **work in flight**
        /// — M49/T49.10 (R4-F3): how many jobs the new build re-issued
        /// through its own `job_for`, and how many it had no job for.
        /// Not a [`warning`](Reloader::warning): a re-issue resets nothing,
        /// and the warning is a bar held over the window until the next
        /// swap. `app.reloads` reports it and the window logs it.
        pub reissued: fulgurite_shell::Reissued,
        /// A build is running.
        pub building: bool,
    }

    /// The copy number, **process-wide** — M88/T88.1. It was a field of
    /// each `Reloader`, so a second `Reloader` in one process numbered its
    /// first copy `-hot-1` again and `std::fs::write` truncated a library
    /// the loader still had mapped: the next symbol lookup through that
    /// image read a rewritten page and the process died with `SIGSEGV`
    /// inside `dlsym`. `fixtures/reload-host` found it, the first loader
    /// to swap the same library with two `Reloader`s
    /// (`docs/plan-platform-requests.md`, What building found).
    static COPIES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

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
            self.pending.as_ref().map(|(p, _)| p.as_path())
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
                        // The build succeeded and is what is running: an
                        // earlier failure no longer describes anything —
                        // the ordinary case of fixing a typo back to the
                        // source that last built.
                        self.error = None;
                        self.pending = None;
                        return;
                    }
                    let n = COPIES.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("lib");
                    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("so");
                    let copy = path.with_file_name(format!("{stem}-hot-{n}.{ext}"));
                    // **Written beside it and renamed over it**, never
                    // written in place: a rename replaces the name and
                    // leaves an image another process still has mapped
                    // intact, where a truncating write rewrites its pages
                    // under it (the defect `COPIES` records).
                    let tmp = path.with_file_name(format!("{stem}-hot-{n}.{ext}.tmp"));
                    if let Err(e) = std::fs::write(&tmp, &bytes).and_then(|()| std::fs::rename(&tmp, &copy)) {
                        self.error = Some(format!("{}: {e}", copy.display()));
                        return;
                    }
                    self.pending = Some((copy, h));
                }
                Outcome::Failed { log, .. } => {
                    self.error = Some(log);
                }
            }
        }

        /// **A session for window program `index` from the build running**
        /// — M89/T89.7 (`docs/plan-windows.md`, hot reload). `None` before
        /// the first swap: the window's own static table serves until then,
        /// through `OpenProgram::hot(None)`. `Some(Err)` with the library's
        /// report when its boot panicked or it has no such entry.
        ///
        /// A window opened after a swap is therefore the **new** build's
        /// program — never one held from the build the window started with
        /// (`docs/events-decision.md` rule 3).
        pub fn open(&self, index: u16) -> Option<Result<Box<dyn Session>, String>> {
            let build = self.build.as_ref()?;
            Some(build.boot(index, None).map(|h| Box::new(h) as Box<dyn Session>))
        }

        /// **At a frame boundary, one session** — the program's own (index
        /// 0): [`Reloader::swap_all`] over `current` alone, for a loader with
        /// one session (`fixtures/reload-host`). `adopt` sets the new one up
        /// (waker, scale, size). Whether it changed.
        pub fn swap(&mut self, current: &mut Box<dyn Session>, adopt: impl FnOnce(&mut dyn Session)) -> bool {
            let mut adopt = Some(adopt);
            self.swap_all(&mut [(0, current)], |_, s| {
                if let Some(a) = adopt.take() {
                    a(s)
                }
            })
        }

        /// **At a frame boundary, every session at once** — M89/T89.7. If a
        /// library is waiting, open it and boot **each** of `sessions` —
        /// `(program index, session)`, the windowless session and every
        /// window's — from it with that session's saved state, **before any
        /// is replaced**. One refusal, a stamp or a boot that panicked,
        /// leaves them all on the build they run (rule 2); otherwise
        /// `adopt(i, new)` sets each up (its own waker, scale and size) and
        /// every one is replaced. Whether they changed.
        ///
        /// The shape change and re-issue reported are the first session's
        /// (the primary, or a daemon's windowless one); the restore logs of
        /// all are joined into [`warning`](Reloader::warning). Both
        /// libraries stay loaded either way (§2.5).
        pub fn swap_all(
            &mut self,
            sessions: &mut [(u16, &mut Box<dyn Session>)],
            mut adopt: impl FnMut(usize, &mut dyn Session),
        ) -> bool {
            let Some((path, h)) = self.pending.take() else { return false };
            let build = match Build::open(&path) {
                Ok(b) => b,
                Err(e) => {
                    self.error = Some(format!("{}: {e}", path.display()));
                    return false;
                }
            };
            let shape = sessions.first().map(|(_, s)| s.shape());
            let mut next: Vec<Box<dyn Session>> = Vec::with_capacity(sessions.len());
            for (index, current) in sessions.iter() {
                let saved = current.save();
                match build.boot(*index, Some(&saved)) {
                    Ok(hot) => next.push(Box::new(hot)),
                    Err(report) => {
                        // Every session booted so far is dropped here, back
                        // into the new library; none was put in place.
                        self.error = Some(if *index == 0 {
                            format!("the new build panicked while booting: {report}")
                        } else {
                            format!("the new build panicked while booting window program {index}: {report}")
                        });
                        return false;
                    }
                }
            }
            let mut logs = Vec::new();
            for (i, (mut s, (_, current))) in next.into_iter().zip(sessions.iter_mut()).enumerate() {
                adopt(i, &mut *s);
                if i == 0 {
                    self.shape_changed = Some(s.shape()) != shape;
                    self.reissued = s.reissued();
                }
                let log = s.take_restore_log();
                if !log.is_empty() {
                    logs.push(log);
                }
                **current = s;
            }
            self.hash = Some(h);
            self.build = Some(build);
            self.reloads += 1;
            self.error = None;
            self.warning = (!logs.is_empty()).then(|| logs.join("\n"));
            true
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
                let p = crate::raw::give(s);
                // SAFETY: `p` is the box `Build::boot` took from this
                // library, handed back once.
                unsafe { (self.drop)(p) };
            }
        }
    }

    impl FrameSource for Hot {
        fn list(&self) -> &fulgurite::DisplayList {
            self.s().list()
        }
        fn take_damage_for_age(&mut self, age: u8) -> fulgurite::retain::Damage {
            self.m().take_damage_for_age(age)
        }
        fn timings(&self) -> (u64, u64) {
            self.s().timings()
        }
        /// The library's session's store, re-asked on every present and
        /// never held by the loader (M84/T84.6): a swapped-in build's
        /// assets reach the renderer through this forward.
        fn images(&self) -> Option<&dyn fulgurite::ImageSource> {
            self.s().images()
        }
    }

    impl Session for Hot {
        fn post(&mut self, ev: fulgurite::Event) {
            self.m().post(ev)
        }
        fn copy(&self) -> Option<String> {
            self.s().copy()
        }
        fn has_focus(&self) -> bool {
            self.s().has_focus()
        }
        fn paste(&mut self, text: &str) -> Option<usize> {
            self.m().paste(text)
        }
        fn compose(&mut self, c: fulgurite::event::Composition<'_>) -> bool {
            self.m().compose(c)
        }
        fn caret_rect(&mut self) -> Option<[f32; 4]> {
            self.m().caret_rect()
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
        fn cursor(&self) -> fulgurite::style::Cursor {
            self.s().cursor()
        }
        fn next_deadline_ms(&mut self) -> Option<f32> {
            self.m().next_deadline_ms()
        }
        fn more_deferred(&self) -> bool {
            self.s().more_deferred()
        }
        fn set_waker(&mut self, waker: std::sync::Arc<dyn fulgurite::inbox::Waker>) {
            self.m().set_waker(waker)
        }
        fn page(&self) -> fulgurite::Color {
            self.s().page()
        }
        fn agent(&mut self, req: &str, screen: &mut dyn Screen) -> String {
            self.m().agent(req, screen)
        }
        fn agent_frame_done(&mut self, timing: fulgurite_shell::session::FrameTimes) {
            self.m().agent_frame_done(timing)
        }
        fn save(&self) -> Vec<u8> {
            self.s().save()
        }
        fn take_restore_log(&mut self) -> String {
            self.m().take_restore_log()
        }
        fn reissued(&self) -> fulgurite_shell::Reissued {
            self.s().reissued()
        }
        fn repaint_all(&mut self) {
            self.m().repaint_all()
        }
        fn damage(&self) -> fulgurite::retain::Damage {
            self.s().damage()
        }
        fn a11y_tree(&self) -> Option<fulgurite_shell::session::A11yTree> {
            self.s().a11y_tree()
        }
        fn a11y_action(&mut self, target: u64, action: fulgurite::Action) -> bool {
            self.m().a11y_action(target, action)
        }
        fn shape(&self) -> u64 {
            self.s().shape()
        }
        fn wants_quit(&self) -> bool {
            self.s().wants_quit()
        }
        fn shaping(&self) -> fulgurite_shell::text::Shaping {
            self.s().shaping()
        }
        fn shaders(&self) -> fulgurite_shell::shader::ShaderTable<'_> {
            self.s().shaders()
        }
        // M88/T88.3: the request seam. A request is drained from, and its
        // answer applied to, the session loaded now.
        fn take_requests(&mut self) -> Vec<fulgurite_shell::request::Outgoing> {
            self.m().take_requests()
        }
        fn answer(&mut self, tag: u64, a: fulgurite::request::Answer<'_>) {
            self.m().answer(tag, a)
        }
        // M90/T90.6: the tray's seam. The model is owned data and the
        // mask is asked of the session loaded now, so a swap's new build
        // answers both.
        fn tray_model(&self) -> Option<fulgurite_shell::tray::TrayModel> {
            self.s().tray_model()
        }
        fn menu_state(&mut self) -> Option<fulgurite::menu::MenuMask> {
            self.m().menu_state()
        }
    }
}

/// Export a window library's entry points — M37/T37.1.
///
/// ```ignore
/// pub fn program() -> Program<Counter> { application(Counter::build) }
/// fulgurite_reload::export_program!(program);
/// ```
///
/// The stamp (with the shell's fields), `fulgurite_boot`, `fulgurite_drop` and
/// `hot_take_panic`. Every one catches a panic inside the library
/// (M36/T36.4); a session's own methods catch theirs in the session.
///
/// **`fulgurite_boot` takes a program index** (M89/T89.7,
/// `docs/plan-windows.md`): 0 boots `program()`, `n` boots entry `n` of
/// its window table through `OpenProgram::hot`. The table is resolved in
/// the build that is loaded, so a loader holds only the number and never
/// a pointer into a library across a swap (rule 3). An unknown `n` is
/// null, with "no window program n" in `hot_take_panic`.
#[cfg(feature = "window")]
#[macro_export]
macro_rules! export_program {
    ($program:path) => {
        $crate::export_abi_stamp!(&::fulgurite_shell::session::abi_fields());

        /// Boot this build's program `index` with the saved state (null
        /// for none) — M89/T89.7: 0 is `program()`, `n` its window table's
        /// entry `n` (`Program::window_program(n)` booted through
        /// `OpenProgram::hot`), resolved in **this** build, so a window a
        /// loader opens after a swap runs the new build's program. An index
        /// the table has no entry for answers null, with "no window program
        /// n" in `hot_take_panic`.
        ///
        /// # Safety
        /// `(saved, len)` must be a valid readable range, or null.
        #[no_mangle]
        pub unsafe extern "C" fn fulgurite_boot(index: u16, saved: *const u8, len: usize) -> *mut ::core::ffi::c_void {
            let saved = (!saved.is_null()).then(|| unsafe { $crate::raw::bytes(saved, len) });
            $crate::panic::guard(::core::ptr::null_mut(), || {
                let program = $program();
                if index == 0 {
                    return $crate::raw::give($crate::hot::boot(program, saved));
                }
                match program.window_program(index) {
                    ::core::option::Option::Some(entry) => $crate::raw::give(entry.hot(saved)),
                    ::core::option::Option::None => {
                        $crate::panic::refuse(::std::format!("no window program {index}"));
                        ::core::ptr::null_mut()
                    }
                }
            })
        }

        /// Drop a session this library booted.
        ///
        /// # Safety
        /// `p` must come from this build's `fulgurite_boot`, handed back once,
        /// or be null, which does nothing (M49/T49.2).
        #[no_mangle]
        pub unsafe extern "C" fn fulgurite_drop(p: *mut ::core::ffi::c_void) {
            let b = unsafe {
                $crate::raw::take::<::std::boxed::Box<dyn ::fulgurite_shell::session::Session>>(p)
            };
            if let ::core::option::Option::Some(b) = b {
                $crate::panic::guard((), move || drop(b));
            }
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
