# ADR-0022: UI Automation tells which session a column belongs to: the pane header's title, nothing below it

**Status:** Accepted
**Atlas id:** `adr.rg.022`
**Links:** `rg.session-source` (Session source (trait)), `rg.glass-window` (Glass window), `rg.dock-window` (Dock), `reviewglass`

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

The owner wants Follow to choose the session of the column the glass is reading, so the Diff tab follows the eye (20.9.2026). Pixels cannot say which session a column belongs to, and the glass reads structure, never content (adr.rg.006, adr.rg.017). A read-only probe on 20.9.2026 found that the Claude desktop app's accessibility tree names each Code pane ('Primary pane', 'Secondary pane', class dframe-pane) with its bounding rectangle, and that a point on a pane's header row hits a button named '<session title>, rename session'. Each Desktop session's transcript carries the same title as a custom-title record, which is session state, not conversation. The same accessibility tree also exposes the chat's text.

## Decision

While the glass is shown in Follow and the owner's switch is on, a background thread reads through UI Automation only: the element under the cursor and its ancestors up to the enclosing Code pane (class name and bounding rectangle), and the names of elements at a few points on that pane's header row until one is named '..., rename session'. The title before ', rename session' is matched exactly to a session's title, read from its transcript's custom-title record, which also becomes the session's name in the Sessions table; the matched session becomes the choice that filters the Diff tab. No element below the header row is read, nothing is kept beyond the current pane's rectangle and title, and nothing is written into the desktop app. A read happens only when the cursor leaves the known pane, plus a re-check every few seconds; never in Lens or Still, never while the glass is hidden. The switch is visible on the Sessions tab; a choice made by hand stands until Follow's column changes to another session.

## Consequences

The app gains the Windows UI Automation client (the windows crate's accessibility and COM features) on a thread of its own. Asking Chromium for its accessibility tree switches the desktop app's renderer accessibility on while Follow runs, a CPU and memory cost in that app, which is measured and recorded. Desktop sessions show their titles in the Sessions table instead of the folder name. A session whose title is not in its transcript's tail, and a CLI session, which has no Desktop pane, cannot be matched: the choice is left as it was and the Sessions tab says which title Follow saw. A change of the pane's class or the button's name in a future desktop app turns every read into 'no match', never into a wrong choice. For anything else (foreground-app profiles, session state in the dock) UI Automation stays an experiment until measured.
