# ReviewGlass CHANGELOG

Newest first. One entry per session; a session that ships several distinct things gets
sub-entries. What changed and *why*, with what was measured, so a later reader can tell
a decision from a habit.

## Session 3 (continued): the second recording, and the dock holds the glass — 17.9.2026 (0.1.0)

The owner recorded again with build `2ed3568`: 50 s in Follow
(`docs/measurements/follow-2026-09-17-owner-2-follow.log`) and 36 s in Lens
(`…-owner-3-lens.log`; the lens does not scan, so there is nothing to hold and nothing
to measure beyond the ride). What the Follow recording says:

- **Reading is calm now.** From 9.6 s to 45 s — the reading itself — the window did not
  resize once. The lock held the right edge of the column at 1463 at 2010 on all 43
  scans while the band read 1975 on twelve of them (a 35 px range); the column at 2011
  at 2557 or 2560 against the band's 37 px range; `none` reached the lock once in 146
  scans (three were read). Seventeen changes reached the glass in 50 s against fifty in
  84 s before, and every one between the settling of the first six seconds and the end
  was a "none" decision for Fit.
- **What remained was the trip to the dock.** At 45–48 s the cursor went to the dock's
  corner to stop the recording; over the dock the band read 0..1456 (the wide-column
  wait held that) and then 0..912 — 36 % of the screen, so it widened at once, to
  1374 px. Eleven of the 146 scans had the cursor over the dock; seven of 218 in the
  first recording, and the rest of that recording's top-strip readings came from the
  strip right beside it.
- **The dock holds the glass.** While the cursor is over the dock, Follow holds its
  source rectangle and scans nothing, as it does while the pointer is over the glass
  itself: reaching for the dock is not reading. The rider thread, which watches the
  cursor anyway, reports the dock's rect to the engine. Replayed against both
  recordings with the dock's scans left out: the second recording's widening to
  1374 px is gone (880 → 825 → 1326, the last with the cursor in the sidebar on the way
  out), the first recording's numbers hold (48 changes to 22, no `none` to the lock).
- **A pause keeps the column's memory.** The tracker now forgets its column only when
  the lock is switched off; a hover, a menu or the dock keeps the furthest line in
  memory, which ages out by itself, so a column comes back after a trip to the dock
  exactly as it was.
- **And the dock's Still has a meaning.** With the source held over the dock, Still
  from the dock freezes what the glass was showing before the cursor left for the dock
  — the answer to the question left open this morning, to be confirmed by the owner
  and, if kept, recorded as a decision.

Both replays are pinned in Rust tests (`replay_of_the_first_recording`,
`replay_of_the_second_recording`), scans over the dock skipped as the engine now skips
them. Verified on the release build over CDP: in Follow, the picture's hash is unchanged
while the cursor sits on the dock. The return to following was not proven by that run
(the check moved the cursor vertically over a white page, which hashes the same twice),
and no further live run was made: the owner was working at the machine, and a
verification that takes the mouse is an interruption (LESSONS).

## Session 3 (continued): the column holds — three tunings from the first recording — 17.9.2026 (0.1.0)

The owner's call on the three proposals: all three as one change, verified against
the recording before recording again.

- **The right edge is the furthest line in memory** (`capture/track.rs`,
  `ColumnTracker`, between the detector and the lock). Per column — the same left edge
  within 8 px — the right edge the lock holds is the widest reading of the last ten
  seconds, not the widest in the current 400 px band. The left edge stays as read; it
  never moved in the recording.
- **A lost column is held for two seconds.** A `none` with a column in hand keeps that
  column; a found column replaces it at once, so a column switch goes column to column
  with no cursor-centred picture in between; after two seconds of `none` the lock lets
  go. The tracker resets whenever scanning stops (lock off, hover, a menu), so a column
  is never remembered across a gap.
- **A wide column waits.** In Fit, a widening to a column wider than half the screen
  waits three seconds — longer than the lock's hold on a lost column, so a reading that
  only persisted through a hold never widens the glass. Everything narrower widens at
  once, as before. (Proposed as "a widening beyond +25 % waits"; against the recording
  that would have delayed the owner's own switch from a code column to the chat column,
  +68 %, by three seconds, while the case that hurt was the 1497 px reading at the top
  of the screen. The screen share is the better test.) Where the 1497 px readings came
  from: the cursor's trips to the dock and the tab strip, y below 110, where the band is
  a quarter title bar and the sidebar's gutter is uniform in 75 % of the slices, under
  the 80 % a plain gutter needs — so sidebar and chat merged into one "column".

