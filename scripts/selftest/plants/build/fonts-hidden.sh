# R17-F7: the shaping tests returned early when no system face existed --
# a pass that shaped nothing. After T43.11 they shape a committed face, and
# only the tests ABOUT discovery still need the system; those fail under the
# recipe's requirement. Fonts are hidden with a user namespace and an empty
# tmpfs over /usr/share/fonts.
CHECK='just fonts'
EXPECT='LUMEN_REQUIRE_FONTS=1 but no system face'
hidden() {
    unshare -rm bash -c 'mount -t tmpfs none /usr/share/fonts && env -u LUMEN_ALLOW_SKIP "$@"' _ "$@"
}
plant() { :; }
applied() {
    [ "$(hidden bash -c 'ls -A /usr/share/fonts | wc -l')" = 0 ]
}
run_check() {
    hidden bash -c "$CHECK"
}
