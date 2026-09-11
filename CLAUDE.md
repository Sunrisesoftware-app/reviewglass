# ReviewGlass — notes for the build agent

Read `docs/REVIEWGLASS-SPEC.md` before changing architecture. Section 6.3 is the module
contract list; section 10 is this file's source.

## Rules

- Language: English for code, comments, commit messages and documentation.
- License: Apache-2.0. Never introduce a GPLv2-only dependency.
- Every consumer of Claude Code data assumes the field may be absent. Absent data **hides the
  element**; it never renders a zero, a dash or an error.
- `explain-service` contains no provider-specific code. A provider name outside
  `src-tauri/src/explain/backend` is a boundary violation.
- All configured Windows paths use forward slashes (Git Bash mangles backslashes).
- The glass window must call `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` or it captures
  itself.
- `rate_limits` is account-wide. Never present it as a per-session figure.
- ReviewGlass is a read surface. It never writes into a session, a repository or a transcript.
- Collector scripts always exit 0 and always print a status line.

## Roadmap gates

- **P1 (magnifier)** has no dependency on anything. Start here.
- **P2 (session panel)** does not begin until the P0 statusLine result is recorded for both
  Desktop and CLI in spec section 4.1.
- **P8 (public release)** waits on name clearance (adr.rg.008).

## Atlas

This system is `reviewglass` in the Atlas model (modules `rg.*`, decisions `adr.rg.*`). Keep the
model and the code in step: a new module, connection or decision here is a proposal there.

## Commands

```bash
pnpm install          # once
pnpm tauri dev        # both windows, hot reload
pnpm check            # svelte-check
cargo check --manifest-path src-tauri/Cargo.toml
pnpm tauri build      # NSIS + MSI under src-tauri/target/release/bundle/
```