Verified against the recording, twice. A Rust test replays the 218 scans through the
tracker (`replay_of_the_first_recording`) and pins the numbers; a Python replay of the
same scans through the old and the new Fit rules counts the resizes.

| | raw readings / old Fit | tracked / new Fit |
|---|---|---|
| changes told to the glass | 50 | 23 |
| `none` reaching the lock | 12 | 0 |
| right edges held in the column at 1463 | 1998, 2001, 2010, 2036 | 2010, 2036 |
| window resizes in 84 s | 9 | 6 |
| widths over 1500 px | 2 (2252, twice) | 0 |
| width sequence | 825 → 866 → 826 → 609 → 1378 → 2252 → 828 → 2252 → 825 | 825 → 866 → 830 → 609 → 1378 → 828 |

What remains in the new sequence is the owner's own movement: the 866 is the 2036 px
line, held for ten seconds and then let go (830); the 609 and the 1378 are the left
column and the chat column. The 23 changes are column switches and a right edge growing
to a longer line. The scan line in the log now carries both readings, `pane=` from the
band and `lock=` from the tracker, and the reader shows both side by side.

Next: the owner records again with this build in the same situations, and the same
reader says whether the picture is calm; then the recorder goes.

## Session 3 (continued): a measurement log for Follow — 17.9.2026 (0.1.0)

Observation 1 — Follow still drifts and the text in the glass changes width — is to be
taken as one conversation with measurements (the owner's ask). The measurements need a
source, so this is the instrument: temporary tooling, to be removed once the tuning is
settled.

- **The log** (`src-tauri/src/measure.rs`): one line per event, seconds since switch-on
  first. `scan` is every run of the detector with the cursor and the column it read
  (`x0..x1/width` or `none`) and whether the reading counted as a change beyond the
  8 px jitter; `src` is the source rectangle when it moved, at most ten a second, with
  the mode and the hover and hold flags; `pane-event` is what the glass was told;
  `fit` is Fit's decision from the glass page (the column, the zoom, the width it
  wants against the width it has, and `widen`, `shrink`, `shrink-wait`, `none` or
  `skip` with the reason); `view`, `resized`, `hover`, `hold`, `release`, `mode`,
  `lock` and `fit-toggle` are the state changes around them. Structure only: never a
  pixel, never a character of text.
- **The affordance**: Settings → *Measurements* → "Record Follow measurements", with
  the path shown (`%TEMP%\reviewglass-follow.log`). Off by default; switching it on
  starts the file over; it stays on across a restart (a new file per run) and stops
  itself after 200 000 lines. Nothing is written while it is off — one atomic load on
  the hot paths.
- **The reader**, `scripts/analyze-follow-log.py`: counts and distributions of the
  detector's readings (width, edges, how long a reading held, the largest edge jumps),
  how often the glass was told, Fit's decisions by kind, resizes and the gaps between
  them, and a timeline of changes. It is what the conversation reads from.
- **Checked on the release build** over CDP: the switch creates the file with its
  header, Follow over a column with the lock and Fit on writes every kind of line
  (13 scans, 5 pane events, 5 Fit decisions, 2 resizes in 14 s of cursor movement),
  and nothing is written after the switch is off. A first hint in that small sample,
  to be confirmed on the owner's reading: the detector alternated between the column
  and `none` as the cursor moved down the same column, and with `none` the lock has
  nothing to hold, so the source rectangle fell back to the cursor's x — a sideways
  jump that Fit never sees (it keeps the width on `none`).

- **Record and Stop, the same evening.** The owner's first recording came in through a
  checkbox and a quit; a recorder that looks like one is easier: a red **Record**
  button that turns into **Stop**, the elapsed time and the line count ticking beside
  it, and every recording its own file named by its local start time
  (`reviewglass-follow-YYYYMMDD-HHMMSS.log`), so a new one never overwrites the last
  and a restart never truncates it (the first version did both, being a persisted
  setting that started the file over). A recording ends with Stop or with the app.
