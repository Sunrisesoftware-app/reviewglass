# ADR-0037: Updates are the user's to fetch: GitHub Releases and winget, and the app never asks the network for a version

**Status:** Accepted
**Atlas id:** `adr.rg.037`
**Links:** `rg.panel-window` (Panel (the dock's drawer)), `rg.installer-integration` (Installer integration), `reviewglass`

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

P7 packages ReviewGlass for other machines, and a packaged app needs a way to a newer version. The owner, 23.11.2026: an application that does not talk to the outside cannot fetch its updates, so they are fetched and installed by the user. The spec's privacy line allows one network path, the user-triggered Explain (adr.rg.023); Tauri's updater plugin would add a second, the app asking a server for versions, with a signing keypair whose loss would strand every installed copy. Windows already has a package manager that updates the rest of the machine (winget), and GitHub Releases is where the source already lives.

## Decision

ReviewGlass never fetches an update and never asks the network whether one exists. Each release is published on GitHub Releases with the NSIS installer; run over the installed version it upgrades in place (the same identifier; the running instance stopped first; the collectors replaced, adr.rg.038). The app's About, in Settings, shows its version and build and a button that opens the releases page in the default browser - the browser fetches, not the app. The release workflow also makes the winget manifest (wingetcreate), so winget upgrade updates ReviewGlass with the rest of the machine; its first submission belongs to P8. Tauri's updater plugin is not used; an in-app check, even opt-in, would be a second network path beside Explain and needs a decision of its own.

## Consequences

The privacy line holds without an exception: no analytics, no accounts, no listener and no update check (spec 8.4). A user who never looks does not learn of a new version from the app; the About's button and winget are the two ways, and the README says so. No update keypair exists to lose. Product decision D10 in the spec (v0.5, 3 and 8.2) is this decision.
