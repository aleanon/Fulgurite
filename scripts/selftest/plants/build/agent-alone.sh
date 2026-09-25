# R13-F7/R3-F8: the agent's tests passed only because core's dev edge
# turned widgets' `observe` on across the workspace. Built alone, without
# the feature on its own manifest, they fail (T43.6).
CHECK='cargo test -q -p lumen-static-agent'
EXPECT='test result: FAILED'
plant() {
    replace crates/lumen-static-agent/Cargo.toml \
        'lumen-static-widgets = { path = "../lumen-static-widgets", features = ["observe"] }' \
        'lumen-static-widgets = { path = "../lumen-static-widgets" }'
}
