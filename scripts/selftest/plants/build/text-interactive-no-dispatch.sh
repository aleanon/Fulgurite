# R11-F4 (M52/T52.1): a binary built to price dispatch must link it before
# its .text is read. Deleting the interactive firmware's dispatch calls
# leaves a smaller, greener number -- unless the precondition names it.
CHECK='./scripts/check_text.sh'
EXPECT='firmware_interactive: 0 pass::dispatch symbols linked'
plant() {
    replace fixtures/embedded/src/bin/firmware_interactive.rs \
        '    let pressed = pass::dispatch(&mut dev, &sty, &mut st, &mut input, &down)
        | pass::dispatch(&mut dev, &sty, &mut st, &mut input, &up);' \
        '    let pressed = { let _ = (&down, &up); false };' &&
    replace fixtures/embedded/src/bin/firmware_interactive.rs \
        "    if pass::dispatch(&mut dev, &sty, &mut st, &mut input, &Event::Text { ch: '7' }) {" \
        '    if false {'
}
