# M61/T61.5 (docs/plan-lss.md, T61.0's item 5): a `.lss` sheet that no
# test reads both ways. include_lss! and lss::load are two lowerings of
# one IR; a sheet only one of them has seen can disagree unnoticed.
CHECK='./scripts/check_lss_coverage.sh'
EXPECT='examples/minimal/orphan.lss: NOT compared'
plant() {
    write examples/minimal/orphan.lss '#title { font-size: 12px; }'
}
