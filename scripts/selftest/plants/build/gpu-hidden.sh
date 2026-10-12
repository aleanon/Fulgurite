# R17-F6: with no adapter the parity tests returned early, and libtest
# swallowed the "skipping" line on a pass -- so `just gpu` passed on a
# machine that compared nothing. Under the recipe's requirement it must
# fail by name (T43.10). The adapter is hidden by pointing the Vulkan and
# EGL loaders at nothing, with no display.
CHECK='just gpu'
EXPECT='FULGURITE_REQUIRE_GPU=1 but no adapter'
hide() {
    env -u DISPLAY -u WAYLAND_DISPLAY -u FULGURITE_ALLOW_SKIP \
        VK_ICD_FILENAMES=/nonexistent.json VK_DRIVER_FILES=/nonexistent.json \
        __EGL_VENDOR_LIBRARY_FILENAMES=/nonexistent.json "$@"
}
plant() { :; }
applied() {
    # The mode must really hide the adapter: the test, run WITHOUT the
    # requirement, must report the skip.
    # (Captured, not piped into `grep -q`: under pipefail an early exit
    # from grep kills cargo with SIGPIPE and reads as "did not apply".)
    local out
    out=$(hide env -u FULGURITE_REQUIRE_GPU cargo test -q -p fulgurite-wgpu --test parity -- --nocapture 2>&1)
    echo "$out" | tail -3
    grep -q 'no adapter' <<<"$out"
}
run_check() {
    hide bash -c "$CHECK"
}
