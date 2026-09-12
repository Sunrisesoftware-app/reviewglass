# ADR-0014: A cursor halo marks the pointer on screen while the glass follows it

**Status:** Accepted
**Atlas id:** `adr.rg.014`
**Links:** `rg.halo-window` (Cursor halo), `rg.glass-window` (Glass window)

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

Follow is the base mode: the glass parked where it is wanted, showing what is around the pointer. With the eyes on the glass, the user loses track of where on the screen the pointer actually is.

## Decision

A third window, click-through and excluded from capture, rides on the cursor in Follow mode and draws a glowing orange ring. It shares the rider thread with the lens; at most one of the two rides at a time. A toggle on the glass's bar turns it off, and the choice persists.

## Consequences

Three windows instead of two, which the topology in spec 6.1 does not show yet. The halo never appears in the picture (it is excluded from capture like the glass), so the cursor's position inside the magnified view is the picture's centre by construction, not a drawn marker. Windows imposes a minimum top-level window width, so the ring is a fixed circle at the window's centre rather than the window's own shape.
