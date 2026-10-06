# M86 — widgets that move: built-in transitions for hover, press and focus

> **Written 2026-09-28 as a section of `docs/plan-iced-roadmap.md`;
> revised 2026-09-29 after that plan's seven-lens review, and moved to
> its own file. T86.0 and T86.1 landed 2026-10-06. K86a held after one respelling; **K86b fired after its written response (the blend `#[cold]`), and fires again re-measured on a quiet machine, so M86 stops at T86.1** (`docs/results-transitions.md` T86.1; "What building found" below).** The shared byte inventory, seams (S1,
> S2), test and doc discipline, and plan-wide kill rules K1–K4 are in
> `docs/plan-iced-roadmap.md` §2–§4 and bind this plan; this file does
> not restate them.

*Size M, six commits (T86.0–T86.5). Closes iced's "widget animations"
for the pointer and focus affordances of **table nodes**: the hover
and press shade that `ops::shaded` resolves, and the focus ring the
host draws. Row hover inside a `Repeat`, a `Chooser`'s option list,
`Select`, `Tree`, `DataGrid` and a `Strip`'s items stays instant, and
is deferred with its reason below. Depends on nothing in this plan.
It contends with M80 and M82 for the spare bytes of `Style` and
`NodeState`, and it adds no `Session` method, `Cmd` variant, `Event`
variant or `Painter` method. Read against `553fa5e`, the tip of
`lumen-static-merge`, which is `92a4033` plus the M61 merge. `.lss`
exists there, so the new property's declaration is unconditional.*

**Decisions.**

- **Where a transition starts: the host, not `record_painted`**
  (P9, A11, B21). `record_painted` (`ops.rs:360`) is a layout-time
  free function. It takes `(st, i, limits, size, style)`, has 59 call
  sites, and has no table and no clock. Under `Boundaries<256>` it
  does not run for a clean node at all. The bits flip earlier, in
  `pass::dispatch` (`pass.rs:1015-1026`, `set_hovered`,
  `release_press`). The one place that already sees every flip is
  `Host::pump_with`'s `PointerFocus` before/after comparison
  (`lumen-static-shell/src/lib.rs:1803`, M25/T25.0), and it already
  marks the old node and the new one. An entry starts there, after
  dispatch and before layout, when `self.styles[n].transition > 0`.
  The frame that flips the bit is drawn at the entry's start levels.
- **The clock: `Host::tick`, the path every animation already takes.**
  `Transitions<8>` is a field of `Host`, in `lumen-static-shell`. Each
  entry is `Transition { node: u32, from: u16, elapsed_ms: u16,
  dur_ms: u16 }`, 12 B, 96 B for the table, per host and not per node.
  - `tick(dt)` advances every entry. It computes the eased levels from
    `from` toward the node's current bits, writes them into
    `states[node].pointer_mix`, and marks the node's subtree with the
    existing `mark_node`.
  - When `elapsed_ms >= dur_ms`, `tick` writes `pointer_mix = 0`,
    marks once more, and retires the entry.
  - `tick(0.0)` stays a pure query (its contract, `lib.rs:2150`). It
    writes and marks nothing, and reports `!is_empty()`.
- **The frame request: `Host::tick`'s `bool`, so no `Session` change**
  (X3, G21). `Session::frame` returns the second `h.tick(0.0)`
  (`session.rs:606-625`). `ui.waitSettled`'s `animating` term is that
  same value (`session.rs:1008`). The window's redraw request reads it
  too (`lumen-static-winit/src/lib.rs:838`). A non-empty table ORs
  into the tick's return value, and all three follow from that one
  line. S1 is untouched: nothing crosses the `Hot` boundary that did
  not before. A swap builds a new `Host` (M37), so a transition in
  flight across a reload ends at its target. That is recorded, not
  gated.
- **How paint sees the level: `NodeState::pointer_mix: u16`, read by
  `shaded`** (C3, A11). Paint has no `&[Style]` and no host, so the
  level has to live in the state array.
  - Bit 15 means active. Bits 7–13 hold the hover level and bits 0–6
    the press level, each out of `MIX_LEVELS` = 127.
  - `0` means settled. `shaded` then takes today's bit path unchanged,
    so a zeroed `NodeState`, and every tree with `transition == 0`, is
    byte for byte today's frame. Nothing outside the host ever writes
    the field.
  - `shaded` (`ops.rs:409`) blends in `u8` integer arithmetic:
    `base = lerp(c, dim(c, hover_dim), h)`, then
    `lerp(base, dim(c, pressed_dim), p)` when `pressed_dim > 0`. At
    the four endpoint pairs this is exactly the bit path, including
    its "pressed with no `pressed_dim` shows the hover shade" case.
  - `record_painted` calls `shaded`, and so does `Toggle`'s layout
    (`toggle.rs:200`, M57/B11). Both are covered with no change at
    either call site.
  - `record_painted` never overwrites an entry's value. It derives
    `bg` from it.
- **Easing and interruption.** Fixed `Easing::EaseOut` (`anim.rs`),
  for both directions and for press. If a bit flips mid-entry, the
  entry restarts from the levels currently displayed, which are exact
  because they are stored, and runs the full duration. The easing is
  not a style field. Reopen when a sheet needs another easing: two
  bits of `transition` would halve the range to 630 ms.
