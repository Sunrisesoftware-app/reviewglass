# ADR-0038: The Claude Code integration is a visible step in the app, with an undo, and the uninstaller asks the app to reverse it

**Status:** Accepted
**Atlas id:** `adr.rg.038`
**Links:** `rg.installer-integration` (Installer integration), `rg.statusline-collector` (statusLine collector), `rg.hook-collector` (PostToolUse hook collector), `rg.panel-window` (Panel (the dock's drawer)), `rg.config-store` (Config store), `reviewglass`

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

ReviewGlass's data comes through two collectors Claude Code runs (adr.rg.010): the statusLine collector and the PostToolUse hook, configured in the user's ~/.claude/settings.json and placed under ~/.reviewglass/bin (adr.rg.019). On the owner's machine they were placed and configured by hand. The spec's installer-integration contract asks for a backup of the settings file, a question before an existing statusLine is replaced with the offer to chain it, reversal by the uninstaller, warnings for workspace trust and for disableAllHooks or allowManagedHooksOnly, and the word that running sessions keep the status line they started with. Editing JSON inside an NSIS script is fragile and invisible, and writing into a user's Claude Code settings without their seeing it goes against the golden rule that ReviewGlass is a read surface. The owner approved the P7 plan on 23.11.2026.

## Decision

The installer places files only. The integration is a step in the app: Settings > Claude Code shows whether the collectors are in place and current and whether settings.json carries ReviewGlass's statusLine and PostToolUse entries, and the first start on a machine without them opens there. Connect shows the change before it is made - the entries, the backup's name, an existing statusLine and the choice to chain it or replace it, the warnings for trust and for disableAllHooks or allowManagedHooksOnly, and that running sessions keep their status line until restarted - then copies the collectors from the install folder to ~/.reviewglass/bin, writes settings.json.bak-reviewglass beside the settings, and adds the entries with an atomic write that keeps every other key as it was. Disconnect removes exactly ReviewGlass's entries, restoring a chained statusLine to the user's own command, and leaves the collectors' folder. At every start the app replaces a collector in ~/.reviewglass/bin that differs from the one it ships, so an upgrade needs no step of its own. The uninstaller runs reviewglass.exe --remove-integration, which the app handles before Tauri starts, so neither a running instance nor the single-instance rule (adr.rg.015) stands in its way; it disconnects and removes ~/.reviewglass/bin, and leaves the spool and the logs.

## Consequences

Nothing is written into a user's Claude Code settings unseen, and every write has an undo in the same place. A configuration made by hand, as on the owner's machine, is recognised as connected rather than doubled. A chained statusLine runs the user's own command through the collector, so their line still shows. The installer stays a plain NSIS installer with one call on uninstall. The step is new UI and new code in the app; it is tested on a settings file of every shape the contract names (absent, empty, with a statusLine, with hooks of the user's own) before P7 ships.
