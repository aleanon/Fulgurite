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

**Re-measured 2026-10-04 at `553fa5e`** (M80/T80.0) with K2's exact
command (`cargo tree -p lumen-static-winit -e normal --prefix none
[--features F] | sed 's/ (\*)$//' | sort -u | wc -l`,
`docs/plan-iced-roadmap.md` K2): default build **96**, `a11y` **167**
(**+71**); `--no-default-features` 91, so `clipboard` is **+5**; and
`dialogs` **+10**. The window crate's manifest cites these; its
earlier "11", "79" and "62" counted the whole workspace graph.

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

## T54.5–T54.6 — contrast as painted (R16-F8, R16-F9)

Measured by `lumen_static_shell::contrast::text_contrast`: every text
run in the display list against the topmost opaque fill under its centre
(clips respected; translucent, gradient, image and glass grounds counted
as unmeasured). WCAG 2.x relative luminance; bounds 4.5:1 for text and
3:1 for the non-text minimum.

### The values that changed (T54.6)

| palette / widget | role or field | was | ratio then | now | ratio now |
|---|---|---|---:|---|---:|
| `examples/showcase` `SHOW_LIGHT` | `accent` (and `focus`) | `#2f6bff` | 3.91 on the Ghost ground, 4.16 on the page | `#265ce6` | 4.86 / 5.17 |
| `Palette::LIGHT` | `accent` | (53, 99, 239) | 4.07 on `surface` | (49, 92, 222) `#315cde` | 4.58 |
| `Palette::LIGHT` | `muted` | (100, 116, 139) | 3.86 on `surface` | (90, 104, 125) | 4.59 (5.66 on `page`) |
| `AppBar` default | background | `#2f6bff` | 4.497 under white text | `#315cde` | 5.65 |
| `Pagination` default | colour | `#2f6bff` | 4.12 on its ground | `#315cde` | 5.18 |
| `DatePicker` weekday strip | dim alpha | 0x99 | 4.04 on white | 0xb4 | 5.63 |

`#265ce6` is the re-triage's search result; the core's accent and muted
are the first value down their own hue (scaled toward black in 1% steps)
that passes every ground the harness reported.

### What the catalogue measures now

| harness | palette | measured | runs under the bound |
|---|---|---:|---:|
| widgets' catalogue, `tests/text_contrast.rs` | defaults | 50 | 0 under 4.5 (min 4.77) |
| showcase, every entry | `SHOW_LIGHT` | 446 of 457 | 0 under 4.5 (was 40) |
| showcase, every entry | `Palette::LIGHT` | 446 of 457 | 0 under 4.5 (was 54, then 17 with the old accent) |
| showcase, every entry | `SHOW_DARK` | 446 of 457 | **120 under 3:1** — not fixed |
| showcase, every entry | `Palette::DARK` | 446 of 457 | **120 under 3:1** — not fixed |

**The dark palettes fail for a reason no palette value can fix**: the
widgets' defaults are light-theme values, not roles, so anything the
stylesheet does not re-colour paints near-black on the dark page
(`docs/plan-a11y-platform.md`, "What building found", T54.5).
