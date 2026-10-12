#!/usr/bin/env bash
# M48/T48.11 (R13-F10): one environment variable names the agent's address.
#
# Launchers read `FULGURITE_AGENT` (showcase, tasks-demo, hot-window,
# panic-app) while counter-window, the client and the drive runner read
# `FULGURITE_AGENT_ADDR` -- so `FULGURITE_AGENT_ADDR=... showcase` bound one port
# while the client dialled another. The window reads `FULGURITE_AGENT_ADDR` in
# one place, `fulgurite-winit`'s `start`, and honours the old name for
# one release with a deprecation line (docs/plan-agent-truth.md §2.5).
#
# Fails on any code line outside crates/fulgurite-winit that names the
# bare `FULGURITE_AGENT` (not `FULGURITE_AGENT_ADDR`, `FULGURITE_AGENT_ALLOW_REMOTE`).
# **Positive control**: it must find the shim inside the winit crate, or
# it is blind and fails as such.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

pat='FULGURITE_AGENT([^_A-Za-z0-9]|$)'
files=$(find crates examples fixtures scripts benches* -type f \( -name '*.rs' -o -name '*.py' -o -name '*.sh' \) \
    -not -path '*/target/*' -not -path 'scripts/check_agent_env.sh' -not -path 'scripts/selftest/*' 2>/dev/null | sort)
bad=0
shim=0
for f in $files justfile; do
    while IFS= read -r hit; do
        [ -z "$hit" ] && continue
        case "$f" in
            crates/fulgurite-winit/*) shim=$((shim + 1)) ;;
            *) echo "agent-env: $f:$hit"; bad=$((bad + 1)) ;;
        esac
    done < <(grep -nE "$pat" "$f" || true)
done
echo "agent-env: $bad reader(s) outside the window crate / bound 0; shim sites in the window crate: $shim (must be >= 1)"
if [ "$shim" -eq 0 ]; then
    echo "agent-env: found no FULGURITE_AGENT shim in crates/fulgurite-winit -- the check is blind" >&2
    exit 1
fi
if [ "$bad" -ne 0 ]; then
    echo "agent-env: read FULGURITE_AGENT_ADDR through fulgurite-winit (Program::agent is the default)" >&2
    exit 1
fi
