# Hot reload

The model in one line: **throw the tree away, rebuild from new code,
graft the old values back on.**

## Three functions

* `save()` — returns a value holding only the *keep* half of the state.
* the program's **boot** — the application's one constructor, run again
  by whichever build is loaded now: a window's swap boots the new
  library's `program()`, and `Host::reload` re-runs the boot the host was
  built from. There is no separate reload constructor (M49 retired
  `fresh()` and `Application::build`, a second path nothing checked
  against the boot).
* `restore()` — grafts saved values onto that freshly booted application,
  **per field, with fallback**.

Per field with fallback is the important phrase. Any incompatibility —
a missing field, a removed enum variant, a changed type — keeps the
**fresh** value for that field alone and logs it. One field going stale
does not abort the reload, and a reload does not silently produce a state
that no version of the code could have written.

Two cases look like success and are not, so they are refused and logged
too: a kept value that **did not save** (serde_json cannot write a map
with non-string keys) is left out of the saved state rather than saved
as `null`, and a saved `null` over a fresh value that is not `null` —
which is how serde writes `f32::NAN` — keeps the fresh value. Without
both, a kept `Option` came back `None`, with no line.

A kept struct restores **field by field** too. Add a field to the
`#[reload(keep)]` struct that holds your state, and the fields it already
had keep their values; the new one gets its boot value, and the log names
it (`Counter.tally.step: missing from saved state`).

## `#[reload(...)]` on each field

```text
#[reload(keep)]    survives the swap
#[reload(fresh)]   comes from the new code
#[reload(reset)]   recomputed after the swap
```

Choosing between them is a design question about the field, not a
mechanical one: a scroll position and a counter are `keep`; a theme
constant or a new field is `fresh`; a cache is `reset`, because carrying
a cache across a code change is how you get a stale answer that looks
plausible.

## What a widget keeps: its user's half

The tree is kept by default, and a kept widget saves **exactly what a
user can change, and nothing the code set**. An edit to a button's label,
a static label or a row height shows after the swap; what the user
typed, scrolled, opened or chose survives it:

| widget | keeps |
|---|---|
| `TextInput`, `TextArea`, `SearchField`, `RichTextEditor` | the text and the caret |
| `Slider`, `RangeSlider`, `NumberInput` | the value |
| `ScrollView`, `Repeat`, `DataGrid` | the scroll |
| `Popup`, `Veil` (`Modal`, `Sheet`, `Drawer`), `Collapsible` | whether it is open |
| `Strip` (`Tabs`, …), `Chooser` (`PickList`, …), `Pagination`, `Picker` | the selection |
| `Tree` | which rows are expanded |
| `SplitPane`, `PaneGrid` | where the dividers are |
| `Toggle` | on or off |
| a widget enum (`#[derive(StaticTree, Reloadable)]`) | which variant shows, and its state |
| `Button`, `Label`, `Fixed`, `Spacer` | nothing: they are code |

A container keeps its children's halves too. A `Repeat`'s row count is
not kept: it comes from the next update's binding, from your data. Each
row's fallback — a saved value this build cannot use — keeps the fresh
value and is logged, and in a window the bar shows the log.

**One consequence, if your application is not split** (M33's `#[state]`):
a handler that writes a label's text directly writes *code's* half, and
after a swap the label reads what the build says again. Text that changes
at run time belongs in `#[bind]`, whose source is state and is kept. A
split application cannot write the tree from a handler, so it loses
nothing. (`crates/lumen-static-shell/tests/reload_widget_state.rs` has a
row per widget above; `reload_code_data.rs` checks the code half.)

## When a field cannot be kept

An untagged field is nested reload state, so its type must be
`Reloadable`. A plain number or a widget of your own that has none is
reported **on the field**, with the fix:

```text
error[E0277]: `u32` is not `Reloadable`, so a hot swap cannot keep or rebuild this field
  |     count: u32,
  |            ^^^ an untagged field is nested reload state, which needs `Reloadable`
  = note: tag plain data `#[reload(keep)]` to save it through serde, and anything the new
          build should supply `#[reload(fresh)]`; a widget of your own can `#[derive(Reloadable)]`
```

Every widget in `lumen-static-widgets` has an impl, stateless ones
included (they keep nothing).

## Code pointers cannot be kept, structurally

A `fn` pointer or a vtable marked `#[reload(keep)]` **fails to compile** —
`Serialize` is not implemented for it.

That matters more than it sounds. The rule "saved state cannot contain
code pointers" is a real requirement (the old dylib is unloaded; a
retained pointer into it is a crash waiting for the next call), and here
it is enforced by the type system rather than by discipline. You cannot
write the bug.

