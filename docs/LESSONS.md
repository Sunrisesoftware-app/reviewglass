# Lessons

Observed pitfalls and what they taught. One entry per lesson, newest first, dated. A
lesson that becomes a rule moves into `CLAUDE.md`; a lesson that becomes a decision
becomes an ADR (`docs/adr/`, rendered from the Atlas model). This file keeps the ones
that are neither yet, and the story behind the ones that are.

## A release webview is not a black box: WebView2 remote debugging shows it (2026-09-13)

A CSP without `connect-src ipc: http://ipc.localhost` blocks Tauri's IPC over the custom
protocol; the JS side logs a warning and falls back to postMessage, and whether the
first calls survive depends on how many are in flight. In dev everything worked; the
release glass showed "Waiting for the first frame…" and nothing else, and the Rust side
had nothing to say because nothing ever reached it. What showed it in a minute, after an
hour of guessing: launch the release exe with
`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9222 --remote-allow-origins=*"`,
read `http://127.0.0.1:9222/json`, and speak CDP over the page's websocket —
`Runtime.evaluate` for DOM state, `Runtime.enable` + `Log.enable` + `Page.reload` for
the console and CSP violations. Lesson: when a release build behaves differently from
dev and the Rust side sees nothing, look inside the webview first, and add
`connect-src ipc: http://ipc.localhost` to any CSP that names `default-src`.

## A window that is excluded from capture cannot be verified by capture — and a fresh binary is not running when its process is (2026-09-13)

Verifying the viewfinder from the build agent's side: `FindWindow` and `GetWindowRect`
show where the glass and the finder are and whether they are visible, but no screenshot
path — `CopyFromScreen`, `PrintWindow`, Windows.Graphics.Capture — shows what they
draw, because `WDA_EXCLUDEFROMCAPTURE` is exactly what they are set to. The first
read-back after a rebuild then said the finder was hidden and the glass unfitted, and an
hour went into a bug that did not exist: a freshly built exe takes over ten seconds to
show its first frame (WebView2 start-up plus Defender scanning a new binary), and the
windows were read eight seconds after launch. The rider and the halo were already up,
which made the state look settled. Lesson: verify geometry by reading window rects back,
verify pixels by asking the owner, and wait for the first frame — not for a timer —
before reading anything. A debug line written to a file under `%TEMP%` was what
finally showed the pipeline whole; `eprintln!` reaches nowhere from a
`windows_subsystem = "windows"` release build.

## A merge command does not wait for CI unless the branch says so (2026-09-13)

`gh pr merge` on the Atlas repo went through with the model guard red: the branch has
no required check, and the guard had a real finding — a decision id renumbered is, to
the guard, a decision lost. Lesson: read `gh pr checks` before merging, every time, and
treat a rename of a model id as a retirement that `model/retired.json` records first.

## Persisting a derived size as the setting it was derived from shrinks the window on every restart (2026-09-12)

`glass_set_view` stored the canvas size — the picture area — as the window's size, and
the canvas is smaller than the window by the border and, from session 1 onward, the
title bar. Every restart and every mode switch set the window to the canvas size, which
made the next canvas smaller still. Measured: a 900×340 window had drifted to 632×352
before anyone noticed. Lesson: a value that is *computed from* a setting is never
written back *as* that setting. The window size is stored only when the user resizes,
from the window's own inner size, into the slot of the current mode (adr.rg.013).

## An optimisation keyed on "did the screen change" is wrong when the question moved (2026-09-12)

The dirty-region skip — no crop when the compositor reports nothing dirty inside the
source rectangle — took idle CPU from 14 % to 1.35 % of a core. It was also wrong
whenever the rectangle itself moved: nothing on screen had changed, so nothing was
dirty, and the crop was skipped although the rectangle now covered different pixels.
The lens "vibrated" and follow mode lagged behind the cursor. Lesson: a skip that reads
"the input did not change" must include every input, and the rectangle's position is
one. The skip now applies only while the rectangle is where it was.

## A hideable window without a fixed point becomes two processes (2026-09-12)

Both windows could be hidden and nothing showed the app was still running. The user
"closed" it, it kept running, a second launch started a second copy, and two instances
were measured alive at once. Lesson: the moment a window can hide, the app needs a place
it is always visible from and a rule that a second launch is not a second copy. Tray
icon and single instance (adr.rg.015). The installer inherits the rule.

## A value from a shell command is not a value until the shell is known (2026-09-11)

The spec described the collectors as scripts and, in the same breath, warned that
Windows runs status-line commands through Git Bash when installed and PowerShell
otherwise, and that Git Bash eats unquoted backslashes. A script would have had to be
written for a shell it could not know it was running in. Lesson: when a document warns
about the fragility of its own choice, the warning is the design input. Native binaries
(adr.rg.010).

## "Every field may be absent" does not cover a field that is present and the wrong shape (2026-09-11)

`context_window.current_usage` is an object of token counts where the spec described a
number. With a plain `Option<u64>` the one mismatched field failed the whole payload,
so the panel lost the session, the model, the cost and the account quota over a field
nothing reads. Lesson: optional fields survive an absence and do nothing about a type
change, and the second is the one that actually happens between versions of someone
else's product. Every field is parsed leniently (adr.rg.011), and the collector reports a
parse failure under `REVIEWGLASS_DEBUG` so a quiet failure is not mistaken for a quiet
machine.

## A payload with no surface marker is not evidence about the surface (2026-09-11)

The P0 spike's first reading said `statusLine` fires in the Desktop Code tab. It does
not. The sessions in the log were terminal CLI sessions, assumed to be Desktop ones
because a Desktop tab had been opened; the statusLine payload carries no marker, and on
2.1.268 both surfaces write transcripts to the same place with the same
`scratchpad_dir`. The transcript's `entrypoint` field is the only thing that separates
them, and it settled the question the other way. Lesson: before reading a measurement
as being about X, check that something in the measurement says X. Recorded in
`atlas_record_decision` as `disagreed`, with the wrong first reading as a signal.

## A session only runs the status line that existed when it started (2026-09-11)

The spike was configured while this Desktop session was open, and this session never
ran it, though it produced dozens of assistant messages afterwards. Settings are read
at session start. Lesson for the installer: say plainly that running sessions are
unaffected until restarted, or the user will conclude the installation failed.

## A shell script that prints nothing blanks the user's status line (2026-09-11)

`statusLine` is a single-value setting; ReviewGlass takes it over entirely. The
collector must print a line on every path, including an empty stdin and a payload it
cannot parse — a fixed minimal line beats a blank one. Contract rule in
`rg.statusline-collector`.
