# T43.1's presence check: every crate but lumen-static-reload carries
# `#![forbid(unsafe_code)]`, and one that loses it is named.
CHECK='./scripts/check_no_unsafe.sh'
EXPECT='lumen-static-widgets.*forbid|forbid.*lumen-static-widgets'
plant() {
    drop_lines crates/lumen-static-widgets/src/lib.rs '#![forbid(unsafe_code)]'
}
