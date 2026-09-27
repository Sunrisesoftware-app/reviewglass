# ADR-0025: The dock is visible to screen capture and the drawer takes its own picture; the glass, the halo and the finder stay hidden

**Status:** Accepted
**Atlas id:** `adr.rg.025`
**Links:** `rg.dock-window` (Dock), `rg.panel-window` (Panel (the dock's drawer)), `rg.glass-window` (Glass window), `rg.halo-window` (Cursor halo), `rg.finder-window` (Viewfinder), `reviewglass`

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

Every ReviewGlass window was excluded from capture (WDA_EXCLUDEFROMCAPTURE): the glass so that it never captures itself, the halo and the finder so that they never land in its picture, and the dock with its drawer (adr.rg.018, adr.rg.020) so that the glass never shows them. On 27.9.2026 the owner wanted to show the Diff tab to explain what a paragraph of new code means, and no screenshot tool could see the drawer. Follow already reads nothing while the pointer is on the dock, so the dock's exclusion no longer protects the glass's reading.

## Decision

The dock window - strip and drawer - is no longer excluded from capture: a screenshot tool, a screen share and the glass see it like any other window. The glass, the halo and the finder stay excluded. The drawer's tab row ends in a camera button that takes the drawer's own picture: WebView2 renders the page itself (CapturePreview, no screen capture), the drawer's part is cut out with the strip left out, and the picture goes to the clipboard as PNG and as a device-independent bitmap and is saved as Pictures\ReviewGlass\ReviewGlass-<date>-<time>.png. A note beside the button says where it went; clicking it shows the file in Explorer. Nothing is sent anywhere.

## Consequences

The glass can show the dock when it looks at it (the pointer beside the dock, not on it), as it shows any other window, and a screen share shows the dock. The picture is exactly the drawer at its own resolution, whatever overlaps it on screen. Two direct dependencies, both already in the build: webview2-com (Tauri's own) and png. adr.rg.020's 'the drawer is excluded from capture with the dock' is superseded by this; its other content stands. The rule that the glass never captures itself is unchanged.
