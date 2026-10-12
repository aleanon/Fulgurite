#!/usr/bin/env bash
# M57/D2: a widget test that hands `on_event` a press or a key directly,
# and never goes through `pump` or `dispatch`, tests the widget's arm and
# not what the pass delivers. That is what hid A4 (bubbled presses carried
# the child's coordinates), A7, A13 and the first `Event::Hover` test:
# each unit test called `on_event` with the event already in hand, and
# the pass never delivered it that way (docs/plan-widget-fixes.md D2).
#
# Such a file fails here unless it carries an `// on_event only:` comment
# saying why a direct call is the right test -- normally, which pass-level
# test in crates/fulgurite-shell/tests/ covers the delivery.
set -euo pipefail
cd "$(dirname "$0")/.."

bad=0
for f in crates/fulgurite-widgets/tests/*.rs; do
  grep -qE 'on_event\(' "$f" || continue
  grep -qE 'PointerDown|PointerUp|Event::Key' "$f" || continue
  grep -qE '\.pump\(|dispatch\(' "$f" && continue
  if ! grep -qE '// on_event only: .{20,}' "$f"; then
    echo "$f: calls on_event with a press or key and never pumps, with no \`// on_event only:\` reason"
    bad=1
  fi
done
if [ "$bad" = 1 ]; then
  exit 1
fi
echo "on_event tests: every direct call goes through the pass or says why not"