- **The focus ring fades in; it does not fade out.** The host draws
  the ring (`paint_focus_ring`, `lib.rs:2601`), so its alpha is a host
  value and needs no `NodeState` byte. A focus entry scales
  `palette.focus`'s alpha. Both call sites pass it: the live paint
  (`lib.rs:2063`) and the shadow oracle's scratch paint (`lib.rs:533`).
  Missing either one is a divergence the oracle reports. There is one
  ring, and the old ring disappears at once. Two rings in flight would
  mean a second draw site, and nothing asks for one.
- **Damage** is the node's own rect for a shade: one changed `bg`
  command. For a ring it is the ring's outer rect (`FOCUS_GAP` + width
  beyond the node).
- **A full table** changes instantly, which is today's behaviour, and
  counts the drop in `Host::transitions_dropped()`. Eight entries
  cover the worst ordinary frame: hover out, hover in, press, release,
  and focus in.
- **Scope: table nodes only** (P10). `input.hovered` is a table index
  (`pass.rs:1015`). A row's hover is `hover_inner`, and moving between
  rows flips no bit. `choose.rs`, `data.rs` and `nav.rs` draw their hot
  row from widget-owned state (`nav.rs:84` `hot`). An entry keyed on
  `(node, inner)` would give those widgets a level they have no place
  to read. Each would need its own byte in its own state, and its own
  paint change.
  - What animates: `Button`, the three toggles, every widget whose
    shade goes through `record_painted`, and a chooser's **trigger**.
  - What stays instant: option rows, tree and grid rows, and strip
    items.
  - Reopen when a row widget's owner asks for it. The work is then that
    widget's, not the table's.
- **The byte budget** (C2, B2, M1, A10, B8). Measured at HEAD: `Style`
  is 80 B with 2 spare (`tests/cursor.rs:32`), and `NodeState` is 84 B
  with 2 spare (`tests/focus.rs:204`). M86 claims **1 B of `Style`**
  (`transition: u8`, tens of ms, 0–2550 ms) and **2 B of `NodeState`**
  (`pointer_mix`).
  - M86 depends on nothing (`docs/plan-iced-roadmap.md` §5, second
    review M16): §2.1's byte table is an order of assignment, so the
    arithmetic is conditional on what has landed.
    - If M80 and M82 have landed: M82 has grown `Style` to 84 B (81
      used), so `transition` takes byte 82 and `Style` does not grow;
      `NodeState` is 84/84, so `pointer_mix` grows it to 88. That is
      168 → 172 B/node, 16.4 → 16.8 KB per 100 nodes against 24 KB
      (§2.1 owns the figures).
    - If they have not: both fields take the next free bytes, and the
      before/after sizes and B/node are computed from the sizes T86.1
      measures at its parent, printed, not from §2.1's row. At HEAD
      that parent is `Style` 80 B and `NodeState` 84 B, each with 2
      spare; whether `pointer_mix` grows `NodeState` is what the
      print says.
    - Either way K86d's maxima (84/88/172) bound the result.
  - T86.1 prints both sizes and the per-100-node figure, whichever
    order held.
  - The sites that change together: `tests/cursor.rs:32-33`,
    `tests/focus.rs:203-204`, `fixtures/embedded/src/lib.rs:409`,
    `:415` and `:416` (`NODE_STATE`, `STYLE`, `PER_NODE_RAM`), and
    `docs/results-embedded.md`'s RAM row.
  - `Cmd` (60 B) and `Event` (16 B) are not per-node and do not move.
    T86.1 asserts that they did not.
- **The firmware: the blend sits behind an additive core feature,
  `transitions`** (B6). `lumen-static-shell` enables it and fixtures
  do not.
  - `firmware_interactive` links 61,922 B, 19,998 B under the 80 KB
    ceiling (`docs/results-embedded.md`, the M56-merge section; D5,
    2026-09-30), and M80–M85 also grow it: `Cmd::Path` and the
    widened gradient under D3, and the required `text_spaced` (M80)
    and `fills` (M82) under D6 (owner, 2026-10-02: accepted firmware
    growth).
  - No firmware writes `pointer_mix`, so the blend would be dead code
    it pays for anyway.
  - Without the feature, `shaded` ignores the field. That degraded
    body is the documented contract ("no transition", today's
    behaviour), not a provided `Painter` method, and nothing wraps
    `shaded`, so it is not the `Deferring` trap.
  - It is additive, like `a11y`, so §8's rejection of size-mode flags
    in `docs/00-handoff-static-tree.md` does not apply. It is not
    mutually exclusive, and a widget crate compiles the same either
    way.
  - The fields are unconditional, so the size assertions do not depend
    on the feature.
  - M86 adds no `Painter` method, so `Checksum`
    (`fixtures/embedded/src/lib.rs:247`) and the other wrappers are
    untouched.
- **Style and sheet surface.**
  - `Patch::transition_ms(ms: u16)` rounds to 10 ms and saturates at
    2550.
  - `Fields::TRANSITION` takes the next free bit at landing, printed
    (expected `1 << 20`, after M80's 17 and 18 and M82's 19;
    `Fields::EACH` 20 → 21). Every seed
    that declares `HOVER_DIM` or `PRESSED_DIM` adds it, listed from
    `grep -rlE 'HOVER_DIM|PRESSED_DIM' --include=*.rs` at landing and
    printed in the commit body. At `553fa5e` that is twelve sites:
    - the derive's container seed
      (`lumen-static-macros/src/lib.rs:1032`);
    - core `Repeat`'s seed (`lumen-static/src/repeat.rs:472-473`);
    - nine `lumen-static-widgets` files: `chart.rs`, `input.rs`,
      `ops.rs`, `pick.rs`, `scroll.rs`, `slider.rs`, `textarea.rs`,
      `toggle.rs`, `trim.rs`;
    - `examples/showcase/src/nav.rs:120-121`.
    (`style.rs`, which defines the bits, is not a seed.) Without that,
    `W1010` flags a `transition-duration` rule on a widget that
    honours it — a `Repeat` with hover shading among them.
  - `.lss` gets `transition-duration: 150ms` with a new `Takes::Millis`.
    A value that is not a multiple of 10, or is above 2550, is refused.
    `check_lss_setters.sh` forces the declaration.
