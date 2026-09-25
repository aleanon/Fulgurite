# R4-F4: code a build script writes into OUT_DIR and the crate `include!`s
# is never scanned by the script; `forbid` covers it.
CHECK='cargo check -q -p lumen-static'
EXPECT='usage of an .unsafe. block'
plant() {
    write crates/lumen-static/build.rs 'fn main() {
    let out = std::env::var("OUT_DIR").unwrap();
    let code = ["pub fn planted_gen() -> u8 { ", "un", "safe", " { *(&3u8 as *const u8) } }"].concat();
    std::fs::write(format!("{out}/planted_gen.rs"), code).unwrap();
}' &&
    append crates/lumen-static/src/lib.rs 'include!(concat!(env!("OUT_DIR"), "/planted_gen.rs"));'
}
