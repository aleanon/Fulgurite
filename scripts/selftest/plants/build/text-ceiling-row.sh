# M80/T80.0: scripts/check_text.sh's default ceiling and the budget row of
# docs/results-embedded.md are one number stated twice, so the script
# reads the row and refuses a disagreement. Planting the row back at the
# old 64 KB must fail that check by name, with both numbers printed.
CHECK='./scripts/check_text.sh'
EXPECT='ceiling: the default 81920 B disagrees with docs/results-embedded.md.s budget row \(65536 B\)'
plant() {
    replace docs/results-embedded.md \
        '| Framework `.text` | **≤ 80 KB** |' \
        '| Framework `.text` | **≤ 64 KB** |'
}
