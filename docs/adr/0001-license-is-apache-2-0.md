# ADR-0001: License is Apache-2.0

**Status:** Accepted
**Atlas id:** `adr.rg.001`
**Links:** `reviewglass`

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

Open-source distribution after personal use; Tauri and the core Rust crates are MIT or MIT/Apache dual.

## Decision

Apache-2.0, verbatim LICENSE at the root, NOTICE only if a dependency requires attribution, per-file headers decided once and applied consistently.

## Consequences

No compatibility problem with the stack. Apache-2.0 is one-way incompatible with GPLv2: a GPLv2-only dependency can never be introduced without relicensing.
