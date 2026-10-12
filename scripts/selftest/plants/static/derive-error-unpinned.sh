# M55/T55.2 (R16-F4): an error sentence the derive can emit and no
# `.stderr` pins. The review disabled two such checks at once and the
# `ui` suite stayed green; the coverage check names the sentence instead.
CHECK='./scripts/check_derive_errors.sh'
EXPECT='no \.stderr pins: .App needs named fields, planted'
plant() {
    replace crates/fulgurite-macros/src/lib.rs \
        '"App needs named fields"' '"App needs named fields, planted"'
}
