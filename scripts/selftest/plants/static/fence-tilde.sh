# R17-F16: the `rust,ignore` ban matched only a backtick fence at column 0.
CHECK='./scripts/check_book_includes.sh'
EXPECT='book/src/glossary\.md.*ignore'
plant() {
    append book/src/glossary.md $'\n~~~rust,ignore\nlet planted = 1;\n~~~'
}
