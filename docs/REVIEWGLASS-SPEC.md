# ReviewGlass — Product & Architecture Specification

**Version:** 0.3
**Date:** 2026-09-23
**Owner:** Petri Korhonen / Sunrise Software Oy
**Status:** In daily use by the author. P0-P2 and P4 (with P4b) done, P3 built. Next, in
the order the owner set on 2026-09-23: P6, then P5, P7, P8.
**Model:** Atlas system `reviewglass` (modules `rg.*`, decisions `adr.rg.001`-`022`).
**Repository:** github.com/Sunrisesoftware-app/reviewglass, public since 2026-09-17 by
the owner's decision; the name check still gates the public release, P8 (`adr.rg.008`).
The repo carries a copy of this document at `docs/REVIEWGLASS-SPEC.md`; this artifact is
the source of truth.

### What changed from v0.2

Twelve days of daily use moved the design more than the three measurements behind v0.2
did. Eleven decisions (`adr.rg.012`-`022`) and the phases they served:

1. **The glass became a window** with a permanent title bar and three named modes -
   Follow, Lens, Still (`adr.rg.013`) - and a freeze became a still, not a locked live
   region (`adr.rg.012`). In Follow a halo marks the pointer on screen (`adr.rg.014`).
2. **The glass reads the pane** (`adr.rg.017`): column boundaries found from pixels,
   structure and never content, so Follow holds the column under the cursor and a
   viewfinder frames it on the real screen.
3. **A hideable app has a fixed point** (`adr.rg.015`, `adr.rg.018`): a tray icon, a
   single instance, and a dock - a strip snapped to a screen corner from which the glass
   is switched on in a mode and off again. The glass hangs from the dock.
4. **The panel is the dock's drawer** (`adr.rg.020`), sized by hand from its free corner
   (`adr.rg.021`). v0.2's "two windows, not one" became four windows and one panel: the
   conventional panel window drifted apart from the dock and was removed (6.1).
5. **`PostToolUse` fires on both surfaces** (4.1, measured 2026-09-18). v0.2 drew the hook
   in the same "CLI only" bracket as the status line; that was an assumption about a
   neighbouring channel. The hook is the live diff's trigger everywhere.
6. **The spool lives under the profile root** (`adr.rg.019`): the Claude desktop app is
   packaged (MSIX) and Windows virtualises AppData for every child it runs, so a spool
   under `%APPDATA%` was invisible to the app that read it.
7. **P2's exit criterion was met** (five concurrent sessions, 2026-09-19), **P4 and P4b
   shipped**: the diff within a second, the whole file around a hunk, the previous edit
   as the baseline where git has none, and the latest edit in a colour of its own.
8. **UI Automation tells which session a column belongs to** (`adr.rg.022`): the pane
   header's title, nothing below it, only in the desktop app's windows. Follow chooses the
   session under the glass, and the Diff tab follows the eye.
9. **The roadmap was re-cut** (7): P6 before P5, and the "hawk eye" recorded as P6b.
10. **State is managed before any window exists** (10): a warm start panicked five times
    in five before this; every panic now leaves a line in `~/.reviewglass/panic.log`.

### What changed from v0.1

Three measurements contradicted the draft, and the architecture moved rather than the
measurements:

1. **`statusLine` does not run in the Desktop Code tab.** It runs in the terminal CLI.
   The primary surface depends on the undocumented channel and the secondary one on the
   documented channel - the inverse of what section 4 assumed. Recorded in 4.1; forces
   `adr.rg.003` and `adr.rg.009`.
2. **`context_window.current_usage` is an object, not a number** (5.1). One field of the
   wrong type failed the whole payload, which made "treat every field as optional"
   insufficient on its own. Forces `adr.rg.011`.
3. **The collectors are binaries, not shell scripts** (6.3), which removes the Git Bash
   path-mangling hazard the draft warned about rather than working around it. Forces
   `adr.rg.010`.

---

## 1. Purpose

ReviewGlass is a desktop companion for AI-assisted coding sessions. It solves three
problems that the Claude Code Desktop app does not currently address:

1. **Unreadable text.** With several parallel sessions open, per-session text becomes
   too small to read, and the Code surface exposes no font-size control. The only
   workaround is OS-wide display scaling, which distorts every other application.
2. **Invisible quota.** Subscription rate-limit consumption is not visible in a form
   that warns before a limit is reached. The user is surprised by a "94% of your Opus
   limit" notification with no preceding signal.
3. **No live view of the work.** Code being written by the agent is not observable in
   real time outside the session pane itself.

ReviewGlass is a magnifier glass that hangs from a small always-present dock, and a
panel - the dock's drawer - that surfaces session state, quota consumption and live
diffs. The glass magnifies any screen region; the drawer shows what the agents are doing
and what they are changing, down to the lines the latest edit brought.

It is built first for the author's own use, distributed later as an open-source tool.

---

## 2. Scope

### 2.1 In scope for v1

- Windows 11 only.
- Claude Code as the only instrumented agent.
- Local-only operation. No network egress except one explicitly user-triggered
  feature (see `explain-service`, section 6.3).
- Free, open-source, signed-installer distribution.

### 2.2 Explicit non-goals for v1

| Non-goal | Rationale |
|---|---|
| macOS support | Doubles capture, permission, and packaging work. Deferred to v2. |
| Codex / other agents | Their telemetry surfaces differ. The `session-source` abstraction (6.3) is designed so an adapter can be added without restructuring. |
| OCR of magnified text | Scope creep. The magnifier shows pixels; the diff view shows text. |
| Non-rectangular magnifier shapes | Circles and squares look attractive and are useless for reading code, which is line-oriented. Rectangle only. |
| Commercial licensing, telemetry, accounts | No commercial pressure exists. Adding these later is cheap; removing them is not. |
| Editing code from inside ReviewGlass | It is a read surface. Writing stays in the agent. |

---

## 3. Decisions

| ID | Decision | Resolution | Date |
|---|---|---|---|
| D1 | License | **Apache-2.0** | 2026-09-11 |
| D2 | Explain-service backend | **Both, behind a pluggable interface.** Remote API and a local compute unit are equal-status options; neither is hardcoded. | 2026-09-11 |
| D3 | Primary target surface | **Desktop Code tab is primary. Terminal CLI is a supported secondary surface**, not a separate build. | 2026-09-11 |
| D4 | Name availability | **Open.** Probes on 2026-09-17 (web, GitHub, npm) found nothing; crates.io was not checked. The repository was opened that day by the owner's decision; the name check proper gates P8 - the installer and the bundle identifier - not the source (`adr.rg.008`). | — |
| D5 | Collector form | **Native binaries, not shell scripts** (`adr.rg.010`). Removes the Git Bash / PowerShell question entirely. | 2026-09-11 |
| D6 | Payload tolerance | **A field of the wrong type costs that field only** (`adr.rg.011`), not only a field that is absent. | 2026-09-11 |
| D7 | Roadmap order | **P6 before P5.** The explain backend - local, or remote with the user's own key - is what the "hawk eye" (P6b) stands on; the cache and PR panels wait. Decided by the owner. | 2026-09-23 |

