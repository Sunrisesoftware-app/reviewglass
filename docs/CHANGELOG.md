# ReviewGlass CHANGELOG

Newest first. One entry per session; a session that ships several distinct things gets
sub-entries. What changed and *why*, with what was measured, so a later reader can tell
a decision from a habit.

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
