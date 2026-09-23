# ReviewGlass — notes for the build agent

A Windows 11 desktop companion for Claude Code sessions: a magnifier glass hanging from
a dock whose drawer is the panel — sessions and quota, threshold alerts, a live diff. Tauri v2, Rust core,
SvelteKit. Apache-2.0. Built first for the author's own use; the repository has been
public since 17.9.2026, and the public release (P8) waits on the name (adr.rg.008).

Read `docs/REVIEWGLASS-SPEC.md` before changing architecture. Section 6.3 is the module
contract list. The Atlas model (system `reviewglass`) is the source of truth for
modules, connections and decisions; the spec is its prose.

## Golden rules

- **Language.** Code, comments, commit messages and documentation in English.
  Conversation with the owner in Finnish.
- **License.** Apache-2.0. Never introduce a GPLv2-only dependency.
- **Absent data hides its element.** Every consumer of Claude Code data assumes a field
  may be absent *or of an unexpected type* (adr.rg.011). Never a zero, a dash or an
  error in place of a missing figure — and where the reason is something the user can
  act on, say the reason ("no CLI session is running").
- **`rate_limits` is account-wide.** Never a per-session figure (adr.rg.007). The gauge
  is borrowed from whatever CLI session is live (adr.rg.009); attribution is a separate,
  relative quantity, and the two are never blended.
- **A read surface.** ReviewGlass never writes into a session, a repository or a
  transcript. Transcripts are opened read-only, and only session state is read from
  them — never the conversation.
- **The glass never captures itself.** `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`
  on the glass and on the halo.
- **`explain-service` contains no provider-specific code.** A provider name outside
  `src-tauri/src/explain/backend` is a boundary violation.
- **Collectors always exit 0 and always print a line.** They are native binaries
  (adr.rg.010), never shell scripts. Configured Windows paths use forward slashes.
- **Nothing shared lives under AppData.** The Claude desktop app is packaged, and
  Windows virtualises AppData for every child it runs — including the Desktop Code
  tab's sessions, their hooks and the build agent's shell. The spool is
  `~/.reviewglass/spool` (adr.rg.019). A check of the app is run with the app launched
  the way the owner launches it (Explorer or a plain PowerShell), never from the
  agent's shell.
- **Ship the affordance with the mechanism.** A visible control for every action, a
  discoverable first-run state, a named empty state. A feature reachable only by a
  keystroke nobody was told about is not finished.
- **A hideable window has a fixed point.** The tray icon and single-instance rule
  (adr.rg.015) are not optional: without them a hidden app becomes two processes.

## Working practice

