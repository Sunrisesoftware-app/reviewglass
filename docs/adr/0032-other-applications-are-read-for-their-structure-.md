# ADR-0032: Other applications are read for their structure only, and only at the user's click; a recorder measures what they expose before P9 is built

**Status:** Accepted
**Atlas id:** `adr.rg.032`
**Links:** `rg.capture-engine` (Capture engine), `rg.glass-window` (Glass window), `reviewglass`

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

On 4.10.2026 the owner named the next phase (spec D9, P9): the glass should understand other applications the way it understands a Code pane - in daily use it found a text column in Chrome by pixels alone, sometimes. A click lock in another application would read that application's UI Automation tree. That tree is a far wider surface than the Claude app's panes: a browser exposes every page's text, a mail client every message, a password manager its entries, all as element names and values. And a Chromium or Electron application turns its accessibility on when a client first asks, which costs it work. What each application exposes about its reading area - a page's main landmark, an editor's text area, a terminal's buffer - is not known yet and differs by application.

## Decision

From an application other than the Claude desktop app, ReviewGlass reads structure and never content: for an element, its control type, localized control type, class name (at most 80 characters), framework, ARIA role, landmark type and bounding rectangle, and whether it is off screen. Never its name, value, help text, automation id, any text or selection pattern, or any other property that carries what the application shows. It reads only the element at a point the user clicked - observed, never intercepted - and the element's ancestors, and later, to find a reading area, the rectangles of elements around it; never on hover, never on a timer. No window of ReviewGlass is read, and none of an application on an exclusion list (password managers and the Windows credential dialogs: KeePass, KeePassXC, 1Password, Bitwarden, Credential UI, consent.exe), whose clicks are not read at all. Before P9's lock is built, a recorder in Settings > Measurements, started and stopped by the user and stopping by itself after eight hours or 2000 clicks, writes for each click outside ReviewGlass the application's executable name, the window's rectangle, the click point, the structure of the element and its ancestors as above, and how long each read took, to a time-named file under ~/.reviewglass/measurements on this machine; nothing is sent anywhere. The Claude desktop app keeps the reads of adr.rg.022 and adr.rg.026.

## Consequences

P9's design starts from a day of the owner's clicks in Chrome, Edge, VS Code and the terminals: which structure marks a reading area, how deep it lies, and what a read costs. The lock in other applications can use only the properties named here; one that needs an element's name or text - a terminal that exposes its buffer only as text, say - needs a new decision. The recorder's file shows the window's executable but not its title. Chromium's accessibility, once woken by the recorder or the lock, stays on in that application until it restarts; its cost is to be measured with the stall log and per-process CPU before P9 ships.
