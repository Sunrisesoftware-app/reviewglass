# ADR-0028: The diff window: the locked pane's session's diff in a frameless window of its own, opened from the glass, first over the neighbouring pane, then where the user put it

**Status:** Accepted
**Atlas id:** `adr.rg.028`
**Links:** `rg.glass-window` (Glass window), `rg.panel-window` (Panel (the dock's drawer)), `rg.diff-service` (Diff service), `rg.config-store` (Config store), `reviewglass`

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

On 27.9.2026 the owner asked that the locked pane's diff open in a frameless window of its own with one command, to the right of the pane at the neighbouring pane's size - to the left when the pane is at the screen's right edge - large enough for the code and its changes to read, and remembering its place once moved. The drawer's Diff tab (adr.rg.020, adr.rg.024) stays where the dock is. The glass's own placement over the neighbour failed the same day (adr.rg.027, amended) because a magnifier the neighbour's size magnifies nothing; a diff window is not a magnifier: it shows text at its own size, so a pane's size reads. On 28.9.2026 the owner chose: a button on the glass's bar and Ctrl+Alt+D; the window follows the lock; first over the neighbouring pane, then where the user puts it; the window last clicked on top when it and the glass overlap.

## Decision

A fifth window, created when first opened and destroyed when closed, shows the Diff tab's view of one session in a frameless window with its own title row (the session's name, a move handle, close), resizable from its edges, always on top like the glass so that, when the two overlap, the one last clicked lies above. It is opened and closed from a button on the glass's bar and by Ctrl+Alt+D. It shows the session of the pane locked by a click (adr.rg.026) and follows the lock to another pane; with nothing locked it shows what the drawer's choice shows. It opens the first time over the neighbouring pane of the locked pane - the next to the right, or the one to the left for the last pane in its row - at that pane's size, never smaller than a size at which a diff reads (560 by 400 px); with no neighbour (a session in its own window, nothing locked) it opens in the middle of the screen. Once the user has moved or resized it, it opens where they left it, remembered across restarts; a placement ReviewGlass makes is never stored as the user's. It is not excluded from capture, like the drawer (adr.rg.025).

## Consequences

The diff of the pane being read sits beside it, in one command, without the dock; the drawer's Diff tab stays for everything else. A fifth webview while the window is open, none while it is closed. The locked pane's neighbour is read with the pane (its sibling in the UI Automation tree, adr.rg.027) so the window can be placed without a point. The glass and the diff window may overlap on a smaller screen; the user moves one, and the last clicked is on top. Ctrl+Alt+D becomes a global shortcut while ReviewGlass runs; taken by another application it fails alone and the bar's button still opens the window.
