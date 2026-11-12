# ADR-0035: The glass lets the screen show through while the pointer is over its picture

**Status:** Accepted
**Atlas id:** `adr.rg.035`
**Links:** `rg.glass-window` (Glass window), `rg.config-store` (Config store), `reviewglass`

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

The owner, 12.11.2026: since adr.rg.033 the mouse passes through the glass picture, and what lies under the cursor shows magnified in the glass, but when the glass covers a window one cannot know exactly where a control is - the side panel toggle of a conversation, a menu, a window corner X - so the user gropes, searches, or in the worst case shrinks the glass. The owner asked that whenever the mouse is over the glass its transparency grow just enough that the pointer can go to the right place in the controls or a menu while the same place shows larger in the glass. On web pages, where the glass often goes to the full screen width because no clear column is found, the same would let the page be used through the magnification.

## Decision

In Follow, while the pointer is over the glass picture - the area that already lets the mouse through (adr.rg.033) - the picture turns see-through: its dark ground is dropped and the magnified picture is drawn at a set opacity, so the window behind it shows in place at its own size under the magnified one and the pointer finds a control without moving the glass. The change fades in and out over 150 ms, so the glass never flickers. The bar and the tab stay opaque, and Still and the Lens are unchanged. How much shows through is the user setting (25 to 75 percent; 45 by default), in Settings > Glass; a button on the glass bar switches see-through on and off, and both are remembered. On by default, since it was asked for.

## Consequences

Controls under the glass are found by sight instead of by groping, in the Claude app, on web pages and in other applications alike. While the pointer sits on the glass the two pictures overlap, so the magnified text is fainter there; the level and the switch are the answer, and the picture is fully opaque again as soon as the pointer leaves it. The page is told when the pointer enters and leaves the picture by the rider that already decides the click-through, so no new polling is added. The glass is still excluded from capture, so what shows through never feeds back into the picture.
