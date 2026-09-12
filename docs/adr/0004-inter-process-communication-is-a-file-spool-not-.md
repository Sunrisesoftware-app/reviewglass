# ADR-0004: Inter-process communication is a file spool, not a network listener

**Status:** Accepted
**Atlas id:** `adr.rg.004`
**Links:** `rg.spool` (File spool), `rg.spool-watcher` (Spool watcher), `rg.statusline-collector` (statusLine collector), `rg.hook-collector` (PostToolUse hook collector)

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

Two processes (Claude Code's collector scripts and the ReviewGlass app) must exchange session and change data on one machine. A localhost port or socket would be the conventional answer.

## Decision

Plain files under the user profile: atomic session records and append-only event files, watched with notify. No listener, no port, no socket.

## Consequences

The simplest implementation and the strongest privacy claim for the public repository. N writers and one reader without locking; readers tolerate partial files by retrying once; closed sessions are detected by record age, since Claude Code emits no close event.