The architecture decisions taken since v0.2 are `adr.rg.012`-`022`, rendered in the
repository under `docs/adr/`; this table keeps the product-level ones.

### 3.1 Consequences of D1 (Apache-2.0)

- `LICENSE` at repository root, verbatim Apache-2.0 text.
- Optional `NOTICE` file; add one only if third-party attribution actually requires it.
- Per-file license headers are recommended by the license but not required. Decide once
  and apply consistently rather than per-file.
- Tauri and the core Rust crates are MIT or MIT/Apache-2.0 dual, so there is no
  compatibility problem. Note that Apache-2.0 is one-way incompatible with GPLv2:
  a GPLv2-only dependency cannot be introduced later without relicensing.

### 3.2 Consequences of D2 (pluggable explain backend)

The explain feature is no longer a single service with a provider setting. It becomes
a consumer plus an interface with two shipped implementations (see `explain-backend`,
section 6). This changes the module boundary and the privacy posture:

- Default state remains **no backend configured**. A fresh installation performs no
  inference of any kind.
- A local backend addresses a `127.0.0.1` endpoint. Loopback traffic is not egress and
  must not be described as such, but the distinction has to be visible in the UI so the
  user always knows which mode is active.
- The interface is deliberately thin, so a third backend can be added without touching
  the diff or panel layers.

### 3.3 Consequences of D3 (Desktop primary, CLI secondary)

- Both surfaces read the same `~/.claude/settings.json`, so a single installer
  configuration covers both. No per-surface setup.
- Session records must carry a `surface` marker. With several Desktop tabs and one or
  more terminal sessions running at once, "which window do I switch to" is only
  answerable if the panel says where each session lives.
- The P0 spike must be run against **both** surfaces, not just Desktop, because the
  fallback path differs per surface (see section 4).

---

## 4. Prerequisite spike (Phase 0)

**Complete. The outcome is in 4.1, and it is not the one this section expected — read
4.1 before 4. The text below is kept as written, because what the spike was asked and
what it returned are both worth having.**

The entire quota panel assumes Claude Code executes the configured `statusLine`
command in the Desktop app's Code tab. The `statusLine` feature is documented from a
terminal perspective. If it does not fire in Desktop sessions, the data source for
Phase 1 changes completely.

**Test:** configure a trivial passthrough statusline that appends its stdin to a log
file. Run it against both surfaces and confirm the log receives JSON containing a
`rate_limits` object:

1. A **Desktop Code tab** session (primary surface, D3).
2. A **terminal CLI** session (secondary surface, D3).

Record the result per surface. They may differ, and the architecture must accommodate
whichever combination is observed.

**Outcomes:**

- **Fires on both:** proceed with the architecture below unchanged. `session-source`
  ships with one implementation.
- **Fires in CLI only:** the quota panel works for terminal sessions and needs a
  second `session-source` implementation for Desktop, built on transcript JSONL files.
  Desktop-created sessions are stored separately from CLI sessions
  (`AppData/Roaming/Claude/claude-code-sessions/` vs `~/.claude/projects/`). This is
  the most likely awkward case, since Desktop is the primary surface.
- **Fires on neither:** both surfaces use the transcript path.

In any fallback case, transcripts yield token counts but **not** rate-limit
percentages. The quota panel then shows consumption trends rather than official limit
percentages, and the optional OAuth usage probe (never specified beyond this mention,
and not built) would become the only source of true percentages. That probe is
reverse-engineered from Claude Code's bundled client rather than documented, so it is
always optional and always fails soft.

### 4.1 Results

| Question | Surface | Result | Date |
|---|---|---|---|
| Is there a native diff panel? | Desktop Code tab | **No.** `/tui` has no function in Code sessions, so neither renderer exists there. `/diff` consequently has no surface to draw on and does nothing. | 2026-09-11 |
| Is there a native diff panel? | Terminal CLI | **Yes.** Shipped in v2.1.260. Requires the fullscreen renderer, a git repo, and a terminal ≥110 columns; auto-opens at ≥144. State persists in `~/.claude.json` as `diffSidebarOpen`. | 2026-09-11 |
| Does `statusLine` run? | Desktop Code tab | **No.** Measured on Claude Code 2.1.268. A Desktop session (`entrypoint: claude-desktop`) started after the passthrough was configured, which produced user and assistant messages, wrote nothing to the log. A CLI session in the same directory three minutes later wrote on every message. | 2026-09-11 |
| Does `statusLine` run? | Terminal CLI | **Yes.** Measured on Claude Code 2.1.268. The first call of a session carries no `rate_limits`, no `prompt_cache` and a null `context_window.current_usage`; from the first API response on, `rate_limits` carries `five_hour` and `seven_day` with `used_percentage` and `resets_at`, `session_name` appears, and `prompt_cache` follows on the next call. `pr` was absent outside a git repository, as specified. Print mode (`claude -p`) runs no status line at all: it has no line to render. | 2026-09-11 |
| Does `PostToolUse` run? | Desktop Code tab | **Yes.** Measured 2026-09-18 on 2.1.274 (`entrypoint: claude-desktop`): a `Write` with the hook configured wrote its event, in a session that was already running when the hook was configured - unlike the status line, no restart is needed. | 2026-09-18 |
| Does `PostToolUse` run? | Terminal CLI | **Yes.** Measured 2026-09-18 on 2.1.268 in print mode (`entrypoint: sdk-cli`), where no status line runs. | 2026-09-18 |

**This is the outcome section 4 called "the most likely awkward case": fires in CLI only,
on the secondary surface, while the primary surface is blind to the channel.**

**CLI payload, measured 2026-09-11 (Claude Code 2.1.268).** Top-level keys:
`session_id`, `transcript_path`, `cwd`, `scratchpad_dir`, `prompt_id`, `effort`, `model`,
`workspace`, `version`, `output_style`, `cost`, `context_window`, `exceeds_200k_tokens`,
`fast_mode`, `thinking`, and — once the session has made an API call — `session_name`,
`rate_limits` and `prompt_cache`.

**How the surfaces were told apart.** Not from the statusLine payload, which offers no
marker: both surfaces write their transcript under `~/.claude/projects/<slug>/` and both
carry a `scratchpad_dir` under `AppData/Local/Temp/claude/`. The spec's earlier note about
Desktop sessions living in `AppData/Roaming/Claude/claude-code-sessions/` does not hold on
2.1.268. The marker is inside the transcript itself: records carry
**`entrypoint`**, `"claude-desktop"` or `"cli"`.

