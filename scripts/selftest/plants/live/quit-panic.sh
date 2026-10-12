# R18-F9, the review's I6 (M42/T42.0): counter-window panicked on
# `app.quit`, after answering it. live_gate.sh ended in `wait "$pid" ||
# true`, so the panic was discarded and the gate stayed green. Once
# fixed, the app's exit status is the verdict.
#
# Planted in the window crate since M60/T60.3: the tally's `app.quit` is
# `Win::answer_agent`'s, the only one there is, now that the example runs
# no loop of its own (`docs/plan-tally-slim.md`).
#
# Anchored to the `"app.quit"` arm since M89: T89.3 added a second
# `self.quit = true;` in `close_window`, earlier in the file, and the
# unanchored first match planted there -- closing the primary window,
# which live_gate.sh never does -- so the plant stayed green (CI at
# 6cd1e0d).
CHECK='./scripts/live_gate.sh'
EXPECT='live-gate.*app exited 101'
plant() {
    sub crates/fulgurite-winit/src/lib.rs \
        '^(\s*"app\.quit" => \{\n(?:.*\n)*?)(\s*)self\.quit = true;\n' \
        '\1\2panic!("quit-panic plant");\n'
}
