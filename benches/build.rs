//! M83/T83.3 (docs/plan-gpu-extension.md, "Feature placement"): turn the
//! core's `shader` feature into `cfg(fulgurite_shader)` for this crate.
//!
//! This crate matches `Cmd` exhaustively, and `Cmd::Shader`/`ShaderEnd`
//! exist only when `fulgurite/shader` is on **anywhere in the graph**.
//! Its own `cfg(feature = "shader")` would test this crate's feature
//! instead, and under Cargo's unification the two can disagree, which is
//! E0004. The core's build script prints `cargo:shader=1` (its `links =
//! "fulgurite"`), which reaches a *direct* dependent as
//! `DEP_FULGURITE_SHADER`; so every arm here is `#[cfg(fulgurite_shader)]`.
//! The check-cfg line is always printed, so the unexpected-cfgs lint knows the
//! name when it is unset.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=DEP_FULGURITE_SHADER");
    println!("cargo:rustc-check-cfg=cfg(fulgurite_shader)");
    if std::env::var_os("DEP_FULGURITE_SHADER").is_some() {
        println!("cargo:rustc-cfg=fulgurite_shader");
    }
}
