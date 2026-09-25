# R17-F10: rust-toolchain.toml declared no `targets`, so a clean runner fails
# at `thumb`; the local runner shares the rustup home and never notices.
CHECK='./scripts/check_toolchain_targets.sh'
EXPECT='thumbv7em-none-eabihf'
plant() {
    drop_lines rust-toolchain.toml 'targets'
}
