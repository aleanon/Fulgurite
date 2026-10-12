# R17-F5: the attributes(...) lists were parsed one line at a time, so a list
# split across lines disappeared -- with every attribute in it.
CHECK='./scripts/check_derive_attrs.sh'
EXPECT='planted_unused'
plant() {
    replace crates/fulgurite-macros/src/lib.rs \
        '#[proc_macro_derive(Reloadable, attributes(reload))]' \
        $'#[proc_macro_derive(\n    Reloadable,\n    attributes(\n        reload,\n        planted_unused,\n    )\n)]'
}
