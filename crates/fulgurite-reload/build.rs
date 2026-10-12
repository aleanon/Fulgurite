//! What built this image, for the ABI stamp (M37/T37.0, M49/T49.1):
//! the compiler, the target, and **the source of every crate whose types
//! cross the boundary**.
//!
//! Two images promise each other nothing about layout unless they were
//! built by one compiler from one source. The stamp's sizes cannot see a
//! reordered enum — `Cmd::Rect` moved after `Cmd::Text` keeps every size
//! and changes every tag (R4-F5, `docs/plan-reload-hardening.md` §2.1) —
//! so the source is hashed instead: `src/` and the manifest of
//! `fulgurite`, `fulgurite-shell` and this crate, the workspace's
//! `Cargo.lock`, and the encoded rustflags. `fulgurite-widgets` is
//! deliberately **not** hashed: no widget type crosses, so editing a
//! widget while a hot window runs keeps working (`just abi-stamp` checks
//! that). The hash is computed only when one of these changes, which is
//! when the crates recompile anyway.
use std::path::{Path, PathBuf};

/// FNV-1a, 64-bit: not adversarial, only "did anything change".
struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    fn eat(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 ^ *b as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

/// Every file under `dir/rel`, as paths relative to `dir` — relative, so
/// a copy of the workspace elsewhere hashes the same.
fn files(dir: &Path, rel: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir.join(rel)) else { return };
    for e in rd.flatten() {
        let r = rel.join(e.file_name());
        match e.file_type() {
            Ok(t) if t.is_dir() => files(dir, &r, out),
            Ok(_) => out.push(r),
            Err(_) => {}
        }
    }
}

/// A crate's `src/` and `Cargo.toml`, hashed by relative path and content.
fn crate_hash(dir: &Path) -> String {
    let mut list = Vec::new();
    files(dir, Path::new("src"), &mut list);
    list.push(PathBuf::from("Cargo.toml"));
    list.sort();
    let mut h = Fnv::new();
    for f in &list {
        h.eat(f.to_string_lossy().as_bytes());
        h.eat(&[0]);
        if let Ok(bytes) = std::fs::read(dir.join(f)) {
            h.eat(&(bytes.len() as u64).to_le_bytes());
            h.eat(&bytes);
        }
    }
    println!("cargo:rerun-if-changed={}", dir.join("src").display());
    println!("cargo:rerun-if-changed={}", dir.join("Cargo.toml").display());
    format!("{:016x}", h.0)
}

/// One `[[package]]` of a lockfile.
struct Package {
    name: String,
    version: String,
    /// Its checksum, when it has one.
    text: String,
    /// From a registry or git, rather than a path in some workspace.
    external: bool,
}

/// The normal dependencies a manifest names (`[dependencies]` and its
/// `[target.….dependencies]` forms, never dev- or build-): external ones
/// by package name, and path ones by directory, which are followed.
fn manifest_deps(dir: &Path, external: &mut Vec<String>, seen: &mut Vec<PathBuf>) {
    let Ok(dir) = dir.canonicalize() else { return };
    if seen.contains(&dir) {
        return;
    }
    seen.push(dir.clone());
    let Ok(text) = std::fs::read_to_string(dir.join("Cargo.toml")) else { return };
    let mut normal = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            normal = t == "[dependencies]" || (t.starts_with("[target.") && t.ends_with(".dependencies]"));
            continue;
        }
        if !normal || t.starts_with('#') {
            continue;
        }
        let Some((key, rest)) = t.split_once('=') else { continue };
        let name = match rest.find("package =").map(|i| &rest[i..]).and_then(quoted) {
            Some(renamed) => renamed,
            None => key.trim().to_string(),
        };
        match rest.find("path =").map(|i| &rest[i..]).and_then(quoted) {
            Some(path) => manifest_deps(&dir.join(path), external, seen),
            None => external.push(name),
        }
    }
}

