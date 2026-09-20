# ReviewGlass

A desktop companion for AI-assisted coding sessions. Windows 11, Tauri v2, Rust core.

**Status:** pre-alpha, in daily use by the author. P1 (magnifier), P2 (session panel),
P3 (threshold alerts) and P4 (live diff) are built; P4b (a read-only whole-file view
around a hunk) and the spec's third revision are next. The repository has been public
since 17 September 2026. The name has not been checked for conflicting use
(ADR-0008); that gates the public release (P8, installer and bundle identifier), not
the source. See `docs/BUILD_INFO.json` for where the build stands and
`docs/CHANGELOG.md` for what shipped and why.

## What it does

Three things the Claude Code Desktop app does not:

1. **Glass** — an always-on-top magnifier window (150–400 %, pixels only, no OCR) with
   three modes: **Follow** (the glass stays put and shows what is around the cursor,
   with a halo marking the cursor on screen), **Lens** (the glass rides on the cursor),
   **Still** (the picture stops, so a captured instruction survives working elsewhere).
   Title bar, right-click menu, hotkeys; geometry persists per mode.
2. **Dock and panel** — a small always-on-top strip (the dock) that carries the quota
   gauge, the glass's switches and a global shortcut; its drawer is the panel: every
   running Claude Code session in one table (Desktop tabs and terminal sessions alike),
   the account-wide 5-hour and 7-day quota with burn rate and a Windows toast before a
   threshold, and the agent's edits as a live `git diff` per changed file. Cache and PR
   state come later. A tray icon keeps the app findable when the windows are hidden.
3. **Explain** (off by default) — a plain-language explanation of one selected diff hunk,
   through a backend you choose: a local model on `127.0.0.1` or a remote API.

## Privacy posture

- Screen pixels are read, scaled and displayed. They are never written to disk or transmitted.
- Session data is read from files Claude Code already writes on this machine: the spool
  two small native collectors fill — a `statusLine` collector (terminal sessions) and a
  `PostToolUse` hook that records which file an edit touched, never its content — and
  the JSONL transcripts, opened read-only (Desktop sessions). Only session state is
  read from a transcript — never the conversation.
- The account quota reaches ReviewGlass only through a terminal `claude` session,
  because that is the only surface Claude Code runs a status line on. A Desktop-only
  user sees sessions but no gauge, and the panel says so.
- Inter-process communication is plain files under your user profile. **No network listener,
  no localhost port, no IPC socket.**
- No analytics, no accounts.
- The explain feature is off by default with no backend selected. A fresh installation
  performs no inference. When enabled, the *local* backend keeps everything on the machine
  over loopback; the *remote* backend is the only path on which anything leaves the machine,
  is user-triggered per hunk, and discloses its exact payload before the first send.
- Only the selected hunk and its immediate context are ever sent. Never the file, the
  repository or the conversation.

## Building

Prerequisites: Rust (stable, MSVC), Node 22+, pnpm, Visual Studio Build Tools with the
Windows 10/11 SDK, WebView2 runtime (present on Windows 11).

```bash
pnpm install
pnpm tauri dev
```

Installers: `pnpm tauri build` produces an NSIS `-setup.exe` (primary) and an MSI (needs the
VBSCRIPT optional Windows feature). Builds are unsigned; expect a SmartScreen prompt on first
run.

## Layout

| Path | What |
|---|---|
| `src-tauri/` | Rust core: capture, session sources, usage model, notifier, config |
| `src-tauri/src/bin/statusline.rs` | the native collector Claude Code runs as its status line |
| `src-tauri/src/bin/hook.rs` | the native `PostToolUse` hook that records a changed path |
| `src/routes/glass`, `dock`, `halo`, `finder` | the four windows; `src/lib/panel/` holds the dock drawer's tabs |
| `docs/REVIEWGLASS-SPEC.md` | product and architecture specification |
| `docs/adr/` | architecture decisions, rendered from the Atlas model |
| `docs/CHANGELOG.md`, `docs/BUILD_INFO.json`, `docs/LESSONS.md` | what shipped, where it stands, what was learned |

## License

Apache-2.0. See `LICENSE`.
