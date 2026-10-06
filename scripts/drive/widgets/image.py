"""M84/T84.3 — an animated image, live (docs/plan-images-in-motion.md).

The showcase's Image demo is the committed GIF (four 96 px frames at
100 ms) in an `Image` whose `frame` the application's tick writes from a
`Playback`, and a button that flips `playing`. The tick answers `true`
only on a frame change and `App::wake_in_ms` says when the next one is
due, so a playing image presents **ten frames a second, not sixty**, and a
paused one presents **none** — the idle rule of M47/T47.8.

Counts, not clocks: `app.presents` is the window's own count of frames it
presented and of redraws that presented nothing, answered without drawing
a frame, so reading it does not move it.

**Every action ends paused.** The scale-2 leg compares each action's
damaged frame with a full repaint of the same state (M57/D6), and a frame
that moves between the two reads would fail it for a reason that is not a
defect. An action that plays the image pauses it again before it returns,
and checks the paused frame differs from the one it started on.
"""
import time

from harness import Action, Failed

PAUSE = "#d_image_pause"
#: K84c's bound (docs/results-images.md): the median layout per animated
#: frame on this entry, in ns. Over it, `App::tick_marked` is built.
K84C_NS = 100_000
#: How many animated frames K84c's median is taken over.
K84C_FRAMES = 50


def _area(r):
    return max(0.0, r[2] - r[0]) * max(0.0, r[3] - r[1])


def _overlaps(a, b):
    return a[0] < b[2] and a[2] > b[0] and a[1] < b[3] and a[3] > b[1]


def _playing(c):
    return c.readout() == "playing"


def _set_playing(c, want):
    """Press the button until the readout says `want`: setup, so asserted."""
    if _playing(c) != want:
        c.click(PAUSE)
        time.sleep(0.15)
    if _playing(c) != want:
        raise Failed(f"setup: the Image demo is not {'playing' if want else 'paused'} "
                             f"after its button (readout {c.readout()!r})")


def _hash(c):
    return c.c.rpc("ui.regionHash", rect=c.box()).get("hash")


def _presents(c):
    r = c.c.rpc("app.presents")
    return r["presents"], r["skipped"]


def _watch(c, ms=1000):
    """Watch the image for `ms` without touching it: presents and skipped
    redraws over the wait, whether its pixels moved, and the journal's
    damage over it. The hash and the change mark are taken *before* the
    first count, so their own redraws are not in the window counted."""
    mark = c.mark()
    h0 = _hash(c)
    # An agent request is answered before the redraw it asks for: let
    # those two redraws land before the count opens, or one of them is
    # counted as the image's (the first runs read 11 on every leg).
    time.sleep(0.03)
    p0, s0 = _presents(c)
    time.sleep(ms / 1000)
    p1, s1 = _presents(c)
    h1 = _hash(c)
    # Four frames round, the image can be back on the frame it was read
    # on after a second (12 changes counting the gaps either side of the
    # count: the second full run read "held" on five legs at presents
    # Δ 10). One more read, past at least one more change, decides it.
    h2 = h1
    if h1 == h0:
        time.sleep(0.15)
        h2 = _hash(c)
    regions = c.c.rpc("ui.lastDamage", since=mark).get("regions", [])
    c.__dict__["_w"] = dict(ms=ms, dp=p1 - p0, ds=s1 - s0, moved=h1 != h0 or h2 != h1,
                            regions=regions)


def _pause_moved(c, h_start):
    """Pause, and make sure the paused frame is not the one the action
    started on (10 changes in a second can land on it, four frames round):
    the harness's own before/after check reads the same rect."""
    _set_playing(c, False)
    for _ in range(4):
        if _hash(c) != h_start:
            return
        _set_playing(c, True)
        time.sleep(0.12)
        _set_playing(c, False)
    raise Failed("the image never paused on a frame other than its first")


def _advance(c):
    """Resume, watch a second of playing, pause."""
    h_start = _hash(c)
    _set_playing(c, True)
    time.sleep(0.05)  # the press's own frames land before the count opens
    _watch(c)
    _pause_moved(c, h_start)


def _advance_expect(c):
    w = c._w
    img = c.box()
    over = [r for r in w["regions"] if _overlaps(r, img)]
    ratio = max(_area(r) for r in over) / _area(img) if over else 0.0
    ok = (w["moved"] and 9 <= w["dp"] <= 11 and w["ds"] <= 11
          and over and abs(ratio - 1.0) <= 0.01)
    return ok, (f"over {w['ms']} ms the image's pixels {'moved' if w['moved'] else 'held'}; "
                f"presents Δ {w['dp']} (bound 10 ± 1), skipped Δ {w['ds']} (bound ≤ 11, one "
                f"re-check per change); the largest damage region over it is "
                f"{ratio:.2f}x its {_area(img):.0f} px² box (bound 1.00x, {len(over)} region(s))")