fn quoted(line: &str) -> Option<String> {
    let a = line.find('"')?;
    let b = line[a + 1..].find('"')?;
    Some(line[a + 1..a + 1 + b].to_string())
}

/// Hash the locked versions of the external packages the crates'
/// manifests name, in sorted order, prefixed with how many.
///
/// **Direct dependencies only, and by version, not by what the lockfile
/// says they depend on.** Both wider forms were built first and refused
/// an unchanged copy of the workspace (`just abi-stamp`): a lockfile
/// lists a workspace member's dev-dependencies beside its normal ones,
/// and it resolves a registry crate's optional dependencies only where
/// some member turns them on — so the same source, in a workspace with
/// other members, locks a different transitive graph. A transitive bump
/// is therefore not seen; no crossing type holds a third-party type
/// today (`docs/step4-hot-reload.md`, the stamp).
fn direct_lock_hash(text: &str, direct: &[String]) -> String {
    let mut lines: Vec<String> = Vec::new();
    for block in text.split("[[package]]").skip(1) {
        let mut p = Package { name: String::new(), version: String::new(), text: String::new(), external: false };
        for t in block.lines().map(str::trim) {
            if let Some(v) = t.strip_prefix("name = ") {
                p.name = v.trim_matches('"').to_string();
            } else if let Some(v) = t.strip_prefix("version = ") {
                p.version = v.trim_matches('"').to_string();
            } else if let Some(v) = t.strip_prefix("checksum = ") {
                p.text = v.trim_matches('"').to_string();
            } else if t.starts_with("source = ") {
                p.external = true;
            }
        }
        if p.external && direct.contains(&p.name) {
            lines.push(format!("{} {} {}", p.name, p.version, p.text));
        }
    }
    lines.sort_unstable();
    lines.dedup();
    let mut h = Fnv::new();
    for l in &lines {
        h.eat(l.as_bytes());
        h.eat(b"\n");
    }
    format!("{}:{:016x}", lines.len(), h.0)
}

fn main() {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let version = std::process::Command::new(rustc)
        .arg("-V")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=FULGURITE_RUSTC={version}");
    println!("cargo:rustc-env=FULGURITE_TARGET={}", std::env::var("TARGET").unwrap_or_default());
    println!("cargo:rerun-if-env-changed=RUSTC");

    let here = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let crates = here.parent().expect("the crate lives in crates/").to_path_buf();
    for (env, name) in [
        ("FULGURITE_SRC_CORE", "fulgurite"),
        ("FULGURITE_SRC_SHELL", "fulgurite-shell"),
        ("FULGURITE_SRC_RELOAD", "fulgurite-reload"),
    ] {
        println!("cargo:rustc-env={env}={}", crate_hash(&crates.join(name)));
    }
    // The lockfile pins the dependencies a crossing type could hold; only
    // the direct ones are hashed (see `direct_lock_hash`). Absent when the
    // crate is built outside a workspace.
    let lock = crates.parent().map(|w| w.join("Cargo.lock"));
    let lock_hash = match lock.as_deref().map(std::fs::read_to_string) {
        Some(Ok(text)) => {
            if let Some(l) = &lock {
                println!("cargo:rerun-if-changed={}", l.display());
            }
            let (mut ext, mut seen) = (Vec::new(), Vec::new());
            for name in ["fulgurite", "fulgurite-shell", "fulgurite-reload"] {
                manifest_deps(&crates.join(name), &mut ext, &mut seen);
            }
            for dir in &seen {
                println!("cargo:rerun-if-changed={}", dir.join("Cargo.toml").display());
            }
            direct_lock_hash(&text, &ext)
        }
        _ => "none".into(),
    };
    println!("cargo:rustc-env=FULGURITE_SRC_LOCK={lock_hash}");
    let mut h = Fnv::new();
    h.eat(std::env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default().as_bytes());
    println!("cargo:rustc-env=FULGURITE_RUSTFLAGS={:016x}", h.0);
    println!("cargo:rerun-if-env-changed=CARGO_ENCODED_RUSTFLAGS");
}
