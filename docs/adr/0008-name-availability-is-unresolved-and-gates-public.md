# ADR-0008: Name availability is unresolved and gates public release only

**Status:** Proposed
**Atlas id:** `adr.rg.008`
**Links:** `reviewglass`

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

'ReviewGlass' has not been checked for conflicting use. Probes on 17.9.2026 (a web search, GitHub and npm) found nothing under the name; crates.io refused the probe, so the registry is unchecked.

## Decision

Development proceeds under the working name. The repository has been public since 17.9.2026 by the owner's decision, after that day's probes; the name check proper is done before P8's installer and bundle identifier, and P8 is what the name gates.

## Consequences

Renaming now touches the public repository's name as well (GitHub redirects the old one), the bundle identifier and the installer; the last two are not public before P8. The README, CLAUDE.md and BUILD_INFO say public since 17.9.2026 (reviewglass c96b412); the spec's header moves with spec v3.
