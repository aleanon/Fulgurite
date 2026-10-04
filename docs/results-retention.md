# Streaming retention vs. a copied display list — M13/T13.8

**Ran 2026-09-05.** Reference machine: i9-13900KF, governor
`powersave`, turbo on — the same machine every other `docs/results-*.md`
number here was taken on. `cargo run --release -p benches --bin retain`,
median of blocked runs.

## The question

T13.8 replaced retention that kept the previous `DisplayList` with
retention that observes commands as they stream past a `Painter`
wrapper, keeping `(hash, ink)` per command. The motivation was that the
old shape *typed* `Retention` against a list — `record(&DisplayList)` —
so a host with no list could not have damage at all, which is the M13
device (`docs/results-embedded.md`).

But motivation is not permission. Streaming hashes every command where
copying compared stored ones, so it might simply be worse, and the plan
set the gate accordingly: **keep the copying implementation if streaming
is not faster.**

## The numbers

Commands are issued straight at a `Painter`, not painted from a tree:
that is the only way to reach a 2 000-command frame, and it keeps the
tree walk — which both forms pay identically — out of the measurement.
`build the list` is the shared floor; retention's own cost is the
difference from it.

| | 34 commands | 2 000 commands | ns/command |
|---|---:|---:|---:|
| build the list (floor) | 225.5 ns | 12 768 ns | — |
| **copying**: copy + diff | **88.8 ns** | **6 879 ns** | 3.44 |
| **streaming**: `Incremental` | **79.8 ns** | **5 421 ns** | 2.71 |
| **streaming**: `Streaming<4096>` | **33.9 ns** | **2 756 ns** | 1.38 |
| `Recording` wrapper alone (`Rebuild`) | ~0 | 155 ns | 0.08 |

And the row that only the new shape can produce at all:

| | 34 commands | 2 000 commands |
|---|---:|---:|
| streaming, **no list built** | 197.6 ns | 11 435 ns |

## Verdict

**Streaming is faster at both scales, so it replaces the copying form
rather than joining it.** No second `Retention` impl is needed for the
desktop.

Four things the measurement says that the plan did not predict:

1. **`Streaming<N>` beats `Incremental` by 2×** (1.38 vs 2.71 ns per
   command). Same algorithm; the difference is a fixed array indexed
   inline against a `Vec` that costs a pointer load and a second bounds
   check per command. The allocator-free implementation is the *fast*
   one, which is not the usual direction.
2. **`Recording` is free.** 0.08 ns per command for the wrapper, at the
   noise floor. Wrapping the painter costs nothing measurable, which is
   what makes the `paint → Recording(Scaled(sink))` composition
   affordable.
3. **Damage with no list is cheaper than a list with no damage.**
   11 435 ns against a 12 768 ns floor: a host that streams to an
   immediate-mode painter *and* computes damage does less work than one
   that merely builds a display list. That is the M10a seam paying off.
4. The desktop saving is ~4.1 µs a frame against a render measured in
   `docs/results-frame-cost.md` — real, but the reason to do this is the
   device, where the cost is the SPI bus rather than the raster.

## What it costs

**Equality became a 64-bit hash.** Two genuinely different commands that
collide would leave stale pixels. At ~2 000 commands per frame that is
a birthday probability around 10⁻¹³ per frame **for text nobody chose**
— the figure assumes a random function, and until M46/T46.1 the text
half of the hash was not one: see "What a text hash guarantees" below.
`crates/lumen-static-shell/tests/damage_oracle.rs` is the standing
check: whatever computes the damage, a partial redraw must stay
pixel-identical to a full one. It is parameterised over `Retention` as
of this task, so `Streaming<256>` gets the same 25-step pixel
comparison `Incremental` does — an implementation nothing renders
through is one nobody has checked. `Streaming<2>`, which cannot hold
the frame, is checked separately: it must report `All` on every step
and never a rect computed from mismatched slots.

**And one thing it gained.** `Cmd::Text` carries a *range* into a list's
text buffer, so two frames whose strings differ but sit at the same
offset and length — "12" becoming "13" — produced identical commands.
The copying diff needed a special case (`text_differs`) to catch that,
and a label that never repaints is what forgetting it looks like. A
stream is handed the `&str`, so the case does not exist to forget.

## What a text hash guarantees (M46/T46.1, 2026-09-26)