This is also why several things in the framework are `fn` pointers
rather than closures — a `Repeat`'s row configuration function, a
handler. A closure could capture, and a capture would be exactly the
thing that cannot survive.

## A window that reloads itself

Build a window with `lumen-static-winit`'s `hot` feature, and it rebuilds
its own crate as a library every time you save a source file. It swaps
the library in between two frames, and your `#[reload(keep)]` state
carries across. The application needs three things.

**The program lives in the library**, behind `export_program!`:

```rust
# extern crate lumen_static;
# extern crate lumen_static_shell;
# extern crate lumen_static_widgets;
# use lumen_static::prelude::*;
# use lumen_static::{App as DeriveApp, Reloadable as DeriveReloadable};
# use lumen_static::style::{Length, Patch, Rule, Selector};
# use lumen_static_widgets::prelude::*;
# use lumen_static_shell::program::{application, Program};
# use lumen_static_shell::text::Shaping;
# #[derive(StaticTree, Default, DeriveReloadable)] #[widget(column)] pub struct Ui { #[widget_id("inc")] inc: Button<Tally>, value: Label<String> }
# #[derive(Default)] pub struct Tally { count: i64 }
# #[derive(DeriveApp, DeriveReloadable)] pub struct Counter { #[tree] ui: Ui, #[state] #[reload(reset)] #[bind(ui.value, count, "count {}")] tally: Tally }
# fn boot() -> Counter { Counter { ui: Ui::default(), tally: Tally::default() } }
# fn windows(_: u16) -> Option<Box<dyn lumen_static_shell::program::OpenProgram>> { None }
# const TOOLS: [lumen_static::menu::MenuEntry; 1] = [lumen_static::menu::MenuEntry::new("Bump", 1)];
# const MENU: [lumen_static::menu::MenuSection; 1] = [lumen_static::menu::MenuSection { title: "Tools", entries: &TOOLS }];
# fn menu_enabled(_: &Tally) -> lumen_static::menu::MenuMask { lumen_static::menu::MenuMask::ALL }
# fn on_menu(_: &mut Tally, _id: u32) -> lumen_static_shell::program::KeyOutcome { lumen_static_shell::program::KeyOutcome::Handled }
# fn main() { let p = program(); assert_eq!(p.width, 320); }
{{#include ../../fixtures/hot-window/src/lib.rs:program}}
```

**The binary runs it.** `main` is the same with or without the feature:

```text
{{#include ../../fixtures/hot-window/src/main.rs:main}}
```

**The crate says what `hot` means** (`LUMEN_HOT_FEATURES` tells the
window which features to build the library with; the default is `hot`):

```toml
{{#include ../../fixtures/hot-window/Cargo.toml:features}}
```

`hot` is the only thing that turns the reload surface on. Without it,
`#[derive(Reloadable)]` expands to nothing and the `#[reload(..)]`
attributes are inert, so a release build carries none of it — no serde_json,
no reload crate — and the library export is `#[cfg(feature = "hot")]`.

At start-up the statically linked application runs at once and the first
build starts, so there is no wait. After that, every save follows the
same path:

1. The window runs `cargo rustc --lib --crate-type cdylib` into
   `target/reload`, with "Reloading…" in the title.
2. It opens the library only if its ABI stamp matches: the same
   compiler, features and layouts. The window **borrows** the library's
   display list rather than copying it, and a stamp is what makes that
   sound.
3. The library boots **its own** `program()` and grafts the saved state
   on. Nothing is built through a pointer into the old code.
4. Work that was **in flight** is started again under the new build:
   every job the old host had spawned and not yet landed goes through the
   new build's `job_for`. A chain whose first step was running finishes
   under the new step function. `app.reloads` reports how many jobs were
   re-issued (`docs/results-tasks.md` T49.10). This is **at least once**:
   the old worker cannot be stopped, so a job with a side effect, such as
   a write or a request, may perform it twice.

Three things can go wrong, and none of them takes the window down:

- **A build fails.** The last good build keeps running, with the
  compiler's error in a bar over it.
- **A field no longer fits.** If a kept field's type changed, that field
  alone gets its boot value, and the bar says which one and why.
- **The new code panics.** You get the panic screen, and the next good
  reload clears it.

### Several windows