- **Defaults** (X3). `Button` and `Toggle`'s `type_defaults` get
  `transition = 12` (120 ms), so every application animates its
  buttons and toggles. That is iced's shape: built-ins move. The
  cost falls on every live script that reads pixels after a hover or
  a press, and on every headless test that asserts a shade after one
  `pump()` (the frame that flips the bit is drawn at the start
  levels), and T86.4 carries both.

**Tasks.**

- **T86.0 — the stepped oracle, and the baselines.** Everything here
  compiles at HEAD and is green at HEAD. It is the harness T86.1–T86.3
  can break, so it has to be standing before them.
  - `crates/lumen-static-shell/tests/transition_oracle.rs`: a
    `Host<_, Incremental, Everything>` and a
    `Host<_, Incremental, Boundaries<256>>`, each on its own tree of a
    `Button`, a `Switch` and a `TextInput`.
    - The same script runs on both. Hover `#go`, press it, release it,
      hover the switch, then Tab to the input. Each step is followed by
      twelve frames of `tick(16.0)` and `pump()`.
    - Every frame, both `NodeState` arrays and both lists'
      `hash_from(0)` must be identical.
    - Setup landed: after each move, `host.input.hovered` or `pressed`
      or `focus` is the intended index, printed with the selector.
      After the Tab, `focus_visible` is `true`.
    - Accept: green at HEAD, with the frame count printed (5 × 13).
    - Injection: delete the `after.hovered != before.hovered` marks in
      `pump_with` (T25.0). It must go red under `Boundaries` on the
      hover step, on the field `bg`, by name.
  - Baselines, recorded in the new `docs/results-transitions.md` and
    not gated:
    - `scripts/check_text.sh`'s four firmware binaries, `.text` and
      `.rodata`;
    - `cargo run --release -p benches --bin appearance`, rows `still`
      and `hover`, eight runs a side under `just bench-lock`, spread
      printed;
    - `benches-competitive/run.sh -p lumen-static-frame`, row
      `idle (floor)`.
  - Docs, same commit: `docs/results-transitions.md` (baseline table;
    thresholds K86a–K86d copied in before anything is built), and its
    `scripts/doc-paths.allow` entry (M80/T80.0) removed.