Four consequences for the architecture:

- **`session-source` ships two implementations after all.** `ClaudeStatusLineSource` serves
  CLI sessions; `ClaudeTranscriptSource`, reading the JSONL under `~/.claude/projects/`,
  is the only way a Desktop session reaches the panel. It is no longer conditional.
- **`surface` comes from the transcript's `entrypoint`, not from a guess.** This satisfies
  the `session-source` contract's "derive it from available signals" requirement exactly,
  and the Desktop path has to open the transcript anyway.
- **The account quota is not lost, but it is borrowed.** `rate_limits` is account-wide, so a
  single CLI session running anywhere supplies the gauge for every session in the panel,
  Desktop ones included. When no CLI session is live the gauge has no source and must be
  hidden, not zeroed — and the panel must say *why* it is absent, because "no CLI session
  running" is a condition the user can act on, unlike "no Pro/Max plan".
- **Per-session attribution for Desktop sessions comes from transcript token counts**, which
  are in different units from the quota again. This does not change adr.rg.007; it widens
  it: attribution now has two derivations, and neither may be blended with the gauge.

Two smaller findings worth keeping:

- The first call of a session carries no `rate_limits`, no `prompt_cache` and a null
  `context_window.current_usage`, exactly as section 5.1 predicts. The absent-data path is
  the normal opening state of every session, not an edge case.
- A session started **before** `statusLine` was configured never runs it. The installer must
  tell the user that running sessions are unaffected until they are restarted.

**Consequence for the roadmap.** The diff panel Anthropic shipped on 2026-09-03 does
not overlap with P4 on the primary surface, because it does not exist there. P4 is
therefore kept as originally specified rather than re-scoped. Two secondary points
worth recording:

- On the CLI the native panel splits terminal width between conversation and diff.
  Below roughly 144 columns this makes both halves narrower, which *worsens* the
  original small-text problem rather than relieving it. P1 gains value alongside the
  native panel, not despite it.
- The native panel can attach selected lines to the next prompt. ReviewGlass cannot
  replicate this: it observes sessions and never writes into them. This is a genuine
  advantage of the native feature on the surface where it exists, and should not be
  chased.

**P1 was unblocked by this result and has since shipped.** It depends on no Claude Code
channel at all, which is why the spec put it first.

---

## 5. Data sources

Five channels, four of them in use. Each degrades gracefully, and which one carries a
session depends on the surface it runs on — the finding of section 4.1, and the reason
5.2 exists at all.

### 5.1 statusLine JSON (primary for CLI, documented)

Claude Code pipes a JSON object to a configured command on stdin, on every assistant
message, after `/compact`, on permission-mode change, on vim-mode toggle, and on a
`refreshInterval` timer. **In the terminal CLI only** (4.1).

Fields consumed:

| Field | Use |
|---|---|
| `session_id`, `session_name` | Session identity and panel row label |
| `workspace.current_dir`, `workspace.project_dir` | Which project a session belongs to |
| `transcript_path` | A direct pointer to the file that knows the surface |
| `model.id`, `model.display_name` | Model attribution |
| `rate_limits.five_hour.{used_percentage,resets_at}` | Account-wide 5h quota |
| `rate_limits.seven_day.{used_percentage,resets_at}` | Account-wide weekly quota |
| `cost.total_cost_usd`, `cost.total_duration_ms` | Per-session cost and elapsed time |
| `cost.total_lines_added/removed` | Per-session change volume |
| `context_window.used_percentage`, `context_window.current_usage` | Per-session context fill |
| `prompt_cache.*` | Cache health panel (P5) |
| `pr.number`, `pr.url`, `pr.review_state`, `pr.kind` | PR panel without GitHub API |
| `effort.level`, `thinking.enabled`, `fast_mode` | Session configuration display |

**Known constraints that shape the design:**

- `rate_limits` appears only for Claude.ai Pro/Max accounts, only after the first API
  response of a session, and each window is dropped once its `resets_at` passes.
  Every field must be treated as optionally absent, not as null.
- `rate_limits` has **no per-model breakdown**. The user's "Opus limit" notification
  corresponds to a model-specific weekly limit that this channel does not expose.
- `context_window.current_usage` is **an object** of token counts
  (`input_tokens`, `output_tokens`, `cache_creation_input_tokens`,
  `cache_read_input_tokens`), not a number - measured on 2.1.268, correcting v0.1. It is
  `null` before the first API call and again after `/compact` until the next call.
- **A field may change type, not only disappear.** Optional fields handle an absence and
  do nothing about a wrong shape: on 2.1.268 the `current_usage` mismatch failed the
  entire payload, costing the session, the model, the cost and the quota over a field
  nothing reads. Every field is therefore parsed leniently - a value that does not fit
  becomes absent and its neighbours are untouched (`adr.rg.011`). The two cases collapse
  to the behaviour the spec already prescribes for an absence.
- `prompt_cache` requires Claude Code v2.1.251+. Handle absence.
- `pr` is absent outside a git repo and disappears when the PR merges or closes.

### 5.2 Claude Code transcripts (primary for Desktop, undocumented)

`~/.claude/projects/<slug>/<session_id>.jsonl`, written for every session on both
surfaces whether or not a status line is configured. Since `statusLine` does not run in
the Desktop Code tab, this is the **only** channel that reaches the primary surface, and
it is load-bearing rather than a fallback (`adr.rg.003`).

Fields consumed: `entrypoint` (`"claude-desktop"` / `"cli"` — the only surface marker
there is), `sessionId`, `cwd`, `version`, the record kinds, for a count of assistant
turns, and the `customTitle` of `custom-title` records - the session's title as the
desktop app shows it, which names Desktop sessions in the Sessions table and is what
`adr.rg.022` matches a pane's header against. The record recurs every few lines of a
live session; a title once seen is kept, since a long transcript may hold one only
beyond the tail that is read. Nothing else. **The conversation itself is never read**,
and the file is opened read-only and never written, truncated or moved.

What it does not carry: `rate_limits`, cost, or context percentages. That absence is what
forces the borrowed gauge of `adr.rg.009`.

Being undocumented, it is the channel most likely to change shape. Its reader tolerates a
half-written last line (a growing file is appended to while it is read), skips any record
it cannot parse rather than treating it as the end of the data, and falls back to the file
name for a session id — so a format change degrades this channel rather than emptying the
panel.

### 5.3 PostToolUse hook (primary, documented, both surfaces)

Fires after every `Edit`, `Write`, `MultiEdit` or `NotebookEdit` (the configured
matcher), receiving the changed file path in its JSON input, **in the Desktop Code tab
and in the CLI alike** (4.1). This is the real-time trigger for the live diff on every
surface; the transcript's tool records are not needed and are not read.

