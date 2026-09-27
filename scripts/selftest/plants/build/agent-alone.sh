# R13-F7/R3-F8: the agent's tests passed only because core's dev edge
# turned widgets' `observe` on across the workspace. Built alone, without
# the feature on its own manifest, they fail (T43.6).
CHECK='cargo test -q -p lumen-static-agent'
# Until M48/T48.9 (and M54/T54.1 for a11y) the missing feature showed as
# 19 failed tests at run time (R13-F7). It is now refused at compile time,
# by name, which is the stronger defence -- so that refusal is what the
# plant must meet.
EXPECT='lumen-static has `observe` on and lumen-static-widgets does not'
plant() {
    replace crates/lumen-static-agent/Cargo.toml \
        'lumen-static-widgets = { path = "../lumen-static-widgets", features = ["observe"] }' \
        'lumen-static-widgets = { path = "../lumen-static-widgets" }'
}