- **T86.1 — the level in `NodeState`, and the blend in `shaded`.**
  - What changes:
    - `NodeState::pointer_mix`;
    - `Style::transition`, `Patch::transition_ms`,
      `Fields::TRANSITION`;
    - the `transitions` core feature, which the shell enables;
    - `shaded`'s blend, with the `ops::encode_mix` helper and the
      `ops::MIX_LEVELS` constant;
    - `.lss` `transition-duration`, at the four sites a property
      touches: a `PROPERTIES` row and a new `Takes::Millis` in
      `lumen-static-lss/src/lower.rs` (with a `describe` arm), a new
      `Val::Millis(u16)` variant that lowering produces (beside the
      `Takes::Byte` arm, `lower.rs:513`), the macro's `val` arm
      (`lumen-static-macros/src/lss.rs:223`), and the shell's
      run-time `set` arm `("transition_ms", Val::Millis(x))`
      (`lumen-static-shell/src/lss.rs:90`);
    - `ui.getAppliedStyles` (`lumen-static-agent/src/lib.rs:1519`,
      which lists its fields by hand) reports `transition` and
      `state.pointer_mix`, so T86.3's live setup can be asserted
      without reading pixels.
  - Nothing writes `pointer_mix` yet except tests, so there is no
    user-visible behaviour and no live drive. The live halves are
    T86.2's and T86.4's.
  - Docs, same commit:
    - rustdoc on every item above, plus `record_painted`'s paragraph on
      which fields it derives;
    - `book/src/styling.md:246`'s property list gains
      `transition-duration`;
    - `docs/results-embedded.md`: the RAM row and the four-binary
      table, attributed;
    - `docs/00-handoff-static-tree.md` §8 gets one line: `transitions`
      is additive, which is not the rejected size-mode flag.
  - Gates:
    - `tests/transition_shade.rs` (shell) covers the endpoints and the
      blend.
      - For 64 colours × `hover_dim` ∈ {0, 0.07, 0.12} ×
        `pressed_dim` ∈ {0, 0.15}, `shaded` at the four encoded endpoint
        pairs equals the bit path byte for byte. It prints
        `0 of 768 differ`.
      - At `h = 64/127` the channel lies strictly between rest and
        hover. It prints the value and both ends.
      - Interior: at `h = 64`, `p = 64` over the same 768 cases,
        `shaded` equals a rounding reference
        (`a + ((b-a)*t + 63.5)/127`, computed in `f32` in the test),
        bound 0 differing channels, count printed.
      - Setup landed: the test asserts that `pointer_mix` it wrote has
        bit 15 set.
      - Injection: `lerp` with truncating division (`(b-a)*t/127`)
        makes the interior check go red, by colour, count printed.
        The endpoint check cannot see it — truncation is exact at
        `t = 0` and `t = 127` — and stays green, which the commit
        body records.
      - Injection: `shaded` ignoring the press level makes the
        press-endpoint check go red.
    - Sizes: `tests/cursor.rs` and `tests/focus.rs` print
      `Style`/`NodeState` and B/node × 100. They assert the new sizes,
      and assert `Cmd == 60` and `Event == 16` unchanged.
    - Firmware Δ (K86a): K4's comparator, M80/T80.0's
      `scripts/firmware_delta.sh` (`just firmware-delta`), against the
      parent, all four binaries, before/after/Δ printed by the script.
      If T80.0 has not landed, T86.1 waits for that one task (its
      gates only, not M80's bytes); M86 adds no comparator.
      - Bound, per binary: `firmware` and `firmware_bind` (no
        `cascade_range` in `check_text.sh`'s `PRE_` lists) 0 B of
        `.text` if neither struct grew, ≤ 64 B if one did; that
        allowance covers copy and offset changes only.
        `firmware_cascade` and `firmware_interactive` link
        `cascade_range`, so `Patch::apply_to` gains one
        `if let Some(v) = self.transition` branch (`style.rs:955`)
        whatever the struct sizes do: those two get the same bound
        plus ≤ 32 B, attributed by symbol diff to
        `apply_to`/`cascade_range` only.
      - Invocation, written out (T80.0's positional `BIN=B` form),
        if a struct grew: `just firmware-delta <parent> firmware=64
        firmware_bind=64 firmware_cascade=96
        firmware_interactive=96`; if neither grew: `firmware=0
        firmware_bind=0 firmware_cascade=32
        firmware_interactive=32`.
      - Injection: add `features = ["transitions"]` to
        `fixtures/embedded`'s `lumen-static` line. The blend links and
        the Δ exceeds the bound, printed.
      - Accept: removing the injection restores the bound's figure,
        printed per binary.
    - `.lss`: the `lumen-static-lss` unit tests.
      - `transition-duration: 150ms` lowers to `transition_ms(150)`.
      - `155ms` and `2560ms` are refused with the `Takes::Millis`
        description.
      - Accept: `0ms` and `2550ms` are accepted.
      - `check_lss_setters.sh` is shown red with the property line
        removed.
      - Parity: `tests/lss_parity.rs` (shell) gains a sheet setting
        `transition-duration: 150ms`, so `include_lss!` and
        `lss::load` are compared on it. Injection: drop the shell's
        `set` arm; `load` panics on the sheet, by setter name.
    - Lint: a `justfile` line `cargo clippy -p lumen-static
      --features transitions --all-targets -- -D warnings` and
      `RUSTDOCFLAGS="-D warnings" cargo doc -p lumen-static --no-deps
      --features transitions` (§3.1, second review X7). The workspace
      line already unifies the feature through the shell; this line
      keeps it linted if that ever changes. Injection: an unused
      binding inside the blend fails the line, by name.
    - `W1010`: a sheet setting `transition-duration` on a `Button` is
      silent and on a `Label` is flagged.
      - Injection: `Fields::TRANSITION` missing from `Button`'s seed
        makes the first case go red.
    - The goldens (`tests/golden.rs`) are unchanged: nothing is set
      anywhere yet.
- **T86.2 — `Transitions<8>` in the host.**
  - What changes:
    - the table;
    - its start in `pump_with`, where the T25.0 marks are;
    - its advance, write, mark and retire in `Host::tick`;
    - the OR into `tick`'s return;
    - `Host::transitions_dropped()`.
  - Docs, same commit:
    - rustdoc on `Host::tick`, `pump_with`'s T25.0 block and
      `Session::frame` ("whether to draw again" now includes a running
      transition);
    - `book/src/animation.md` "Why there is no animation pass" is
      amended. There *is* now a registry, bounded at eight and only in
      the host, and a tree that sets no `transition` pays one
      `is_empty` per tick. The new "Built-in transitions" section
      includes `transition_clock.rs`'s setup by the named anchor
      `transition_setup`;
    - `docs/00-handoff-static-tree.md` §2 gets a DECIDED (M86/T86.2)
      marker pointing at `docs/results-transitions.md`;
    - that doc gets the K86c row.
  - Gates:
    - `tests/transition_clock.rs`, shade at t (P11's headless half).
      - A `Button` with `transition_ms(150)` and `hover_dim(0.12)`.
        After hovering it, `tick(50)` and `pump()`, then
        `tick(50)` and `pump()`.
      - `states[go].bg` at 0, 50, 100 and 150 ms, each printed beside
        `lerp(rest, hover, EaseOut(t/150))` ± 2/255. At 150 ms it is
        exactly the hover shade and `pointer_mix == 0`.
      - Setup landed: `host.input.hovered == Some(go)`.
      - Accept: the same with `transition_ms(0)` gives the hover shade
        at 0 ms and no entry.
      - Injection: the advance writes the target level without easing
        (ignores `elapsed_ms`). The 50 ms shade then equals the final
        one and the check goes red.
    - The frame request (G21, X3). Same file, through a `Local`
      session as `tests/agent_settled.rs` builds one.
      - `Session::frame(16.0, ..)` returns `true` on every frame until
        the entry retires, then `false`. It prints the count of `true`
        frames against `ceil(150/16) = 10` ± 1.
      - `settled` is `false` at 50 ms and `true` at 176 ms, both
        printed.
      - Injection: `Host::tick` returns its old value, ignoring the
        table. The count is 0 against a bound of 10, red, and
        `settled` is `true` at 50 ms.
      - This injection is observable only because the gate reads the
        request itself. A test that pumps its own frames would stay
        green (P11), so the shade gate above does not use it.
    - The per-frame mark (B21, A11): `transition_oracle.rs` gains the
      same script with `transition_ms(150)` on all three nodes.
      - It prints the count of frames with `pointer_mix != 0`, which
        must be ≥ 8.
      - Injection: drop the `mark_node` in the tick's advance. It must
        go red under `Boundaries` on `bg` at the first intermediate
        frame, while `Everything` still animates.
    - Over-marking: the tree gains a sibling leaf beside `#go` that
      counts its `layout` calls. On each intermediate frame under
      `Boundaries<256>` its count is printed, bound 0.
      - Injection: mark the parent's subtree instead of the node's.
        The sibling re-lays out every frame, count printed > 0, red.
      - Damage is not this gate: `Host::damage` is
        `retain.finish()` (`lib.rs:2093`, `:2453`), which folds each
        command's content (`retain.rs:940-948`), so re-laid-out
        siblings with unchanged commands add no damage and the
        injection could not move it. `host.damage()` area ÷ the
        button's rect is printed per frame as a record (expected
        1.00), not asserted against this injection.
    - Overflow: nine nodes flipped in one frame. Eight entries run,
      `transitions_dropped() == 1`, and the ninth node shows its final
      shade at once.
      - Injection: drop the overflow branch's instant write. The ninth
        stays at its old shade and goes red.
    - Live: `just widget-drive Button` under both strategies and the
      `Boundaries+oracle` leg.
      - `scripts/live/shadow.py` must report `compared > 0` and
        `first is None` across a hover transition.
      - The live half needs a transition on a showcase node, and the
        defaults land in T86.4. This task's live check therefore runs
        after it sets `transition-duration: 120ms` on `#d_button_a` in
        `examples/showcase/showcase.lss`.
      - Injection: dropping the per-frame mark makes `app.shadow` name
        a `colour` divergence.
- **T86.3 — the focus ring fades in.**
  - What changes: a focus entry kind, the ring alpha passed at both
    `paint_focus_ring` call sites, the focused node marked per frame,
    and `transition-duration: 150ms` on `#d_textinput_a` in
    `examples/showcase/showcase.lss` (line 47 at `553fa5e`). T86.4's
    defaults cover `Button` and `Toggle` only, so without this line
    the showcase's `TextInput` has `transition == 0`, the ring is at
    full alpha on the Tab frame at both sites, and the live injection
    below cannot diverge. The line stays after T86.4.
  - Docs, same commit: rustdoc on `ops::focus_ring` and
    `paint_focus_ring` ("fades in over the focused node's
    `transition`; never out"), and a `docs/results-transitions.md`
    row.
  - Gates (`tests/transition_ring.rs`):
    - Setup: Tab to a `TextInput` with `transition_ms(150)`. Setup
      landed: `input.focus == Some(field)` and `focus_visible`.
    - The ring command's alpha at 0, 50 and 150 ms is printed against
      `255 · EaseOut(t/150)` ± 2. At 150 ms it is exactly
      `palette.focus`.
    - Damage per frame is the ring's outer rect, ratio 1.00.
    - Accept: with `transition_ms(0)` the ring is at full alpha on the
      Tab frame.
    - Injection: pass the alpha at the live site (`lib.rs:2063`)
      only. The two strategies still agree, so `transition_oracle.rs`
      stays green. That is expected: the oracle it misses is the
      shadow's. The `Boundaries+oracle` leg of `just widget-drive
      TextInput` must report `app.shadow` `first` on the Tab frame,
      field `paint`, printed.
    - Injection: no per-frame mark for the focus entry. The oracle goes
      red under `Boundaries`.
    - Live: `just widget-drive TextInput` under both strategies. The
      keyboard pass (M57/D3) Tabs to it, and the `Boundaries+oracle`
      leg reports `first is None`.
      - Setup landed: `ui.getAppliedStyles` on `#d_textinput_a`
        reports `transition` 15 (150 ms), printed, before the Tab;
        the drive refuses to continue on 0.
- **T86.4 — the defaults, and the live scripts that read pixels.**
  - What changes:
    - `transition = 12` in `Button` and `Toggle`'s `type_defaults`;
    - `scripts/drive/harness.py` waits `ui.waitSettled` (timeout
      1000 ms) after `pre_hover` and before `pix_before`, and again
      after `a.do` before (2) and (2b);
    - `scripts/drive/parity.py` (M57's D5) waits before its live read;
    - `scripts/motion_present.py` leg 2 counts a transition's frames
      instead of one;
    - `scripts/tasks_drive.py` is re-run, not amended: M88/T88.3
      reads its presents bound between a P0 taken after
      `ui.waitSettled` and a P1 after the timer ticks, so a 120 ms
      `Button` transition settles before anything is counted (M88
      second review X6). If M88 has not landed, nothing to do;
    - `examples/showcase/showcase.lss` drops T86.2's explicit
      `#d_button_a` line, because the default now carries it;
    - every headless test that asserts a `Button` or `Toggle` shade,
      or a damage set, after a pointer event and one `pump()`.
      `Host::pump` never advances the table (only `tick(dt)` writes
      `pointer_mix`), so at the defaults such a test sees the start
      level for good. Found by grep for `Button`/`Toggle`/`Switch`
      with `hovered`/`pressed`/`.bg`/`PointerMove` under
      `crates/*/tests`, the list printed in the commit body. At
      `553fa5e` it includes at least
      `lumen-static-shell/tests/pointer_invalidation.rs:97-116`
      (hover, `assert_eq!(painted, expected)`) and `:146-163`
      (press), and `damage_scale.rs:249-256` (the hover step, which
      asserts at `:206-209` that it damages a set of regions; a
      start-level frame changes no pixel and is `Damage::None`).
      Each pins `transition_ms(0)` in its sheet with a comment that
      it tests the instant path, or ticks to settle. `just test` is
      run red at the defaults commit before the fix and the failing
      count recorded.
  - Docs, same commit:
    - `book/src/styling.md`: buttons and toggles move by default, and
      how to turn it off (`transition-duration: 0ms`);
    - `.claude/skills/verifying-apps/SKILL.md`: a pixel read after a
      hover or press waits for `ui.waitSettled`;
    - the `docs/results-transitions.md` K86c row.
  - No widget is exported, so there are no drive-inventory, observe or
    a11y obligations. None is owed.
  - Gates:
    - M57's D1 settle (X3): the new plant
      `scripts/selftest/plants/live/transition-settle.sh`, in the shape
      of `hover-noop.sh`.
      - It removes the Button's press shade (`pressed_dim(0.0)`) with
        transitions on, and removes the harness's settle wait.
      - Without the wait, the "a click paints" action passes on a
        hover fade still in flight. That is the vacuous shape.
      - With the wait, it fails by name.
      - `just selftest-live` runs it, and both directions are recorded
        in the commit body.
    - `just widget-drive` in full, under both strategies and the oracle
      leg. Every action is green, and M57's D1 pixel rects are read after
      settle.
      - It prints, per action, the frames `ui.waitSettled` waited. At
        least one Button action must print > 1, so the gate cannot pass
        on a build where nothing animates.
    - `just motion-present`, leg 2: crossing `#d_button_a`'s edge
      presents N frames.
      - Bound: 2 ≤ N ≤ 1 + ceil(120 / 8.3), printed with the window's
        measured frame interval.
      - Over the following 500 ms there are 0 presents.
      - The button's centre pixel after settle equals the hover shade
        ± 2/255.
      - Leg 1's 200-motions-over-a-label bound (≤ 2) is unchanged. A
        label has no transition, which is the accept direction.
      - Injection: `Host::tick` ignores the table. N = 1, and the centre
        pixel is the rest shade. Red on both.
    - M57's D5 parity (`just live`) green under both strategies after the
      settle.
      - Injection: remove the settle from `parity.py`. The Button row
        must differ, printed.
    - The goldens are unchanged. None hovers or presses, and
      `tests/golden.rs` is green with the pinned files untouched.
- **T86.5 — close.** The `docs/results-transitions.md` summary, and
  the AGENT.md queue entry citing `just ci-verify <sha>` green (S5).
  No behaviour and no other docs, since each task carried its own
  (M3).

**Kill rules.**

- **K86a — the firmware does not pay for the blend.** This is
  T86.1's firmware Δ, measured by K4's comparator against the parent,
  all four binaries, with the per-binary bounds T86.1 writes out
  (`just firmware-delta <parent> firmware=64 firmware_bind=64
  firmware_cascade=96 firmware_interactive=96` if a struct grew;
  0/0/32/32 if neither did). It fires if `.text`
  exceeds T86.1's per-binary bound on any of them (≤ 64 B of struct
  Δ; plus ≤ 32 B for `Patch::apply_to`'s branch on
  `firmware_cascade` and `firmware_interactive`), or if
  `firmware_interactive` crosses 81,920 (the 80 KB ceiling, D5). When
  it fires, the task stops
  and finds what linked, with a symbol diff as the M56 merge did. The
  feature is not made default.
- **K86b — unused costs nothing.** Every tree has
  `transition == 0`, but the shell enables `transitions`. Measured by
  `benches --bin appearance`'s `still` and `hover` rows and
  `benches-competitive/run.sh -p lumen-static-frame`'s
  `idle (floor)`, eight runs a side under `just bench-lock`. It fires
  if a row leaves its T86.0 spread. When it fires, the blend moves out
  of line (`#[cold]`) behind the `pointer_mix != 0` test and is
  re-measured. If the row still fires, M86 stops at T86.1 and the
  number is recorded.
- **K86c — an active transition costs what a hover costs.** Measured
  by the new `transition` case in `benches --bin appearance`, where
  the two toggles carry an advancing `pointer_mix`. It fires if
  `transition` exceeds 1.10 × `hover`. When it fires, T86.4 does not
  ship: `type_defaults` stay at 0, transitions remain opt-in per
  sheet, and the number is recorded.
- **K86d — the per-node budget.** Measured by T86.1's size prints,
  against K1's exact post-M86 sizes (`docs/plan-iced-roadmap.md`
  §2.1): it fires if, after T86.1, `Style` exceeds 84 B, `NodeState`
  exceeds 88 B or `PER_NODE_RAM` exceeds 172 — K1's end-of-plan
  maxima, which hold in either landing order. (The first draft's
  "more than 8 B" bound equalled the worst case of 1 B + 2 B each
  padded to 4, and 172 × 100 = 16.8 KB sits far under 24 KB, so it
  could not fire.) When it fires, `pointer_mix` becomes a `u8` with
  4-bit levels, and 16 steps exceed a 266 ms transition's frames at
  60 Hz. The quantisation is printed.
- **K86e — the live scripts can settle.** Measured by T86.4's full
  `just widget-drive`. It fires if any action's `ui.waitSettled` times
  out at 1000 ms with no transition running. That is a settle defect,
  not a transition one, and it blocks T86.4 until it is fixed.

**What building found (M86).**

- **T86.1 — `pointer_mix` is a `u32`, not a `u16`.** The `u16` fired
  K86a: +140 / +164 / +148 / +144 B against bounds of 64 / 96 / 64 / 96,
  +142 of it in the firmware's `_start`. The struct was 88 B either way,
  but the `u16` left two bytes of padding, and the fixture's
  `[NodeState::default(); N]` became a clear per element and a store per
  field instead of one clear. As a `u32` (bits 16–31 always 0) there is
  no padding and the Δ is +40 / +60 / +36 / +44, attributed to copies and
  offsets, with `cascade_range` +2 for `Patch::apply_to`'s branch. The
  encoding (bit 15 active, 7–13 hover, 0–6 press) is unchanged;
  `encode_mix` and `MIX_LEVELS` are `u32` to match.
- **T86.1 — the `W1010` negative case is a `Fixed`, not a `Label`.** The
  plan's own seed rule (every seed declaring `HOVER_DIM` or `PRESSED_DIM`
  adds `TRANSITION`) gives `Label` the bit: it declares `HOVER_DIM`,
  shades through `record_painted`, and its hovered bit flips like any
  table node's. So a transition on a `Label` is honoured and correctly
  not flagged; `Kind("fixed")`, which declares no shade, is.
- **T86.1 — the seed list is 17 sites in 15 files**, not twelve:
  `selectable.rs`, `rich.rs` and `vector.rs` arrived after `553fa5e`, and
  `chart.rs` and the widgets' `ops.rs` carry two seeds each.
- **T86.1 — the shade gate's counts.** 64 colours × 3 × 2 is 384 cases,
  not 768: the endpoint check compares 1,536 (384 × 4 level pairs), the
  interior 1,152 channels. Printed as built.
- **T86.1 — `.lss` needed a unit.** The lexer took `px` or a bare number;
  `150ms` was a bad unit. `Unit::Ms` is lexed, printed and refused by
  every other `Takes`, and the bad-unit message now names `ms`
  (`fail_bad_unit.stderr` follows).
- **T86.1 — the lint line's injection.** §3.1 says the workspace clippy
  line stays green under an unused binding in the feature's code; for
  `transitions` it does not, because the shell unifies the feature into
  every workspace build. The default-feature `cargo clippy -p
  lumen-static` is the line that stays green, and was the one checked.
- **K86b fired, after its response, on `idle (floor)`.** Inlined, the
  blend put `appearance`'s `still` above its spread (18,664 against
  14,779–17,524 ns). `#[cold] #[inline(never)]` brought `still` and
  `hover` inside, but two of three re-measured frame-arm runs read
  `idle (floor)` above the base's (64.0 against 55.8–62.5 µs; 63.85
  against 53.3–62.4), and a third, in reversed order, inside (61.45). The
  shell without `transitions` reads on the base, so it is the feature,
  and `idle` lays nothing out, so the likely cause is code placement
  rather than the branch running. By the rule's written response M86
  stops at T86.1; T86.1 stands, since it adds no behaviour. Every figure
  is `docs/results-transitions.md` T86.1's.
