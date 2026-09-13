# ReviewGlass CHANGELOG

Newest first. One entry per session; a session that ships several distinct things gets
sub-entries. What changed and *why*, with what was measured, so a later reader can tell
a decision from a habit.

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
