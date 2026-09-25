# R17-F15: a `/* ... */` block comment was never read.
CHECK='./scripts/check_doc_identifiers.sh'
EXPECT='PlantedBlockGhost'
plant() {
    append crates/lumen-static/src/lib.rs \
        '/* the old `PlantedBlockGhost` lived here */'
}
