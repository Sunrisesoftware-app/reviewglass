# ReviewGlass

A Windows 11 desktop companion for Claude Code sessions: a magnifier that reads a Code
pane at a size you can follow, a dock that shows every running session and the account's
quota, and a live diff of what the agent just changed. Tauri v2, Rust core, SvelteKit.

> ReviewGlass is an independent project. It is not made, endorsed or supported by
> Anthropic. "Claude" and "Claude Code" are Anthropic's names for their products; this
> tool only reads files Claude Code already writes on your own machine.

**Status: pre-alpha.** It is in daily use by its author, and there is no installer
or release yet. The phases built so far are
the magnifier (P1), the session panel (P2), threshold alerts (P3), the live diff with a
whole-file view (P4, P4b) and a provider-agnostic explain feature (P6, off by default).
The public release (P8) is gated on the name: it has not yet been checked for conflicting
use (ADR-0008). The source has been public since 17 September 2026. Where the build
stands: [`docs/BUILD_INFO.json`](docs/BUILD_INFO.json); what shipped and why:
[`docs/CHANGELOG.md`](docs/CHANGELOG.md).

## What it does

Reading an agent's output in a narrow pane, several sessions side by side, is small text
that keeps moving. ReviewGlass adds four things the Claude desktop app does not have:

1. **The glass**: an always-on-top magnifier (150–400 %, pixels only, no OCR).
   - **Follow**: the glass stays put and shows what is around the cursor. A halo marks
     the cursor on the screen.
   - **Click-lock to a pane**: a click on a Code pane locks the glass to that pane, as
     the app's own UI Automation tree reports it. The glass then reads only that pane,
     at the pane's width times the zoom. The arrow keys step the lock to the
     neighbouring pane, and a window of another application over the pane pauses the
     picture.
   - **Lens**: the glass rides on the cursor.
   - **Still**: the picture stops, so an instruction you captured survives while you
     work elsewhere.
2. **The dock and its drawer**: a small always-on-top strip with the quota gauge and the
   glass's switches. Its drawer is the panel.
   - **Sessions**: every running session in one table, whether it runs in a Desktop tab
     or a terminal.
   - **Quota**: the account-wide 5-hour and 7-day quota, with the burn rate and a
     Windows notification before a threshold you set.
   - **Diff**: the agent's edits as a live `git diff` per changed file, grouped by
     session, with the whole file around a hunk and the latest edit's lines highlighted.
3. **The diff window**: the locked pane's session's diff in its own frameless window,
   beside the pane. Open it from the glass's bar or with `Ctrl+Alt+D`.
4. **Explain** (off by default): a plain-language explanation of one selected diff
   hunk. The backend is yours to choose: a local model on `127.0.0.1` or a remote API
   with your own key.

A tray icon keeps the app findable when every window is hidden, and a second launch
brings the running copy's dock forward instead of starting another.

## Requirements

- Windows 11 (Windows.Graphics.Capture, WebView2 and UI Automation are used directly).
- Claude Code: the Claude desktop app's Code tab, the terminal `claude` CLI, or both.
- To build: Rust (stable, MSVC), Node 22+, pnpm, Visual Studio Build Tools with the
  Windows 10/11 SDK. The WebView2 runtime comes with Windows 11.

## Building and setting it up

There is no installer yet (that is P7), so setting it up is manual.

```bash
pnpm install
pnpm tauri build
```

This builds the app, `src-tauri/target/release/reviewglass.exe`, and two small
collectors beside it: `reviewglass-statusline.exe` and `reviewglass-hook.exe`. Copy the
two collectors to `~/.reviewglass/bin/`, then add them to `~/.claude/settings.json`.
Merge the following with your existing settings; use forward slashes in the paths:

```json
{
  "statusLine": {
    "type": "command",
    "command": "C:/Users/<you>/.reviewglass/bin/reviewglass-statusline.exe"
  },
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Edit|Write|MultiEdit|NotebookEdit",
        "hooks": [
          {
            "type": "command",
            "command": "C:/Users/<you>/.reviewglass/bin/reviewglass-hook.exe",
            "timeout": 5
          }
        ]
      }
    ]
  }
}
```

Keep these in mind when you set it up:

- **The status line becomes ReviewGlass's.** `statusLine` holds a single command, so the
  collector replaces any status line you had. It prints a short line of its own.
- **A session uses the status line that existed when it started.** Terminal sessions
  that were already running must be restarted before they report. The hook does not
  need a restart.
