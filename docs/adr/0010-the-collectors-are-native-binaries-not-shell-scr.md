# ADR-0010: The collectors are native binaries, not shell scripts

**Status:** Accepted
**Atlas id:** `adr.rg.010`
**Links:** `rg.statusline-collector` (statusLine collector), `rg.hook-collector` (PostToolUse hook collector)

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

Spec section 6.3 describes statusline-collector and hook-collector as scripts, and warns in the same breath that on Windows Claude Code runs status-line commands through Git Bash when it is installed and PowerShell otherwise, and that Git Bash consumes unquoted backslashes and silently mangles Windows paths. A script therefore has to be written twice, or written once in a shell that may not be the one that runs it, and its own path has to survive a shell that eats part of it.

## Decision

Both collectors ship as small native binaries built from the same crate as the app (reviewglass-statusline, and later the hook collector). Claude Code invokes the executable directly, so no shell interprets the command or its path.

## Consequences

The shell question disappears rather than being worked around: there is no Git Bash / PowerShell branch, no quoting rule to remember, and the forward-slash discipline in the module contract becomes a convention rather than a correctness requirement. Startup is single-digit milliseconds against tens for a shell, which matters inside a 300 ms debounce that runs on every assistant message. The collectors share the app's payload types, so the parser that survives a field changing shape is the same code in both places and cannot drift. The cost is that the collectors are platform binaries: a macOS port (v2) rebuilds them rather than copying a script, and the crate now declares default-run because two binaries ship. The contract in section 6.3 is unchanged — it was always about behaviour (always print, always exit 0, finish fast, write atomically), never about language.
