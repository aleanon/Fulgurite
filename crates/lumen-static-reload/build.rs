//! The compiler that built this image, for the ABI stamp (M37/T37.0):
//! two images from different compilers promise nothing about layout.
fn main() {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let version = std::process::Command::new(rustc)
        .arg("-V")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=LUMEN_RUSTC={version}");
    println!("cargo:rustc-env=LUMEN_TARGET={}", std::env::var("TARGET").unwrap_or_default());
    println!("cargo:rerun-if-env-changed=RUSTC");
}
