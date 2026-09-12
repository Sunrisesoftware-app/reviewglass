# ADR-0003: Desktop Code tab is primary; terminal CLI is a supported secondary surface

**Status:** Accepted
**Atlas id:** `adr.rg.003`
**Links:** `rg.session-source` (Session source (trait)), `rg.panel-window` (Panel window)

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

Both surfaces read the same ~/.claude/settings.json but expose different native features (the CLI has a native diff sidebar since 2.1.260, the Desktop Code tab has none), and it was unknown whether the documented statusLine channel runs on both. The P0 spike settled it on 2.1.268: a Desktop session started after the passthrough was configured, which produced assistant messages, wrote nothing, while a CLI session in the same directory three minutes later wrote on every message.

## Decision

One build, one installer configuration, both surfaces supported. statusLine reaches CLI sessions only; Desktop sessions are reached through their JSONL transcript, so rg.session-source ships BOTH implementations in v1 rather than one plus a conditional fallback. Every SessionSnapshot carries a surface marker taken from the transcript's entrypoint field.

## Consequences

The primary surface depends on the undocumented channel and the secondary one on the documented channel, which is the inverse of what the spec assumed; the transcript reader is therefore load-bearing rather than a fallback, and a change to the transcript format degrades the primary surface. The surface marker is measured rather than guessed: the statusLine payload carries no marker, and on 2.1.268 both surfaces write transcripts under ~/.claude/projects/ and both carry a scratchpad_dir, so the spec's earlier note about AppData/Roaming/Claude/claude-code-sessions/ does not hold. Transcripts yield token counts but no rate_limits, which is what forces adr.rg.009. P4 (live diff) is still kept as specified, because the native diff panel does not exist on the primary surface. Two smaller consequences for installer-integration: a session started before statusLine was configured never runs it, and claude -p runs none at all.
