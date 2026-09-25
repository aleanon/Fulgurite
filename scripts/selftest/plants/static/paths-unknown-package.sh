# R17-F3: `-p` was never extracted, so a package that does not exist passed.
CHECK='./scripts/check_doc_paths.sh'
EXPECT='planted-no-such-crate'
plant() {
    append docs/results-embedded.md 'Planted: `cargo test -p planted-no-such-crate`.'
}
