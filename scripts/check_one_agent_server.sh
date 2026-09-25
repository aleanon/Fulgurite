#!/usr/bin/env bash
# M39/T39.1: one agent transport, and nothing else listens.
#
# R14-F1 (a web page can drive a dev app) was carried by TWO servers,
# because the socket loop had been copied into examples/counter-window.
# Fixing one would have left the other open. So the workspace has exactly
# one call that binds a TCP listener outside tests, and it is in
# crates/lumen-static-agent/src/transport.rs (docs/plan-agent-transport.md).
#
# Scans every .rs under crates/, examples/, fixtures/ and benches*, except
# tests/ directories (a test binds its own listener to play a client
# against). Comments are stripped first, so the rule can be written about.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

want="crates/lumen-static-agent/src/transport.rs"
sites=()
while IFS= read -r f; do
    while IFS= read -r hit; do
        [ -n "$hit" ] && sites+=("$f:$hit")
    done < <(sed -E 's@//.*$@@' "$f" | grep -nE '\b(TcpListener|UnixListener)::bind\b' | cut -d: -f1 || true)
done < <(find crates examples fixtures benches* -name '*.rs' -not -path '*/target/*' \
    -not -path '*/tests/*' 2>/dev/null | sort)

for s in "${sites[@]}"; do echo "one-agent-server: listener bound at $s"; done
if [ "${#sites[@]}" -ne 1 ] || [[ "${sites[0]}" != "$want":* ]]; then
    echo "one-agent-server: expected exactly one listener bind, in $want; found ${#sites[@]} (above)." >&2
    echo "  Serve through lumen_static_agent::transport::serve_addr instead of a second loop." >&2
    exit 1
fi
echo "one-agent-server: 1 listener bind, in the transport"
