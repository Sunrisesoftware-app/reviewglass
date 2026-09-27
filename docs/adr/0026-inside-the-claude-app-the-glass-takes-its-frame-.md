# ADR-0026: Inside the Claude app the glass takes its frame from the app: a click locks it to a pane, the box stays inside the frame, a window in front holds it

**Status:** Accepted
**Atlas id:** `adr.rg.026`
**Links:** `rg.glass-window` (Glass window), `rg.capture-engine` (Capture engine), `rg.finder-window` (Viewfinder), `rg.session-source` (Session source (trait)), `rg.panel-window` (Panel (the dock's drawer)), `reviewglass`

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

At the start of session 7 (27.9.2026) the owner said ReviewGlass was not yet in daily use because it does not feel like part of the Claude app, which they always run windowed, never full screen. Their wish: a click on a Code pane locks the glass to that whole pane, a frame appears, the mouse moves freely and the source box travels tightly inside the frame; and the target: the tool knows the app's bounds and never crosses them, and recognises a window that opens in front - where today the glass usually stretches wide sideways. adr.rg.017 reads the pane from a band of pixels across the whole monitor and cannot tell which window it is looking at: a window in front, or the desktop beside a windowed app, reads as a column, and Fit widens the glass to it. A read-only measurement the same day found the app's own structure exact: each Code pane's rectangle through UI Automation to within 1 px and stable across four runs (four panes of about 528 px in a 2387 px window), a popped-out session window with no pane whose page is its frame, the window's visible frame from DWM, keyboard focus inside a pane's message box, the windows above the app from the z-order in 0.03 ms, and a pane read in about 3 ms. adr.rg.022 already reads, under the cursor, the pane's rectangle and its header title.

## Decision

While the glass is shown in Follow, a left click inside a window of the desktop app (claude.exe) locks the glass to the pane under the click: the Code pane's rectangle read through UI Automation - the element at the click and its ancestors up to the pane, class names and rectangles only, as adr.rg.022 reads them - or, for a session in a window of its own, that window's page. Any click in a pane moves the lock there. The click is observed, never intercepted: the button's state is polled, nothing is hooked, and a click outside the app's windows does nothing. The lock shows as a frame: an outline on screen around the pane, click-through and excluded from capture like the finder. Inside it the source box follows the cursor and is clamped to the frame; with the cursor outside the frame the box stays at the frame's nearest edge. The glass's width follows the pane (Fit), never the pixels. While locked the pane is read again twice a second, so the frame follows the app's window when it moves or is resized and the pane when it changes width. A visible window of another application above the app's window that covers the source box holds the glass on its last picture and says so on the glass's bar; the glass never widens for it. The lock is released from a button on the glass's bar, and ends by itself - said on the bar - when its pane is gone. The locked pane's session is the session chosen for the Diff tab (adr.rg.022). Outside the app's windows the pixel reading of adr.rg.017 stays as it is.

## Consequences

Inside the app no pixel is guessed at, so a window in front or the desktop beside a windowed app can no longer stretch the glass: adr.rg.017's detector is superseded inside claude.exe's windows and stays for every other application. The UI Automation client reads on a click and twice a second while locked, inside the desktop app only, which keeps the app's renderer accessibility on while the glass is locked (the cost adr.rg.022 records). A pane whose rectangle cannot be read - a click on the resize handle between panes, a future app that renames its panes - is not locked, and the bar says so instead of guessing. Polling the mouse button is not a hook: a click is never swallowed, delayed or recorded. The frame is one more overlay on screen, never in the picture. The frameless diff window beside the locked pane is a decision of its own.
