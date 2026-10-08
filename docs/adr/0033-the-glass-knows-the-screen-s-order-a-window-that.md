# ADR-0033: The glass knows the screen's order: a window that comes to the front over the locked pane releases the lock, and the picture lets the mouse through

**Status:** Accepted
**Atlas id:** `adr.rg.033`
**Links:** `rg.glass-window` (Glass window), `rg.finder-window` (Viewfinder), `rg.capture-engine` (Capture engine), `reviewglass`

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

The owner's week of use (8.10.2026): with Code panes on the screen the glass works perfectly, but the moment anything else comes up - a console, File Explorer, a browser, the browser Claude itself opens, Excel - ReviewGlass had to be closed every time. Windows were hard to close and move, and reading Excel came to nothing; 'the tool is very static, and it does not answer the way varied PC work should'. Every one of ReviewGlass's windows is always on top of the whole desktop, and adr.rg.026 made a window of another application in front of the locked pane hold the glass on its last picture, so the glass stayed over the new window, still showing Claude, and took its clicks. Asked on 8.10.2026, the owner chose: when another application comes up over Claude, the pane reading ends, the glass is released and reads the window in front; moving the mouse behind the glass does not darken it but lets the mouse through, so a box can be dragged out from behind the glass; the magnification keeps helping while it recognises other applications and the order of the windows on the screen. The dock stays always on top. Amended the same day, after the owner's first afternoon with it: an Explorer or Excel window opened over Claude but beside the locked pane did not release the lock - 'covers Claude' was the owner's word, not 'covers the pane' - and the first build hung its own main thread (a lock held across a window call), so the glass froze on the Claude pane and the window behind it could not be clicked; the hang, not the rule, was also what Print Screen met.

## Decision

While a pane is locked (adr.rg.026), a window of another application that becomes the foreground window and overlaps the Claude app's window - or that window being minimised - releases the lock at once; the shell's own windows (the taskbar, the desktop, the start menu, task view) do not: the frame and the box are gone, the bar says which window came to the front, and the glass follows the cursor over whatever is in front, reading columns from pixels as adr.rg.017 does outside the app. A window of another application that overlaps the box without being in the foreground (the taskbar, a notification) still only holds the picture, as before. Returning to the Claude app is a click on a pane, and that click locks again. In Follow the glass's picture lets the mouse through: the area below the bar takes no click, wheel or hover, so the window behind it can be clicked, scrolled and dragged from; the picture is never darkened, and the source holds only while the pointer is on the glass's bar or its tab, where its controls are. Still keeps a picture that is grabbed and dragged; Lens is unchanged. The dock stays always on top, the fixed point of adr.rg.015 and adr.rg.018.

## Consequences

Working in another application no longer needs ReviewGlass closed: the glass gives up Claude's pane the moment something else comes to the front, and magnifies what the user is now looking at. The picture's own gestures in Follow - the wheel's zoom, a double-click's freeze, the right-click menu - move to the bar, which has all three. adr.rg.026's 'a window in front holds the glass' now holds only for a window that is not in the foreground. A locked pane is given up even for a moment's look at another window; a click on the pane takes it back, and a glass locked here (adr.rg.030) keeps its place throughout. The window's name on the bar is its title, as the covering check already shows it. P9 builds on this: the window in front is the one a click would lock to.
