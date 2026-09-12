# ADR-0012: Freeze is a still, not a locked live region

**Status:** Accepted
**Atlas id:** `adr.rg.012`
**Links:** `rg.glass-window` (Glass window), `rg.capture-engine` (Capture engine)

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

Spec 6.3 defined freeze as detaching from the cursor, locking the source rectangle, and scrolling it with the wheel — a live view of a fixed place. The first real use (12.9.2026) was to capture an instruction, drag the glass into a corner, and go work in Power BI. A locked live region would then have shown Power BI.

## Decision

Freeze holds the last published frame. The engine publishes nothing until resumed; zoom and the wheel are inert on a still; the still is dragged wherever it should live. F, the title bar, the right-click menu or a double-click in the lens freezes; the same controls resume.

## Consequences

The scrollable live-region variant is gone, and can return as a fourth mode if a use for it appears — none did in this session. A still restored from disk has no pixels in hand, so one frame is let through and then held. The spec's 6.3 text is superseded by this record.
