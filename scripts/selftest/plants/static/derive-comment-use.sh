# R17-F5: a use inside a comment counted as a use.
CHECK='./scripts/check_derive_attrs.sh'
EXPECT='planted_commented'
plant() {
    replace crates/fulgurite-macros/src/lib.rs \
        'attributes(reload))]' 'attributes(reload, planted_commented))]' &&
    append examples/counter/src/main.rs '// #[planted_commented] is only ever written about'
}
