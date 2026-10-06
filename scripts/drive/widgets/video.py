"""M84/T84.8 — a video, live (docs/plan-images-in-motion.md).

The showcase's Video demo is the committed GIF (four 96 px frames at
100 ms) played as a **stream**: a `GifFeed` subscription sends each frame as
the application's message, `apply` hands its pixels to the application's
own `ImageStore` (`App::images`), and the `Video` widget shows `(id,
frame)` with a play/pause button, a seek track and the time. Every field it
shows is the store's; the widget reports play, pause and seek as intent.

The readout says it all in one line, which these actions read:
``playing, 300 / 400 ms, frame 3, seek 0.750, stale 0`` — the state, the
position over the duration, the GIF frame on show, the last fraction
`on_seek` was handed, and how many frames landed from a feed the
application had already replaced (T84.7's stop keeps it at 0).

**Every action ends paused**, as the Image demo's do: the scale-2 leg
compares each action's damaged frame with a full repaint of the same state
(M57/D6), and a picture that moves between the two reads fails it for no
defect.
"""
import re
import time

from harness import Action, Failed

PLAY = "#d_video > #video-play"
TRACK = "#d_video > #video-track"
PIC = "#d_video > #video-picture"
#: The GIF's frames: a seek to fraction f shows frame floor(f * FRAMES).
FRAMES = 4
#: One frame's delay, the plan's "± one frame" bound on a seek's position.
FRAME_MS = 100

_READ = re.compile(r"(playing|paused), (\d+) / (\d+) ms, frame (\d+), seek ([-\d.]+), stale (\d+)")


def _state(c):
    m = _READ.fullmatch(c.readout())
    if not m:
        raise Failed(f"the Video demo's readout is not its status line: {c.readout()!r}")
    return dict(playing=m[1] == "playing", pos=int(m[2]), dur=int(m[3]), frame=int(m[4]),
                seek=None if m[5] == "-" else float(m[5]), stale=int(m[6]))


def _hash(c, sel=PIC):
    return c.c.rpc("ui.regionHash", rect=c.box(sel)).get("hash")


def _set_playing(c, want):
    """Press play until the readout says `want`: setup, so asserted."""
    if _state(c)["playing"] != want:
        c.click(PLAY)
        time.sleep(0.15)
    if _state(c)["playing"] != want:
        raise Failed(f"setup: the Video demo is not {'playing' if want else 'paused'} after "
                     f"its button (readout {c.readout()!r})")


def _track_x(c, f):
    t = c.box(TRACK)
    return t[0] + (t[2] - t[0]) * f, (t[1] + t[3]) / 2, t[2] - t[0]


def _press_track(c, f):
    x, y, _ = _track_x(c, f)
    c.c.rpc("input.click", x=x, y=y)
    time.sleep(0.08)


def _paused_at_start(c):
    """Setup: paused, on the first frame (a press at the track's left end)."""
    _set_playing(c, False)
    _press_track(c, 0.0)
    s = _state(c)
    if s["frame"] != 0 or s["pos"] != 0:
        raise Failed(f"setup: a press at the track's start left {c.readout()!r}")


def _not_on(c, h_start):
    """End paused on a picture other than `h_start`: the harness's own
    before/after check reads the same rect. A press at 50% shows frame 2."""
    _set_playing(c, False)
    if _hash(c) == h_start:
        _press_track(c, 0.5)
    if _hash(c) == h_start:
        raise Failed("the picture never left the frame the action started on")


# ---- play ------------------------------------------------------------

def _watch_play(c, press):
    """`press`, then poll the picture every 25 ms for 250 ms: when did it
    first change, and was the readout playing then."""
    h0 = _hash(c)
    t0 = time.time()
    press(c)
    first, playing = None, False
    while time.time() - t0 < 0.25:
        time.sleep(0.025)
        if _hash(c) != h0:
            first = time.time() - t0
            playing = _state(c)["playing"]
            break
    return dict(h0=h0, first=first, playing=playing)


def _play(c):
    w = _watch_play(c, lambda c: c.click(PLAY))
    _not_on(c, w["h0"])
    c.__dict__["_p"] = w


