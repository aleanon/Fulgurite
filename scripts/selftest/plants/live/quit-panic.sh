# R18-F9, the review's I6 (M42/T42.0): counter-window panicked on
# `app.quit`, after answering it. live_gate.sh ended in `wait "$pid" ||
# true`, so the panic was discarded and the gate stayed green. Once
# fixed, the app's exit status is the verdict.
#
# Planted in the window crate since M60/T60.3: the tally's `app.quit` is
# `Win::answer_agent`'s, the only one there is, now that the example runs
# no loop of its own (`docs/plan-tally-slim.md`).
CHECK='./scripts/live_gate.sh'
EXPECT='live-gate.*app exited 101'
plant() {
    sub crates/lumen-static-winit/src/lib.rs \
        '^(\s*)self\.quit = true;\n' \
        '\1panic!("quit-panic plant");\n'
}
