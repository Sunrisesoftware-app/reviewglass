# ReviewGlass CHANGELOG

Newest first. One entry per session; a session that ships several distinct things gets
sub-entries. What changed and *why*, with what was measured, so a later reader can tell
a decision from a habit.

## Session 6 (continued): the startup crash, read from its own log and fixed — 23.9.2026 (0.1.0)

The crash seen during the Follow check, reproduced with the owner's leave. With the
panic log in place (`~/.reviewglass/panic.log`, #27), five launches right after a quit
died within a second, all five logging on the main thread "state() called before
manage() for reviewglass_lib::config::Store" — four of them before the build agent's
probe had made a single call, so the probe was not the cause.

- **The cause.** Tauri creates the windows declared in `tauri.conf.json` before
  `setup` runs, and creating a WebView2 pumps messages while it waits for the
  controller. A page already loaded — fast when WebView2 is warm, as it is right after
  a quit — has its commands served in that gap; the state was managed in `setup`, so a
  command reaching the config through `app.state()` panicked. Possible since the
  first build; a warm start made it near certain.
- **The fix.** Every piece of state is managed on the builder, before the app and any
  window exist. The config directory is computed as Tauri's `app_config_dir` does (the
  config directory joined with the bundle identifier) from the context generated
  before the builder; the file is the same.

Measured on the release build of the fix, the same five warm launches with the probe
calling the dock's state command from the first moment the page existed: all five
stayed up, each answering about 3 500 calls (the first at 0.65–0.85 s after launch),
no line added to the panic log. A rule in CLAUDE.md and the story in LESSONS.md.

## Session 6 (continued): Follow chooses the session — adr.rg.022 — 23.9.2026 (0.1.0)

The owner's wish of 20.9.2026, next on the list they agreed: while the glass follows,
the Diff tab should show the session of the column the glass is reading. Pixels
cannot say which session a column is; the desktop app's accessibility tree can.

- **The decision first** (adr.rg.022, Atlas #344, amended in #345): UI Automation is a
  read source for one thing only, which session is under the cursor. What is read:
  the element under the cursor and its ancestors up to the Code pane, and the names
  on the pane's header row until one is "<title>, rename session"; for a session the
  desktop app has opened in a window of its own, the page's name. Only windows of the
  desktop app's process (claude.exe) are read at all. Nothing below the header row,
  nothing written, nothing kept but the current pane's rectangle and title.
- **Desktop sessions are named by their titles.** Each Desktop transcript carries a
  `custom-title` record every few lines — the title the desktop app shows, session
  state and not conversation. The transcript reader takes the newest (leniently) and
  keeps a title once seen, since a long transcript may hold one only beyond the tail.
  The Sessions table shows Desktop sessions by those titles instead of folder names.
- **follow_session.rs.** A UI Automation client on a thread of its own (COM,
  multithreaded apartment). While the glass is shown in Follow, not held, with the
  switch on: the window under the cursor is checked first (the desktop app's, or
  nothing is asked); a read happens only when the cursor leaves the pane it was last
  found in, plus a re-check every 4 s; a point that found nothing is not asked again
  within 40 px for 1.5 s. The title is matched to a session's name (exactly, then by
  case) and sent to the dock, which makes it the choice, marked as Follow's. A miss
  leaves the choice as it was; a choice by hand stands until the pane changes.
- **The affordance.** A checkbox on the Sessions tab, "Follow chooses the session
  under the glass" (on by default), with a line saying what Follow sees: the title
  it is on, a title that is not in the table, or why it is idle. The Diff tab's
  filter line says "chosen by Follow".