- **The quota gauge needs a terminal session.** Claude Code runs a status line only in
  the terminal CLI, not in the desktop app's Code tab. With the desktop app alone you
  still see every session and the live diff, and the panel says why the gauge is empty.
- The collectors always exit 0 and never write into a session. If ReviewGlass is not
  running, they cost a few milliseconds and leave a few small files; edit events are
  pruned after an hour.

For a development build with hot reload, run `pnpm tauri dev`. `pnpm tauri build` also
produces an NSIS `-setup.exe` and an MSI (the MSI needs the VBSCRIPT optional Windows
feature). These packages are unsigned and do not yet set up the collectors.

## Keys

| Key | What it does |
|---|---|
| `Ctrl+Alt+G` | Show or hide the glass (changeable in Settings) |
| `Ctrl+Alt+F` | Still: freeze the picture, or let it go |
| `Ctrl+Alt+L` | Lens on or off |
| `Ctrl+Alt+←` / `→` | Move the lock to the neighbouring pane (plain `←` / `→` while the glass has the focus) |
| `Ctrl+Alt+D` | Open or close the diff window |
| `Esc` (in the glass) | Hide the glass |

Every key has a mouse equivalent as well: a button on the glass's bar or the dock, an item
in the right-click menu, or, for the arrows, a click on another pane.

## Privacy posture

- **Screen pixels are read, scaled and shown, never kept.** The glass and its halo
  exclude themselves from screen capture. The glass's pixels are never written to disk
  or transmitted. The one picture ReviewGlass saves is one you ask for: the drawer's
  camera button copies an image of the drawer's own page (not the screen) to the
  clipboard and to `Pictures\ReviewGlass`.
- **Session data comes from files Claude Code already writes on this machine.** These
  are the spool the two collectors fill and the JSONL transcripts, which are opened
  read-only. The hook records which file an edit touched, never the edit's content.
  Only session state is read from a transcript, never the conversation.
- **ReviewGlass never writes into a session, a repository or a transcript.**
- **Inter-process communication is plain files under your user profile**
  (`~/.reviewglass`). There is no network listener, no localhost port and no IPC socket.
- **There are no analytics and no accounts.** The diagnostic logs (`panic.log`,
  `stall.log`) stay under `~/.reviewglass` and hold timings, sizes and places in the
  code, never pixels or text.
- **Explain is off by default, with no backend selected**, so a fresh installation
  performs no inference.
  - The *local* backend keeps everything on the machine, over loopback.
  - The *remote* backend is the only path on which anything leaves the machine. Each
    send is triggered by you, per hunk, and the exact payload is shown before the
    first one. Only the selected hunk and its immediate context are sent, never the
    file, the repository or the conversation. The key lives in Windows Credential
    Manager.

## Documentation

| | |
|---|---|
| [`docs/REVIEWGLASS-SPEC.md`](docs/REVIEWGLASS-SPEC.md) | The product and architecture specification: the measured facts about Claude Code's surfaces, the module contracts, the roadmap |
| [`docs/adr/`](docs/adr/) | The architecture decisions, one file each |
| [`docs/CHANGELOG.md`](docs/CHANGELOG.md) | What shipped, why, and what was measured, newest first |
| [`docs/LESSONS.md`](docs/LESSONS.md) | Pitfalls met on the way, with the story behind each rule |
| [`docs/BUILD_INFO.json`](docs/BUILD_INFO.json) | Where the build stands and what is next |

## Layout

| Path | What |
|---|---|
| `src-tauri/src/capture/` | Windows.Graphics.Capture, the crop, the pane detector |
| `src-tauri/src/glass.rs`, `frame.rs` | The glass's modes, hotkeys and menus; the click-lock to a pane |
| `src-tauri/src/session/`, `usage.rs` | Session sources (spool and transcripts) and the quota model |
| `src-tauri/src/diff/`, `diffwin.rs` | The live diff and the diff window |
| `src-tauri/src/explain/` | The explain service; `backend/` is the only place a provider is named |
| `src-tauri/src/bin/statusline.rs`, `bin/hook.rs` | The two collectors Claude Code runs |
| `src/routes/glass`, `dock`, `halo`, `finder` | The windows' pages; `src/lib/panel/` holds the drawer's tabs |

## Contributing

This is a single-author project for now. Issues describing what you saw are welcome.
Pull requests are best discussed in an issue first, because the design decisions are
kept in the ADRs and a change that crosses one needs a new decision.

## License

Apache-2.0. See [`LICENSE`](LICENSE).
