# ADR-0021: The drawer is sized by hand from its free corner, and the size is remembered

**Status:** Accepted
**Atlas id:** `adr.rg.021`
**Links:** `rg.dock-window` (Dock), `rg.config-store` (Config store), `reviewglass`

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

adr.rg.020 made the drawer's height a setting, not a drag, and its width a constant (640 px): wide enough for a file list and a hunk side by side. On 20.9.2026 the owner followed a script being written in the Diff tab and saw only the left edge of each line: the code needs more width than the drawer has, the drawer could not be enlarged, and scrolling sideways with the bars is not reading. The dock's window is the drawer's window (adr.rg.020), snapped to a corner, so a resize must keep that corner fixed and grow away from it.

## Decision

The drawer has a resize handle at its free corner, the corner diagonally opposite the one the dock is snapped to, and dragging it sizes the open window with the operating system's own resize, the snapped corner staying put. The size persists as dock.drawer_width and dock.drawer_height (defaults 640 and 620, physical pixels) and is what the drawer opens to next time. The window has a minimum while open (640 by 344) and is never larger than the work area; closed, it is the strip and cannot be resized. The handle is visible and named; the Settings tab keeps no size fields. This supersedes adr.rg.020's 'a setting, not a drag' for the height and its fixed width; the rest of adr.rg.020 stands.

## Consequences

dock_drawer_resized persists the size the window ends a resize with and re-places it in its corner; DockConfig gains drawer_width. The Diff tab's file list has a maximum width, so a wider drawer goes to the code. A resize is the owner's act: nothing resizes the drawer on its own (the stillness standard behind adr.rg.018). The dock's own window declaration turns resizable, and the strip is kept unresizable by the minimum and maximum being the strip's size while closed.
