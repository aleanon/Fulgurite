# Step 1 — compile-time firewall: **PASS**

*2026-08-30. The §10 Step 1 kill criterion from
`docs/00-handoff-static-tree.md`, measured. This gate could have said no;
it says yes, with the caveats below recorded so the next reader knows what
was and was not measured.*

## The criterion

Fixed before measuring (handoff §12.7): **leaf edit → usable rebuild in
under 2s** on the reference machine.

Reference machine: i9-13900KF (32 threads), Linux 6.8, `rustc 1.94.0`
(same pin as Lumen), dev profile defaults (incremental on, full debuginfo),
toolchain-default linker (rust-lld on this target). Timings are wall clock
via `scripts/measure_firewall.sh`; three leaves per arm (comp007, comp100,
comp191), spread across the dependency list on purpose.

## The setup

`scripts/gen_firewall.py` emits a workspace of 200 component crates, each
exporting only what the design's §9 seam allows: a `static SUB_TABLE:
[NodeDesc; N]`, a `static OPS: [WidgetOps; K]` of fn pointers, and nothing
generic. A thin `root` binary concatenates the 200 sub-tables at startup and
runs one kind-dispatched layout+paint loop. The control arm (`monolith`) is
the *identical* component code as 200 modules of a single crate — the
"no firewall" world. Components carry real function bodies and intra-crate
generic `Pair<A, B>` instantiations standing in for `#[fuse]`.

Two scales, because a toy that passes proves nothing about the trend:

| | light (`--kinds 8 --nodes 24`) | heavy (`--kinds 24 --nodes 96`) |
|---|---|---|
| lines per component | ~250 | ~994 |
| total nodes | 4,800 | 19,200 |

## The numbers (median of 3)

| arm | light | heavy | scaling |
|---|---|---|---|
| full cold build | 2,223 ms | 5,682 ms | with total code |
| no-op `cargo build -p root` | 47 ms | 50 ms | flat |
| **firewall**: edit leaf → rebuild `root` binary | **197 ms** | **254 ms** | leaf size + fixed tail |
| leaf-only: edit leaf → rebuild that rlib | 72 ms | 89 ms | leaf size |
| **monolith control**: same edit, one crate | **627 ms** | **1,577 ms** | **with total app size** |

(The heavy-scale monolith arm was re-measured with `#![allow(dead_code)]`
to rule out warning-emission overhead: 1,560/1,577 ms — unchanged, so the
warnings were not distorting the control.)

## Reading

- **The criterion holds with ~8× headroom.** A leaf edit is a usable binary
  in ~0.25s at the heavy scale. The design's claimed structure is visible in
  the decomposition: leaf compile (~90 ms) + a fixed ~165 ms tail (root
  crate re-check against 200 dependency metadata files + link). The tail did
  not grow when component weight quadrupled.
- **The control arm shows what the firewall is for.** The monolith scales
  with *total* app size — 4× the code made the same one-line edit 2.5×
  slower, incremental compilation notwithstanding. Extrapolated, it crosses
  the 2s budget somewhere around 250k total lines; the firewall arm has no
  comparable growth term on the leaf path.
- **The ratio grew from 3.2× to 6.2×** between the scales. The advantage
  compounds exactly where the design needs it to — at "extremely large".
- The leaf-only arm (~90 ms) is the proxy for the §9 hot-reload unit
  (rebuild one component as a dylib, swap): sub-100 ms before any dylib
  machinery exists.

## What this does not prove (re-measure when real)

1. **The fixed tail grows with component count and binary size.** At 200
   crates it is ~165 ms; at 500+ crates or release-sized binaries the
   metadata-check and link terms need re-measuring. The trend here is flat
   in component *weight*, not in component *count*.
2. **No proc macros yet.** The real derive (`NodeDesc` table emission) adds
   per-crate expansion cost. It stays inside the leaf crate, so it moves the
   ~90 ms leaf term, not the structure — but measure it when the macro
   exists (Step 2).
3. **The seam is honest but simulated.** Components export only statics and
   fn pointers, which is the design's contract; if Step 2's real API leaks
   generics across the crate boundary, this result is void — that is
   precisely the §4 trap, and this experiment is the regression test for it.
   Keep the generator; rerun it against the real crate layout when one
   exists.

## Verdict

**Proceed to Step 2.** The component-boundary firewall delivers what §9
claims on the machine and budget fixed in §12.7. The generator and measure
script are kept as a permanent regression harness
(`scripts/gen_firewall.py`, `scripts/measure_firewall.sh`).
