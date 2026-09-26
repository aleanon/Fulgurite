# M47 — a timer fires on an idle window, and the loop around it keeps count

> **T47.1 DONE 2026-09-25**, landed ahead of phase 3 with its own T47.0
> rows (the two unobserved `tasks-drive` phases, the panic-path step, the
> `control_flow_for` unit test): an idle window's 120 ms timer reads 26
> ticks where it read 1 (bound 24, both strategies), a retry after a 2 s
> panic screen adds 0 extra ticks where it added 19 (bound 2), and
> periods of 1e30 and `f32::MAX` wait without a deadline instead of
> panicking. Its drive evidence is re-gathered with the identity line
> once M42/T42.2 lands. See "What building found" at the end of T47.1.
>
> **T47.3 DONE 2026-09-26**, early, because M48/T48.2 and M49/T49.10
> read its ledger: `Threads` is a lazy pool of at most 4 workers (20
> timer firings ran on 3 threads, were 20), 6.0x faster spawn-to-apply
> than a thread per job, and the host keeps a ledger whose length is the
> in-flight count (`Host::in_flight`).
>
> **Written 2026-09-24.** Closes R7-F1, R7-F2, R7-F3,
> R7-F4, R7-F5, R7-F7, R7-F8, R7-F9, R7-F10, R7-F11 and R15-F6 from
> `docs/results-deep-review.md`, as re-triaged against `main` at
> `1717808` on 2026-09-24. Where the review and the re-triage differ, the
> re-triage is what this plan follows: the code moved under M36 and M37
> after the review was written.
>
> **Phase 3** of the fixed order: after M41 and M42. T47.1 may land
> earlier, for the reason given in its entry, but its live evidence is
> gathered again once M42 lands, because M42 is what lets a drive say
> which binary it drove.
>
> **The headline is that M37 rewrote the loop and R7-F1 survived it.**
> An idle window with a 120 ms subscription still ticks once in three
> seconds, and `just tasks-drive` is still green, because every read the
> drive makes asks for a frame. **M36 has since added a second way in**:
> the panic screen freezes the timer clock, so the first frame after a
> retry is handed the whole time the screen was up. That makes R7-F3's
> hang and R7-F4's silent loss reachable from an ordinary window, not
> only from a custom shell.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and prints the measured quantity beside its bound.*

---

## 0. The rules for this milestone

1. **A gate for a wake must not wake the window itself.** The drive's
   reads each end in a redraw request, so a gate that polls can only
   ever see a window that is being kept awake. Every check of a wake or
   a deadline in this milestone makes **one** read after a sleep with no
   traffic. This is R7-F2's lesson, and it goes into the vacuous-test
   record in T47.1 (where M43 §2.8 keeps that record).
2. **Counts, not clocks.** Ticks against periods elapsed, threads
   created against firings, presents against motions. None of these
   moves with machine load the way a microsecond figure does.
3. **R7-F1, R7-F2 and R7-F10 land in one commit.** R7-F2's second
   injection only becomes distinguishable from the stock build once
   R7-F1 is fixed, and R7-F10 rewrites the same expression.

## 1. What is wrong, at 1717808

### 1.1 Nothing turns a deadline into a frame (R7-F1, R7-F10)

`Win::about_to_wait` (`crates/lumen-static-winit/src/lib.rs:557-564`)
arms WaitUntil for `now + remaining`, where `remaining` is what the
host's timers said at the last frame. When the loop wakes at that
instant, nothing asks for a redraw: `Win` has no `new_events`, and the
only redraw requests come from input, the agent, a worker's wake or an
animation. So the timer is not checked until something else produces a
frame. The rustdoc on `about_to_wait` (`:546-556`) and on
`Host::next_deadline_ms` (`crates/lumen-static-shell/src/lib.rs:1334-1343`)
both say the opposite.

