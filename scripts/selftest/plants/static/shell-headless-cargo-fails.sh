# T83.0 fix (scripts/check_shell_headless.sh): the count reads cargo's
# stdout only, so a lock-wait line on stderr is not a crate -- and a
# `cargo tree` that FAILS must still be refused by name, not counted.
# The edit is a dependency on a crate that exists nowhere, so the offline
# resolve fails for real (no fake cargo). The serde_json edge that
# check_locks.sh's plant uses does NOT fail `cargo tree --locked` here: it
# resolved, the count rose, and with no window or GPU crate in it the
# check stayed green.
CHECK='./scripts/check_shell_headless.sh'
EXPECT='cargo tree failed for the default build'
plant() {
    replace crates/fulgurite-shell/Cargo.toml \
        '[dependencies]' $'[dependencies]\nplanted-edge = { package = "planted-no-such-crate", version = "1" }'
}
