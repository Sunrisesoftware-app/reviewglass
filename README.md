# ReviewGlass

A desktop companion for AI-assisted coding sessions. Windows 11, Tauri v2, Rust core.

**Status:** pre-alpha, phase 1 (magnifier) in progress. Not yet usable. Private repository
until the name is cleared and phase 8 is reached.

## What it does

Three things the Claude Code Desktop app does not:

1. **Glass** — a frameless, always-on-top magnifier for any rectangular screen region
   (150–400 %). Pixels only, no OCR. Freeze mode, global hotkey, persisted geometry.
2. **Panel** — every running Claude Code session in one table, the account-wide 5-hour and
   7-day quota with burn rate and threshold toasts, the agent's edits as a live `git diff`,
   cache and PR state.
3. **Explain** (off by default) — a plain-language explanation of one selected diff hunk,
   through a backend you choose: a local model on `127.0.0.1` or a remote API.

## Privacy posture

- Screen pixels are read, scaled and displayed. They are never written to disk or transmitted.
- Session data is read from files Claude Code already writes on this machine
  (a `statusLine` passthrough and a `PostToolUse` hook, both installed by the installer).
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
| `src-tauri/` | Rust core: capture, spool watcher, git, config |
| `src/routes/glass` | the magnifier window |
| `src/routes/panel` | the sessions / diff / cache / PR / settings window |
| `scripts/statusline`, `scripts/hook` | the collector scripts Claude Code runs (P2, P4; not yet present) |
| `docs/REVIEWGLASS-SPEC.md` | product and architecture specification |

## License

Apache-2.0. See `LICENSE`.
