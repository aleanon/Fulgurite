# R12-F7: a Cargo.toml comment lost its backticked names -- "#  implies ,
# the tiering  uses" -- and no check read TOML comments.
CHECK='./scripts/check_doc_identifiers.sh'
EXPECT='crates/lumen-static-shell/Cargo\.toml:[0-9]+'
plant() {
    append crates/lumen-static-shell/Cargo.toml '#  implies , the planted tiering  uses'
}