The re-triage reproduced it at `1717808` on two runs: after a click on
`#toggle-sub` and 3.00 s with no RPC traffic, the first read showed
**1 tick against about 26 expected**, and the second read showed 26 (the
catch-up the read's own frame caused). The five-line fix it tried
(`new_events` requesting a redraw on ResumeTimeReached) read 25–26 of 26
on the first read (one earlier run read 14/26 under load). Those are the
re-triage's numbers.

The same expression converts the deadline with Duration::from_secs_f32
(`:558-563`). `Timers::every` refuses only non-finite and non-positive
periods (`crates/lumen-static/src/spawn.rs:507-508`), so a period it
accepts can panic the loop (R7-F10; re-triage: periods of 1e30 and
`f32::MAX` panic, 1e9 and 1e20 do not). Downgraded to S4: it needs a
period above about 1.8e22 ms.

**New since the review, from M36.** While the panic screen is shown,
`redraw` passes a `dt` of 0 and does not advance `last_frame`
(`lib.rs:463-469`). The timers' deadline therefore never moves, so with
the five-line fix alone the loop would redraw the panic screen at the
timer's rate. And after R (`lib.rs:714`), the first frame's `dt` is the
whole time the screen was up.

### 1.2 The gates cannot see it (R7-F2)

`scripts/tasks_drive.py` polls through `settle()` (`:47`) every 20 ms,
and every agent reply ends in `request_redraw`
(`crates/lumen-static-winit/src/lib.rs:651-653`). The window is never
idle while the drive watches it. The re-triage ran both of the review's
injections at `1717808`:

- **A**, the waker install commented out (it moved to `lib.rs:292` in
  M37): the drive passed all 10 checks. An unpolled probe on the same
  build read `slow=0` at 1.55 s against 1.
- **B**, `next_deadline_ms()` replaced with `None` in `about_to_wait`:
  the drive passed all 10 checks. The unpolled probe read 1 tick against
  about 13, **identical to the stock build**, because R7-F1 already
  breaks the deadline path. B only becomes a meaningful injection after
  T47.1.

`examples/wake-gate` builds its own `ApplicationHandler` and installs its
own waker (`examples/wake-gate/src/main.rs:76`, `:170-171`), so no gate
exercises `run()`'s wiring. `docs/results-tasks.md` T28.12 credits the
tasks drive with proving both.

### 1.3 A large `dt` hangs the catch-up or loses its firings (R7-F3, R7-F4)

`Timers::advance` (`crates/lumen-static/src/spawn.rs:537-552`) catches up
one period at a time in `f32`: `while remaining <= 0 { remaining +=
period }`. For a large enough `dt`, adding the period no longer changes
`remaining`, and the loop never ends. The re-triage ported the review's
probe to `1717808`: four cases (period 16 with `dt` 1e9 and 2.7e8, period
1 with 3.4e7, period 120 with 2.2e9) **did not return within 5000 ms**;
one hour at period 16 returned with 225000 fired.

When the loop does return, each firing is a push into an `Outbox` whose
capacity is 32 (`spawn.rs:334`). `Host::tick`
(`crates/lumen-static-shell/src/lib.rs:1288-1318`) advances into a
stack-local scratch outbox and drops its `refused()` count. The
re-triage's probe: a 10 ms timer with `tick(1000)` **fired 100, delivered
32, lost 68, and counted 0** anywhere a caller can read. `advance`'s own
return value counts the refused pushes as fired.

Before M36 this took a custom or embedded shell passing a stalled `dt`.
Now a window can: keep the panic screen up and press R (§1.1).

### 1.4 A thread per firing (R7-F5)

`Host::tick`'s comment says "no thread is created for a timer"
(`crates/lumen-static-shell/src/lib.rs:1289-1294`). Under the only
spawner a window installs, `Threads` (`crates/lumen-static-shell/src/session.rs:199`),
each job is one `std` thread, and a timer firing is a job. The
re-triage ran `strace -f -e clone,clone3` on stock `tasks-demo`: **28
ticks came with 28 new clones**. `docs/results-tasks.md` T28.11's row
"zero threads for a timer" is false under the shipped spawner.

### 1.5 Frames that should have been asked for again (R7-F7, R7-F8)

`Gpu::present` (`crates/lumen-static-wgpu/src/lib.rs:1216-1230`) returns
`()` and swallows Outdated, Lost and Timeout. Its comment says dropping
one frame "is what every other backend does here too", which is false:
softbuffer panics. `Win::redraw` asks for another frame only while
something animates or the inbox has a backlog (`winit/src/lib.rs:528-534`),
so on an idle window a dropped present stays unrepaired until input
arrives (R7-F7; the review's forced-Outdated run left the window blank).
The winit `Backend::present` now returns a `Frame`
(`crates/lumen-static-winit/src/surface.rs:25`), which is a ready place
to report the drop.

`Animated::tick` (`crates/lumen-static/src/anim.rs:178`) and
`LongPress::tick` (`crates/lumen-static/src/gesture.rs:205`) clamp `dt`
with `num::clamp`, which returns a NaN unchanged. A NaN `dt` then leaves
the animation unsettled forever (R7-F8; re-triage: still unsettled after
10000 ticks against a bound of 13). The winit shell cannot produce a NaN
today; a custom shell can.

### 1.6 A failure that leaves no trace (R7-F9)

`FailFn`'s rustdoc (`crates/lumen-static/src/spawn.rs:104-107`) says the
host "counts the drop" when an application does not model a failure.
`report()` (`:217-223`) drops it, and its own comment says a spawner has
no host. The re-triage's probe: a panicking job in an application that
leaves `App::failed` at its default produced no result and moved no
counter; the panic reached stderr only.

### 1.7 Images lost on a second `resumed` (R15-F6)

`resumed_impl` (`crates/lumen-static-winit/src/lib.rs:669-697`) creates a
new window and backend on every call and hands images over with
`self.config.images.take()` (`:686`). A second `resumed` gets none, and
every image vanishes. There is no `suspended` handler. `Config::images`
says "Taken once" (`:179-190`). M32 did not add a re-creatable source:
`Program::images` (`crates/lumen-static-shell/src/program.rs:165`, set
at `:240`) is also a take-once box that `run` moves into `Config`
(`winit/src/lib.rs:867`).

### 1.8 A citation to a section that does not exist (R7-F11)

Four comments say they keep "the idle frame numbers in
`docs/results-frame-cost.md`" true: `crates/lumen-static-winit/src/lib.rs:530-531`
and `:554-555`, `crates/lumen-static/src/anim.rs:23`, and
`crates/lumen-static/src/event.rs:954` (new since the review). That file
has no idle figure at all. `just static` checks only that the file
exists.

## 2. Decisions

### 2.1 The deadline is an instant, not a remainder

- **(a) The five-line fix**: `new_events` requests a redraw on
  ResumeTimeReached. It works on an idle window. It still arms
  `now + remaining` from the last *frame*, so every wake that produces
  no frame pushes the deadline later. A steady stream of untranslated
  window events faster than the period would starve the timer.
- **(b) An absolute deadline.** `Win` keeps `Option<Instant>`, set after
  each frame from `next_deadline_ms()` measured from that frame's start.
  `about_to_wait` arms WaitUntil at that instant; `new_events` requests
  a redraw on ResumeTimeReached or whenever the instant has passed.
  While `session.failed()`, the loop waits without a deadline. On
  retry, `last_frame` is reset, so the first frame's `dt` is 0 rather
  than the screen's whole duration.

**Recommended: (b).** About 25 lines. It breaks nothing a user sees. It
changes one documented behaviour: timers do not run while the panic
screen is up, and a retry does not credit them with the screen's time.
That is written into `docs/plan-panic-recovery.md` §2.2.

### 2.2 Catch-up is arithmetic, and what does not fit is counted

- **(a) Clamp `dt`** to, say, a thousand periods. Simple, and it
  silently changes the documented catch-up count.
- **(b) Deliver every firing.** Not possible: the outbox holds 32.
- **(c) Count arithmetically and coalesce.** `remaining` and `period`
  move to integer microseconds. If `remaining <= 0`, the number due is
  computed in one step, and `remaining` is set from the remainder.
  `advance` pushes at most the outbox's free slots for each timer and
  records the rest as coalesced, per timer, where the host can read
  them. It returns delivered and coalesced separately.

**Recommended: (c).** An application that needs exact tick counts reads
the per-timer count, and one that does not gets at most a full outbox of
ticks after a stall rather than a hang. What it changes: "Catches up
rather than skipping" gains the capacity caveat, and `fired` stops
counting refused pushes.

### 2.3 The spawner stops creating a thread per job

- **(a) Documentation only.** Say "no *dedicated* timer thread; under
  `Threads`, one short-lived thread per firing".
- **(b) A small pool.** `Threads` becomes a fixed set of long-lived
  workers fed through a channel. That removes the per-job thread for
  every job, not only for timers, and keeps `docs/plan-tasks.md` §2.4's
  rule that the inbox is the only way back in.
- **(c) Run timer jobs inline on the loop thread.** Rejected: a job is
  application code and could block the frame.

**Recommended: (b), with (a)'s correction in the same commit.** What it
breaks: a blocking job now occupies a worker, and with every worker busy
the next job waits, timer jobs included. §5 names the reveal.

### 2.4 Images are taken back, not cloned

- **(a) Take back.** `Backend` gains a **required** method that hands
  the image source back. A new `suspended` handler takes the images back
  into `Config`, then drops the backend and the window. `resumed_impl`
  stays as it is.
- **(b) A shareable source**: an `Arc` over `ImageSource`, or
  `Program` taking a factory. Simpler to call, but it changes the
  public builder, and a factory re-decodes every image on each resume.

**Recommended: (a).** The new method is required and not provided,
because both backends own the source and a provided default returning
nothing would reproduce this bug. That is `AGENT.md`'s rule that a
provided method is right for a leaf and wrong for a wrapper.
`Backend::set_images` stays provided (`crates/lumen-static-winit/src/backend.rs:61-70`),
because a backend that cannot draw images is a real configuration.

## 3. Tasks

### T47.0 — the gates first

Every check below is committed **red**, with the red output in the
commit body, as M35/T35.0 and M36/T36.0 did. Three small extractions
are made verbatim, with no change in behaviour, so a unit test can reach
the code: `about_to_wait`'s conversion becomes `control_flow_for`, still
using Duration::from_secs_f32; `resumed_impl`'s backend step becomes a
function that takes a backend factory; and the idle-citation rule is
added to `just static`.

| finding | check | red at 1717808 |
|---|---|---|
| R7-F1 | `scripts/tasks_drive.py`, new **unobserved** phase: click `#toggle-sub`, sleep 3.0 s with no RPCs, then one read. Prints `ticks N (expected E, bound B) after X ms unobserved`; asserts `N >= 1 + floor(X/120) - 2` | 1 against about 26 |
| R7-F2 | the same phase for the waker: click `#start-slow`, sleep 1.5 s, one read. Prints `slow N (want M) after X ms unobserved, job takes 400` | green on stock, **red under injection A** (0 against 1) while the old phases stay green, which is the finding |
| R7-F1 (panic path) | `fixtures/panic-app` gains a 120 ms timer and a tick label. `scripts/panic_drive.py` turns it on, arms a panic, keeps the screen up 2 s, presses R, and asserts that the retry frame added at most 2 ticks, printed beside the bound | about 16 against 2 |
| R7-F3 | `crates/lumen-static-shell/tests/timers.rs`: `advance` with `dt` 1e9 at period 16, on a thread with a 200 ms watchdog. Prints elapsed time beside the bound. The 80/25 → 3 case stays | does not return |
| R7-F4 | the same file: a 10 ms timer, `tick(1000)`. Asserts `delivered + counted == fired` and prints all three; counted is the sum of every counter a caller can read today | 32 + 0 against 100 |
| R7-F5 | the same file: the spawner a session installs, driven through 20 firings, with OS threads counted from the process's task list before and after. Prints `threads created N for 20 firings (bound P)` | 20 against the pool size |
| R7-F7 | a debug-only switch in the wgpu crate, LUMEN_GPU_DROP_FIRST=n, makes the first n presents answer Outdated. A new step in `just gpu-parity`: `tasks-demo` on the GPU with n=2, idle 2 s, sample the window, assert more than one distinct pixel value, printed beside the bound | one distinct value |
| R7-F8 | unit tests in `anim.rs` and `gesture.rs`: after `tick(f32::NAN)`, 13 ticks of 16 ms must settle with a finite value. Prints frames taken against 13. The `-inf` and `1e9` arms stay | unsettled after 10000 |
| R7-F9 | `crates/lumen-static-shell/tests/failure.rs`: a panicking job, `App::failed` at its default, the `Threads` spawner, pumped until the job settles. The accessor `Host::unmodelled_failures` is added here, reading a shared counter that nothing increments yet | 0 against 1 |
| R7-F10 | a unit test in `crates/lumen-static-winit`: `control_flow_for` with `Some(f32::MAX)` and `Some(1e30)` must not panic; `Some(120.0)` must give WaitUntil at now + 120 ms ± 1 ms | panics |
| R7-F11 | `just static`: a comment citing "the idle … numbers in X" requires X to contain an idle heading. Prints the citing sites | four sites, no heading |
| R15-F6 | a unit test over the extracted resume step with a fake backend recording what it was given: resume, suspend, resume, and print `backend #2 images: …` | `None` |

**Other direction, recorded in the same commit:** the unobserved phase
run against a build carrying the re-triage's five-line diff reads 25–26
of 26; the 1 h timer case returns; the 30-firing case delivers 30 of 30.
Those show the red checks fail for the defect and not for their own
setup.

**Effort:** M. **Depends on:** M42 for the drive's identity line (the
unobserved phase prints which binary it drove). If T47.1 lands before
M42 (its entry says why it may), the rows T47.1 needs — the unobserved
phases, the panic-path step and the `control_flow_for` test — are
committed red without the identity line, and printed with it once
M42/T42.2 lands; the rest of this table waits for M42.

### T47.1 — the deadline is an instant (one commit with R7-F2 and R7-F10)

**Closes:** R7-F1, R7-F2, R7-F10.

**Change:** §2.1(b) in `crates/lumen-static-winit/src/lib.rs`: an
absolute deadline on `Win`; `new_events`; no deadline while
`session.failed()`; `last_frame` reset on retry. `control_flow_for`
uses the fallible conversion and falls back to waiting without a
deadline. `scripts/tasks_drive.py` keeps its observed phases and gains
T47.0's unobserved ones permanently.

**Gate:** T47.0's unobserved phase (both strategies, under
`just tasks-drive`), T47.0's panic-path step, and the
`control_flow_for` unit test.