The hook must be non-blocking and fast. It writes one content-free event file - which
file, which session, which tool, when - and exits 0. It never returns a non-zero exit
code, because PostToolUse "blocking errors" surface in the session UI without actually
blocking anything, which would be pure noise. It prunes its own events older than an
hour, so a ReviewGlass that is not running leaves nothing to pile up.

### 5.4 Filesystem watcher (fallback, universal - not built)

Would watch the project directory for changes and run `git diff` on modification. Slower
and coarser than the hook, but agent-agnostic: the path a future Codex adapter reuses.
Not built in v1: the hook turned out to reach both surfaces (5.3), which took away the
reason v0.2 had for it.

### 5.5 The desktop app's accessibility tree (Desktop only, `adr.rg.022`)

The Claude desktop app is a Chromium application and answers Windows UI Automation. One
fact is taken from it: which session the point under the cursor belongs to. In the main
window each Code pane is a group (class token `dframe-pane`) whose header row holds a
button named "<session title>, rename session"; a session opened in a window of its own
has a page named with the session's title. Only windows of the desktop app's process are
asked, only the pane and its header row are read - the same tree exposes the chat's text,
which is never touched - and only while the glass is in Follow. The first query of a run
wakes Chromium's accessibility and finds nothing; a read takes 4-15 ms (2026-09-23).

---

## 6. Architecture

### 6.1 Topology

```
Claude Code session 1..N  (Desktop Code tab and terminal CLI)
  │
  ├─ CLI sessions only
  │    statusLine collector ──► ~/.reviewglass/spool/sessions/<session_id>.json   (atomic)
  │
  ├─ both surfaces
  │    PostToolUse hook ──────► ~/.reviewglass/spool/events/<ts>-<tool_use_id>.json (content-free)
  │    transcript (Claude Code's own)  ~/.claude/projects/<slug>/<id>.jsonl        (read-only)
  │
  └─ Desktop Code tab only
       the desktop app's accessibility tree: a pane's header title             (read-only, 5.5)

ReviewGlass process (Tauri v2)
  ├─ Rust core: session sources, usage model, diff loop and git, capture, follow-session
  ├─ glass   frameless, always-on-top: the magnifier, in Follow, Lens or Still
  ├─ dock    a strip in a screen corner; its drawer is the panel (Sessions, Diff, Settings)
  ├─ halo    a ring on the pointer in Follow; click-through
  └─ finder  a frame on the real screen around what the glass shows; click-through
```

The split down the middle is the P0 result made structural: the status line reaches CLI
sessions, the hook and the transcript reach every session, and only the transcript
knows which is which.

**No network listener. No localhost port. No IPC socket.** Inter-process
communication is plain files under the user profile - `~/.reviewglass/spool`, never
under AppData: the Claude desktop app is packaged, and Windows virtualises AppData for
every child it runs, the collectors included (`adr.rg.019`). This is both the simplest
implementation and the strongest privacy claim for the public repository.

**Four windows, one panel.** v0.2 kept the glass and a conventional panel window apart,
since hosting both in one window is the failure mode that stalls projects of this shape.
That still holds for the glass. The panel did not survive as a window of its own: opened
beside the dock but not of it, it could drift away from the thing it belonged to, and in
the owner's words no design had been done for it. It became the dock's drawer, in the
dock's own window, growing from the strip in the corner the dock is snapped to
(`adr.rg.020`) and sized by hand from its free corner (`adr.rg.021`). The halo and the
finder are single-purpose overlays that never take a click. All four windows are
excluded from capture, so the glass never shows ReviewGlass itself.

### 6.2 Technology stack

| Layer | Choice | Rationale |
|---|---|---|
| Shell | Tauri v2 | Transparent always-on-top windows with an HTML/CSS UI at a fraction of Electron's footprint. Electron reserves 200-300 MB before doing anything. |
| Core | Rust | Required by Tauri; also the right layer for Win32 capture interop. |
| Capture | `windows-capture` crate over `Windows.Graphics.Capture` | Microsoft's current recommendation over the legacy Magnification API. |
| Watcher | none yet | The spool is read on a 300 ms tick; `notify` stays the choice for 5.4 if it is ever built. |
| Pane identity | Windows UI Automation through the `windows` crate | The only structural source for which session a column belongs to (`adr.rg.022`). |
| Frontend | SvelteKit (adapter-static) | Small bundle, no virtual-DOM overhead in an overlay redrawn continuously. |
| Diff rendering | `diff2html` (MIT) | Do not hand-roll a diff renderer. |
| Line diff | `similar` (Apache-2.0) | Where git has no baseline (a file outside a repository, or untracked), and for the lines the latest edit brought. |

### 6.3 Module contracts

Modules are specified in Atlas contract form: inputs, outputs, dependsOn.

---

**`statusline-collector`** *(native binary, ships as an installed asset - `adr.rg.010`)*

- **inputs:** statusLine JSON on stdin
- **outputs:** atomic write to `~/.reviewglass/spool/sessions/<session_id>.json`; a
  rendered status line string on stdout. Installed as
  `~/.reviewglass/bin/reviewglass-statusline.exe`
- **dependsOn:** none
- **contract:** MUST always print a status line to stdout. `statusLine` is a
  single-value setting, so ReviewGlass takes over the user's status line entirely; if
  the collector prints nothing the line goes blank. MUST complete in well under the
  300 ms debounce window. MUST exit 0 on any internal failure, falling back to a
  minimal status line. MUST store the payload verbatim and uninterpreted, so a field the
  app does not read yet is still there when it learns to. MUST refuse a session id that
  is not a safe file name rather than writing to a path of the payload's choosing.
  MUST report a parse failure on stderr under `REVIEWGLASS_DEBUG` and say nothing
  otherwise: a collector that has quietly stopped understanding Claude Code is the
  failure nobody would notice.
- **platform note:** on Windows, Claude Code runs status line commands through Git Bash
  when installed, otherwise PowerShell. Git Bash consumes unquoted backslashes, silently
  mangling Windows-style paths. Shipping the collector as a native binary that Claude
  Code invokes directly removes the question - no shell interprets the command or its
  path (`adr.rg.010`). Forward slashes in configured paths remain the convention.

---

**`hook-collector`** *(native binary, ships as an installed asset - `adr.rg.010`)*

- **inputs:** PostToolUse JSON on stdin, on both surfaces (5.3)
- **outputs:** `~/.reviewglass/spool/events/<ts>-<tool_use_id>.json` containing
  `{ts, session_id, transcript_path, cwd, hook_event, tool, tool_use_id, file_path}` -
  never the file's content. Installed as `~/.reviewglass/bin/reviewglass-hook.exe`,
  configured with the matcher `Edit|Write|MultiEdit|NotebookEdit`
