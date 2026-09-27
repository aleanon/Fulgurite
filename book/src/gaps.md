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

**A style file format.** Stylesheets are `Vec<Rule>` in Rust. There is no
parser, and the `.lss` shape is referenced as a possible future rather
than a thing that exists.

**Italic, and rich text beyond weight.** `RichText` and `Markdown`
(M27/T27.12) put a bold word inside a sentence, but a span's only
typographic axis is weight, so Markdown has no italic and says why. A
span is a word: it never wraps inside itself.

**Arbitrary vector paths.** Lines and gradients shipped; general paths
and SVG rendering did not. The drawing vocabulary is the `Cmd` enum in
`crates/lumen-static/src/display.rs`, and is meant to stay small enough
for an embedded target to implement.

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
