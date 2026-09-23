# ADR-0005: Tauri v2 with a Rust core, not Electron

**Status:** Accepted
**Atlas id:** `adr.rg.005`
**Links:** `rg.capture-engine` (Capture engine), `rg.glass-window` (Glass window), `rg.panel-window` (Panel (the dock's drawer))

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

The glass is a transparent always-on-top overlay redrawn continuously; it must stay cheap enough to leave running all day.

## Decision

Tauri v2 shell, Rust core for capture interop and the watcher, a lightweight frontend (Svelte) without virtual-DOM overhead, an existing diff-rendering library.

## Consequences

Electron's 200-300 MB baseline is avoided. Windows.Graphics.Capture is reached through the windows crate. Two windows (glass and panel) rather than one, because hosting both in one window is the failure mode that stalls projects of this shape.
