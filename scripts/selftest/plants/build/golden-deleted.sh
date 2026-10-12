# I-NEW-4: failure_screen.rs matched `v => println!` for every verdict but
# `Differs`, so a deleted golden was re-recorded and the test passed.
CHECK='cargo test -q -p fulgurite-shell --test golden --test failure_screen'
EXPECT='a first run is not a pass'
plant() {
    remove crates/fulgurite-shell/tests/golden/panic_screen_480x320.ppm
}
