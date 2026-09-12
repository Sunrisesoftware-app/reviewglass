# ADR-0011: Every Claude Code field tolerates a wrong type, not only an absence

**Status:** Accepted
**Atlas id:** `adr.rg.011`
**Links:** `rg.statusline-collector` (statusLine collector), `rg.session-source` (Session source (trait))

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

The spec's rule is that every field may be absent and that absent data hides its element. Optional fields deliver that. They do not deliver anything against a field that changes TYPE, and one did: measured on 2.1.268, context_window.current_usage is an object of token counts, where spec section 5.1 describes a number. With a plain Option the single mismatched field failed the whole payload, so the panel lost the session, the model, the cost and the account quota over a field it does not even read.

## Decision

Every field of every Claude Code payload deserialises through a lenient wrapper: a value that does not fit its type becomes None and its neighbours are untouched. Absence and wrong-type collapse to the same outcome, which is the outcome the spec already prescribes.

## Consequences

One field changing shape can cost that field and never the panel, which is what spec section 9's risk row asks for and what a bare Option does not give. The price is that a genuine schema change is silent by default, so the collector reports a parse failure on stderr under REVIEWGLASS_DEBUG: a collector that has quietly stopped understanding Claude Code is the failure nobody would otherwise notice. This is a stronger rule than feature-detection: it also covers the case where a field is present, named as expected, and the wrong shape.
