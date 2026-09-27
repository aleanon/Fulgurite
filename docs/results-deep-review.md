# Results: M38 — a deep review of the four goals

*M38, `docs/plan-deep-review.md` (R1–R18). Written 2026-09-24, reviewed
against `ac91d51`. Owns every figure the review produced; the follow-up
milestones cite this document. Machine: i9-13900KF (32 threads), X11 on
`:0`. The RTX 4070 was unusable for the whole review (kernel module
595.84 against NVML 595.91, "Driver/library version mismatch"), so every
wgpu figure here is **llvmpipe** unless it says otherwise. Load average
was 3–38 throughout, because eighteen reviewers shared the machine: every
timing is marked **LOADED**, and no finding rests on a timing alone —
each also rests on a count, a byte size or a pixel difference.*

*Nothing in this milestone was committed to `main`. Every probe,
injection and prototype fix lives in a scratch commit, cited by SHA; the
scratch worktrees were removed and the commits remain reachable in the
object store.*

**The stale lock.** R6's `cargo tree` rewrote
`benches-competitive/Cargo.lock` (+12 lines, adding
`lumen-static-widgets`), because the committed lock has been stale since
M30 (R6-F6, R17-F12). The rewrite was reverted before this document was
committed. Any `cargo` run in that workspace repeats it until R17-F12's
fix lands.

**The two S0s were each reproduced a third time**, outside the workflow,
before this document was committed. For R14-F1: counter-window on a
private port was sent the bytes a cross-origin `fetch(…, {mode:
'no-cors'})` POST puts on the wire (`Origin: http://evil.example`,
`Content-Type: text/plain`). Six header lines were each answered with
`-32700`, and the body line answered `"handled":true`. The tally read
**0 before and 10 after**.

## 0. What survived, in numbers

182 findings survived verification: none drew a REFUTED, and each was
either CONFIRMED or reproduced as PLAUSIBLE. **None was refuted** (§9). 164 were CONFIRMED and 18 are
PLAUSIBLE, meaning reproduced with the severity or the attribution
disputed. Severities are the verifiers' adjusted values where they agreed
on one. Where two verifiers disagreed (R1-F4: S1 against S2), the
author's severity stands and the dispute is noted beside the finding.

| severity | count | meaning (§2) |
|---|---|---|
| **S0** | **2** | R1-F1 (stale pixels at any scale ≠ 1.0 in the default configuration), R14-F1 (a browser page can drive the agent) |
| **S1** | **16** | R1-F2, R1-F3, R1-F4, R1-F5, R1-F6, R2-F1, R3-F1, R4-F1, R7-F1, R7-F2, R8-F1, R8-F4, R10-F1, R10-F2, R10-F3, R13-F1 |
| **S2** | **77** | §2.3 |
| **S3** | **66** | §2.4 |
| **S4** | **21** | §2.4 |

PLAUSIBLE, with the dispute recorded on each: R1-F7, R2-F4, R3-F8,
R3-F12, R4-F2, R5-F1, R6-F5, R8-F2, R8-F6, R8-F10, R9-F1, R9-F2, R9-F5,
R12-F3, R12-F4, R16-F5, R16-F7, R18-F6.

By category, the review's central hypothesis (the two vacuous shapes in
AGENT.md) held up: §4 lists **74 vacuous tests or gates**, each with the
injection, printed quantity or fixture question that exposed it.

## 1. Verdicts

Each verdict cites finding IDs only. "Held" rows cite the claims-ledger
entries in §3 that were tested and survived.

### Per goal

| goal | verdict | against | held (§3) |
|---|---|---|---|
| **(1) peak performance and resource efficiency** | **partly met** | R1-F1 (S0), R8-F1, R7-F1, R6-F1, R6-F2, R6-F3, R7-F5, R7-F6, R8-F3, R8-F5, R16-F7, R9-F1, R9-F6, R9-F8, R18-F4, R14-F4, R6-F5, R8-F2, R8-F6 | R6 ledger: "9× iced" (8.9× at the matched stopping point), banded 4K, thread wake cost; R7 ledger: a still UI idles at 0 frames / 0 CPU; R8 ledger: idle pump allocates nothing on FontMetrics, Shadow bounded by damage; R9 ledger: +18% glass material reproduces, GPU culls per region |
| **(2) configurability — only what is used is compiled in** | **partly met** | R12-F1, R12-F2, R12-F4, R12-F3, R12-F5, R6-F5, R5-F8, R14-F8, R11-F4, R3-F8, R13-F7 | R12 ledger: depth-2 feature powerset 70/70 clean, paint-only firmware links no `on_event`, `Button<A>`/`Button<B>` share code; R5 ledger: reload off links 0 serde_json symbols; R14 ledger: non-agent release links 0 agent symbols |
| **(3) a built-in agent for observability and control** | **not met** | R14-F1 (S0), R14-F2, R14-F3, R13-F1, R13-F2, R13-F3, R13-F4, R13-F5, R13-F6, R13-F8, R1-F5, R15-F10, R18-F1, R18-F2, R3-F2, R14-F4, R14-F5 | R13 ledger: a layout right after a click sees the result, `handled` is `pump_with`'s value, 0/20 redraw races; R14 ledger: 1,250 hostile requests, no panic, idle cost 0 |
| **(4) hot reload that cuts iteration time** | **not met** | R4-F1, R4-F2, R4-F3, R5-F2, R5-F1, R4-F5, R4-F6, R5-F3, R5-F4, R5-F5 | R4 ledger: the swap itself is 60–187 µs, per-field fallback logs; R5 ledger: adopt under Boundaries matches Everything (0 px), the inbox crosses adopt, E0119 refuses Default |

### Per platform

| platform | verdict | against | held (§3) |
|---|---|---|---|
| **embedded** (thumbv7em) | **partly met** | R11-F1, R11-F2, R11-F3, R11-F4, R11-F5, R12-F2, R12-F4, R17-F13, R2-F9 | R11 ledger: .text 27,024 B, framework .bss zero, RAM 164 B/node gated, per-level stack under 272 B in release; R12 ledger: paint-only firmware pays nothing for event methods; R15 ledger: a named LongPress costs +32 B |
| **desktop** (X11/Wayland/Windows/macOS via winit) | **partly met** | R1-F1 (S0), R14-F1 (S0), R10-F1, R10-F2, R10-F3, R3-F1, R7-F1, R8-F4, R15-F1, R15-F2, R3-F6, R3-F7, R9-F6, R10-F12, R10-F13 | R7 ledger: resize storm, 1×1, unmap/remap and close-with-job all behave on both backends; R15 ledger: focus follows Tab through AT-SPI (counter-window only), HiDPI canvas = inner_size at startup |
| **mobile** (Android/iOS) | **not met** | R15-F4, R15-F6, R12-F6, R1-F1, R9-F5, R15-F8 | R12 ledger: core, widgets and shell check for aarch64-linux-android; winit checks with `android-native-activity`. iOS is not executable (§6) |

## 2. The surviving findings

Ranked by severity, then by task. S0–S2 findings carry the full §2
record. S3 and S4 findings are one row each (§2.4). "Repro" names the
scratch commit at `ac91d51` that holds the probe. "Confidence" is high
where two verifiers confirmed, or one confirmed by a method different
from the author's; medium where the verdict is PLAUSIBLE.

### 2.1 S0

#### R1-F1 — Damage is recorded in logical pixels and applied to a physical surface, so at any scale other than 1.0 a partial frame leaves stale pixels in the default configuration

- **Severity** S0 · **Category** correctness · **Goal** (1) HiDPI desktop and mobile; (3) `ui.screenshot` shows the stale canvas · **Verdict** CONFIRMED ×2 (S0, S0)
- **Claim:** shell `pump_with`: "Logical geometry in, physical commands out (M10b), with retention observing the stream on the way past (T13.8)". `Host::take_damage_for_age`: "What a surface buffer `age` frames old needs redrawn". `surface.rs`: "Render only what changed".
- **Premise:** held. `Recording` sits inside `Scaled`, so damage is logical; both window backends apply it to a physical viewport.
- **Evidence:** Headless, `Host<_, Incremental, Everything>` against `Rebuild`, hover over `inc` (logical rect [42,20,78,54]). Scale 1.0: 0 / 0 px. Scale 1.5: damage still [[42,20,78,54]] while the physical rect is [63,30,117,81]; **2073 / 0 px differ**, differing bbox exactly [63,30,117,81]. Scale 2.0: 4272 / 0 px, bbox [84,40,156,108]. After `adopt`: 14,691 px (41,159 on the large canvas), persisting through the idle frame. **Live**, showcase `--features agent` (default Everything + Incremental, software), `WINIT_X11_SCALE_FACTOR=1.5`, canvas 1440×1008, port 7601 owned by pid 2328203: after `app.setPalette dark`, the logical-sized top-left [0,0,960,672] changed 160,333 / 161,280 sampled px; the right strip changed **0 / 120,960** and the bottom strip 0 / 80,640. Control at 1.0: every region changed. Screenshot: `scratchpad/live_palette_after.png`.
- **Injection:** the harness switch R1_SCALEDMG (regions × `host.scale()`) turns the damage-only arm green on 4 variants × 33 frames, which completes the attribution.
- **Repro:** scratch `2feb332`, test `r1_diag_damage_at_scale`.
- **Scope:** every turnkey window at a scale factor other than 1.0: HiDPI desktop, and every mobile device. GPU is expected to match by reading (`gpu.rs:83` passes the same logical regions); not executed (§6).
- **Fix and owner:** record damage in physical space (wrap `Scaled` outside `Recording`), or scale and snap the regions in `take_damage_for_age`. Gate: an Incremental-vs-Rebuild pixel oracle at 1.5 and 2.0. **M40**, a prerequisite of M34 (it affects both strategies) and M37.
- **Confidence:** high. The reproducer used a different trigger (a bound label, not a hover) and the fixture's own oracle. The attribution skeptic swapped `Scaled` and `Recording` in product code, which put the damage exactly on the physical rect and removed every scale-related difference.

#### R14-F1 — A browser page can execute agent methods: no auth, no Origin or HTTP rejection, and the endpoint serves in documented configurations

- **Severity** S0 · **Category** security · **Goal** (3) · **Verdict** CONFIRMED ×2 (S0, S0)
- **Claim:** `hostile.rs:11`: "An agent is untrusted input". R14 pass criterion: a browser-originated request cannot execute a method. `just tally` (counter-window, `window.rs:415`) serves on 127.0.0.1:9230 on every run; showcase `--features agent` defaults to 127.0.0.1:7654.
- **Premise:** held. `serve()` (winit `agent.rs:118-158`) binds any address, has no auth, and treats a line that fails to parse as a -32700 reply, not a reason to close.
- **Evidence:** (a) `nc` with `GET / HTTP/1.1` headers and a JSON body line: two -32700 replies, then the `ui.resolve #title` result. (b) `curl -X POST -H 'Origin: http://evil.example' -H 'Content-Type: text/plain'` with an `input.click #select-trigger` body: `#select-menu` went from [28,86,28,86] to [28,86,328,646], the menu opened, although curl reported an HTTP/0.9 error. (c) **Headless Firefox** on `http://127.0.0.1:18614/evil.html` running `fetch(..., {mode:'no-cors', body})`: the menu opened, frame 71→74.
- **Injection:** n/a (a live exposure). Verifier reproduction on counter-window: three browser-shaped clicks moved the counter by +30.
- **Repro:** live; scratch `476a0cc`.
- **Scope:** every build with the `agent` feature that serves: `just tally` always, showcase and tasks-demo when the feature is on, release builds included (no `debug_assertions` guard).
- **Fix and owner:** refuse an HTTP-shaped first line; require a per-session token; refuse non-loopback binds without an override. **M39**, ahead of M32's `.agent()`, M34/T34.1 and M37's agent forwarding.
- **Confidence:** high. Two verifiers, one using a raw socket sending an exact browser request against a different fixture, and one confirming no hidden factor (the result does not depend on the showcase, the backend or the feature set beyond `agent`).

### 2.2 S1

#### R1-F2 — `Host::compose` never marks, so under Boundaries an IME commit is painted against stale layout state

- **Severity** S1 · **Category** correctness · **Goal** (1) Boundaries by default (M34); IME entry · **Verdict** CONFIRMED ×2 (S1, S1)
- **Claim:** `invalidate.rs` `Invalidation`: "reporting a dirty subtree clean costs correctness". M34 §1: "Only frames marked precisely can differ: hover, an unconsumed press, focus, and tick- and host-driven writes".
- **Premise:** held. `Host::compose` (shell `lib.rs` ~:1016) calls `node_mut(i).compose(c)` and no mark.
- **Evidence:** oracle step s16, `compose Commit("defghijklmnopqrs")` into a focused 70 px TextInput; `handled=false`; dirty set (all=false, len=0). Invalidation-only arm: nodes differing [] | list equal false | **72 / 0 px**. Everything: text rect [-90,64,62,83.2] (scrolled), caret at -36. Boundaries<256>: text rect [8,64,62,83.2] (scroll 0), caret at 62. Stale through s17 and s18.
- **Injection:** the fix direction: one `mark_node` on the focused subtree in `compose` takes s16 to 0 px and an equal list.
- **Repro:** scratch `2feb332` (oracle, variant "single").
- **Scope:** any IME commit that scrolls a field, once Boundaries is on (the `boundaries` feature today, the default after M34).
- **Fix and owner:** mark the focused index (and `arenas_dirty`) when `compose` returns true. **M41**, prerequisite of M34/T34.0.
- **Confidence:** high. The reproducer used Tab focus and a bare `App`; the attribution skeptic's single precise mark removed the whole divergence.

#### R1-F3 — A tick-driven write to the tree is skipped under Boundaries: `Host::tick` never marks and the window discards its result

- **Severity** S1 · **Category** correctness · **Goal** (1) Boundaries by default · **Verdict** CONFIRMED ×2 (S1, S1)
- **Claim:** `event.rs` `App::tick` rustdoc describes tick as where animations advance and says nothing about invalidating. The plan names "Boundaries skipping a tick-driven repaint before M34" as the S1 example.
- **Premise:** held. winit `lib.rs:427` `self.host.tick(dt);` discards the result; `Host::tick` has no mark.
- **Evidence:** variant "single" (tick writes `ui.clock.t.set_str(..)` directly, as `Glass::tick` writes its own field): s17/s18, tick → true, pump handled=false. Invalidation-only: nodes [10, 11] differ, clock text width 24 against 56, tail label at x=28 against 60, **908 / 0 px**. Variant "via-update" (state written in tick, presented by `update()`): 0 px. Glass's own press animation: 0 px on s04–s10.
- **Injection:** the skeptic forced a mark whenever tick reports motion: 3/40 diverging steps → 0/40.
- **Repro:** scratch `2feb332`.
- **Scope:** applications that write tree fields from `App::tick` under Boundaries. The documented pattern (write state in tick, present in `update`) is unaffected.
- **Fix and owner:** mark all when `Host::tick` returns true, or require a matching invalidate in the rustdoc with a debug check. **M41**. The skeptic's correction: the discarded result in winit is not a cause, because the loop's `tick(0.0)` query already reports motion.
- **Confidence:** high.

#### R1-F4 — `Repeat` binds only inside layout, so under Boundaries a data change with no mark is never shown

- **Severity** S1 (verifiers split: S1 and S2) · **Category** correctness · **Goal** (1) · **Verdict** CONFIRMED + PLAUSIBLE
- **Claim:** `repeat.rs` layout: "Bound every frame, so a data change needs no invalidation call".
- **Premise:** held. The only layout skip is in derive-generated containers (`macros lib.rs:953-959`); with nothing marked, the root returns early and `Repeat::layout` never runs.
- **Evidence:** oracle s24, the thread-local data offset moved 0→100, pump handled=false, set (false,0). Invalidation-only: nodes differing [] | list equal false | **912 / 0 px**. Everything paints "row 102"; Boundaries paints "row 2". Stale in all 4 variants.
- **Injection:** reproducer's independent fixture plus an injection that removes the early return: divergence gone.
- **Repro:** scratch `2feb332`.
- **Scope:** any `Repeat` whose bind reads data changed outside a handler, `apply` or `update`, under Boundaries. The handled-frame `mark_all` hides it today; M29/T29.6 narrowing exposes it on every click.
- **Fix and owner:** correct `repeat.rs:417`, `:50`, `:139`; either a NodeDesc always-dirty flag for `Repeat` subtrees or a required `Host::invalidate`. **M41**, and a prerequisite of M29/T29.6.
- **Confidence:** high on the behaviour. The skeptic (PLAUSIBLE, S2) argues the attribution is too narrow: *any* unmarked change goes stale under Boundaries, and `Repeat` is only the case its own rustdoc promises is safe.

#### R1-F5 — The agent audit's probe mutates a live TextInput, and under Boundaries+agent a later skipped frame paints the mutation