**Injection:**

- **A** — comment out the waker install at `lib.rs:292`. The waker
  phase goes red with `slow=0` against 1.
- **B** — have `about_to_wait` ignore `next_deadline_ms` and delete
  `new_events`. The tick phase goes red with about 1 against 13. At
  `1717808` this injection was indistinguishable from stock; after this
  task it is not, which is the point of landing the gate here.
- Remove the `last_frame` reset on retry: the panic-path step goes red
  with about 16 ticks against 2.
- Go back to Duration::from_secs_f32: the unit test panics by name.

**Accepts good input:** the fixed build reads 25–26 of 26; a 120 ms
period gives WaitUntil at now + 120 ms; a window with no timer still
waits without a deadline.

**Why it may land before phase 3:** it touches no invalidation path, so
M41 cannot change what it proves, and it is an S1 on every idle window
with a timer. Its drive evidence is gathered again after M42.

**Docs in the same commit:** `docs/results-tasks.md` T28.12 (the
WaitUntil claim was false until now; the unobserved figures before and
after; wake-gate covers the `ProxyWaker` wire, T28.1, not `run()`);
`docs/plan-tasks.md` (the T28.12 claim marked corrected);
`docs/plan-panic-recovery.md` §2.2 (timers are frozen while the screen is
shown, and a retry starts from `dt` 0); `docs/00-handoff-static-tree.md`,
deferred work; the rustdoc on `about_to_wait`, on
`Host::next_deadline_ms` and on `Timers` in `spawn.rs` (and
`Timers::every`'s accepted range); the comment in
`scripts/tasks_drive.sh`. The vacuous-test record gains the row
"tasks-drive vs `run()`'s wiring — every read requested a frame, so the
window was never idle": it is the R7-F2 row of
`docs/results-deep-review.md` §4's dated addendum (M43/T43.15 creates it
if this lands first), not a row in `AGENT.md`'s table, which M43 §2.8
keeps to one summary row. Rule 1 above says the same.

