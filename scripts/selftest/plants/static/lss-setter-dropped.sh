# M61/T61.2 (docs/plan-lss.md): a `Patch` setter with no `.lss`
# declaration. The lss crate cannot depend on the core, so the compiler
# cannot see the table fall behind; scripts/check_lss_setters.sh reads both
# files. Planted as the task's injection: drop `hover-dim` from the table.
CHECK='./scripts/check_lss_setters.sh'
EXPECT="Patch setters with no .lss declaration: \['hover_dim'\]"
plant() {
    drop_lines crates/lumen-static-lss/src/lower.rs 'name: "hover-dim"'
}
