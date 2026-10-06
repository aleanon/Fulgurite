//! M83/T83.3 (docs/plan-gpu-extension.md, "Feature placement"): tell the
//! crates that depend on this one whether `Cmd::Shader` exists.
//!
//! A crate outside the core cannot write `cfg(feature = "shader")` for an
//! arm over `Cmd`: that tests *its own* feature, and under Cargo's feature
//! unification the core's can be on while its own is off, so the arm would
//! be dropped while the variant exists (E0004). With `links =
//! "lumen_static"`, the `cargo:shader=1` printed here reaches every direct
//! dependent's build script as `DEP_LUMEN_STATIC_SHADER`, which each turns
//! into `cfg(lumen_shader)`. Runs on the host, for the firmware's
//! `thumbv7em-none-eabihf` build too, and has no dependencies.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_SHADER");
    if std::env::var_os("CARGO_FEATURE_SHADER").is_some() {
        println!("cargo:shader=1");
    }
}
