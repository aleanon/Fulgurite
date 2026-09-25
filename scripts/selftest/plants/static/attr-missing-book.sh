# R17-F4: with the book's table missing the check exited 0 ("skipped").
CHECK='./scripts/check_attr_table.sh'
EXPECT='no book'
plant() {
    remove book/src/attributes.md
}
