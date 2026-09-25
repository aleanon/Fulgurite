# R6-F6's other half: an advisory that nobody decided to accept must fail.
# Removing one accepted entry from deny.toml must name it (T43.12).
CHECK='./scripts/check_deny.sh'
EXPECT='RUSTSEC-2026-0206'
plant() {
    drop_lines deny.toml 'RUSTSEC-2026-0206'
}
