# R17-F16: an indented fence passed the `rust,ignore` ban.
CHECK='./scripts/check_book_includes.sh'
EXPECT='book/src/glossary\.md.*ignore'
plant() {
    append book/src/glossary.md $'\n   ```rust,ignore\n   let planted = 1;\n   ```'
}
