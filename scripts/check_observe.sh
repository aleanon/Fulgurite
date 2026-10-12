#!/usr/bin/env bash
# M57/A17: every widget in fulgurite-widgets tells a screen reader and
# the agent what it is. `Node::observe` is provided, so a widget that
# forgets it compiles and publishes `GenericContainer` with no value --
# which is how ten widgets shipped silent (docs/widget-audit.md X9).
# A `Node` impl without `fn observe` fails here unless it is listed below
# with the reason it has nothing of its own to say.
set -euo pipefail
cd "$(dirname "$0")/.."

# name  reason
ALLOW='
Glass      a wrapper: its content speaks
Popup      a wrapper: its content speaks
ScrollView a wrapper: its content speaks
Card       a wrapper: its content speaks
Pin        a wrapper: its content speaks
Pop        a wrapper: its content speaks
Tooltip    a wrapper: its control and its tip speak
'

missing=$(for f in crates/fulgurite-widgets/src/*.rs; do
  awk -v F="$f" '
    /^impl.* Node for /{ match($0,/Node for [A-Za-z]+/); name=substr($0,RSTART+9,RLENGTH-9); has=0; depth=0; inimpl=1 }
    inimpl && /fn observe\(/{has=1}
    inimpl{ n=gsub(/\{/,"{"); m=gsub(/\}/,"}"); depth+=n-m; if(depth==0 && seen){ if(!has) print F": "name; name=""; inimpl=0; seen=0 } if(n>0) seen=1 }
  ' "$f"
done)

bad=0
while IFS= read -r line; do
  [ -z "$line" ] && continue
  name=${line##*: }
  if ! grep -qE "^$name[[:space:]]" <<<"$ALLOW"; then
    echo "no observe: $line"
    bad=1
  fi
done <<<"$missing"
if [ "$bad" -ne 0 ]; then
  echo
  echo "Give it an observe (a role, and a value if it shows one), or list it"
  echo "in scripts/check_observe.sh with the reason it has nothing to say."
  exit 1
fi
echo "observe: every widget speaks or is listed"
