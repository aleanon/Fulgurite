# R17-F1/R9-F4: the golden gates compile to zero tests in every --workspace
# build. The shell alone runs them; a corrupted golden must fail there.
CHECK='cargo test -q -p fulgurite-shell --test golden --test failure_screen'
EXPECT='differs'
plant() {
    python3 - <<'PY'
p = "crates/fulgurite-shell/tests/golden/panic_screen_480x320.ppm"
b = bytearray(open(p, "rb").read())
# Past the header: flip a run of pixels in the middle of the image.
mid = len(b) // 2
for i in range(mid, mid + 3 * 64):
    b[i] ^= 0xFF
open(p, "wb").write(bytes(b))
PY
    ! git diff --quiet -- crates/fulgurite-shell/tests/golden/panic_screen_480x320.ppm
}
