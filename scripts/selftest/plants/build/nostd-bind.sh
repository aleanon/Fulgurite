# R12-F2: no gate executed core's no_std bind branch. The review's fa64952:
# the no_std path formats and then discards. `cargo test -p lumen-static`
# builds with std and cannot see it; the embedded bind test must (T43.8).
CHECK='LUMEN_REQUIRE_NOSTD=1 cargo test -q -p embedded --test bind_nostd'
EXPECT='stored 0 of 1000, expected the no_std cap'
plant() {
    replace crates/lumen-static/src/bind.rs \
        '    let changed = dst.set_str(buf.as_ref());' \
        '    #[cfg(not(feature = "std"))]
    buf.clear(); // PLANTED: the no_std bind discards what it formatted
    let changed = dst.set_str(buf.as_ref());'
}