def _play_outside(c):
    c.__dict__["_p"] = _watch_play(c, lambda c: c.click_outside())


def _play_expect(c):
    w = c._p
    first = w["first"]
    ok = first is not None and first <= 0.25 and w["playing"]
    when = f"changed at {first * 1000:.0f} ms" if first is not None else "held for 250 ms"
    return ok, (f"after the press the picture {when} (bound 250 ms), the readout "
                f"{'playing' if w['playing'] else 'not playing'} then")


# ---- pause -----------------------------------------------------------

def _pause(c):
    label0 = c.value(PLAY)
    c.click(PLAY)
    time.sleep(0.1)
    h0 = _hash(c)
    time.sleep(0.5)
    h1 = _hash(c)
    c.__dict__["_q"] = dict(label0=label0, label1=c.value(PLAY), held=h0 == h1,
                            playing=_state(c)["playing"])


def _pause_outside(c):
    label0 = c.value(PLAY)
    c.click_outside()
    time.sleep(0.1)
    h0 = _hash(c)
    time.sleep(0.5)
    h1 = _hash(c)
    c.__dict__["_q"] = dict(label0=label0, label1=c.value(PLAY), held=h0 == h1,
                            playing=_state(c)["playing"])


def _pause_expect(c):
    q = c._q
    ok = q["held"] and not q["playing"] and (q["label0"], q["label1"]) == ("Pause", "Play")
    return ok, (f"the button read {q['label0']!r} then {q['label1']!r} (want 'Pause' then 'Play'); "
                f"the picture {'held' if q['held'] else 'moved'} over 500 ms; readout "
                f"{'playing' if q['playing'] else 'paused'}")


# ---- seek ------------------------------------------------------------

def _seek(c, at=0.75, outside=False):
    s0 = _state(c)
    x, y, w = _track_x(c, at)
    if outside:
        c.click_outside()
    else:
        c.c.rpc("input.click", x=x, y=y)
    time.sleep(0.1)
    c.__dict__["_s"] = dict(at=at, w=w, s0=s0, s1=_state(c))


def _seek_expect(c):
    s = c._s
    s0, s1, at = s["s0"], s["s1"], s["at"]
    want_pos, want_frame = at * s1["dur"], int(at * FRAMES)
    moved = s1 != s0
    ok = (moved and s1["seek"] is not None and abs(s1["seek"] - at) <= 1.0 / s["w"] + 0.0005
          and abs(s1["pos"] - want_pos) <= FRAME_MS and s1["frame"] == want_frame)
    return ok, (f"a press at {at:.0%} of the {s['w']:.0f} px track: on_seek got {s1['seek']} "
                f"(bound {at} +- {1.0 / s['w']:.4f}), position {s1['pos']} ms (bound "
                f"{want_pos:.0f} +- {FRAME_MS}), frame {s1['frame']} (want floor({at} * {FRAMES}) = "
                f"{want_frame}); readout {'moved' if moved else 'unchanged'}")


# ---- scrub -----------------------------------------------------------

def _scrub(c, outside=False):
    """Play from the start, then drag 25% -> 50% while the feed runs: the
    press re-keys the feed, the release re-keys it again, and the host
    stops each replaced feed (T84.7). Watch 400 ms after the release for a
    frame from a stopped feed; then pause on a frame other than the first."""
    h_start = _hash(c)
    _set_playing(c, True)
    time.sleep(0.15)
    s0 = _state(c)
    x25, y, w = _track_x(c, 0.25)
    x50, _, _ = _track_x(c, 0.5)
    if outside:
        b = c.box()
        x25 = x50 = (b[0] + b[2]) / 2
        y = b[1] - 6 if b[1] > 6 else b[3] + 6
    c.c.rpc("input.pointerDown", x=x25, y=y)
    for k in range(1, 6):
        time.sleep(0.04)
        c.c.rpc("input.hover", x=x25 + (x50 - x25) * k / 5, y=y)
    c.c.rpc("input.pointerUp", x=x50, y=y)
    released = _state(c)
    seen = []
    for _ in range(8):
        time.sleep(0.05)
        seen.append(_state(c))
    # Recorded before the pause, so a pause that cannot hold (a feed the
    # host never stopped keeps sending) is reported beside the stale count
    # rather than in its place.
    d = dict(w=w, s0=s0, released=released, seen=seen, still=None)
    c.__dict__["_d"] = d
    try:
        _not_on(c, h_start)
    except Failed as e:
        d["still"] = str(e)


