# ADR-0009: The account quota gauge is borrowed from any live CLI session

**Status:** Accepted
**Atlas id:** `adr.rg.009`
**Links:** `rg.usage-model` (Usage model), `rg.panel-window` (Panel (the dock's drawer)), `rg.session-source` (Session source (trait))

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

rate_limits arrives only through statusLine, which runs only in the CLI (adr.rg.003). A user working entirely in Desktop tabs would therefore see no quota at all, which is the exact problem the product exists to solve.

## Decision

rate_limits is account-wide and identical in every session, so the gauge is fed by ANY live CLI session and shown for every session in the panel, Desktop ones included. With no CLI session live the gauge is hidden and the panel says that a CLI session is what feeds it.

## Consequences

The headline feature has a dependency the user must satisfy: one terminal session somewhere. That is worth stating plainly in the README rather than hiding, and it is a condition the user can act on, unlike no-Pro/Max-plan, so the two absences must not share a message. It does not weaken adr.rg.007: the gauge is still one shared account-wide figure and attribution is still a separate relative quantity, now with two derivations (statusLine cost deltas for CLI sessions, transcript token counts for Desktop ones) which are labelled and never blended. If a future Claude Code runs statusLine on Desktop, this ADR is superseded and nothing else changes.