- **dependsOn:** none
- **contract:** fire-and-forget; always exits 0; never writes to stderr; does no git
  work itself; prunes its own events older than an hour.

---

**`spool-watcher`** *(Rust)*

- **inputs:** the spool directory, `~/.reviewglass/spool` (`adr.rg.019`)
- **outputs:** `SessionSnapshot` and `ChangeEvent` values
- **dependsOn:** none
- **contract:** reads by polling, not by filesystem events: the session records on the
  usage loop's tick, the events on the diff loop's 300 ms tick. MUST tolerate partially
  written files (readers retry once on parse failure, then leave the file for the next
  pass). MUST expire session records whose file has not been touched within a TTL
  (10 minutes) so closed sessions leave the panel — Claude Code writes no close record on
  either channel, so age is the only signal a session ended. MUST handle N concurrent
  writers and one reader without locking. Event files are consumed: deleted once read,
  and events from before the app started are consumed without being reported. The path
  is named in one function that the app and both collectors share, and nothing shared
  ever lives under AppData.

---

**`session-source`** *(Rust, trait)*

- **inputs:** spool records (CLI sessions); transcript records (every session)
- **outputs:** normalized `SessionSnapshot`
- **dependsOn:** `spool-watcher`, the transcript directory
- **contract:** the abstraction boundary that lets a future Codex adapter be added
  without touching consumers, and the place the two Claude Code surfaces are reconciled.
  **v1 ships BOTH implementations** (`adr.rg.003`), not one plus a conditional:
  `ClaudeStatusLineSource` reads the spool and reaches CLI sessions;
  `ClaudeTranscriptSource` reads the JSONL under `~/.claude/projects/` and is the only
  thing that reaches a Desktop session, because `statusLine` does not run there.
  Every `SessionSnapshot` MUST carry a `surface` field (`desktop` / `cli` / `unknown`)
  taken from the transcript's **`entrypoint`**, never guessed: the statusLine payload
  carries no marker, and on 2.1.268 both surfaces write transcripts under
  `~/.claude/projects/` and both carry a `scratchpad_dir`, so neither separates them.
  A session seen on both channels is one session keyed by `session_id`, with the
  statusLine record winning field by field (fresher, richer) while the transcript owns
  `surface` outright. A session already found through the spool resolves its surface
  from the transcript its own payload names, rather than depending on a directory scan
  reaching it. Transcripts are opened read-only and never written, truncated or moved.
  A Desktop session's name is its transcript's `custom-title` (5.2); a statusLine
  `session_name` wins where both exist.
- **which session is under the cursor** (`adr.rg.022`, `follow_session.rs`): while the
  glass is shown in Follow and the owner's switch is on, a UI Automation client on a
  thread of its own reads the pane header's title (or a session window's page name) at
  the cursor, only in the desktop app's windows, and matches it exactly (then by case)
  to a session's name; the match becomes the session choice that filters the Diff tab.
  A read happens only when the cursor leaves the pane it was last found in, plus a
  re-check every 4 s; a point that found nothing is not asked again within 40 px for
  1.5 s; nothing is read over ReviewGlass's own windows. A miss leaves the choice as it
  was; a choice made by hand stands until the pane changes.

---

**`usage-model`** *(Rust)*

- **inputs:** stream of `SessionSnapshot`
- **outputs:** `AccountQuota` (shared) and `SessionAttribution[]` (per session)
- **dependsOn:** `session-source`
- **contract — this is the module most easily specified wrongly:**
  - `rate_limits` values are **account-wide**. All concurrent sessions report the same
    `five_hour.used_percentage`. The model MUST NOT present the shared percentage as a
    per-session figure.
  - `rate_limits` arrives only through `statusLine`, which runs only in the CLI, so the
    gauge is **borrowed from whatever CLI session is live** and shown for every session
    including Desktop ones (`adr.rg.009`). With no CLI session the gauge has no source
    and MUST be hidden with its reason named: "no CLI session is running" is a condition
    the user can act on, unlike "no Pro/Max plan", so the two absences MUST NOT share a
    message.
  - Attribution has **two derivations**: statusLine cost deltas for CLI sessions,
    transcript token counts for Desktop ones. They are not comparable unit for unit, so
    each share carries its basis and a mixed set is flagged rather than blended.
  - Per-session attribution is a **separate, derived** quantity. It answers "which
    session is consuming most" in relative terms only.
  - Attribution and quota are different units. Quota weights by model; token counts do
    not. The UI MUST show them as two distinct things: one shared gauge plus N
    relative shares, never a single blended number.
  - Burn rate is computed from consecutive `(used_percentage, observed_at)` pairs, and
    is absent until two samples exist far enough apart: a rate from one point is not an
    estimate but a fabrication. Time-to-limit is a linear projection and MUST be
    labelled as an estimate.
  - `resets_at` is Unix epoch seconds. Crossing it resets the window; the model MUST
    discard pre-reset samples rather than averaging across the boundary. 94 % to 3 % is a
    reset, not a fall of 91 points per hour.

---

**`capture-engine`** *(Rust)*

- **inputs:** target rectangle in virtual-desktop coordinates, zoom factor
- **outputs:** scaled frame buffer
- **dependsOn:** none
- **contract:**
  - MUST call `SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)` on the glass
    window. Without this the glass captures itself and produces infinite recursion.
  - MUST be per-monitor DPI aware and handle mixed-scaling multi-monitor setups. It
    attaches to the monitor under the source rectangle and re-attaches when the
    rectangle crosses to another.
  - MUST throttle when the source region is static, to keep idle CPU negligible. The
    effective mechanism is the compositor's dirty regions: when none intersects the
    source box the crop is skipped entirely, which is what the cost actually sits in —
    a crop allocates a staging texture and copies through it.
  - Zoom is clamped to 150-400%. Above roughly 400% bitmap scaling visibly degrades;
    offering higher factors would promise sharpness the technique cannot deliver.
  - A hidden glass holds no capture session at all.
  - **Pane detection** (`adr.rg.017`): the column under the cursor is found from pixels -
    a gutter or a border line between two textured spans is a boundary, an indentation
    gap narrower than a gutter is not - and a tracker holds the column through brief
    losses (the right edge is the furthest line of the last ten seconds; a lost column is
    held two seconds). Structure only, never content: no OCR (`adr.rg.006`).

---

**`glass-window`** *(Tauri window + frontend)*

