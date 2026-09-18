# ADR-0019: The spool lives under the profile root, not AppData: the desktop app is packaged and virtualises AppData for its children

**Status:** Accepted
**Atlas id:** `adr.rg.019`
**Links:** `rg.spool` (File spool), `rg.statusline-collector` (statusLine collector), `rg.hook-collector` (PostToolUse hook collector), `rg.spool-watcher` (Spool watcher), `rg.installer-integration` (Installer integration)

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

The spool was %APPDATA%/ReviewGlass/spool. On 18.9.2026 the first live diff (P4) worked in every check the build agent ran and showed nothing for the owner: the hook had fired and the event file existed from the agent's shell, while the owner's app, launched from its shortcut, read an empty directory at the same path. The Claude desktop app is a packaged (MSIX) application, and Windows virtualises AppData for a packaged process and every child it starts: the Desktop Code tab's sessions, the hooks and collectors they run, and the build agent's own shell all wrote %APPDATA%/ReviewGlass into %LOCALAPPDATA%/Packages/Claude_…/LocalCache/Roaming, visible only to processes with the package identity; an app started from Explorer read the real Roaming, where the directory did not exist. The app's own config directory escaped because the real app had created it first — virtualisation merges what exists and redirects what is new. The profile root is not virtualised: ~/.claude, written by the same sessions, is real for everyone.

## Decision

The spool is ~/.reviewglass/spool — the profile root, beside the collectors' binaries under ~/.reviewglass/bin — and nothing ReviewGlass shares between a Desktop session's child process and the app is ever placed under AppData. One function (spool.rs, shared by the app and both collectors) names the path. The app's own configuration stays where Tauri puts it: the app creates it itself, so it is real. Spec 6.1's 'plain files under the user profile' is now literally true and is stated as the path in 6.3 at v3.

## Consequences

The app and both collectors change together and the collectors are reinstalled; spool data under the package cache is abandoned (stale triggers and one session record from the P0 spike). The installer (P7) places the spool nowhere under AppData and says so. A verification run from the build agent's shell proves the mechanism, not the delivery, because that shell is itself a child of the packaged app: from now on the app under test is launched the way the owner launches it (Explorer, or a plain PowerShell), and a check that has passed only from the agent's shell is not a pass. The same boundary explains why the P0 spike's session record was visible to the agent and never to a plain process, which nobody noticed because the gauge is borrowed from any live CLI session (adr.rg.009) and none had been run outside the desktop app.
