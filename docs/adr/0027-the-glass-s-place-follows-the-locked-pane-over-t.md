# ADR-0027: The glass's place follows the locked pane: over the neighbouring pane at its width and three quarters of its height, a handle outside its corner, the arrow keys move the lock

**Status:** Accepted
**Atlas id:** `adr.rg.027`
**Links:** `rg.glass-window` (Glass window), `rg.finder-window` (Viewfinder), `rg.config-store` (Config store), `reviewglass`

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

The owner's look at the frame (28.9.2026): the pane lock steadied the use and made it controlled, 'a good change in the right direction'. The glass itself was not: it opened where it had last been, on top of whatever lay there, and its controls lived inside it, so reaching for them crossed the picture, which dimmed and flickered. The owner asked for the glass's place to be controlled too - sized like a pane, three quarters of the pane's height, so the neighbouring pane's message box stays visible below it and can be clicked - for a move handle and a hide button outside the glass's bottom-right corner, and for the arrow keys to move the lock from pane to pane without the mouse. Asked the same day, the owner chose: a glass moved by hand stays until the lock moves to another pane; Ctrl+Alt+arrows everywhere and plain arrows while the glass has focus; the pane's size replaces Fit while a pane is locked; the hide button hides. A read-only measurement showed a pane's neighbours are its siblings in the UI Automation raw view (found in 1-2 ms), and that a point beside the locked pane may lie under the glass itself, so a neighbour cannot be found by a point.

## Decision

While a pane is locked (adr.rg.026), the glass is placed over the neighbouring Code pane - the next one to the right, or the one to the left when the locked pane is the last in its row - at that pane's left and top, as wide as that pane and three quarters of its height, so the rest of that pane, its message box included, stays visible and clickable below. Neighbours are found among the locked pane's siblings in the UI Automation raw view, never by a point. The glass is placed again whenever the lock moves to another pane, and follows the neighbour when the app's window moves or its panes change width, unless the user has moved or resized the glass by hand since it was placed: a hand-placed glass stays where it was put until the lock moves to another pane. A session in a window of its own has no neighbour, and the glass stays where it is. While locked, the glass's size and place are the pane's, never stored as the user's own; Fit applies to the unlocked Follow only. Outside the glass, hanging below its bottom-right corner, a small tab carries a move handle and a hide button, so the glass is moved or put away without the pointer crossing the picture; the window is shaped so that nothing else outside the glass takes a click. Ctrl+Alt+Left and Ctrl+Alt+Right move the lock to the neighbouring pane from anywhere; plain Left and Right do the same while the glass has the focus.

## Consequences

The glass has a predictable place beside what it magnifies and never covers the pane it reads, nor the neighbour's message box. Clicking that message box is a click in a pane, so it moves the lock there and the glass moves beside the new pane - the owner's own rule of 27.9.2026, to be watched in use. The glass moves on its own when the lock moves, which is the one movement the owner asked for; within one lock it moves only with the app's window. Ctrl+Alt+Left and Right become global shortcuts while ReviewGlass runs; if another application holds them, only the plain arrows work, and the others keep working. The glass's window grows by the tab's height below the glass, cut to the glass and the tab by a window region. The frameless diff window (next) will reuse the same placement.
