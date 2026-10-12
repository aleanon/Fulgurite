//! Links `fwq` -- and only `fwq` -- with its own memory map (M52/T52.6).
//!
//! The other firmwares are linked with no script at all, because they are
//! measured for their sections and never run; `fwq` runs under qemu, so
//! it needs a vector table at 0x0 and a stack where RAM is
//! (`fwq.ld`, `docs/plan-embedded-footprint.md` §2.6).
//!
//! And, since M61/T61.0, chooses where `SHEET` comes from: with
//! `FULGURITE_SHEET=lss` in the environment, from `src/sheet.lss` through
//! `include_lss!` (`cfg(fulgurite_lss_sheet)`). **A cfg from here and not a
//! cargo feature**, because a feature changes the crate's `-C metadata`,
//! every mangled name with it, and so the order the linker lays functions
//! out in: two builds that differed only by an unused feature had 780
//! `.text` bytes apart. Kill rule K1 compares bytes
//! (`scripts/check_lss_parity.sh`), so the two builds must differ in the
//! sheet and in nothing else.
fn main() {
    println!("cargo:rustc-check-cfg=cfg(fulgurite_lss_sheet)");
    println!("cargo:rerun-if-env-changed=FULGURITE_SHEET");
    if std::env::var("FULGURITE_SHEET").as_deref() == Ok("lss") {
        println!("cargo:rustc-cfg=fulgurite_lss_sheet");
    }
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    println!("cargo:rustc-link-arg-bin=fwq=-T{dir}/fwq.ld");
    println!("cargo:rerun-if-changed=fwq.ld");
    println!("cargo:rerun-if-changed=build.rs");
}
