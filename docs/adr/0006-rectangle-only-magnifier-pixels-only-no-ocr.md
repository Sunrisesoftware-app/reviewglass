# ADR-0006: Rectangle-only magnifier, pixels only, no OCR

**Status:** Accepted
**Atlas id:** `adr.rg.006`
**Links:** `rg.capture-engine` (Capture engine), `rg.glass-window` (Glass window)

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

Circular or square lenses look attractive; OCR of the magnified region would turn the glass into a reader.

## Decision

Rectangle only, zoom clamped 150-400 %, bitmap scaling only. The glass shows pixels; the diff view shows text.

## Consequences

Code is line-oriented and a rectangle is the only shape that reads it. Above roughly 400 % bitmap scaling visibly degrades, so higher factors are not offered. Pixels are never written to disk or transmitted.
