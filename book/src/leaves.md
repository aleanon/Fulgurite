# Leaves

A leaf is a widget with no children: it measures itself, paints itself
and answers hit tests. Five of them ship, and between them they cover
most of what an interface is made of.

## `Label`

```text
Label<S: TextStorage = &'static str>
```

Two storage choices, and the difference matters more than it looks:

* `Label` — `&'static str`. The text is fixed at compile time. A caption,
  a heading, a button's word.
* `Label<String>` — owned. The text can change, which means a `#[bind]`
  can format into it.

The reason storage is a type parameter rather than always-`String` is
`no_std`: a `&'static str` label needs no allocator at all, so an
embedded application pays nothing for the ability to have changing text
it does not use.

`set()` is the way in, and it compares before it writes — because a
`Label` caches its measurement and writing an identical string would
throw that away for nothing. See [Where state lives](./state-lives.md).

## `Button`

```text
Button<S: 'static, T: TextStorage = &'static str>
```

Two parameters: the **application state type** and the label's storage.
`on_click` is a `fn(&mut S, &EventCtx)`, which is why `S` is there — the
button's type says what it is allowed to mutate.

`Button<Counter>` has a static label; `Button<Form, String>` has one that
can be bound. Both appear in the examples.

A button measures its label, and there is a story in that: until
2026-09-03 it did not. Width was estimated as a fraction of an em per
character, the text box was one `font_size` tall, and every label
therefore sat a few pixels low. The fix and what it cost are in
`docs/results-frame-cost.md`.

## `Fixed` and `Spacer`

`Fixed { w, h }` is a box of a given size. It is the simplest thing that
occupies space, which makes it what layout tests are written with — and
it is genuinely useful as a placeholder or a rule.

`Spacer` is the one leaf that is *filler rather than content*: its
defaults are `FILL` on both axes, so it takes whatever is left. Two of
them around something is main-axis centring, as [Layout](./layout.md)
showed, because the framework has no `justify-content` and does not want
one.

## `Image`

**The core never carries pixels and never decodes.** An `Image` holds an
`ImageId`, and the *host* resolves it through `ImageSource`.

That indirection is the whole design. An embedded host resolves an id to
a `&'static [u8]` in flash; a desktop host resolves it to a decoded
buffer. The widget, the drawing command and the layout are identical in
both — so images are available in the `no_std` profile, which they could
not be if the widget owned a decoder.

`Fit` decides how the pixels sit in the box the layout gave them.

## What a leaf owes the framework

If you write one — [Writing your own widget](./custom-widget.md) is the
full account — a leaf implements `Widget` for the compile-time facts and
`Node` for the methods. The provided method bodies on `Node` are written
for exactly this case: a leaf with nothing to forward to. That is also
why they are wrong for a wrapper, which is the subject of that chapter.