**Effort:** S. **Depends on:** T47.0.

**What building found (2026-09-25).**

- **The clock is dropped on the frame that shows the panic, not in the R
  handler.** `app.retry` reaches the session through the agent's
  dispatch, which the window never sees, so a reset "on retry" in the
  window's R arm would have covered one of the two retry paths. Clearing
  `last_frame` on every frame a panic is shown covers both with one line,
  and it is the line injection C removes.
- **The drive retries with `app.retry`, not R.** The plan said "presses
  R"; the agent refuses every `input.*` call while the screen is up
  (M36 §2.2), so a drive cannot press it. With the reset where it is,
  the two paths share the code the step tests.
- **A consumed deadline is disarmed.** `new_events` sets the wait back
  to `Wait` when it asks for the frame, and the frame arms the next
  instant. Left armed, a past `WaitUntil` spins the loop wherever the
  platform holds a redraw back (a hidden window).
- **Injection A leaves the tick phase green** (25 against 24): the
  deadline frames apply the jobs' results one period late without a
  waker. That is why the waker has its own phase with a 400 ms job and
  no timer running.
- **"No deadline while the panic screen is up" has no gate here.** What
  it prevents is the screen being redrawn at the timer's rate, and
  nothing the agent reports counts frames while a panic is shown (the
  journal is not fed on those frames). T47.8's idle section is the place
  to count it.

