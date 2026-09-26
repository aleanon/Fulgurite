# R18-F9, the review's I6 (M42/T42.0): counter-window panics on
# `app.quit`, after answering it. live_gate.sh ended in `wait "$pid" ||
# true`, so the panic was discarded and the gate stayed green. Once
# fixed, the app's exit status is the verdict.
CHECK='./scripts/live_gate.sh'
EXPECT='live-gate.*app exited 101'
plant() {
    sub examples/counter-window/src/window.rs \
        '("result": \{ "ok": true \}\n\s*\}\)\);\n)(\s*)event_loop\.exit\(\);' \
        '\1\2panic!("quit-panic plant");'
}
