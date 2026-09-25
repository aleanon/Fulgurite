# R17-F14: the justfile stated dependency counts ("wgpu is 37 crates") that
# no longer held.
CHECK='./scripts/check_ci_claims.sh'
EXPECT='justfile:[0-9]+'
plant() {
    append justfile '# wgpu is 37 crates and minutes of compile'
}
