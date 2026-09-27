# M55/T55.3 (R16-F10): the tables listed every attribute NAME and still
# omitted `#[widget(wrap)]`, `grid`, `columns` and `tick`. The check reads
# the parser's keys, so a dropped row is named.
CHECK='./scripts/check_attr_table.sh'
EXPECT='missing: .*wrap \(book\)'
plant() {
    drop_lines book/src/attributes.md '| `#[widget(wrap)]` |'
}
