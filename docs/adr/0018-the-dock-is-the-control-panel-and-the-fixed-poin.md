# ADR-0018: The dock is the control panel and the fixed point: the glass hangs from it

**Status:** Accepted
**Atlas id:** `adr.rg.018`
**Links:** `rg.dock-window` (Dock), `rg.glass-window` (Glass window), `rg.usage-model` (Usage model), `rg.config-store` (Config store)

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

After two days of use the owner's verdict was that the glass, alone on the screen, demanded attention: it appeared at start whether wanted or not, its controls sat on the thing being read, and with the pane lock it changed width on its own. The tray (adr.rg.015) keeps the app findable but is not a place to work from. The owner asked for a control panel that sits in a corner of the screen and from which the glass is switched on and off — the glass on a rubber band to the dock, not a window that leaps about on its own.

## Decision

A fourth kind of window, the dock: a small always-on-top strip snapped to a screen corner (top-left by default; dragging it snaps it to the nearest corner and the corner is remembered), excluded from capture, visible at every start while the glass starts hidden. It carries one button per glass mode — Follow, Lens, Still — where a click switches the glass on in that mode and a click on the lit button switches it off; the account gauge in miniature (the 5-hour percentage and the time to its reset, or the reason there is none); the panel button; and the build stamp. Activation belongs to the dock: the glass's bar keeps its controls (adr.rg.013) so the glass can still be left from itself, but nothing else appears on screen until the dock is asked.

## Consequences

Five windows. The glass no longer restores itself visible: `visible` is not a setting any more but the dock's state, and a restart begins with the dock alone. The tray remains the fallback for a dock that was hidden or lost off-screen. The gauge in the dock is the same account-wide figure the panel shows (adr.rg.007), never a per-session one, and its absence names the reason. The dock's snapping is the one automatic movement in the design; every other automatic movement or resize of a ReviewGlass window is now measured against the owner's standard that the tool must calm the work, not demand attention.