- **The first recording** (84 s, 218 scans, 50 pane events, 9 resizes; kept as
  `docs/measurements/follow-2026-09-17-owner-1.log`) says three things, each a
  separate cause of restlessness:
  1. *The right edge flickers, the left edge never does.* In the column at x 1463 the
     right edge read 2010 (58 times), 2001 (22), 1998 (3) and 2036 (3): a 38 px range;
     in the column at 2011, 2523 to 2560, 37 px; the left edges were exact every time.
     Each flicker crosses the 8 px jitter, reaches Fit, and Fit's 16 px slack on the
     window is 11 px of column: sixteen shrink-waits were scheduled and cancelled, and
     one outlier reading (2036) widened the glass at once from 825 to 866 px and let it
     narrow back two seconds later. The "level of the line that reaches furthest" is
     read per band, so it changes with every scroll.
  2. *A `none` inside a column throws the picture sideways.* 12 of 218 scans read
     `none` with the cursor inside a column it had just read; Fit keeps the width on
     `none` as designed, but the lock has nothing to hold and the source rectangle
     falls back to the cursor's x — 548 px at t 35.4, 180–190 px and back at 37.6 and
     40.5–41.0. That is the drift.
  3. *The top strip reads as a 1497 px column* (0..1497 at y below 110, 58 % of the
     screen, under the 70 % rule) and the glass widened at once to 2252 px, twice.
  Proposals, to talk through: a per-column memory of the widest right edge over the
  last ten seconds (the furthest line over time, not per band); the lock holds its
  column on `none` while the cursor stays inside it and lets go when the cursor leaves
  or after two seconds; and a widening beyond a quarter of the current width waits
  like a narrowing does.

## Session 3: a still that holds, a menu that stays put — 17.9.2026 (0.1.0)

Three of the four observations from session 2's close (2, 3 and 4), taken as one change
in the engine and the rider, and a fourth cause found on the way that sat under two of
them.

- **Still from the dock was black** (observation 3). `dock_activate("still")` on a
  glass that was off froze an engine that had published no frame: the still flag
  stopped every frame before one had arrived. A freeze now asks whether a frame is *in
  hand* — the capture is running and has published since it attached — and with none
  it lets one frame through and starts the hold after it, the path a still restored
  from disk already took. Leaving a still cancels a freeze still waiting for its frame,
  or the next frame would have frozen a glass that was following again. Semantics as
  they fall out: Still from the dock is a still of where the glass last looked, since
  at the click the cursor is on the dock; at a first start with nothing looked at yet,
  the top-left corner of the screen. What the dock's Still *should* mean is open.
- **The picture and the lens hold while a menu is open** (observations 2 and 4). Every
  ReviewGlass menu — the glass's right-click menu, the `Aa` sizes, the dock's — holds
  the engine for as long as it is open: no frame is published, the source rectangle
  stays where it was at the click, the pane scan pauses, and the rider does not move
  the lens, so the menu no longer has to be chased. `popup` blocks until the menu
  closes, so the hold is a bracket around it; a pick reaches `on_menu` afterwards
  through the event loop, inside a 120 ms grace in which nothing is published either,
  so a frame composed while the menu was closing cannot slip into a still. After the
  grace the next frame is forced through. "Freeze this picture" therefore keeps the
  frame that was right-clicked on — never the menu.
- **The canvas was wiped on every state change** (found on the way). `reportView`
  assigned the canvas size on every relayout, and assigning a size clears a canvas. A
  following glass repaints within a frame; a still gets no frame, so it stayed black —
  and that is why the old still showed the menu: the last frame published before the
  freeze, menu and all, was the one that refilled the canvas after the wipe. The canvas
  is now sized only when the picture area really changed, and the last frame is painted
  back at the new size, so a resized still scales instead of going blank.

Measured on the release build (warm launch, the second start after the build), from
outside over CDP and window rects, with the owner's mouse in use at the same time — so
every check compares two reads taken in the same state, never a read from before a menu
with one from after it:

- Still from the dock with the glass off since start: still mode, a picture within
  0.6 s, the same canvas hash a second later. Off, then Still again: a picture within
  0.3 s.
