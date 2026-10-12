# R12-F1: core's release-shape tests compiled with `observe` on, through a
# dev-dependency. The tripwire (T43.7) fires when any edge turns it on.
CHECK='just release-shape'
EXPECT='release-shape tripwire'
plant() {
    replace crates/fulgurite/Cargo.toml \
        'fulgurite-widgets = { path = "../fulgurite-widgets" }' \
        'fulgurite-widgets = { path = "../fulgurite-widgets", features = ["observe"] }'
}
