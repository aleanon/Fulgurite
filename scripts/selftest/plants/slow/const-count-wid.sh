# R11-F5: the fixture contained no `wid_index!`, so it never evaluated
# `count_wid` and a regression there could not move the budget. A quadratic
# `count_wid` must be red on a rerun too (T43.5).
CHECK='just const-budget >/dev/null 2>&1; r2=0; just const-budget || r2=1; r3=0; just const-budget || r3=1; echo "run2 red=$r2 run3 red=$r3"; [ "$r2$r3" = 00 ]'
EXPECT='run2 red=1 run3 red=1'
plant() {
    replace crates/fulgurite/src/style.rs \
        '        if bytes_eq(table[i].wid, wid) {
            n += 1;
        }' \
        '        // PLANTED: rescan the whole table for every entry.
        let mut k = 0;
        while k < table.len() {
            if k == i && bytes_eq(table[k].wid, wid) {
                n += 1;
            }
            k += 1;
        }' &&
    sub fixtures/const-budget/src/main.rs '^(use )' '#![allow(long_running_const_eval)]\n\n\1'
}
