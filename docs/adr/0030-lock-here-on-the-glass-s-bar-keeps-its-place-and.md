# ADR-0030: 'Lock here' on the glass's bar keeps its place and size until the open lock beside it is pressed; the implicit placing of adr.rg.029 is gone

**Status:** Accepted
**Atlas id:** `adr.rg.030`
**Links:** `rg.glass-window` (Glass window), `rg.config-store` (Config store), `rg.finder-window` (Viewfinder), `reviewglass`

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

The owner the day after adr.rg.029 (30.9.2026): the glass still jumped - a change of pane moved it, and a click on the project list in the app's left sidebar spread it over the whole screen. Two causes. Fit's width follows each locked pane and, widening, pushes the window back onto its monitor, so a placed glass still moved and grew; adr.rg.029 stopped only the centring. And the app now names its main window's page after the active session ('<title> - Claude Code', read by UI Automation the same day), so a click in the sidebar climbed to the page, took the main window for a session's window of its own and locked the glass to the whole window. The owner asked for the centred, locked view to be a button of its own on the glass's top bar: move the glass where it should be, press 'Lock here', and it does not move until released from an open-lock icon beside it. They also asked for the bar's text a little larger.

## Decision

The glass's bar carries 'Lock here'. Pressed, the glass keeps its place and its size: a pane locked from none does not centre it, Fit does not change its width (Fit's button shows it waiting), and the bar's and the tab's move handles, the picture's drag, the resize grips and the size buttons do nothing but say why. The bar shows 'Locked here' with an open lock beside it; the open lock releases it, and Fit takes the width back if it is on. The place and the size at the moment of locking are stored as the user's own (glass.pinned with x, y, width, height), so a restart opens the glass exactly as locked. The implicit rule of adr.rg.029 - a drag sets glass.placed - is removed. The glass menu's 'Centre on the screen' is offered while not locked. Separately, a page named '<title> - Claude Code' is the main window's, never a session's own window, and a click elsewhere in the window that holds the locked pane - its sidebar, its title bar - leaves the pane lock as it is.

## Consequences

The glass moves or grows only when the user has it unlocked; the owner's reading place survives pane changes, sidebar clicks and restarts. Unlocked, adr.rg.027's behaviour is unchanged: a lock from none centres the glass at Fit's width. A pane wider than a locked glass at the user's zoom shows its left part, where lines begin; the zoom is the user's to change. The bar's base text is 13 px instead of 12 at bar size 100 %, the bar 30 px high instead of 28. A session in a window of its own is still recognised by a page named by its title alone; should the app rename those pages too, only the sidebar case in the main window is guarded by the lock's own window.