- Lens with the menu open: the window rect unchanged after the cursor moved 500 px and
  the canvas hash unchanged; "Freeze this picture" picked from the menu by keyboard:
  the still's hash equals the hash read under the menu, holds a second later, and the
  window is where the lens was.
- Follow with the menu open: the canvas hash unchanged; after Esc the glass is still
  visible and following, and the hash changes with the cursor again.
- The first run of the same checks, before the canvas fix, showed the still as a black
  canvas (hash 0) after the menu pick — the wipe made visible by the hold, which had
  taken away the pending frame that used to refill it.

Four new engine tests (a freeze without a frame waits for one, a freeze with one holds
at once, leaving a still cancels a pending freeze, a hold pauses and a release forces
the next frame); 72 tests in all. Observation 1 (Follow restless, the width changes)
is untouched and is the next conversation, with measurements.

## Session 2, closing: observed with the dock in use, not yet fixed — 13.9.2026

The owner's verdict at the end of the day: "the direction is good now." Four things
seen in use, recorded here as the start of session 3, in the owner's order:

1. **Follow still drifts, and the text in the glass changes width — restless.** The
   two-second shrink delay and the anchored edge were not enough. To be taken as one
   conversation with measurements (the owner's ask): candidates are a longer hold, a
   width that only ever grows within one column visit, Fit off by default with a
   one-click fit, and row snapping so the picture moves by whole lines.
2. **Lens: a right-click should hold the lens still while the menu is open.** The menu
   pops at the cursor, below and to the right, and the lens rides after the cursor as
   the user reaches for it, so the menu has to be chased. The rider should pause from
   the click until the menu closes (the X, or a pick), then ride again.
3. **Still from the dock is black.** `dock_activate("still")` on a hidden glass freezes
   an engine that has published no frame, so nothing ever shows. "Freeze this picture"
   from the menu works because a frame is in hand. The restore path already solves
   this (`freeze_after_publish`: let one frame through, then hold) and Still-from-off
   must take it. Not intended; a bug.
4. **The still keeps the menu in the picture.** Freezing from the right-click menu
   captures the menu itself — a system popup, not excluded from capture — into the
   still ("Freeze this picture, Leave lens…" visible in it). Freeze the frame from
   before the menu opened, or freeze a beat after the menu has closed.

## Session 2 (continued): the dock — 13.9.2026 (0.1.0)

The owner's verdict after a day with the pane build: the glass, alone on the screen,
demanded attention — it appeared at start wanted or not, its controls sat on the thing
being read, and it resized itself. Agreed: a control panel in a corner, from which the
glass is switched on and off; the glass on a rubber band to it (adr.rg.018, Atlas
#305/#306).

- **The dock** (`rg.dock-window`, `src/routes/dock`): a 470×44 strip, always on top,
  excluded from capture, snapped to a screen corner (top-left by default; dragged
  anywhere it snaps to the nearest corner of its monitor's work area and the corner is
  remembered). One button per glass mode — **Follow · Lens · Still** — lit in the mode's
  colour when the glass is on in that mode; a click on the lit button switches the
  glass off. The account gauge in miniature (5-hour percentage, time to reset, dimmed
  when stale) or the reason there is none ("no CLI session", "not reported yet"), the
  panel button, and the build stamp behind the `RG` mark. Right-click: panel, quit.
- **The glass and the panel start hidden.** `glass.visible` is no longer restored;
  every start begins with the dock alone. The glass's bar keeps its controls
  (adr.rg.013) so the glass can still be left from itself; the tray remains the
  fallback. The glass's state is now broadcast to every window (`glass:state`), so the
  dock's lit button is always the glass's mode, whichever way it was changed.
- **One click, one key.** The `RG` mark on the dock is a switch: a click shows the
  glass as it last was (mode and all), a click on the lit mark hides it. The global
  shortcut does the same from any application — `Ctrl+Alt+G` had existed since session
  1 without being said anywhere; it is now in the mark's tooltip and the dock's menu,
  and the Settings tab lets the owner change it (parsed and registered before it is
  stored; a combination another application holds is refused with the reason and the
  old one stays). Verified over CDP: `last` restores Still after an off; `NotAKey` is
  refused; `Ctrl+Alt+R` takes effect at once and reads in the dock.
- **The panel is the dock's drawer.** The owner saw the panel land in the middle of
  the screen at every click on the shortcut: the single-instance hook still showed the
  glass and the panel (session 1's behaviour). It now brings the dock forward and
  nothing else. And the panel opens beside the dock — under a dock at the top, above one
  at the bottom, flush with its outer edge — at a remembered size (500×620 by default,
  stored when the user resizes it); its position is never stored, the dock's is.
- **Verified on the release build** by reading window rects and the dock's DOM over
  CDP: at start the dock is at (8,8) and the glass and the panel hidden; a second
  launch of the exe changes nothing; `panel_show` opens the panel at (8,58); `dock_activate` Follow shows
  the glass and lights Follow, Still relights, off hides everything; a
  `SetWindowPos` to (1500,900) snapped the dock to (2082,1340) — bottom-right of the
  work area, above the taskbar — with `corner: bottom-right` in the config, and back.

## Session 2: the glass reads the pane — 13.9.2026 (0.1.0)

The owner's verdict on session 1 was that the glass did not yet make the work easier:
it followed the hand sideways while the eye read down a column, and its width was a
guess over panes that are twice as wide with two Code-tab columns as with five. This
session gives the glass a sense of the column (adr.rg.017), without OCR and without a
UI Automation dependency.

- **Pane detection in the capture engine.** `capture/pane.rs`: a 400 px band around the
  cursor is classified column by column as uniform or textured; a uniform run wide
  enough to be a gutter (≥ 28 px), or one that carries a border line, is a boundary;
  the pane is the textured span between the nearest boundaries. Structure, never
  content — the band is scanned in the capture callback and dropped. Seven unit tests
  cover a gutter, a border inside padding, an indentation gap, a caret, a cursor in a
  gutter, a strip too narrow to be a pane, and an empty band. Scans at most 4/s while
  the cursor moves, 1/s while it rests; 8 px hysteresis so the picture never nudges.
- **Pane lock.** In Follow the source rectangle takes its row from the cursor and its
  column from the pane; a pane wider than the source shows its left part, a narrower
  one sits centred. Off, or with no pane found, Follow behaves as before.
- **Fit.** The glass's width follows the pane at the current zoom, capped at the
  monitor minus a margin and kept on the monitor; when the pane does not fit even so,
  the zoom in effect comes down (never below 150 %) and the bar shows it as `150%↓`
  with the reason in the tooltip. The user's own zoom and width are never overwritten
  by a value the fit derived: `glass_set_view` takes a `derived` flag and
  `glass_save_size` skips the width while Fit is on; turning either off restores the
  user's own.
- **The viewfinder** (`rg.finder-window`, `src/routes/finder`): a fourth window,
  click-through and excluded from capture, framed on the engine's source rectangle by
  the rider thread in the Follow colour. It shows what the detector found and where the
  glass is looking, so a wrong guess is visible instead of silent.
- **The bar.** Two toggles, `Pane` and `Fit` (Fit disabled without the lock), and a
  value that says the pane's width in screen pixels or **"no column here"** — the
  reason, not a number, when nothing is found (adr.rg.011 applied to layout).

- **Second pass the same afternoon, from the owner's first look.** The lock is labelled
  **Column**, not "Pane" (a vulgar homonym in spoken Finnish; the code keeps `pane`).
  The **Aa** button opens a menu of bar sizes (100–200 %, the current one checked)
  instead of a blind five-step cycle, and the bar **wraps** to a second row when its
  buttons no longer fit, so a larger size never pushes a control out of sight — checked
  in a 700 px glass at 150 % and 200 %: three rows, nothing hidden.
- **CSP fix.** `connect-src ipc: http://ipc.localhost` was missing: Tauri's IPC over the
  custom protocol was blocked, every first `invoke` failed and fell back to postMessage.
  One first call survived that; two concurrent ones (a relayout effect added this pass)
  both rejected and the poll never started — "Waiting for the first frame…" forever on
  a release build that was fine in dev. Found through WebView2 remote debugging (CDP),
  which is now the way to see inside a release webview (`docs/LESSONS.md`).

- **Start-up race fixed.** The windows in `tauri.conf.json` are created before `setup`
  runs, so the glass's first `invoke` can land before the core has registered its
  state and is rejected with no message; the page then sat on "Waiting for the first
  frame…" for good. It had passed every earlier check because a freshly built exe
  loads its webview slowly enough to lose the race the right way. Measured: the build
  without the fix failed 3 of 3 warm launches from the Desktop shortcut; with the
  retry (`stateWhenReady`, 250 ms steps, the reason shown on the picture after a
  second) 4 of 4 launches show the picture within six seconds.

- **Detector, second version, from the owner's test over real columns.** The right
  Code column was found only in the middle fifth of the screen; near the top and
  bottom the pane jumped to the whole width, and on the left the sidebar and chat
  merged. Two causes, both in the "uniform over every row" rule. (1) A tab strip, an
  input box or a hover highlight crosses every gutter on its rows; the band is now
  cut into 25 px slices and a column is *blank* when uniform in the cursor's slice
  and *strong* when uniform in ≥ 60 % of them (a plain gutter without a border line
  needs ≥ 80 %, so a block indented for most of the band still does not pass). (2) The
  end of a short line leaves a pane's text area blank on the cursor's row too, so a
  blank run reached far into the text and its weakest column sank the whole run; a
  boundary is now the run's strong core alone. The band also slides onto the screen
  at the edges instead of being clipped. Checked offline on a screenshot with the
  Python port of the same rules (`scratchpad/detect2.py`; live and port agree on the
  same screen): the right column 1896–2470 px and the middle column 1208–1842 px at
  every height from the tab strip to the input box, where the first version had
  found them at one height in five. Fourteen detector tests.
- **Margins, and an edge that does not breathe.** The pane had been the textured
  span exactly: the picture began on the first glyph and the longest lines lost their
  ends, because a column only the longest line reaches counts as blank in most slices.
  Now the text's edge is the gutter's last *clear* column (as blank as the gutter's
  blankest, give or take one slice), a 5 % margin (12 px at least) is added into the
  gutter, and where the gutter carries a line — a border, a scrollbar — within three
  margins' reach, the edge sits on the line: that is the pane's own edge and it stays
  put as the cursor scrolls past longer and shorter lines, so Fit does not resize the
  glass on every row. On the screenshot: right column 1846–2533, middle 1158–1845, at
  every height, text at 1896–2480 and 1208–1842 inside. Fifteen detector tests.
- **Calmer, from the owner's verdict that the glass "jumps and flickers".** Three
  changes. The pane's edge is the level of the line that reaches furthest (the
  one-slice tolerance cut the ends of the longest lines). Fit widens at once but
  narrows only after a narrower reading has held for two seconds, so the glass no
  longer breathes with every row's line length. And a pane wider than 70 % of the
  screen is not a column: "no column here", the glass keeps its size, no jump to the
  full screen width. The larger question — a glass on a rubber band to the dock,
  switched on and off from there — is the dock's (next).
- **Build stamp on the bar**: `0.1.0 c4c4521`, with `+` when the tree had uncommitted
  changes (`build.rs` asks git), so a test never assumes the wrong build.

Measured on the release build, this machine (2560×1440, one monitor): idle CPU
2.3–3.7 % of one core over 20 s with the screen not static (the Code tab streaming),
lock on and off alike — the scan is inside the noise of what the screen is doing.
Over the empty desktop the detector reports one 2560 px pane and the fit widens the
glass to 2520 px at 150 %; over the Desktop app's sidebar + chat column it reports
0–1365 px and the glass fits to 2395 px at 175 %; the finder lands on the source
rectangle to the pixel (window rects read back with `GetWindowRect`). Over the Code tab's columns: to be
measured by the owner, since the glass and its overlays are excluded from every
capture path and cannot be screenshotted by the build agent.

Atlas: adr.rg.017 and `rg.finder-window` merged in Sunrisesoftware-app/atlas#302,
renumbered from 018 in #303, the guard's retirement record in #304; MCP worker
redeployed. `docs/adr/` re-rendered. The spec (6.3 `capture-engine`, `glass-window`;
6.1 topology) is still at v0.2 and now owes both this and session 1's changes.

## Session 1 (continued): the glass becomes a window — 12.9.2026 (0.1.0)

The first extended hands-on session, and it moved the glass more than the spec did.
Seven changes, all from use, none of which could have been known before it.

- **P3 shipped: threshold alerts.** `rg.notifier` decides what crossed (75 % and 90 %
  by default, per window, once per reset period, re-armed on a new `resets_at` or a
  falling percentage); the usage loop delivers a Windows toast. The usage model moved to
  its own thread so alerts fire from a closed panel (adr.rg.016). Settings tab: the
  thresholds, on/off, and a **test notification** button — measured on this machine,
  the toast appears, attributed to "Windows PowerShell" because an unpackaged exe
  borrows its notification identity (P7 fixes that).
- **Tray icon and single instance** (adr.rg.015). Two instances were measured alive at
  once after the user "closed" an app that had only hidden itself.
- **Lens mode, then a still.** The lens rides on the cursor at its own size. Freeze
  became a **still** rather than a locked live region (adr.rg.012): the first real use
  was to capture an instruction, park it, and go work in Power BI.
- **Two bugs behind the lens "vibrating".** The window was repositioned from the
  webview's 30 Hz poll; it now rides on a Rust thread at ~120 Hz. And the dirty-region
  skip was wrong whenever the source rectangle moved — see `docs/LESSONS.md`.
- **The glass is a window** (adr.rg.013): a permanent title bar with the mode as three
  labelled buttons (Follow, Lens, Still), zoom, size, halo, chrome scale, panel, hide,
  quit; a 3 px border in the mode's colour; a right-click menu with the same actions.
  The bar's own size steps 100–200 % and is remembered.
- **Cursor halo** (adr.rg.014): a third window, click-through and excluded from
  capture, marks the pointer on screen while the glass follows it. Verified centred to
  the pixel.
- **Size drift fixed.** `glass_set_view` had persisted the canvas size as the window
  size, shrinking the window on every restart (900×340 had become 632×352). Sizes are
  now stored only on a user resize, from the window's real inner size, per mode.

Documentation brought to the family standard: `docs/adr/` rendered from the Atlas model
by `scripts/adr-from-model.mjs`, `docs/LESSONS.md`, this file, `docs/BUILD_INFO.json`,
and `CLAUDE.md` restructured with a cold-start section.

## Session 1: from spec to P2, and the spike that inverted the design — 11.9.2026 (0.1.0)

- **Set-up.** Atlas system `reviewglass` drafted from `REVIEWGLASS-SPEC` v1, validated
  in studio (seven lenses, provisional green light), promoted and merged (#292). Repo
  created private under Sunrisesoftware-app; Tauri v2 + Rust + SvelteKit scaffold, two
  windows, Apache-2.0, CI on windows-latest. Rust toolchain installed.
- **P0 spike, first reading wrong, then right.** `statusLine` runs in the terminal CLI
  and **not** in the Desktop Code tab (2.1.268). The first reading said the opposite,
  because the logged sessions were assumed to be Desktop ones; the transcript's
  `entrypoint` field settled it. Consequences: `rg.session-source` ships two
  implementations (adr.rg.003), the account gauge is borrowed from any live CLI session
  (adr.rg.009), and `surface` is measured from the transcript rather than guessed. Model
  corrected in #293; recorded in the decision log as `disagreed`.
- **P1 shipped: the magnifier.** Windows.Graphics.Capture via `windows-capture`,
  `WDA_EXCLUDEFROMCAPTURE` on the glass, monitor re-attach on crossing, zoom 150–400 %.
  Idle CPU measured on a release build: 14 % of one core, then **1.35 %** after
  skipping the crop when no dirty region intersects the source (0.06 % of a 24-thread
  machine); 30.9 % with the screen churning. Both installers build (NSIS 1.4 MB, MSI
  2.1 MB). A frame-decode bug (`Uint8Array` vs `ArrayBuffer`) found by the owner on
  first run and fixed; visible controls added the same day.
- **P2 built: the session panel.** `reviewglass-statusline` as a native binary
  (adr.rg.010), installed in place of the spike script. Payload parser made lenient per
  field after `context_window.current_usage` turned out to be an object (adr.rg.011).
  Both channels verified on this machine — a Desktop session via transcript, a CLI
  session via the spool. One shared gauge, N relative shares, never blended; the panel
  names why a figure is missing. Five concurrent sessions not yet observed (P2's exit
  criterion stays open).
- **Spec v0.2** published as Atlas artifact `REVIEWGLASS-SPEC` v2, repo copy in step.
