# M55/T55.3 (R16-F10, rule 4): the gap list said "nine commands" while
# `Cmd` had eleven. A count in prose is computed or removed.
CHECK='./scripts/check_attr_table.sh'
EXPECT='gaps\.md says nine commands; `enum Cmd` has'
plant() {
    append book/src/gaps.md 'The drawing vocabulary is nine commands.'
}
