//! Links `fwq` -- and only `fwq` -- with its own memory map (M52/T52.6).
//!
//! The other firmwares are linked with no script at all, because they are
//! measured for their sections and never run; `fwq` runs under qemu, so
//! it needs a vector table at 0x0 and a stack where RAM is
//! (`fwq.ld`, `docs/plan-embedded-footprint.md` §2.6).
fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    println!("cargo:rustc-link-arg-bin=fwq=-T{dir}/fwq.ld");
    println!("cargo:rerun-if-changed=fwq.ld");
    println!("cargo:rerun-if-changed=build.rs");
}
