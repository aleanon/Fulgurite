# R18-F1 (M42/T42.0, docs/plan-drive-identity.md §1.1): a leftover window
# holds the port. Both "strategy" legs of widget-drive used to fail to
# bind, drive that one stranger, and print `0 failed` with exit 0. Once
# fixed, the launcher refuses a port that already answers, before any
# build is driven. The stranger is started by run_check (the "edit" is a
# process, not a file), and it must be answering before the check runs.
PORT=7890
CHECK="LUMEN_AGENT_PORT=$PORT ./scripts/widget_drive.sh Switch"
EXPECT="port already answering on 127\.0\.0\.1:$PORT"
plant() { :; }
applied() {
    cargo build --release -q -p showcase --features agent || return 1
    [ -x target/release/showcase ]
}
run_check() {
    cp target/release/showcase target/selftest-stranger
    LUMEN_AGENT="127.0.0.1:$PORT" LUMEN_AGENT_ADDR="127.0.0.1:$PORT" \
        target/selftest-stranger >/dev/null 2>&1 &
    local s=$!
    local up=0
    for _ in $(seq 100); do
        if (exec 3<>"/dev/tcp/127.0.0.1/$PORT") 2>/dev/null; then up=1; break; fi
        sleep 0.2
    done
    if [ "$up" -ne 1 ]; then
        echo "selftest: the stranger never answered on $PORT -- the plant did not apply"
        kill "$s" 2>/dev/null; wait "$s" 2>/dev/null
        return 1
    fi
    echo "selftest: stranger pid $s answering on 127.0.0.1:$PORT"
    bash -c "$CHECK"
    local rc=$?
    kill "$s" 2>/dev/null; wait "$s" 2>/dev/null
    return "$rc"
}