Measured without the mouse, against the owner's running desktop app through the
ignored diagnostic `live_pane_titles` (read-only, points given, the cursor never
moved): the main window's three Code panes read their titles in 4.5–15 ms each
("Luviamo projektin tila ja kirjautumisvirhe", "Suno v6 korjaukset jatkuu", "Atlas
kehitys ja kirjautumisongelman ratkaisu"), every point marked as the desktop app's
window. The first query of a run found nothing: it is what wakes Chromium's
accessibility tree. On the same screen the owner had three sessions open in windows
of their own, whose pages are named with the session titles — the reason for the
amendment. clippy clean, 114 tests (9 new: the header title, the page title, the
pane class token, the header points, title matching, the claude.exe image, the
known-pane and miss holds, the transcript title). svelte-check 0 errors, 0 warnings.

Observed on the release build (main `23b2d01`, rebuilt 15:15, launched from a plain
PowerShell; the cursor moved with `SetCursorPos` with the owner's leave and put back
afterwards; the drawer read over CDP). The Sessions table named every live Desktop
session by its title. By the time of the check the owner's session windows stood over
the main window, so the three points landed on windows of their own — the amended
path:

- Three moves, three reads (12.0, 4.5, 4.1 ms), each choosing its session: "Virus-
  seuranta näkymät ja tiedotteet", then "Tilastosilta Projektin tilanteen
  tarkistus", then Virus-seuranta again; the row bold, the note "… is chosen by
  Follow", the switch's line naming the title.
- A choice by hand (another row) held through a re-check 5 s later with the cursor
  still in the same window: one read, no new choice.
- Over the dock: no read, the choice held. Back to a point in the same window: no
  read (the known rectangle held).
- Switch off: moving to another window changed nothing ("off: the choice is yours
  alone"). Switch on: the window under the cursor was chosen at once.
- Cost with the cursor still for 10 s: no UI Automation read; ReviewGlass used 1.14 s
  of CPU (the glass rendering in Follow) against 0.22 s with the glass off. The
  desktop app used 10.5 s with Follow on and 15.7 s with the glass off: its load is
  the owner's running sessions, and Follow's share is not separable from it.

One crash on the way: the first launch of the rebuilt exe died about a second after
start (Windows Error Reporting: 0xc0000409 in reviewglass.exe, the release build's
panic-abort) while the build agent's readiness probe was already calling the dock's
state command; the second launch stayed up. The release build's stderr goes nowhere,
so nothing said why: a panic now leaves a line in `~/.reviewglass/panic.log`, and
the cause is to be read from there.

## Session 6: the latest edit in a colour of its own — 23.9.2026 (0.1.0)

The owner had used the drawer, the handle and the session choice the day before:
"the direction is right; the Diff view is fine". The wish: see the code that is new
in its own window, or in the diff in a different colour — ReviewGlass's own effect,
nothing written into the files.

In a tracked file the Diff tab measures against HEAD, so the latest edit and every
earlier uncommitted change looked the same. What was missing was a memory of the file
as it stood at the previous edit.

- **The diff loop remembers every listed file.** The working copy is kept in memory
  for every file in the list that reads as text within the 512 KB cap — tracked files
  too, no longer only those git has no baseline for — and dropped with the view. Each
  view carries `fresh`: the lines the latest edit brought, 1-based in the working
  copy, measured against the remembered copy (`similar`, 250 ms timeout, line endings
  normalised). At the first sighting every line the diff adds is fresh, and the view
  says so (`fresh_from_previous`).
- **A highlighter.** The Diff tab paints the fresh lines yellow with a darker bar at
  the left — in the Hunks view over diff2html's rows by their new-side number, in the
  File view over the green tint — and older uncommitted additions stay diff-green. A
  legend under the header names both colours and counts the new lines.
- **New: the new code alone.** A third view beside Hunks and File shows only the
  fresh lines, as they stand now, in runs with their line numbers and "lines a–b
  unchanged" between them; a named empty state when the latest edit only removed
  lines. This is the "own window" of the wish, inside the drawer: the drawer is one
  window (adr.rg.020) and can now be made as large as the work area (adr.rg.021).
- **File and New pin their file.** With no file clicked, the Diff tab follows the
  newest edit; an edit to another file used to take an open File view away. That was
  the one-time fall-back to the hunks seen on 20.9.2026: the build agent's own edit
  to `Diff.svelte` landed between two writes of the watched script.

Measured: cargo clippy clean, 105 tests (4 new: fresh lines by content across line
endings; a tracked file's latest edit apart from older uncommitted work; fresh lines
outside a repository; the state remembering a tracked file between edits);
svelte-check 0 errors, 0 warnings.

Observed on the release build (main `6ceb181`, the shortcut's exe rebuilt 14:29,
launched from a plain PowerShell with the owner's leave, driven over CDP, no mouse): a
throwaway repository with one committed file, edited twice through this session's own
Edit tool so the hook fired as for any agent edit.

- First edit (four lines added, one changed): the view `changed`, fresh 1, 2, 3, 5,
  `fresh_from_previous` false; the Hunks view painted exactly those four rows
  (rgb 255 241 168), the legend "new — the first edit ReviewGlass saw of this file (4
  lines)"; File the same four lines; New the three runs with "lines 4–4 unchanged".
- Second edit (three lines): with New open, it re-read in place and showed lines 9–11
  alone, "new in the latest edit (3 lines)". The HEAD diff held seven added lines; the
  three fresh rows were yellow and the first edit's four green (rgb 221 255 221), the
  legend naming both; File the same split.
- With File open on that file, a write to another file put the other file at the top
  of the list and left the view where it was: File, the same file, lines 9–11 yellow.

The owner decided the same day that P6 (the explain backend, local or remote with the
user's own key) comes before P5 (cache and PR panels), since the hawk eye stands on
P6's backend.

## Session 5 (continued): the owner's proposal — a drawer sized by hand, a chosen session, and the column's session read from the pane — 20.9.2026 (0.1.0)

The owner followed a script being written in the Diff tab and wrote up what the tool
should do next: the code showed only the left edge of each line and the drawer could
not be enlarged; with five sessions live, one should be choosable on the Sessions tab
and the Diff tab should show that one alone; Follow should choose the session of the
column the glass is on; and, later, a "hawk eye" — an AI watcher through a key of
one's own — should read the columns and the code as they change and raise what looks
wrong. "Now the benefits this tool was started for begin to show."

- **The drawer is sized by hand from its free corner** (adr.rg.021, Atlas #319,
  supersedes adr.rg.020's "a setting, not a drag"). A visible handle (◢ ◣ ◥ ◤, named)
  in the corner diagonally opposite the snapped one; dragging it resizes the window
  with the operating system's own resize, the snapped corner staying put; the size
  persists as `dock.drawer_width` / `dock.drawer_height` and is what the drawer opens
  to next time. Resizable only while open, between 640 × 344 and the work area;
  closed, the strip cannot be resized. The Diff tab's file list stops at 260 px, so a
  wider drawer goes to the code.
- **A chosen session filters the Diff tab.** The Sessions table's first column is a
  radio: the header's for all sessions, a row's for that one, the chosen row bold in
  the accent and the note under the table saying what the choice means. The choice is
  module state shared with the Diff tab (not persisted: sessions come and go); the
  Diff tab shows the chosen session's views alone with a line saying so and "show all
  sessions" one click away, names the session when it has no edits, and the Sessions
  tab names a chosen session that has left the table with the same way out. Nothing
  clears the choice but the user.
- **Follow → the column's session: measured, not built.** Pixels cannot say which
  session a column belongs to, so the question was whether the desktop app's
  accessibility tree can. A read-only UI Automation probe (PowerShell, no mouse) says
  yes: the Code panes are groups named "Primary pane" / "Secondary pane" (class
  `dframe-pane`) with real bounding rectangles, and a point on a pane's header row
  hits a button named "<session title>, rename session", with "<project>, <branch>"
  beside it. Structure, never content — adr.rg.006 stands — and only the header row
  would ever be read, since the chat's text is exposed too. Walking children from the
  window returns nothing; reading by points works. To be decided as an ADR (UI
  Automation as a read source) before it is built as a Rust read at (column centre,
  pane top + 24 px) when Follow's pane changes.
- **Hawk eye: a vision, recorded.** It fits adr.rg.002's pluggable backend (local or
  remote, one's own key), but a continuous send of every change is a different
  privacy line from today's "one hunk, on demand, payload disclosed" and needs its own
  decision. Into spec v3's roadmap as a vision item, not built.

Observed on the release build (main `3d3e189`, rebuilt 20:57, launched from a plain
PowerShell, driven over CDP; the drag itself is the OS's, so the size was changed
through the window command and the rest of the chain read): the handle present and
named in the free corner (◣, `nesw-resize`, for a top-right dock); a resize to 900 ×
760 remembered as 900 / 716 in the config and the window's right edge exactly where
it was (x 1652 + 900 = the work area's edge less the margin); the Diff tab's grid at
"260px 598px"; closed, the strip 470 × 44 in the same corner; reopened, 900 × 760
again. Three Desktop sessions in the table; choosing this session made its row bold
(weight 700), the note said so, the Diff tab read "Only project-docs-status-check
(1 of 1) · show all sessions" and listed exactly that session's file; the link
restored all; choosing a session with no edits gave the named empty state with the
same way out. The size was restored to 640 × 620 and the drawer closed afterwards.

## Session 5: the repository was already public; P4b, the whole file around a hunk; the previous edit as the baseline where git has none — 20.9.2026 (0.1.0)

The session opened from the cold-start list and found one thing nobody had picked up:
the Atlas project thread's line of 17.9. that the repository is public by the owner's
decision (name probes that day: web search, GitHub and npm empty; crates.io refused
the probe), while README, CLAUDE.md, BUILD_INFO and adr.rg.008 still said "private
until P8". Fixed first (PR #21, docs only): the name check still gates P8 — the
installer and the bundle identifier — not the source. adr.rg.008 revised in the model
(Atlas #318, worker deployed), rendered to `docs/adr/`; the thread line marked
handled. The rebase merge rewrote the commit ids, so the ADR's consequences cite a
commit that landed as `9b3df50`; to be corrected with the next model change.

The owner's look at the dock with its drawer, session 4's closing item: everything
works. The one observation was the Diff tab's empty area for a file outside any
repository, and the wish to watch such a file — a script being written — come into
being, its code live.

- **P4b: the whole file around a hunk, read-only.** `panel_file_view(path)`
  (`src-tauri/src/diff/file.rs`) reads the working copy once and hands it over with
  the last diff's marks: the lines it added, the positions it removed lines before,
  the hunks by their new-side start. Only a path the diff loop has listed since start
  can be opened, so the panel's IPC cannot be turned into a file reader; the
  secret-file denylist, the 512 KB cap and the binary check apply as to a new file's
  diff. The Diff tab's header gains a Hunks | File toggle; File opens at the first
  change, a hunk header in the rendered diff opens the file at that hunk (click, or
  Enter once tabbed to), ‹ › walk the changes with a count, added lines tinted, a red
  cut mark where lines were removed, the current change's line number in the accent.
  Picking another file returns to its hunks. A `diff:update` re-reads an open file,
  so a file being written updates in place.
- **The previous edit is the baseline where git has none.** A file outside any
  repository, or not yet tracked, used to show "not inside a git repository" with
  nothing, or the whole file as new on every edit. The diff loop now remembers the
  working copy of such a file at its previous edit (in memory, text within the cap,
  one per listed path, dropped with the view) and shows what the latest edit brought
  against it, as a unified diff in git's form (the `similar` crate, Apache-2.0); the
  first sighting shows the whole file as new. `DiffView.baseline` says which — `head`,
  `last-edit`, `whole-file` — and the view's header says it in words; a write of the
  same content says "no change since the previous edit". Nothing is written for this
  either.

Measured: cargo clippy clean, 101 tests (7 new: marks by new-side line, removals-only
and the synthesised addition, the working copy with its marks, refusals reading
nothing, the previous-edit baseline outside a repository, an untracked file's second
edit measured from its first, the state remembering between edits); svelte-check 0
errors, 0 warnings.

Observed on the release build (main `a42aefa`; the shortcut's exe rebuilt 20.9.2026
20:24, launched from a plain PowerShell, driven over CDP with the owner's leave while
they were away from the machine): a script written under the profile root, outside
any repository, seven times.

- The first write listed the file 0.11 s after the hook's clock, as `outside git,
  +21`, the header saying "the whole file, new", 21 added lines rendered.
- The second write showed 0.24 s after the hook as `+13 −3`, "since the previous
  edit", two hunks rendered with 13 added, 3 removed and 15 context lines; the same
  figures from the core (`panel_file_view`: 13 added lines, cuts before lines 8 and
  25, hunks at 2 and 13). A path never edited is refused: `not-offered`, "not a file
  the agent has edited since ReviewGlass started".
- The File toggle opened the working copy at its first change: 31 lines, the 13
  added lines tinted, the two cut marks in red, "1/2"; › went to line 13 and ‹ back
  to 2.
- With the File view open, the fourth and fifth writes re-read it in place (33 → 34 →
  37 lines, the marks following, the list's counts too), the drawer's DOM not
  re-mounted. After the third write the view had fallen back to the hunks once; that
  did not reproduce in two further tries and stays an open observation.
- The hunk headers were not wired: diff2html marks the header's two cells `d2h-info`
  and the inner div only `d2h-code-line`, so the effect matched nothing. Fixed
  (`a42aefa`), rebuilt, seen: two header rows with the title, a tab stop and the
  pointer; a click on the second opened the file at line 25 ("2/2", scrolled 163 px);
  back on Hunks the rows were wired again; Enter on the first row opened the file at
  line 5 ("1/2").

A synthetic DOM click does not open the drawer, because the strip's button acts on
pointer-down; a real press does, and the core's `dock_drawer` command served from CDP.

## Session 4, closing: a visible way out — 19.9.2026 (0.1.0)

The owner's last observation of the session: the dock has no quit button, and the
drawer's ✕ closes only the drawer. Quitting lived in the dock's right-click menu and in
the tray — a mechanism without its affordance, against this repo's own rule. The strip
now ends in a ✕ that quits ReviewGlass for real (the glass, the drawer, the process),
red on hover so it is not mistaken for the drawer's toggle beside it. The menus and the
tray keep their quit items.

Session 4 closes here: the hook spike, P4 built and observed, the spool moved
(adr.rg.019), P2's exit criterion met, the drawer (adr.rg.020). Next session starts
from the owner's look at the dock with its drawer, then P4b and spec v3.

## Session 4 (continued): the panel is the dock's drawer, in the dock's own window — 19.9.2026 (0.1.0)

The owner's verdict on the Diff tab was about the box it came in: the panel was a loose
conventional window, opened beside the dock but not of it, and no real UX design had
been done for it. The panel — Sessions, Diff, Settings — should be attached to the
dock, open from it, move with it, never be two separate windows that can be apart.
Decided (adr.rg.020, Atlas #317): one window that grows, 640 px wide open, the tabs in
the drawer.

- **The dock window is the panel's window.** Closed, the 470×44 strip as before. Open,
  the same window is 640 × (44 + 620): the drawer unfolds below the strip in a top
  corner and above it in a bottom one (the page lays itself out `column-reverse`
  there, so the strip is always the row nearest the screen's edge), snapped to the
  same corner. The Rust side positions the window from the size it is about to have
  before resizing, so a bottom-corner drawer never spends a frame below the screen. A
  drag with the drawer open moves the whole thing, and a snap re-lays it out.
- **The strip's ▤ button opens and closes the drawer** (it reads ▴ or ▾ while open,
  pointing the way it closes); the tray's item and left click, the glass's bar button
  and its menu all open the same drawer — there is no other window. The tabs sit in
  the drawer's first row; the last tab is remembered (`dock.drawer_tab`), the drawer
  starts closed at every start. Its height is a setting (`dock.drawer_height`, 620),
  not a drag.
- **Four windows, not five.** The panel window, its close-to-hide handling, its
  size-on-resize command and `place_panel` are gone; the panel's components moved to
  `src/lib/panel/` and mount inside the dock route; the diff loop's update event goes
  to the dock. The Diff tab's list and hunk are sized for 640 px and scroll inside the
  drawer's height. The usage poll runs every 2 s while the drawer is open and every
  5 s while it is a strip.
- The drawer is excluded from capture with the dock, so it can never appear inside
  the glass.

Verified on the release build, launched from a plain PowerShell (adr.rg.019's rule)
and driven over CDP without touching the cursor: no panel window exists; the strip is
470×44 at (8,8) with no drawer in the page; opening gives 640×664 at (8,8) with the
tabs Sessions · Diff · Settings and the strip as the top row; the Diff tab remembered;
a hook event appears in the drawer's Diff tab with the hunk rendered; moved to
(1500,900) and snapped, the open window sits bottom-right with its bottom on the work
area's edge and the strip as the bottom row; closed, the strip is 470×44 in that
corner; `panel_show` (the glass's and the tray's route) opens the drawer on the
remembered tab.

## Session 4 (continued): P2's exit criterion met — five concurrent CLI sessions — 19.9.2026 (0.1.0)

Open since 11.9.: "five concurrent sessions in the panel with correct shared-vs-attributed
semantics". The owner asked whether the build agent could run the CLI sessions itself.
It can: a Claude Code CLI started under a pseudo-terminal (Windows ConPTY, `pywinpty`)
believes it is interactive, renders its status line and runs the collector — print mode
never does (4.1). No window, no focus taken; the owner's screen and mouse untouched.

- **Method** (`scratchpad/cli_sessions.py`, kept out of the repo): five `claude --model
  haiku` sessions in five throwaway directories, each driven through its ConPTY — the
  trust dialog answered (its default is "No, exit"; the marker is moved to "Yes" and
  checked before Enter, since a redraw can put it back), one prompt ("reply with one
  word") sent once the input box is idle, the reply awaited by the collector's own
  rendered line (`ctx 23% · 5h 9%`) appearing on the session's screen. About 0.02 $ of
  Haiku per session.
- **Measured, 19.9.2026:** five records in `~/.reviewglass/spool/sessions`, one per
  session, each carrying `rate_limits` with the same figures — 5-hour 9 %, 7-day 55 %,
  the same `resets_at` — and its own cost (0.016–0.026 $) and context (23 %). The
  app's own reader (`session::read_all`, the code the panel renders, run through the
  diagnostic test `live_reading` while the sessions were alive) listed all five as
  `Cli / Both` with cost and context per session and the quota flag set, beside the
  agent's own Desktop session and the owner's second one through the transcript
  channel, and the previous runs' sessions still inside the 10-minute TTL as
  transcript-only. Shared gauge, per-session attribution, never blended.
- The first three attempts failed for reasons worth a line each: the prompt sent
  before the input box existed (no reply, a first-call record with no `rate_limits`);
  a `\r` typed into a heredoc arriving as a real carriage return; one session's trust
  dialog answered while it was redrawing.

The dock's gauge shows the 5-hour figure again; the panel's Sessions tab lists the
sessions while their records are younger than ten minutes.

## Session 4 (continued): the spool moves to the profile root — the owner saw nothing, and why — 18.9.2026 (0.1.0)

The owner opened the Diff tab and asked for an edit; nothing appeared. The hook had
fired — the event file was there, from the build agent's shell — and the app read an
empty directory at the same path.

- **The cause** (adr.rg.019). The Claude desktop app is a packaged (MSIX) application,
  `Claude_pzs8sxrjxfjjc`, and Windows virtualises AppData for a packaged process and
  every child it starts. The Desktop Code tab's sessions, the hooks and collectors they
  run, and the build agent's own shell all wrote `%APPDATA%\ReviewGlass` into
  `%LOCALAPPDATA%\Packages\Claude_…\LocalCache\Roaming`, visible only with the package
  identity; the app started from its shortcut read the real `Roaming`, where the
  directory did not exist — `hook_installed: false`, `collector_installed: false`, and
  every event unconsumed. The app's own config directory escaped because the real app
  had created it first: virtualisation merges what exists and redirects what is new.
  The profile root is not virtualised (`~/.claude` is real for everyone), so the spool
  moved to **`~/.reviewglass/spool`**, beside the collectors' binaries, in the one
  function the app and both collectors share (`spool.rs`). Spec 6.1's "plain files
  under the user profile" is now literally true.
- **Measured across the boundary**, the way the owner launches the app (a plain
  PowerShell `Start-Process` of the release exe, remote debugging on): `hook_installed`
  true; one `Edit` from the build agent's Desktop session — the real path, the hook run
  by Claude Code — appeared in the Diff tab within the first poll (0.06 s after the
  edit's tool call returned): `gauge.rs`, `changed`, `+10 −3`, session id the agent's
  own, the hunk rendered; the event consumed. Before the move, the same launch showed
  nothing for four events in a row. `collector_installed` is now false until a CLI
  session runs the reinstalled statusLine collector, which is the honest state: the
  P0 spike's session record had lived in the package cache all along, visible to the
  agent and never to a plain process, unnoticed because no CLI session had been run
  outside the desktop app since.
- **A rule for every check from here on:** the app under test is launched the way the
  owner launches it (Explorer or a plain PowerShell), never from the agent's shell,
  which is itself a child of the packaged app. A check that has passed only from the
  agent's shell has proved the mechanism, not the delivery.
- Both collectors rebuilt and reinstalled under `~/.reviewglass/bin`; the spool data
  in the package cache is abandoned. The installer (P7) inherits the rule: nothing
  shared lives under AppData.

## Session 4 (continued): P4 built — the events reader, the diff service, the Diff tab — 18.9.2026 (0.1.0)

Built in the background while the owner had the machine, so nothing here has been seen
on screen yet; the exit criterion (an agent edit in the panel within ~1 s) is to be
observed, as P2's was.

- **The events reader** (`src-tauri/src/diff/events.rs`, the events half of
  `rg.spool-watcher`): consumes `spool/events/*.json` oldest first into `ChangeEvent`s,
  field by field (adr.rg.011; only `ts` is required), retries a file that will not
  parse once and then leaves it to the collector's prune, deletes what it has read, and
  consumes events from before the app started without reporting them — the live diff
  is live; what happened before it watched is git's to tell.
- **The diff service** (`src-tauri/src/diff/mod.rs`, `rg.diff-service`): the denylist
  first (`.env*`, `*.pem`, `*.key`, `id_*`, `*.tfvars` by default, in the config as
  `diff.denylist`, matched against the file name, case-insensitively) — a denied path
  is listed as touched but never handed to git; then `git rev-parse --show-toplevel`
  for the repository, `ls-files --error-unmatch` for tracked-or-not, `diff HEAD` scoped
  to the path for the unified text and `--numstat` for the counts. A new file is shown
  as the addition it is, synthesised in unified form (size-capped at 512 KB, binary
  detected); a path outside any repository, a deleted file, a binary, a file identical
  to HEAD and a git error each get their own status and a reason in the user's terms.
  Git runs without a console window (`CREATE_NO_WINDOW`; this is a GUI process). The
  loop runs on its own thread every 300 ms; every event for one path within a tick
  becomes a single diff, which is the debounce; the views (30 at most, one per path,
  newest first) go to the panel with a `diff:update` event.
- **The Diff tab** (`src/routes/panel/Diff.svelte`): the files touched since start,
  newest first, each with `+N −M` or its status word, the tool and how long ago; the
  selected file's diff rendered by **diff2html** (MIT; its dependencies `diff`,
  BSD-3, and `@profoundlogic/hogan`, Apache-2.0), not hand-rolled (spec 6.2). Three
  empty states, each with its reason: the hook collector has never run (its events
  directory does not exist), no edit since ReviewGlass started, or a file git cannot
  show (the reason from the service).
- Eleven new tests, four of them against a throwaway git repository: a tracked edit
  gives `changed` with +2 −1 and the hunk text; a new file gives `untracked` with the
  synthesised addition; unchanged, missing, denied (the file exists and git is never
  asked), binary and outside-a-repository each give their status. 94 tests in all;
  the frontend bundle builds with the renderer.

- **Observed on the release build** (`0.1.0 31dc6d0`, the shortcut's exe), once, with
  the owner away from the mouse: the panel opened on the Diff tab shows the named
  empty state; a one-line edit to a committed file in a throwaway repository, with the
  collector run on the PostToolUse payload an agent's edit produces, appeared in the
  panel **0.23 s** after the hook ran — `probe.txt`, `changed`, `+1 −1` (the file's
  only line had no trailing newline, so the append changed it rather than adding to
  it), the unified text carrying the line, diff2html rendering it as one deleted and
  one inserted line; a `.env` written next to it was listed as `denied` with no text.
  The exit criterion (an agent edit in the panel within ~1 s) is met on the
  synthesised path; the owner's own edit from a live session is the same path with
  the hook fired by Claude Code, which the spike already measured.

## Session 4: the hook spike — PostToolUse fires on both surfaces — 18.9.2026 (0.1.0)

P4 (live diff) opens with the measurement the topology had assumed and P0 had never
made: section 6.1 put the PostToolUse hook under "CLI sessions only", next to the
statusLine, but 4.1 measured only the statusLine and the native diff panel. The primary
surface is the Desktop Code tab (adr.rg.003), so the answer decides P4's shape.

- **`hook-collector` built** (`src-tauri/src/bin/hook.rs`, `reviewglass-hook.exe`,
  installed beside the statusLine collector under `~/.reviewglass/bin`). A native binary
  (adr.rg.010): reads the PostToolUse JSON from stdin field by field (adr.rg.011), writes
  `spool/events/<ts>-<tool_use_id>.json` with the session id, the transcript path, the
  cwd, the tool and the file path — **never the edit's content** (`old_string`,
  `new_string`, a Write's `content` stay in the payload and go nowhere) — exits 0 and
  prints nothing. An unparseable payload still leaves an event with the byte count and
  the reason, so "ran and failed" is never mistaken for "never ran". It prunes events
  older than an hour on every run, so a spool nobody reads stays a handful of files.
  Configured in `~/.claude/settings.json` (backup `settings.json.bak-reviewglass-p4`)
  with the matcher `Edit|Write|MultiEdit|NotebookEdit` and a five-second timeout.
- **Measured, the same way 4.1 records its rows:**

| Question | Surface | Result | Date |
|---|---|---|---|
| Does `PostToolUse` fire? | Desktop Code tab | **Yes.** Claude Code 2.1.274, session `entrypoint: claude-desktop`. One `Write` from the build agent's own session produced `spool/events/1789740911042-toolu_01HQ….json` with the session id, the transcript path, the cwd, `tool: Write` and the file path, 1 501 bytes read from stdin. The session had been running when the hook was configured; it fired without a restart. | 2026-09-18 |
| Does `PostToolUse` fire? | Terminal CLI (print mode, `claude -p`) | **Yes.** Claude Code 2.1.268, `entrypoint: sdk-cli`. One `Write` produced its event the same way. The statusLine did not run in that session, as 4.1 already said of print mode. | 2026-09-18 |

- **Consequences.** The hook is the live diff's primary trigger on *both* surfaces;
  the "CLI sessions only" bracket in 6.1 was a diagram's assumption, not a
  measurement, and moves to "both surfaces" in spec v3. The transcript's tool records
  are not needed as a trigger, so the rule that only session state is ever read from a
  transcript stands untouched. The filesystem watcher (5.4) stays what it was: the
  agent-agnostic fallback for v2. A third `entrypoint` value exists, `sdk-cli`, which
  the transcript reader maps to `Unknown` by design (a new surface is exactly what a
  guess would get wrong); how the panel treats such sessions is a P4 question.
- **Planned extension, from the owner:** after the hunk view, a read-only view of the
  whole file around the change — the diff tab shows the hunks with context (the spec's
  `DiffView`), and a "whole file" toggle opens the file at the hunk. A read surface
  still; nothing is written. Recorded in the roadmap for spec v3 as P4b.

Next in P4: `spool-watcher` grows an events reader (`ChangeEvent`), `diff-service`
runs `git diff` scoped to the path with the debounce, the untracked case and the
secret-file denylist, and the Diff tab renders `DiffView` with an existing diff
renderer. Exit criterion unchanged: an agent edit appears in the panel within ~1 s.

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