- **K86b re-measured on a quiet machine: it still fires.** The landing
  measurement ran beside a `just ci`, so it was ruled not quiet and taken
  again (load1 ≤ 1.07, two locks, base `db138c0` against `e77b7b2` and
  against `1148444` separately, since T85.1's stack fix touches derived
  layout). Each lock has a row above its base spread on both trees — lock
  1 `idle (floor)` 64.00 / 63.40 µs against 55.8–61.9, lock 2 `still`
  18,050 / 18,148 ns against 14,800–17,461 — so the stop stands, and the
  stack fix is not the cause. The appearance rows are bimodal and both
  modes move up, which is a cost, not a draw.

**What the review found (M86).**

- **C2** — adopted. The Decisions carry a byte inventory: 1 B of
  `Style` and 2 B of `NodeState`: after M82, `Style` is already 84 and
  `transition` takes byte 82; `NodeState` goes 84 → 88. The
  per-100-node figure is printed in T86.1.
- **C3** — adopted. The level lives in `NodeState::pointer_mix`, which
  the host writes each frame and `shaded` reads in layout.
  `record_painted` draws nothing and starts nothing. `NodeState` is in
  T86.1's size gate.
- **G21** — adopted, changed for step equality. The frame request is
  gated on `Session::frame`'s own return, through `Local`, and on
  `app.presents` live. Boundaries/Everything equality is a headless
  stepped oracle in one process with identical `dt`, plus live
  `app.shadow` on widget-drive's oracle leg. It never compares two
  windows.
