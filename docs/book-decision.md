# The book lives in this repository — decision and measurements

*2026-09-10. Question: should the mdBook for lumen-static be a directory
here or a repository of its own?*

**Verdict: here, in `book/`.** Built with mdBook v0.5.4, gated by
`scripts/check_book_includes.sh` (every commit) and
`scripts/build_book.sh` (pre-push, via `just book`).

## Why not its own repository

The ecosystem split is real but it tracks one variable, and it is not
project size. Books separate when they have a different **cadence**, a
different **author set**, or a different **licence** — `rust-lang/book` is
written by people who do not land compiler patches. Books stay in-repo
when the person writing the prose is the person writing the code:
wasmtime (`docs/`), cargo (`src/doc/`), rustup (`doc/`), embassy. This
project is the second case on all three variables.

The deciding constraint is local, though, and it is `AGENT.md`'s
doc-currency rule: a commit that changes public behaviour must update the
affected prose **in the same commit**. Across a repository boundary there
is no such commit — the best available is a submodule pin, which is a
second commit in a second place that no reviewer sees in the diff. The
measured failure rate for that rule when docs are written in a separate
pass is 11 of 11 milestone-sized commits (`AGENT.md`, measured
2026-09-02). A second repository makes every book commit a separate pass
by construction.

One checkout also buys the two mechanisms the book depends on, neither of
which works across repositories:

* `{{#include ../../examples/counter/src/main.rs:tree}}` — chapters show
  code from files the build compiles and `just fixtures` runs, so a
  chapter cannot describe an API the code no longer has.
* `mdbook test -L target/book/debug/deps` — every snippet is compiled and
  **run** against the real rlibs. Out of repo it could only be tested
  against a *published* version, i.e. against yesterday's API.

## What mdBook does when an include breaks

Measured 2026-09-04 through 2026-09-10 on mdBook v0.5.4, by injection.
This is why the gates exist; none of it is visible from a passing build.

| injected fault | `mdbook build` | `mdbook test` | effect on the page |
|---|---|---|---|
| anchor renamed | **silent** | **exit 0** | include expands to the empty string |
| file missing | `ERROR` logged | fails to parse | literal `{{#include}}` left in the source |
| line-number include drifts | silent | passes | includes the wrong lines |

The first row is the dangerous one and it is the repository's documented
vacuous-test shape — *the setup silently did not happen, so both sides
agree about a frame in which nothing occurred*. Renaming
`// ANCHOR: app` in `examples/minimal` shrank the introduction's example
from **26 rendered lines to 3** — the two hidden `extern crate` lines and
the fence — while `mdbook build` printed no diagnostic and `mdbook test`
exited 0. The book's front page lost its entire example and every check
stayed green.

Row two is why `scripts/build_book.sh` greps stderr for `ERROR` instead
of trusting the exit status: mdBook reports a missing include file and
then **exits 0 anyway**.

Row three is why line-number includes are refused rather than checked. A
line number is a bound computed from a quantity that changes on every
edit, and it fails by silently including different code. Named anchors
only.

## Two tooling traps, recorded so they are not rediscovered

**`-L target/debug/deps` does not work in this workspace.** That
directory holds 6818 files and **36** distinct `liblumen_static-*.rlib`
built with different feature sets; rustdoc refuses to choose (E0464) and
the error names neither the cause nor the fix. `just book-deps` builds
exactly one of each into `target/book`, and **wipes that directory
first** — cargo never deletes a superseded artifact, so building
`-p counter` and later `-p counter -p minimal` leaves two rlibs behind
and reintroduces the ambiguity. The wipe costs ~8 s and makes the
directory a function of one line.

**rustdoc compiles book snippets as edition 2015 unless told otherwise.**
Without `[rust] edition = "2021"` in `book.toml`,
`use lumen_static::prelude::*` cannot resolve and the compiler's
suggestion ("you might be missing a crate named `lumen_static`") points
away from the cause.

## Rules this establishes

1. **The book cites; it does not restate.** Every figure in a chapter
   names the `docs/results-*.md` that owns it, in the same sentence —
   the existing "numbers are stated once" rule, applied to a surface
   that will be tempted to repeat them in friendlier prose.
2. **Code in the book is included, never retyped.** By named anchor.
3. **A SUMMARY.md entry that points nowhere is an error**, not a request
   for a blank page (`create-missing = false`). Unwritten chapters are
   draft entries.

## Verification

Both directions, per `AGENT.md`'s rule that a gate refusing bad input
must also accept good input. Seven injections, 2026-09-10: renamed
anchor, missing file, line-number include, malformed anchor name, a
flipped `NODE_COUNT` assertion (8 → 9, which fails at *runtime* —
proving snippets are executed, not merely compiled), a dangling
`docs/*.md` citation in a chapter, and a dead backticked identifier in a
chapter. Each names the file and the anchor; with all seven reverted the
gates report `6 includes, all resolve` and the snippets pass.

The first attempt at the include gate **survived its own injection**:
`grep -F "ANCHOR: $name"` is a substring match, so renaming
`// ANCHOR: tree` to `// ANCHOR: treex` still matched and the gate
reported 6 of 6 resolving while two chapters had already stopped
compiling. The bound was computed from a prefix rather than the name.
The check is end-anchored now.
