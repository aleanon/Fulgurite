# R18-F9, the drives' half (M42/T42.0): the app panics where the drive's
# assertions cannot see it -- here a worker thread, which leaves the
# window up and every RPC answering -- and the drive scripts ended in
# `kill; wait || true`, so nothing read the log. Once fixed, the
# launcher scans the log before it stops the app.
CHECK='./scripts/widget_drive.sh Switch'
EXPECT='app log: [1-9][0-9]* panic'
plant() {
    sub examples/showcase/src/main.rs \
        '^(fn main\(\) \{\n)' \
        '\1    std::thread::spawn(|| { std::thread::sleep(std::time::Duration::from_millis(1500)); panic!("late-panic plant"); });\n'
}
