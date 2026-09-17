# Lessons

Observed pitfalls and what they taught. One entry per lesson, newest first, dated. A
lesson that becomes a rule moves into `CLAUDE.md`; a lesson that becomes a decision
becomes an ADR (`docs/adr/`, rendered from the Atlas model). This file keeps the ones
that are neither yet, and the story behind the ones that are.

## A verification that takes the mouse is an interruption (2026-09-17)

The release-build checks drive the app from outside: they launch it, move the cursor
with `SetCursorPos`, show the glass and the dock, and pick menu items by keyboard —
30 to 60 seconds each, several times in an evening. The owner was at the same machine,
in the middle of settings in a browser, and could not even reach the window's close
button until the run was stopped. Lesson: a live check is run at most once per change
and only after asking whether it is a good moment; everything that can be verified
without the screen — the replay tests on the recordings, the unit tests — comes first,
and a weak live assertion is fixed in the script, not re-run on the owner's mouse.

## A held value is not a confirmation: the wait must outlast the hold (2026-09-17)

The lock now holds a lost column for two seconds, and Fit waits before widening the
glass to a suspect column. Drafted with both at two seconds, the replay of the first
recording showed the 1497 px top-strip reading widening the glass anyway: the reading
had been lost, the lock was holding it, and the wait expired while the hold was still
supplying it. Lesson: when one mechanism holds a value and another waits for that
value to persist, the wait must be longer than the hold, or the hold confirms itself.
Fit's widen-wait is three seconds against a two-second hold.

## Ask the recording where a reading came from before tuning the reader (2026-09-17)

The glass leapt to 2252 px twice in the recording, on a 1497 px "column". It was easy
to read as the detector's 70 % rule being too loose (1497 is 58 % of the screen). The
cursor positions in the same lines said the readings came at y below 110 — the
owner's trips to the dock and the tab strip — where the band is a quarter title bar and
the sidebar's gutter is uniform in only 75 % of its slices, under the 80 % a plain
gutter needs, so sidebar and chat merged. Lesson: a log that carries the input's
context (here the cursor) turns a threshold question into a mechanism, and the fix
lands in the right place — the wait on a wide column, not a looser or a tighter rule
that would have moved the problem.

## A symptom can be the visible half of a bug you have not seen (2026-09-17)

"The still keeps the menu in the picture" read as a capture problem: freeze the frame
from before the menu. It was two bugs. The glass page assigned the canvas size on every
state change, and assigning a size wipes a canvas; a following glass repaints within a
frame, a still gets no frame, so the wipe was always refilled by the last frame still
pending in the engine — the one with the menu in it. Holding the picture while the menu
is open took that pending frame away and the wipe showed itself: a black still. Lesson:
when a fix makes a *different* symptom appear in the same place, the first symptom was
being masked, not caused, by what the fix removed; look for the state change that
clears something only a frame can restore.

## The owner's mouse is live during a test from outside (2026-09-17)

A verification script moved the cursor with `SetCursorPos`, read the lens's window rect,
and found the lens hundreds of pixels from where the cursor had been put: the owner was
using the machine, and every cursor placement lasted well under a second. The first run
failed five checks that compared a read from before a menu with one from after it.
Lesson: from the agent's side, a check that involves the cursor compares two reads taken
in the *same* state (both while the menu is open, both after the pick), never a read
from before a state change with one from after it; and it records where the cursor
actually was at each read, so a failure says whose hand moved it.

## A build that works right after building is not a build that works (2026-09-13)

The glass's first `invoke` raced the core's `setup`: config windows are created before
`setup` runs, and a webview that is quick enough calls `glass_state` before the state
is managed. A freshly compiled exe is never quick — WebView2 and Defender see to that —
so every check made right after a build passed, and every launch from the Desktop
shortcut afterwards failed (3 of 3 measured). Lesson: verify on a *warm* second and
third launch, not on the first one after the build; and a first call to the core
retries and says why it is waiting, instead of leaving an empty state that looks like
"still loading".

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
