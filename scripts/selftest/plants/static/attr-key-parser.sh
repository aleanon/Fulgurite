# M55/T55.3 (R16-F10): a key the parser accepts and neither table
# documents.
CHECK='./scripts/check_attr_table.sh'
EXPECT='missing: .*planted_key \(rustdoc\)'
plant() {
    replace crates/lumen-static-macros/src/lib.rs \
        '} else if meta.path.is_ident("fuse") {' \
        '} else if meta.path.is_ident("fuse") || meta.path.is_ident("planted_key") {'
}
