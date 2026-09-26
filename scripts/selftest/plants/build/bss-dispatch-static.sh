# R11-F4 (M52/T52.1): the firmware gates read a binary that never
# dispatched, so a framework static in `pass::dispatch` -- the review's
# `6697ad2` -- left check_bss.sh green. The interactive firmware links
# dispatch now, and must name the static by its full path.
CHECK='./scripts/check_bss.sh'
EXPECT='lumen_static::pass::PLANTED_DISPATCH  \(4 bytes\)'
plant() {
    replace crates/lumen-static/src/pass.rs \
        'pub fn dispatch<S: crate::event::App>(
    state: &mut S,
    style: &[Style],
    st: &mut [NodeState],
    input: &mut crate::event::InputState,
    ev: &crate::event::Event,
) -> bool {' \
        'pub static PLANTED_DISPATCH: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
pub fn dispatch<S: crate::event::App>(
    state: &mut S,
    style: &[Style],
    st: &mut [NodeState],
    input: &mut crate::event::InputState,
    ev: &crate::event::Event,
) -> bool {
    PLANTED_DISPATCH.fetch_add(1, core::sync::atomic::Ordering::Relaxed);'
}