def _moves_from(c, h, ms=400):
    """Whether the image's pixels leave `h` within `ms`, sampled every
    50 ms. One read after a fixed wait is not enough: four frames round,
    a resumed image can be back on the frame it paused on by then (the
    first live run read "stayed still" on both legs at 350 ms)."""
    for _ in range(ms // 50):
        time.sleep(0.05)
        if _hash(c) != h:
            return True
    return False


def _hold(c):
    """Pause; the frame holds over 500 ms; a second press resumes it; then
    pause again."""
    c.click(PAUSE)
    time.sleep(0.1)
    h0 = _hash(c)
    time.sleep(0.5)
    h1 = _hash(c)
    c.click(PAUSE)
    resumed = _moves_from(c, h1)
    c.__dict__["_h"] = dict(held=h0 == h1, resumed=resumed, playing=_playing(c))
    _set_playing(c, False)


def _hold_outside(c):
    """The same two presses, outside the button: nothing pauses or resumes."""
    c.click_outside()
    time.sleep(0.1)
    h0 = _hash(c)
    time.sleep(0.5)
    h1 = _hash(c)
    c.click_outside()
    resumed = _moves_from(c, h1)
    c.__dict__["_h"] = dict(held=h0 == h1, resumed=resumed, playing=_playing(c))


def _hold_expect(c):
    h = c._h
    ok = h["held"] and h["resumed"] and h["playing"]
    return ok, (f"after the first press the frame {'held' if h['held'] else 'moved'} over 500 ms; "
                f"after the second it {'resumed' if h['resumed'] else 'stayed still'} "
                f"(readout {'playing' if h['playing'] else 'paused'}; want held, resumed, playing)")


def _k84c(c):
    """K84c: `app.perf`'s `layout_ns` on presented animated frames — each
    new present is caught by polling `app.presents` (which draws nothing),
    and that frame's timing read at once, before anything else draws."""
    _set_playing(c, True)
    seen = {}
    last_p = _presents(c)[0]
    end = time.time() + 2 * K84C_FRAMES * 0.1 + 2.0
    while len(seen) < K84C_FRAMES and time.time() < end:
        time.sleep(0.01)
        p = _presents(c)[0]
        if p == last_p:
            continue
        r = c.c.rpc("app.perf")
        last = r["last"]
        if last.get("layout_ns") is not None and (last.get("damaged_px") or 0) > 0:
            seen[r["frame"]] = last["layout_ns"]
        last_p = _presents(c)[0]
    c.__dict__["_k"] = sorted(seen.values())
    _set_playing(c, False)


def _k84c_expect(c):
    v = c._k
    med = v[len(v) // 2] if v else None
    ok = len(v) >= K84C_FRAMES and med is not None and med <= K84C_NS
    return ok, (f"K84c: layout_ns per animated frame, median of {len(v)} presented frames "
                f"{(med or 0) / 1000:.2f} µs (bound {K84C_NS / 1000:.0f} µs, n ≥ {K84C_FRAMES}); "
                f"p10 {(v[len(v) // 10] if v else 0) / 1000:.2f}, max {(v[-1] if v else 0) / 1000:.2f} µs")


IMAGE = ("Image", "#d_image", [
    Action(
        visible=True,
        name="time advances the frame",
        needs="the Image demo paused on a frame, so the before frame is a still one",
        setup=lambda c: _set_playing(c, False),
        do=_advance,
        expect=_advance_expect,
        # Paused, the same wait: no present, no pixel moves (M47/T47.8).
        isolate=lambda c: _watch(c),
        injection="the showcase's tick writes the frame and answers false (the Boundaries "
                  "legs hold the hash; Everything stays green); or next_deadline_ms "
                  "ignores wake_in_ms (presents Δ 0 while playing)",
    ),
    Action(
        visible=True,
        region=PAUSE,
        name="pause holds the frame",
        needs="the Image demo playing",
        setup=lambda c: _set_playing(c, True),
        do=_hold,
        expect=_hold_expect,
        isolate=_hold_outside,
        injection="the button's handler sets playing = false instead of flipping it "
                  "(the resume fails)",
    ),
    Action(
        name="K84c: the whole-tree mark per animated frame is priced",
        needs="the Image demo playing, so every presented frame is an animated one",
        do=_k84c,
        expect=_k84c_expect,
        static=True,
        injection="none can lower a timing; the bound is K84c's kill rule, and a median over "
                  "it builds App::tick_marked (docs/results-images.md)",
    ),
])

ALL = [IMAGE]
