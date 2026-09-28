# R18-F2, the review's I5b/I5c (M42/T42.0): Radio's inventory dropped and
# one Switch row deleted; the run stayed green at 38/92 against 39/94,
# because coverage was checked against the inventory the loop had just
# walked. Once fixed, run.py compares the discovered inventories with
# scripts/drive/expected.json before any window opens -- which is why
# this plant needs no display.
CHECK='python3 scripts/drive/run.py --manifest-only'
EXPECT='manifest expects Radio'
plant() {
    replace scripts/drive/widgets/toggles.py 'ALL = [BUTTON, CHECKBOX, SWITCH, RADIO]' 'ALL = [BUTTON, CHECKBOX, SWITCH]' &&
    sub scripts/drive/widgets/toggles.py \
        '^    Action\(\n(?:        [a-z_]+=[^\n]*\n)*?        name="the track changes colour with the state",[\s\S]*?\n    \),\n' \
        ''
}
