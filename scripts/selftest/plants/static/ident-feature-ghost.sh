# M54/T54.9 (R15-F9): a comment called `image` a feature of the shell,
# and no [features] table had it -- the identifier rule passed it, because
# `image` is a live token.
CHECK='./scripts/check_doc_identifiers.sh'
EXPECT='`plantedghost` is called a feature, and no \[features\] table has `plantedghost`'
plant() {
    append crates/lumen-static/src/lib.rs \
        '// Decoding lives behind the default-off `plantedghost` feature.'
}
