# ADR-0027: When a pane is locked the glass centres on the screen at Fit's width; a handle outside its corner; the arrow keys move the lock

**Status:** Accepted
**Atlas id:** `adr.rg.027`
**Links:** `rg.glass-window` (Glass window), `rg.finder-window` (Viewfinder), `rg.config-store` (Config store), `reviewglass`

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

The owner's look at the frame (28.9.2026): the pane lock steadied the use and made it controlled, 'a good change in the right direction'. The glass itself was not: it opened where it had last been, on top of whatever lay there, and its controls lived inside it, so reaching for them crossed the picture, which dimmed and flickered. The owner asked for the glass's place to be controlled too - sized like a pane, three quarters of the pane's height, so the neighbouring pane's message box stays visible below it and can be clicked - for a move handle and a hide button outside the glass's bottom-right corner, and for the arrow keys to move the lock from pane to pane without the mouse. Asked the same day, the owner chose: a glass moved by hand stays until the lock moves to another pane; Ctrl+Alt+arrows everywhere and plain arrows while the glass has focus; the pane's size replaces Fit while a pane is locked; the hide button hides. A read-only measurement showed a pane's neighbours are its siblings in the UI Automation raw view (found in 1-2 ms), and that a point beside the locked pane may lie under the glass itself, so a neighbour cannot be found by a point. Amended the same day, after the owner's first use of the first build: a glass the neighbouring pane's size showed a reading area half the locked pane's width at 200 %, since two panes of one size hold text of one size and the glass is a magnifier. The build before, whose reading area was the whole locked pane (Fit: the glass as wide as the pane times the zoom), had been right. The owner's words: go back to that, the glass centring on the screen when a pane is locked, to be moved by hand.

## Decision

When a pane is locked from none (adr.rg.026), the glass centres on the screen at Fit's width - the locked pane's width times the zoom, capped at the monitor with the zoom lowered to fit, as adr.rg.017 had it - so the reading area is the whole pane, magnified; its height is the user's own. A lock that moves between panes, by a click or a key, leaves the glass where it is and lets only Fit's width follow the pane; the user moves it by hand wherever it should be. Outside the glass, hanging below its bottom-right corner, a small tab carries a move handle and a hide button, so the glass is moved or put away without the pointer crossing the picture; the window is shaped so that nothing else outside the glass takes a click. Ctrl+Alt+Left and Ctrl+Alt+Right move the lock to the neighbouring Code pane from anywhere, and plain Left and Right do the same while the glass has the focus; neighbours are the locked pane's siblings in the UI Automation raw view, never a point beside it, which may lie under the glass, and a header under the glass is read through the tree, its top 48 px only.

## Consequences

The glass magnifies the whole locked pane again and appears in one predictable place, the middle of the screen, when a lock begins; after that it moves only by the user's hand or by Fit's width. The first build's placement over the neighbouring pane is gone, with its three-quarter height and its re-placing; the owner's wish that the neighbour's message box stay visible is left to where the user puts the glass. Ctrl+Alt+Left and Right become global shortcuts while ReviewGlass runs; if another application holds them, only the plain arrows work, and the others keep working. The glass's window grows by the tab's height below the glass, cut to the glass and the tab by a window region. The frameless diff window (next) will not reuse a placement over the neighbour; its place is decided with it.