### T47.2 — catch-up by arithmetic, and every firing accounted for

**Closes:** R7-F3, R7-F4.

**Change:** §2.2(c) in `Timers::advance` (`spawn.rs`): integer
microseconds, one-step count, at most the free slots pushed per timer,
the rest counted per timer. `Host::tick` advances straight into the
application's outbox where the borrow allows it, by taking the timers
out for the call; where it cannot, it folds the scratch outbox's refused
count into the host's counter. The counter is reported through the
agent's existing counters.

**Gate:** T47.0's watchdog case must return with `fired == 62_500_000`
and `remaining` in (0, 16], printing the elapsed time beside 200 ms. The
loss case asserts `delivered + counted == fired == 100`.

**Injection:** restore the per-period `while` loop: the watchdog fires
by name. Drop the counter increment (or discard the scratch count
again): 32 ≠ 100.

**Accepts good input:** 80 ms at 25 ms still fires 3; one hour at 16 ms
still fires 225000; 30 firings deliver 30 with 0 counted.

**Docs in the same commit:** `Timers::advance` rustdoc (the capacity
caveat) and the `Outbox` rustdoc ("Overflow refuses and counts");
`Host::tick`'s comment; `docs/results-tasks.md` T28.11 (answer the plan's
`tick(1e9)` question with the measured count and time);
`docs/plan-tasks.md`.

**Effort:** S. **Depends on:** T47.0, T47.1 (so the panic path no longer
feeds it).

