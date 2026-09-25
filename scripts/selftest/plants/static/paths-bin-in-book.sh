# R17-F3: `--bin` was read only from docs/*.md and the justfile, not the book.
CHECK='./scripts/check_doc_paths.sh'
EXPECT='planted_book_bin'
plant() {
    append book/src/performance.md 'Planted: `cargo run --release -p benches --bin planted_book_bin`.'
}
