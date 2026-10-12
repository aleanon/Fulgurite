#!/usr/bin/env bash
# M48/T48.12 (R14-F8): a window built without the `agent` feature links
# no agent.
#
# Builds examples/showcase for release twice -- without `agent`, then with
# it -- and counts symbols from `fulgurite_agent` and `serde_json` in
# each with `nm -C`. The first must be 0; the second is the positive
# control and must not be, or the count is blind (a stripped binary, a
# renamed crate) and would pass anything. accesskit is not counted: M54
# may make a screen-reader tree default-on for desktop, and then it is
# linked without the agent on purpose (docs/plan-agent-truth.md T48.12).
#
# Compiles two release builds, so it is not in `static`; `just
# agent-symbols` runs it, and `check-extra` includes it.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
out=target/agent-symbols
mkdir -p "$out"
count() { nm -C "$1" 2>/dev/null | grep -cE "$2" || true; }
cargo build -q --release -p showcase
cp target/release/showcase "$out/plain"
cargo build -q --release -p showcase --features agent
cp target/release/showcase "$out/agent"
a_plain=$(count "$out/plain" 'fulgurite_agent::')
j_plain=$(count "$out/plain" 'serde_json::')
a_agent=$(count "$out/agent" 'fulgurite_agent::')
j_agent=$(count "$out/agent" 'serde_json::')
echo "agent-symbols: fulgurite_agent symbols: $a_plain / bound 0 (agent build: $a_agent)"
echo "agent-symbols: serde_json symbols: $j_plain / bound 0 (agent build: $j_agent)"
if [ "$a_agent" -eq 0 ]; then
    echo "agent-symbols: the agent build links no agent symbols -- the count is blind" >&2
    exit 1
fi
if [ "$a_plain" -ne 0 ] || [ "$j_plain" -ne 0 ]; then
    echo "agent-symbols: a build without the agent feature links it" >&2
    exit 1
fi
