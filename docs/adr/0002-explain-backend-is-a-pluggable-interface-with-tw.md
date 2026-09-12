# ADR-0002: Explain backend is a pluggable interface with two shipped implementations

**Status:** Accepted
**Atlas id:** `adr.rg.002`
**Links:** `rg.explain-service` (Explain service (novice mode)), `rg.explain-backend` (Explain backend (trait, two implementations))

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

The explain feature could hardcode a remote API or a local model; neither should be privileged.

## Decision

rg.explain-service composes the request and hands it to trait ExplainBackend; RemoteBackend and LocalBackend ship as equal-status options; default is no backend at all.

## Consequences

A fresh install performs no inference. Local loopback is not egress but the active mode must stay visible. A third backend touches only rg.explain-backend. A provider name outside that module is a boundary violation.