- **B2** — adopted. The inventory states the cumulative result and
  names the embedded asserts at their HEAD lines
  (`fixtures/embedded/src/lib.rs:409/415/416`; the finding's 403-404
  moved at the M56 merge).
- **B6** — adopted for M86. The measured four-binary Δ is K86a. The
  blend is behind an additive `transitions` feature. `Checksum` is
  unaffected because M86 adds no `Painter` method.
- **B8** — adopted. Sizes are exact and printed per struct, and
  RAM/100 nodes is counted only for `Style` + `NodeState`. `Cmd` and
  `Event` are asserted unchanged rather than judged per node. K86d's
  8 B bound could not fire after all; the second review's B13
  replaced it with K1's exact maxima.
- **B21** — adopted. Entries start in `pump_with`, and `tick` writes,
  marks and damages the node's rect. `record_painted` derives from the
  level and never overwrites it. The active-transition cost is K86c,
  and the firmware clause is measured (K86a).
- **A10** — adopted. There is no "padding" claim, and the firmware gate
  is "0 B, or ≤ 64 B of struct Δ, printed", plus ≤ 32 B for
  `Patch::apply_to`'s branch on the two binaries that link the
  cascade (second review, B13).
- **A11** — adopted: start in the host, mark every frame, and read
  through `shaded` so toggles are covered. Row keys: rejected as out of
  scope (see P10).
