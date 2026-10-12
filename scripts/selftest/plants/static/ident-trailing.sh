# R17-F15: candidates came only from lines that START with `//`, so a
# trailing comment was never read.
CHECK='./scripts/check_doc_identifiers.sh'
EXPECT='PlantedTrailingGhost'
plant() {
    append crates/fulgurite/src/lib.rs \
        'const _PLANTED_TRAILING: u8 = 0; // see `PlantedTrailingGhost`'
}
