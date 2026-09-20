# Architecture decision records

One file per decision, rendered from the Atlas model's `decisions[]` for system
`reviewglass`. The model is the source of truth; these files exist so the decisions
are readable in the repository without Atlas. Regenerate with:

```bash
node scripts/adr-from-model.mjs
```

A new decision is added to the model (a proposal or a PR against the Atlas repo), not
here. A hand edit here is lost on the next run.

- [ADR-0001](0001-license-is-apache-2-0.md) — License is Apache-2.0 *(accepted)*
- [ADR-0002](0002-explain-backend-is-a-pluggable-interface-with-tw.md) — Explain backend is a pluggable interface with two shipped implementations *(accepted)*
- [ADR-0003](0003-desktop-code-tab-is-primary-terminal-cli-is-a-su.md) — Desktop Code tab is primary; terminal CLI is a supported secondary surface *(accepted)*
- [ADR-0004](0004-inter-process-communication-is-a-file-spool-not-.md) — Inter-process communication is a file spool, not a network listener *(accepted)*
- [ADR-0005](0005-tauri-v2-with-a-rust-core-not-electron.md) — Tauri v2 with a Rust core, not Electron *(accepted)*
- [ADR-0006](0006-rectangle-only-magnifier-pixels-only-no-ocr.md) — Rectangle-only magnifier, pixels only, no OCR *(accepted)*
- [ADR-0007](0007-account-wide-quota-and-per-session-attribution-a.md) — Account-wide quota and per-session attribution are two quantities, never blended *(accepted)*
- [ADR-0008](0008-name-availability-is-unresolved-and-gates-public.md) — Name availability is unresolved and gates public release only *(proposed)*
- [ADR-0009](0009-the-account-quota-gauge-is-borrowed-from-any-liv.md) — The account quota gauge is borrowed from any live CLI session *(accepted)*
- [ADR-0010](0010-the-collectors-are-native-binaries-not-shell-scr.md) — The collectors are native binaries, not shell scripts *(accepted)*
- [ADR-0011](0011-every-claude-code-field-tolerates-a-wrong-type-n.md) — Every Claude Code field tolerates a wrong type, not only an absence *(accepted)*
- [ADR-0012](0012-freeze-is-a-still-not-a-locked-live-region.md) — Freeze is a still, not a locked live region *(accepted)*
- [ADR-0013](0013-the-glass-is-a-window-a-permanent-title-bar-and-.md) — The glass is a window: a permanent title bar and three named modes *(accepted)*
- [ADR-0014](0014-a-cursor-halo-marks-the-pointer-on-screen-while-.md) — A cursor halo marks the pointer on screen while the glass follows it *(accepted)*
- [ADR-0015](0015-a-hideable-app-stays-findable-a-tray-icon-and-a-.md) — A hideable app stays findable: a tray icon and a single instance *(accepted)*
- [ADR-0016](0016-the-usage-loop-runs-on-its-own-thread-not-on-the.md) — The usage loop runs on its own thread, not on the panel's poll *(accepted)*
- [ADR-0017](0017-the-glass-reads-the-pane-column-boundaries-from-.md) — The glass reads the pane: column boundaries from pixels, structure never content *(accepted)*
- [ADR-0018](0018-the-dock-is-the-control-panel-and-the-fixed-poin.md) — The dock is the control panel and the fixed point: the glass hangs from it *(accepted)*
- [ADR-0019](0019-the-spool-lives-under-the-profile-root-not-appda.md) — The spool lives under the profile root, not AppData: the desktop app is packaged and virtualises AppData for its children *(accepted)*
- [ADR-0020](0020-the-panel-is-the-dock-s-drawer-in-the-dock-s-own.md) — The panel is the dock's drawer, in the dock's own window: one window that grows, not two that can drift apart *(accepted)*
- [ADR-0021](0021-the-drawer-is-sized-by-hand-from-its-free-corner.md) — The drawer is sized by hand from its free corner, and the size is remembered *(accepted)*
