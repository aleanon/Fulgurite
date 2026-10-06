# T84.0 fix (scripts/check_default_crates.sh): the count reads cargo's
# stdout only, so a lock-wait line on stderr is not a crate -- and a
# `cargo tree` that FAILS must still be refused by name, not counted.
# The edit is a dependency on a crate that exists nowhere, so the offline
# resolve fails for real in the work tree (no fake cargo). The serde_json
# edge check_locks.sh's plant uses does NOT fail `cargo tree --locked`
# here: it resolved, counted 100 against 96, and went red as a leak --
# red, but not by name.
CHECK='./scripts/check_default_crates.sh'
EXPECT='cargo tree failed in the work tree'
plant() {
    replace crates/lumen-static-winit/Cargo.toml \
        '[dependencies]' $'[dependencies]\nplanted-edge = { package = "planted-no-such-crate", version = "1" }'
}
