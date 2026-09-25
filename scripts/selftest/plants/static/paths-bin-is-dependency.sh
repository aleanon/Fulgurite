# R17-F3: the target set was every "name" string in cargo metadata, so a
# dependency's name passed as a `--bin`.
CHECK='./scripts/check_doc_paths.sh'
EXPECT='serde_json'
plant() {
    append docs/results-embedded.md 'Planted: `cargo run --release -p benches --bin serde_json`.'
}
