# ADR-0008: The name is ReviewGlass, kept by the owner's decision; the public release is no longer gated on it

**Status:** Accepted
**Atlas id:** `adr.rg.008`
**Links:** `reviewglass`

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

'ReviewGlass' has not been checked for conflicting use. Probes on 17.9.2026 (a web search, GitHub and npm) found nothing under the name; crates.io refused the probe, so the registry is unchecked. A web research pass on 4.10.2026, at the owner's request (the repository's docs/NAME-CLEARANCE.md): the trademark registers it could query (TMview, which carries EUIPO, PRH, USPTO and WIPO data, and Trademarkia) hold no ReviewGlass or Review Glass mark; the Finnish company register has none; the package names are free on npm, crates.io, PyPI and NuGet. But two unrelated e-commerce review services trade as ReviewGlass and hold reviewglass.com (Trustbase LTD, since 2020) and reviewglass.app (BRF Tech Solutions, Georgia US, registered 11.6.2026); neither shows a registered mark. reviewglass.dev, .io and .fi were not registered. Not a legal clearance: phonetic and figurative similarity, common-law rights and the full store listings need a professional search. The choice - keep the name, rename before P8, or a professional search first - is the owner's. The owner, 13.11.2026, while the Sunrise Software site's page for ReviewGlass was being written (site copy round S, item S.1): the name ReviewGlass is kept and stays, and the record that the name is unresolved is old.

## Decision

The name is ReviewGlass, and it stays (the owner, 13.11.2026). It is no longer a working name and no longer gates P8: the public release, its installer and its bundle identifier go out under ReviewGlass. The repository has been public since 17.9.2026 by the owner's decision, under the same name.

## Consequences

The README, CLAUDE.md, BUILD_INFO and the spec's D4 stop saying that P8 waits on the name. docs/NAME-CLEARANCE.md stays as the research record of 4.10.2026: the two e-commerce review services that trade as ReviewGlass and hold reviewglass.com and reviewglass.app remain a known overlap in use, and those two domains are theirs. A rename later would need a decision of its own.