**What building found (2026-09-26).**

- **"The agent's existing counters" do not exist.** No host counter —
  `unknown_kind`, `unspawned`, the inbox's refused — reaches the agent
  today, so there was nothing to add this one to. It is a host accessor,
  `Host::timer_coalesced`, beside those. The reply shape belongs to M48,
  which is rewriting `app.diagnostics`/`ui.waitSettled` concurrently;
  reporting the counters there is its call, and adding a field under it
  would have been a merge conflict for no reader.
- **No scratch outbox is left to fold.** Taking the timers out for the
  call works for every application that has both; one with timers and no
  outbox has nothing that could ever spawn a firing, so every firing is
  counted as coalesced.
- **Periods are held in whole microseconds**, at least 1 and at most
  `u64::MAX` (about 580,000 years). `Timers::every` still accepts every
  finite positive `f32`; a period above that ceiling fires at the ceiling.

### T47.3 — a pool instead of a thread per job

**Closes:** R7-F5.

**Change:** §2.3(b) in `crates/lumen-static/src/spawn.rs`: `Threads`
keeps its name and becomes a fixed pool of long-lived workers fed
through a channel, sized from the available parallelism with a small
cap. The pool size is exposed for the gate. The "no thread is created
for a timer" comment and T28.11's row are corrected in the same commit.

**Gate:** T47.0's thread count: 20 firings create at most the pool size
of threads, printed beside the bound. The existing counting-spawner test
has its label corrected from threads to spawns.

**Injection:** spawn per job again: 20 against the pool size.

**Accepts good input:** the pooled build creates exactly the pool size,
once. T47.1's unobserved tick phase stays green **with `#start-slow`'s
400 ms job running at the same time**, which is the check that a busy
worker does not delay a tick.

**Docs in the same commit:** `Host::tick`'s comment; `docs/results-tasks.md`
T28.11 ("zero threads for a timer" becomes what is measured); the
`Threads` rustdoc; `docs/plan-tasks.md` §2.4.

**Effort:** M. **Depends on:** T47.0.

**What building found (2026-09-26).**

- **The in-flight bookkeeping is a host ledger, and its length is the
  in-flight count.** M48/T48.2 needs "is any job pending" and M49/T49.10
  needs "which descriptors were in flight"; the consistency review asked
  for one structure. It is `spawn::Ledger`, owned by the host: a
  descriptor goes in when `Host::spawn` hands its job to a spawner, and
  comes out when the job's body has returned **and** every result it sent
  has been drained (applied or stale). `Host::in_flight` is its length,
  pruned as it answers. The link from a result back to its job is the
  sender: the host hands each job its own (`Inbox::tracked_sender`), whose
  messages carry a shared `inbox::JobTrack` through the channel, and whose
  **drop** ends the job — so it holds under any spawner, `Queued` and a
  custom one included, and nothing about the pool is load-bearing for it.
  A "done" marker through the inbox was rejected: the inbox is bounded,
  and a refused marker is a job listed for ever. T49.10 still has to add
  what it re-issues from (`Ledger::descriptors`) to `Session::save`, and
  T48.2's counter is this length, not a new integer.
- **The thread count is distinct `ThreadId`s, not the task list.** T47.0's
  row said "OS threads counted from the process's task list before and
  after". A thread per job exits before the next firing, so the list
  reads the same before and after on the stock build, and the check could
  not go red. A `ThreadId` is never reused, so the ids the jobs ran on are
  threads created.
- **"Creates exactly the pool size, once" is "at most the cap".** Workers
  start lazily — one when a job arrives and none is free — so a window
  that never spawns creates no thread, and 20 sequential firings started
  3 of 4.
