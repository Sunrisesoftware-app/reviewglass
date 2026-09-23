# ADR-0007: Account-wide quota and per-session attribution are two quantities, never blended

**Status:** Accepted
**Atlas id:** `adr.rg.007`
**Links:** `rg.usage-model` (Usage model), `rg.panel-window` (Panel (the dock's drawer))

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

rate_limits from Claude Code is account-wide and identical in every concurrent session; the user's question 'which session is eating my limit' cannot be answered from it directly, and there is no per-model breakdown.

## Decision

AccountQuota is one shared gauge. SessionAttribution is a derived relative share from deltas in cost and context over time. The UI shows one gauge plus N shares, in different units, never a single number.

## Consequences

The panel never claims a per-session percentage it does not have. Burn rate is a linear projection labelled an estimate; samples before a resets_at boundary are discarded. The model-specific weekly limit behind the 94 % notification is not exposed by this channel and is not fabricated.
