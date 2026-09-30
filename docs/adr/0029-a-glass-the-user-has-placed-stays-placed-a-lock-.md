# ADR-0029: A glass the user has placed stays placed: a lock centres it only until the first drag, and the place survives a restart

**Status:** Superseded
**Atlas id:** `adr.rg.029`
**Links:** `rg.glass-window` (Glass window), `rg.config-store` (Config store), `reviewglass`

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

The owner's daily use (29-30.9.2026): after moving the glass to a place of their own, a click on another pane now and then sent it back to the middle of the screen. adr.rg.027 centres the glass when a pane is locked from none, and a lock is lost and found again more often than it looks: whenever the glass is hidden or in Still, when the lock is released, and when the Claude app redraws the locked pane so that its UI Automation element goes stale; the next click then locks from none and centres. The centring also saved the middle of the screen as the glass's position, so a restart lost the user's place too. The owner asked that a glass moved once stay where it was put, across pane changes and restarts.

## Decision

The glass's configuration keeps whether the user has placed it (glass.placed). A drag the user starts - on the bar, the picture or the tab's move handle - that moves the window sets it; a move made by code (the centring, Fit keeping the window on its monitor) never does, and a press that did not move the window within a second is not a drag. While it is set, a lock from none leaves the glass where it is and applies only Fit's width; the position and the flag are stored together, so after a restart the glass opens where the user left it. The glass's right-click menu offers 'Centre on the screen', which centres the glass at once and clears the flag, so locks centre it again until the next drag. adr.rg.027's centring stays the behaviour of a glass nobody has placed.

## Consequences

The glass moves only by the user's hand or by Fit's width once the user has put it somewhere; the moments at which a lock begins from none stop mattering. A first run and a config from before this decision still centre on the first lock. The way back to centring is a visible menu item, not a reset of the settings. Tooltips on both move handles say that the glass stays where it is put and how to undo it. Superseded the next day by adr.rg.030: the owner found the glass still moving and asked for a control of their own - 'Lock here' on the bar - rather than a rule inferred from a drag.