- **P9** — adopted. The start hook is the host's existing
  `PointerFocus` comparison, and a `Boundaries` gate runs with no
  further input (`transition_oracle.rs`, T86.2).
- **P10** — changed. Scoped to table nodes. The chooser *trigger*
  animates and its rows, `Tree`/`DataGrid` rows and `Strip` items do
  not, with the reason and a reopen condition. `(node, inner)` keys
  were rejected because those widgets draw their hot row from their own
  state, which a host entry cannot reach.
- **P11** — adopted. It is split into a headless shade-at-t gate
  (injection: ignore elapsed time) and a frame-request gate that reads
  the request itself, plus live `app.presents`.
- **M1** — adopted. 80/84 are baselines, not invariants, and no gate
  says "stays 80".
- **M3** — adopted. There is no docs task: every task carries its
  rustdoc, book, handoff and results rows, and T86.5 is the summary and
  the ci-verify queue entry only.
- **M20** — adopted. Every gate in T86.0–T86.4 names a mechanism
  injection and an accept run.
- **X3** — adopted. The table ORs into `Host::tick`'s return, which is
  `Session::frame`'s value and `waitSettled`'s `animating`, and the
  frame-request gate shows it. The defaults are `type_defaults`
  (Button, toggles). T86.4 makes the harness, parity and motion-present
  wait for settle before a pixel read. A live plant proves the wait is
  needed. The headless tests the defaults break are T86.4's too
  (second review, X1).

