# ADR-0036: A session in a window of its own is read by its message column, and Fit has a widest glass the user sets

**Status:** Accepted
**Atlas id:** `adr.rg.036`
**Links:** `rg.glass-window` (Glass window), `rg.finder-window` (Viewfinder), `rg.capture-engine` (Capture engine), `rg.config-store` (Config store), `reviewglass`

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

The owner, 12.11.2026: with a conversation popped out into a window of its own over the Code app, the glass became needlessly large, about 65 percent of the screen width; narrowed by hand, the glass followed well. A lock in a session window took the whole page (adr.rg.026), and Fit made the glass that width times the zoom. Measured the same day by UI Automation, with the window widened to 1600 px for two seconds by the owner leave: the message list spans the page, but the messages sit in a centred column about 928 px wide; at the usual width of about 830 px the column is the window less its padding, so reading the column alone would narrow a 2x glass by some 40 px. The glass is the column times the zoom (LESSONS, 28.9.2026). Shown these figures, the owner chose both: the column, and a widest glass of their own.

## Decision

A lock on a session in a window of its own takes the message column as its reading area: the horizontal span of the messages shown in the conversation scroll area (the element whose class carries overflow-y-auto and contain:strict, and its list of messages), between the page top and bottom; where the column is not narrower than the page by more than 40 px, or cannot be read, the page as before. It is read at the lock and again with the frame every reread. Fit has a widest glass, a share of the screen width the user sets in Settings > Glass (40 to 100 percent; 60 by default; 100 means no limit): a column times the zoom wider than that makes the glass that wide at the user zoom, and the box follows the cursor sideways inside the column, as a locked frame wider than the box always did (adr.rg.026); only a column too wide for the screen itself lowers the zoom, as before. Lock here (adr.rg.030) keeps its rule: Fit widens and narrows around the locked centre, within the widest glass.

## Consequences

A wide popped-out window no longer stretches the glass past its message column, and no column makes the glass wider than the user allows; a column wider than that is read by moving the pointer along it, as a long line is in a narrow pane. Reading the column costs a few dozen UI Automation calls at a lock and at each reread of a session window; panes in the main window are unchanged. The widest glass also applies to panes in the main window and to the pixel detector outside the app, where a page with no clear column no longer makes the glass the width of the screen. The column is found by a class the app may rename; it falls back to the page, and the fallback is the old behaviour.
