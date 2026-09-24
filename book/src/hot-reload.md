# Hot reload

The model in one line: **throw the tree away, rebuild from new code,
graft the old values back on.**

## Three functions

* `save()` — returns a value holding only the *keep* half of the state.
* `fresh()` — the constructor from whichever dylib is currently loaded.
* `restore()` — grafts saved values onto a fresh tree, **per field, with
  fallback**.

Per field with fallback is the important phrase. Any incompatibility —
a missing field, a removed enum variant, a changed type — keeps the
**fresh** value for that field alone and logs it. One field going stale
does not abort the reload, and a reload does not silently produce a state
that no version of the code could have written.

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
# extern crate lumen_static_reload;
# use lumen_static::prelude::*;
# use lumen_static::{App as DeriveApp, Reloadable as DeriveReloadable};
# use lumen_static::style::{Length, Patch, Rule, Selector};
# use lumen_static_widgets::prelude::*;
# use lumen_static_shell::program::{application, Program};
# #[derive(StaticTree, Default)] #[widget(column)] pub struct Ui { #[widget_id("inc")] inc: Button<Tally>, value: Label<String> }
# #[derive(Default)] pub struct Tally { count: i64 }
# #[derive(DeriveApp, DeriveReloadable)] pub struct Counter { #[tree] #[reload(fresh)] ui: Ui, #[state] #[reload(reset)] #[bind(ui.value, count, "count {}")] tally: Tally }
# impl Application for Counter { fn build() -> Self { Counter { ui: Ui::default(), tally: Tally::default() } } }
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

Three things can go wrong, and none of them takes the window down:

- **A build fails.** The last good build keeps running, with the
  compiler's error in a bar over it.
- **A field no longer fits.** If a kept field's type changed, that field
  alone gets its boot value, and the bar says which one and why.
- **The new code panics.** You get the panic screen, and the next good
  reload clears it.

`just hot-window-drive` does all of this to a real window, editing a
copy of the source with `sed`. `docs/step4-hot-reload.md` has what a
reload costs and why the boundary is borrowed rather than copied.

`fixtures/hot-app` and `just hot-demo` still run the same cycle headless,
with no window. They assert each clause in turn:

- a counter, a renamed title, a repeater's row count and a scroll
  position are kept;
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

A reload is one dylib and one save/fresh/restore cycle, and it is fast
because the rebuild is one crate. That is the same seam as the compile
firewall, the erasure dial and the observability namespace — see [The
component boundary](./component-boundary.md), which is where the argument
for making all four coincide lives.

If reloads are slow, the answer is usually that the component boundary is
in the wrong place, not that reload is slow.
