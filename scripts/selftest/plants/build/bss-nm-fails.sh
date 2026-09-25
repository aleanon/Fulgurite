# R17-F9: `nm` failing produced an empty symbol list, which passed.
CHECK='./scripts/check_bss.sh'
EXPECT='nm failed'
plant() {
    write .plant-bin/nm $'#!/bin/sh\necho "planted nm: refusing" >&2\nexit 1' && chmod +x .plant-bin/nm
}
applied() {
    [ "$(PATH="$PWD/.plant-bin:$PATH" command -v nm)" = "$PWD/.plant-bin/nm" ]
}
run_check() {
    PATH="$PWD/.plant-bin:$PATH" bash -c "$CHECK"
}
