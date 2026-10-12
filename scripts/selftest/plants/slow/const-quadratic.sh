# R11-F5: `just const-budget` touched main.rs and timed an INCREMENTAL
# rebuild, so a quadratic in the table builder (the review's b634311, with
# the const-eval lint allowed) went red once and was green on every rerun.
# Both reruns must be red (T43.5).
CHECK='just const-budget >/dev/null 2>&1; r2=0; just const-budget || r2=1; r3=0; just const-budget || r3=1; echo "run2 red=$r2 run3 red=$r3"; [ "$r2$r3" = 00 ]'
EXPECT='run2 red=1 run3 red=1'
plant() {
    replace crates/fulgurite/src/widget.rs \
        '    while j < src.len() {
        dst[write_at + j] = src[j];' \
        '    while j < src.len() {
        // PLANTED: an O(n^2) "dedupe" scan of everything spliced so far.
        let mut k = 0;
        let mut dup = 0usize;
        while k < write_at + j {
            if dst[k].kind.len() == src[j].kind.len() {
                dup += 1;
            }
            k += 1;
        }
        let _ = dup;
        dst[write_at + j] = src[j];' &&
    sub fixtures/const-budget/src/main.rs '^(use )' '#![allow(long_running_const_eval)]\n\n\1'
}
