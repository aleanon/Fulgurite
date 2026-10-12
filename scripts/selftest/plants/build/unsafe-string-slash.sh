# R17-F2 at the compiler: once `forbid(unsafe_code)` is on the crate, the
# string-slash bypass cannot build (T43.1).
CHECK='cargo check -q -p fulgurite'
EXPECT='usage of an .unsafe. block'
plant() {
    append crates/fulgurite/src/lib.rs \
        'pub fn planted_string_slash() -> u8 { let _s = "http://example"; unsafe { *(&1u8 as *const u8) } }'
}
