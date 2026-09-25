"""The derives' helper attributes, read from the proc-macro source (M43/T43.3).

Shared by scripts/check_derive_attrs.sh and scripts/check_attr_table.sh,
so the two cannot disagree about what is declared.

The previous parser was `grep -o 'attributes([^)]*)'`, one line at a time:
a list split across lines disappeared with every attribute in it
(R17-F5), and the attribute-table check never read the lists at all -- it
compared two hand-written tables with each other (R17-F4).

This reads each `#[proc_macro_derive( .. )]` whole, across lines, on code
with comments and literals blanked (scripts/lib/rustsrc.py), and refuses
to answer unless it parsed as many derives as the source has
`#[proc_macro_derive` tokens -- a parser that silently misses one is the
defect being fixed.

    python3 derive_attrs.py declared      -> one `Derive<TAB>attr` per line
    python3 derive_attrs.py uses FILE...  -> `attr<TAB>count`, uses in code
                                             (comments and literals blanked)
"""

import glob
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rustsrc  # noqa: E402


def declared(root="."):
    out = []
    tokens = 0
    for path in sorted(glob.glob(os.path.join(root, "crates/*/src/**/*.rs"), recursive=True)):
        code = rustsrc.split(open(path, encoding="utf-8").read())[0]
        tokens += len(re.findall(r"#\s*\[\s*proc_macro_derive\b", code))
        for m in re.finditer(r"#\s*\[\s*proc_macro_derive\s*\(", code):
            # The balanced parenthesised body, however many lines it spans.
            i, depth = m.end(), 1
            while i < len(code) and depth:
                depth += {"(": 1, ")": -1}.get(code[i], 0)
                i += 1
            body = code[m.end() : i - 1]
            name = body.split(",")[0].strip()
            attrs = []
            a = re.search(r"\battributes\s*\(([^)]*)\)", body)
            if a:
                attrs = [x.strip() for x in a.group(1).split(",") if x.strip()]
            out.append((name, attrs, path))
    if len(out) != tokens:
        sys.exit(f"derive_attrs: parsed {len(out)} derives but the source has {tokens} "
                 "`#[proc_macro_derive` -- the parser missed one")
    return out


USE = r"#\[{a}\b|(?:^|[\s,(]){a}\s*[=(]"


def uses(attrs, files):
    code = "\n".join(rustsrc.split(open(f, encoding="utf-8").read())[0] for f in files)
    return {a: len(re.findall(USE.format(a=re.escape(a)), code, re.M)) for a in attrs}


def main(argv):
    if argv[1] == "declared":
        for name, attrs, _ in declared():
            if not attrs:
                print(f"{name}\t")
            for a in attrs:
                print(f"{name}\t{a}")
    elif argv[1] == "uses":
        attrs = sorted({a for _, al, _ in declared() for a in al})
        for a, n in uses(attrs, argv[2:]).items():
            print(f"{a}\t{n}")
    else:
        sys.exit(f"derive_attrs.py: unknown mode {argv[1]}")


if __name__ == "__main__":
    main(sys.argv)