- **inputs:** frame buffer, user input, the dock's activation
- **outputs:** rendered overlay
- **dependsOn:** `capture-engine`, `config-store`, `dock-window`
- **contract:** a window like any other (`adr.rg.013`): frameless and always-on-top,
  with a permanent title bar carrying the drag handle, the mode as three labelled
  buttons, zoom, size, the halo toggle and a chrome-scale step; resizable by explicit
  grips. Three named modes: **Follow** (the glass stays put and shows the column around
  the cursor, held by the pane lock, with Fit sizing the glass to the column),
  **Lens** (the glass rides on the cursor, click-through), **Still** (the picture stops -
  a still, not a locked live region, so a captured instruction survives working
  elsewhere, `adr.rg.012`). Switched on and off from the dock (`adr.rg.018`) or the global
  shortcut, which the dock names. Starts hidden at every start. In Follow the source holds
  still while the pointer is over the glass or the dock, so reaching for a control never
  swaps the picture. Geometry, zoom and mode persist; the parked glass and the lens keep
  separate sizes, stored only when the user resizes. Every action has a visible control
  as well as its keyboard or mouse gesture.

---

**`dock-window`** *(Tauri window + frontend)*

- **inputs:** glass state, the usage view, user input
- **outputs:** activation of the glass; the drawer
- **dependsOn:** `glass-window`, `usage-model`, `config-store`
- **contract:** the control panel and the fixed point (`adr.rg.018`): a strip snapped to a
  screen corner, visible at every start, carrying the three mode buttons (a click shows
  the glass in that mode, a click on the lit one hides it), the account gauge in
  miniature (the same account-wide figure as the panel, its absence named), the drawer
  button, the build stamp and a visible quit. Dragged anywhere, it snaps to the nearest
  corner and remembers it. Its drawer is the panel (`adr.rg.020`): the same window grows
  from the strip away from the screen's edge and back; its free corner is a named handle
  that resizes it with the snapped corner fixed, and the size is remembered
  (`adr.rg.021`). The tray, the glass's bar and its menu open the same drawer; there is
  no other panel window.

---

**`halo-window`** *(Tauri window + frontend)*

- **inputs:** the cursor position, glass mode
- **outputs:** a ring on the pointer
- **dependsOn:** `glass-window`
- **contract:** while the glass is in Follow, a ring rides on the pointer so the eye, on
  the parked glass, still knows where the hand is (`adr.rg.014`). Centred to the pixel;
  never takes a click; excluded from capture; hidden in Lens and Still and while the
  glass is hidden; off by one click on the glass's bar, and the choice persists.

---

**`finder-window`** *(Tauri window + frontend)*

- **inputs:** the engine's source rectangle, the pane
- **outputs:** a frame on the real screen
- **dependsOn:** `capture-engine`
- **contract:** while the glass is in Follow with the pane lock on, a frame drawn on the
  real screen around the rectangle the glass shows, in the Follow colour: it answers
  what the pane detector found and where exactly the glass is looking (`adr.rg.017`).
  Coincides with the source rectangle to the pixel; never takes a click; excluded from
  capture; hidden in Lens and Still, while the glass is hidden, while the lock is off and
  while no pane is found.

---

