# ADR-0013: The glass is a window: a permanent title bar and three named modes

**Status:** Accepted
**Atlas id:** `adr.rg.013`
**Links:** `rg.glass-window` (Glass window), `rg.config-store` (Config store)

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

The first extended hands-on session found the glass confusing: a control bar that came and went, modes shown as glyphs, a thin border that read as part of the picture, and hover behaviour layered on top. The user asked for an application-style bar that is always there, and said it would not hurt use.

## Decision

A permanent title bar like any other window's — drag handle, app name, the mode as three labelled buttons (Follow, Lens, Still), zoom, window size, halo toggle, chrome-scale step, panel, hide, quit — and a 3 px border in the mode's colour. The bar's own size steps through 100–200 % and is remembered, because a magnifier's own chrome must not be one of the things that are too small. In the lens the cursor is always at the window's centre, so the bar cannot be reached by mouse; there it shows what the keys and the right button do. The lens is not click-through, so that a right-click and a double-click on it work; it has its own remembered size, and it rides the cursor on a thread at ~120 Hz rather than from the frame poll.

## Consequences

Hover-hold and the dim stay in Follow mode as the thing that makes pressing Still safe: the picture cannot change to what is under the glass between reaching for the button and pressing it. Lens users lose click-through, which the reading-then-parking workflow does not need. Sizes are stored only when the user resizes, from the window's real inner size and into the slot of the current mode — the earlier practice of persisting the canvas size as the window size shrank the window on every restart (measured: 900×340 had drifted to 632×352).
