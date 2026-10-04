# Name clearance: "ReviewGlass"

**Date:** 2026-10-04 · **Gates:** P8, the public release (`adr.rg.008`, spec D4)
**Status:** a web research pass, read-only. **This is not a legal clearance.** Nothing
was registered, bought or submitted anywhere.

## Verdict

| Question | Verdict |
|---|---|
| Registered trademarks for "ReviewGlass" / "Review Glass" | **Clear** in every register that could be queried |
| The name in use by others | **Conflict in use**: two unrelated software businesses trade as ReviewGlass and hold the `.com` and the `.app` |
| Finnish company register | **Clear** |
| Package registries, GitHub, extension marketplaces, app stores | **Clear**, as far as checked |
| Implied affiliation with Anthropic | **Low**: the name contains neither "Claude" nor "Anthropic" |

Neither business appears to hold a registered mark, and both serve e-commerce reviews,
not developer tools, so the legal risk reads as moderate rather than high. The name's
search visibility and its obvious domains are taken, though, and both operators could
claim unregistered (common-law) rights in the US or the UK from prior use.

## 1. Trademarks

- **TMview** (EUIPO's portal, which also carries USPTO, PRH, UKIPO and WIPO/Madrid
  data; it states it is not an official register; each office's coverage was not
  verified): no results for `reviewglass`, `reviewglas`, `codeglass` or `reviewlens`.
  `review glass` returns only unrelated marks: PUREVIEW GLASS (Australia, classes 11/21)
  and an expired Czech mark, G NEW GLASS REVIEW (classes 16/35/41). Control searches
  returned USPTO marks, so the tool worked.
  <https://www.tmdn.org/tmview/#/tmview/results?page=1&pageSize=30&criteria=C&basicSearch=reviewglass>
- **Trademarkia** (a US mirror): no results for `reviewglass`, `codeglass` or `reviewlens`.
- **Google's GLASS** (USPTO 6380474, class 9) is live for wearable computer hardware,
  not software.
- **Not queried directly:** USPTO's own search (the page never ran a query), Justia
  (403), WIPO Global Brand Database (JavaScript only). Not covered: phonetic and
  figurative similarity, and unregistered rights.

## 2. The name in use

- **reviewglass.com**: a live "human-verified reviews" platform with Shopify,
  WooCommerce and Magento widgets and subscription pricing. Its terms name Trustbase LTD
  as the operator, "legal name to be finalized", dated 11.11.2025. The domain was
  registered 18.10.2020. No ® or ™ is shown.
- **reviewglass.app**: a live AI analysis of Amazon reviews, sold in credit packs. Its
  operator is BRF Tech Solutions, a Georgia (US) company; the footer reads "© 2026
  ReviewGlass", and it has an X account, @ReviewGlassApp. The domain was registered
  11.6.2026, before this repository.
- **GitHub:** the only `reviewglass` repository is this one; no user or organisation
  is named `reviewglass`.
- **Package names:** `reviewglass` and `review-glass` are free on npm, crates.io and
  PyPI; `reviewglass` is free on NuGet.
- **Marketplaces and stores:**
  - No match on the VS Code Marketplace, Open VSX, the Apple App Store, Google Play
    (a weak signal), the Shopify App Store or the winget/msstore catalogue.
  - The Chrome Web Store and the full Microsoft Store were not checked reliably.

## 3. Domains (RDAP)

| Domain | Status |
|---|---|
| reviewglass.com | taken, in use (Trustbase LTD) |
| reviewglass.app | taken, in use (BRF Tech Solutions) |
| reviewglass.dev, .io, .fi, .net, .org | not registered (404; control domains answered) |

A 404 is a good signal, not proof that a name can be bought: a registry may reserve or
price a name separately.

## 4. Finnish company register

The YTJ/PRH open data API has no company named "reviewglass" or "review glass".
<https://avoindata.prh.fi/opendata-ytj-api/v3/companies?name=reviewglass>

## 5. Other risks

- **"Glass" in software is crowded and weak as a mark.** Pickle's open-source "Glass" (a
  desktop AI assistant that "sees what you see" and hides from screen capture, popular
  since July 2025) is conceptually close; its mark was not searched. GLASSWIRE and
  GLASSDOOR are registered in software classes but are distinct compounds; Glassdoor
  makes a "review" + "glass" association, with low confusion.
- **Anthropic and Claude.** Anthropic's trademark guidelines bar uses that imply
  sponsorship or affiliation. "For Claude Code" or "a companion for Claude Code" in
  plain text, with the README's non-affiliation note and no Claude logo or brand
  assets, is the usual pattern. The release listing should be checked against
  <https://www.anthropic.com/legal/trademark-guidelines>.

## What remains for a professional

- A full search of the USPTO, EUIPO, PRH and WIPO registers, including phonetic and
  figurative similarity, in classes 9, 42 and 35.
- A common-law use assessment of the two existing ReviewGlass businesses, and whether
  either has filed a mark not yet published.
- The Chrome Web Store and the Microsoft Store storefronts.
- Who owns the unregistered domains, and their price.

## Fallback names (quick checks only, not cleared)

| Name | TMview | YTJ | GitHub | Domains |
|---|---|---|---|---|
| **SessionGlass** | 0 | 0 | 0 repos | `.app`, `.dev`, `.io`, `.fi` free; `.com` registered (use not checked); free on npm and crates.io |
| **DiffGlass** | one hit: paints, class 2 (India) | 0 | 0 repos | `.app`, `.dev`, `.io`, `.fi` free; `.com` registered (use not checked); free on npm and crates.io |
| **CodeLoupe** | not searched | 0 | 5 repos | `.com`, `.dev`, `.io` registered; `.app`, `.fi` free (weaker) |

Every "Glass" name stays in a crowded field; a coined word would clear more easily.

## The decision this asks for

The owner decides, and the decision enters the Atlas model (`adr.rg.008`, still
*proposed*) before the bundle identifier, the installer or a store listing changes:

1. **Keep ReviewGlass.** The registers are clear and the markets differ. Use a domain
   that is free, `.dev` or `.fi`, and accept the shared name in search results.
2. **Rename before P8.** SessionGlass or DiffGlass, or a coined word, after a
   professional search.
3. **Commission the professional search first**, and decide on its answer.