A program with a window table (`Program::windows`, the platform chapter)
reloads **every** window from the new build at once. The library's
`lumen_boot` takes a program index — 0 is `program()`, `n` its window
table's entry `n` — and a swap boots each open window's session, and a
daemon's windowless one, from the new library **before it replaces any**.
If one of them panics while booting, none is replaced: every window
stays on the build it runs, and the bar says which program's boot
failed. Each new session is adopted into its own window — its own waker,
its window's scale and size, its own shaping — so a timer in one window
redraws that window and no other.

A window opened **after** a swap is booted from the build that is
loaded, never from the one the window started with; before the first
swap it is opened from the binary's own table with a saver installed, so
the first swap carries its `#[reload(keep)]` state. The window's size and
renderer always come from the binary's table, because an image source
from a library would be library code held across a swap. A daemon with
no window open swaps as soon as a build lands: the window takes a
windowless step for it. `just hot-window-drive`'s `windows` and `daemon`
legs drive all of this (`docs/results-windows.md` T89.7).

### A build's assets

Images a new build should bring with it belong in the **application's**
store, not the window's. `Program::images` is window-side: the window
reads it once, at start-up, and hands it to its backend, so a swap never
sees it. `App::images` — `#[app(images = f)]` on a derived application —
is asked on every present, through the frame (`FrameSource::images`), and
the window never holds what it answers. After a swap it asks the new
build's session, whose boot filled the store, so the new pixels are on
screen with the swap. `fixtures/hot-window`'s `#pic` is that shape: its
store is `#[reload(fresh)]`, its colour is one line of the boot, and the
drive edits that line.

The store comes first and the window's second, so an id both hold draws
the application's pixels. The GPU renderer caches pixels by id and frame
on the window's side of the swap, so the window tells its backend to
forget them when a swap lands (`Backend::invalidate_images`); otherwise an
unchanged id at frame 0 would keep the old build's picture there. While
the error bar is up, the bar draws over the running session's frame, so
the store's images stay on screen under it. The numbers are in
`docs/results-images.md` T84.6.

`just hot-window-drive` does all of this to a real window, editing a
copy of the source with `sed`. `docs/step4-hot-reload.md` has what a
reload costs and why the boundary is borrowed rather than copied.

`fixtures/hot-app` and `just hot-demo` still run the same cycle headless,
with no window. They assert each clause in turn:

- a counter, a title the user typed, a row count kept as data, and a
  scroll position are kept;
- a new field comes fresh, and a cache is recomputed;
- an enum variant removed in the new code falls back to the fresh
  value, and **logs** it.

## When the new code panics

A reload loop is where a panic is most likely — you just changed the
code — so in a debug build, or a release build with
`lumen-static-winit/reload`, a panic does not end the window. It shows
this instead, drawn as a plain display list so that nothing on it goes
through the layout or paint that may just have failed:

```rust
# extern crate lumen_static;
# extern crate lumen_static_shell;
use lumen_static::cx::BlockMetrics;
use lumen_static::display::Cmd;
use lumen_static_shell::recover::{failure_frame, PanicReport};

// A panic inside the framework, caused from the application's code.
let report = PanicReport {
    message: "index out of bounds: the len is 3 but the index is 7".into(),
    location: Some(("crates/lumen-static/src/style.rs".into(), 1147, 18)),
    app_frame: Some(("examples/counter/src/main.rs".into(), 42)),
    note: None,
};
let screen = failure_frame(&report, 480, 320, &mut BlockMetrics);
let lines: Vec<&str> = screen
    .cmds
    .iter()
    .filter_map(|c| match c {
        Cmd::Text { text, .. } => Some(screen.str_of(*text)),
        _ => None,
    })
    .collect();
assert_eq!(lines[0], "The application panicked");
assert!(lines.contains(&"at crates/lumen-static/src/style.rs:1147:18"));
// The line to open when the panic's own location is the framework's.
assert!(lines.contains(&"in your code: examples/counter/src/main.rs:42"));
assert_eq!(*lines.last().unwrap(), "Press R to retry");
```

Your state is kept as the panic left it, with no rollback, and nothing
retries by itself: R (or the agent's `app.retry`) runs the frame again.
A panic inside a hot-reloaded *library* is caught inside that library,
because it has its own copy of std and its own panic hook, and in a
window a successful reload clears the screen. `docs/step4-hot-reload.md` lists what is
caught, what is kept, and what is not recovered.

## Why the boundary matters

A reload is one dylib and one save/boot/restore cycle, and it is fast
because the rebuild is one crate. That is the same seam as the compile
firewall, the erasure dial and the observability namespace — see [The
component boundary](./component-boundary.md), which is where the argument
for making all four coincide lives.

If reloads are slow, the answer is usually that the component boundary is
in the wrong place, not that reload is slow.
