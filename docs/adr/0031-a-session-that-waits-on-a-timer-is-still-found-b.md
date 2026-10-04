# ADR-0031: A session that waits on a timer is still found by its pane's title, and a chosen session with no recent edits shows its project's uncommitted changes

**Status:** Accepted
**Atlas id:** `adr.rg.031`
**Links:** `rg.diff-service` (Diff service), `rg.session-source` (Session source (trait)), `rg.transcripts` (Claude Code transcripts), `rg.panel-window` (Panel (the dock's drawer)), `reviewglass`

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

The owner's finding of 28.9.2026: a session that only waits on a timer (Somnus, a wake-up every few hours) writes nothing for longer than the live table's 10-minute TTL, so its Code pane's title matched no live session and the diff window did not switch to it; and it had no edits in the hook's hour to show. Three ways were laid out: (a) match a pane's title against every session's title, not only the live ones; (b) with no recent edits, show the session's project's uncommitted changes from git; (c) a 'waiting on a timer' state in the Sessions table. On 4.10.2026 the owner chose (a) and (b) now; (c) waits, since the wake time is inside a tool call and needs a privacy decision of its own.

## Decision

(a) A pane's title that matches no live session is looked for in the transcripts written in the last three days: the newest session whose title record carries it (exactly, else ignoring case) is chosen, and the sighting says the session is not live. Only the title and the session state the transcript reader already reads are used; the files read are cached by their modification time, and a title already answered is not looked for again on every read. A session's working folder is found the same way by its id, from the transcript named by it - a UUID only, never a path. (b) When the chosen session - by Follow, by the frame or by hand - has no edit in the Diff tab's list, the tab and the diff window show its project's uncommitted changes instead: git status in the repository of the session's working folder (the live table's, else its transcript's), changed, staged, untracked and renamed files, each diffed as an edit is, the secret denylist first, at most 40 files, the newest first, the rest counted. The list says it is the project's changes since the last commit, not the session's latest edit, and marks no line as fresh; it is read again every ten seconds while shown, off the main thread, and the whole-file view is offered on it. Nothing is written.

## Consequences

The diff window follows a waiting session's pane like any other and shows what is pending in its project. A session whose working folder is not in a repository, no longer exists or is not known says so in place of the list. The project's changes may include other sessions' or the user's own edits in the same repository; the header says what they are. A session that has not written for more than three days is not found by its title. (c) remains open.
