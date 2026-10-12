# T43.1's presence check: every crate but fulgurite-reload carries
# `#![forbid(unsafe_code)]`, and one that loses it is named.
CHECK='./scripts/check_no_unsafe.sh'
EXPECT='fulgurite-widgets.*forbid|forbid.*fulgurite-widgets'
plant() {
    drop_lines crates/fulgurite-widgets/src/lib.rs '#![forbid(unsafe_code)]'
}