- Verify before declaring done: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test`, `pnpm check`. CI on windows-latest runs the same and is the gate.
- Build the release and run it: `pnpm tauri build`, then launch
  `src-tauri/target/release/reviewglass.exe` (the Desktop and Start Menu shortcuts point
  there). Stop the running instance first — it holds the config file. Measure on the
  release build, never the dev build.
- Commit straight to `main` while the repo is single-author; keep CI
  green. Commit subject: what changed and why, in English; the body records what was
  measured.
- **A decision enters the Atlas model first.** New ADR → `decisions[]` in
  `model/systems/reviewglass.model.json` in the Atlas checkout, validate
  (`pnpm validate:models`, `pnpm review --system reviewglass`), PR, merge, then
  `pnpm --filter @atlas/mcp-worker exec wrangler deploy` (CI only dry-runs the deploy).
  Then render the repo mirror: `node scripts/adr-from-model.mjs`. A hand edit under
  `docs/adr/` is lost on the next run.
- End each session by updating `docs/CHANGELOG.md`, `docs/BUILD_INFO.json` and, when a
  lesson was learned, `docs/LESSONS.md`; put the status to Atlas
  (`atlas_put_status`, system `reviewglass`).
- The spec is Atlas artifact `REVIEWGLASS-SPEC`; `docs/REVIEWGLASS-SPEC.md` is its copy.
  Change both in the same session or neither.

## Session handover: what a cold start reads

1. **This file** — rules, practice, the lessons below.
2. **`docs/BUILD_INFO.json`** — where the build stands, what is next, how it is deployed.
3. **`docs/CHANGELOG.md`** — what shipped and why, newest first, with what was measured.
4. **`docs/adr/`** — the decisions, rendered from the Atlas model; `README.md` there is
   the index.
5. **`docs/LESSONS.md`** — the pitfalls, with the story behind the rules above.
6. **`docs/REVIEWGLASS-SPEC.md`** — the product and architecture specification (v0.3,
   23.9.2026; 6.3 carries the module contracts, 7 the roadmap in the owner's order).
7. Atlas (optional but useful): system `reviewglass` — `atlas_get_workspace` for status
   and thread, `atlas_get_rationale` for the ADRs as the model holds them.

The owner's machine has the spike rig still installed
(`~/.reviewglass/spike/`, `~/.claude/settings.json.bak-reviewglass-p0`); the real
collector replaced it in `settings.json` on 11.9.2026.

## Roadmap gates

- **P0** done: statusLine fires in the CLI, not in the Desktop Code tab.
- **P1** done. **P2** done (five concurrent sessions observed 19.9.2026). **P3** built;
  the toast is measured working.
- **P4** (live diff) built and observed 18.9.2026: `hook-collector`, `diff-service`, the
  Diff tab. **P4b** (the whole file around a hunk) observed 20.9.2026, with the previous
  edit as the baseline where git has none; the latest edit's lines are highlighted and
  a New view shows them alone (23.9.2026).
- **P6** (novice mode: the explain backend, local or remote with the user's own key)
  built 23.9.2026 (adr.rg.023), before P5 by the owner's decision; the "hawk eye" is P6b, a vision
  that needs its own privacy decision first (spec v0.3, section 7).
- **P8** (public release) waits on name clearance (adr.rg.008); the repository itself is
  already public.

## Layout

| Path | What |
|---|---|
| `src-tauri/src/capture/` | `rg.capture-engine`: Windows.Graphics.Capture, crop, dirty-region skip |
| `src-tauri/src/glass.rs` | `rg.glass-window` Rust side: modes, hotkeys, lens/halo rider thread, context menu |
| `src-tauri/src/tray.rs` | tray icon; the single-instance hook is in `lib.rs` |
| `src-tauri/src/session/` | `rg.session-source`: payload types, spool reader, transcript reader, merge |
| `src-tauri/src/usage.rs` | `rg.usage-model`: one gauge, N shares, burn rate |
| `src-tauri/src/notifier.rs` | `rg.notifier`: threshold logic (delivery is in `panel.rs`) |
| `src-tauri/src/dock.rs` | `rg.dock-window`: the strip, its corner, and the drawer that is the panel (adr.rg.020) |
| `src-tauri/src/panel.rs` | usage loop thread, the drawer's tab commands, alert settings |
| `src-tauri/src/config.rs` | `rg.config-store`: atomic JSON, corrupt-file recovery |
| `src-tauri/src/spool.rs` | `rg.spool`: paths (`~/.reviewglass/spool`), atomic write, safe file names |
| `src-tauri/src/bin/hook.rs` | `rg.hook-collector` (PostToolUse → `spool/events`) |
| `src-tauri/src/diff/` | `rg.diff-service` and the events reader: git diff per changed path, the Diff tab's data |
| `src-tauri/src/explain/` | `rg.explain-service` (provider-agnostic) and `backend/` (`rg.explain-backend`: local, remote, the key in Credential Manager) — the only place a provider is named |
| `src-tauri/src/follow_session.rs` | Follow chooses the session (adr.rg.022): the Code pane's header title under the cursor, by UI Automation |
| `src-tauri/src/bin/statusline.rs` | `rg.statusline-collector` |
| `src/routes/glass`, `dock`, `halo`, `finder` | the four windows; `src/lib/panel/` holds the drawer's tabs (Sessions, Diff, Settings) |
| `scripts/adr-from-model.mjs` | renders `docs/adr/` from the Atlas model |

## Lessons that became rules

Full stories in `docs/LESSONS.md`. The short forms:

- **A payload with no surface marker is not evidence about the surface** (11.9.2026).
  The spike's first reading was backwards. Read a transcript's `entrypoint`, never guess.
- **Optional is not lenient** (11.9.2026). `current_usage` changed type and one strict
  `Option` failed the whole payload. Every Claude Code field parses leniently.
- **A skip keyed on "the input did not change" must include every input** (12.9.2026).
  The dirty-region skip ignored the rectangle's own position; the lens vibrated.
- **Never write a derived value back as the setting it came from** (12.9.2026). The
  canvas size was stored as the window size and the window shrank on every restart.
- **A hideable window needs a fixed point** (12.9.2026). Two instances were alive at
  once before the tray existed.
- **A session runs the status line that existed when it started** (11.9.2026). The
  installer must say so.
- **State is managed on the builder, never in `setup`** (23.9.2026). Tauri creates the
  config windows before `setup`, and their commands can be served in between; a warm
  start panicked 5 of 5. A panic is logged to `~/.reviewglass/panic.log`: read it first.
- **A packaged app's children write to an AppData nobody else can see** (18.9.2026).
  The first live diff passed every agent-side check and showed the owner nothing.
