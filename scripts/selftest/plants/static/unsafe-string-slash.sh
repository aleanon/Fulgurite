# R17-F2: a `//` inside a string literal hid the rest of the line from the
# comment stripper, so an `unsafe` after it passed (the review's dea0bb6).
CHECK='./scripts/check_no_unsafe.sh'
EXPECT='crates/lumen-static/src/lib\.rs:[0-9]+'
plant() {
    append crates/lumen-static/src/lib.rs \
        'pub fn planted_string_slash() -> u8 { let _s = "http://example"; unsafe { *(&1u8 as *const u8) } }'
}