**Until T46.1 a text command's bytes went through `display::fold`**,
`h·K + b` over ℤ/2⁶⁴. That is linear, so two strings of one length
collide whenever a small integer combination of their byte differences
vanishes; a lattice reduction finds such pairs, and folding the rect
first does not help, because the prefix cancels (M38's R2-F2, R2-F3).
`crates/lumen-static-shell/tests/text_hash_collision.rs` shows it
end to end: `" k ih  lsio "` → `"$a#aa$-aaaa("` reported `Damage::None`
and left **1,440 stale pixels** through the default `Incremental` host.
The same fold keyed `FoldedText`, from 0 and with no length, so `"x"`
and `"\0x"` shared a memo and `Button`, `Badge` and `Chip` served the
wrong width (`crates/lumen-static-widgets/tests/folded_text_identity.rs`).

**Now all three text call sites — `Recording::text_spaced` (`text_weighted` until M80/T80.1),
`DisplayList::hash_from` and `FoldedText::fold_of` — call
`display::hash_bytes`**: the length as seed, a folded multiply per 8-byte
word (the 128-bit product's halves xored), SplitMix64's finaliser.
Scalars keep `fold`. What that guarantees:

- **Structural:** strings of different lengths start from different
  seeds, and below a word the tail is read one-to-one, so every string
  of up to two bytes has its own hash (`crates/lumen-static/tests/text_hash.rs`,
  65,793 of 65,793 distinct).
- **Algebraic:** the old fold's solved pairs, and a pair *constructed*
  for the review's rotate-xor-multiply mix, are all separated. That mix
  was rejected on this: its per-word step is a bijection of the word, so
  a second word can be computed to cancel a first with one xor
  (`--bin text-hash` builds such a pair at start-up and prints
  `constructed=true` for it). A folded multiply is not a bijection.
- **Not adversarial.** It is unkeyed. Someone who reads it can search for
  a collision, and what a found one does is keep a stale label on screen
  (R2-F2's verified reach); it cannot skip any other command. The
  retention stream is not a security boundary. A keyed PRF was measured
  (below) and refused on K1.

**The kill rule was written before the build** (`docs/plan-hash-identity.md`
§5), and measured as it says: eight runs a side, the pre-T46.1 binary
(A) and the T46.1 binary (B) alternating, pinned to one core, under the
bench lock; difference of medians, ns.

| arm | A median (min–max) | B median (min–max) | B − A | bound | verdict |
|---|---|---|---|---|---|
| K1 `FoldedText` hit, 9 B (`--bin text-hash`) | 5.60 (5.35–7.83) | 5.50 (5.49–6.62) | **−0.10** | +0.5 | pass |
| K1 `FoldedText` hit, 60 B | 25.73 (25.71–31.25) | 6.21 (5.94–7.96) | **−19.53** | +0.5 | pass |
| K1 `Badge` layout, unchanged (`--bin bindings`) | 7.42 (7.40–7.49) | 7.69 (7.68–7.89) | **+0.28** | +0.5 | pass |
| K1 `Chip` layout, unchanged | 7.42 (7.39–7.47) | 7.69 (7.65–7.88) | **+0.26** | +0.5 | pass |
| K2 `--bin retain --text`, 2,000 text commands, build + `Incremental` | 32,356 (32,300–51,383) | 19,505 (19,097–19,890) | **−12,851 (−39.7%)** | +42.6 (the floor) | pass |
| K3 leaf firmware `.text` (`scripts/check_text.sh`) | 27,024 B | 27,424 B | **+400 B** | 38,512 B headroom | pass |

Loads: K1 `text-hash` 13.83 → 11.45, `bindings` and K2 9.93 → 13.84, on
32 cores. **K2's floor** is the base binary against a copy of itself, the
same way, before T46.1 was built: 32,249 against 32,292 ns, **+42.6 ns
(+0.13%)**. The bare function, without the memo around it: 2.09 → 2.20 ns
at 9 B (+0.11), 22.97 → 4.20 ns at 60 B.

**The candidates, one binary, `--bin text-hash`** (medians of eight, ns,
pinned, load 3.57 → 3.52, before T46.1; `survives` = the pair still
collides):

| arm | 9 B | 60 B | solved pair | `"x"`/`"\0x"` | constructed |
|---|---|---|---|---|---|
| linear (pre-T46.1) | 2.10 | 22.99 | survives | survives | — |
| length + finaliser | 3.20 | 25.41 | **survives** | separated | — |
| rotate-xor-multiply word mix (M38's) | 2.38 | 3.71 | separated | separated | **survives** |
| folded-multiply word mix (**shipped**) | 2.20 | 4.20 | separated | separated | separated |
| SipHash-1-3, per-process key | 4.74 | 10.55 | separated | separated | separated |

SipHash-1-3 would have failed K1 at 9 B by itself (+2.6 ns on the bare
function before a memo is consulted), before counting the key's
plumbing through `LayoutCx` into `Recording`, which a `no_std` core with
no entropy source and no 64-bit atomics on its embedded target cannot
set once and read everywhere. **Length plus a finaliser**, the first
plan's fix, does not separate same-length pairs at all (R2-F4); the gate
shows it, with that function swapped in as the injection.

**K3's +400 B is all in `Button::layout`** (848 → 1,248 B): the firmware
links a `Button`, whose `FoldedText` key is `hash_bytes` inlined, and no
`Recording`. A `thumbv7em-none-eabihf` build of the core puts the whole
function at +532 B inside `DisplayList::hash_from` (418 → 950 B), which
the firmware does not link.

**What changing it breaks.** Every text hash value changed. The values
live in a host's retention state and in `FoldedText` memos, and a host
built before T46.1 compared against a library built after it compares
unequal once: one full-damage frame, or one re-measure, and never a
skipped one — the safe direction.
