# ADR-0008: Name availability is unresolved and gates public release only

**Status:** Proposed
**Atlas id:** `adr.rg.008`
**Links:** `reviewglass`

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

'ReviewGlass' has not been checked for conflicting use.

## Decision

Development proceeds under the working name in a private repository; the check is done before the repository is opened (P8).

## Consequences

Renaming touches the repo, the bundle identifier and the installer, none of which are public before P8.
