"""A literal-aware reader of Rust source, for the drift checks (M43).

The checks that grep Rust used to strip comments with `sed 's@//.*$@@'`,
which also strips from a `//` inside a string literal -- so
`let s = "http://x"; unsafe { .. }` hid its `unsafe` (R17-F2) -- and read
comment text only from lines that *start* with `//`, so trailing and
`/* .. */` comments were never checked (R17-F15).

This is a small lexer, not a parser. It knows exactly the token kinds that
decide whether a `//` or a `"` is what it looks like:

  * string literals: "..", b"..", c"..", with backslash escapes;
  * raw strings: r".."  r#".."#  br#".."#  cr#".."# (any number of #);
  * char and byte literals ('x', '\\n', '\\u{..}', b'x') -- told apart from
    lifetimes ('a, 'static) by whether a closing quote follows one char;
  * line comments (//, ///, //!) and nested block comments (/* /* */ */).

`split(src)` returns `(code, comments)`:

  code      the source with every literal's CONTENTS and every comment
            replaced by spaces, newlines kept, so line numbers still hold
            and a grep over it sees only code;
  comments  a list of (line, kind, text), kind being "line" (the comment
            is the first thing on its line), "trailing" (code precedes it
            on that line), or "block".

Usage as a script:  python3 rustsrc.py code FILE     -> the blanked code
                    python3 rustsrc.py nocomments FILE... -> the code of
                        every file, comments blanked, literals KEPT
                    python3 rustsrc.py comments FILE... -> file<TAB>line<TAB>kind<TAB>text
                    python3 rustsrc.py grep REGEX FILE...
                        -> file:line:original line, for every line whose
                           CODE (literals and comments blanked) matches
"""

import sys


def _ident(c):
    return c.isalnum() or c == "_"


def split(src, keep_literals=False):
    out = list(src)
    comments = []
    n = len(src)
    i = 0
    line = 1
    line_has_code = False

    def blank(a, b):
        for k in range(a, b):
            if out[k] != "\n":
                out[k] = " "

    def blank_lit(a, b):
        if not keep_literals:
            blank(a, b)

    while i < n:
        c = src[i]
        if c == "\n":
            line += 1
            line_has_code = False
            i += 1
            continue
        # Line comment.
        if src.startswith("//", i):
            j = src.find("\n", i)
            j = n if j < 0 else j
            comments.append((line, "trailing" if line_has_code else "line", src[i + 2 : j]))
            blank(i, j)
            i = j
            continue
        # Block comment, nested.
        if src.startswith("/*", i):
            start, start_line = i, line
            depth = 0
            while i < n:
                if src.startswith("/*", i):
                    depth += 1
                    i += 2
                elif src.startswith("*/", i):
                    depth -= 1
                    i += 2
                    if depth == 0:
                        break
                else:
                    if src[i] == "\n":
                        line += 1
                    i += 1
            comments.append((start_line, "block", src[start + 2 : max(start + 2, i - 2)]))
            blank(start, i)
            continue
        prev = src[i - 1] if i > 0 else " "
        # Raw strings: r"..", r#".."#, br.., cr.. -- only at a token start.
        if not _ident(prev) and (c == "r" or (c in "bc" and i + 1 < n and src[i + 1] == "r")):
            k = i + (1 if c == "r" else 2)
            h = 0
            while k < n and src[k] == "#":
                h += 1
                k += 1
            if k < n and src[k] == '"':
                close = '"' + "#" * h
                end = src.find(close, k + 1)
                end = n if end < 0 else end
                line += src.count("\n", k + 1, end)
                blank_lit(k + 1, end)
                i = end + len(close)
                line_has_code = True
                continue
        # Byte / C string prefix: b".." c"..".
        if not _ident(prev) and c in "bc" and i + 1 < n and src[i + 1] == '"':
            i += 1
            c = '"'
        # Normal string.
        if c == '"':
            k = i + 1
            while k < n and src[k] != '"':
                if src[k] == "\\":
                    k += 1
                k += 1
            line += src.count("\n", i + 1, k)
            blank_lit(i + 1, k)
            i = k + 1
            line_has_code = True
            continue
        # Byte char b'x'.
        if not _ident(prev) and c == "b" and i + 1 < n and src[i + 1] == "'":
            i += 1
            c = "'"
        # Char literal, or a lifetime.
        if c == "'":
            if i + 1 < n and src[i + 1] == "\\":
                k = i + 2
                while k < n and src[k] != "'":
                    k += 1
                blank_lit(i + 1, k)
                i = k + 1
                line_has_code = True
                continue
            if i + 2 < n and src[i + 2] == "'":
                blank_lit(i + 1, i + 2)
                i += 3
                line_has_code = True
                continue
            # A lifetime or a label: plain code.
            i += 1
            line_has_code = True
            continue
        if not c.isspace():
            line_has_code = True
        i += 1
    return "".join(out), comments


def _read(path):
    return open(path, encoding="utf-8").read()


def main(argv):
    mode = argv[1]
    if mode == "code":
        sys.stdout.write(split(_read(argv[2]))[0])
    elif mode == "nocomments":
        for path in argv[2:]:
            sys.stdout.write(split(_read(path), keep_literals=True)[0])
            sys.stdout.write("\n")
    elif mode == "comments":
        for path in argv[2:]:
            for ln, kind, text in split(_read(path))[1]:
                sys.stdout.write(f"{path}\t{ln}\t{kind}\t{text.replace(chr(10), ' ')}\n")
    elif mode == "grep":
        import re

        pat = re.compile(argv[2])
        for path in argv[3:]:
            src = _read(path)
            code = split(src)[0].split("\n")
            orig = src.split("\n")
            for k, line in enumerate(code):
                if pat.search(line):
                    sys.stdout.write(f"{path}:{k + 1}:{orig[k].strip()}\n")
    else:
        sys.exit(f"rustsrc.py: unknown mode {mode}")


if __name__ == "__main__":
    main(sys.argv)