**`panel-window`** *(the dock's drawer - frontend, `src/lib/panel/`)*

- **inputs:** `AccountQuota`, `SessionAttribution[]`, `DiffView`, `FileView`, the Follow
  sighting; later `CacheStats`, `PrInfo`
- **outputs:** user interactions
- **dependsOn:** `usage-model`, `diff-service`, `dock-window`
- **contract:** the dock's drawer (`adr.rg.020`), not a window of its own. Tabs in its
  first row: **Sessions** (the default: one shared quota gauge above, N relative shares
  below, never blended; Desktop sessions by their titles), **Diff**, **Settings**; the
  last tab is remembered. All tabs render correctly when their data source is absent
  (no Pro/Max plan, no git repo, no CLI session, older Claude Code version) by hiding the
  affected element rather than showing zeros or errors — and by naming the reason
  wherever the user can act on it. **The session choice:** a radio column on the Sessions
  table chooses all sessions or one, the chosen row bold; the Diff tab then shows that
  session's edits alone and says so, with "show all sessions" one click away. Follow can
  make the choice (5.5, `session-source`), marked as Follow's, behind a visible switch on
  the Sessions tab that says what Follow sees.

---

**`diff-service`** *(Rust)*

- **inputs:** `ChangeEvent`
- **outputs:** `DiffView` (path, status, baseline, unified text, add/remove counts, the
  lines the latest edit brought); `FileView` (the working copy with the last diff's marks)
- **dependsOn:** `spool-watcher`, `config-store`
- **contract:** runs `git diff HEAD` scoped to the changed path, in the repository that
  contains it, on its own 300 ms loop; a tick is the debounce: every event for one path
  within it becomes one diff. Never edits, stages or writes into a repository.
  - **Statuses, each with its reason:** changed, unchanged, untracked, not in a
    repository, denied, missing, too large (over 512 KB), binary, git failed.
  - **Denylist first:** a path matching the configurable secret-file denylist (`.env*`,
    `*.pem`, `*.key`, `id_*`, `*.tfvars`) is shown as denied - the user sees that the
    agent touched it - and is never handed to git or read.
  - **Where git has no baseline** (outside a repository, or untracked) the baseline is
    the working copy remembered from the previous edit ReviewGlass saw, diffed with
    `similar` in git's unified form; the first sighting shows the whole file as new. The
    view says which baseline it has: `head`, `last-edit` or `whole-file`.
  - **The latest edit:** the working copy of every listed file that reads as text within
    the cap is remembered in memory (dropped with the view) and each view carries the
    lines the latest edit brought against it; at the first sighting, every added line.
    The Diff tab paints them with ReviewGlass's own highlighter and leaves older
    uncommitted work diff-green; a New view shows them alone.
  - **The whole file (P4b):** `panel_file_view` reads the working copy of a path the
    loop has listed - no other path - under the same denylist, cap and binary check, and
    returns it with the last diff's marks (added lines, where lines were removed, the
    hunks). The hunk view stays the default; a hunk header opens the file at that hunk.

---

**`explain-service`** *(Rust + frontend) — novice mode*

- **inputs:** a selected diff hunk, explicit user action
- **outputs:** natural-language explanation
- **dependsOn:** `diff-service`, `explain-backend`
- **contract:**
  - MUST be explicitly user-triggered by a button on a specific hunk. MUST NOT run
    automatically, on a timer, or as a background process.
  - MUST be disabled by default in a fresh installation, with no backend selected.
  - MUST honour the same secret-file denylist as `diff-service`.
  - MUST display which backend is active whenever a request is made, so local and
    remote modes are never confused for one another.
  - MUST NOT contain any provider-specific logic. It composes a request and hands it
    to the backend interface.
  - Construction of the prompt is this module's responsibility: the hunk, the file
    path, and the surrounding declaration are sufficient context. It MUST NOT send the
    whole file, the repository, or the session transcript.

---

**`explain-backend`** *(Rust, trait + two implementations)*

- **inputs:** an explain request (hunk, path, context)
- **outputs:** explanation text, or a typed failure
- **dependsOn:** `config-store`
- **contract:**
  - The trait is intentionally minimal: one call in, text or error out. Adding a third
    backend must require no change outside this module.
  - **`RemoteBackend`** — a hosted API addressed over the public internet. Credentials
    MUST be stored in Windows Credential Manager, never in a settings file, never in
    the repository, never in logs. On first use per installation it MUST show the
    exact payload that will leave the machine and require confirmation.
  - **`LocalBackend`** — an endpoint on `127.0.0.1` provided by a local compute unit.
    Configured by host, port, and model name. Loopback traffic is not egress and MUST
    NOT be described as such in the UI, but the active mode MUST remain visible.
    Failure to reach the endpoint is a normal, expected state (the local unit may
    simply be off) and MUST produce a plain "backend unavailable" message, not an
    error dialog.
  - Both implementations MUST time out (proposed: 30 s) and MUST be cancellable from
    the UI.
  - Neither implementation may retry automatically. A failed explanation is a
    user-visible non-event, not something to keep attempting in the background.

---

**`notifier`** *(Rust)*

- **inputs:** `AccountQuota`
- **outputs:** Windows toast notifications
- **dependsOn:** `usage-model`
- **contract:** configurable thresholds (default 75% and 90%) per window. Fires once
  per threshold per window; resets at `resets_at`. Never repeats within a window.

---

**`config-store`** *(Rust)*

- **inputs:** user settings
- **outputs:** persisted configuration
- **dependsOn:** none
- **contract:** JSON under the user profile
  (`%APPDATA%/app.sunrisesoftware.reviewglass/config.json`, which the app creates itself
  and is therefore real, not virtualised). Contains no secrets. Writes atomically (temp
  file plus rename). A corrupt config file resets to defaults, keeps the old one beside
  it as `.bak`, and says so visibly rather than failing to start. Opened before the
  app's windows exist (10).

---

**`installer-integration`** *(installer step)*

- **inputs:** existing `~/.claude/settings.json`
- **outputs:** both collectors placed under `~/.reviewglass/bin`; updated settings with
  `statusLine` and `PostToolUse` entries; the app's own toast identity
- **dependsOn:** none
- **contract:** MUST back up the existing settings file before modification. MUST
  detect an existing `statusLine` value and ask before replacing it, offering to
  chain the previous command. MUST be reversible by the uninstaller. MUST warn when
  workspace trust has not been accepted, since neither hooks nor status line run
  until it is, and when `disableAllHooks` or `allowManagedHooksOnly` is set, which
  suppresses both silently. MUST tell the user that sessions already running are
  unaffected until they are restarted, since a session only runs the status line that
  existed when it started (4.1) - the hook, measured, does not need the restart. MUST
  place the spool and the collectors under the profile root, never under AppData
  (`adr.rg.019`). MUST give the app its own toast identity: the unpackaged exe's toasts
  are attributed to Windows PowerShell today.

---

## 7. Roadmap

Each phase is independently useful and independently shippable.

| Phase | Deliverable | Modules | Exit criterion |
|---|---|---|---|
| **P0** | Spike | — | **Done.** Diff-surface question resolved per surface; statusLine confirmed on both surfaces (CLI yes, Desktop no); PostToolUse on both (2026-09-18) |
| **P1** | Magnifier | `capture-engine`, `glass-window`, `halo-window`, `finder-window`, `dock-window`, `config-store` | **Done.** Readable magnified text over the Code tab, no self-capture, geometry persists. Idle CPU 1.35 % of one core with a static source. Since grown into three modes, the pane lock and the dock (`adr.rg.012`-`018`) |
| **P2** | Session panel | `statusline-collector`, `spool-watcher`, `session-source`, `usage-model`, `panel-window` | **Done.** Five concurrent CLI sessions observed 2026-09-19 through the app's own reader, with the same account-wide figures and per-session cost and context, beside Desktop sessions from their transcripts |
| **P3** | Burn rate & alerts | `notifier`, `usage-model` extension | **Built.** The toast measured working through the test button (2026-09-12); a real threshold crossing not yet observed |
| **P4** | Live diff | `hook-collector`, `diff-service`, the Diff tab | **Done.** An edit in the Diff tab 0.23 s after the hook (2026-09-18), on both surfaces |
| **P4b** | The whole file, the new code, the session under the glass | `diff-service`, `session-source`, the drawer | **Done.** The whole file around a hunk (2026-09-20); the previous edit as the baseline where git has none; the latest edit highlighted and a New view (2026-09-23); the drawer sized by hand (`adr.rg.021`); a session choice, made by hand or by Follow (`adr.rg.022`) |
| **P6** | Novice mode | `explain-service`, `explain-backend` | **Next** (D7). Explanation on demand through both backends - local on `127.0.0.1`, or remote with the user's own key; disclosure and credential storage verified; default-off state verified on a clean install |
| **P6b** | Hawk eye | `explain-service`, `explain-backend`, `diff-service` | **Vision.** A watcher through the P6 backend that reads the columns and the new code as they change and raises what looks wrong. A continuous send is a new privacy line (opt-in, per repository, denylist, payload disclosed) and needs its own decision before anything is built |
| **P5** | Cache & PR panels | drawer tabs | `prompt_cache` and `pr` surfaces rendered, absent-data paths verified |
| **P7** | Packaging | `installer-integration`, updater | Clean install and uninstall on a fresh Windows 11 machine; both collectors placed; the app's own toast identity |
| **P8** | Public release | — | The name cleared (D4), a signed or documented installer, release notes. The repository itself is already public |
| **v2** | macOS, Codex adapter | new `session-source` impl, ScreenCaptureKit backend | out of v1 scope |

Phase 1 alone solved the original complaint and was shipped to the author before anything
else was written. That ordering held up: it was the one phase that depended on no Claude
Code channel, and the channel the other phases depended on turned out not to exist on the
primary surface.

---

## 8. Packaging and distribution

### 8.1 Build

Tauri's bundler produces both Windows installer formats, and both were verified on
2026-09-11 (NSIS 1.4 MB, MSI 2.1 MB):

```
src-tauri/target/release/bundle/nsis/ReviewGlass_0.1.0_x64-setup.exe
src-tauri/target/release/bundle/msi/ReviewGlass_0.1.0_x64_en-US.msi
```

NSIS (`-setup.exe`) is the primary artifact: it can be built on non-Windows hosts,
which keeps CI options open. MSI is secondary; building it requires the VBSCRIPT
optional Windows feature to be enabled, otherwise the WiX step fails.

Three binaries ship: the app, and the two collectors - the statusLine collector and the
PostToolUse hook collector - installed under `~/.reviewglass/bin` (`adr.rg.010`,
`adr.rg.019`), which is why the crate declares `default-run`.

### 8.2 Updates

Tauri's updater plugin with `"createUpdaterArtifacts": true` produces signed `.sig`
files checked against a project keypair. This secures the update channel and is
distinct from Authenticode code signing, which addresses SmartScreen.

### 8.3 Signing

Not required for personal use or an open-source release. A SmartScreen warning on an
unsigned build is expected and should be documented in the README rather than worked
around. If signing is added later, Microsoft's own Artifact Signing service (from
about $10/month, no hardware token) is the appropriate route; an EV certificate no
longer bypasses the first-run SmartScreen prompt and does not justify its premium.

