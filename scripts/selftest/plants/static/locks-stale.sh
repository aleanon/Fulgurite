# R17-F12/R6-F6: benches-competitive/Cargo.lock had not matched its
# manifests since M30, and `--locked` appeared nowhere. A dependency edge
# added without updating the lock must be named. (At 1717808 the
# unmodified tree is already red: the baseline shows it.)
CHECK='./scripts/check_locks.sh'
EXPECT='benches-competitive'
plant() {
    replace benches-competitive/arms/observe-probe/Cargo.toml \
        '[dependencies]' $'[dependencies]\nplanted-edge = { package = "serde_json", version = "1" }'
}
