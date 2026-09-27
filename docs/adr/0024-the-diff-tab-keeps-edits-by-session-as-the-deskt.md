# ADR-0024: The Diff tab keeps edits by session, as the desktop app does: each session's own files, the hour before the start, an agent's own files set aside

**Status:** Accepted
**Atlas id:** `adr.rg.024`
**Links:** `rg.diff-service` (Diff service), `rg.panel-window` (Panel (the dock's drawer)), `rg.spool-watcher` (Spool watcher), `rg.hook-collector` (PostToolUse hook collector), `rg.session-source` (Session source (trait))

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

The Diff tab listed the 30 newest edited files across every session, newest first, from the moment ReviewGlass started, each by its path inside its repository. On 27.9.2026 the owner looked for lukitus.ts, which the desktop app's 'Edited 14 files' card of its session showed, and could not find it among the thirty: four sessions had written 50 distinct files in the 24 minutes after the start, and the file had left the list about nine minutes after its edit. Agents' own scratch and memory files took slots too, and a path relative to its repository named neither the project nor the session. The owner works in several Desktop sessions at once and reads the diff one session at a time.

## Decision

The list is kept by session: for each session up to 100 project files and up to 100 set-aside files, newest first, 400 in all; the working copies remembered for the fresh-line mark are held to 64 MB, the newest views' first. Change events stay in the spool for the hour the hook keeps them and are read once, not deleted, so a restarted app picks up the last hour's edits again (the hook's prune age and the app's look-back are one constant). A file under an agent's own working folders - the temp scratchpad (%TEMP%\claude) or ~/.claude - is set aside: listed in a group of its own at the end, closed until opened, and never shown by itself when nothing is picked. The Diff tab groups the list under each session's name - the Sessions tab's while the session runs, else the title from its transcript (a subagent's edit counts as its parent's) or its working folder's name - the session with the newest edit first; each file is named first, with its project and folder under it, and a worktree is named by the repository it belongs to.

## Consequences

A quiet session's edits stay listed however busy another session is. The first view after a restart can hold edits from before it, measured against the file as it stands; their fresh-line mark is a first sighting's. Memory grows with the list, bounded by the caps; a working copy dropped for the budget costs only the next edit's fresh-line precision. What the desktop app's card counts and ReviewGlass does not see - edits older than an hour before the start, changes made by a shell command rather than an edit tool - stays git's to tell: the hook fires for Edit, Write, MultiEdit and NotebookEdit only. Session names come from session state (the custom-title record, the working folder), never from the conversation. Deleting events on read, as the reader first did, left a restarted app an empty list while four sessions worked (the live check of 27.9.2026); the events directory now holds up to an hour of small files.
