# Results — accessibility and platform (M54)

The numbers `docs/plan-a11y-platform.md` decides by. Stated once here;
the manifests, rustdoc and the book cite this file.

## T54.2 — does `a11y` default on for desktop? (§2.3's kill rule)

The rule, written before the adapter was built: `a11y` ships default-on
in `lumen-static-winit` **unless** either

1. the showcase's idle frame moves outside the eight-run spread of the
   same build without the feature, or
2. the stripped release showcase binary grows by more than **3 MB**.

**Both fired. `a11y` ships default-off**, and an application asks for it
(`examples/showcase` does, in its manifest).

### Binary size — condition 2

`examples/showcase`, release profile, no agent, `strip`ped with GNU
strip 2.42, 2026-09-27:

| build | stripped | unstripped |
|---|---:|---:|
| without `a11y` | 5,025,456 B | 6,930,960 B |
| with `a11y` | 9,349,176 B | 12,525,296 B |
| **delta** | **+4,323,720 B (4.12 MiB)** | +5,594,336 B |

Bound: 3 MB. **Fired.** The cost is `accesskit_unix` and the D-Bus stack
under it: `lumen-static-winit` resolves **95** crates without the
feature and **166** with it (`cargo tree -e normal`, deduplicated).

### Idle redraw — condition 1

`scripts/a11y_cost.py` (`just a11y-cost`): the showcase with
`--features agent`, with and without the window crate's `a11y`,
alternating, eight runs a side; each run drives 400 idle redraws
(`ui.waitSettled`, which changes nothing and ends in a redraw request)
and reads the process's CPU time from `/proc` before and after.
**LOADED**: load average 2.44 before and 5.43 after, other agents were
compiling; the clock tick is 10 ms, so one run resolves 25 µs.

| build | median CPU per idle redraw | eight-run spread |
|---|---:|---:|
| without `a11y` | 300.0 µs | 175.0 – 450.0 µs |
| with `a11y` | 487.5 µs | 350.0 – 850.0 µs |

The `a11y` median lies **outside** the other build's spread. **Fired.**

**This is the attached case, not the idle one the rule names.** The
development desktop runs the AT-SPI registry with accessibility enabled,
so the adapter is active from the first frame and every redraw builds
and sends the whole tree (incremental updates are deferred, the plan's
§4). With no assistive technology attached, `update_if_active` builds
nothing; that case was not available to measure here. The decision does
not depend on it: condition 2 fired on its own.

### What would reopen it

An incremental `TreeUpdate` (the plan's §4) addresses condition 1 when a
reader is attached; nothing in this workspace addresses condition 2,
which is the platform stack's size. A lighter bridge — `accesskit_unix`
without its async runtime — would be the thing to measure.
