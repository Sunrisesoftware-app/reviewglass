# ADR-0034: The diff window is glued to a side of the glass: one click opens it where there is room, a drag beside the glass glues it to that side, and it follows the glass's place and size

**Status:** Accepted
**Atlas id:** `adr.rg.034`
**Links:** `rg.glass-window` (Glass window), `rg.panel-window` (Panel (the dock's drawer)), `rg.config-store` (Config store), `reviewglass`

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

The owner, 11.11.2026, after a month of occasional use: the basic use in the Code app is fine, but the diff comparison is hardly used because using it should be easier. Ctrl+Alt+D works and the window appears; the owner wants the same with one click, the window coming up automatically on a side of the magnifier where there is room, movable by dragging to the left, the right, below or above the glass, and from then on glued to it, following its size as the glass changes it to fit the pane or the view. adr.rg.028 opened the diff window first over the locked pane's neighbouring pane and then where the user had left it: a place of its own, unrelated to the glass, so every lock change or Fit width left the two apart and the diff had to be looked for. The owner's standard is a calm tool (no jumping) that feels part of the Claude app.

## Decision

The diff window (adr.rg.028) is attached to one side of the glass - left, right, above or below - or free. One click on the glass's Diff button (or Ctrl+Alt+D) opens it attached: to the side the user last chose, or, before any choice, to the first side of right, left, below and above with room on the glass's monitor for a diff that reads (420 px beside the glass, 240 px above or below); with no such side, the side with the most room. Attached, the shared edge is the glass's: beside the glass the window takes the glass's top and height, above or below it the glass's left and width (never shorter than 240 px), with a gap of a few pixels; the other dimension, its thickness, is the user's, set by resizing the free edge and remembered per orientation across restarts. It never leaves the screen: short of room it is narrowed down to the minimum, then moved onto the screen over the glass, never cut. It follows every move and resize of the glass - a lock, Fit, Lock here, a drag - at once, driven by the glass window's own move and resize events, never by a poll; the Lens, which rides the cursor, is not followed. The title row carries four side buttons that attach it with one click, the lit one naming the side; dragging the window by its title and dropping it within 64 px of a side of the glass, or over it, attaches it to the side its centre lies towards, and dropping it further away frees it, where it stays and is remembered as adr.rg.028 had it, the row then saying that a drag beside the glass attaches it. With the glass hidden or in the Lens, the window opens free. The side holds while the window is open: the glass moving to a side with less room moves the window onto the screen, it does not change sides by itself. Supersedes adr.rg.028's placement; the rest of adr.rg.028 stands.

## Consequences

The diff of the pane being read sits against the glass and moves with it, in one click, without the user placing it; the two read as one tool. A move or resize of the glass now also moves a second window (two calls on the main thread per glass event, none while the diff window is closed). The glass and the diff window can cover the neighbouring pane; the side buttons and a drag move the diff away. A free window keeps the old behaviour for those who want it elsewhere. The remembered place of adr.rg.028 is kept for the free state; a configuration from before this decision opens attached.
