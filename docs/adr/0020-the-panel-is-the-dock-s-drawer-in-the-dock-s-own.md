# ADR-0020: The panel is the dock's drawer, in the dock's own window: one window that grows, not two that can drift apart

**Status:** Accepted
**Atlas id:** `adr.rg.020`
**Links:** `rg.dock-window` (Dock), `rg.panel-window` (Panel window), `rg.diff-service` (Diff service), `rg.usage-model` (Usage model), `rg.config-store` (Config store)

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

adr.rg.018 made the panel open beside the dock as its drawer, but it stayed a separate, conventional window: it could be moved on its own, it did not follow the dock to another corner, and nothing tied the two together once open. When the live diff (P4) arrived as a tab in it, the owner's verdict (19.9.2026) was that the panel was a loose application box, disconnected from the dock, and that no real UX design had been done for it: the panel — Sessions, Diff, Settings — should be attached to the dock, open from it, move with it, and never be two separate windows that can be apart. Two ways to get there: glue the two windows together with position logic on every move and resize, or make the panel part of the dock's window. The owner chose the second, with the width and the tabs decided: 640 px open, the tabs inside the drawer.

## Decision

The panel lives in the dock window. Closed, the dock is the 470×44 strip it has been. Open, the same window grows to 640 px wide and the drawer's height (620 px by default) below the strip in a top corner or above it in a bottom corner, snapped to the same corner; the strip is the drawer's title row and stays where it was. The strip's ▤ button, the tray, the glass's menu and its bar button all open the drawer — there is no other panel window. The tabs (Sessions, Diff, Settings) sit in the drawer's first row, not on the strip, which stays a strip. The drawer starts closed at every start and remembers its last tab. The drawer is excluded from capture with the dock, so it can never appear inside the glass.

## Consequences

Four windows, not five: the panel window and its close-to-hide handling go; the panel's components move under the dock route; the diff loop and the usage loop address the dock. Attachment costs no logic: a drag, a snap, a corner change or a resize moves one window. Dragging the strip with the drawer open drags the whole thing, which is the point. The drawer cannot be moved or sized independently — a deliberate loss, in exchange for never being lost; the height is a setting, not a drag. Content is designed for 640 px: the Diff tab's file list and hunk share that width. The dock window becomes resizable in the one way the drawer needs (programmatic set_size), still not by the user. adr.rg.018's 'the panel opens beside the dock' is superseded by this; its other content stands. rg.panel-window's contract stays (tabbed, absent data hides its element) but the module is hosted by rg.dock-window rather than a window of its own.