**What the second review found (M86).**

- **C12** — adopted. `Fields::TRANSITION`'s seed list is printed from a
  grep at landing; at `553fa5e` it is the derive, core `Repeat`
  (`repeat.rs:472-473`), nine widget files (not ten) and
  `examples/showcase/src/nav.rs:120-121`.
- **G14** — adopted. An interior gate at `h = p = 64` against a
  rounding reference (bound 0 of 768, printed) is what the truncating
  `lerp` injection turns red; the endpoint check is exact under
  truncation and is recorded as unable to see it.
- **G15** — adopted. The over-marking injection is observed by a
  sibling leaf's layout count (bound 0); `host.damage()` stays a
  printed record, since retention folds command content and could
  not move.
- **G16** — adopted. T86.3 sets `transition-duration: 150ms` on
  `#d_textinput_a` and asserts it through `ui.getAppliedStyles`, which
  T86.1 extends to report `transition` and `pointer_mix`.
- **B13** — adopted. T86.1's firmware bound is per binary, with ≤ 32 B
  for `Patch::apply_to`'s branch on `firmware_cascade` and
  `firmware_interactive`; K86d is K1's end-of-plan maxima (84/88/172)
  instead of an 8 B bound equal to the worst case.
- **X1** — adopted. T86.4 names the headless tests that assert a shade
  or damage after one `pump()` (`pointer_invalidation.rs` hover and
  press, `damage_scale.rs`'s hover step, and the rest of a printed
  grep); each pins `transition_ms(0)` or ticks, and `just test`'s red
  count is recorded.
- **D6** — applied. The firmware paragraph attributes `text_spaced`
  and `fills` to D6 (owner, 2026-10-02), not D3.
- **D7, X5** — no change: M86 needs no `xclip` and links no
  `rustybuzz`.
- **B4, M16, M7, X7** (from `plan-iced-roadmap.md`) → adopted. K86a
  and T86.1 measure through T80.0's `firmware_delta.sh`, invocation
  written per binary in T80.0's positional `BIN=B` form (the
  request's `--bound 64` is not that script's interface); the byte
  inventory is conditional, since §5 orders M86 after nothing;
  T86.1 names the `PROPERTIES` row, `Takes::Millis`,
  `Val::Millis(u16)`, the macro's `val` arm, the shell's `set` arm
  and an `lss_parity.rs` sheet; a clippy and doc line for
  `transitions` is added.
- **R10** (from `plan-shaped-paint.md`) → adopted, as above: the hand
  `check_text.sh` comparison is gone from T86.1 and K86a; bounds
  kept (0/64 B, plus 32 B on the two cascade binaries).
- **X6** (from `plan-platform-requests.md`) → adopted. T86.4 re-runs
  `scripts/tasks_drive.py` and does not amend it: T88.3's P0 is read
  after `ui.waitSettled`.
- consistency-3 item 19 → M57's gates read "M57's D1/D5" (four sites),
  not the owner labels.
- consistency-3 item 24 → T86.0 removes `docs/results-transitions.md`'s
  `doc-paths.allow` entry.
