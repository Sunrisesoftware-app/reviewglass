# ADR-0017: The glass reads the pane: column boundaries from pixels, structure never content

**Status:** Accepted
**Atlas id:** `adr.rg.017`
**Links:** `rg.capture-engine` (Capture engine), `rg.glass-window` (Glass window), `rg.finder-window` (Viewfinder)

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

The first weeks of use showed the glass at a fixed width over panes of very different widths: two Code-tab columns are far wider than five, and the source rectangle either cut lines short or carried half of the neighbouring pane. Follow mode also tracked the cursor in both axes, so a hand that drifted sideways swung the picture sideways while the eye was reading down a column. adr.rg.006 rules out OCR, and the read-surface rule rules out UI Automation as a first choice: Chromium builds an accessibility tree only once a client asks, and maintaining it is a known cost for the host application.

## Decision

The capture engine detects the pane under the cursor from a band of pixels around it — a column is either textured or uniform; a run of uniform columns wide enough to be a gutter, or containing a colour change that is a border line, is a boundary; the pane is the textured span between the nearest boundaries. Structure only: no character is ever recognised and the band never leaves the process. Three behaviours hang on it, each with a control on the bar. Pane lock: in Follow mode the source rectangle follows the cursor vertically and is fixed to the pane horizontally. Fit: the glass's width follows the pane at the current zoom, capped at the monitor's width, lowering the zoom when it has to. The finder: a fourth window, click-through and excluded from capture, draws the source rectangle on screen so the user sees what the detector found and where the glass is looking.

## Consequences

A pixel heuristic can be wrong: a deeply indented band, a wide empty stripe inside a pane, a caret. The finder exists so a wrong guess is visible instead of silent, and a pane that is not found says so on the bar rather than falling back to a number. A width derived from the pane is never persisted as the glass's remembered width; turning Fit off restores the user's own. Detection runs at most four times a second and costs one band crop, measured on the release build. Four windows instead of three, one more WebView2 process. adr.rg.006 stands: this reads layout, not text, and UI Automation stays an experiment to be measured, not a dependency.