### 8.4 Privacy posture

Stated plainly in the README, because it is a genuine differentiator:

- Screen pixels are read, scaled, and displayed. They are never written to disk or
  transmitted.
- Session data is read from files Claude Code already writes on the same machine: the
  spool the collectors fill, and the JSONL transcripts, which are opened read-only. Only
  session state is read from a transcript - its surface, folder, version, record kinds
  and title - never the conversation. The hook records which file an edit touched,
  never its content.
- While the glass is in Follow, the desktop app's accessibility tree is asked which
  session is under the cursor: the pane header's title, nothing below it, only in the
  desktop app's windows (`adr.rg.022`). The same tree exposes the chat; it is not read.
- The files an agent edits are read to show their diff: git answers for a repository;
  elsewhere, and to mark the latest edit, the working copy is remembered in memory
  between edits and forgotten with the view. Nothing is written into a file, a
  repository or a session.
- A crash writes one line - build, thread, place, message - to
  `~/.reviewglass/panic.log`, on this machine only.
- The account quota reaches ReviewGlass only through a terminal `claude` session, because
  that is the only surface Claude Code runs a status line on. A Desktop-only user sees
  sessions but no gauge, and the panel says so rather than showing a blank.
- No analytics, no accounts, no network listener.
- The explain feature is off by default with no backend selected. A fresh installation
  performs no inference.
- When enabled, it offers two modes. A **local** backend keeps everything on the
  machine over loopback. A **remote** backend is the only path on which anything
  leaves the machine, is user-triggered per hunk, and discloses its payload before the
  first send.
- Only the selected hunk and its immediate context are ever sent. Never the file,
  the repository, or the conversation.

**One measurement that does not support a claim in 6.2.** On 2026-09-11 the running app
held about 700 MB of private bytes across eight processes, essentially all of it
WebView2's. On 2026-09-23, with four windows, idle (the glass hidden, the drawer closed,
a few minutes after start): 318 MB of private bytes across ten processes, 15 MB of it the
Rust core and the rest nine WebView2 processes (660 MB of working set). The stack table's
"a fraction of Electron's 200-300 MB" still does not hold as written. This is recorded
rather than quietly dropped, and it should not stand in the README unexamined.

---

## 9. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| statusLine does not run in Desktop Code tab | Removes the primary data source | **This fired.** The transcript reader is no longer a fallback but the primary surface's only channel; the account gauge is borrowed from a CLI session (`adr.rg.003`, `adr.rg.009`) |
| Claude Code changes the statusLine JSON shape | Panel degrades or breaks | **This fired, in a form the mitigation did not cover.** Treating every field as optional survives a field that disappears and not a field that changes type; `current_usage` changed type and failed the whole payload. Every field is now parsed leniently (`adr.rg.011`) |
| The transcript format changes | Desktop sessions vanish from the panel | The channel is undocumented and load-bearing, which is uncomfortable and unavoidable. Its reader skips what it cannot parse and falls back to the file name for a session id, so a change degrades the channel rather than emptying it |
| User already has a status line configured | ReviewGlass silently replaces it | Installer detects, asks, offers to chain |
| Capture recursion | Unusable overlay | `WDA_EXCLUDEFROMCAPTURE`, verified in P1 |
| Mixed-DPI multi-monitor | Misaligned or wrongly scaled capture | Per-monitor DPI awareness from the start, not retrofitted |
| Idle CPU cost of continuous capture | Tool becomes annoying to leave running | Measured in P1: 14 % of one core before the dirty-region skip, 1.35 % after |
| A packaged app virtualises what its children write | Collectors write where the app cannot read | **This fired** (2026-09-18): the spool under AppData was invisible to the app. It lives under the profile root (`adr.rg.019`), and the app under test is launched the way the owner launches it |
| The desktop app changes its pane structure | Follow stops choosing the session | Every read then finds "no match", never a wrong session; the choice stays as it was and the Sessions tab says what Follow saw (`adr.rg.022`) |
| Chromium's accessibility cost in the desktop app | The desktop app does more work while Follow runs | Reads only when the cursor changes pane, every 4 s otherwise; measured at 4-15 ms per read. The app's own load could not be separated from the owner's running sessions on 2026-09-23 |
| State missing when a window's first command arrives | The app dies on start | **This fired** (2026-09-23, 5 of 5 warm starts). State is managed on the builder, before any window exists; a panic leaves a line in `~/.reviewglass/panic.log` |
| Scope creep into an IDE | Project never ships | Non-goals in 2.2 are binding |

---

## 10. Notes for the build agent

- Repository language: English for code, comments, commit messages, and documentation.
- All configured file paths on Windows use forward slashes.
- Every consumer of Claude Code data must assume the field may be absent **or of an
  unexpected type**. The correct behaviour for both is to hide the element, not to render
  a zero. Absent data should also say *why* it is absent wherever the reason is something
  the user can act on.
- License is Apache-2.0. Do not introduce a GPLv2-only dependency at any point.
- `explain-service` must contain no provider-specific code. If a provider name appears
  outside `explain-backend`, the boundary has been violated.
- ReviewGlass is a read surface. It never writes into a session, a repository or a
  transcript.
- P0-P2, P4 and P4b are done and P3 is built. P6 is next (D7), then P5, P7, P8.
- Tauri creates the windows declared in `tauri.conf.json` before `setup` runs, and serves
  a loaded page's commands while it creates the others. Every piece of state is managed
  on the builder, never in `setup`; a panic is logged to `~/.reviewglass/panic.log`, and
  that log is read before anything else is guessed.
- Nothing shared lives under AppData (`adr.rg.019`). The app under test is launched the
  way the owner launches it - Explorer or a plain PowerShell - never from the agent's
  shell, whose children see a virtualised AppData.
- A check that needs the screen or the mouse is run with the owner's leave, once per
  change; everything that can be verified without the screen comes first.
- Ship the affordance with the mechanism: a visible control for every action, a
  discoverable first-run state, and a named empty state. A feature reachable only by a
  keystroke nobody was told about is not finished.
- D4 (name) blocks P8 only. Development proceeds under the working name, in a public
  repository.
