# ADR-0016: The usage loop runs on its own thread, not on the panel's poll

**Status:** Accepted
**Atlas id:** `adr.rg.016`
**Links:** `rg.notifier` (Notifier), `rg.usage-model` (Usage model), `rg.panel-window` (Panel window)

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

P2 first computed the usage model when the panel asked for it. P3's alerts must fire when the user is elsewhere — the whole point of an alert — and the burn-rate history must accumulate whether or not anyone is looking.

## Decision

A Rust thread reads both session channels every two seconds, updates the usage model, runs the notifier and delivers toasts. The panel reads the latest view and drives nothing.

## Consequences

Alerts fire from a closed panel. The panel's first two seconds after start have no view and say so. Delivery is a Windows toast through the notification plugin; on an unpackaged executable it is attributed to Windows PowerShell, which the installer (P7) fixes by giving the app its own identity. A failed toast is one line on stderr, never a retry: the notifier has already marked the threshold fired.