- **The live accepting check could not see a starved tick by its count.**
  Run as written — the unobserved 3 s tick phase with the 400 ms job in
  flight — a pool of one worker still read 26 of 25, because the ticks
  queued behind the job are applied late, not lost. Phase 7 of
  `scripts/tasks_drive.py` adds one read 330 ms after the job starts,
  which a one-worker pool fails ("no tick landed in 331 ms while a 400 ms
  job ran"); `tests/ledger.rs` has the headless form.
- **The kill rule did not fire:** the pool is 6.0x faster spawn-to-apply
  (`docs/results-tasks.md` T47.3).

### T47.4 — a dropped GPU present asks for another frame

**Closes:** R7-F7.

**Change:** `Gpu::present` reports whether it presented. `GpuBackend`
sets a dropped flag on the `Frame` it returns. `Win::redraw` requests
another frame when the flag is set, with a bound on consecutive retries
so a permanently lost surface cannot spin. The comment at
`crates/lumen-static-wgpu/src/lib.rs:1224-1226` is corrected.

**Gate:** T47.0's `just gpu-parity` step, plus a unit test in the winit
crate of the retry decision (retry on a drop, stop after the bound),
because `just test` builds that crate and not the wgpu one.

**Injection:** remove the redraw request on a dropped frame: one
distinct pixel value on the idle window.

**Accepts good input:** with n=0 the distinct count matches the
review's clean run; with n=2 and the fix, the same.

**Docs in the same commit:** the wgpu comment; `docs/results-gpu.md`,
the present path.

**Effort:** S. **Depends on:** T47.0. M51 touches the same backend;
whichever lands second rebases.

**What building found (2026-09-26).**

- **"More than one distinct pixel value" cannot go red.** A window that
  never presented reads back whatever the X server has under it: with
  every present dropped (n = 1000) the capture held 2,578 distinct
  values. The step compares with the presented GPU frame instead (1% of
  pixels over a delta of 8), with n = 0 as the accept leg.
- **The bound's live check counts attempts, not pixels.** A retry with no
  bound burns through the armed drops and then presents, so the picture
  is right either way; the switch logs one line per drop, and 20 armed
  drops must see at most 8 attempts in 3 s (measured: 4).
- `tasks-demo` has no GPU build; the step uses `examples/minimal`, which
  `just gpu-parity` already builds with `gpu` and is as idle.

### T47.5 — a NaN `dt` advances nothing

**Closes:** R7-F8.

**Change:** guard at the two call sites: a NaN `dt` is treated as 0
before the clamp, in `Animated::tick` and `LongPress::tick`, mirroring
`Timers::advance`'s non-finite reject. `num::clamp` is not changed: it
is used widely, and a global change in its NaN behaviour needs its own
audit (§4).

**Gate:** T47.0's unit tests.

**Injection:** remove the guard: "unsettled after 10000 frames".

**Accepts good input:** the `-inf` arm settles in 13 frames and the
`1e9` arm in 1.

**Docs in the same commit:** the rustdoc on `Animated::tick` and
`settled()` (the "Guarded rather than trusted" paragraph) and on
`LongPress::tick`.

**Effort:** S. **Depends on:** T47.0.

### T47.6 — an unmodelled failure is counted

**Closes:** R7-F9.

**Change:** the inbox `Sender` gains a shared counter, as `refused` is
shared today. `report()` increments it when an application has no
failure handler or the handler returns `None`. `Host::unmodelled_failures`
reads it, and the agent's counters report it.

**Gate:** T47.0's test in `failure.rs`.

**Injection:** delete the increment in `report()`: 0 ≠ 1.

**Accepts good input:** an application that models the failure gets one
message, and the counter stays 0.

**Docs in the same commit:** the `FailFn` and `report()` rustdoc;
`docs/results-tasks.md` (the failure row); the agent method docs for the
counters answer.

**Effort:** S. **Depends on:** T47.0.

**What building found (2026-09-26).** As in T47.2, the agent reports no
host counters, so "the agent's counters report it" has nothing to extend:
`Host::unmodelled_failures` is the reader, and M48 owns the reply shape.
A spawn refused for want of a spawner stays `Host::unspawned`'s alone, so
no failure is counted twice.

### T47.7 — images survive suspend and resume

**Closes:** R15-F6.

**Change:** §2.4(a). `Backend` gains the required `take_images`, which
the software and GPU backends implement. `Win` gains `suspended`, which
takes the images back into `Config` and drops the backend and the
window.

**Gate:** T47.0's fake-backend test, now printing `backend #2 images:
Some`. A live step: a debug-only resume hook in `examples/glasswall`
(an env switch, LUMEN_TEST_RESUME) calls the suspend and resume paths
once; a drive compares `ui.screenshot` before and after and prints
`differing pixels: n / total (bound 0.1%)` and the distinct colours on
each side.

**Injection:** make `suspended` not take the images back: the unit test
prints `None`, and the live step differs on almost every pixel. Make the
GPU backend's `take_images` answer nothing: the GPU arm fails the same
way.

**Accepts good input:** with no resume, the live step reports 0
differing pixels, so the comparison is sensitive only to the resume.

**Docs in the same commit:** `Config::images` rustdoc ("Taken once"
becomes "held across suspend and resume"); `Program::images` rustdoc;
the `Backend` trait's rustdoc; `docs/mobile-decision.md`, lifecycle;
`docs/plan-program.md` where it describes images.

**Effort:** S. **Depends on:** T47.0.

**What building found (2026-09-26).**

- **The hook lives in the window crate, not in `examples/glasswall`.**
  `suspended` and `resumed` are `Win`'s, which an example cannot call;
  the debug-only `LUMEN_TEST_RESUME=<ms>` is read by `Win::new` and runs
  both from `about_to_wait` once, `<ms>` in. Glasswall gained an `agent`
  feature so the drive can reach it (`just resume-drive`).
- **`ui.screenshot` cannot see the GPU backend**: it answers "this host
  does not render", because `GpuBackend` has no `frame_rgba`. The GPU
  leg reads the window back with `xwd`, as `just gpu-parity` does.
- **The unit test cannot see `suspended` itself.** It needs an
  `ActiveEventLoop`, so the test drives the extracted `suspend_backend`.
  The plan's injection — `suspended` not taking the images back — leaves
  the unit test green and is caught live only (98.96% of pixels differ,
  1,771 colours against 85,279); the unit test's own injection is the
  same removal inside `suspend_backend`.
- **The drive checks its setup**: the app's log must show one round trip
  and two backends made on a resume leg, and none on the still leg.

### T47.8 — the idle numbers the comments cite

**Closes:** R7-F11.

**Change:** `docs/results-frame-cost.md` gains an **Idle** section:
with no timer, frames and wakes over 60 s; with a 120 ms timer after
T47.1, wakes and frames against periods elapsed, and context switches
from `/proc`. The four citing comments keep pointing there rather than
being repointed, because M53/T53.4 needs the same section for its
per-motion figure.

**Gate:** T47.0's `just static` rule goes green.

**Injection:** delete the Idle heading: `just static` fails naming the
four comments.

**Accepts good input:** with the section present, it passes; a comment
citing the file without the word "idle" is not affected.

**Docs in the same commit:** the section itself, and the four comments
if their wording changes.

**Effort:** S. **Depends on:** T47.1 (the timer row), T47.3 (the thread
count beside it). M53/T53.4 adds the motion row later.

## 4. Boundaries of this plan

- **Not the motion-present cost.** R7-F6, a full pump and a whole
  present per pointer motion over content it does not change, belongs
  to M53 with R6-F1, because they share `pump_with`'s idle fast path.
  T47.8's section is where M53 records its figure.
- **Not `num::clamp`'s NaN semantics.** T47.5 guards the two call sites.
  Changing `clamp` everywhere is a separate audit and is not needed to
  close R7-F8.
- **Not a mobile lifecycle.** T47.7 makes a second `resumed` correct on
  desktop and keeps the images. Surface recreation on Android, and the
  rest of `docs/mobile-decision.md`, stay deferred.
- **No won't-fix items.**
- **A finding that touches another plan:** R7-F1's panic-path half
  changes a behaviour `docs/plan-panic-recovery.md` §2.2 describes. The
  change is recorded there in T47.1, and M36 is not reopened.

## 5. Kill rules and risks

T47.3 is the one change here with a cost. **Kill rule, written before
the build:** T47.3 first adds an arm that times a no-op job from spawn
to `apply`, and records it on the per-job spawner, eight runs a side.
If the pool's median is slower than the per-job spawner's by more than
the per-job arm's own spread, T47.3 ships only the correction of the
claim, (a) of §2.3, and the pool is recorded in `docs/results-tasks.md`
as measured and not built. The thread count is the gate either way; the
clock only decides whether the pool ships.

Risks, and what would reveal each:

- **A pool starves a timer behind a slow job.** Revealed by T47.3's
  accepting check: the unobserved tick phase run with the 400 ms job in
  flight.
- **The absolute deadline drifts under a stream of events that produce
  no frame.** Revealed by a drive step that moves the pointer outside
  the window at 50 Hz during the unobserved sleep; the tick count must
  still meet its bound.
- **The drive's first read still races the frame it requests.** The read
  is answered before its own frame, so it can only report what a wake or
  a deadline produced. If a future agent change reorders that, the
  unobserved phase stops being unobserved. Revealed by injection B
  going green again, which is why B is re-run in every commit that
  touches `answer_agent`.
- **The GPU retry spins on a surface that never comes back.** Revealed
  by T47.4's unit test of the bound, and by LUMEN_GPU_DROP_FIRST set
  larger than the bound: the window must stop asking.

## Appendix: finding → task

| ID | sev now | status | task |
|---|---|---|---|
| R7-F1 | S1 | OPEN | T47.1 |
| R7-F2 | S1 | OPEN | T47.1 |
| R7-F3 | S2 | OPEN | T47.2 |
| R7-F4 | S2 | OPEN | T47.2 |
| R7-F5 | S2 | OPEN | T47.3 |
| R7-F7 | S2 | OPEN | T47.4 |
| R7-F8 | S3 | OPEN | T47.5 |
| R7-F9 | S3 | OPEN | T47.6 |
| R7-F10 | S4 | OPEN | T47.1 |
| R7-F11 | S4 | OPEN | T47.8 |
| R15-F6 | S3 | OPEN | T47.7 |
