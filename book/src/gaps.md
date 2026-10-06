# What this does not do

A framework's honest gap list is worth more than its feature list, and
this one comes with a caveat that is itself instructive.

## Read the survey, and check its date

`docs/competitive-feature-gaps.md` compares this framework against Qt,
GTK, iced, Xilem, Slint and Lumen, per framework, from the sources on the
machine rather than from memory. It is the right document for "what does
a mature toolkit have that this does not".

**Its "common to all six" table is stale.** It was surveyed on
2026-09-06, and M15 shipped thirteen of the capabilities it lists as
missing on 2026-09-07 — images, vector lines and gradients, animation,
touch and gestures, focus navigation, drag and drop, complex-script text,
localisation, multi-window, platform dialogs and a GPU backend, with the
overlay mechanism following. A reader taking that table at face value
today would conclude the framework is a milestone smaller than it is.

This is the failure mode the whole book is built to avoid, arriving
through a citation rather than around it — which is why it is named here
rather than quietly worked around. The per-framework sections and the
"how much of this is a decision" section are the parts still worth
reading as written.

## What is genuinely absent

From the code, not from the survey:

**A menu bar.** There is a dropdown menu — `Menu`, one of the three
`Chooser` kinds (M27/T27.9) — and the overlay phase that carries it, but
no application menu bar and no platform menu integration.

**Most of a style language.** Since M61 a sheet can be a `.lss` file,
parsed at compile time ([Styling](./styling.md)), but only the part of
Lumen's grammar this cascade can apply: one simple selector per rule. No
compound selectors, no combinators, no `:state`, `@media`, `@keyframes`
or `@theme`, and no classes that change at run time — each is a compile
error that says so (`docs/plan-lss.md` §4).

**Italic, and rich text beyond weight and colour.** `RichText` and
`Markdown` (M27/T27.12) put a bold word inside a sentence, and since M87
a `Markdown` has a per-element theme, blockquotes, and links — drawn in
their own colour and underlined, and in a `LinkedMarkdown` running the
application's handler with the link's index (`docs/plan-markdown.md`).
But a span's only typographic axis is weight, so Markdown has no italic
and says why. A span is a word: it never wraps inside itself.

**Most of vector drawing.** Paths shipped in M82/T82.5: `Painter::path`
fills and strokes moves, lines and quadratic and cubic Béziers under a
per-path transform, with round joins and caps, on the software canvas
(and the GPU backend draws them since M82/T82.6, as coverage masks in
its glyph atlas). Text turns too, since M82/T82.8: a `VectorText` paints
its run's outlines as one path (see [Text](./text.md)). What did not ship
is the rest of SVG: miter joins, dashes, even-odd fill, `clip-path`, a
transform for any command but a path, and SVG import. The drawing vocabulary is the
`Cmd` enum in `crates/lumen-static/src/display.rs`, and its box commands
stay small enough for an embedded target to implement.

**A layer tree in the GPU backend.** There is a persistent target texture
that damage is scissored into, but no per-subtree texture and no
transform-only frame — see [The retained-mode baseline](./baseline.md).

**A model layer.** No lists-with-selection abstraction, no sorting or
filtering views. `Repeat` gives you virtualisation; what you put in the
rows is yours.

## What is deliberate rather than missing

The survey's own distinction is the useful one, and these have recorded
reasoning rather than an empty backlog slot:

* **A reactive view tree.** Recorded in `docs/plan-lumen-parity.md` under
  "what we should not copy".
* **A heavyweight text stack.** Measured and declined, twice, in
  `docs/text-render-adoption-decision.md`.
* **Pipelined raster.** `docs/results-threading.md` shows it raises
  input-to-photon latency by a frame and pays almost nothing here.
* **Per-instance styling of arena content.** A consequence of styling by
  template — see [Containers and escape
  hatches](./containers.md).

## And the two things that run the other way

A gap list read alone is misleading, so the survey ends by naming what
this framework has that none of the six do:

* **A no-allocator profile.** Not "low allocation" — none, with a
  measured `.text` footprint and an empty `.bss`
  (`docs/results-embedded.md`).
* **Source provenance on every node**, exposed through an agent protocol
  — a report that names a file and a line rather than a widget. See [The
  agent protocol](./agent.md).

Both are consequences of the static tree, and they are the reason for
choosing it. If neither matters to your project,
[What it costs](./costs.md) is the chapter to reread.
