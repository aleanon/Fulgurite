"""The body of scripts/check_test_counts.sh: cargo's JSON on stdin, the
allow file as argv[1]. See that script's header."""
import json, os, re, subprocess, sys

allow = {}
if os.path.exists(sys.argv[1]):
    for line in open(sys.argv[1]):
        line = line.split("#", 1)[0].strip()
        if not line:
            continue
        key, _, why = line.partition(":")
        allow[key.strip()] = why.strip()
justfile = open("justfile").read()

bins = []
for line in sys.stdin:
    try:
        m = json.loads(line)
    except ValueError:
        continue
    if m.get("reason") != "compiler-artifact" or not m.get("executable"):
        continue
    if m["target"]["kind"] != ["test"]:
        continue
    # package_id: `path+file:///dir#name@ver`, or `path+file:///dir#ver`
    # when the package is named after its directory.
    frag = m["package_id"].rsplit("#", 1)[-1]
    pkg = frag.split("@")[0] if "@" in frag else os.path.basename(os.path.dirname(m["manifest_path"]))
    bins.append((f"{pkg}/{m['target']['name']}", m["executable"]))

zero, allowed, bad = [], 0, 0
for key, exe in sorted(bins):
    r = subprocess.run([exe, "--list", "--format", "terse"], capture_output=True, text=True)
    n = sum(1 for l in r.stdout.splitlines() if l.endswith(": test"))
    if r.returncode != 0:
        print(f"{key}: --list failed (exit {r.returncode})", file=sys.stderr)
        bad += 1
        continue
    print(f"{key}: {n} tests")
    if n:
        continue
    zero.append(key)
    why = allow.get(key)
    if why is None:
        print(f"test-counts: {key} lists ZERO tests -- a cfg compiled it out in this selection; "
              f"run it where it has tests, or allow it in {sys.argv[1]} with the reason", file=sys.stderr)
        bad += 1
    elif why == "justfile":
        p, t = key.split("/", 1)
        if not re.search(rf"-p {re.escape(p)}\b[^\n]*--test {re.escape(t)}\b", justfile):
            print(f"test-counts: {key} lists ZERO tests here and is allowed as run alone by the "
                  f"justfile, but no justfile line runs `-p {p} ... --test {t}`", file=sys.stderr)
            bad += 1
        else:
            allowed += 1
    else:
        allowed += 1
print(f"zero-test binaries: {len(zero)} of {len(bins)} (allowed {allowed}); "
      f"unallowed {len(zero) - allowed}")
sys.exit(1 if bad else 0)