- **Severity** S1 · **Category** correctness · **Goal** (3) the agent must be inert; (1) · **Verdict** CONFIRMED ×2 (S1, S1)
- **Claim:** `audit.rs` `natural()` goes through `Node::probe` "not layout" so the probe leaves no trace (F2.1); `audit_is_inert.rs`: "the probe left no trace".
- **Premise:** held. The default `Node::probe` is `layout` (`widget.rs:246`); `TextInput` does not override it; the audit probes every live leaf at 1e6/2e6 px.
- **Evidence:** Fill-width TextInput, 62-char commit, text rect x=-148. With `frame_done` after each frame (the window's order) and two unrelated hovers: Everything+agent -148, -148; **Boundaries+agent -148, then x=8** (scroll reset to 0, caret out of view); Boundaries without the agent -148, -148.
- **Injection:** a TextInput `probe` override that saves and restores only `scroll` (`4852f6f`) removes the effect in every arm; removing only that override brings it back.
- **Repro:** scratch `2feb332`, `r1_diag_audit_scroll`.
- **Scope:** Boundaries builds with the agent attached; Fill-width text fields.
- **Fix and owner:** a size-only `probe` in TextInput and TextArea. **M41**, and an R13/M48 concern.
- **Confidence:** high (a second fixture, a sibling container at 300 px, scroll 268).

#### R1-F6 — M34/T34.1's shadow oracle compares only NodeState and relays out the live tree, so it cannot see three of the four Boundaries defects and would repair them

- **Severity** S1 · **Category** vacuous-gate (planned) · **Goal** (1) the evidence M34 needs · **Verdict** CONFIRMED ×2 (S1, S1)
- **Claim:** `plan-boundaries-default.md` T34.1: "compares the two NodeState arrays. Paint attributes such as the hover shade are resolved in layout, so this comparison covers them"; the second pass "does run the layout code again".
- **Premise:** false as stated (hence the finding).
- **Evidence:** s16 compose: `nodes differing []` while the list differs by 72 px; s24 Repeat: `nodes differing []` and 912 px; the audit-scroll case is widget-internal (`TextInput::scroll`). Only R1-F3 (s17/18) moves NodeState. A T34.1-shaped second layout on the live widgets recomputes the scroll and re-runs the binds, so the inspected frame is repaired.
- **Injection:** both verifiers built the T34.1 mechanism and measured blindness (0 divergent nodes on all three) and repair.
- **Repro:** scratch `2feb332`.
- **Scope:** the queued M34 gate.
- **Fix and owner:** compare a bounded frame's display-list hash against an all-dirty frame; run the second pass after paint and compare the next frame, or snapshot widgets. Required injection set: R1-F2, R1-F4, R1-F5 and the hover no-op. **M41** (T34.1 redesign).
- **Confidence:** high.

#### R2-F1 — `Host::paste` lays out with BlockMetrics, and text memos do not key on the measurer, so Ctrl+V leaves a TextInput with the stand-in's box

- **Severity** S1 · **Category** correctness · **Goal** (1) memoisation; the default window · **Verdict** CONFIRMED ×2 (S1, S1)
- **Claim:** `text.rs` `MeasureKey`: "Everything a text measurement depends on besides the text itself"; `input.rs:87-90` `shaped_key`: "Text length and the style inputs".
- **Premise:** held. `paste` pumps with BlockMetrics every 64 characters and at the end; `shaped_key` is (len, px, weight, max_w).
- **Evidence:** focus, `paste("mmmmmmmm")`, two real-measurer pumps: **w=80, caret 8**; typing the same 8 characters: w=131.2. Persists across 2 real frames. BlockMetrics then a proportional measurer: Label 64 vs 73.6, Button 92 vs 101.6, TextInput 80 vs 89.6. Setup asserted: both fields hold "mmmmmmmm".
- **Injection:** the typed path is the control. The skeptic split the quantity: any single BlockMetrics layout after an edit produces the 80; with none, the result is correct.
- **Repro:** scratch `6772c89` (`r2_paste.rs`), `02a0b31` (`r2_keys.rs`).
- **Scope:** every winit window: Ctrl+V calls `Host::paste`.
- **Fix and owner:** paste posts events and lets the caller's `pump_with` lay out, or `paste_with(&mut dyn TextMeasure)`; key memos on a measurer generation. **M45**.
- **Confidence:** high. Reproduced with a real system font and winit's call order. Same defect seen from R10's side as R10-F4.

#### R3-F1 — Arrow keys never reach a TextInput, NumberInput or Slider that has a focusable sibling

- **Severity** S1 · **Category** correctness · **Goal** (1) desktop usability; (3) · **Verdict** CONFIRMED ×2 (S1, S1)
- **Claim:** `pass.rs:434-439`: "Only when the parent has more than one focusable child, so an arrow inside a text field still edits text rather than jumping away"; `:457-459`.
- **Premise:** held. The group rule (`pass.rs:440-471`) runs before pass 1.
- **Evidence:** row{TextInput, Button}, "abc", caret 3: one Left → focus on the button; two Lefts → focus back, **caret still 3**, both handled=true. NumberInput: End, Left, Left, '9' turns "0.00" into "0.009". Slider: Right moves focus, value stays 0. Agent level: after `input.fill 'abc'`, `input.key Left` → focus moved, caret 3.
- **Injection:** prototype `11492f9` (offer the arrow to the focused widget first, 7 lines) turns the probes green and keeps `focus.rs` (9), `events.rs` (14) and `slider.rs` green. A "focus unchanged after two Lefts" assertion is itself vacuous (focus round-trips), so the probe asserts after one Left.
- **Repro:** scratch `3209323` (`r3_probe::r3_q3_*`).
- **Scope:** every form: a field next to a button.
- **Fix and owner:** as the prototype. **M44**. Also R10-F3 (same defect, text-editing view).
- **Confidence:** high. A disabling injection on only the group branch isolated the cause; the skeptic found it also takes arrows from sliders beside fields.

#### R4-F1 — No window can hot reload, but the handoff and the book say it can

- **Severity** S1 · **Category** doc-drift · **Goal** (4) · **Verdict** CONFIRMED ×2 (S1, S1)
- **Claim:** handoff §10 Step 4 "DONE 2026-08-30 ... Live dylib swap demo passes the full §7 sequence"; §1 "supports hot reload during development"; `book/src/first-tree.md:98-99`, `:104-105`; `book/src/agent.md:94-96`; `book/src/component-boundary.md:41-42`.
- **Premise:** held. The only dlopen is `fixtures/hot-host` (headless, no Host, no window); the only cdylib is `fixtures/hot-app`; `lumen-static-winit` has no `reload` feature; nothing calls `Host::reload`.
- **Evidence:** by `cargo metadata` target list and the inverse dependency graph (verifier), not grep alone. `plan-panic-recovery.md:13-18` already says "Nothing in the workspace calls Host::reload". Missing (M37 §1–§2): build step or watcher, a loader in winit or shell, an example cdylib, the Host in the library, a list-taking present, agent forwarding, failure display.
- **Injection:** n/a (absence).
- **Repro:** read at `ac91d51`.
- **Scope:** every reader of the handoff and book.
- **Fix and owner:** mark Step 4 "headless fixture only; windowed reload = M37" and move the four book passages to the future tense now; the mechanism is **M37**.
- **Confidence:** high.

#### R7-F1 — `WaitUntil` does not make a timer fire on an idle window: the loop wakes at the deadline and never requests a frame

- **Severity** S1 · **Category** correctness · **Goal** (1); timers and subscriptions · **Verdict** CONFIRMED ×2 (S1, S1)
- **Claim:** `results-tasks.md` T28.12: "`about_to_wait` sets `WaitUntil` from the host's deadline. Without it a timer on an idle window is never checked"; winit `lib.rs:507-512`; `spawn.rs` Timers doc.
- **Premise:** held. `Win` implements no `new_events`; `next_deadline_ms()` advances only inside `tick()`, which runs only in `redraw()`.
- **Evidence:** tasks-demo release, `toggle-sub` (120 ms timer), 3.04 s with no input: **40 wakes 75.7 ms apart, 0 frames**, ticks 1 (expected ~25). The next read forces a frame with dt=3041.4, ticks=24 (the catch-up). 8.04 s idle: 0 frames, ticks 1→1, then 33 (67 expected; see R7-F4). About 13 empty wakes/s while a timer exists.
- **Injection:** n/a (direct demonstration); R7-F2 is the gate that should have caught it.
- **Repro:** scratch `7038bc2` (frame/about_to_wait log behind R7PROBE).
- **Scope:** every turnkey window with a timer or subscription.
- **Fix and owner:** request a redraw when the deadline has passed (`new_events` with ResumeTimeReached), or keep an absolute `Instant`. **M47**.
- **Confidence:** high. The reproducer ran a stock build with 0 ticks over 5 s idle; the skeptic ruled out a timer thread, an absolute deadline going ≤ 0, and winit redrawing on timeout.

#### R7-F2 — tasks-drive and wake-gate are vacuous for `run()`'s wiring: removing the waker install or the `WaitUntil` leaves both green

- **Severity** S1 · **Category** vacuous-gate · **Goal** (1); M28's "use the thing" · **Verdict** CONFIRMED + PLAUSIBLE (both S1)
- **Claim:** `results-tasks.md` T28.12: "`run()` installs the waker and the spawner" and "`about_to_wait` sets `WaitUntil`", as proven by `just tasks-drive`; wake-gate: "a completion wakes an idle window".
- **Premise:** held. `tasks_drive.py`'s `settle()` polls every 20 ms and every RPC ends in `request_redraw` (`lib.rs:575-577`); wake-gate is its own `ApplicationHandler` (`examples/wake-gate/src/main.rs:170-174`) and never calls `run()`.
- **Evidence:** under injection A the defect is real: a 400 ms job, 1.5 s unpolled idle, slow=0 (clean: slow=1).
- **Injection:** A (`294dee1`, waker removed): wake-gate exit 0, tasks-drive 10/10 ok. B (`14477cf`, deadline ignored): tasks-drive 10/10 ok. Reverts `e4fa357`, `edd808f`.
- **Repro:** as above.
- **Scope:** the M28 evidence.
- **Fix and owner:** a live gate that reads once after an unobserved idle; a wake-gate sibling that drives `run()`. **M47**.
- **Confidence:** high. The skeptic (PLAUSIBLE) narrows it: wake-gate is sound for the `Sender` wake it names (injection D, `bc50a26`, turns it red); what is missing is *any* gate over `run()`.

#### R8-F1 — "Rasterising a frame allocates nothing" is false on the truetype path every window runs

- **Severity** S1 · **Category** vacuous-gate · **Goal** (1) allocation discipline · **Verdict** CONFIRMED ×2 (S1, S1)
- **Claim:** `alloc_gate.rs` `rasterising_a_frame_allocates_nothing`; handoff "the gate covers a rasterised frame"; `results-allocation.md` T26.13 "after: 0".
- **Premise:** held. The gate builds `Canvas::new` and calls `host.pump()` (BlockMetrics); winit uses `FontStack::system` + `Canvas::with_font` with `truetype`.
- **Evidence:** the gate's own PlainApp on the truetype canvas: **25.80 allocs/frame** raster (1290/50), 4.60/frame in `pump_with(FontMetrics)`, idle pump 0 (held). 3000-row list, one row changed: pump 6.00, raster 156.00/frame. Per event: PointerMove 0/0 (held); Text 7.00 + 106.76; Key Left 0 + 162.00; Backspace 5.66 + 62.28; `paste("hello")` 0.08 + 475.56. Sources: two fontdue `Layout`s per run, `Vec<Placed>`, `Vec<Glyph>`, one `Vec<u8>` per glyph.
- **Injection:** the truetype twin `q1_plain_truetype` is red (1290 over 50 frames); the original stays green (11 passed). The skeptic's sum of the named sources reproduces 25.80 exactly.
- **Repro:** scratch `77f355a`/`d8d328a` (`tests/r8_probe.rs`).
- **Scope:** every windowed application.
- **Fix and owner:** a truetype arm in `alloc_gate`; a glyph cache; reuse one `Layout` per canvas. Correct handoff §3 and `results-allocation.md` now. **M50**.
- **Confidence:** high.

#### R8-F4 — `Host::paste` drops every newline pasted into a TextArea while reporting it delivered

- **Severity** S1 · **Category** correctness · **Goal** (1)/(3) text entry · **Verdict** CONFIRMED ×2 (S1, S1)
- **Claim:** `Host::paste`: "Returns how many characters were delivered, which is every non-control one unless a widget refused them"; the filter deliberately keeps `'\n'`.
- **Premise:** held. `TextArea::on_event` accepts `Text` only if `!ch.is_control()` (`textarea.rs:566`); a newline enters only as `Key::Enter`.
- **Evidence:** `paste("ab\ncd")` returned 5, TextArea holds "abcd"; typed with Enter: "ab\ncd". A 1,048,572 B paste of 38,836 lines: length 1,009,736, the difference being exactly the line count.
- **Injection:** n/a; the typed path is the control.
- **Repro:** scratch `d8d328a` (`q7_paste_newline_into_textarea`).
- **Scope:** Ctrl+V into any TextArea; the agent's `type` path has the same shape.
- **Fix and owner:** map `'\n'` to Enter in paste, count accepted characters only. **M45**. Same defect as R10-F5 (S2 there).
- **Confidence:** high.

#### R10-F1 — The default measurer and painter disagree about their own box: a run ending in a negative-bearing glyph is measured 1 px short and repainted onto a second line

- **Severity** S1 · **Category** correctness · **Goal** (1) · **Verdict** CONFIRMED ×2 (S1, S1)
- **Claim:** shell `text.rs:257-263` (`measure_run`): "the two cannot disagree about a box"; `text.rs:176-181` "Width from the advance, not the ink box".
- **Premise:** held. Width is `max(g.x + max(adv, g.width))` and fontdue's `g.x` already includes `xmin`.
- **Evidence:** NotoSans 16 px "Taj": measured 21.13 (ceil 22), pen end 22.13, relayout at 22 → 2 lines (DejaVu: 24 against 24.45). Real Host: **4 of 13 labels** ("Raj", "Taj", "Hadj", "Tj") paint taller than their box at every scale; at k=1 ink rows span 38 px against a 22 px box. Corpus of 1200 runs: 205 wrapped, 75 with ink more than 1 px outside horizontally, worst vertical overhang 80 px against a 1.0 px bound. A second mechanism (f32 round-down of `w × scale`) accounts for 20 of the 1200.
- **Injection:** pen-based width (`dcde5ef`, reverted `7e96729`): 4/13 → 0/13, corpus 205 → 20.
- **Repro:** scratch `dcde5ef` (probes `r10_q3_attr`, `r10_q3_host`).
- **Scope:** every window, every scale, any run ending in 'j' or a similar glyph. The GPU path calls the same `place_run` (read, not run).
- **Fix and owner:** width from pen plus advance in `measure_run`, `place_run`, `shape_into`; a containment gate pairing FontMetrics with Canvas at fractional scales. **M45**.
- **Confidence:** high. The reproducer used DisplayList+Canvas with no Host and a different corpus (234/520 failures, all ending in 'j'); the skeptic confirmed from fontdue's source that it wraps on ceiled pen advances.

#### R10-F2 — `lumen-static-text` (T15.8) shapes only the measurement: Arabic is painted unjoined and left to right

- **Severity** S1 · **Category** perf-claim (capability claim) · **Goal** (1), (2) · **Verdict** CONFIRMED + PLAUSIBLE (both S1)
- **Claim:** `plan-capabilities.md` T15.8 gate "an Arabic string measures and paints right-to-left"; `book/src/text.md:31`; `lumen-static-text/src/lib.rs:25` "this changes where the glyphs go, not how they are drawn".
- **Premise:** held; also, no turnkey window can install `ComplexText` at all (`Config`/`Program` have no measurer seam, `lib.rs:312`).
- **Evidence:** "مرحبا": ComplexText 37×18.6, FontMetrics 51×19, unconstrained ink 47 wide. Painted in the 37 px box the run wraps: ink 29 px tall against an 18.6 px box. Pixel hash equals the isolated presentation forms U+FEE1… (`911a29388d32e60a` both); fontdue places م at x=1 and ا at x=46. "abc אבג def": measured 92 against ink 96, wraps to 31 px tall. `TextInput::x_of` assumes monotone byte order, so every caret offset but the end maps to x=0 for visual-order clusters (read).
- **Injection:** `44fac1a` (bidi reorder deleted) turns only `a_mixed_line_produces_runs_in_both_directions` red; see R10-F11.
- **Repro:** scratch probes `r10_q1`.
- **Scope:** any application that needs complex scripts.
- **Fix and owner:** paint from the shaper's glyph ids and positions, or downgrade T15.8's DONE, the book table and the crate doc to "measurement only"; a measurer seam on `Config`/`Program`. **M45**.
- **Confidence:** high on the behaviour (the reproducer compared glyph ids directly). The PLAUSIBLE verifier keeps S1 on "a goal-level claim is false", noting no turnkey window can reach the path.

#### R10-F3 — In a text field with a focusable sibling, the arrow keys move focus instead of the caret

- **Severity** S1 · **Category** correctness · **Goal** (1) · **Verdict** CONFIRMED ×2 (S1, S1)
- **Claim:** `pass.rs:458-459`: "the key belongs to the widget, which is what keeps Left and Right editing text inside a field".
- **Premise:** held (the same rule as R3-F1).
- **Evidence:** column [TextInput "hello", TextArea, TextInput], focus on the first after End: Left → (focus 3, caret 5), expected (1, 4). In the TextArea, Up moves focus 2→1. A lone field: (1, 4).
- **Injection:** disabling the `group.len() > 1` branch restores the caret move and keeps focus; the skeptic found the same loss for a Slider beside a TextInput.
- **Repro:** probe `r10_arrows`.
- **Scope:** every multi-field form.
- **Fix and owner:** as R3-F1. **M44**.
- **Confidence:** high.

#### R13-F1 — `ui.waitSettled` answers `settled:true` in every state, including mid-animation and with a task pending; its only test pins the constant

- **Severity** S1 · **Category** correctness · **Goal** (3) · **Verdict** CONFIRMED ×2 (S1, S1)
- **Claim:** agent `lib.rs:787` "No animation scheduler exists: a laid-out tree is settled." SKILL.md lists it among the synchronisation methods; winit `lib.rs:456` says it tells an agent "when it has finished moving".
- **Premise:** held (`lib.rs:786-789` returns the constant).
- **Evidence:** showcase Spinner: three polls 0.2 s apart answered true at frames 113, 560 and 1160 (447 and 600 frames between polls), while `ui.lastChange` reported content changes on `d_spinner_b/c`. tasks-demo: `input.click #start-slow` → waitSettled true at frame 5 while `#status` read "last: "; 1.0 s later "last: slow=99". Measured true in 3/3 animated polls and 1/1 pending-task polls.
- **Injection:** `4ccbf52` (return false) turns exactly one test red, `protocol.rs:196`, which asserts the constant.
- **Repro:** live on port 7613.
- **Scope:** every agent client that synchronises on it.
- **Fix and owner:** answer from `host.tick(0.0)`, `next_deadline_ms()`, `more_deferred()` and pending tasks; a test that is unsettled during a Spinner tick. **M48**.
- **Confidence:** high. The reproducer used tasks-demo's subscription timer and chain against ground truth.

### 2.3 S2

Each record keeps the §2 fields. Where the premise held and the repro is
the probe named under "Evidence", those lines are folded together to keep
the section readable.

#### R1-F8 — The software partial raster does not snap fractional damage edges, leaving a persistent residue row
- **S2** · correctness · (1) · CONFIRMED (S2)
- **Claim:** wgpu `lib.rs:150-156` `snap_out`: a partial redraw is "exactly equal to a full one" there; `software.rs` has no equivalent.
- **Evidence (premise held):** damage-only arm at scale 1.0, s04 first glass press tick, damage [[0,87.2,360,173.2]]: **32 / 0 px** in bbox [0,173,40,174], persisting s04–s22.
- **Injection:** switch R1_SNAP (snap regions outward) removes every residue in all 4 variants.
- **Repro / scope:** scratch `2feb332`; software backend, any fractional damage edge over text.
- **Fix and owner:** snap outward in `Retention::finish` (shared by both backends). **M40**.
- **Confidence:** high. The verifier corrected the attribution: fills already iterate floor..ceil; the outlier is the truetype glyph path.

#### R2-F2 — A same-length collision of `display::fold` leaves stale pixels under Incremental retention
- **S2** · correctness · (1) · CONFIRMED (S2)
- **Claim:** `retain.rs:45-49`: "a probability around 10⁻¹³ per frame, and shell/tests/damage_oracle.rs is the standing check".
- **Evidence (premise held: `display.rs:52` is a linear fold with no length):** LLL on Σ dᵢK^(11-i) ≡ 0 mod 2^64 in 7.8 s gave printable 12-byte pairs (" k ih  lsio " / "$a#aa$-aaaa(", "akaihaalsioa" / "eadaaenaaaai"). Through `Host<Probe, Incremental>`: control → damage Regions, 0 stale; collision → **damage None, 1440 stale px** (bound 0). The difference is linear, so it survives any prefix, including `DisplayList::hash_from` (the agent journal).
- **Injection:** fix variant `e8bbd21` (word mix + splitmix finaliser): 0 stale, `damage_oracle.rs` still 10/10.
- **Repro / scope:** `ae9125b` (`r2_collision.rs`); adversarial or unlucky text in any Incremental host.
- **Fix and owner:** nonlinear word-at-a-time fold, seeded if adversarial text is in scope; correct the 10⁻¹³ sentence. **M46**.
- **Confidence:** high.

#### R2-F3 — `FoldedText` (Button, Badge, Chip) serves a stale width for fold-equal text
- **S2** · correctness · (1) · CONFIRMED (S2)
- **Claim:** `text.rs` `FoldedText`: "the key carries a fold of the bytes in place of a dirty bit".
- **Evidence (premise held):** "x"→"\0x": Button served 36 vs 44, Badge 20 vs 26, Chip 30.5 vs 37. Same-length pair: Button 112 vs 120, Badge 77 vs 83, Chip 92.25 vs 98.75. Control "eadaaenaaaaj": served = correct.
- **Injection:** `1f78718` (word mix) makes every row correct; `text_memo.rs` 4/4 still green, so the memo still hits.
- **Repro / scope:** `7cadfa6` (`r2_folded.rs`); the three `FoldedText` widgets.
- **Fix and owner:** as R2-F2 ("length plus finaliser" fixes only the NUL-prefix case). **M46**.
- **Confidence:** high.

#### R2-F5 — `table_is_well_formed` accepts tables whose `subtree_range` is wrong, and a one-line derive mutation that crashes observe
- **S2** · vacuous-gate · (1)(2) · CONFIRMED (S2)
- **Claim:** `macros lib.rs:29-31` "the spliced table is well-formed"; `verification-notes.md` Tier 0; `table_invariants.rs:3`; `equivalence.rs:57-59`.
- **Evidence (premise held: `widget.rs:362-423` has no pre-order clause and lets a last child link to any later node):** counterexamples A–D pass (C returns `subtree_range(2) = 2..99` on a 3-node table). I3 (`3051257`, last child links to its parent's end): `cargo check --workspace --all-targets` green, `cargo test --workspace` **888 passed / 69 failed**, panicking at `observe.rs:79` and `equivalence.rs:44`.
- **Injection:** I1 (next+1) 54 const errors, I2 (sibling swap) 23; I3 green at compile time. Proposed clause (`e510bdd`, pre-order + last child `next == 0`): rejects A–D and I3 (71 errors), workspace checks clean, `table_invariants.rs` 12/12.
- **Repro / scope:** `f7d5def` (`r2_tables.rs`); every derived table; M34's marks rest on it.
- **Fix and owner:** the two clauses plus negative cases. **M41**.
- **Confidence:** high. The verifier's precision: I3/D break the chain walk, not `subtree_range` itself.

#### R2-F6 — TextInput and TextArea shape keys omit the display scale, so T26.17 #12's monitor-drag defect survives there
- **S2** · correctness · (1) · CONFIRMED (S2)
- **Claim:** `text.rs` `MeasureKey`: "The display scale is in the key — ... measure_scale.rs is the gate".
- **Evidence (premise held):** TextInput 1×→2× at identical limits: served 145, correct 146; Label (MeasureKey) re-measures correctly. With a real FontMetrics the stale width is 27.5 px (verifier). TextArea: same tuple, shown by reading.
- **Injection:** n/a; a fresh TextInput at 2× gives 146.
- **Repro / scope:** `02a0b31` (`r2_keys.rs`); HiDPI monitor drags.
- **Fix and owner:** build the keys from `MeasureKey::of(.., cx.scale())`; extend `measure_scale.rs`. **M45**.
- **Confidence:** high.

#### R3-F2 — Input overflow is silent: a lost PointerUp strands capture; agent `input.type`/`fill` misreport past 64 events
- **S2** · correctness · (1)(3) · CONFIRMED (S2)
- **Claim:** `event.rs` `InputQueue::push`: "The drop is *counted*, not silent"; agent `lib.rs:700-706` "the queue has no cap".
- **Evidence (premise held: `dropped()` has no caller; `Host::post` discards the bool; winit posts every `CursorMoved`):** press the slider, 64 moves + Up (65 events): capture stays `Some(7)`; a later button-less move drags 0.365→0.95. 63 moves + Up: released. Agent: `input.type` 100 chars replies `chars:100`, field holds 64, `dropped()=36`; `input.fill` of 83 events fails with "not an editable field".
- **Injection:** 65 red, 64 green, same session.
- **Repro / scope:** `30a1b0f`/`fd73e6f` (`r3_host`, `r3_agent`); slow frames with a fast mouse; every agent typing call over 64 characters.
- **Fix and owner:** `post` reports and accumulates refusals, merges moves; agent chunks like `paste`. **M44**.
- **Confidence:** high. The verifier reproduced on the real `Live::enqueue` path, not the author's test double.

#### R3-F3 — Activation fires on whatever is under the release, not on what was pressed
- **S2** · correctness · (1) · CONFIRMED (S2)
- **Claim:** `event.rs` `EventCtx.row`: "can never go stale the way a captured index does".
- **Evidence (premise held: `pass.rs:682-712` activates on the release's hit):** press #save, release on #other: others=1, saves=0. Press row 1, release on row 7: picked=[107]. Press row 5 (item 105), remove row 4, release: **handler sees 106**.
- **Injection:** n/a (setup asserted handled=true and that the handler ran).
- **Repro / scope:** `3209323`; every button and list row.
- **Fix and owner:** latch (node, row, key) at PointerDown; activate only on a match. **M44**.
- **Confidence:** high.

#### R3-F4 — A captured slider keeps receiving moves and changing value after it is hidden
- **S2** · correctness · (1) · CONFIRMED (S2)
- **Claim:** `pass.rs:382-405` C5 covers focus only; capture checks only `cap >= table.len()`.
- **Evidence:** column{Either<Slider,Label>}: press captures, `show(Side::B)`, slider size 0, move +80 → handled=true, **value 0.1→0.9**. The equivalent focus test holds.
- **Injection:** the existing capture release is gated (removing `*capture = None` turns 2 tests red).
- **Repro / scope:** `3209323`; any drag across a variant switch.
- **Fix and owner:** apply C5's area test to capture. **M44**.
- **Confidence:** high.

#### R3-F5 — `Repeat`'s `Handlers::handler` reads the unrotated window slot, so per-row handlers are misrouted after scrolling
- **S2** · correctness · (1) · CONFIRMED (S2)
- **Claim:** `repeat.rs:647-649` "Routes to the template's handlers with the hit row attached"; `slot()` doc.
- **Evidence:** `repeat.rs:651-656` uses `window.get(k)`, `hit()` uses `slot(k)`. Per-row parity handlers, 100 rows, scroll 100: **5 of 10 clicks ran the other row's handler**.
- **Injection:** `4c6bab6` (`get(self.slot(k))`) → 0/10; `events.rs` 14/14.
- **Repro / scope:** `4c6bab6`; binds that set per-row handlers (none in the repository today, verifier).
- **Fix and owner:** the one line. **M44**.
- **Confidence:** high.

#### R3-F6 — Keyboard-only users cannot reach or pick rows
- **S2** · a11y · desktop; (3) · CONFIRMED (S2)
- **Claim:** `pass.rs:670-673` keyboard activation "is what makes a Button reachable without a pointer"; T15.6 "keyboard navigation … done".
- **Evidence:** 100-row Repeat + 2 Buttons: Tab×50 visits {1,2} only; 0 row activations. Select opens only on PointerDown (`select.rs:297-308`); Chooser, Picker, Tree, DataGrid have no row navigation. **Live** showcase (port 7603, pid and exe verified): 12-key sequence, menu rows stay 0; Enter/Space on `#select-trigger` report handled=true and open nothing; 1 catalogue entry reachable. (A reading premise that Select is not focusable was false: its trigger is a Tab stop.)
- **Injection:** n/a (capability absent; setup proven: 18 rows materialised, trigger focused).
- **Repro / scope:** `8715c64` (`r3-evidence/live_kbd_r3.txt`); every list and picker.
- **Fix and owner:** Enter/Space open; active-row index with Up/Down/Home/End. **M44**, plus keyboard actions in M42's harness.
- **Confidence:** high. The verifier notes T15.6 claims only Tab, Shift+Tab, arrow groups and the ring, so this is a missing capability, not a falsified sentence.

#### R3-F7 — Tab under a Tooltip drops focus to None; Escape on a Modal leaves focus on a hidden button
- **S2** · a11y · desktop · CONFIRMED (S2)
- **Claim:** `pass.rs:227-236` "Tab is trapped inside the topmost overlay".
- **Evidence:** tooltip open, Tab: focus 1→**None** (without the tooltip 1→3). Modal: trap cycles [10,11,10,11,10]; Escape → focus Some(10), a size-0 button, then None; never the trigger.
- **Injection:** accept direction green in the same session.
- **Repro / scope:** `fd73e6f`; any page with a tooltip or modal.
- **Fix and owner:** trap only for modal overlays (Veil); restore focus on close. **M44**.
- **Confidence:** high (every Tab stays dead while the pointer rests on the control, verifier).

#### R4-F2 — "~90–250 ms, comfortably inside the 2 s criterion" is from a synthetic crate; a real app in the demo's release profile misses 2 s
- **S2** · perf-claim · (4) · PLAUSIBLE (S2)
- **Claim:** `step4-hot-reload.md` "~90–250 ms — comfortably inside the 2 s criterion end to end"; `book/src/component-boundary.md` "reload times sub-second".
- **Evidence (LOADED, load 6.1–12.7):** hot-app debug median 227 ms, release 465 ms. showcase as a cdylib (`b895f55`), 8 runs: debug median **0.655 s**, release **2.735 s (min 2.59) / 2.0 s**, 8.8–11.4 s user CPU; codegen 1.289 s of `finish_ongoing_codegen` in a 2.613 s total. The swap itself held: 76–187 µs.
- **Injection:** n/a; `strings` confirmed each rebuilt `.so` held the edited literal.
- **Repro / scope:** `b895f55`; M37's latency budget.
- **Fix and owner:** name the profile; build reloads in the dev profile; gate on a real-app fixture. **M37** rule 4.
- **Confidence:** medium. The verifier reproduced both numbers (min 2.60 s) and disputes only framing: the cited figure is honestly caveated as synthetic in its own doc.

#### R4-F3 — T28.7's cross-swap chain gate exercises no framework code
- **S2** · vacuous-gate · (4), (1) · CONFIRMED (S2)
- **Claim:** `plan-tasks.md` T28.7 "the inbox, the epoch and the pending descriptor live in `hot-host`"; `events-decision.md`/`results-tasks.md` "demonstrated rather than argued".
- **Evidence (premise false: the finding):** no `epoch`/`inbox` in `hot-host`; `nm -C` finds 0 `lumen_static::(spawn|inbox)` symbols in `libhot_app_v2.so` and in hot-host; the chain calls the fixture's own `hot_step`.
- **Injection:** `79e7c7c` (`Descriptor::at` ignores its step): demo PASS, "5 -> 5000".
- **Repro / scope:** `79e7c7c`; M28's rule-3 evidence.
- **Fix and owner:** a real Inbox and Spawner across a swap with a framework-side injection. **M49** / M37.
- **Confidence:** high. The verifier notes the gate still demonstrates the principle on real code; the overstatement is in three sentences.

#### R4-F4 — `check_no_unsafe.sh` can be bypassed three ways
- **S2** · vacuous-gate · safety of the core · CONFIRMED (S2)
- **Claim:** `check_no_unsafe.sh:28-29` "it cannot hide a real `unsafe` token outside a string literal".
- **Evidence:** `dea0bb6` plants `"a//b"; unsafe {..}` on one line, a `#[path]` module outside `src`, and a `build.rs`-generated `include!`. Gate exit 0; `cargo build --release -p lumen-static` succeeds; a plain unsafe is refused. Only `lumen-static-widgets` had `forbid(unsafe_code)`.
- **Injection:** fix `54d3d79` (string-aware strip) refuses 1 of 3; `c275587` (`forbid(unsafe_code)` on 7 crates) refuses 3 of 3 and accepts the current tree.
- **Repro / scope:** `dea0bb6`; the M0 "no unsafe" claim.
- **Fix and owner:** both fixes; forbid is the gate. **M43**. Same mechanism as R17-F2.
- **Confidence:** high.

#### R5-F2 — Stateful widgets have no Reloadable impl: 7 of 9 user-state values are lost with 0 log lines
- **S2** · correctness · (4) · CONFIRMED (S2)
- **Claim:** R5 goal "a reloaded app ... keeps the state being iterated on"; `reload.rs:58` "silent fallback is indistinguishable from a bug".
- **Evidence (premise held: only Label, Button, Toggle, Fixed, Spacer, Repeat, Vec, Option implement it):** TextInput text and caret, Slider, ScrollView scroll, Popup open, Collapsible open, Tabs selected: all LOST; 0 log lines. An untagged TextInput is E0277; `#[reload(keep)]` Slider is E0277 (no serde); a StaticTree enum field is refused. Fields are private and the orphan rule forbids app-side impls, so `#[reload(fresh)]` is the only option that compiles.
- **Injection:** probe asserts "lost == 0 || log non-empty"; a no-op Toggle restore makes it report Toggle LOST.
- **Repro / scope:** `8f88545` (`r5_widget_state.rs`); 21 widget families plus every derived enum.
- **Fix and owner:** Reloadable for the user half of every stateful widget; the enum derive. **M49**, prerequisite of M37.
- **Confidence:** high.

#### R6-F1 — The idle frame "under Boundaries is the 0.01 µs row" is false: the full idle pump is ~111 µs at 10k nodes
- **S2** · perf-claim · (1) · CONFIRMED (S2)
- **Claim:** `results-competitive.md` §5b "Under `Boundaries` that frame is the 0.01 µs row".
- **Evidence (LOADED, 8 runs):** idle pump under `Boundaries<256>`: 1k 11.89, 3k 33.54, **10k 111.47 µs**; split layout 0.02, place 22.57, paint 80.68, finish 9.21. Everything on the same tree: 193.05. So 193 → 111, not 211 → 0.01.
- **Injection:** n/a (measurement; the verifier's independent List-of-10,000 probe: 176 against 224 µs, layout 0.03).
- **Repro / scope:** `8512ace`/`397e910`; every Boundaries idle frame.
- **Fix and owner:** state the whole pump beside the layout row; an idle-frame skip. **M53**.
- **Confidence:** high.

#### R6-F2 — The M29 grid probe's "frame" omits `cascade_arenas_deep`, so "20.7 µs, 36% of the frame" is ~25% of the pump
- **S2** · perf-claim · (1), M29 · CONFIRMED (S2)
- **Claim:** `results-damage-precision.md` T29.1 "20.7 µs, 36% of the frame".
- **Evidence (LOADED, 8×40):** whole pump mark_all 81.37, narrowed 60.27, handled 88.18 µs against the column's 58.28 / 37.30 / 58.28. The cascade ran 40/40 frames in every arm (~21.5 µs). Saving 21.1 of 82.8 µs = **25.5%**. The kill-rule verdict is unaffected.
- **Injection:** I3 (`b50d738`, `arenas_dirty` forced): the frame column stayed inside its spread; no arm sees the step.
- **Repro / scope:** `1b6515b`, `29cb1de`; M29's justification.
- **Fix and owner:** whole-pump and cascade columns in `grid`. **M29**/**M53**.
- **Confidence:** high (verifier: 20.8 µs saving, ~26%).

#### R6-F3 — Every click and every `Host::invalidate` re-runs `cascade_arenas_deep` even when no arena was touched
- **S2** · perf-claim · (1) · CONFIRMED (S2)
- **Claim:** `pump_with`: `if handled { mark_all; arenas_dirty = true }`; `Host::invalidate` sets it unconditionally.
- **Evidence:** showcase Toggle click (no arena touched), 221 nodes: cascade 40/40, **0.94 of a 2.54 µs pump** (37%). Grid: 21.5 of 81.4 µs, more than its layout (23.6) and more than the 21.1 µs narrowing saves.
- **Injection:** suppressing the flag: cascade calls 40→0 with identical pixels (verifier A/B: 0.98 µs and ~22 µs).
- **Repro / scope:** `42f05e1`; every click.
- **Fix and owner:** Phase D finding 9; set `arenas_dirty` only on structural change. **M53**, and a prerequisite for any net M29/T29.6 saving.
- **Confidence:** high.

#### R7-F3 — `Timers::advance` never returns for a large dt
- **S2** · correctness · UI-thread hang · CONFIRMED (S2)
- **Claim:** `spawn.rs` "Catches up rather than skipping".
- **Evidence:** once ulp(|remaining|) > 2·period, `remaining + period == remaining`. 16 ms, 1 h: returned (225,000 firings); 16 ms, dt 1e9: **did not return**; 75 h, 9.4 h at 1 ms, 25 days at 120 ms: did not return. With R7-F1, dt is the whole idle time, so a 1 ms timer idle overnight hangs on the next mouse move.
- **Injection:** n/a; the 1 h case is the accepting side. The verifier showed an exact f32 fixed point at the first step and a threshold near 2²⁴ × period.
- **Repro / scope:** `0afedab` (`r7_huge_dt_terminates`); any timer.
- **Fix and owner:** arithmetic fire count with `rem_euclid`, clamp dt. **M47**.
- **Confidence:** high.

#### R7-F4 — A catch-up past `Outbox::CAPACITY` loses firings uncounted
- **S2** · correctness · (1) · CONFIRMED (S2)
- **Claim:** `spawn.rs` `Outbox` "Overflow refuses and counts".
- **Evidence:** `Host::tick` advances into a local scratch outbox whose `refused()` is discarded. 10 ms timer, `tick(1000)`: fired 100, delivered 32, **lost 68, counted 0**; live 35 lost of 67.
- **Injection:** accept side 30/30.
- **Repro / scope:** `0afedab`; any catch-up.
- **Fix and owner:** use the app's outbox or surface the count. **M47**.
- **Confidence:** high (the verifier adds that `advance`'s own `fired` over-reports).

#### R7-F5 — "No thread is created for a timer" is false under the shipped spawner
- **S2** · perf-claim · (1) · CONFIRMED (S2)
- **Claim:** shell `lib.rs:972-976` "no thread is created for a timer"; `results-tasks.md` T28.11 "zero threads for a timer".
- **Evidence:** firings go through `Host::spawn` into `Threads`: strace shows **26 clone3 for 26 ticks**; `timers.rs` itself asserts 20 spawns for 20 firings.
- **Injection:** n/a.
- **Repro / scope:** live tasks-demo on `:77`; every window with a timer.
- **Fix and owner:** deliver firings straight to the inbox, or reword. **M47**.
- **Confidence:** high.

#### R7-F6 — A pointer moving over content it does not change costs a full pump and a whole-surface present per motion event
- **S2** · perf-claim · (1) · CONFIRMED (S2)
- **Claim:** winit `lib.rs:470-472` "Another frame only while something is moving. A still UI idles".
- **Evidence (counts load-insensitive):** 960×720, ~120 Hz for 10.2 s: software 1150 motions → **1149 frames, 0 damaged px**, CPU 1.3%; llvmpipe 1172 → 1171, CPU 61.1%.
- **Injection:** n/a; the verifier's 3 s idle control produced 1 frame.
- **Repro / scope:** live, Xephyr `:77`; every window.
- **Fix and owner:** skip present on self-requested redraws with empty damage, or no redraw for a move that changes no hover or capture. **M47**.
- **Confidence:** high.

#### R7-F7 — GPU present on Outdated/Lost drops the frame and asks for no redraw
- **S2** · correctness · desktop/mobile robustness · CONFIRMED (S2)
- **Claim:** wgpu `lib.rs:1223-1228` "let the next redraw present".
- **Evidence:** `Gpu::present` returns `()`; nothing redraws. Forced Outdated: window has **1 distinct pixel value** (blank) against 55, no frame over 4 s.
- **Injection:** `bb2649a` (reverted `c907982`).
- **Repro / scope:** llvmpipe; any surface loss on an idle window. The verifier notes the software path panics rather than drops, so "what every other backend does" is also false.
- **Fix and owner:** return a dropped status and redraw. **M47**.
- **Confidence:** high.

#### R8-F3 — Text raster cost scales with the run's length, not with the damage
- **S2** · perf-claim · (1) · CONFIRMED (S2)
- **Claim:** `software.rs` Text arm culls whole runs only.
- **Evidence (LOADED; allocation counts load-insensitive):** caret move in a TextArea: 4 KB 1.20 ms / 6,676 allocs; **64 KB 32.77 ms / 106,832**; 256 KB 176 ms / 427,246; 1 MB full raster 573 ms / 1,708,840. About 47% of a 43-char caret frame redoes byte-identical work.
- **Injection:** n/a; the verifier's per-line control shows the cull works when runs are short.
- **Repro / scope:** `d8d328a`; TextArea with real documents.
- **Fix and owner:** rasterise only glyphs meeting the clip; per-line Text commands. **M45**/**M50**.
- **Confidence:** high.

#### R8-F5 — Paste latency is superlinear: a 1 MB paste blocks the UI thread for 8.5 s
- **S2** · perf-claim · (1) · CONFIRMED (S2)
- **Claim:** `Host::paste` chunking rustdoc (no cost stated).
- **Evidence (LOADED):** 16 KB 3.5 ms, 64 KB 48.7, 256 KB 482.5, **1 MB 8,487 ms**. The verifier split it: one pump per 64 characters × O(length) predicts 404.6 against 410.9 ms at 256 KB.
- **Injection:** n/a.
- **Repro / scope:** `d8d328a`; large pastes.
- **Fix and owner:** one edit, one pump with the window's measurer. **M45**.
- **Confidence:** high.

#### R9-F1 — The GPU glyph atlas silently drops glyphs every frame once a frame's glyph set does not fit an empty 1024² atlas
- **S2** (adjusted from S1 by both verifiers) · correctness · (1) · PLAUSIBLE ×2 (S2, S2)
- **Claim:** `results-gpu.md` §4 "the frame is *built again* rather than finishing with glyphs missing"; `lib.rs:74` "far more than a screen has".
- **Evidence:** 4K, 8 sizes × 2 weights × printable ASCII: scale 1 no reset; **scale 2: one reset and one give-up every frame, 10.6% of text ink missing** (275,634 vs 308,404 dark px); with Latin-1 48% missing; q4b 17,489 allocations / 8.8 MB inside one draw. No counter or log reports it.
- **Injection:** `70f4713` (atlas 64²): all parity tests green; parity's text cannot reach the path.
- **Repro / scope:** GPU backend, HiDPI.
- **Fix and owner:** multi-page atlas; a give-up counter; an over-one-page parity case. **M51**.
- **Confidence:** medium. The reproducer matched the pixels exactly; the skeptic found the trigger is fragmentation (64.8% texel use at first failure), not "exceeds 1024²".

#### R9-F2 — `Cmd::Image` is pixel-tested on no renderer's correctness
- **S2** · vacuous-gate · (1) · PLAUSIBLE (S2)
- **Claim:** `parity.rs:31` "A frame using every command in the vocabulary"; `results-gpu.md` §2.
- **Evidence:** no pixel corpus holds an Image. With one added the backends agree at 1:1, 2× and tinted 3×.
- **Injection:** `7415a9d` (neither renderer draws images): 776 tests green across four crates.
- **Repro / scope:** image correctness on both renderers.
- **Fix and owner:** Image in parity; a `blit_image` pixel test; correct the "every command" sentences. **M51**.
- **Confidence:** medium. The verifier found `glasswall`'s `the_wallpaper_is_a_picture` fails under `cargo test --workspace` (range 0 against > 40), so a coarse gate exists; scale, tint and alpha are untested.

#### R9-F3 — The `MAX_DEPS` backdrop fallback to `Damage::All` is unguarded
- **S2** · vacuous-gate · (1) · CONFIRMED (S2)
- **Claim:** `retain.rs:477-486`; `dilate()` rustdoc "A cap with a Damage::All escape would be a branch that cannot fire".
- **Evidence:** 5-panel oracle: base 0 px differ; one frame past the limit at 1080p costs 32.72 ms against 6.41 ms (LOADED).
- **Injection:** `ecaf67f` (fallback removed): every existing suite green; the new `r9_deps` shows **1,822 px** differing.
- **Repro / scope:** `aa76682`; ≥ 5 glass panels.
- **Fix and owner:** land the oracle; separate the two caps in the rustdoc. **M41**/**M51**.
- **Confidence:** high (verifier: workspace 954 green, own oracle 728 px).

#### R9-F4 — The golden-image gate (M12/T12.2) runs 0 tests in every gate
- **S2** · vacuous-gate · (3)/(1) · CONFIRMED (S2)
- **Claim:** `results-harness.md` T12.2 "The gate: change a radius by 1 px".
- **Evidence:** `--workspace` unifies `truetype`; `golden.rs` is `#![cfg(not(feature = "truetype"))]`: 0 tests; per crate: 2 passed.
- **Injection:** `232c1e8` (radius 9): workspace forms "running 0 tests ... ok"; per-crate FAILED at `golden.rs:58`.
- **Repro / scope:** `232c1e8`; the whole T12.2 gate. Same as R17-F1.
- **Fix and owner:** a per-crate step asserting a nonzero count. **M43**.
- **Confidence:** high.

#### R9-F5 — At fractional scale the software canvas draws 1-px dividers up to 1.61× heavier than their geometry
- **S2** · correctness · HiDPI · PLAUSIBLE (S2)
- **Claim:** `results-gpu.md` §2 bound 0.2%; `Scaled` does not snap.
- **Evidence:** scale 1.25: **1,901 px over 8 = 7.92% / 0.20%**; divider ink software 1.61×, GPU 0.93×. Scale 1.5: 4.62%. StrokedRect agrees (shared SDF).
- **Injection:** n/a (unexercised configuration).
- **Repro / scope:** probe q6; every fractional-scale display.
- **Fix and owner:** snap rect edges in `Scaled`; a parity arm at 1.25. **M40**.
- **Confidence:** medium. The verifier confirmed the software heaviness through the real `Scaled` path and disputes the headline ratio as a cross-renderer framing.

#### R9-F6 — The "CPU fallback" does not happen on a driverless machine with Mesa: wgpu opens llvmpipe
- **S2** · perf-claim · (1) · CONFIRMED (S2)
- **Claim:** winit `lib.rs:125-134`; `results-gpu.md` §6.
- **Evidence (8 alternating rounds, load 3.2–3.5):** 1080p full frame llvmpipe median 2.673 ms vs software 0.822 (**3.3×**); caret 0.146 vs 0.037 ms (3.9×); RSS 3.7 → 188.6 MB after `Gpu::headless`, 219.7 after one frame; threads 2 → 133.
- **Injection:** n/a.
- **Repro / scope:** `gpu` + `Prefer::Auto` on any machine whose only adapter is a CPU rasteriser.
- **Fix and owner:** treat wgpu's DeviceType::Cpu as absent under Auto. **M51**.
- **Confidence:** high (verifier reproduced on the windowed path).

#### R9-F7 — The parity bound cannot see a border losing all antialiasing on the documented adapter
- **S2** · vacuous-gate · (1) · CONFIRMED (S2)
- **Claim:** `parity.rs:46-50` "this is what holds them to it"; `results-gpu.md` §2 "~20× headroom".
- **Evidence:** llvmpipe baseline 56 px over 8 (headroom 1.5×, not 20×). Injection-only footprint: **56 px < 86.4 px bound**; on the documented 4-px adapter the total would be ~60 (inferred, not observed).
- **Injection:** `c68c51c`/`d7dd9f4` red on llvmpipe only because of baseline noise; Line AA (`712424a`, 186 px) and gamma (`2fa7eb7`, 3,751 px) red.
- **Repro / scope:** `8633ff2`; border fidelity.
- **Fix and owner:** per-region bounds; per-adapter baselines. **M51**.
- **Confidence:** high.

#### R10-F4 — `Host::paste` ends with a BlockMetrics pump whose shaped memo survives the real frame
- **S2** · correctness · (1) · CONFIRMED (S2)
- **Claim:** shell `lib.rs:1071-1078` "no second edit path to get subtly different".
- **Evidence:** typed field 169 px, pasted 168; a click at x 84.5 puts the caret at 9/19 typed and **19/19 pasted**. 100 kB paste: 58.3 ms, 30 allocations; the next real frame 0.1 ms (the stale memo served).
- **Injection:** `7e0b30d` (no final pump): pasted = typed. Reverted `5420c21`.
- **Repro / scope:** every Ctrl+V. Same defect as R2-F1.
- **Fix and owner:** `paste_with` a measurer. **M45**.
- **Confidence:** high.

#### R10-F5 — Paste drops every newline into a TextArea while reporting it delivered
- **S2** · correctness · (1) · CONFIRMED (S2)
- **Claim / evidence:** as R8-F4 (S1): "line one\nline two" → "line oneline two", returned 17; 100 kB of 40-char lines kept 97,500 of 100,000 bytes. The agent's type path has the same shape.
- **Injection:** n/a. **Fix and owner:** **M45**. **Confidence:** high (verifier added `\r\n`).

#### R10-F6 — `paste_skips_control_characters_but_keeps_newlines` contains no newline
- **S2** · vacuous-gate · (1) · CONFIRMED (S2)
- **Claim:** `clipboard.rs:98-107`.
- **Evidence:** input `"a\tb\u{7}c"` into a TextInput; the TextArea claim is a comment.
- **Injection:** `ec07f29` (filter drops '\n'): 8/8 green (verifier: 168/168).
- **Fix and owner:** paste "a\nb" into a TextArea. **M45**. **Confidence:** high.

#### R10-F7 — Backspace deletes one codepoint, not one grapheme
- **S2** · correctness · (1) · CONFIRMED (S2)
- **Claim:** `book/src/input-widgets.md:57-58` "every path that moves it steps by clusters".
- **Evidence:** `char_indices` everywhere, no grapheme code; End+Backspace across five widgets: "xé"→"xe", ZWJ family → dangling ZWJ, flag → lone indicator: **0 of 15 pass**.
- **Injection:** n/a. **Repro:** `r10_q4`. **Fix and owner:** grapheme stepping. **M45**. **Confidence:** high (verifier: Delete after Right also splits clusters).

#### R10-F8 — Key text with more than one codepoint is cut to its first char
- **S2** · correctness · (1) · CONFIRMED (S2)
- **Claim:** `translate.rs:305-308`.
- **Evidence:** `'é'`(decomposed)→`'e'`, ZWJ family→`'👨'`, flag→`'🇳'`, `'क्ष'`→`'क'`. The verifier notes winit emits two-char text on Windows for a dead key followed by a non-combining character (US-International).
- **Injection:** n/a. **Fix and owner:** route through `compose(Commit)`. **M45**. **Confidence:** high.

#### R10-F9 — No test routes composition through `Host::compose`
- **S2** · vacuous-gate · (1) · CONFIRMED (S2)
- **Claim:** T14.8 DONE; `ime.rs` "the whole model".
- **Evidence:** `ime.rs` calls the inherent methods.
- **Injection:** `125aa35` (both `Node::compose` impls no-ops): 352 green (verifier: workspace 954 green). Reverted `cdb24b6`.
- **Fix and owner:** a Host-level Tab/Preedit/Commit test. **M41**/**M45**. **Confidence:** high.

#### R10-F10 — BakedFace's wrap breaks words mid-word
- **S2** · correctness · (1) embedded · CONFIRMED (S2)
- **Claim:** `font.rs:111-113` "Wraps at the last space that fits".
- **Evidence:** "aaa bbbb" at 60 → "aaa bb|bb"; "Hello world again" at 70 → "Hello w|orld aga|in".
- **Injection:** n/a (verifier's "no line starts mid-word" assertion fails at byte 6).
- **Fix and owner:** move the pending word on wrap. **M45**/**M52**. **Confidence:** high.

#### R10-F12 — The default font stack draws '?' for CJK, Arabic, Hebrew and emoji although installed faces cover them
- **S2** · correctness · (1) · CONFIRMED (S2)
- **Claim:** `plan-desktop.md` T14.8; `text.rs:83-87` "must not render as a box if any face has it".
- **Evidence:** "你好" measures and paints identically to "??"; same for 👍, مرحبا, שלום. NotoSansCJK, DejaVuSans and NotoColorEmoji are installed.
- **Injection:** n/a; the verifier's fallback-face controls show the probe distinguishes.
- **Fix and owner:** per-char fallback faces. **M45**. **Confidence:** high.

#### R10-F13 — Ctrl/Cmd+C, X, V are swallowed even with nothing to copy; NumberInput copies nothing
- **S2** · dx · (1) · CONFIRMED (S2)
- **Claim:** winit `lib.rs:372-377`.
- **Evidence:** `'c'|'x'` return true when `copy()` is None; `'v'` always; CTRL or META on every OS. NumberInput with "42.00", Ctrl+A: `copy()` = None.
- **Injection:** n/a. **Fix and owner:** return false when nothing happens; forward `selection()`. **M45**. **Confidence:** high.

#### R11-F1 — `check_text.sh` does not catch the `f32::clamp`/flt2dec regression it was built after
- **S2** · vacuous-gate · (1)(2) embedded · CONFIRMED (S2)
- **Claim:** `results-embedded.md` "One f32::clamp cost 26.5 KB" / "a closing margin is visible".
- **Evidence:** baseline 27,024 / 65,536 (41.2%), 0 flt2dec symbols. Injected: **48,720 / 65,536 (74.3%), exit 0**, 18 flt2dec/fmt::float symbols.
- **Injection:** `2189c21`.
- **Fix and owner:** a symbol gate, bound 0. **M52**. **Confidence:** high.

#### R11-F2 — "f64 in the frame path: zero" is prose only
- **S2** · vacuous-gate · (1) embedded · CONFIRMED (S2)
- **Evidence:** f64 arithmetic in `Constraints::clamp`: six `__aeabi_d*`/`__*df3` symbols, `.text` 28,642, every gate green.
- **Injection:** `12e6d3d`. **Fix and owner:** the same symbol gate. **M52**. **Confidence:** high.

#### R11-F3 — The stack row reports the model as a measurement
- **S2** · vacuous-gate · (1) embedded · CONFIRMED (S2)
- **Claim:** `results-embedded.md` "816 B at depth 3 ... asserted at compile time".
- **Evidence (qemu mps2-an386, stack painting, calibration 2000 B → 2032 B):** release frame 1,144 B cold / 948 warm against model 816; **dev 6,524 B / 4,096 budget**; slope ~200 B/level release plus a ~400 B base the model omits, ~808 B/level dev.
- **Injection:** `937885b` (4 KB local in `TextInput::layout`): **5,016 B** measured, const assert and every gate green.
- **Repro / scope:** `1364dde` on `c6550f4`. **Fix and owner:** label the row; a qemu stack gate. **M52**. **Confidence:** high.

#### R11-F4 — The `.text` and `.bss` gates price a firmware that never dispatches
- **S2** · vacuous-gate · (1)(2) embedded · CONFIRMED (S2)
- **Evidence:** interactive variant (`69d7d3d`/`83a6fed`): **47,678 B (+88%)**, with queue + Streaming + damage replay 57,076 (87.1%); plus `f32::clamp` 68,092 > 65,536.
- **Injection:** `6697ad2` (static in `pass::dispatch`): `check_bss` green, interactive `.bss` holds it; a static in `paint` is caught.
- **Fix and owner:** gate an interactive firmware. **M52**. **Confidence:** high (verifier: typed-character dispatch alone +61%).

#### R11-F5 — `const-budget`'s 5 s timer measures an incremental no-op
- **S2** · vacuous-gate · (2), iteration · CONFIRMED (S2)
- **Claim:** `results-const-eval.md` "Current: 0.13–0.21 s".
- **Evidence:** the recipe's rebuild: 141–176 ms (the doc's figure); `CARGO_INCREMENTAL=0` 325–386; rustc `type_check_crate` ≤ 0.119 s. `count_wid` is never evaluated by the fixture.
- **Injection:** `b634311` (O(n²) splice): red only through rustc's lint (2.8–3.9 s); with the lint allowed **50,832 ms, then 181 and 207 ms green**.
- **Fix and owner:** non-incremental timing; a `wid_index!` use. **M52**/**M29** T29.2. **Confidence:** high (verifier: 53.8 s then green).

#### R12-F1 — `just release-shape` compiles core with `observe` on in its test and clippy steps
- **S2** · vacuous-gate · (2) · CONFIRMED (S2)
- **Claim:** `justfile:30-31` "This is the ONLY build that compiles what docs/verification-notes.md scopes its argument to".
- **Evidence:** the `lumen_static` lib rustc line under `cargo test -p lumen-static` carries `observe` and `std`, even with `--no-default-features`; cause `Cargo.toml:52` dev-dep on widgets[observe].
- **Injection:** `520ed2e` (`compile_error!` under observe): test and clippy red; build, miri-smoke, const-budget green. Reverted `759675e`.
- **Fix and owner:** a separate test crate or a tripwire; correct `justfile:30`. **M43**. **Confidence:** high.

#### R12-F2 — No gate executes core's no_std runtime paths
- **S2** · vacuous-gate · (2), embedded · CONFIRMED (S2)
- **Evidence:** only a crate with no dev edge runs the branch (widgets tests at the alloc tier): 1000-byte label stores 256.
- **Injection:** `fa64952` (no_std `format_into` stores 0 bytes): core 423 passed, thumb check and clippy, embedded tests all green.
- **Fix and owner:** a no-dev-edge test crate; a `#[bind]` in the embedded fixture. **M43**. **Confidence:** high.

#### R12-F4 — Naming cascade or dispatch in firmware links every widget's interaction methods: +15.7 KB for a style-only firmware
- **S2** · perf-claim · (1)(2) embedded · PLAUSIBLE (S2)
- **Claim:** `results-widget-crate.md` "the linker ... drop[s] what a tree does not name"; `results-traversal.md` fuse; `results-competitive.md` §7 "a vtable in every build".
- **Evidence:** base 27,024; one dispatch 42,846 (+15,822); `cascade_deep` only 42,718 vs 27,008 (+15,710, +58%), of which 5,618 B are methods cascade never calls; fuse saves 32 B. `rdyn_opaque` adds +10,750.
- **Injection:** `0d0ce1a`: paint-only +0 B, dispatching +28,706 B.
- **Fix and owner:** a narrower per-pass view. **M52**/**M35**.
- **Confidence:** medium. The verifier reproduced to the byte and found the actual cause narrower: a no-alloc `cascade_deep_with` loop that does nothing still calls through `node(i)`.

#### R13-F2 — `app.perf` reports 0 ns per stage in every turnkey window, and `ui_check` passes budgets on the zeros
- **S2** · perf-claim · (3)(1) · CONFIRMED (S2)
- **Evidence:** winit `lib.rs:461` passes `FrameTiming::default()`. showcase `app.perf`: all zeros; `ui_check`: "frame p95 0.00 ms (ceiling 16)" and "regress 0.000 ms (0.18)" pass; latency FAILs on nan.
- **Injection:** n/a (quantity beside bound). The live gate is unaffected (counter-window feeds real timings).
- **Fix and owner:** real FrameTiming, or null plus NOT MEASURED. **M48**. **Confidence:** high.

#### R13-F3 — Input replies and `ui.lastDamage` count frames differently
- **S2** · correctness · (3) · CONFIRMED (S2)
- **Evidence:** `input.click` → frame 24; `ui.lastDamage{since:23}` → agent frame 19, empty, while `#select-menu` is 300×560. Host counter advances twice per RPC, `Agent::frame` once per redraw.
- **Injection:** n/a (reply beside ground truth; verifier: fresh windows happen to line up).
- **Fix and owner:** one counter. **M48**. **Confidence:** high.

#### R13-F4 — No agent method reports a checkbox's checked state or a slider's value
- **S2** · a11y · (3) · CONFIRMED (S2)
- **Evidence:** 101 live `d_*` nodes: bounds agree 101/101; CheckBox, Switch, Radio, Slider have value None; 17 kinds are GenericContainer with no text. A checkbox click changed the app readout and left `ui.resolve` byte-identical.
- **Injection:** n/a. **Fix and owner:** serialise toggled/numeric/hidden; observe impls. **M48**/**M54**. **Confidence:** high.

#### R13-F5 — `live_nodes`' "residue" case is buildable: `Glass<Glass<Label>>` is reported hidden and `lastChange` misses its text
- **S2** · correctness · (3) · CONFIRMED (S2)
- **Claim:** agent `lib.rs:113-114` "needs a type to contain itself".
- **Evidence:** `live_nodes` [true, true, false, false]; `lastChange` after `set_str` → **0 / 1 expected**; single Glass control → 1. Verifier: `Card<Card<…>>` too.
- **Injection:** the probe itself; `2b21ded` confirms kind is load-bearing.
- **Repro:** `c9d6760`. **Fix and owner:** structural stand-in detection. **M48**/M35. **Confidence:** high.

#### R13-F6 — Session replay checks no step and never compares the stylesheet
- **S2** · vacuous-gate · (3) · CONFIRMED (S2)
- **Evidence:** a corrupted recording replays Ok with n=1 of 3; a mismatched stylesheet replays Ok; the exported test passes with `#does-not-exist`.
- **Injection:** `580f9f6`. **Fix and owner:** Err on divergent replies; check the stylesheet; assert in `export_test`. **M48**. **Confidence:** high.

#### R13-F7 — `cargo test -p lumen-static-agent` fails 19 tests
- **S2** · config · (3)(2) · CONFIRMED (S2)
- **Evidence:** `audit_is_inert` 0/3, `input` 14/19, `protocol` 6/9, `rows` 2/10; dev-dep lacks `observe`; with it all pass.
- **Injection:** building the crate alone. **Fix and owner:** add the feature; a per-package step. **M43**. **Confidence:** high. (R3-F8 records the same fact at S3.)

#### R13-F8 — Removing the redraw after an agent request leaves the journal stale, and nothing notices
- **S2** · vacuous-gate · (3) · CONFIRMED (S2)
- **Evidence:** `harness.py:386` `if mine:` skips the damage bound when no region overlaps.
- **Injection:** `1beed32`: 22/22 winit tests green, widget-drive exit 0 with **0 "damage:" lines** (2 without the injection).
- **Fix and owner:** fail on empty regions for repainting actions. **M42**. **Confidence:** high.

#### R14-F2 — A non-loopback bind is accepted silently
- **S2** (adjusted from S1 by both verifiers) · security · (3) · CONFIRMED + PLAUSIBLE (S2, S2)
- **Evidence:** `LUMEN_AGENT=0.0.0.0:7614`: LISTEN 0.0.0.0, the LAN address answered with the tree. Not tested from a second machine.
- **Injection:** n/a. **Fix and owner:** refuse without an override. **M39**. **Confidence:** high on the behaviour; every documented address is loopback, hence S2.

#### R14-F3 — One idle client blocks every other; a line has no length cap
- **S2** · security · (3) · CONFIRMED (S2)
- **Evidence:** second client timed out at 5.01 s; 1 GiB line: RSS 46,184 → **1,098,864 KiB**.
- **Injection:** `f927617` red (idle 3.04 s / 3 s; 257 MiB / 16 MiB); `4ac7e2d` green (363 µs; 1 MiB).
- **Fix and owner:** thread per connection, line cap. **M39**. **Confidence:** high.

#### R14-F4 — "Zero allocations per `frame_done`" has read 4.00 since `075bec1`, and nothing asserts it
- **S2** · vacuous-gate · (1)(3) · CONFIRMED (S2)
- **Evidence:** `benches --bin agent` 4.00 / 0, all from `live_nodes` (~45 KB/frame at 901 nodes).
- **Injection:** `2a60b9c` (reuse buffers) → 0.00.
- **Fix and owner:** reuse, and assert in `just test`. **M48**/M35. **Confidence:** high.

#### R15-F1 — No turnkey app is visible to a screen reader
- **S2** · a11y · desktop; (3) · CONFIRMED (S2)
- **Claim:** handoff `:626`, `:704` "one tree for screen readers and agents".
- **Evidence:** AT-SPI shows counter-window `--features a11y` (9 named nodes); showcase is absent from the 34-entry application list. accesskit comes only through `observe`.
- **Injection:** n/a. **Fix and owner:** an `a11y` feature in winit, or correct the handoff. **M54**. **Confidence:** high.

#### R15-F2 — Only Button is a named accessible control
- **S2** · a11y · desktop/mobile · CONFIRMED (S2)
- **Evidence:** 20 of 45 Node impls without `observe`; `set_selected`, `set_disabled`, `set_description`, `labelled_by`: 0; `add_action`: 2.
- **Injection:** n/a. **Fix and owner:** labels, roles, actions. **M54**. **Confidence:** high.

#### R15-F3 — No gate covers any widget's accessibility role or state
- **S2** · vacuous-gate · a11y · CONFIRMED (S2)
- **Injection:** `d85e851` (Toggle `set_role` deleted): identical failure set (19 = 19); verifier's stronger injection (role and toggled): workspace 954/0.
- **Fix and owner:** a table-driven role snapshot. **M54**. **Confidence:** high.

#### R15-F4 — A touch-only input stream reaches no widget
- **S2** · correctness · mobile, touch desktop · CONFIRMED (S2)
- **Evidence:** pointer taps 3/3; touch taps **0/3**. winit Wayland and Android emit only `Touch`.
- **Injection:** n/a (verifier confirmed hits land). **Repro:** `85ad947`. **Fix and owner:** promote the primary touch. **M44**. **Confidence:** high.

#### R15-F5 — T15.7 (platform half), T15.10 and T15.11 are DONE but their gates never ran
- **S2** · doc-drift · desktop · CONFIRMED (S2)
- **Evidence:** 0 callers of `translate::dropped`, `Pane`/`Panes`, `Dialogs`; no tests; `run()` discards file drops; winit delivers N Hovered then N Dropped or 1 Cancelled.
- **Injection:** n/a. **Fix and owner:** assemble each with a gate, or re-mark. **M54**. **Confidence:** high.

#### R15-F8 — "`Drag<()>` is one byte" is false (24 B), and its test stays green at 1,024 B
- **S2** · vacuous-gate · (2) · CONFIRMED (S2)
- **Injection:** `45fa10a` (+1000 B): 8/8 green. **Fix and owner:** assert 24. **M54**. **Confidence:** high (the relative assertion still guards against type erasure).

#### R16-F1 — W1002 (OFFSCREEN) has no test in either direction and fires on healthy ScrollView content
- **S2** · vacuous-gate · (3) · CONFIRMED (S2)
- **Evidence:** 2 findings on an unscrolled ScrollView, 1 scrolled, bound 0.
- **Injection:** retain-filter W1002: 150/150 green (verifier: removing the condition, workspace 956 green; 7 false findings). **Fix and owner:** **M55**. **Confidence:** high.

#### R16-F7 — TextArea re-shapes the whole document per keystroke, with superlinear growth
- **S2** · perf-claim · (1) · PLAUSIBLE (S2)
- **Evidence (LOADED):** 1 KB 0.038 ms, 100 KB 29.2 ms, 1 MB 2,409 ms per keystroke.
- **Injection:** n/a. **Fix and owner:** **M45**.
- **Confidence:** medium. The verifier found the superlinear part is a quadratic `line_of` scan in `shape_into`; a monotonic cursor gives 1 MB 2.1 s → 75 ms and 100 KB 25 → 7 ms.

#### R16-F8 — Nothing checks painted colour
- **S2** · vacuous-gate · widgets, a11y · CONFIRMED (S2)
- **Injection:** `fd31a02` (selected nav label in the pill colour): 147/147 unit tests, widget-drive BottomNav (E) and NavigationRail (B) 0 failed.
- **Fix and owner:** a painted-contrast check. **M54**. **Confidence:** high.

#### R17-F1 — The golden-image gate runs in no recipe and no CI job
- **S2** · vacuous-gate · (1) · CONFIRMED (S2) — same mechanism as R9-F4.
- **Injection:** `6881359` (`compare` always Match): unified 0 tests green; shell alone 1 failed. **Fix and owner:** **M43**. **Confidence:** high.

#### R17-F6 — With no GPU adapter the parity gate reports 6/6 ok, and the skip notice is invisible
- **S2** · vacuous-gate · (1) · CONFIRMED (S2)
- **Evidence:** adapter removed: 6 ok in 0.01 s; the notice appears only with `--nocapture`. `parity.rs:65` "so a skipped test does not look like a passing one".
- **Injection:** environmental. **Fix and owner:** LUMEN_REQUIRE_GPU. **M43**. **Confidence:** high.

#### R17-F8 — The milestone-defining live gates are manual-only, and all five exit 0 without an X display
- **S2** · vacuous-gate · (3) · CONFIRMED (S2)
- **Evidence:** widget, agent, tasks drive, wake-gate, gpu-parity exit 0 with DISPLAY unset, including with WAYLAND_DISPLAY set; none in `test`/`check`/CI.
- **Injection:** DISPLAY removed. **Fix and owner:** skip = 77, refused by `check`; xvfb. **M42**. **Confidence:** high.

#### R17-F10 — On a clean runner ci.yml's gate job fails at `just thumb`
- **S2** · config · process, embedded · CONFIRMED (S2)
- **Evidence:** `rust-toolchain.toml` has no `targets`; clean-sysroot simulation: E0463 "can't find crate for `core`".
- **Injection:** simulated sysroot. **Fix and owner:** add the target. **M43**. **Confidence:** high (earlier failures on a real image not verifiable).

#### R18-F1 — widget-drive cannot tell which binary or strategy it drove
- **S2** · vacuous-gate · (1) via M34 · CONFIRMED (S2)
- **Evidence:** real builds hash `c54937d6…` and `f48fbd38…`.
- **Injection:** I5a (`1a1f478`, same build both legs): 94/94 both, same sha256. Leftover process on 7618 (`3d3333a`): both new binaries logged "cannot bind", both drives drove pid 3845992, exit 0.
- **Fix and owner:** strategy, features, pid in `app.diagnostics`; refuse a pre-bound port. **M42**, prerequisite of M34. **Confidence:** high.

#### R18-F2 — The drive harness's coverage assertion cannot fire
- **S2** · vacuous-gate · process · CONFIRMED (S2)
- **Evidence:** `harness.py:431-433` compares the inventory with itself; 17+ exported widgets have no inventory (Select, TextArea, ScrollView among them).
- **Injection:** I5b/I5c (`9623a3e`): green at 38/92. **Fix and owner:** pinned counts; `pub use` check. **M42**. **Confidence:** high.

#### R18-F4 — The allocation gate cannot see the threaded rasteriser
- **S2** · vacuous-gate · (1)(2) · CONFIRMED (S2)
- **Evidence:** 1920×1080, 8 bands: 25 allocs/frame on the frame thread (3w+1); `alloc_gate` fixtures never band.
- **Injection:** I2 (`045e0b3`): TLS 25/frame, process-wide 57/frame, `alloc_gate` 11/11 green.
- **Fix and owner:** a banding arm with a process-wide counter, or a persistent pool. **M50**. **Confidence:** high.

#### R18-F5 — Surviving mutant: `Regions::push` may drop the new region when full
- **S2** · vacuous-gate · (1) · CONFIRMED (S2)
- **Evidence:** the test bounds area from above; dropping a region passes more easily.
- **Injection:** push-1 (`7a03a6d`): four-crate suite green; coverage probe (`a229249`) finds 1 of 5 uncovered. **Fix and owner:** a coverage assertion. **M41**. **Confidence:** high.

#### R18-F9 — `live_gate` exits 0 when the app panics after the checks
- **S2** · vacuous-gate · (3) · CONFIRMED (S2)
- **Injection:** I6 (`481773e`): rc=0 with "panicked at ... window.rs:131". **Fix and owner:** drop `|| true`. **M42**. **Confidence:** high.

### 2.4 S3 and S4

One row each: the claim that failed, the evidence or injection that
failed it, the verdict, and the owner.

| ID | sev | cat | finding | evidence / injection | verdict | owner |
|---|---|---|---|---|---|---|
| R1-F7 | S3 (from S2) | vacuous-gate | The host line that hands the display scale to the text memo (T26.17 #12) has no gate | d2 (`ed94b88`) removes `.with_scale(self.scale)`: 317 shell+widgets tests green; `measure_scale.rs` builds its own LayoutCx | PLAUSIBLE: a working gate is easy to write; latent | M40 |
| R2-F4 | S3 | perf-claim | "Length plus a finaliser" does not remove same-length collisions; a word mix does at +0.14–0.18 ns per FoldedText hit | `b546755`; 60 B: today 22.00 ns, len+fin 24.72, mix8 3.39 (LOADED, pinned) | PLAUSIBLE: mix8 separates the solved pair, not all same-length collisions | M46 |
| R2-F7 | S3 | dx | `Host::invalidate`/`restyle_subtree` return true on a duplicated id and act on one instance; debug `restyle_subtree` panics blaming the sheet | `9823e46` (`r2_dup.rs`): node 2 dirty, node 4 clean | CONFIRMED | M46 |
| R2-F8 | S4 | doc-drift | `expand_enum`'s rustdoc says an enum contributes one table node | `macros lib.rs:1304` vs `:1356` (F4.3) | CONFIRMED | doc pass |
| R2-F9 | S4 | correctness | `Assembled` truncates `child_count` to u16 and is never checked | `concat_table(70000)` → child_count 4464, wf=false (`f7d5def`) | CONFIRMED | M46 |
| R2-F10 | S4 | doc-drift | `MeasureKey` compares limits with f32 `PartialEq`, so NaN limits always miss | 5 layouts, 5 measurer calls (`3cc8149`) | CONFIRMED | M45 |
| R3-F8 | S3 (from S2) | vacuous-gate | 19 agent tests pass only through workspace feature unification | `-p lumen-static-agent`: 19 red; `30a1b0f` adds observe → green | PLAUSIBLE: tests are real in every gate that runs; see R13-F7 | M43 |
| R3-F9 | S3 | a11y | Arrow groups larger than 16: members 17+ unreachable and a dead key | 16 of 20 visited; `heapless_vec::Small` CAP=16 | CONFIRMED | M44 |
| R3-F10 | S3 | doc-drift | Past the addressable ceiling rows overlap and probe disagrees with paint; no debug_assert exists | 50M×20 px: 14/20 rows probe elsewhere, up to 72 px | CONFIRMED | M44 |
| R3-F11 | S4 | doc-drift | Shift+Tab "not implemented" above its implementation; "the queue has no cap" | `pass.rs:419-430`; agent `lib.rs:702` | CONFIRMED | M44 |
| R3-F12 | S3 | perf-claim | Every event sweeps the state array twice; moves not merged | 1,685 nodes: 2.6–4.4 µs/move (LOADED); hit test flat at 35.3 ns | PLAUSIBLE (S3/S4): the quoted claim is about handler calls | M53 |
| R4-F5 | S3 | safety | The loader destroys v1 before checking v2 | missing `hot_step` → exit 101 after `hot_drop`; v2 panics → SIGABRT 134; `d2b71c9` ABI swap → SIGSEGV 139 | CONFIRMED | M49/M36 |
| R4-F6 | S3 | safety | `hot_restore(p, null, 0)` is UB | debug abort on `from_raw_parts` precondition; release returns 1; miri transcription flags it | CONFIRMED | M49 |
| R4-F7 | S3 | doc-drift | "Swap the dylib, and the identity survives" fails for a shape change | `#check` node 4/6 (v1, v2) → 2/7 (v3), `3e4e032` | CONFIRMED | M37 docs |
| R4-F8 | S3 | vacuous-gate | CI miri job cannot reach the only unsafe code | miri-smoke does not depend on `lumen-static-reload` | CONFIRMED | M49 |
| R5-F1 | S3 (from S2) | correctness | A reload keeps code-authored values (Button label, Label text, Repeat count/extent), 0 log lines | `204955c`: count 18 vs 19, label "Save" vs "Submit" | PLAUSIBLE: `#[reload(fresh)]` exists and is documented; a DX gap | M49 |
| R5-F3 | S3 | correctness | Option-typed keep fields restore None over the fresh value on NaN or save failure | `79b0c1f`: Some(NaN)→None, 0 restore log lines | CONFIRMED | M49 |
| R5-F4 | S3 | doc-drift | A kept struct that gains a field loses all old subfields | Settings{a:7,b} → {0,"",false}, 1 log line | CONFIRMED | M49 |
| R5-F5 | S3 | correctness | counter-window's `Application::style` is empty; a Program without `.style()` shows no sheet until reload | card bg (0,0,0,0) vs (22,28,40,255); `3df7fb7` pixel [255,…] vs [200,30,30] | CONFIRMED | M32 |
| R5-F6 | S3 | vacuous-gate | The adopt-oracle's "remove mark_all" injection cannot fire; no repo test runs adopt under Boundaries | injection 1 green (0 px); injection 2 red (3560 px) | CONFIRMED | M41 |
| R5-F7 | S3 | dx | Reloadable derive errors point at the derive and never mention `#[reload(..)]` | showcase: 16 errors, all spanned on the derive (`1a3a4bb`) | CONFIRMED | M49 |
| R5-F8 | S3 | config | With reload off, an app using the derive does not build without `cfg_attr` everywhere | E0433 ×12; `5007248` | CONFIRMED | M49 |
| R5-F9 | S4 | doc-drift | `event.rs:562-564` says any App+Default is refused; step4 honesty items 2–3 stale | Tally, Show compile | CONFIRMED | M32 |
| R5-F10 | S4 (from S3) | doc-drift | M32 removes `Application::style`/`build` but `Host::adopt` and the derive's `fresh()` depend on them | shell `lib.rs:172/217`; `macros :1234` | CONFIRMED: a compile error, cannot ship silently | M32/T32.0 |
| R6-F4 | S3 | vacuous-gate | The stages arm's "first six are exactly what pump_with does" is stale; its gap has no bound | I3 green (62.83 inside 58.52–65.10) | CONFIRMED | M53 |
| R6-F5 | S3 (from S2) | config | `simd`/`threads` are unreachable from winit's features; the turnkey 4K frame misses 16.7 ms | scalar 4K 19.72 ms vs simd+threads 2.55 (LOADED) | PLAUSIBLE: a direct shell dependency line unifies them (counter-window does) | M50 |
| R6-F6 | S4 | config | `benches-competitive/Cargo.lock` stale since M30 | +12 lines on any cargo run | CONFIRMED | M43 |
| R7-F8 | S3 | correctness | A NaN dt leaves an Animated unsettled forever | 10,000 ticks after NaN: settled=false; `LongPress` same shape | CONFIRMED | M47 |
| R7-F9 | S3 | doc-drift | An unmodelled job panic leaves no trace; the documented counter does not exist | all host counters 0 | CONFIRMED | M47 |
| R7-F10 | S4 (from S3) | correctness | A huge accepted period panics `Duration::from_secs_f32` in `about_to_wait` | 1e30 and f32::MAX panic (`c8dfdbc`) | CONFIRMED | M47 |
| R7-F11 | S4 | doc-drift | `results-frame-cost.md` has no idle figure though four comments cite one | 0 grep hits in 203 lines | CONFIRMED | M47 |
| R8-F2 | S3 (from S2) | perf-claim | Banded raster re-shapes straddling runs (2.46× at 1080p, 4.09× at 4K) | threaded 0.73 vs 0.45 ms (1080p) on 50 runs | PLAUSIBLE: the verifier's fixture is 2–3.5× faster threaded; §4's sentence is imprecise | M50 |
| R8-F6 | S3 (from S2) | perf-claim | Software glass blur is bounded by the panel, not the damage | 24×24 render_within 592 µs vs whole 3,921 µs | PLAUSIBLE: the real frame never passes a 24×24 box; the whole panel repaints | M51 |
| R8-F7 | S3 | perf-claim | The pixel buffer never shrinks; a 1 px grow at 4K doubles it | cap 66,355,200 after shrink to 1080p | CONFIRMED | M50 |
| R8-F8 | S3 | vacuous-gate | A shadow ignoring PushClip is caught only by `damage_oracle` | `c472f80`: clip_test 4/4 green, damage_oracle 3 red | CONFIRMED | M50 |
| R8-F9 | S4 | doc-drift | `surface.rs` present p95 disagrees with its cited doc; results-appearance prices stale sizes | 0.044 vs 0.039 ms; 148 vs 164 B/node | CONFIRMED | doc pass |
| R8-F10 | S4 | perf-claim | Idle memory is dominated by fontdue's parsed faces, unattributed | 8.61 MB heap per 0.51 MB face | PLAUSIBLE: attribution gap confirmed, figure not re-run | M50 |
| R9-F8 | S3 | perf-claim | The GPU frame path allocates 62–410 times per frame, ungated | caret 92, full 410, empty list 62 | CONFIRMED | M51 |
| R9-F9 | S3 | perf-claim | Two full-surface glass scratch textures are allocated whether or not glass is drawn | 16.6 MB at 1080p by arithmetic | CONFIRMED | M51 |
| R9-F10 | S3 | vacuous-gate | `svg.rs` "whole vocabulary" test omits StrokedRect and Glass | `c75d6c4`: 170/170 green | CONFIRMED | M51 |
| R9-F11 | S4 | doc-drift | `glass-cost`'s "the bevel" row no longer measures the bevel | median 42.5% / 34% vs §5b's 18% / 5% (LOADED) | CONFIRMED | doc pass |
| R9-F12 | S4 | doc-drift | `results-gpu.md` §7 says the crossover is not located; §5b locates it | `:371-373` vs `:286-297` | CONFIRMED | doc pass |
| R10-F11 | S3 | vacuous-gate | shaping.rs's RTL test stays green when RTL is shaped LTR | `ca8547c` green; no Arabic test | CONFIRMED | M45 |
| R10-F14 | S3 | dx | No word motion, no double/triple-click selection, TextArea has no Home/End | `EventCtx.clicks` read by no widget | CONFIRMED | M45 |
| R10-F15 | S3 | correctness | NumberInput refuses IME composition | Preedit and Commit accepted false | CONFIRMED | M45 |
| R11-F6 | S3 | vacuous-gate | `check_text.sh`/`check_bss.sh` read `./target` and pass a stale binary under CARGO_TARGET_DIR | static in paint: both exit 0 with the env var | CONFIRMED | M43 |
| R12-F3 | S3 (from S2) | dx | `winit/agent` without `widgets/observe` reports every widget as GenericContainer, silently | `3635061` both directions | PLAUSIBLE: the seam is documented in `results-widget-crate.md` | M55 |
| R12-F5 | S3 | config | `lumen-static-winit/boundaries` is non-additive: spelling `Everything` breaks under unification | E0308 in `r12_inval.rs` | CONFIRMED | M55 |
| R12-F6 | S4 | doc-drift | `mobile-decision.md` says core `cargo check` for Android cannot complete; it completes | core, widgets, shell Finished; winit needs `android-native-activity`, fails with `clipboard`/`dialogs` | CONFIRMED | M54 |
| R12-F7 | S4 | doc-drift | The widgets Cargo.toml features comment has lost its identifiers | `cat -A` shows "#  implies ," | CONFIRMED | doc pass |
| R13-F9 | S3 | dx | `agent_client.py quit` reports success against windows with no `app.quit` | -32601 turned into `{"ok": true}` | CONFIRMED | M48 |
| R13-F10 | S3 | dx | Launchers and clients read different env vars for the agent address | `LUMEN_AGENT` vs `LUMEN_AGENT_ADDR` across 6 sites | CONFIRMED | M48/M32 |
| R13-F11 | S4 | doc-drift | Stale agent module and trait docs (in-process only, no input, no AgentHost impl, no scheduler) | `lib.rs:5-8, 24-25, 787, 1370` | CONFIRMED | M48 |
| R14-F5 | S3 | safety | A panic in dispatch kills the whole application | `e00c288`: exit 101; 1,250 hostile requests found no real panic | CONFIRMED | M39 |
| R14-F6 | S3 | correctness | A trailing `>` garbles error messages and amplifies the reply 15× on the UI thread | 10 MB → 150 MB, 3.45 s | CONFIRMED | M39 |
| R14-F7 | S3 | correctness | JSON-RPC envelope errors use -32601; no batches; notifications answered; non-finite coords echoed null | live replies; no NaN state reached | CONFIRMED | M39 |
| R14-F8 | S3 | perf-claim | An agent-feature build runs `frame_done` every frame with no endpoint; no desktop gate checks 0 agent symbols | 3/3 startup frames; 0/47/122/22 symbol counts | CONFIRMED | M35/M48 |
| R15-F6 | S3 | correctness | A second `resumed` loses `Config::images` | 99.0% of pixels change, colours 85,279 → 1,771 (`e5da538`) | CONFIRMED | M32/M47 |
| R15-F7 | S3 | correctness | Window cursor ignores repeated rows | row Button → Default, table Button → Pointer (`07eb009`) | CONFIRMED | M44 |
| R15-F9 | S4 | doc-drift | `image.rs` cites a shell `image` feature that does not exist | cargo: "does not contain this feature" | CONFIRMED | doc pass |
| R15-F10 | S3 | doc-drift | The agent tree drops toggled, numeric, expanded, modal, hidden; two serialisers | `observe.rs:131-168` vs agent `lib.rs:798` | CONFIRMED | M48 |
| R16-F2 | S3 | vacuous-gate | W1003's reservation rationale is false; its tripwire has no ScrollView | child bottom 74 / parent 40 | CONFIRMED | M55 |
| R16-F3 | S3 | correctness | The App derive's `changed` local is visible to bind expressions | `#[bind(.., changed)]` prints "3 false" | CONFIRMED | M33/T33.1 |
| R16-F4 | S3 | dx | Attribute misuse accepted silently; six derive error paths untested | `11d3b34`: second-`#[tree]` and columns=0 removals green | CONFIRMED | M55 |
| R16-F5 | S3 (from S2) | correctness | One non-finite style value blanks the tree silently | spacing=NaN: 5/6 non-finite, 0 commands, click misses | PLAUSIBLE: severity only | M55 |
| R16-F6 | S3 | correctness | Negative padding lets children escape and become unclickable | buttons at (-50,-10); click misses | CONFIRMED | M55 |
| R16-F9 | S3 | a11y | Pagination digits 4.12:1 and Ghost 3.91:1 below 4.5:1; accent 4.4988:1 | 31 of 428 runs below 4.5 (`efc734f`) | CONFIRMED | M54 |
| R16-F10 | S3 | doc-drift | The book's gap list and command count stale; attribute tables omit wrap, grid, columns, tick | 11 commands, not nine; 69 of 91 widget types never named in the book | CONFIRMED | M55 |
| R16-F11 | S4 | correctness | NaN step resets NumberInput/RangeSlider; min > max pins to min; SplitPane stores NaN | `1033efb` | CONFIRMED | M55 |
| R17-F2 | S3 (from S2) | vacuous-gate | `check_no_unsafe` misses unsafe after `//` in a string; only widgets forbids unsafe | `6105909`: gate exit 0, compiles | CONFIRMED | M43 |
| R17-F3 | S3 | vacuous-gate | `check_doc_paths` accepts dependency names as `--bin`, never checks `-p`, skips book/AGENT.md | 4 of 6 plants green | CONFIRMED | M43 |
| R17-F4 | S3 | vacuous-gate | `check_attr_table` never reads the derive | `ghostr17` plant green | CONFIRMED | M43 |
| R17-F5 | S3 | vacuous-gate | `check_derive_attrs` drops multi-line lists; a comment counts as a use | 7 attributes checked fall to 4 | CONFIRMED | M43 |
| R17-F7 | S3 | vacuous-gate | Without a system font, 15 text tests pass doing nothing, 7 silently | fonts hidden: 911/911 green | CONFIRMED | M43 |
| R17-F9 | S3 | vacuous-gate | `check_bss` passes with 0 symbols when nm fails | `5af3aee` + failing nm: exit 0 | CONFIRMED | M43 |
| R17-F11 | S3 | doc-drift | Five `ci.yml` comments are false | "six crates" lists 7, lock has 393; std::fs present; 8 bench bins | CONFIRMED | M43 |
| R17-F12 | S3 | config | benches-competitive lock stale; no gate builds it; no `--locked` | `--locked` fails; 0 `--locked` uses | CONFIRMED | M43 |
| R17-F13 | S3 | doc-drift | `results-embedded.md` headline 20,524 / "now 22,458" while measured 27,024 | `size -A` at `ac91d51` | CONFIRMED | M52 |
| R17-F14 | S4 | doc-drift | justfile dependency counts stale | 106 → 170; wgpu 60 | CONFIRMED | M43 |
| R17-F15 | S3 | vacuous-gate | `check_doc_identifiers` misses trailing and block comments and AGENT.md; 8 of 76 allowlist entries stale | 3 evasions green | CONFIRMED | M43 |
| R17-F16 | S4 | vacuous-gate | The book's `rust,ignore` ban misses tilde and indented fences | 2 evasions green | CONFIRMED | M43 |
| R18-F3 | S3 | doc-drift | "Three assertions per action": 31/94 have no isolate, 23/94 skip the landed check | `c9171b6` instrumented | CONFIRMED | M42 |
| R18-F6 | S3 (from S2) | vacuous-gate | Surviving mutant: `Boundaries::mark` loses an absorbed range's end | mark-2 (`1cb070a`): suite green; probe red at `is_clean(5..6)` | PLAUSIBLE: shipped code correct; a missing test | M41 |
| R18-F7 | S3 | vacuous-gate | Overlay stacking order untested; `escape_closes_the_topmost` has one overlay | dispatch-2 (`7f47109`), dispatch-4 (`78e8e07`) survive | CONFIRMED | M44 |
| R18-F8 | S3 | vacuous-gate | The invalidation oracle's headline test does not check its clicks land | I4a (`9a74944`): count -1, headline green | CONFIRMED | M41 |
| R18-F10 | S3 | vacuous-gate | `simd_shadow` cannot tell whether SIMD compiled | I8 (`deeea8b`): 2/2 green, clippy clean | CONFIRMED | M50 |
| R18-F11 | S4 (from S3) | doc-drift | The adversary ran for 1 of 9 M27 tranches, not "once per tranche" | commit-body grep | CONFIRMED | M42 |

## 3. Claims ledger

Every doc or comment claim a reviewer tested. Status: **held**,
**false**, **overstated**, **not executable here** (§6 says what each
needs). A claim marked false or overstated names the finding that
overturned it; the follow-up commit that fixes the finding must fix the
doc in the same commit (doc-currency rule).

### 3.1 Docs and markers that no longer hold

These are the handoff markers and results-doc figures the review
overturned. Each follow-up milestone owns the edit.

| doc | statement | now | finding |
|---|---|---|---|
| `docs/00-handoff-static-tree.md` §10 Step 4 | "DONE ... Live dylib swap demo passes the full §7 sequence"; §1 "supports hot reload during development" | headless fixture only | R4-F1 |
| handoff §9, `:626`, `:704` | Step 5 DONE: "one tree for screen readers and agents" | agent only; screen readers only in counter-window `--features a11y` | R15-F1 |
| handoff §9 / `book/src/agent.md:94-96` | "the node ID is still valid (it is an offset, not a generated key)" | not across a shape change | R4-F7 |
| handoff "What survives from §3" | "the gate covers a rasterised frame" | block glyphs only | R8-F1 |
| handoff §10 | "Zero allocation for the entire UI" | truetype raster, threads banding, GPU frame path and `frame_done` all allocate | R8-F1, R18-F4, R9-F8, R14-F4 |
| `docs/results-allocation.md` T26.13 | raster "after: 0" | 25.80/frame on truetype | R8-F1 |
| `docs/results-threading.md` §4 and `:208-209` | "Once strips stop re-shaping"; "alloc_gate asserts zero steady-state allocation" | 2.46×/4.09× re-shape; 3w+1 allocs/frame | R8-F2, R18-F4 |
| `docs/results-competitive.md` §5b | "Under `Boundaries` that frame is the 0.01 µs row" | ~111 µs pump at 10k | R6-F1 |
| `docs/results-competitive.md` §3 | smallest resident, attributed to GPU contexts | dominated by fontdue faces | R8-F10 |
| `docs/results-competitive.md` §7 | "a vtable in every build" | not in paint-only firmware | R12-F4 |
| `docs/results-damage-precision.md` T29.1 | "20.7 µs, 36% of the frame" | ~25.5% of the pump | R6-F2 |
| `docs/results-tasks.md` T28.11/T28.12 | "zero threads for a timer"; tasks-drive proves `run()`'s waker and `WaitUntil` | a thread per firing; gate vacuous; timers never fire idle | R7-F5, R7-F2, R7-F1 |
| `docs/plan-tasks.md` T28.7, `events-decision.md`, `results-tasks.md` | inbox and epoch live in hot-host; rule 3's defect "demonstrated" | no framework code on the path | R4-F3 |
| `docs/step4-hot-reload.md` | "~90–250 ms — comfortably inside the 2 s criterion"; added field "fresh default + log"; honesty items 2–3 | 2.735 s release showcase; top-level only; stale | R4-F2, R5-F4, R5-F9 |
| `docs/results-embedded.md` | `.text` 20,524 / "now 22,458"; "816 B at depth 3"; "f64 in frame path: zero"; "every threshold met" | 27,024; 1,144 release / 6,524 dev measured; ungated; interactive build 72.8–87.1% | R17-F13, R11-F3, R11-F2, R11-F4 |
| `docs/results-const-eval.md` | "Current: 0.13–0.21 s" | an incremental no-op rebuild | R11-F5 |
| `docs/results-gpu.md` §2, §4, §6, §7 | "every command"; "~20× headroom"; atlas rebuilt "rather than finishing with glyphs missing"; CPU fallback; crossover "not located" | no Image; 1.5× on llvmpipe; glyphs dropped; llvmpipe opened; §5b locates it | R9-F2, R9-F7, R9-F1, R9-F6, R9-F12 |
| `docs/results-glass.md` §5b | bevel +18% / +5% | row now measures dispersion etc. (42.5% / 34%, LOADED) | R9-F11 |
| `docs/results-harness.md` T12.2 | "The gate: change a radius by 1 px" | runs nowhere | R9-F4, R17-F1 |
| `docs/results-agent-cost.md` | "zero allocations per frame_done in steady state" | 4.00 | R14-F4 |
| `docs/results-frame-cost.md` | cited for idle figures | contains none | R7-F11 |
| `docs/plan-capabilities.md` T15.7, T15.8, T15.10, T15.11 | DONE, with gates | T15.8 paints unshaped; the other three gates never ran | R10-F2, R15-F5 |
| `docs/plan-desktop.md` T14.8 | "CJK entry" | composition untested through Host; default stack draws '?' | R10-F9, R10-F12 |
| `docs/mobile-decision.md:29-31` | core Android `cargo check` "cannot complete" | completes | R12-F6 |
| `docs/plan-widget-parity.md` §5, AGENT.md M27 | "green both ways"; "coverage as an assertion"; "three assertions per action"; adversary "once per tranche" | unverifiable strategy; tautological coverage; 31/94 no isolate; 1 of 9 | R18-F1, R18-F2, R18-F3, R18-F11 |
| `justfile:30-31` | release-shape is "the ONLY build that compiles" the observe-off shape | tests compile with observe | R12-F1 |
| `.github/workflows/ci.yml` | five comments | false | R17-F11, R17-F10 |
| `book/src/gaps.md`, `attributes.md`, `first-tree.md`, `component-boundary.md`, `input-widgets.md`, `text.md` | gap list, command count, attribute table, reload, clusters, complex text | stale or false | R16-F10, R4-F1, R4-F2, R10-F7, R10-F2 |

### 3.2 Per task

Short form: claim (source) — status — evidence or finding.

**R1 (invalidation and damage).**
- `Host::tick` never marks; winit ignores its result (plan) — **held** — winit `lib.rs:427`.
- Pointer and focus state is marked by the host (`invalidate.rs`, T25.0) — **held** — injection (b) `6f32d0f`: s02 2176 px, s27 2410 px.
- "reporting a dirty subtree clean costs correctness" (implementors conservative) — **false** — R1-F2, R1-F3, R1-F4, R1-F5.
- "Bound every frame, so a data change needs no invalidation call" (`repeat.rs`) — **false** — R1-F4.
- `App::tick` rustdoc says nothing about invalidating — **held** — the doc gap behind R1-F3.
- A backdrop reader depends on a wider region (`retain.rs` glass) — **held** — injection (c) `4c9e399`: 18 px from s03.
- widget-drive and tasks-drive run both strategies (M34 §1) — **held** by reading; see R18-F1 for what they cannot verify.
- "Only frames marked precisely can differ" (M34 §1) — **overstated** — R1-F2, R1-F4, R1-F5.
- "Paint attributes are resolved in layout" (T34.1) — **overstated** — R1-F6.
- `set_palette` marks all — **held** — s25 0 px everywhere.
- Glass press: settled frame byte-identical; tick deformation needs no layout — **held** — s04–s10 0 px.
- `Boundaries<N>` overflows to `mark_all` — **held** — largest narrowed set 3 of 256; 12–14 of 33 frames collapsed.
- A partial redraw repaints what changed (`take_damage_for_age`) — **false** — R1-F1.
- Changing the scale forces a full redraw — **held** — s26 0 px; R1-F1 appears on the next precise frame.
- The text memo keys on scale, gated (T26.17 #12) — **overstated** — R1-F7.
- Software partial redraws equal full ones — **false** — R1-F8.
- Overlay open/hover/close under Boundaries matches Everything — **held** — s23a–c 0 px.
- Adopt marks all and matches Everything — **held** — s28 0 px at 1.0.

**R2 (tables, hashes, memo keys).**
- Q1 `subtree_range` = pre-order range for every derived table — **held** — stricter clause checks all 25 members; parent-link oracle 0 mismatches up to 901 nodes.
- Q2 a table cannot pass well-formedness with a wrong range — **false** — R2-F5.
- Q3 enum STARTS agree with `subtree_range` — **held**.
- "the spliced table is well-formed" (macros) — **overstated** — R2-F5.
- verification-notes Tier 0 — **overstated** — R2-F5, R2-F9.
- `retain.rs` ~10⁻¹³ per frame — **false** — R2-F2.
- "The display scale is in the key" — **overstated** — R2-F6.
- `MeasureKey::of` scale "Required, not defaulted" — **held**.
- `MeasureKey` carries everything a measurement depends on — **false** — R2-F1, R2-F10.
- Q6 price of length + finaliser — **held** (priced) — R2-F4.
- Q7 `wid_index!` refuses a duplicate — **held**; the Host methods do not (R2-F7).
- `equivalence.rs` const fn agrees with runtime invariants — **false** — R2-F5 (I3).

**R3 (input).**
- C5: hidden focus takes no keys — **held** — injection red.
- `group.len() > 1` guard — **held** for lone fields — 6 tests red under `if true`; see R3-F1.
- Capture released on PointerUp — **held** — 2 tests red.
- T15.6 Shift+Tab, arrow groups, focus-visible done — **overstated** — R3-F1, R3-F9.
- "Shift+Tab is not implemented" — **false** — R3-F11.
- `EventCtx.row` "can never go stale" — **overstated** — R3-F3.
- Overflow "counted, not silent" — **false** — R3-F2.
- Tab trapped in the topmost overlay — **held** for Modal; loses focus under Tooltip (R3-F7).
- Escape returns focus to the trigger (plan Q4) — **false** — R3-F7.
- `paste`'s `InputQueue::<64>::CAP` matches — **held**.
- Hit testing costs the window, not the row count — **held** — 35.3 ns at 100/10k/1M rows (LOADED; flat).
- "Moves cost one call when a gesture is in flight" — **overstated** — R3-F12.
- "Above [the ceiling] layout debug-asserts" — **false** — R3-F10.
- `input.fill` refuses rather than typing into the wrong control — **held**.
- `input.fill`/`input.type` report what they did — **false** — R3-F2.
- Agent click equals a real click — **overstated** — no preceding Hover; live comparison not executable.
- Showcase Select is keyboard-operable — **false** — R3-F6.
- Agent input tests are a gate — **overstated** — R3-F8.

**R4 (hot reload).**
- Handoff Step 4 DONE — **overstated** — R4-F1.
- "the swap itself is milliseconds" — **held** — 76–187 µs; loop median 188 µs over 100 swaps.
- "~90–250 ms ... inside 2 s" — **overstated** — R4-F2.
- "reload times sub-second" (book) — **overstated** — R4-F2.
- "Leaks a few MB of address space per reload" — **held** — +426 kB/swap RSS, 101 images mapped.
- T28.7 inbox and epoch in hot-host — **false** — R4-F3.
- `check_no_unsafe.sh` cannot hide unsafe — **false** — R4-F4.
- miri unusable here — **held** (environmental; no-dependency crates run).
- Any panic in v2 aborts — **held** — exit 134 (R4-F5).
- No ABI fingerprint — **held** — SIGSEGV 139 (R4-F5).
- Per-field fallback logs incompatibilities — **held** (removed fields drop silently; not claimed).
- "the identity survives" — **overstated** — R4-F7.
- Log drainable via `take_log` — **held** only through the image's own `hot_take_log`; host-side 0 lines.
- Two-call size probe refuses an undersized buffer — **held** — `ef90385` makes it fire.
- miri-smoke "exercises every unsafe path" — **false** — R4-F8.

**R5 (reload state).**
- Added field → fresh default + log — **overstated** — R5-F4.
- "Any incompatibility keeps the fresh value ... and logs it" (book) — **false** — R5-F3.
- E0119 refuses a reloadable app that derives Default, both directions — **held**.
- "an application that derives Default alongside App is a compile error" (`event.rs`) — **false** — R5-F9.
- `is_application` cannot misfire — **held** by construction.
- Adopt marks all — **held** — 0 px; redundant mark (R5-F6).
- Reload re-asks the stylesheet — **held**.
- Program: "one definition" — **overstated** — R5-F5.
- `Reloadable::fresh` reaches Tally's assignments — **false** — R5-F5.
- Reload off links no serde_json — **held** — 0 vs 33 symbols.
- "Reload surface is not yet cfg-gated; fresh() = Default" — **false** (stale) — R5-F9.
- Queued results cross adopt — **held** — injection `inbox.cancel()` red.
- Toggle `on` survives a swap — **held** — hot demo exit 101 under injection.
- Label and Button restores gated — **held**.
- A popup closed by reload does not eat the next click — **held** (medium confidence).
- A press started before reload completes after it — **held**.
- Reloaded app equals fresh launch + restore — **overstated** — 3176 px from kept hover/press.

**R6 (frame cost).**
- Idle under Boundaries is the 0.01 µs row — **false** — R6-F1.
- 9.0× iced at 3000 rows — **held** — 8.9× (8.7× under the competitors' profile); the shipped pump is 4.7×.
- Stages sum within 1% of the whole frame — **not executable** (LOADED; ~1.8%).
- 20.7 µs, 36% — **overstated** — R6-F2.
- Stages "exactly what pump_with does" — **false** — R6-F4.
- Raster arm waits on the GPU inside the timed closure — **held**.
- Unique strings do not change the CPU raster figure — **held** — +0.6% / +4%.
- Banding ~3 ms at 4K — **held** — 2.55 ms; unreachable from winit (R6-F5).
- A thread costs 5–10 µs to wake — **held** — 6.5–7.6 µs per worker.
- benches with observe not distorted on grid — **held**.
- observe costs nothing until it runs — **held** — 2.54 vs 2.58 µs.
- Layout linear in nesting depth — **held** by reading.
- The frame arm's `layout` excludes place (plan premise) — **false** — I1 moved it 14.45→56.85.
- Boundaries buys nothing on a click frame (§2.5) — **held**.
- No number in results-competitive from a single run — **not executable** (history).

**R7 (idle, timers, lifecycle).**
- A still UI idles — **held** — 60 s: 0 frames, 0 wakes, 0% CPU, 0 switches.
- WaitUntil fixes idle timers — **false** — R7-F1.
- tasks-drive proves `run()`'s wiring — **false** — R7-F2.
- wake-gate red without the Sender wake — **held** — injection D.
- "a sender cannot send and forget" — **held** for Senders minted after `set_waker`.
- Huge dt "guarded" — **overstated** — R7-F8.
- `Timers::advance` catches up — **false** — R7-F3, R7-F4.
- Outbox overflow counts — **overstated** — R7-F4.
- "no thread is created for a timer" — **false** — R7-F5.
- No lock across send/recv/present — **held**.
- Agent thread cannot hang after the loop exits — **held** by reading.
- Resize storm, 1×1, unmap, close-with-job — **held** on both backends.
- GPU Outdated: "let the next redraw present" — **false** — R7-F7.
- FailFn None: "the host counts the drop" — **false** — R7-F9.
- Idle figures in `results-frame-cost.md` — **false** — R7-F11.
- Caret blink costs frames — **held** by reading (no blink code).

**R8 (raster and memory).**
- alloc_gate "rasterising a frame allocates nothing" — **false** — R8-F1.
- Handoff "the gate covers a rasterised frame" — **overstated** — R8-F1.
- A state-change frame allocates nothing — **overstated** — R8-F1.
- An idle pump allocates nothing on FontMetrics — **held**.
- PointerMove with nothing captured allocates and damages nothing — **held**.
- "No size_of assert pins NodeState, Style or Cmd" (plan premise) — **false** — they are pinned (`cursor.rs:32-33`, `focus.rs:197-198`, `glass_cmd.rs:15`); injection 3 red at 88 vs 84. NodeDesc (48 B) is unpinned.
- `surface.rs`: render is the expensive half — **held**; its number drifted (R8-F9).
- results-threading §4 — **false** — R8-F2.
- Multi-region damage walks the list per region — **held** — ~65% of a small region's cost.
- Shadow bounded by damage — **held** — 0.3 µs.
- Glass bounded by damage — **false** — R8-F6.
- SIMD coverage by Cmd kind — **held** — only Shadow and the present blits.
- `Canvas::resize` cheap and idempotent — **held**; retention not (R8-F7).
- `paste` returns characters delivered — **false** — R8-F4.
- Idle RSS ~24.6 MB — **held** — VmHWM 24.3 MB (R8-F10 on attribution).
- `DisplayList.text` capacity after a 1 MB paste — **held** (pinned, about 2 MB retained).

**R9 (GPU and fidelity).**
- M22 +18% — **held** — medians 17.2 / 17.3%.
- §0h 148 → 1.44 ms — **not executable**.
- 17.6× at 4K; crossover 3–5% — **not executable**.
- The GPU gets the region set — **held** — 2 quads / 208 B for a caret.
- sRGB-space blending on both — **held** — 128 vs 128.
- parity within 0.2% — **held** on llvmpipe (56 px).
- "~20× headroom ... catch a shape moved by one pixel" — **overstated** — R9-F7, R9-F5.
- "every command in the vocabulary" — **false** — R9-F2.
- svg.rs "whole vocabulary" — **overstated** — R9-F10.
- Atlas rebuilt rather than finishing with glyphs missing — **false** — R9-F1.
- GPU falls back to software without a driver — **false** — R9-F6.
- golden.rs gates with exact equality — **false** (never runs) — R9-F4.
- GPU partial redraw bit-identical — **held** — (0,0).
- MAX_DEPS fallback keeps glass correct — **held**; ungated (R9-F3).
- none-tier Glass degrades visibly — **held** by reading.
- Scalar and SIMD byte-identical — **held**; see R18-F10.

**R10 (text).**
- BakedFace scaled advance equals fontdue — **held** — 0.000 px at every size.
- IME candidate window not placed — **held** (documented gap).
- Preedit visible in the bound value — **held** (by design).
- `FontMetrics::shape` maps clusters back after substitution — **held** by reading.
- ComplexText usable by a turnkey window — **false** — R10-F2.
- Direction consulted anywhere — **overstated** — no non-test caller.
- Caret "steps by clusters" — **false** — R10-F7.
- Arrows edit text in a field — **false** — R10-F3.
- Measurer and painter "cannot disagree about a box" — **false** — R10-F1.
- T15.8 "measures and paints right-to-left" — **false** — R10-F2.
- BakedFace "Wraps at the last space that fits" — **false** — R10-F10.
- paste "keeps newlines" — **false** — R10-F5, R10-F6.
- T14.8 "CJK entry works" — **overstated** — R10-F15, R10-F9, R10-F12.

**R11 (embedded).**
- `.text` 27,024 B — **held**.
- Framework `.bss`/`.data` zero, gate fires — **held** for linked code (R11-F4, R11-F6).
- f64 in frame path zero — **held** today, ungated (R11-F2).
- Stack 816 B at depth 3 — **overstated** — R11-F3.
- 272 B/level is the upper end — **held** in release (~190–220); dev ~808.
- A tree that could overflow does not compile (Dyn excepted) — **held**; Dyn documented.
- The flt2dec regression would be caught — **false** — R11-F1.
- "Every threshold met" — **overstated** — R11-F3, R11-F4.
- RAM ≤ 24 KB for 100 nodes — **held** (asserts 84/80/164/1168).
- const-eval 0.13–0.21 s — **overstated** — R11-F5.
- const-budget fails a quadratic regression — **overstated** — R11-F5.
- Panic machinery cost — **held** — ~1.4 KB text.
- Profile matrix — **held** — z+lto+cgu1 12,848 B (−52%).
- `#[bind]` 256 B scratch fits — **held** — 520 B release / 976 dev.
- Host, retention, damage, rasteriser std-only (plan premise) — **false** — retention, damage and `InputQueue` link no-alloc (57,076 B); only Host and Canvas need std.

**R12 (configuration).**
- release-shape the ONLY observe-off build — **false** — R12-F1.
- "`#[cfg(feature)]` and the linker drop what a tree does not name" — **overstated** — R12-F4.
- fuse's remaining value is code size — **overstated** — −32 B (R12-F4).
- "a vtable in every build" — **false** — R12-F4.
- strict tier "the strong proof" — **overstated** — R12-F2.
- observe has zero release cost — **held** (the leak is test-only).
- widgets alloc tier builds no_std — **held**, ungated; none tier cannot compile 12 of 14 test files.
- no_std `format_into` truncates at a char boundary — **held**; undocumented at `#[bind]`.
- depth-2 powerset — **held** — 70/70, 0 warnings.
- reload seam mismatches are loud — **held**.
- `Button<AppA>` + `Button<AppB>` duplicate code — **false** — 29,320 B both.
- Boundaries code size — **held** — <16> +324 B, <256> +372 B.
- Marginal `.text` per capability — **held** — anim +280, `#[bind]` +892, Either +1,084, cascade +15,710, one dispatch +15,822.

**R13 (agent protocol).**
- A layout right after a click sees the result — **held**.
- No redraw race between pipelined RPCs — **held** — 0/20, X11 only.
- `handled` is `pump_with`'s value — **held**.
- Kind in `live_nodes` is load-bearing — **held** — `2b21ded` red.
- The residue "needs a type to contain itself" — **false** — R13-F5.
- Kinds from seeds, protected — **held** — `1e1c180` red.
- waitSettled tested — **overstated** — R13-F1.
- `app.perf` budgets checkable live — **false** — R13-F2.
- Reply `frame` usable with `lastDamage` — **false** — R13-F3.
- getTree reports role, value, label, bounds — **false** — R13-F4.
- Under Boundaries getTree shows current values — **held**.
- `input.fill` into a repeater row — **not executable**.
- fill refuses rather than destroying — **held**.
- Replay header mismatch is a hard error — **overstated** — R13-F6.
- Replay asserts each step — **false** — R13-F6.
- Emitted Rust compiles — **held**; asserts nothing.
- The agent crate's tests pass — **overstated** — R13-F7.
- `(frame, handled)` plus the damage bound — **overstated** — R13-F8.
- `app.quit` exists — **overstated** — R13-F9.

**R14 (agent transport).**
- "Default-OFF ... a release build does not" — **overstated** — R14-F1.
- `Config::agent` inert without the feature — **held** — 0 agent symbols.
- hostile.rs "the host stays up and answers" — **overstated** — R14-F3, R14-F5, R14-F6.
- 0 allocations per frame_done — **false** — R14-F4.
- frame_done 45.7 / 53.7 µs, "6–7× the frame" — **overstated** — 114 / 118 µs (one run, LOADED; open question).
- Serde text spliced into raw JSON yields invalid JSON — **held as not a present defect** — 7/7 replies valid JSON.
- NodeDesc 48 B release / embedded gate — **not executable** — R14-F8.
- Idle with agent on and no client costs nothing — **held**.
- Non-finite coordinates cannot corrupt state — **held**.

**R15 (platform and a11y).**
- TreeUpdate focus follows Tab — **held** (counter-window a11y).
- Repeater ids row-stable — **held** — `84c55a6` red.
- "a tree that holds none carries nothing" (gestures) — **held** — +32 B for one LongPress.
- canvas = inner_size under HiDPI — **held** at startup (1920×1008).
- ScaleFactorChanged does not resize the surface (plan) — **false** — `redraw()` resizes every frame.
- GPU atlas refuses an oversize image — **held** (documented).
- A run/Program app can supply images — **overstated** — Program lacks `.images` (known, M32 §2.1).
- T15.10 multi-window done — **overstated** — R15-F5.
- accesskit_winit panics on a visible window — **held as not a defect** on X11.
- dnd and dialogs build — **held**.
- `Drag<()>` is one byte — **false** — R15-F8. *Corrected by M54/T54.9 (2026-09-27): the rustdoc says 24 B and `tests/drag.rs` bounds it.*
- Decoding behind the shell's `image` feature — **false** — R15-F9. *Corrected by M54/T54.9 (2026-09-27); `scripts/check_doc_identifiers.sh` now refuses a backticked name called a feature that no `[features]` table has.*
- One implementation serves both — **overstated** — R15-F1.

**R16 (DX and widgets).**
- A widget_id typo is a build error — **held**.
- Common derive mistakes give spanned errors — **held**.
- Each W10xx except W1002 has a red-on-removal test — **held** — matrix.
- "Each lint gets a firing case and a quiet case" — **false** — R16-F1.
- W1003 unreachable — **false** — R16-F2.
- compile_fail tests fail for their stated reason — **held**.
- A NaN in a style disables every skip (plan) — **false** — still frames measure 0; the real effect is R16-F5.
- Chart handles NaN/inf/empty/1e6 — **held** by reading.
- DataGrid cost independent of rows — **held** — 9 binds at 1k/100k/10M.
- Markdown with 10k unclosed `**` bounded — **held** — linear.
- TextArea scales to real documents — **overstated** — R16-F7.
- Every const KIND driven separately — **held**.
- Selected-label ground "asserted rather than trusted" — **overstated** — R16-F8.
- The book lists what is absent — **false** — R16-F10.
- Attribute tables agree — **held** with each other; both stale (R16-F10).
- Agent selector grammar = stylesheet's — **false**, but no doc claims parity (recorded as fact).
- Targets ≥ 24×24 — **held** — 0 of 82.

**R17 (gates and CI).**
- ci.yml "There was none before this file" and implied CI coverage — **false** — R17-F10.
- "six crates" — **false** — R17-F11.
- zero `std::fs` — **false** — R17-F11.
- check_no_unsafe cannot hide unsafe — **false** — R17-F2.
- "Six checks mechanise the part a human misses" — **overstated** — 5 of 6 scripts miss a plant (R17-F3, -F4, -F5, -F15, -F16, R17-F2).
- "They run before any compile" — **held**.
- "Numbers are stated once" — **false** — R17-F13.
- `just test` "deterministic, no numbers" — **overstated** — const-budget is a wall-clock bound.
- parity.rs "a skipped test does not look like a passing one" — **false** — R17-F6.
- golden 0 passed under truetype (results-harness) — **held**, and why it runs nowhere — R17-F1.
- "151 / 134 test binaries" — **not executable** (history; today 145 `[[test]]` targets, 181 dev-shape executables).
- Thirty-three widgets — **held** (Canvas shipped as PaintBox).
- 39 entries, 94 actions — **held**.
- 19 App impls at T28.2 — **held**.
- firmware 27,024 B — **held**.
- results-embedded headline — **false** — R17-F13.
- `boundaries` = `Boundaries<256>` — **held**.
- bss.allow one entry — **held**.
- allowlist is "real but not ours" — **overstated** — R17-F15.
- justfile crate counts — **false** — R17-F14.
- lint_sheet_deep 4.04× — **overstated** — after-arm only (40.3–43.4 µs min, LOADED); before-arm gone.
- 5.9 ns/node — **not executable** (the arm is gone).
- "15 DECIDED markers" (plan premise) — **false** — 17.
- "20 cross-file rustdoc paragraphs" — **not executable**.
- Thumb closure dependency-free — **held**; lock not enforced.
- ci.yml benches comments — **false** — R17-F11.

**R18 (tests themselves).**
- wid_index missing-id doctest — **held** — I1c red.
- Duplicate id refused — **held** — I1d red; shares E0080 with the missing-id case.
- `Seed::new` without a mask fails — **held** — E0061.
- TextBuffer split doctest — **held** — I1a red.
- Shell suite passes with debug assertions — **held** — 168/175/168/174, 0 failures.
- "Every path asserts zero" — **overstated** — R18-F4.
- results-threading per-thread state pre-allocated — **false** — R18-F4.
- widget-drive "green both ways" — **overstated** — R18-F1.
- "coverage as an assertion" — **false** — R18-F2.
- "three assertions per action" — **overstated** — R18-F3.
- live_gate "Exit code is the verdict" — **false** — R18-F9.
- ui_check headroom — **held** — two bounds above 5× (50×, 114×) are documented ceilings.
- `just test` deterministic — **held** — 30/30 warm runs, load up to 38.
- const-budget headroom — **held** — 148–352 ms / 5000.
- Strategy-comparing tests assert their scenario — **held** except R18-F8.
- nextest agrees with libtest — **held** except doctests (934 vs 945; nextest alone would drop the four compile_fail guards).
- Mutants of the six named functions killed — **overstated** — 18 of 24 killed; R18-F5, R18-F6, R18-F7, and two `table_is_well_formed` relaxations (wellformed-2 `9157664`, wellformed-3 `a768b0b`, S3 by reading).

## 4. The vacuous-test table

AGENT.md's table, extended. Each row names what exposed it. 74 rows;
where two reviewers found the same test (golden, check_no_unsafe, the
agent's feature unification), both rows are kept because the injections
differ. **How caught**: *injection* = a fault was injected and the test
stayed green; *quantity* = the measured number was printed beside the
bound; *contents* = asking what the fixture contains; *use* = using the
thing end to end.

| what | why it proved nothing | how caught | injection / evidence |
|---|---|---|---|
| `tests/scale.rs`, `measure_scale.rs` vs the host's scale wiring (R1) | `measure_scale` builds its own `LayoutCx`; `scale.rs` uses scale-linear BlockMetrics | injection | d2 `ed94b88`: `.with_scale` removed, 317 green |
| `deferred.rs` (Boundaries arm), `palette_roles.rs` (R1) | every Boundaries frame they drive is a `mark_all` frame | injection | (a) `5c5b298` mark no-op: 6/6, 5/5 green; (b) `6f32d0f` |
| any cross-strategy oracle vs strategy-independent defects (R1) | both arms share memo, measurer, retention | injection | d1 `10a204b`, d2 `ed94b88` green |
| damage oracles at scale ≠ 1.0 (R1) | no test combines Incremental and a partial raster with scale ≠ 1 | quantity | R1-F1: physical rect beside logical damage |
| the derive's `table_is_well_formed` const assert (R2) | the predicate allows a last child to link anywhere later and does not require pre-order | contents + injection | I3 `3051257`: const green, 69 runtime failures |
| `damage_oracle.rs` as "the standing check" for collisions (R2) | integer counts cannot collide under the fold | injection | `r2_collision.rs`: 1440 stale px, oracle green |
| `measure_scale.rs` as the gate for "scale is in the key" (R2) | lays out only a Label | injection | TextInput 145 vs 146 |
| every arrow/caret test as a gate for "arrows edit text" (R3) | every fixture puts the field alone among focusables | contents | `11492f9` fix turns new probes green; red at `ac91d51` |
| agent `input.rs`/`rows.rs`/`protocol.rs`/`audit_is_inert.rs` per package (R3) | green only via workspace unification of `widgets/observe` | use | `30a1b0f` adds observe: 19 red → green |
| `overflow_rejects_the_newest_and_counts_it` (R3) | proves the counter; nothing reads `dropped()` | contents | 65 events strand capture |
| hot-host chain assertion, T28.7 (R4) | no framework code on its path | injection | `79e7c7c`: PASS |
| hot-host `mode=Basic` (R4) | the fresh default is Basic | injection | `bc023e0` (restore no-op): stayed green |
| hot-host `window=23` (R4) | window length ignores scroll | injection | `75ad027`: only `scroll=240` fired |
| `scripts/check_no_unsafe.sh` (R4) | comment strip not string-aware; scans only `src` text | injection | `dea0bb6`: 3 bypasses, exit 0 |
| CI miri job (R4) | miri-smoke does not link the reload shims | contents | miri transcription flags `(null,0)` |
| the adopt oracle's "remove mark_all" injection (R5) | `set_stylesheet` also marks all | injection | injection 1 green (0 px); injection 2 red (3560 px) |
| `reload_sheet.rs` as the only adopt coverage (R5) | runs under Everything only | contents | Boundaries oracle red at 3560 px under injection 2 |
| `benches --bin grid` "frame µs" column (R6) | omits dispatch, the arena cascade and finish | quantity + injection | I3 `b50d738`: column inside spread |
| stages `pump-sum6` fidelity check (R6) | the arm's Host has no stylesheet; no bound | injection | I3: 62.83 inside 58.52–65.10 |
| `just tasks-drive` for `run()`'s wiring (R7) | the observer's polls drive the frames | injection | A `294dee1`, B `14477cf`: 10/10 ok |
| `just wake-gate` for the turnkey window (R7) | never calls `run()` | injection | A: exit 0 in 603 ms |
| `alloc_gate` raster arm (R8) | runs the BlockMetrics stand-in | use | truetype twin red, 1290 allocs / 50 frames |
| `alloc_gate`'s thread-local counter vs banding (R8) | fixture never bands; counter per thread | quantity | 25–97 frame-thread, 6619–21891 workers |
| `clip_test.rs` "pins [clipping] at the pixel level" (R8) | only Rect commands | injection | `c472f80`: 4/4 green |
| `golden.rs` (R9) | compiles to 0 tests under truetype unification | use | `232c1e8`: 0 tests ok; per crate FAILED |
| every renderer test vs `Cmd::Image` (R9) | no test gives a renderer an image | contents + injection | `7415a9d`: 776 green |
| every test vs the `MAX_DEPS` fallback (R9) | no fixture has more than 4 backdrop readers | injection | `ecaf67f`: green; new oracle 1,822 px |
| `parity.rs` vs StrokedRect AA (R9) | frame-wide bound over a 56 px ring | quantity | `c68c51c`: 56 px < 86.4 |
| `svg.rs` whole-vocabulary test (R9) | omits StrokedRect and Glass | injection | `c75d6c4`: 170/170 |
| `parity.rs` vs the atlas reset path (R9) | its only text fits a 64² atlas | injection | `70f4713`: green |
| `clipboard.rs` `..._keeps_newlines` (R10) | input has no newline | injection | `ec07f29`: 8/8 |
| `Host::compose` routing (R10) | tests call the inherent method | injection | `125aa35`: 352 green |
| lumen-static-text RTL test (R10) | asserts what BlockMetrics satisfies | injection | `ca8547c`: green |
| `check_text.sh` vs flt2dec (R11) | ceiling loose for a paint-only firmware | injection | `2189c21`: 48,720 / 65,536, exit 0 |
| "f64 in the frame path: zero" (R11) | prose; nothing reads symbols | injection | `12e6d3d`: 6 symbols, all green |
| fixture `STACK <= 4096` assert (R11) | computed from depth × 272, not the stack used | quantity + injection | `937885b`: 5,016 B measured, green |
| `check_bss`/`check_text` vs interactive code (R11) | the firmware never dispatches | injection | `6697ad2`: dispatch static green |
| `just const-budget` timer (R11) | incremental no-op; `count_wid` never evaluated | injection | `b634311`: 50.8 s, then 181/207 ms green |
| `check_text`/`check_bss` under CARGO_TARGET_DIR (R11) | read a hard-coded `./target` | injection | both exit 0 on a stale binary |
| `just release-shape` as the observe-off test (R12) | dev-dep unifies observe | injection | `520ed2e`: `compile_error!` red only in test/clippy |
| every gate vs core's no_std runtime (R12) | no gate runs it | injection | `fa64952`: stores 0 bytes, all green |
| embedded `.text` as the framework's device price (R12) | paint-only firmware links no vtable | injection | `0d0ce1a`: +0 B paint-only, +28,706 dispatching |
| `protocol.rs` waitSettled clause (R13) | asserts the hard-coded constant | use | live: true at 447/600-frame gaps; `4ccbf52` |
| `session::Recording::replay` (R13) | replies discarded; stylesheet never compared | injection | `580f9f6`: Ok with n=1 of 3 |
| `Recording::export_test` output (R13) | no assertions | use | `#does-not-exist` passes |
| `harness.py` damage bound (R13) | `if mine:` skips it on empty regions | injection | `1beed32`: 0 damage lines, exit 0 |
| winit tests vs the live agent path (R13) | nothing drives `answer_agent → redraw → frame_done` | injection | `1beed32`: 22/22 |
| `benches --bin agent` allocation count (R14) | prints, never asserts | quantity | 4.00 / 0 since `075bec1` |
| embedded `NODE_DESC == 32` vs an observe leak (R14) | observe needs std; E0463 first | injection | features=[observe] → E0463 |
| `a_tree_that_drags_nothing_carries_one_byte` (R15) | asserts `Option<()>`, not Drag | quantity + injection | `45fa10a`: Drag 1024 B, green |
| the widget a11y surface (R15) | no test reads a widgets-crate role | injection | `d85e851`: 19 = 19 failures |
| W1002 OFFSCREEN (R16) | no test either direction | injection | retain-filter: 150/150 |
| `overflow_is_impossible_here_...` (R16) | fixture has no ScrollView | contents | invariant already broken on main |
| `a_filled_strip_has_a_ground_for_its_selected_label` (R16) | asserts defaults, not the painted Cmd | injection | `fd31a02`: 147/147, widget-drive green |
| the derive's second-`#[tree]` and columns=0 checks (R16) | no compile-fail case | injection | `11d3b34`: green |
| `scripts/check_attr_table.sh` (R16) | compares two hand tables | contents | wrap, grid, columns, tick missing from both |
| `golden.rs` under every automated build (R17) | truetype unified on | use | `6881359`: unified 0 tests, alone red |
| wgpu `parity.rs`, `glass_parity.rs` without an adapter (R17) | return early; eprintln captured | injection | adapters removed: 6/6 ok in 0.01 s |
| font-dependent tests without a system font (R17) | return early; 7 silently | injection | fonts hidden: 911/911 |
| `check_bss.sh` when nm fails (R17) | empty list passes | injection | `5af3aee` + failing nm: exit 0 |
| `check_no_unsafe.sh` vs `//` in a string (R17) | not string-aware | injection | `6105909`: exit 0 |
| `check_attr_table.sh` vs a parser attribute (R17) | never reads the parser | injection | `ghostr17`: green |
| `check_derive_attrs.sh` multi-line / comment use (R17) | per-line grep; comments count | injection | 7 → 4 attributes checked |
| `check_doc_paths.sh` (R17) | any cargo metadata name counts; no `-p`; narrow scan | injection | 4 plants green |
| `check_doc_identifiers.sh` (R17) | leading-`//` lines only; AGENT.md unscanned | injection | 3 evasions green |
| the five live gates without DISPLAY (R17) | skip exits 0 | injection | all 5 exit 0 |
| `widget_drive.sh` both-strategies (R18) | strategy is a label | injection | I5a `1a1f478`; leftover process `3d3333a` |
| `harness.py` coverage assertion (R18) | compares the inventory with itself | injection | `9623a3e`: 38/92 green |
| `alloc_gate.rs` under `threads` (R18) | fixtures never band; TLS counter | quantity + injection | I2 `045e0b3`: 11/11 green |
| `a_full_set_degrades_by_merging_rather_than_collapsing` (R18) | area bound easier with a dropped region | injection | push-1 `7a03a6d`: green |
| `escape_closes_the_topmost` (R18) | one overlay | injection | dispatch-2 `7f47109`: green |
| invalidation oracle headline, click steps (R18) | never asserts handled or a count | injection | I4a `9a74944`: count -1, green |
| `simd_shadow.rs` under simd (R18) | nothing asserts the SIMD branch compiled | injection | I8 `deeea8b`: 2/2 |
| `live_gate.sh` exit status (R18) | `wait "$pid" \|\| true` | injection | I6 `481773e`: rc=0 after a panic |

**What caught them**, counted from the column above: injection alone 52 rows; injection with a printed quantity 4, with a question about the fixture's contents 2; a printed quantity alone 4; contents alone 6; using the thing end to end 6. Reading alone
caught none. **The new shape this review adds to AGENT.md's two:**
*the gate never runs* — a test cfg'd out by feature unification
(golden), a check that skips as a pass (GPU, fonts, DISPLAY), or a
firmware that never links the code under test. It looks like the first
shape, but the cause is the build graph rather than the fixture.

### §4 addendum (dated rows, added as the fixes land)

Rows found or re-run after this review, in the table's shape. M43/T43.15
completed the addendum on 2026-09-26 with the re-triage's I-NEW-1..4 and
R7-F2's re-run; a later finding adds its own row. Counted with these five
rows, the table holds 79, and the "what caught them" tally above covers
the 74 the review found.

| what | why it proved nothing | how caught | injection / evidence |
|---|---|---|---|
| `just tasks-drive` vs `run()`'s waker and `WaitUntil` (R7-F2; re-run 2026-09-25, M47/T47.1) | every read requested a frame, so the window was never idle while the drive watched it | injection | at `b213ae0`: waker uninstalled, or deadline ignored → 10/10 green; an unobserved read showed 1 tick of 25. Since T47.1 the drive's unobserved phases go red under both (`docs/results-tasks.md` T28.12) |
| the shadow oracle (M34/T34.1; I-NEW-1 and R1-F6, fixed 2026-09-25, M41/T41.2) | it re-ran the code it checked before the output it checked, so it repaired every defect it could see; and its one test asserted only that it compared and stayed quiet | re-triage injection | a scratch cloned from the states left `tests/shadow.rs` green. Since T41.2 that injection names nothing in three positive cases, and moving the check back before paint fails `the oracle repaired the frame it judged` |
| `tests/tick_marks.rs` (I-NEW-2; closed 2026-09-25, M41/T41.0) | it compared the two strategies' frames, and a tick that never ran left both drawing the same empty label -- the setup-did-not-happen shape | re-triage injection | `self.app.tick(dt_ms)` replaced with `false` in `Host::tick`: identical hashes, green before T41.0; since then red with `setup: the tick did not run`, and deleting `mark_all` in `Host::tick` still turns it red at the equality (`docs/plan-boundaries-repair.md` T41.0) |
| `agent_drive.sh`'s `Boundaries` leg vs the shadow oracle (I-NEW-3; closed 2026-09-25, M42/T42.4) | `if sh.get("supported"):` had no `else`, so a build without the oracle compared nothing and printed `both strategies` | re-triage injection | `shadow-oracle` dropped from the leg: exit 0, and the only difference from the baseline was a missing "compared 264 frames" line. Since T42.4 the plants `live/shadow-absent` and `live/shadow-unexpected` go red by name (`docs/plan-drive-identity.md` §1.2) |
| `failure_screen.rs`'s golden, and `golden.rs` (I-NEW-4; closed 2026-09-26, M43/T43.9) | a missing golden was recorded and the test passed -- `Verdict::Recorded` fell into the `println!` arm; `golden.rs` re-ran the comparison against the file it had just written. And under `--workspace` neither ran at all | re-triage injection + use | golden deleted: green before T43.9; since then `a first run is not a pass` (plant `build/golden-deleted`), a corrupted golden `differs` (`build/golden-corrupt`), and the recipe line removed names `golden` as a zero-test binary (`build/zero-tests`). `golden.rs`'s share of the shape was found by building (`docs/plan-gate-hygiene.md`, What building found) |

## 5. Gate reality

Each gate the reviewers touched: where it runs, whether it can skip, and
whether it has now been shown to fail. "Shown to fail: yes" means an
injection in this review turned it red; "no" names the injection it
missed. **Bold M43 notes (added 2026-09-26, M43/T43.15)** name the plant
under `scripts/selftest/plants/` that now turns the row red on every run
of its tier (`static` in `just test`, `build` in `just check`, `slow` and
`live` in `just ci`), or where the row now runs.

| gate | runs where | can skip | shown to fail |
|---|---|---|---|
| `invalidation_oracle.rs` | `just test` | no | yes: mark no-op 2/7 red, PointerFocus no-op 1/7; **headline test green under I4a** (R18-F8) |
| `pointer_invalidation.rs` | `just test` | no | yes: 4/4 and 3/4 red |
| `enum_relayout.rs` | `just test` | no | yes under mark no-op (1/2) |
| `palette_host.rs` | `just test` | no | yes under mark no-op (1/6) |
| `deferred.rs` (Boundaries arm) | `just test` | no | **no**: 6/6 under both injections |
| `palette_roles.rs` | `just test` | no | **no**: 5/5 under mark no-op |
| `scale.rs`, `measure_scale.rs` (host scale wiring) | `just test` | no | **no**: green under d2 |
| derive const assert `table_is_well_formed` | every compile | hand-built generic tables only when spliced; `Assembled` never | yes for I1 (54 errors), I2 (23); **no** for I3 and A–D |
| `table_invariants.rs` | `just test` | no | no negative cases for pre-order; 12/12 with the proposed clause |
| `equivalence.rs` | `just test` | no | yes: red under I3 |
| `damage_oracle.rs` | `just test`, `just simd`, `just threads` | no | yes: shadow-clip injection 3/10 red; **no** for collisions |
| `text_memo.rs` | `just test` | no | accepts the fixed fold; no collision case |
| `focus.rs`/`slider.rs`/`selection.rs` vs the group guard | `just test` | no | yes: 6 red |
| `events.rs` C5 focus clear | `just test` | no | yes |
| `slider.rs`/`select.rs` vs capture release | `just test` | no | yes: 2 red |
| agent `input.rs` fill tests | `just test` (workspace only) | per package: 19 red | yes, by removing unification. **M43/T43.6: yes, plant `build/agent-alone`; alone in `just test-crates`, a `just ci` step** |
| `just hot-demo` | `just fixtures` in `just test`; CI Linux | no on Linux | yes for keep/reset/bind/log (INJ1–INJ7); **no** for the chain (`79e7c7c`), sheet selectors, `mode=Basic` alone |
| `scripts/check_no_unsafe.sh` | `just static`, CI | bypassable | plain unsafe yes; **no** for 3 bypasses (`dea0bb6`, `6105909`). **M43/T43.1: yes, plants `static/unsafe-string-slash`, `static/unsafe-forbid-removed`, `build/unsafe-path-module`, `build/unsafe-outdir-include`; `forbid(unsafe_code)` is the gate** |
| CI miri job | CI, `continue-on-error` | non-blocking | never green here; covers no unsafe |
| `reload.rs` (core) | `just test` | no | yes: Label no-op restore, 3 red |
| macros `ui.rs` trybuild | `just test` | no | yes: stack spacing, `fail_bind_into_default_label`; **no** for second `#[tree]`, columns=0 |
| `reload_sheet.rs` | `just test` | no | Everything only |
| benches-competitive frame arm | manual | n/a | yes for place (I1), finish via floor (I2) |
| stages `pump-sum6` | manual | prints only | yes I1, I2; **no** I3 |
| `benches --bin grid` | manual; CI bench job (never run) | column unasserted | **no** for I3 |
| `just wake-gate` | local with a display | exits 0 without DISPLAY | yes for Sender wake (D); **no** for `run()` (A). **exits 77 without a display since M42/T42.2, and `just live` (M43/T43.13) refuses a 77** |
| `just tasks-drive` | local with a display | exits 0 without DISPLAY | **no** for A and B. **exits 77 without a display since M42/T42.2, and `just live` (M43/T43.13) refuses a 77** |
| `inbox_wake.rs` | `just test` | no | yes: D, 2 red |
| `timers.rs` | `just test` | no | cannot exhibit R7-F1, F3, F4 by construction |
| `alloc_gate.rs` raster arms | `just test` | never truetype, never bands | truetype twin red; **no** under I2 threads |
| `cursor.rs`/`focus.rs` size asserts, `glass_cmd.rs` | `just test` | no | yes: 88 vs 84 |
| `banded.rs` | `just test`, `just threads` | no | correctly blind to the clip injection |
| any gate on `Canvas::resize` | none exists | n/a | **no**: `68443f6` green everywhere |
| `just gpu` (`parity.rs`, `glass_parity.rs`) | `check-extra`, CI | **skips as pass** without an adapter | yes for Line AA (186 px), gamma (3,751); **no** for Image, atlas reset; border only on llvmpipe. **M43/T43.10: the skip is refused, plant `build/gpu-hidden`** |
| `golden.rs` | **nowhere** | always 0 tests under unification | yes only per crate. **M43/T43.9: runs alone in `dev-shape`; plants `build/golden-corrupt`, `build/golden-deleted`, `build/zero-tests`** |
| `glass_oracle.rs`, `glass_*.rs` | `just test` | no | **no** for `MAX_DEPS` |
| `svg.rs` | `just test` | no | **no** for StrokedRect, Glass |
| `just gpu-parity` (live) | manual | skips without display, adapter, xwd | not run (llvmpipe only). **in `just live` since M43/T43.13; a missing display, adapter or `xwd` exits 77, refused unless LUMEN_ALLOW_SKIP=1** |
| `lumen-static-text` `shaping.rs` | `just test` | skips silently without a face | partially: 1 of 6 red under two injections. **M43/T43.11: shapes the committed face; discovery refused without a face, plant `build/fonts-hidden`** |
| `clipboard.rs` | `just test` (BlockMetrics) | no | **no** for newlines |
| `ime.rs` | `just test` | no | **no** for routing |
| `baked_face.rs` | `just test` | skips without DejaVuSans | checks bytes, not wrap. **M43/T43.11: reads the committed face; no skip** |
| `check_text.sh` | `just thumb` in `check-extra` | stale under CARGO_TARGET_DIR | Toggle/task halves per docs; **no** for flt2dec, f64. **M43: in `just test` as `firmware` (T43.13, `docs/plan-gate-hygiene.md` §2.10); plants `build/text-stale-headline`, and the stale target dir through `build/bss-stale-target` (shared `scripts/lib/firmware.sh`)** |
| `check_bss.sh` | `just thumb` | blind to unlinked code; stale dir; passes on nm failure | yes: static in paint; **no** in dispatch, nm failure. **M43/T43.4: yes, plants `build/bss-nm-fails`, `build/bss-stale-target`; in `just test` since T43.13** |
| fixture `STACK <= 4096` | thumb builds | always, unless DEPTH changes | only for depth arithmetic |
| fixture size equality asserts | thumb builds | UI only on 32-bit | per docs |
| `just const-budget` | `just test` / CI | warm cache is a no-op | red only via rustc's lint; timer green on rerun. **M43/T43.5: yes, plants `slow/const-quadratic` (red on rerun), `slow/const-count-wid`** |
| "f64 in the frame path: zero" | nowhere | n/a | cannot fail |
| `just release-shape` | `just test`, CI | no | yes for `compile_error!` and widgets observe list; runs with observe on. **M43/T43.7: observe off; plant `build/observe-tripwire`** |
| `just thumb` | `check-extra` | type-checks only | **no** for `fa64952`. **M43/T43.8: `bind_nostd.rs` under LUMEN_REQUIRE_NOSTD=1; plant `build/nostd-bind` (`fa64952`)** |
| `counter-window --drive2` | fixtures / live | one app | not re-run |
| `protocol.rs` waitSettled | `just test` | no | red only by pinning a constant |
| `changes.rs`/`hidden_subtree.rs`/`lint.rs`/`off_table_visibility.rs` | `just test` | no | yes: `2b21ded`, `1e1c180` |
| `session.rs` refusals | `just test` | no | viewport, measurer, start frame yes; per-step and stylesheet absent |
| `scripts/widget_drive.sh` | manual | exits 0 without DISPLAY; per action on empty regions | **no** for `1beed32`, I5a, leftover process, I5b/I5c, colour (`fd31a02`). **Since M42/T42.2: shown to fail: yes, by the stranger and leg-drop plants** (`scripts/selftest/plants/live/`); exits 77 without a display. **In `just live` (M43/T43.13)** |
| `scripts/ui_check.py` | `just live-gate` with a display | 2 of 3 budgets pass on zeros | latency fails on nan |
| `benches --bin agent` allocation count | manual | never fails | no (reads 4.00) |
| `hostile.rs` | `just test` | never reaches the transport | not injected |
| R14 scratch `r14_transport.rs` | scratch only | n/a | yes: red on `serve()`, green with `4ac7e2d` |
| `observe.rs` repeater ids row-stable | `just test` all-features | needs observe | yes: `84c55a6` |
| `drag.rs` one-byte test | `just test` | no | **no**: `45fa10a` |
| T15.10 / T15.7 / T15.11 gates | nowhere | n/a | never executed |
| widget a11y roles | none exists | n/a | **no**: `d85e851` |
| agent lint tests | `just test` | no | yes for W1001, W1004–W1011; **no** for W1002 |
| `scripts/check_attr_table.sh` | `just static` | exits 0 if the book file is missing | **no** for parser drift. **M43/T43.3: yes, plants `static/attr-ghost`, `static/attr-missing-book`** |
| `check_doc_identifiers.sh` | `just static` | no | leading `//` and handoff yes; trailing, block, AGENT.md no. **M43/T43.2: yes, plants `static/ident-trailing`, `-block`, `-agent-md`, `-stale-allow`, `-toml-stripped`** |
| `check_derive_attrs.sh` | `just static` | no | one-line list yes; multi-line, comment use no. **M43/T43.3: yes, plants `static/derive-multiline`, `static/derive-comment-use`** |
| `check_doc_paths.sh` | `just static` | `--bin` half skipped without cargo | docs path and docs `--bin` yes; `--bin <dep>`, `-p`, book no. **M43/T43.2: yes, plants `static/paths-bin-is-dependency`, `-bin-is-package`, `-unknown-package`, `-bin-in-book`** |
| `check_book_includes.sh` | `just static` | exits 0 without `book/src` | anchor and `rust,ignore` yes; `~~~`, indented no. **M43/T43.2: yes, plants `static/fence-tilde`, `static/fence-indented`** |
| dev-shape `cargo test --workspace` | `just test`, CI | 15 font tests skip; golden 0 tests | 911/911 with and without fonts. **M43/T43.9, T43.11: zero-test binaries refused (`build/zero-tests`); fonts pinned (`build/fonts-hidden`)** |
| `floats`, `simd`/`threads` (`check_banded`), `book`, fixtures | `check` / `test` | varies | not tested here |
| `simd_shadow.rs` + clippy | `just simd` in `check` | no | **no**: I8 |
| `live_gate.sh` | `check` with a display | skips without one | **no**: I6. **In `just live` (M43/T43.13): no display is a 77, refused unless LUMEN_ALLOW_SKIP=1** |
| compile_fail doctests (4) | `just test` | dropped under nextest alone | yes: I1a, I1c, I1d |
| `cargo deny` / `cargo audit` | nowhere | n/a | run here: 0 vulnerabilities; 2 unmaintained (rustybuzz, ttf-parser); no license policy. **M43/T43.12: `just deny`, a `just ci` step; plant `static/deny-unaccepted`** |
| shell debug-assertions off (profile) | all dev/test builds | n/a | turning them on found nothing |

## 6. Not executable here

Every question that could not run on this machine, and what it needs.

| task | question | needs |
|---|---|---|
| R1 | GPU damage regions × strategy × scale | a hardware adapter; by reading `gpu.rs:83` passes the same logical regions, so R1-F1 is expected there |
| R1 | live widget-drive under Boundaries with an IME preedit | an agent method for composition (none among the `input.*` methods) |
| R1 | Q7 `is_all` rate on showcase over a real session | an instrumented build exposing it per frame |
| R2 | live Ctrl+V confirmation of R2-F1 on port 7602 | a display session and time; reproduced headless |
| R2 | "font loaded after first layout", "fallback face set" as separate stale boxes | an API that changes a face after construction (none exists) |
| R2 | Q6 as a whole-frame retention arm, eight runs a side | a retention arm (not built) |
| R2, R3, R4, R6, R7, R8, R9, R10, R12, R14, R16, R17, R18 | unloaded timings | a quiet machine (load 3–38 throughout); `perf_event_paranoid=4` blocks instruction counts |
| R3 | Q9 agent click vs a real xdotool click | xdotool (not installed) |
| R3 | Q2/Q6 how often more than 64 events arrive between redraws live | a synthetic high-rate pointer source |
| R4 | Q2 edit-to-pixel time for a windowed app | a windowed reload path (M37) |
| R4 | Q7 v2 binding v1's agent port | a dylib that runs `run()` with the agent (unsupported) |
| R4 | Q8 cargo miri on the workspace | a nightly whose cargo-miri handles build scripts |
| R4 | load-insensitive edit-to-.so | perf with paranoid < 4 |
| R5 | Q7 Program without `.style()` by pixel in a live window | an agent hook on Program (none) |
| R5 | Q8c T32.0's fn-pointer reload | dropped: removed at `c8f4c5c` |
| R5 | Q5(b)/Q6 adopt with a structurally different tree; capture across it | not reachable in-process (`Host<A>` is typed); moves to M37's Host re-creation |
| R6 | Q5 GPU half (glyph atlas, results-gpu §1) | a hardware adapter (only llvmpipe reachable) |
| R6 | Q4 in a live window on port 7606 | not attempted; answered headless |
| R6 | "stages and frame agree within 1%" | a quiet machine (~1.8% measured) |
| R6 | `workers_for` 2.4× / 1.8× throttle claim | a code change and the vello arm |
| R6 | competitors at their own best configuration | rebuilding the sibling Lumen harness |
| R7 | GPU zero-damage present cost; natural Lost/Outdated | a hardware adapter |
| R7 | 0×0 resize on minimise | Wayland or Windows |
| R7 | display-server cost of a whole-surface present | a real compositor profile |
| R7 | agent thread behind an rfd dialog | a caller of `Dialogs` (none) |
| R8 | Q5 GPU RSS/VmHWM at 4K | a wgpu build (disk ran out mid-run) |
| R8 | Q6 live resize-drag allocation count | a counting allocator in counter-window and a scripted drag |
| R8 | Q3 present cost at a live 24×24 caret | a live app with a focused field on the agent port |
| R9 | Q3 present blocking and event-to-present latency | a working hardware GPU driver |
| R9 | results-glass §0h 1.44 ms; results-gpu 17.6× and 3–5% | the RTX 4070 |
| R9 | Q8 framework share of GPU RSS | a hardware adapter (partial answer by arithmetic) |
| R9 | the border-AA injection on the RTX | a working RTX driver (inferred green) |
| R9 | atlas resets in showcase | showcase `--features gpu` on hardware |
| R10 | Q7 live IME candidate placement | an input-synthesis tool |
| R10 | Q6 live Ctrl+C reaching `update` | synthesised platform keys (`KeyEvent` has no public constructor) |
| R10 | FontMetrics + wgpu ink containment | a GPU adapter (by reading, R10-F1 reproduces there) |
| R10 | BakedFace + embedded painter containment | a pixel-producing BakedFace painter (none ships) |
| R10 | copy from a Repeat row | row-level focus (rows are not in the table) |
| R11 | `-Zemit-stack-sizes` / cargo-call-stack | tools not installed; qemu painting used instead |
| R11 | `panic_immediate_abort` / build-std | rust-src and the thumb target on nightly |
| R11 | Dyn stack on the Cortex-M4 | a global allocator in the qemu probe |
| R11 | stack and timing on real silicon | hardware |
| R12 | Q8 `aarch64-apple-ios` checks | the iOS target (not installed) |
| R12 | Q7 x86 cost of two Host strategies in one binary | time |
| R12 | Q6 marginal `.text` of Repeat, Popup, gesture | an alloc-tier linked firmware with an allocator stub |
| R12 | Q2 behaviour loss per feature combination beyond the observe seam | behavioural tests per combination |
| R13 | waitSettled during a Glass press | a showcase entry that places Glass |
| R13 | `input.fill` into a repeater row | a fixture with editable rows |
| R13 | frame-domain races on Wayland/macOS | a non-X11 display |
| R13 | Glass/Veil residue in a shipped example | a search of examples (not done) |
| R14 | public-origin page and DNS rebinding | network access |
| R14 | the 0.0.0.0 bind from another machine | a second host |
| R15 | Q3 runtime `ScaleFactorChanged` without `Resized` | Xvfb/xsettingsd or a second monitor |
| R15 | Q5 three-file XDND drop live | xdotool or an XDND source |
| R15 | Q5 redraw/timer/RPC stall behind an rfd dialog | Xvfb + xdotool |
| R15 | Q4 real Android suspend/resume | an NDK linker and a device |
| R15 | Orca speech output | out of scope; pyatspi used |
| R16 | beginner mistakes #9 (hidden focus) and #10 (allocating bind) | time and a fixture |
| R16 | M27 §5 "the adversary passes its own gate" | an adversarial agent run |
| R16 | contrast and target size for repeater and arena rows | an AccessKit or display-list walk per row |
| R16 | Tree at 100k rows | inapplicable (`MAX_TREE_NODES = 32`) |
| R17 | has `ci.yml` ever run on GitHub; a clean ubuntu runner | a remote with Actions, or a runner image |
| R17 | the ci.yml steps verbatim in a clean container | network to install rustup, just, mdbook |
| R17 | would parity skip on the CI runner | a GitHub runner |
| R17 | an injection into the GPU render path itself | a scratch wgpu build (disk contended) |
| R17 | 5.9 ns/node and the 4.04× before-arm | the removed arm and pre-fix code |
| R17 | "four fixtures sit at 0%" coverage | cargo-llvm-cov |
| R18 | cargo-mutants on the six functions | the tool (24 hand mutants instead) |
| R18 | cold-compile flake rate under load | a longer exclusive window (one rustc SIGSEGV seen, not reproduced) |
| R18 | `just test` under stress-ng | the tool (24 busy loops instead) |

## 7. Queued-work impact

From Phase 3. **Scratch evidence:** `proto/state-split` rebased onto
`ac91d51` as `bc91156` (from `d861f4f`) and `6339b29` (from `a1ded79`);
probe `09f76e5`; injection `9879464`, reverted `1edde20`.

**The rebase.** No textual conflicts (`b7ddf07` cherry-picks empty, a
duplicate of `25f360d`). One semantic conflict:
`benches-competitive/arms/stages/src/main.rs:180`, E0046 missing
`State`, `state_mut` — the impl T33.0 names, and no other is missed.
`cargo check --workspace --all-targets --exclude lumen-static-wgpu`
exits 0; the four core crates with `widgets/observe` pass 872/0.
`#[bind]`'s source path keeps compare-before-write: forcing
`changed |= true` (`9879464`) turns `derive_app.rs:198` and the probe
(`p3_split_reload.rs:53`) red. The reload derive survives the split
(`{"state":{"count":5}}` restores; first update writes, second reports
false), but the tree must be `#[reload(fresh)]` (Button has no
Reloadable, the R5-F2 shape) and `widgets/reload` must be on explicitly;
the derive's `changed` local is still visible (R16-F3).

| plan / task | findings | effect | can its kill rule / injection fire? |
|---|---|---|---|
| **M32 T32.0** | R5-F10, R5-F5, R5-F9 | Premise gap: `Host::adopt` (shell `lib.rs:172/217`) and the derive's `fresh()` (macros `:1234`) depend on `Application::style` and `build`; neither is in T32.0's site list. Fix `counter-window/app.rs:101` and `event.rs:562` in the same commit. | Style injections: yes. **The re-stated M31 gate: no** — passes on E0277 for the wrong reason; needs a trybuild `.stderr` match on E0119 with an Application-free fixture. |
| M32 §2.1 `.agent`/`.images`/`.backend` | R14-F1, R14-F2, R13-F10, R12-F3, R15-F6, R10-F2 | New prerequisite (security): M39 before `.agent()`. One env var, refuse non-loopback, enable or enforce `widgets/observe`; `.images` called per backend (fixes R15-F6); a measurer seam. | n/a |
| M32 rule 3, T32.1 | R4-F3, R4-F1 | Premise weakened, conclusion kept: correct the T28.7 citation. | T32.1's drop-a-rule injection: yes. |
| **M33 T33.0** | (rebase) | `stages` is the one missed impl. | compile_fail pairs: yes. |
| M33 T33.1 | R16-F3, R16-F4 | Fix hygiene (`Span::mixed_site`) and duplicate-attribute refusals in the same block. | Verified: 2 tests red; ignoring the source path is a compile error. |
| M33 T33.2 | — | Unaffected. | yes |
| **M33 T33.3** | R18-F2, R18-F1, R18-F3, R3-F6, R3-F7, R3-F5 | Premise false: **Select has no drive inventory**. Add keyboard open and rows, focus restore, `slot(k)`. | Only after an inventory exists and M42 lands. |
| M33 T33.4 | R18-F1, R17-F8 | Drive gate depends on M42. | byte identity yes; drive half no. |
| M33 §2.3 (tick) | R1-F3 | Answered: a defect; M34 prerequisite. | — |
| **M34 §1** | R1-F2, R1-F4, R1-F5, R1-F1, R5-F6, R2-F1 | Premise incomplete: add compose, Repeat data, the audit probe, adopt, paste; R1-F1 affects both strategies and goes first. | — |
| **M34 T34.0** | R1-F2..F5, R5-F6, R10-F9, R18-F6, R18-F8, R2-F5 | Inventory misses R1's mark paths (compose, tick write, Repeat, probe, adopt, `set_scale`, `restyle_subtree`+arenas, nested mark). New prerequisites M41, M42. | Hover injection yes; **headline oracle cannot see missed clicks (R18-F8); converted drives cannot see the strategy (R18-F1)** — inverting the legs, T34.2's exact edit, stayed green at 94/94. |
| **M34 T34.1** | R1-F6 | Premise false; redesign as a display-list comparison. | Planned injection yes, but green on 3 of the 4 known defects. |
| **M34 T34.2** | R13-F2, R6-F1, R6-F2, R6-F3, R12-F5 | Premise false: no agent method exposes `timings3`, and `app.perf` returns 0 ns. Measure headless or fix R13-F2 first; price the arena cascade; idle is ~111 µs. | **no**: the instrument returns zeros. |
| **M35 T35.0/T35.1** | R14-F8, R14-F4, R13-F5, R13-F4/R15-F10, R12-F4, R6-F2 | Fix `live_nodes` identity, the 4 allocs/frame, missing fields in the rewrite. Rule 1 would **preserve** R13-F5; amend it. R14-F8 contradicts §3 for feature builds with `agent: None`. Scope the vtable premise to desktop. | symbol gate: not re-verified; per-container and equivalence: yes; T35.2 is a scoping rule. |
| **M36 T36.0–T36.3** | R14-F5, R7-F10, R7-F3, R3-F4, R3-F2, R18-F1, R17-F8 | Rule 4 must add agent dispatch and `about_to_wait`'s Duration; a hang in `advance` cannot be caught (fix in M47 first); pointer reset should release capture. T36.3 depends on M42. | T36.1, T36.4 yes; T36.3 only at scale 1.0 until M40. |
| **M36 T36.4** | R4-F5, R4-F6, R4-F8 | The per-image hook survives the duplicated-statics finding (fresh statics per dlopen, v1 never unloaded). The shim must validate symbols and an ABI stamp before `hot_drop`, guard `(null,0)`, and gain a miri harness. | yes |
| **M37** | R4-F1, R4-F2, R4-F3, R4-F5, R4-F7, R5-F1, R5-F2, R5-F3, R5-F4, R5-F7, R5-F8, R1-F1, R14-F1 | New prerequisites M49 and M40. Rule 4 must pick a profile (release showcase 2.74 s / 2 s; debug 0.66 s). **T37.1's gate blesses resets** of palette, scale, focus, overlays, subscriptions and inbox. T37.3's error bar cannot see Option/NaN or nested structs. T37.4 step 5 cannot see kept code text or lost widget state. Agent forwarding widens the S0 surface: M39 first. | T37.0 either way (state the arm mismatch); T37.1 only if framework-side; T37.3 only top-level non-Option; T37.4 yes. |
| **M29 T29.2** | R11-F5, R2-F5 | `just const-budget` times a no-op; fix the recipe first. | pointer-identity gate yes. |
| M29 T29.3/T29.4 | R6-F2, R6-F3, R17-F8 | Measure whole pump (36% → 25.5%); `grid` in CI is gate reality. | off-by-one yes; T29.1's verdict stands. |
| **M29 T29.6** | R1-F4, R1-F2, R2-F1/R10-F4, R6-F3, R6-F2, R18-F8, R18-F6, R3-F1, R18-F7 | New prerequisite R1-F4 (narrowing exposes it on every click). The consumed set must cover `compose` and `paste`. The kept arena cascade (21.5 µs) exceeds the saving (21.1 µs): needs finding 9. Assert clicks land. Depends on M44. | Injection 1 yes; 2 only with distinct overlay areas (add a stacked fixture); 3 yes. |

**Kill rules and injections that cannot fire as written:** M32's
re-stated M31 gate; M33/T33.3's drive gate; M34/T34.0's converted
drives; M34/T34.1 (3 of 4 defects); M34/T34.2's instrument; T29.2's
const-budget; M37/T37.3's error bar. Everything else examined can fire.

## 8. Proposed follow-up milestones (M39+)

> **Superseded as a proposal, 2026-09-25.** `docs/plan-m38-fixes.md`
> plans M39–M55 against `main` at 1717808, after re-triaging every
> finding there. The table below is the review's proposal as written, and
> is kept for the record. Where it differs from the plans (membership,
> order, owners), the plans are right.

In dependency order. Each closes named findings and names the injection
its gate must turn red. Every SHA is a reviewer's scratch commit.

| # | milestone | depends on | closes | gate, verified by injection |
|---|---|---|---|---|
| **M39** | Agent transport hardening: refuse HTTP-shaped lines; per-session token; loopback only without override; a thread per connection with a line cap; `catch_unwind` in `answer_agent`; envelope validation | — | R14-F1, R14-F2, R14-F3, R14-F5, R14-F6, R14-F7 | commit `r14_transport.rs` (`f927617`); reverting `4ac7e2d` turns idle-client and flood red; an Origin text/plain POST must not open the menu; `e00c288` answers -32603 instead of exit 101 |
| **M40** | Physical-space damage and edge snapping; a Host-level scale→memo test | — | R1-F1, R1-F8, R9-F5, R1-F7 | oracle at 1.5 and 2.0: reverting prints bbox [63,30,117,81], 2073 px; removing the snap brings back the 32 px row; d2 turns the memo test red |
| **M41** | Boundaries completeness: mark on compose, on tick; a Repeat decision; probe overrides; table clauses; Host-level compose and mark tests; T34.1 redesign | M40 | R1-F2, R1-F3, R1-F4, R1-F5, R1-F6, R5-F6, R10-F9, R18-F6, R18-F8, R2-F5 | R1's oracle (`2feb332`) promoted: s16 72 px, s17 908 px, s24 912 px red on revert; probe override removal → x=8; mark-2 (`1cb070a`), I4a (`9a74944`), I3 (`3051257`) red |
| **M42** | Drive-harness and live-gate integrity: strategy/features/pid in `app.diagnostics`; refuse a pre-bound port; pinned coverage + `pub use` check; skip = 77; live_gate exit codes; empty damage fails; Select, TextArea, ScrollView inventories | — | R18-F1, R18-F2, R18-F3, R18-F9, R17-F8, R13-F8, R18-F11 | I5a `1a1f478`, leftover `3d3333a`, I5b/I5c `9623a3e`, I6 `481773e`, `1beed32` red; DISPLAY removed exits nonzero under `check` |
| **M43** | Static-gate and CI hygiene: `forbid(unsafe_code)`; per-package tests; agent dev-dep observe; release-shape without observe edge; no_std runtime crate; golden per crate; LUMEN_REQUIRE_GPU; pinned test face; thumb target; drift-check fixes; `--locked` | — | R17-F1, R9-F4, R17-F2, R4-F4, R17-F3, R17-F4, R17-F5, R17-F6, R17-F7, R17-F9, R11-F6, R17-F10, R17-F11, R17-F12, R6-F6, R17-F14, R17-F15, R17-F16, R3-F8, R13-F7, R12-F1, R12-F2, R12-F7 | `6881359` red where golden runs; `dea0bb6`, `6105909` fail to compile; `520ed2e` fails release-shape; `fa64952` red; adapter-less red under the flag; `--bin serde`, `ghostr17`, trailing-comment plants red |
| **M44** | Input semantics and keyboard reach: arrows to the widget first; `post` reports refusals, merges moves; press/release latch with row key; capture cleared on hidden; `slot(k)`; keyboard rows; modal-only trap with focus restore; touch promotion; cursor from the arena hit | M42 | R3-F1, R10-F3, R3-F2, R3-F3, R3-F4, R3-F5, R3-F6, R3-F7, R3-F9, R3-F10, R3-F11, R3-F12, R15-F4, R15-F7, R18-F7 | reverting `11492f9` reddens `r3_q3_*`; reverting `4c6bab6` gives 5/10; 65 events red, 64 green; `85ad947` 0/3; `7f47109`, `78e8e07` fail a two-overlay fixture |
| **M45** | Text correctness: pen width; measurer + scale in memo keys; `paste_with`, newline as Enter; graphemes; multi-codepoint keys via compose; fallback faces; clipboard routing; word/line motion; NumberInput compose/selection; BakedFace wrap; shaped paint or measure-only; per-line runs; the `line_of` quadratic | M41 | R10-F1, R10-F2, R10-F4, R2-F1, R2-F6, R8-F4, R10-F5, R10-F6, R10-F7, R10-F8, R10-F10, R10-F11, R10-F12, R10-F13, R10-F14, R10-F15, R8-F3, R8-F5, R16-F7, R2-F10 | reverting `dcde5ef` returns the containment gate to 205/1200; `7e0b30d` control equals typed (169 vs 168); `ec07f29` fails a TextArea paste of "a\nb"; `ca8547c`, `44fac1a` fail a visual-order RTL assertion |
| **M46** | Hash identity: nonlinear text fold in `Recording`, `FoldedText`, `hash_from`; duplicate-id refusal; `concat_table` bounds | — | R2-F2, R2-F3, R2-F4, R2-F7, R2-F8, R2-F9 | reverting `e8bbd21`/`1f78718`: 1440 stale px, stale width; `r2_dup` refuses |
| **M47** | Timers, wake and lifecycle: redraw at the deadline; arithmetic catch-up; outbox-loss accounting; no thread per firing; skip empty presents; report dropped GPU presents; NaN guards; failure counter; `try_from_secs` | — (before M36) | R7-F1, R7-F2, R7-F3, R7-F4, R7-F5, R7-F6, R7-F7, R7-F8, R7-F9, R7-F10, R7-F11, R15-F6 | a one-read-after-unpolled-idle gate: `294dee1`, `14477cf` red; the 1e9 case returns; 100/100 accounted; 0 clones per firing; `bb2649a` redraws |
| **M48** | Agent truthfulness: waitSettled from host state; real FrameTiming or null; one frame counter; one serialiser with toggled/numeric/hidden/kind; replay checks; `app.quit`; one env var; buffer reuse | M39 (before M34 T34.1/T34.2) | R13-F1, R13-F2, R13-F3, R13-F4, R15-F10, R13-F6, R13-F9, R13-F10, R13-F11, R14-F4 | `4ccbf52` fails an unsettled-during-Spinner test; zeros fail `ui_check`'s n=0 case; `580f9f6` returns Err; 4.00 goes red in `just test` |
| **M49** | Reload state completeness: Reloadable for stateful widgets (user half); enum derive; Button saves nothing; Repeat count/extent decision; omit key on save failure; nested `serde(default)`; spanned errors + `on_unimplemented`; derive with reload off; validate before drop; null guard; miri shim harness; framework-side cross-swap chain test | M36/T36.4 | R5-F1, R5-F2, R5-F3, R5-F4, R5-F7, R5-F8, R5-F9, R4-F3, R4-F5, R4-F6, R4-F7, R4-F8 | `8f88545` 7/9 lost → 0; no-op Toggle restore reported LOST; Option NaN row logs; miri `null` red before the guard; `d2b71c9` refused before `hot_drop`; a `79e7c7c`-shaped framework injection red |
| **M50** | Software raster and allocation truth: truetype arm + glyph cache; threads arm with a process-wide counter or a pool; shape once per frame when banding; damage-bounded glass; resize retention; clip tests for every Cmd; SIMD_SHADOW const | M40 | R8-F1, R18-F4, R8-F2, R8-F6, R8-F7, R8-F8, R8-F10, R18-F10, R6-F5 | `77f355a` red before the fix; I2 `045e0b3` seen process-wide; `68443f6` fails a resize arm; `c472f80` fails a shadow clip case; I8 `deeea8b` fails the const assert |
| **M51** | GPU fidelity and footprint: multi-page atlas + give-up counter; Image, scale 1.25 and border cases with per-region bounds; 5-panel oracle; CPU adapters absent under Auto; reused Vecs; lazy glass scratch; SVG vocabulary | M40, M43 | R9-F1, R9-F2, R9-F3, R9-F6, R9-F7, R9-F8, R9-F9, R9-F10, R9-F11, R9-F12 | `70f4713` fails an over-one-page case; `7415a9d` fails Image parity; `ecaf67f` fails `r9_deps` (1,822 px); `c68c51c` fails a per-region bound; `c75d6c4` fails SVG |
| **M52** | Embedded gates that price a real device: flt2dec/soft-f64 symbol gate; interactive firmware in `check_text`/`check_bss`; qemu stack gate; non-incremental const-budget; paint-only cascade walk | M43 | R11-F1, R11-F2, R11-F3, R11-F4, R11-F5, R12-F4, R17-F13 | `2189c21` 18/0 red; `12e6d3d` 6/0 red; `937885b` 5,016/4,096 red; `6697ad2` fails `check_bss` on the interactive binary; `b634311` with the lint allowed fails on rerun |
| **M53** | Frame-path cost: idle-frame skip; whole-pump and cascade columns in `grid` and `stages`; const arena-owner list, `arenas_dirty` only on structural change; O(changed) focus/hover sweeps | M34, with M29/T29.6 | R6-F1, R6-F2, R6-F3, R6-F4, R3-F12 | I3 `b50d738` moves the new columns; Toggle-click cascade calls 40/40 → 0 with identical pixels; idle Boundaries pump prints ≈0 place/paint/finish |
| **M54** | Accessibility and platform: a winit `a11y` feature split from `observe`; labels and roles for 20 widgets; actions; role table test; assemble or re-mark T15.7/10/11; Drag size test; contrast | M48 | R15-F1, R15-F2, R15-F3, R15-F5, R15-F8, R15-F9, R16-F8, R16-F9, R12-F6 | `d85e851` fails the role table; `45fa10a` fails `size_of::<Drag<()>>() == 24`; `fd31a02` fails a 3:1 painted-contrast check |
| **M55** | Derive, lint and doc DX (hygiene folds into T33.1) | M33 | R16-F1, R16-F2, R16-F3, R16-F4, R16-F5, R16-F6, R16-F10, R16-F11, R12-F3, R12-F5 | W1002 retain-filter fails new positive and ScrollView-negative cases; m08 with a `changed` extra refused; `11d3b34` fails new second-`#[tree]` and columns=0 tests; a NaN-spacing fixture hits a new debug_assert |

**Order.** M39, M40, M42, M43 and M47 have no prerequisites and can run
in parallel (M47 before M36). Then M41, then M32, then M33 (with M55's
hygiene in T33.1), then M34. M48 precedes M34/T34.1 and T34.2, and M35
absorbs its `live_nodes`, allocation and R14-F8 parts. M44 and M45 land
before M29/T29.6, which follows M34/T34.2; M53 follows T29.6. M36, then
M49, then M37. M50–M52 and M54 are independent once M40 and M43 land.

## 9. The refuted appendix

**No finding was refuted.** Every one of the 182 received at least one
CONFIRMED or PLAUSIBLE verdict, and no verifier returned REFUTED. The
nearest misses are the eighteen PLAUSIBLE findings, plus two confirmed
findings where one of two verifiers returned PLAUSIBLE (R1-F4, R7-F2):
the number held and the story or the severity did not. They are recorded here because an argument that
nearly held is the one worth keeping:

| finding | what the verifier disputed |
|---|---|
| R1-F4 | the skeptic (S2): any unmarked change is stale under Boundaries; Repeat is the case its own rustdoc promised |
| R1-F7 | severity: latent, and a working gate is easy to write (S3) |
| R2-F4 | mix8 separates the one solved pair, not same-length collisions in general |
| R3-F8 | not vacuous: the tests are real in every gate that runs; they turn falsely red per package (S3) |
| R3-F12 | the quoted claim is about handler calls; the sweep is an unclaimed cost; timings LOADED |
| R4-F2 | the cited figure is caveated as synthetic in its own doc; the numbers reproduce |
| R5-F1 | `#[reload(fresh)]` exists and is documented; a DX gap (S3) |
| R6-F5 | a direct shell dependency line unifies `simd`/`threads`, as counter-window does (S3) |
| R7-F2 | wake-gate is sound for the Sender wake it names; the gap is that no gate runs `run()` |
| R8-F2 | the verifier's fixture is 2–3.5× faster threaded; the doc sentence is imprecise (S3) |
| R8-F6 | the real frame never passes a 24×24 box; the whole panel repaints instead (S3) |
| R8-F10 | attribution gap confirmed; the per-face figure not re-run |
| R9-F1 | fragmentation, not size, triggers it (64.8% texel use at first failure); S2 |
| R9-F2 | `glasswall`'s wallpaper test is a coarse software image gate under `--workspace` |
| R9-F5 | the cross-renderer headline ratio; the software heaviness itself confirmed |
| R12-F3 | the seam is documented in `results-widget-crate.md` (S3) |
| R12-F4 | the cause is a no-alloc cascade loop that still calls through `node(i)`, not the vtable shape |
| R16-F5 | severity only (S3) |
| R16-F7 | the superlinear part is a quadratic `line_of` scan, not whole-document reshaping |
| R18-F6 | the shipped code is correct; a missing test (S3) |

**Premises the reviewers themselves overturned** before filing, recorded
so a later reader does not re-chase them: the cross-arm bench skips
`place` (false, R6); `ScaleFactorChanged` does not resize the surface
(false, R15); no `size_of` pins NodeState/Style/Cmd (false, R8); a
`WaitUntil` from a deadline ≤ 0 spins (false, R7); a NaN style disables
every skip (false, R16); Host, retention and damage are std-only (false,
R11); a paint-only firmware links the Node vtable (false, R12); no
crate depends on `accesskit_winit` (false, counter-window does, R15);
Select is not focusable (false, R3); T32.0's fn-pointer reload (removed
at `c8f4c5c`, R5).

## 10. Open questions carried forward

Raised by reviewers, outside their own scope or not settled in budget.
They are not findings and carry no severity.

- `audit_is_inert.rs` fails 3/3 when run per package (R1, R15); belongs with R3-F8/R13-F7.
- TextInput caret painted at x=-36 under BlockMetrics after a long commit (R1): a measurer artefact or a layout/paint disagreement.
- Repeat rows never receive hovered/pressed bits, so a row hover shade is unreachable under either strategy (R1).
- A Glass hidden by `visible(false)` appears to still record an `observe_dep` region (R1).
- `Host::adopt` re-applies `Application::style()`, dropping a hand-set sheet (R1); `restyle_subtree` does not set `arenas_dirty` (R1).
- The agent journal's `fingerprint()` omits `next_sibling`, wid, classes, flags and kind (R2).
- Text identity: keyed per process with a random seed, or not — a policy decision (R2).
- The first Tab in a fresh showcase reports handled with focus None (R3); `handled=true` from a placeholder Button is not evidence of an effect (R3).
- Should a dropped removed field be logged on reload (R4); does M37's retire-and-unload conflict with "never dlclose" and kept `&'static str` selectors (R4)?
- M37's new Host per swap resets palette, scale, input, overlays, queue, inbox, subscriptions, spawner and retention (R5); where does a fallback log line go in a window (R5)?
- A redraw per mouse move at 10k nodes costs 125 µs under Boundaries (R6); do derived containers cull paint (R6)?
- `tasks_drive.py`'s cancel check depends on the waker delivering a frame (R7); `Live::pump` does not call `Host::tick` (R7); why wgpu picks llvmpipe beside an NVIDIA device (R7, R9).
- Should NodeDesc (48 B) be pinned (R8); should the alloc gate count globally or pin one worker (R8)?
- Why llvmpipe differs from the software canvas by 54 px in shadow areas (R9); does CI skip the wgpu tests as a pass (R9, R17); glasswall exceeds `MAX_DEPS` every frame (R9); GPU `render_ns` is submit time only (R9).
- Should the embedded budget be stated for a size profile, gain a stack base term, and hold in debug (R11)? `InputQueue`'s rustdoc cites a figure `results-embedded.md` may not contain (R11).
- `Boundaries<256>::mark` is linear in occupancy, so a frame is O(k²) in marks (R12, LOADED micro-bench); should `rdyn_opaque` be a cfg flag (R12)?
- The agent endpoint serves one connection at a time (R13, now R14-F3); every read-only RPC redraws (R13); the showcase Spinner free-runs at ~3,000 frames/s (R13).
- `frame_done` measured 114 µs against the documented 45.7 (R14, one LOADED run).
- Should accessibility be split from `observe` (R15); counter-window's a11y handler turns every action into a click (R15).
- Does W1006 fire on NaN-blanked frames (R16); which hard-coded colours survive `set_palette(dark)` (R16)?
- `syn` v3.0.4 duplicated beside 2.0.119 in the winit closure (R17); `check_bss` word-splits demangled names (R17).
- A rustc 1.94.0 SIGSEGV (LLVM DWARF stack overflow) compiling `tests/painter.rs` from a copied incremental cache, not reproduced (R18); `failure.rs`'s two tests race on the global panic hook (R18); `Boundaries::default()` starts with `all=true`, a trap for unit tests (R18).
