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

Decoding is the desktop shell's, behind its default-off `image` feature
(M84): `decode_png` and `decode_gif` turn a file's bytes into RGBA8
frames, and an `ImageStore` — which needs no decoder, so it is there
without the feature — holds them by id and is what a renderer asks. A
GIF frame may cover only part of the picture and say what happens to it
afterwards, so the decoder composites each frame over what the previous
one left before handing it out, and every frame it returns is a whole
picture:

```text
{{#include ../../crates/lumen-static-shell/src/image.rs:composite}}
```

What the feature costs — nine crates, and about 200 KB in a program that
calls both decoders, nothing in one that calls neither — is in
`docs/results-images.md` T84.1.

## `Video`

A video is not a codec here, and the widget is not one either. `Video`
(M84/T84.8) shows an image id at a frame, like an `Image`, and draws a
play/pause button, a seek track and the time — and everything it shows is
the application's: whether it plays, where it is, how long it is, and
which frame to paint. The pixels come from a `VideoFeed`, which the
application runs as a subscription; each frame it sends is a message, and
`apply` copies it into the application's own `ImageStore`, whose answer
is the `frame` the widget paints next. A pause un-declares the feed, a seek
declares it again under a new key, and the host stops the old one, so no
frame from before a seek lands after it. `GifFeed`, a decoded GIF played at
its own delays, is the feed that ships.

The widget's one piece of real work is the seek. Only it knows how wide
its track is, so it turns the press into a fraction itself, and the
fraction reaches the application's `on_seek` through `Node::take_action`
— the pass's route for a value a consumed gesture produced, since a handler
runs only on activation and a captured release activates nothing:

```text
{{#include ../../crates/lumen-static-widgets/src/video.rs:seek}}
```

A drag moves the knob without seeking, and its release seeks: a seek
restarts the feed, and one per pointer move would restart it sixty times
a second. What a frame costs on the frame thread (no allocation) and what
the software backend holds at 1280 × 720 are in `docs/results-images.md`
T84.8.

## What a leaf owes the framework

If you write one — [Writing your own widget](./custom-widget.md) is the
full account — a leaf implements `Widget` for the compile-time facts and
`Node` for the methods. The provided method bodies on `Node` are written
for exactly this case: a leaf with nothing to forward to. That is also
why they are wrong for a wrapper, which is the subject of that chapter.
