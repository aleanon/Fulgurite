# M90 — desktop integration: context menus, the menu bar (D2), the tray

> **Written 2026-09-28 as a section of `docs/plan-iced-roadmap.md`;
> revised 2026-09-29 after that plan's seven-lens review, and moved to
> its own file. Not started.** The shared byte inventory, seams (S1,
> S2), test and doc discipline, and plan-wide kill rules K1–K4 are in
> `docs/plan-iced-roadmap.md` §2–§4 and bind this plan; this file does
> not restate them.

*Size L, **split in two**: **M90a** (L: T90.0–T90.5, context menus, the
drawn menu bar, accelerators) and **M90b** (M: T90.6–T90.7, the Linux
tray). M90a is L by M80's criterion (`docs/plan-text-fit.md`'s size
note): it touches `Event`, dispatch, `Host`, `Program`, three widgets,
the agent, `translate.rs`, `Win`'s key path, the a11y adapter and
`fixtures/hot-window` (second review M14). Closes iced's "Context menus", "System menus" (drawn, on every
platform) and "Tray icon" (Linux). **M90a depends on nothing in this
plan** for what it builds: it is core input semantics plus widgets.
It has one **ordering** dependency: it lands after M85 and rebases on
it, because T90.2's button filter and M85's `Pop` visibility delivery
(`pass::tell_visibility`) both edit dispatch, and `ContextMenu` is a
deferring overlay that M85's culling
must never cull (`docs/plan-iced-roadmap.md` §5); it re-runs M85's
dispatch and culling gates. It also needs T80.0's queue entry (M28).
**M90b depends on M89**: a tray application is a daemon (T89.6), it
reopens its window
with `Request::OpenWindow { program, tag }` (S1, D1d), and its drive
reads `app.windows` with no window open, where M89 lists only the
windowless session, key 0 (G23, R1). **Deferred, with its
reopen condition:** native `muda` menus and the tray on macOS and
Windows (D2's native half). The reference machine has no macOS or
Windows target installed (`rustup target list --installed`: Android,
thumbv7em, wasm32, x86_64-linux only), so none of their gates could
run, and `docs/plan-iced-roadmap.md` §3.1 makes a skipped gate a
milestone that is not done. Reopen
when `scripts/ci.sh` has a macOS or Windows leg. Until then the drawn
bar is the menu bar on every platform, which is non-native on macOS,
and that is recorded, not hidden.*

**Read against the tree, 2026-09-29** (`553fa5e`, a descendant of
`92a4033`). What exists: `PointerButton::{Left, Right, Middle}`
(`event.rs`), translated by `translate::pointer_button`; `Menu =
Chooser<MENU>` with its take-once `take_chosen` (M57/B23);
`Popup::place`'s viewport clamp (M57/A18); `gesture::LongPress`, which a
widget cannot tick; `Host`'s own clocks for `Event::Linger` and
`Event::ScrollEnd`; `Program::on_key`, installed by `Program::host()`
and called from `Host::key_hook` inside `pump_with`. What does not:
any button check in dispatch or in `lumen-static-widgets` (21
`Event::PointerDown { .. }` arms ignore it), a `Key` for the Menu key,
a core `Action` for AccessKit's `ShowContextMenu` (only
`observe.rs`'s action-name table has it), and any way for the agent to
press a button other than Left (`input.click` hard-codes it,
`input.pointerDown`/`Up` call `Event::down`/`up`).

**Decisions.**

- **A secondary button never reaches the press machinery** (P28, C16).
  Today a right *release* activates (pass 2's `activates` matches
  `PointerUp { .. }` for any button, `pass.rs`), and a right *press*
  acts in pass 1 on every widget that matches `PointerDown { .. }`:
  collapsibles toggle, tabs select, sliders capture, fields move the
  caret, and dispatch latches `pressed` and `PressLatch` for it. So the
  filter is in dispatch, **before pass 1**, not in pass 2, and no
  widget file changes.
  - At the top of `overlaid` (and of `dispatch_at` for callers that
    skip overlays), a `PointerDown`/`PointerUp` whose `button != Left`
    skips the press block: no `swallow_release` reset, no
    `PressLatch`, no `pressed` bit, no focus move, **no capture end**
    (a right click during a left slider drag used to end the drag on
    its release, through the capture arm's `PointerUp { .. }`). Hover
    is updated as a move updates it.
  - A **Right press** first runs the outside-press dismissal, as a left
    press does, so a right click outside an open menu closes it. It is
    then delivered as `Event::ContextRequest { x, y }` from the
    overlaid hit node, bubbling through `on_event` like
    `Event::Linger` (`pass::deliver`), so right-clicking elsewhere with
    a menu open moves the menu. Its release, and every Middle event,
    reaches nothing and returns `false`.
  - Middle-click paste (X11's primary selection) and right-button drags
    are **not supported** after this; a widget that wants the secondary
    button gets `ContextRequest` and nothing else. The primary selection
    itself is M88/T88.4's, which is ordered before M90 and whose gate
    runs on the reference machine with `xclip` installed as a recorded
    prerequisite (D7), so middle-click paste waits on nothing but a
    user: it is a `Win` step (a real Middle
    press read through `get_primary` into `Session::paste`, as Ctrl+V's
    `Win::clipboard_shortcut` does), and no drive can send `Win` a real
    Middle press (agent input goes to `Session::agent`, the G5 shape),
    so it is not built here. Reopen when an application asks for it, or
    a widget needs a secondary drag.
  - **Budget:** `Event` stays 16 B (`ContextRequest` is two `f32`,
    `tests/modifiers.rs` asserts the size). No `Style`/`NodeState`
    field: their sizes and the RAM per 100 nodes are unchanged from
    their values at landing (`docs/plan-iced-roadmap.md` §2.1; D4,
    printed).
  - **Crate:** core `lumen-static`. The filter is unconditional; the
    `ContextRequest` delivery and the keyboard/AT synthesis below are
    `#[cfg(feature = "std")]`, so `firmware_interactive` links only the
    filter. A `no_std` host feeds only `Left` (the primary touch is
    promoted as `Left`), so it loses nothing.
- **Every route to a context menu becomes one event** (P29, C28).
  - **Keyboard:** Shift+F10 (`Key::F(10)` with `Mods::SHIFT`) and a new
    `Key::Menu` (`translate::key_of` maps `NamedKey::ContextMenu`; the
    agent's key-name table gains `"Menu"` and `"F1"`–`"F12"`, since
    today it knows Enter..PageDown plus one character and answers
    `"F10"` as an unknown key, `lumen-static-agent/src/lib.rs`
    :2172–2215). Dispatch synthesises
    `ContextRequest` at the focused node's top-left plus its height
    (the point under the node), bubbling from it. If nothing consumes
    it the key continues as a key, so `on_key` still sees Shift+F10 in
    a tree with no context menu.
  - **Touch:** the Host holds one `gesture::LongPress` for the primary
    touch, fed in `pump_with` and ticked in `Host::tick` beside the
    linger clock; `next_deadline_ms` includes its remainder, or an idle
    window would never fire it (the M28/T28.12 lesson). When it fires,
    the Host dispatches `ContextRequest` at the touch point and marks
    the touch **spent**: `promote_touch` then turns its `End` into a
    `PointerCancel`, not a `PointerUp`, so a control that activates on
    **release** (`Button`, `CheckBox`, a menu row) does not activate
    and the menu just opened is not dismissed. Host state, not per
    node: one `LongPress` (about 28 B) per `Host`.
    - **A press-acting control has already acted** (second review
      P16). `promote_touch` turns `Begin` into `PointerMove` plus
      `Event::down` at once (`lumen-static-shell/src/lib.rs`
      :2529–2531), so 500 ms before the hold fires a `Collapsible` has
      toggled, a `Tabs` item selected and a `Slider` captured, as the
      pass-1 list above says a press does. Cancelling `End` cannot undo
      that. Not built: deferring the primary touch's press until the
      hold resolves or slop is exceeded, which would delay every tap's
      press, and its press shade, by the tap's duration. Reopen when an
      application puts a context menu over a press-acting control, with
      the tap latency measured on the deferred path. Stated in the
      `Event::ContextRequest` rustdoc and `book/src/gestures.md`.
  - **Assistive technology:** core `Action::ContextMenu`;
    `a11y::action_of` maps `accesskit::Action::ShowContextMenu` to it;
    dispatch delivers it as `ContextRequest` at the target's rect.
    `Action` stays 8 B (`SetValue(f32)` is the largest). A repeater
    row's synthesised node still understands only `Default`
    (`Session::a11y_action`'s rustdoc); recorded.
- **A menu is data, and every activation is one hook** (A23, A14, M22).
  - Core `lumen_static::menu` (no code, so no firmware cost): `MenuEntry
    { label: &'static str, id: u32, accel: Option<Accel> }`, `Accel {
    key: Key, mods: Mods }`, `MenuSection { title: &'static str,
    entries: &'static [MenuEntry] }`, `MenuMask(u64)` (bit *n* = the
    *n*-th entry in flattened order; `MenuMask::ALL`). **Named
    `MenuEntry`, not `MenuItem`**: `MenuItem` is allowlisted as
    AccessKit's role name (`scripts/doc-identifiers.allow`), and a live
    token of the same name would fail `check_doc_identifiers.sh` and
    make every doc mention ambiguous. At most 64 entries per menu,
    refused at `Program::menu`/`Program::tray` with a sentence naming
    the count.
  - Core `Event::Menu { id: u32 }` (16 B). A **native** activation (the
    tray today, `muda` when the native half reopens) reaches the session
    through the existing `Session::post`, so it is in the one input
    queue and a recorded session replays it (E0). No new `Session`
    method is needed for activation.
  - A **drawn** activation (a `ContextMenu` or `MenuBar` row) is taken
    once, the way `Chooser::take_chosen` is, through a new provided
    `Node::take_menu(&mut self) -> Option<u32>` (default `None`, the
    documented degraded default of a query hook, like `dismiss`). The
    Host asks each node in `consumed` after the event that consumed it
    — the consuming node itself (the row list, a leaf), by table index,
    so no wrapper forwards it and the provided-method trap does not
    apply. On `Consumed` overflow it walks the table once. Asked only
    when a menu hook is installed, so an application without one pays
    nothing (K90b). `std`-gated, so the firmware's vtables do not grow.
  - Both routes end in `Host::menu_hook`, beside `key_hook` in
    `pump_with`, which calls `Program::on_menu(fn(&mut A::State, u32)
    -> KeyOutcome)`. `KeyOutcome` is reused: `Handled` marks as a
    handled event marks, `Quit` sets `quit_requested`, which after M89
    closes the window the session runs in (`docs/plan-windows.md`,
    "Closing a window and quitting are different"). **From a tray or a
    daemon the hook runs in the windowless session, key 0, which has no
    window**, so `Quit` there closes nothing: M89 defines `wants_quit`
    on key 0 as ignored with one stderr line naming `Request::Quit`
    (second review A21). A hook that means to end the process pushes
    `Request::Quit`; `on_menu`'s rustdoc says so, and T90.7 gates it.
    **Installed by
    `Program::host()` exactly as `on_key` is**, so a hot swap's new
    session carries the new build's hook (rule 3): the loader holds no
    `fn` of the library's.
  - **Enabled is state, not a `fn` per entry.** `Program::menu(sections,
    enabled: fn(&A::State) -> MenuMask)`. The Host calls it when an
    accelerator matches (a disabled entry's accelerator falls through to
    `on_key`) and when the window asks for the tray's state. A drawn
    widget gets the same mask through M33's `#[bind]` value form, bound
    from the same function. One source, two readers; no `fn(&State)`
    ever reaches the loader-side `Win`, which is type-erased over
    `Box<dyn Session>` and could not call one (A23).
- **The menu bar is drawn by the framework, on every platform** (D2,
  P26). `MenuBar` in `lumen-static-widgets`, built from the same
  `&'static [MenuSection]` the program declares: a row of titles, each
  opening a `Popup<CommandList>`. The application places it (a static
  tree has no slot the framework could insert it into). Titles are
  focusable and Tab-reachable (M57/D3, not the owner's D3);
  Left/Right move between open menus,
  Down/Enter open, Escape closes and returns focus (M44/T44.9's path).
  Not built: F10/Alt focusing the bar, mnemonics, submenus, check and
  radio entries (reopen: an application that needs one).
- **Accelerators are the framework's** (P26). A key press no widget
  consumed is offered to the installed menu's accelerator table
  **before** `on_key`, with `key_hook`'s `owns_arrows` exemption. So one
  key is handled once: by a widget, by a menu entry, or by `on_key`.
  - **`Win`'s clipboard shortcuts come first, and today too often**
    (second review X3). `Win::clipboard_shortcut` runs before
    `keys::deliver_key` (winit `lib.rs`:1342–1350) and decides on
    `focused: self.session.has_focus()` (:748), which is *any* focused
    node (`session.rs`:565–566). So Ctrl+V with a `Button` or a
    `MenuBar` title focused and a non-empty clipboard is `Clip::Paste`
    (`keys.rs`:79–86) and is swallowed (`lib.rs`:772–776): an Edit
    menu's Paste accelerator would never fire. No headless gate or
    agent input passes through `Win`, so none could see it (the G5
    shape).
  - **Fixed in T90.5:** a paste consumes the key only when the focused
    node took it. `Session::paste` answers `Option<usize>` (`None`: the
    focused node's `Widget::paste` answered `None`; a caught panic is
    `Some(0)`), required, so `impl Session for Hot` follows by compile
    error. On `None` the key goes on through `keys::deliver_key` like
    any key. The routing moves from `Win::clipboard_shortcut` into a
    free `keys::clipboard_shortcut` that a test drives, as
    `keys::deliver_key` is (M45/T45.0 item 10).
  - Ctrl+C/X still copy whenever the focused node answers `selection`
    (M80's `Selectable` included), as a native toolkit copies a
    selected label; a Cut on a read-only selection posts a `Backspace`
    that nothing consumes.
  - So the order is: a focused node that takes the clipboard key, then
    a widget, then an accelerator, then `on_key`.
- **`ContextMenu<W>` and `CommandList`** in `lumen-static-widgets`.
  `CommandList` is the row leaf (drawn like `OptionList`, rows from
  `&'static [MenuEntry]`, disabled rows from a `MenuMask`, answering
  `take_menu`). `ContextMenu<W>` wraps a subtree, consumes the
  `ContextRequest` that bubbles to it, and opens a
  `Popup<CommandList>` at the point through **`Anchor::At { x: i32, y:
  i32 }`** in logical px, reusing `Popup::place`'s clamp. Integer pixels
  keep `Anchor`'s `Eq` derive (M23); a context menu does not need
  sub-pixel placement. No in-tree code compares `Anchor` with `==`.
- **The Linux tray is D-Bus, not GTK** (D2, P27). `tray` feature on
  `lumen-static-winit`, default-off, target-gated to `target_os =
  "linux"` (Android is `target_os = "android"`, so `just mobile-check`
  is unaffected). It uses `ksni` (a StatusNotifierItem with a
  `com.canonical.dbusmenu` menu, blocking API over `zbus`), pinned
  `ksni = { version = "0.3", optional = true, default-features =
  false, features = ["blocking", "async-io"] }`: `blocking` adds no
  runtime, and ksni 0.3.6 refuses to compile without exactly one of
  `tokio` (its default) or `async-io` (`src/compat.rs`:1–5). The
  default would pull `tokio`, which is not in `Cargo.lock` (second
  review P15). **The GTK
  thread D2 asked about is designed out, with evidence:**
  - `tray-icon` on Linux needs a GTK loop on the creating thread
    (`tray-icon-0.24.2/README.md`), and the `dialogs` feature's `rfd`
    already starts its own GTK thread on first use and calls
    `gtk_init_check` there (`rfd-0.15.4/src/backend/gtk3/utils.rs`:
    "You're stuck on the thread on which you first initialize GTK"). A
    second GTK thread for the tray would make `tray` + `dialogs` two
    GTK initialisations on two threads.
  - `ksni` needs no GTK, and with `async-io` its transport is already
    in the lock: `zbus` 5.19 and `async-io` 2.6 are in `Cargo.lock`
    through `a11y`'s `accesskit_unix`/`atspi`. New to the lock: `ksni`,
    `futures-channel`, `task-local` and `pastey` (none present at
    `553fa5e`). A build without `a11y` links the D-Bus stack for the
    tray alone, which is what K90c prices. The desktop's watcher is
    there too: the
    reference machine's session bus has `org.kde.StatusNotifierWatcher`
    (Cinnamon), and `libayatana-appindicator3.so.1` is installed for
    the fallback.
  - `ksni`'s menu callbacks are loader-side closures holding the winit
    proxy and an id: `move |_| proxy.send_event(UserEvent::Menu(id))`.
    `Win` answers `UserEvent::Menu(id)` by posting `Event::Menu { id }`
    to the program's **primary session**: key 0 for a daemon, else
    window 1 (`docs/plan-windows.md`'s default target). It then runs
    that session: a windowless step for key 0, which otherwise runs
    only at its deadline or on `Wake(0)` (`docs/plan-windows.md`, "A
    daemon"), or the slot's `request_redraw`; and drains the requests
    the hook pushed (second review P14). No library code pointer is
    held.
  - **Model and state cross by data** (S1's rule): new required
    `Session::tray_model(&self) -> Option<TrayModel>` (owned `String`s,
    the icon as owned RGBA, and a content hash) and
    `Session::menu_state(&mut self) -> Option<MenuMask>` (`Some` only
    when the enabled mask changed since last asked). `Win` asks for the
    model at start and after every swap, and rebuilds the tray only
    when the hash differs; it asks for the state after each presented
    frame and after each windowless pump.
  - **The icon is pixels in the spec**, `TraySpec { icon: (u32, u32,
    &'static [u8]), tooltip, menu, enabled }`, copied into `TrayModel`.
    P27's "resolve the icon from `Config.images`" is not taken:
    `Config.images` is lent to the backend while a window exists and
    taken back on suspend, a daemon may have no backend, and it is read
    once from the static binary, so a swap could never change the icon.
- **Which side of the seam each setting is on** (A5):

  | setting | consumed by | on a hot swap |
  |---|---|---|
  | `Program::on_menu`, `Program::menu` (hook, accelerators, mask fn) | `Host`, via `Program::host()` | re-installed: new session |
  | `MenuBar`, `ContextMenu` entries | the tree | new build's tree |
  | `TraySpec` content (labels, icon, tooltip, mask fn) | `Host`; `Win` reads `tray_model` | re-read, rebuilt on hash change |
  | whether a tray exists at all | `Win`, from the static binary's `Config`, once | **not reloadable**; a build that adds a tray needs a restart |

- **Budget summary.** Core gains four enum variants (`Event::
  ContextRequest`, `Event::Menu`, `Key::Menu`, `Action::ContextMenu`),
  a data module, and the unconditional filter; everything else is
  `std`-gated or outside core. Printed per task against the 80 KB
  `.text` ceiling (81,920 B, `scripts/check_text.sh`; D5, 2026-09-30),
  from the headline's 28,616 B (`firmware`) and 61,922 B
  (`firmware_interactive`, 19,998 B of headroom from the M56 merge,
  `docs/results-embedded.md`), minus whatever M80–M89 spend first
  (K90a).

**Test discipline for this milestone** (S3). T90.0 commits only gates
that compile today. Each **pins today's behaviour** with a message
naming the task that flips it (`"GAP: a right release activates;
T90.2 flips this"`), so `just test` stays green; the flipping task's
commit body records the red run of the flipped gate under that task's
injection. Every number below is printed beside its bound. Numbers are
owned by a new `docs/results-desktop-integration.md`, created in
T90.0; everything else cites it.

**Tasks — M90a.**

- **T90.0 — pins over what exists today, and the baselines.**
  - `crates/lumen-static-shell/tests/secondary_button.rs`: built with
    `Program::host()`, driven by `Host::post` and `pump`, under
    `Everything` and `Boundaries<256>`. A right press and release
    (constructed as `Event::PointerDown { button: PointerButton::Right,
    .. }`, which compiles today) on a `Button`, a `CheckBox`, a
    `Slider`, a `Collapsible`, a `Tabs` item and a `TextInput`. Pins,
    per widget, today's `handled` and effect (handler count, toggled
    flag, value, selected tab, caret index), printed. And a right
    release during a left `Slider` drag, pinned as ending the capture
    (the value no longer follows the next move).
  - A long hold on the primary touch: `Touch` Begin over a `Button`,
    `Host::tick(600.0)`, End. Pins the handler count at 1. The same
    hold over a `Collapsible` and a `Tabs` item pins their state after
    the hold (toggled, selected; printed), which T90.3 keeps (P16).
  - `translate.rs` test: `key_of(NamedKey::ContextMenu)` is `None`.
    With `a11y`: `action_of` on `ShowContextMenu` is `None`.
  - The four firmware binaries' `.text`/`.rodata`, and `size_of` for
    `Event` (16), `Action`, `Style` and `NodeState` (their sizes at
    landing, printed; `docs/plan-iced-roadmap.md` §2.1), as the
    baseline rows of `docs/results-desktop-integration.md`.
  - **Bench rows K90b can see** (second review G23). The two `dispatch`
    rows in `benches --bin invalidation` price one `PointerMove`
    through `pass::dispatch` (`benches/src/bin/invalidation.rs`
    :132–177), with no `Host` and no press, so neither can see a filter
    that branches on `PointerDown`/`PointerUp` nor a `take_menu` that
    `Host::pump_with` asks. T90.0 adds `dispatch, left press+release`
    and `dispatch, right press+release` over the same tree, and
    `pump_with, one consumed click` (a `Host` over it, no hook). They
    compile today, so the parent of every later task has them.
  - **Crate counts by K2's method** (second review B8): `cargo tree -p
    lumen-static-winit -e normal --prefix none | sed 's/ (\*)$//' |
    sort -u | wc -l` for the default build, `+dialogs` and `+a11y`,
    printed beside the manifest's figures. The manifest's 79 for
    `dialogs` (218 → 297, `Cargo.toml`:90, :115) is another method: the
    same default build reads 218 there and 95 in
    `docs/results-a11y-platform.md`. Read at `553fa5e` on 2026-10-02,
    K2's method gives 96 default, 106 with `dialogs` (+10) and 167 with
    `a11y` (+71); T90.0 records its own run. Without the `sed`, a `(*)`
    repeat counts as a second crate (117 default).
  - **Doc/static:** `docs/results-desktop-integration.md` is created,
    and its `scripts/doc-paths.allow` entry (committed by M80/T80.0) is
    removed in the same commit.
  - **Injections** (second review M14): one temporary perturbation per
    pin, each turning it red at T90.0 and reverted before the commit;
    the red run, by pin name, goes in the commit body:
    (1) pass 2 ignores a non-Left `PointerUp`: the `Button` pin's
    handler count reads 0 against its pinned 1, and the release-acting
    widgets' pins fire likewise; (2) dispatch drops a non-Left
    `PointerDown` before pass 1: the press-acting widgets' pins
    (`CheckBox`, `Slider`, `Collapsible`, `Tabs`, `TextInput`'s caret)
    fire; (3) a non-Left `PointerUp` leaves the capture: the drag pin
    sees the value follow the next move; (4) the Host turns a touch
    `End` into `PointerCancel`: the long-hold `Button` pin reads 0;
    (5) `key_of(NamedKey::ContextMenu)` answers `Some(Key::Escape)`:
    the `translate.rs` pin fires; (6) `action_of` maps
    `ShowContextMenu` to `Action::Click`: the `a11y` pin fires. Each
    flipping task still names its own injection.
- **T90.1 — the agent can press any button** (G25). `input.click`,
  `input.pointerDown` and `input.pointerUp` take `button: "left" |
  "right" | "middle"` (default `"left"`); the reply echoes it.
  `scripts/drive/harness.py`'s `click` gains `button=`. No method
  name changes, so `check_agent_methods.sh`'s sets are unchanged.
  - **Doc/static:** the parameter in `.claude/skills/verifying-apps/
    SKILL.md` and `book/src/agent.md`; rustdoc on the three arms.
  - **Gates:**
    - an agent test (in `lumen-static-agent`) whose probe widget
      records the button of the `PointerDown`/`PointerUp` in
      `ctx.event` (`EventCtx` has no `button` field, `event.rs`
      :403–426) sees `Right` for `button: "right"`, `Middle`
      for `"middle"`, and `Left` for an omitted parameter (accept
      direction); the recorded button is printed beside the requested
      one;
    - `button: "side"` answers `INVALID_PARAMS` naming the value.
  - **Injection:** parse the parameter and enqueue `Event::down` anyway
    (today's code). The probe reports `Left` for `"right"`, and the gate
    names the arm.
- **T90.2 — secondary buttons stop acting, and `Event::ContextRequest`**
  (P28, C16, C22). The filter and delivery as decided. The T90.0 pins
  flip.
  - **Doc/static:** handoff §7 ("Events must be data, not closures")
    gains a DECIDED line for the secondary-button rule pointing here;
    `docs/plan-desktop.md` T14.3 and `docs/plan-capabilities.md` T15.1
    ("a context menu opens on `PointerButton::Right`") point here;
    `book/src/events.md` gains a "Secondary buttons" section including
    the flipped test by named anchor; rustdoc on `Event::ContextRequest`,
    `PointerButton` and pass 2's comment (which says Enter, Space and
    release activate, and must now say a *left* release);
    `ContextRequest` leaves `scripts/doc-identifiers.allow`; the
    firmware and `size_of` rows in the results doc.
  - **Gates:**
    - `secondary_button.rs`, flipped: for each of the six widgets a
      right press and release is `handled == false`, runs no handler,
      leaves no `pressed` bit and no `input.press`, and does not move
      focus. **Setup landed:** the same position with the left button is
      `handled == true` and changes the effect (accept direction), both
      strategies. Printed per widget: handled, effect before/after;
    - the slider drag: a right press and release mid-drag leave the
      capture held, and the next move changes the value (printed);
    - **T90.1's agent gate is rewritten here**, since after the filter
      no widget receives a Right or Middle pointer event (second review
      G22): `button: "right"` is seen as one `ContextRequest` at the
      pressed point by a probe that consumes it, `button: "middle"` as
      `handled: false` with the reply's echo `"middle"`, and an omitted
      button still as a `Left` press (printed per button);
    - a right press outside an open `Select` list closes it
      (`consumed` holds the popup) and its release activates nothing
      beneath; a probe node inside `W` that consumes `ContextRequest`
      receives it once at the pressed point (printed);
    - **live, both strategies:** `just widget-drive` gains a
      secondary-button leg. For every inventory action that clicks,
      the same click with `button: "right"` answers `handled: false`
      and leaves the widget's `ui.regionHash` unchanged. Printed: the
      number of actions checked beside the manifest's click-action
      count (bound: equal, so a leg that silently skipped entries
      fails);
    - K90a on the four firmwares.
  - **Injections:** (1) delete the filter: the headless gate names each
    widget that acted (the verifier's list predicts `Collapsible`,
    `Tabs`, `Slider`, `TextInput`), and the live leg names each drive
    entry; (2) filter in pass 2 only, the first draft's design: still
    red on the pass-1 widgets, which is what proves the gate tells the
    two designs apart; (3) let a secondary release through the capture
    arm: the slider gate names the ended drag.
- **T90.3 — keyboard, touch and assistive-technology routes** (P29,
  C28). `Key::Menu` and its translation, the agent's `"Menu"` and
  `"F1"`–`"F12"` key names (`KEY_NAMES` 15 → 28 entries, `parse_key`
  mapping `"F<n>"` to `Key::F(n)`), Shift+F10 and Menu synthesis, the
  Host long press, `Action::
  ContextMenu`. The remaining T90.0 pins flip.
  - **Doc/static:** `book/src/gestures.md` (the Host recognises a long
    press, the one exception to the module's "an application owns
    recognizers" rule, and why: the release must be cancelled, which
    only the intake can do); the `Event::Touch` rustdoc ("Gesture
    recognition — … long-press — is not here") and `gesture.rs`'s module
    doc are amended in the same commit; `Key` and `Action` rustdoc; the
    agent's key list in `SKILL.md` and `book/src/agent.md`.
  - **Gates** (headless; a live leg needs a consumer and lands in T90.4):
    - Shift+F10 with focus on node N delivers one `ContextRequest` at
      N's bottom-left: the point is printed beside N's rect. With no
      consumer, `on_key` sees `F(10)` + Shift exactly once (count
      printed; accept direction). The Menu key likewise, and
      `key_of(NamedKey::ContextMenu) == Some(Key::Menu)`;
    - touch: after Begin, `tick(499.0)` delivers nothing; `tick(1.0)`
      delivers one `ContextRequest` at the touch point; End then
      reaches the `Button` as a cancel, handler count 0 (the T90.0 pin
      flipped). **Accept:** a hold that drifts 11 px (slop 10) delivers
      nothing, and End activates the button as a tap (count 1);
    - with a hold pending, `next_deadline_ms()` is `Some(≤ 500)`
      (printed); with none, it is what it was before the task;
    - touch over a press-acting control (P16): after the 500 ms hold
      over a `Collapsible` and a `Tabs` item their state is the T90.0
      pin's (toggled, selected; printed), and End is a cancel;
    - an agent test: `input.key` with `"F10"` and `mods: ["shift"]`
      reaches the Host as `Key::F(10)` + Shift (printed); `"F0"` and
      `"F13"` answer `INVALID_PARAMS` naming the value (both
      directions);
    - with `a11y`: `action_of(ShowContextMenu)` is `Some(ContextMenu)`,
      and `Host::action(n, Action::ContextMenu)` delivers one
      `ContextRequest` inside n's rect (printed).
  - **Injections:** (1) leave the spent touch's End as a `PointerUp`:
    handler count 1, named; (2) drop the hold from `next_deadline_ms`:
    the deadline gate prints `None`; (3) synthesise the keyboard request
    at (0, 0): the point gate prints it outside N's rect; (4) drop the
    `"F<n>"` arm of `parse_key`: the agent test prints `unknown key`
    for `"F10"`, and T90.4's live Shift+F10 action fails by name.
- **T90.4 — `ContextMenu<W>`, `CommandList`, `Anchor::At`, the menu data
  and `Program::on_menu`** (A23, A24, M4, M23). Core: `menu` module,
  `Event::Menu`, `Node::take_menu`; shell: `Host::menu_hook`,
  `Program::on_menu`; widgets: the two widgets and the anchor.
  - **Doc/static, in this commit** (S4): `ContextMenu` and `CommandList`
    exported from `lumen-static-widgets/src/lib.rs`, each with an
    `observe` impl — `CommandList` as `Role::Menu` with its labels as
    value (as `OptionList`), `ContextMenu` as `GenericContainer` with
    `showContextMenu` in its actions (a claim, like `PullToRefresh`'s) —
    so neither needs a `check_observe.sh` ALLOW line; `ROWS` entries
    and catalogue fields in
    `crates/lumen-static-widgets/tests/a11y_roles.rs` for both
    (`every_node_impl_has_a_row`), and a
    `scripts/drive/roles.py` row for `ContextMenu` only; a showcase
    catalog entry; a drive inventory `scripts/drive/widgets/menus.py`
    with a `ContextMenu` entry (no `undriven.allow` line), its entry and
    actions with their `counts` in `scripts/drive/expected.json`, and a
    `scripts/drive/keys.py` entry for the demo, whose wrapped control
    is focusable (`docs/plan-iced-roadmap.md` §3.2; second review M3);
    and
    `CommandList = ContextMenu` under `undriven.allow`'s "driven under
    another name", as `Popup = Select` is (second review M5). A
    `roles.py` row with no inventory entry fails `run.py`:99–101, and
    an exported name with neither fails `check_drive_coverage.sh`;
    `book/src/overlays.md` gains a "Context menus" section that
    includes the showcase entry by named anchor; rustdoc on `Anchor::At`
    and `Node::take_menu`.
  - **Gates:**
    - headless, both strategies: a right press at (x, y) inside `W`
      opens the list with its top-left at (x, y), and at (x, y) near the
      right edge it is clamped inside the viewport (both points and the
      viewport printed); choosing entry *k* by release calls `on_menu`
      once with `ENTRIES[k].id` (count and id printed). **Setup
      landed:** `pump`'s `handled` is true and the list node is in
      `consumed`. A disabled entry (bit clear): `on_menu` count 0 and
      the list stays open. An application without `on_menu`: `take_menu`
      is never asked (a counting probe reads 0);
    - the frame that opens the menu damages the list's box and the
      wrapper, no more (ratio printed, bound 1.00× as M27/T27.3's
      per-widget damage);
    - `size_of::<Event>() == 16` printed;
    - **live, both strategies**, `menus.py` for `ContextMenu`: open by
      right click, by Shift+F10, by the Menu key; choose by click and
      by Enter (the showcase readout shows the id); dismiss by Escape
      and by a click outside. Each action with the harness's three
      assertions; M57/D3's keyboard pass reaches the wrapped control, and
      the secondary-button leg from T90.2 now **expects** `handled:
      true` on this one entry (the manifest says so);
    - `just a11y-drive` with a new `--context NAME`: the adapter's
      context-menu action on the wrapper opens the menu (AT-SPI
      menu-node count 0 → 1, printed).
  - **Injections:** (1) ask `take_menu` of the `ContextMenu` wrapper
    instead of the consuming node: `on_menu` count 0, named; (2) ignore
    `Anchor::At` (fall back to `Below` at the parent): the point gate
    prints the parent's origin against (x, y); (3) ignore the mask: the
    disabled entry's count reads 1.
- **T90.5 — `MenuBar`, `Program::menu` and accelerators** (P26, G26).
  - **Doc/static** (S4): `MenuBar` exported with an `observe` impl
    (`Role::MenuBar`), an `a11y_roles.rs` `ROWS` entry and a `roles.py`
    row, a showcase entry, a `MenuBar` entry in
    `scripts/drive/widgets/menus.py` with its actions and `counts` in
    `expected.json`, and a `keys.py` entry (its titles are focusable;
    §3.2's list, as T90.4);
    `docs/competitive-feature-gaps.md`'s "Menus" row and
    `book/src/gaps.md`'s "A menu bar" paragraph rewritten;
    `book/src/overlays.md` gains "The menu bar" by named anchor; rustdoc
    on `Program::menu` states the 64-entry limit and the precedence
    (widget, then accelerator, then `on_key`).
  - **Gates:**
    - headless, both strategies: Ctrl+S with nothing focused calls
      `on_menu(SAVE)` once and `on_key` zero times (both counts
      printed); **accept:** with a `TextInput` focused, typing `s`
      inserts it and calls neither; a disabled Save lets Ctrl+S through
      to `on_key` once;
    - the clipboard key's precedence, through `keys::clipboard_shortcut`
      (X3): with a `Button` focused, `"x"` on the clipboard and an Edit
      entry bound to Ctrl+V, the function answers not consumed and the
      key then calls `on_menu(PASTE)` once (count printed). **Accept:**
      with a `TextInput` focused, Ctrl+V inserts `"x"` and `on_menu`
      counts 0;
    - `Program::menu` with 65 entries panics with the sentence; 64 is
      accepted;
    - a click on a title opens its menu; Right moves to the next title's
      menu (which is open is printed); Escape closes it and focus
      returns to the title;
    - **live, both strategies:** `menus.py`'s `MenuBar` entry: click a
      title, choose an entry, hover-switch between open menus, Escape,
      and the keyboard route; the accelerator by the agent's
      `input.key` with modifiers. **Why no OS-level key is injected**
      (G26): the path under test starts at `Session::post`, which is
      where `Win` puts a translated key; winit's translation is covered
      by `translate.rs`'s tests, and the reference machine has no
      `xdotool`;
    - **hot:** `fixtures/hot-window` gains a one-entry `Program::menu`
      with Ctrl+K; `just hot-window-drive` edits it to Ctrl+J in the
      copy and, after the swap, asserts Ctrl+J fires and Ctrl+K does
      not (both `handled` values printed).
  - **Injections:** (1) offer the key to `on_key` before the
    accelerators: counts read on_menu 0, on_key 1; (2) install the
    accelerator table once into `Win`'s `Config` rather than through
    `Program::host()`: after the swap Ctrl+K still fires, named; (3)
    restore `focused: has_focus()` and consume on `Clip::Paste`
    whatever `paste` answers (today's code): `on_menu(PASTE)` counts 0,
    named.
  - **Closes M90a** with `just ci-verify <sha>` green and the queue
    entry citing it (S5).

**Tasks — M90b** (after M89).

- **T90.6 — the tray's seam: `TraySpec`, `TrayModel`,
  `Session::tray_model`, `Session::menu_state`** (A14, A5). Both
  methods are **required**; `impl Session for Hot` forwards them, and a
  missing forward is a compile error, which is the gate for forwarding
  (S1). `Program::tray(TraySpec)` installs the spec through
  `Program::host()`.
  - **Doc/static:** rustdoc on both methods (what crosses, when `Some`);
    the seam table above in `docs/plan-window-reload.md`'s list of
    session-carried state.
  - **Gates** (headless):
    - `menu_state` over ten pumps in which the enabled answer changes
      once is `Some` exactly once (count printed); **accept:** a pump
      that changes it again yields a second `Some`;
    - `tray_model` after `Host::reload` against a boot whose spec
      differs returns the new labels and a different hash (both
      printed); the same boot returns an equal hash (accept).
  - **Panic guard.** Both run application code (`tray_model` reads the
    boot's spec; `menu_state` calls the app's `enabled` fn), so
    `Local` asks each through `self.caught(None, ..)` (`session.rs`,
    as `copy` does); `menu_state`'s changed-since-last-asked compare
    stays outside the guard. `fixtures/panic-app` gains a site whose
    `enabled` fn panics, at the next free site number at landing
    (printed), and `--expect-panics` rises by one from its value at
    landing; `just panic-drive`'s recovering legs assert M36's screen,
    the location and a clean retry. **Injection:** `Local::menu_state`
    calls `enabled` outside `caught`: the panic escapes the session,
    the process dies at that site and the launcher reports it.
  - **Injections:** (1) `menu_state` returns `Some` on every call: the
    count reads 10; (2) the model hash ignores labels: the reload gate
    prints two equal hashes over different labels.
- **T90.7 — the `tray` feature (`ksni`), `UserEvent::Menu`, `app.tray`,
  `examples/tray`, `just tray-drive`** (P27, G23, G26).
  - `Win`, with a tray in its `Config`, starts `ksni`'s blocking service
    at `resumed` (or at start for a daemon), rebuilds it when
    `tray_model`'s hash changes, and forwards clicks as above. `app.tray`
    (registered, labels, enabled mask, model hash) is answered by `Win`
    **before** `answer_agent`'s backend check, beside M89's
    `app.windows`. If M89 landed `app.windows` after that check, this
    task moves it: today `answer_agent` replies `"no window yet"` to
    everything when `self.backend` is `None`, so a daemon with its
    window closed cannot be observed at all (G23).
  - `examples/tray`: `daemon(boot)` with a tray (Open, a toggled
    entry, Quit). Open pushes `Request::OpenWindow { program: 1, tag }`;
    the window's own Close button pushes `Request::Close`, since its
    session is never told its key (`WindowOpened` goes to the opener,
    `docs/plan-windows.md`; second review P17, R19); the daemon closes
    a window it opened by `Request::CloseWindow { window }` with the
    key from that answer. Quit pushes `Request::Quit`, which ends the
    process: `KeyOutcome::Quit` from key 0 closes nothing (A21,
    Decisions).
  - `Cargo.toml` takes the `ksni` line from Decisions verbatim, and the
    `justfile` gains `cargo clippy -p lumen-static-winit --features
    tray --all-targets -- -D warnings` beside the `gpu` line, and
    `RUSTDOCFLAGS="-D warnings" cargo doc -p lumen-static-winit
    --no-deps --features tray` so the feature's rustdoc links resolve
    (`docs/plan-iced-roadmap.md` §3.1; second review X7).
  - **Doc/static:** `lumen-static-winit/Cargo.toml` states the feature's
    crate count, by K2's method and naming it, and stripped-showcase
    size delta, citing `docs/results-desktop-integration.md` (K2, B8);
    `book/src/platform.md` gains "## The tray" with its
    anchor; `app.tray` in `SKILL.md` and `book/src/agent.md`
    (`check_agent_methods.sh`); `windows.rs`'s rustdoc naming "a tray
    application" now points at the example; `just deny` and
    `check_locks.sh` pass with `ksni` added.
  - **Gates:**
    - **live, both strategies, on the reference machine:** `just
      tray-drive` launches `examples/tray` through
      `scripts/live/launch.py`, finds its item in
      `org.kde.StatusNotifierWatcher`'s `RegisteredStatusNotifierItems`
      whose owner's pid is the launched child's (via `gdbus`), and
      reads the item's `com.canonical.dbusmenu` layout: labels equal
      `app.tray`'s (printed). It sends the dbusmenu `Event(id,
      "clicked")` for Open — **the OS path, not a debug hook** — and
      `app.windows` goes from `[0]` to `[0, k]` (setup landed), M89's
      shape (`docs/plan-windows.md`'s `app.windows`: key 0 is the
      windowless session, its `windowless` flag printed; second review
      R1). Closing that window by its Close button leaves
      `app.windows == [0]` (an answer, not an error) and the item still
      registered. Toggling the app's state
      flips the entry's dbusmenu `enabled` property (printed). Quit
      exits with status 0 within 2 s (printed);
    - without `gdbus` the drive fails (exit 1) with the install line
      (`gdbus` is in `libglib2.0-bin`; the reference machine has
      `/usr/bin/gdbus`), never 77: a tool the reference machine is
      recorded as having is a prerequisite, not a capability (D7's rule,
      which M88/T88.4 applies to `xclip`);
    - without a StatusNotifierWatcher on the session bus, or without a
      session bus, the drive exits 77 with the reason. Per
      `docs/plan-iced-roadmap.md` §3.1 a skip is not a pass: the reference machine has the watcher, so M90b closes
      only there;
    - `nm` on the release `showcase`: zero `ksni` symbols without the
      feature, and a nonzero count with it (both printed, both
      directions: B7's falsifiable form of "default-off is free");
    - K90c.
  - **Injections:** (1) handle the click in the tray thread without
    `proxy.send_event`: `app.windows` stays `[0]`, named; (2) answer
    `app.windows` after the backend check: the drive prints the `"no
    window yet"` error where it expected `[0]`; (3) exit when the last
    window closes (M89's rule inverted): the drive reports the process
    gone before Quit; (4) post `Event::Menu` to a window slot rather
    than the primary session (P14): `app.windows` stays `[0]` after
    Open, named; (5) the tray's Quit answers `KeyOutcome::Quit` instead
    of pushing `Request::Quit` (A21): the 2 s exit check fails and the
    drive prints M89's stderr line; (6) build `ksni` with its default
    features (P15): K90c's crate list shows `tokio`, printed, and the
    bound fires.
  - **Closes M90b** with `just ci-verify <sha>` green and the queue
    entry citing it (S5).

**Kill rules.**

- **K90a — firmware growth** (T90.2–T90.4). Measured by M80/T80.0's
  `just firmware-delta <parent> firmware=0 firmware_interactive=256`
  (per-binary bounds now enforce both clauses; G32: an exact diff,
  since `just
  firmware-size` only prints and `check_text.sh` tolerates 1%): per
  binary, `.text`/`.rodata` before, after and Δ.
  Bounds: `firmware` (paint only) Δ **0 B** exactly; `firmware_interactive`
  Δ ≤ **256 B** per task and absolute ≤ 81,920 B (the 80 KB ceiling,
  D5). Fires: the task stops.
  If the filter itself is over, it moves behind `std` too, since a
  `no_std` host sends only `Left`; recorded with the number.
- **K90b — no menus, no cost.** With no menu hook and no
  `ContextMenu`, `benches --bin invalidation`'s `dispatch` rows (the
  two `PointerMove` rows and T90.0's press/release and consumed-click
  rows, which are the ones that can see the filter and `take_menu`)
  and
  `benches-competitive/arms/frame`'s `idle (floor)` row stay inside the
  parent's spread, eight runs a side under `just bench-lock`. Fires: the
  per-event work that fired it (the filter's branch, or a `take_menu`
  asked without a hook) is gated on the hook's presence, and remeasured.
- **K90c — the tray's cost** (T90.7). The crate count, by T90.0's
  K2 command, added to `lumen-static-winit` by `tray`, and the stripped
  release-showcase delta, measured **without** `a11y` (which already
  links the D-Bus stack) and printed with it too. Bound: more than
  `a11y`'s +71 crates (95 → 166, `docs/results-a11y-platform.md`, by
  K2's method), or more than K2's 3 MB
  (`docs/results-a11y-platform.md`'s bound, not its
  measurement), or any `unsafe` in `crates/*/src` (`check_no_unsafe.sh`).
  Fires: the Linux tray falls back to `tray-icon` on a dedicated GTK
  thread, commands crossing by `glib::MainContext::invoke`, and `run`
  refuses `tray` with `dialogs` on Linux with a sentence naming `rfd`'s
  GTK thread; recorded.
  The first draft bounded against `dialogs`' 79, which the manifest
  counted by another method (second review B8); by K2's method
  `dialogs` adds about 10 (T90.0 prints it), so the "no more than the
  most expensive feature" bound now names `a11y`, the feature whose
  stack the tray shares.

**What the review found (M90).**

- **C16** → adopted: the filter is before pass 1, in dispatch; "a right
  press activates" corrected to "a right release activates, and a right
  press acts in pass 1".
- **C22** → adopted: `(frame, handled)` cited to M27/T27.10.
- **C28** → adopted: `Key::Menu`, its `translate::key_of` mapping and
  the agent's key name are in T90.3; the second review (G21) adds
  `"F1"`–`"F12"`, without which the live Shift+F10 action could not be
  sent.
- **G23** → adopted, split with M89: `Win` answers `app.windows` and
  `app.tray` before the backend check (T90.7 moves it if M89 did not),
  and the drive asserts M89's `[0]` (the windowless session), not an
  error, under injection (2); `[]` was changed to `[0]` by the second
  review (R1).
- **G25** → adopted: T90.1 adds the `button` parameter, in its own
  commit before any right-click gate, and asserts the button that
  arrived.
- **G26** → changed: no debug hook is needed. The drawn bar is inside
  the window, so agent input is the real path; accelerators enter at
  `Session::post` like a translated key (why stated in T90.5); the tray
  click is injected through the real OS path, dbusmenu over D-Bus.
  Native `muda` (where a hook would be needed) is deferred.
- **G27** → adopted as S3: T90.0 pins behaviour that compiles today,
  and each new-API gate lands with its task.
- **G32** → adopted: K90a is an exact per-binary diff against the parent
  with a 0 B bound on the leaf, through M80/T80.0's
  `scripts/firmware_delta.sh`, cited.
- **A5** → adopted: the seam table in Decisions; the tray's existence is
  window-side and not reloadable, and says so.
- **A14** → adopted: `Session::tray_model` and `Session::menu_state`
  (T90.6), required; activation reuses `Session::post(Event::Menu)`.
- **A23** → adopted, changed: `on_menu` installed through
  `Program::host()`; `enabled` is a `fn(&State) -> MenuMask` evaluated
  in the Host, never in `Win`; activation is `Event::Menu` rather than a
  new `Session::menu` method, so it is recorded and replayed as input.
- **A24** → adopted: `ContextMenu`, `CommandList` and `MenuBar` each get
  an `observe` impl, a role row, a drive inventory and a showcase entry
  in the task that exports them.
- **P26** → adopted: native menus scoped to macOS/Windows and deferred
  (no runner); the framework draws the bar everywhere (D2); accelerators
  in the framework's key path.
- **P27** → changed: the GTK thread is designed out — `ksni` over
  D-Bus, with `rfd`'s own GTK thread as the evidence against a second
  one; `tray-icon` on a GTK thread is K90c's fallback. Enabled state
  through `Session::menu_state`. The icon is RGBA in the spec, not from
  `Config.images` (reason in Decisions). The second review (P15) pins
  `ksni` to `default-features = false` with `async-io`, the only form
  in which "transport already built" holds.
- **P28** → adopted: dispatch-level filter; no press latch, pressed bit
  or capture change for a secondary button; injection (2) is the pass-2
  filter the first draft proposed, and it must stay red.
- **P29** → adopted: the long press is the Host's, beside `Linger`, and
  its release becomes `PointerCancel`; `Key::Menu` added; gate is a
  500 ms hold whose release activates nothing beneath. Narrowed by the
  second review (P16): "nothing beneath" holds for release-activating
  controls only, and the gate now prints a `Collapsible` and a `Tabs`
  item that acted at `Begin`.
- **M3** → adopted: no docs task; each task carries its doc updates.
  The second review (M3) adds the index's §3.2 list to T90.4 and
  T90.5: `ROWS`, `expected.json` and `keys.py` beside `roles.py`.
- **M4** → adopted: every exporting task names its crate and its
  inventory, `observe`, role and showcase obligations.
- **M18** → adopted: existing chapters only — `events.md`,
  `gestures.md`, `overlays.md`, `platform.md`, `agent.md`, `gaps.md` —
  each change by named anchor.
- **M20** → adopted: every task names mechanism-breaking injections.
- **M22** → adopted: the type is `MenuEntry`; the `MenuItem` allowlist
  entry stays, since it names AccessKit's role.
- **M23** → changed: `Anchor::At { x: i32, y: i32 }` keeps `Eq` rather
  than dropping it from a public type.
- **M28** → adopted as a dependency: T80.0 creates the M80–M90 queue
  entry; M90a and M90b each cite their `ci-verify` SHA in it.

**What the second review found (M90).**

- **G21** → adopted: T90.3 adds `"F1"`–`"F12"` to the agent's
  `KEY_NAMES`/`parse_key` (15 → 28 entries), with an agent test in both
  directions, the key lists in `SKILL.md`/`book/src/agent.md`, and
  injection (4).
- **G22** → adopted: T90.1's probe reads the button from `ctx.event`,
  and T90.2 rewrites T90.1's gate (Right as one `ContextRequest`,
  Middle as `handled: false` with the echo, Left unchanged).
- **G23** → adopted: T90.0 adds left and right press/release
  `pass::dispatch` rows and a `pump_with` consumed-click row, so the
  parent has them; K90b names them beside the `PointerMove` rows.
- **B8** → adopted, changed: T90.0 re-measures by K2's method (96
  default, +10 `dialogs`, +71 `a11y` at `553fa5e`); K90c bounds
  against `a11y`'s +71 from `docs/results-a11y-platform.md`, since
  `dialogs` at about +10 is no longer the most expensive feature. The
  manifest rewrite and §0's "62 corrected" are the index's and T80.0's
  (request filed).
- **A21** → adopted: Decisions say `Quit` from key 0 closes nothing,
  M89 defines it as ignored with a stderr line (request filed to
  `plan-windows.md`), and T90.7's injection (5) gates the tray's
  `Request::Quit`.
- **P14** → adopted: `Event::Menu` goes to the primary session (key 0
  for a daemon, else window 1), followed by a windowless step or a
  redraw request and a drain; injection (4).
- **P15** → adopted: the `ksni` line pinned with `default-features =
  false, features = ["blocking", "async-io"]`, the four crates new to
  the lock listed, K90c measured without `a11y`, injection (6).
- **P16** → changed: the claim is narrowed to release-activating
  controls rather than deferring the press (which would delay every
  tap). The gap is recorded with its reopen condition, and T90.0/T90.3
  print `Collapsible` and `Tabs` state after a 500 ms hold.
- **P17**, **R19** → adopted: the detail window pushes `Request::Close`;
  `CloseWindow { window }` stays for the daemon, which has the key from
  `WindowOpened`.
- **M5** → adopted (verifier's second option): `CommandList =
  ContextMenu` in `undriven.allow`, an `a11y_roles.rs` row, and no
  `roles.py` row.
- **R1** → adopted: T90.7 asserts `[0]` → `[0, k]` → `[0]` with key
  0's `windowless` flag printed, citing `docs/plan-windows.md`;
  injections (1) and (2) expect `[0]`.
- **X3** → adopted: Decisions state the real precedence; T90.5 makes
  `Session::paste` answer `Option<usize>`, moves the routing into a
  testable `keys::clipboard_shortcut` that consumes Ctrl+V only when
  the focused node took the paste, gates it with a focused `Button` and
  an Edit Paste accelerator, and injection (3) restores `has_focus()`.
- **D6** → no change: this plan adds no `Painter` method and attributes
  none to D3.
- **D7** → adopted where it reaches: the middle-click note cites
  M88/T88.4's `xclip` prerequisite, and the tray drive applies the same
  rule to `gdbus` (missing: exit 1 with the install line, never 77).
  "D3" for the keyboard pass now reads M57/D3, not the owner's D3.
- **X5** → no change: rustybuzz and `deny.toml` are not in this plan.
- **M14, M28, M3, X7** (from plan-iced-roadmap.md) → adopted:
  T90.0 names six temporary perturbations, one per pin, red at T90.0
  and recorded in the commit body (e.g. pass 2 ignores a non-Left
  `PointerUp`); M90a re-sized to L by M80's criterion, with the list
  of what it touches; T90.4 and T90.5 meet §3.2 (`roles.py`,
  `a11y_roles.rs` `ROWS`, `expected.json`, `keys.py`); T90.7 adds
  the `clippy --features tray` line.
- **B5/B16 (K90a), R17 (T90.0)** (from plan-text-fit.md) → adopted:
  K90a invokes `just firmware-delta <parent> firmware=0
  firmware_interactive=256`, M80/T80.0's positional form; T90.0
  removes the results doc's `doc-paths.allow` entry when it creates
  the doc.
- consistency-3 item 10 → T90.6 routes `tray_model` and `menu_state`
  (the app's `enabled` fn) through `self.caught(None, ..)`, adds a
  panic-app site (next free, printed) and the call-outside-`caught`
  injection.
- consistency-3 item 12 → T90.7 adds `cargo doc -p lumen-static-winit
  --features tray` beside its clippy line.

## What building found (M90)

- **T90.0, the `TextInput` pin starts at caret 0.** A host with no font
  shapes no clusters, so a press anywhere in the field puts the caret at
  the run's end (`TextInput::offset_at`'s only candidate). The pin moves
  the caret home in the boot and reads 0 → 11, a press's effect either
  way; the plan's "caret index" stands.
- **T90.0, injection (2) also turns the release-acting pins red.**
  Dropping a non-Left `PointerDown` before pass 1 leaves no press latch,
  so the release that follows activates nothing (M44/T44.2's same-node
  rule): all six widgets are named, not only the four press-acting ones.
  Injection (1) names exactly `Button` and `CheckBox`. So the two pins
  together tell the release from the press, as T90.2's injection (2)
  needs.
- **T90.0, injection (6) maps to `Action::Default`.** The core has no
  `Action::Click`; `Default` is what `action_of` answers for AccessKit's
  `Click`, so the injection is "route `ShowContextMenu` as a click".
- **T90.0, the P16 pins have their own perturbation.** Injection (4)
  cannot move them (they act at `Begin`), so a fifth was run:
  `promote_touch`'s `Begin` without its press, the deferred-press design
  P16 rejected, which reads `Collapsible` and `Tabs` 0 → 0 by name. The
  three hold pins are collected and reported together, so one red run
  names all three.
- **T90.0, a size pin was added.** `the_sizes_m90_must_not_move` asserts
  `Event` 16, `Action` 8, `Key` 8, `Style` 84 and `NodeState` 88, so
  "unchanged from their values at landing" is an assertion each later
  task inherits rather than a print to compare by eye. Red under a
  temporary `Action::Wide(f64)`: `Event 24 B`, named.
- **T90.0, the consumed-click row needs a consumer.** Nothing in `Ten10`
  consumes a click, so `pump_with, one consumed click` is `Ten10` with a
  `Button` at its head (903 nodes), and the row prints and asserts the
  clicks consumed beside the samples. The press rows print three
  decimals, since the `PointerMove` rows' two are a 10 ns step.
- **T90.1, `EventCtx` does carry a `button`.** The plan says it has none
  (`event.rs`:403–426 at `553fa5e`); today's `EventCtx::button` is set
  from the event by dispatch. The probe reads the button from
  `ctx.event` as specified, and prints `ctx.button` beside it; both
  read `Right`/`Middle`/`Left` as asked. The refusal answers for any
  non-string as well (`"button": 2`), and its red run is the parser
  answering `Left` for an unknown name.
- **T90.1, the drive clients take the parameter too.**
  `scripts/agent_client.py`'s `click` gains `button=` beside
  `scripts/drive/harness.py`'s, since the harness forwards to it; both
  send it only when it is not `"left"`, so every existing drive's
  request is byte-identical.
- **T90.2, the live leg replays each click after the action's checks,
  not before it.** Sent before, its hover and press would have landed in
  the action's own change window and could have made a click that missed
  look landed (check 1). After the checks, in the state the action left,
  nothing the action measured can see it; the pointer is first rested at
  the click's point past `LINGER_MS`, so a hover is not read as the
  click's doing.
- **T90.2, a right click with a popup open is not sent, and the manifest
  says so.** A right press outside every open overlay dismisses them, as
  decided above, so "answers `handled: false` and changes nothing" holds
  only with none open. The window could not say whether one was:
  `app.diagnostics` gained `open_overlays` (`AgentHost::open_overlays`,
  provided, `null` from a host that cannot say), and the leg files each
  clicking action as `checked` or `overlay_open`, both lists in
  `scripts/drive/expected.json`'s `secondary`. So the plan's single
  "click-action count" is two lists, compared by name for the entries a
  run drives: 85 checked and 19 not sent, 104 clicking actions of 166.
  The dismissal itself is gated headlessly
  (`crates/lumen-static/tests/context_request.rs`).
- **T90.2, a disabled node takes no context request**, as it takes no
  press (M57/A16), and after a right press has dismissed an overlay the
  request goes to what is under the point without it, since the closed
  overlay is still in the list until the next paint.
- **T90.2, injection (2) names all six widgets, not only the four.** With
  the filter in pass 2 alone a release-acting `Button` and `CheckBox` run
  no handler, but the gate also reads the pressed bit, the latched press
  and focus, which a right press still set on all six; by effect alone
  the four pass-1 widgets are the ones named, as the verifier predicted.
- **T90.2 cost the interactive firmware −4 B.** The filter's hover update
  is the move's, shared as one function (`track_hover`), so the bytes the
  filter adds are paid back by the dedup.
- **T90.2 reversed T14.3's own gate.** `crates/lumen-static/tests/
  modifiers.rs`'s `a_handler_can_tell_the_buttons_apart` asserted that the
  right button activates (`docs/plan-desktop.md` T14.3's gate) and went
  red under the filter, which is this task's point; it now asserts a right
  press and release runs no handler and a left click at the same point
  does, and `docs/plan-desktop.md` T14.3 says it was superseded.
- **T90.3, the keyboard test sits on the key's own path.** Placed ahead
  of the position branch it was a discriminant test on every pointer
  event, and the first build's left press+release read 0.064–0.065 µs
  against the parent's 0.060–0.063 (K90b). Moved into the focused-key arm
  no pointer event reaches. The bisect that followed found the dispatch
  rows move by up to 7 ns with nothing but `request_context`'s inlining
  changed on the parent, so the shipped build marks it `#[cold]` and
  says why; the numbers are in `docs/results-desktop-integration.md`
  T90.3.
- **T90.3, `Action::ContextMenu` asks at the centre of its target**, not
  at the bottom-left the keyboard uses: the plan's gate wants the point
  inside the rect, and the bottom-left is on its edge.
- **T90.3, the touch is spent whether or not anything took the request.**
  A hold is not a tap even in a tree with no context menu, which is what
  T90.0's pin (a `Button` and no consumer) flips to.
- **T90.3, Shift+F10 is exactly Shift.** Ctrl+Shift+F10 is another
  shortcut and goes on as a key.
- **T90.3, the agent's `"F<n>"` parse is digits only and 1–12**, so
  `"F0"`, `"F13"` and `"F1x"` are unknown keys, refused naming the value;
  `"F"` alone is still the character.
- **T90.4, `just a11y-drive --context` cannot be built: no AT-SPI call
  produces `ShowContextMenu`.** AccessKit's AT-SPI adapter in the lock
  (`accesskit_atspi_common` 0.20.0, `src/node.rs`: `n_actions` is 1 for a
  clickable node and 0 otherwise, `do_action` sends only `Action::Click`)
  exposes no context-menu action, so the adapter can never hand the window
  the request the gate would have sent, and an AT-SPI menu-node count
  could not move. Replaced, not skipped: the agent's `input.invokeAction`
  takes `contextMenu`, which queues the same `Event::Action` the adapter's
  `ActionRequested` becomes (`a11y::action_of` already maps
  `ShowContextMenu` to `Action::ContextMenu`, T90.3), and `menus.py`'s
  "a reader's contextMenu action opens it" drives it live under both
  strategies; the role table's `every_published_action_is_handled` now
  dispatches `ShowContextMenu` too, so the wrapper's claim is checked like
  every other published action. Reopen the AT-SPI leg when AccessKit's
  adapter maps an AT-SPI action onto `ShowContextMenu`.
- **T90.4, the damage gate needed the list outside the wrapper.** The
  first fixture opened the menu inside a 360 × 240 wrapper, so "inside the
  list or the wrapper" held for any damage the size of the list, and the
  one region (the list's shadow, which drops 0.35 × its spread) fell
  outside the list's box and was credited to the wrapper at 0.44×. The
  shipped fixture opens the menu near the wrapper's bottom so the list
  hangs out of it, takes the list's box as its ink
  (`display::shadow_ink`, what retention damages), and bounds the damage
  that neither covers at 0 px² beside the 1.00× ratio. Injection (4,
  added): the wrapper paints a window-sized scrim while open — 76,719 px²
  outside, 2.03×.
- **T90.4, "the list node is in `consumed`" is read through the hook.**
  `Host` publishes no consumed set, so the gate's evidence is the hook
  running once with the row's id, which the host can only learn by asking
  the node that consumed the release; injection (1) (ask the consumer's
  grandparent, the wrapper) reads `on_menu` `[]`.
- **T90.4, the host asks every node consumed so far this pump, not only
  this event's slice.** `Consumed` de-duplicates, so a list that took a
  `Hover` earlier in the same pump is not in the release's slice; a take
  is take-once, so asking again has no effect.
- **T90.4, a release on a disabled row is consumed.** The menu's panel owns
  its releases: a disabled row (or the padding) chooses nothing and the
  list stays open, but the release does not fall through to what is under
  the menu.
- **T90.4, the secondary leg's `handled: true` is per click, from the
  manifest.** `expected.json`'s `secondary.context` names the entries whose
  replayed right click must open their menu when it lands inside the
  widget (handled true, one overlay, Escape restores the pixels), filed in
  a third list, `opened`. Only clicks inside the wrapper are; the inventory
  opens its menu near the button's bottom edge so the rows hang below it
  and their replays are ordinary `checked` ones, and a static "a left click
  presses the button and opens no menu" puts one click inside.
- **T90.4 cost `firmware_interactive` +12 B, in `pass::dispatch`.** The
  `menu` module is data and every menu routine is behind `std`, as
  decided; the bytes are the `match` over `Event` re-laid for its new
  variant, `Event::Menu` (5,396 → 5,408 B by `nm`). Inside K90a's 256 B.
- **T90.4, `CommandList` refuses more than 64 entries itself**, naming the
  count, since a drawn menu never passes through `Program::menu`; and the
  wrapped content carries a part id (`#context-menu-content`), so the
  keyboard pass and the drive address the control the menu belongs to.
- **T90.4, K90b's consumed-click row read 0.005 µs past the parent's max
  on the first eight-run set**, and the written response was applied: the
  hook test moved from inside `Host::take_menus` to its call sites, so a
  program with no menus makes no call. Twelve interleaved runs then read
  inside every spread (`docs/results-desktop-integration.md` T90.4).
