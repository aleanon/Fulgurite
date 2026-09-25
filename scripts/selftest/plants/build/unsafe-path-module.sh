# R4-F4: a `#[path]` module outside `src/` is never scanned by the script;
# `forbid` covers it because it expands inside the crate.
CHECK='cargo check -q -p lumen-static'
EXPECT='usage of an .unsafe. block'
plant() {
    write crates/lumen-static/planted/outside.rs \
        'pub fn planted_path() -> u8 { unsafe { *(&2u8 as *const u8) } }' &&
    append crates/lumen-static/src/lib.rs '#[path = "../planted/outside.rs"] pub mod planted_outside;'
}