def _scrub_expect(c):
    d = c._d
    r, s0 = d["released"], d["s0"]
    want = 0.5 * r["dur"]
    stale = max(s["stale"] for s in d["seen"] + [r]) - s0["stale"]
    ok = (r["seek"] is not None and r["seek"] != s0["seek"] and abs(r["seek"] - 0.5) <= 1.0 / d["w"] + 0.0005
          and abs(r["pos"] - want) <= FRAME_MS and stale == 0 and r["playing"] and d["still"] is None)
    return ok, (f"a drag 25% -> 50% while playing: on release on_seek got {r['seek']} (bound 0.5 +- "
                f"{1.0 / d['w']:.4f}), position {r['pos']} ms (bound {want:.0f} +- {FRAME_MS}); "
                f"frames from a stopped feed over the next 400 ms: {stale} (bound 0); positions "
                f"{[s['pos'] for s in d['seen']]}" + (f"; then: {d['still']}" if d["still"] else ""))


# ---- the picture -----------------------------------------------------

def _picture(c):
    s0, h0 = c.readout(), _hash(c)
    r = c.click(PIC)
    time.sleep(0.15)
    c.__dict__["_i"] = dict(handled=r.get("handled"), same=c.readout() == s0, held=_hash(c) == h0,
                            readout=c.readout())


def _picture_expect(c):
    i = c._i
    ok = i["handled"] is False and i["same"] and i["held"]
    return ok, (f"a press on the picture: handled {i['handled']} (want False), readout "
                f"{'unchanged' if i['same'] else 'changed to ' + repr(i['readout'])}, picture "
                f"{'held' if i['held'] else 'moved'}")


VIDEO = ("Video", "#d_video", [
    Action(
        visible=True,
        region=PIC,
        name="play advances the picture",
        needs="the Video demo paused on its first frame",
        setup=_paused_at_start,
        do=_play,
        expect=_play_expect,
        isolate=_play_outside,
        injection="on_toggle is not called (Video::handler answers None for the play button): "
                  "the picture holds",
    ),
    Action(
        visible=True,
        region=PLAY,
        name="pause holds the picture",
        needs="the Video demo playing",
        setup=lambda c: _set_playing(c, True),
        do=_pause,
        expect=_pause_expect,
        isolate=_pause_outside,
        injection="the play button's label ignores `playing` (Video::layout never writes it): "
                  "the button reads 'Play' throughout and its pixels do not change",
    ),
    Action(
        visible=True,
        region=PIC,
        name="a press at 75% of the track seeks there",
        needs="the Video demo paused on its first frame",
        setup=_paused_at_start,
        do=_seek,
        expect=_seek_expect,
        isolate=lambda c: _seek(c, outside=True),
        injection="the fraction is computed against the video's box, not the track's "
                  "(the track's width set to the video's): on_seek gets the wrong fraction, printed",
    ),
    Action(
        visible=True,
        region=PIC,
        name="a drag from 25% to 50% ends at 50%, and no frame from before it lands after it",
        needs="the Video demo paused on its first frame, played from there",
        setup=_paused_at_start,
        do=_scrub,
        expect=_scrub_expect,
        isolate=lambda c: _scrub(c, outside=True),
        injection="the host never sets a stopped job's flag (T84.7's stop removed from "
                  "diff_subscriptions): every replaced feed keeps sending and its frames land "
                  "(stale > 0); T84.7's drain check alone removed could not be made to fire live "
                  "(docs/plan-images-in-motion.md, What building found, T84.8)",
    ),
    Action(
        name="a press on the picture changes nothing",
        needs="the Video demo paused",
        setup=lambda c: _set_playing(c, False),
        do=_picture,
        expect=_picture_expect,
        static=True,
        injection="Video::handler answers on_toggle for the picture: handled true and the "
                  "readout says playing",
    ),
])

ALL = [VIDEO]
