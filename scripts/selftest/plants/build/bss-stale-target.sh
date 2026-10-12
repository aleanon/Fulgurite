# R11-F6: the firmware scripts read a hard-coded `target/...`, so with
# CARGO_TARGET_DIR set they judged a stale binary. A framework static is
# planted in `pass::paint` and the firmware built elsewhere; the clean
# binary the baseline left in ./target must not be what is read.
CHECK='./scripts/check_bss.sh'
EXPECT='PLANTED_BSS'
plant() {
    replace crates/fulgurite/src/pass.rs \
        'pub fn paint<W: Node + ?Sized>(tree: &W, st: &[NodeState], out: &mut dyn Painter) {' \
        'pub static PLANTED_BSS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
pub fn paint<W: Node + ?Sized>(tree: &W, st: &[NodeState], out: &mut dyn Painter) {
    PLANTED_BSS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);'
}
applied() {
    # The stale binary must exist, or the plant tests nothing.
    [ -f target/thumbv7em-none-eabihf/release/firmware ]
}
run_check() {
    CARGO_TARGET_DIR="$PWD/target/selftest-alt" bash -c "$CHECK"
}
