# ADR-0015: A hideable app stays findable: a tray icon and a single instance

**Status:** Accepted
**Atlas id:** `adr.rg.015`
**Links:** `rg.glass-window` (Glass window), `rg.panel-window` (Panel window)

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

Both windows can be hidden — the panel on close, the glass on Esc or the hotkey — and that left a process running with no trace of itself. The user closed the app, it kept running invisibly, a second launch started a second copy, and two instances were measured alive at once, fighting for the config file and the capture.

## Decision

A tray icon is the fixed point: show glass, show panel, lens on/off, quit for real; a left click brings the panel up. The single-instance plugin turns a second launch into 'bring the running copy forward'. Both windows' hints name the tray as the way back.

## Consequences

Quit and hide are now distinct actions with distinct controls. Closing the panel still hides it, which is only friendly because the tray exists. The installer (P7) inherits the rule: never ship a hideable window without the tray.
