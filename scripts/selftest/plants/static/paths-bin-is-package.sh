# R17-F3: a package name with no binary target passed as a `--bin`.
CHECK='./scripts/check_doc_paths.sh'
EXPECT='fulgurite-macros'
plant() {
    append docs/results-embedded.md 'Planted: `cargo run --bin fulgurite-macros`.'
}
