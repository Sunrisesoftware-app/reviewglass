<script lang="ts">
  // The Diff tab (P4): what the agent changed, as git sees it, within a second of the
  // edit. A list of the files touched since ReviewGlass started, newest first, and the
  // selected file's diff rendered by diff2html (an existing renderer, spec 6.2). Every
  // empty state names its reason: no hook, no edit yet, a file git cannot show.
  //
  // P4b: the whole file, read-only, around a change. Opened from the header's toggle
  // or from a hunk's header, it shows the working copy with the last diff's added
  // lines tinted and a cut mark where lines were removed; ‹ › walk the changes. The
  // hunk view stays the default: picking another file returns to it.
  //
  // The latest edit in a colour of its own (the owner's wish, 23.9.2026): the lines the
  // latest edit brought carry ReviewGlass's highlighter in every view, older uncommitted
  // work stays diff-green, and a legend says which is which. "New" shows the new code
  // alone. A mark on ReviewGlass's own view; the files are only read.
  //
  // With no file clicked, the Hunks view follows the newest edit. File and New pin the
  // file they opened, so an edit elsewhere does not take the view away.
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { html as diffHtml } from "diff2html";
  import "diff2html/bundles/css/diff2html.min.css";
  import type { Baseline, DiffTab, DiffView, FileView } from "./types";
  import { selection, choose } from "./selection.svelte";

  let tab = $state<DiffTab | null>(null);
  let selected = $state<string | null>(null);
  let failure = $state<string | null>(null);
  let now = $state(Date.now());

  /** "hunks" (the default), "file" (P4b) or "new" (the new code alone). */
  let mode = $state<"hunks" | "file" | "new">("hunks");
  let file = $state<FileView | null>(null);
  let fileFailure = $state<string | null>(null);
  /** Which of the file's changes the view last went to (0-based), for ‹ ›. */
  let at = $state(0);
  let viewEl = $state<HTMLElement | null>(null);

  /** The views on show: every session's, or the chosen session's alone. */
  const shown = $derived<DiffView[]>(
    tab
      ? tab.views.filter((v) => selection.session === null || v.session_id === selection.session)
      : [],
  );
  const current = $derived<DiffView | null>(
    shown.find((v) => v.path === selected) ?? shown[0] ?? null,
  );
  const rendered = $derived(
    current?.unified
      ? diffHtml(current.unified, {
          drawFileList: false,
          matching: "lines",
          outputFormat: "line-by-line",
        })
      : "",
  );

  /** The file's lines; a trailing newline is not an extra empty line. */
  const lines = $derived.by(() => {
    if (!file?.text) return [] as string[];
    const ls = file.text.split(/\r?\n/);
    if (ls.length > 0 && ls[ls.length - 1] === "") ls.pop();
    return ls;
  });
  const added = $derived(new Set(file?.added ?? []));
  const cuts = $derived(new Set(file?.removed_before ?? []));
  /** The lines the latest edit brought: from the file when one is open, else the diff. */
  const fresh = $derived(new Set(mode !== "hunks" && file ? file.fresh : (current?.fresh ?? [])));
  const freshFirst = $derived(
    !(mode !== "hunks" && file ? file.fresh_from_previous : (current?.fresh_from_previous ?? true)),
  );
  /** Added lines that are not the latest edit's: older, uncommitted work. */
  const older = $derived(
    mode !== "hunks" && file
      ? file.added.filter((l) => !fresh.has(l)).length
      : Math.max(0, (current?.added ?? 0) - (current?.fresh.length ?? 0)),
  );

  /** The new code alone: runs of consecutive fresh lines, each with its first line. */
  const blocks = $derived.by(() => {
    const out: { start: number; lines: string[] }[] = [];
    for (const n of [...fresh].sort((a, b) => a - b)) {
      if (n < 1 || n > lines.length) continue;
      const last = out[out.length - 1];
      if (last && last.start + last.lines.length === n) last.lines.push(lines[n - 1]);
      else out.push({ start: n, lines: [lines[n - 1]] });
    }
    return out;
  });

  async function refresh() {
    try {
      tab = await invoke<DiffTab>("panel_diffs");
      failure = null;
    } catch (e) {
      failure = e instanceof Error ? e.message : String(e);
    }
  }

  /** Another file from the list: its hunks, the default view. */
  function pick(path: string) {
    selected = path;
    mode = "hunks";
    file = null;
  }

  /** Read the working copy of the selected file. Nothing is written. */
  async function loadFile(): Promise<FileView | null> {
    if (!current) return null;
    try {
      const f = await invoke<FileView>("panel_file_view", { path: current.path });
      fileFailure = null;
      file = f;
      return f;
    } catch (e) {
      fileFailure = e instanceof Error ? e.message : String(e);
      return null;
    }
  }

  async function goTo(line: number) {
    await tick();
    viewEl?.querySelector(`[data-line="${line}"]`)?.scrollIntoView({ block: "center" });
  }

  /** Open the whole file at `line`, or at its first change. The file is pinned. */
  async function openFile(line?: number) {
    if (current) selected = current.path;
    const f = await loadFile();
    mode = "file";
    if (!f) return;
    let idx = 0;
    if (line != null) {
      f.hunks.forEach((h, k) => {
        if (h.start <= line) idx = k;
      });
    }
    at = idx;
    await goTo(line ?? f.hunks[0]?.start ?? 1);
  }

  /** The new code alone, from the top. The file is pinned. */
  async function openNew() {
    if (current) selected = current.path;
    await loadFile();
    mode = "new";
    await tick();
    viewEl?.scrollTo({ top: 0 });
  }

  function step(d: number) {
    if (!file || file.hunks.length === 0) return;
    at = (at + d + file.hunks.length) % file.hunks.length;
    void goTo(file.hunks[at].start);
  }

  /** The list changed: re-read an open file (it is pinned), or fall back to the hunks
   *  if it has left the list or the session filter. */
  async function onUpdate() {
    await refresh();
    if (mode === "hunks") return;
    if (current && file && current.path === file.path) await loadFile();
    else {
      mode = "hunks";
      file = null;
    }
  }

  $effect(() => {
    // Each hunk header in diff2html's rendering opens the whole file at that hunk:
    // by click, or by Enter/Space once tabbed to. The rows are diff2html's, so they
    // are wired here, after the render, and unwired before the next one. `mode` is
    // read so the wiring repeats when the hunks come back after the file view.
    if (mode !== "hunks" || !rendered || !viewEl) return;
    const undo: (() => void)[] = [];
    // diff2html marks the header's cells `d2h-info` (the line-number cell empty, the
    // other holding the `@@` text); the row is the control.
    const rows = new Set<HTMLElement>();
    viewEl.querySelectorAll("td.d2h-info").forEach((cell) => {
      const row = cell.closest("tr");
      if (row) rows.add(row as HTMLElement);
    });
    rows.forEach((row) => {
      const m = /\+(\d+)/.exec(row.textContent ?? "");
      if (!m) return;
      const line = Number(m[1]);
      const open = () => void openFile(line);
      const key = (e: KeyboardEvent) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          open();
        }
      };
      row.title = "Open the whole file here";
      row.setAttribute("role", "button");
      row.tabIndex = 0;
      row.addEventListener("click", open);
      row.addEventListener("keydown", key);
      undo.push(() => {
        row.removeEventListener("click", open);
        row.removeEventListener("keydown", key);
      });
    });
    return () => undo.forEach((u) => u());
  });

  $effect(() => {
    // The latest edit's lines in the rendered hunks: diff2html's added rows whose
    // new-side number is fresh carry ReviewGlass's highlighter. Cleared first, so a
    // change of the fresh set alone repaints too.
    const set = fresh;
    if (mode !== "hunks" || !rendered || !viewEl) return;
    viewEl.querySelectorAll("tr.rg-fresh").forEach((r) => r.classList.remove("rg-fresh"));
    viewEl.querySelectorAll("td.d2h-code-linenumber.d2h-ins").forEach((cell) => {
      const n = Number(cell.querySelector(".line-num2")?.textContent?.trim());
      if (n && set.has(n)) cell.closest("tr")?.classList.add("rg-fresh");
    });
  });

  function ago(ms: number) {
    const s = Math.max(0, Math.round((now - ms) / 1000));
    if (s < 5) return "just now";
    if (s < 60) return `${s} s ago`;
    const m = Math.round(s / 60);
    if (m < 60) return `${m} min ago`;
    return `${Math.round(m / 60)} h ago`;
  }

  /** The one-word status for the list, and the reason for the detail. */
  function label(v: DiffView) {
    const counts = `+${v.added ?? 0} −${v.removed ?? 0}`;
    switch (v.status) {
      case "changed":
        return counts;
      case "untracked":
        if (v.baseline === "last-edit") return v.unified ? counts : "unchanged";
        return `new, +${v.added ?? 0}`;
      case "unchanged":
        return "unchanged";
      case "not-in-repo":
        if (v.baseline === "last-edit") return v.unified ? counts : "unchanged";
        return v.unified ? `outside git, +${v.added ?? 0}` : "not in a repository";
      case "denied":
        return "not shown";
      case "missing":
        return "gone";
      case "too-large":
        return "too large";
      case "binary":
        return "binary";
      case "git-failed":
        return "git failed";
    }
  }

  /** What the marks are measured against, for the header. */
  function since(b: Baseline) {
    switch (b) {
      case "head":
        return "since the last commit";
      case "last-edit":
        return "since the previous edit";
      case "whole-file":
        return "the whole file, new";
    }
  }

  onMount(() => {
    void refresh();
    const tick = setInterval(() => (now = Date.now()), 5000);
    let unlisten: (() => void) | undefined;
    void listen("diff:update", () => void onUpdate()).then((u) => (unlisten = u));
    return () => {
      clearInterval(tick);
      unlisten?.();
    };
  });
</script>

{#if failure}
  <p class="state bad">The core is not answering: {failure}</p>
{:else if !tab}
  <p class="state">Reading…</p>
{:else if !tab.hook_installed}
  <p class="state">
    The hook collector has not run yet. Live diff needs <code>reviewglass-hook.exe</code>
    configured as a <code>PostToolUse</code> hook in <code>~/.claude/settings.json</code>;
    until then no edit reaches this tab.
  </p>
{:else if tab.views.length === 0}
  <p class="state">
    No edit since ReviewGlass started. When Claude Code edits a file, its diff appears
    here within a second — in Desktop and CLI sessions alike.
  </p>
{:else if shown.length === 0}
  <p class="state">
    No edit from <strong>{selection.name}</strong> since ReviewGlass started; the other
    sessions have {tab.views.length}.
    <button class="link" onclick={() => choose(null)}>Show all sessions</button>
  </p>
{:else}
  {#if selection.session !== null}
    <p class="filter">
      Only <strong>{selection.name}</strong>{selection.by === "follow" ? ", chosen by Follow," : ""} ({shown.length} of {tab.views.length})
      <button class="link" onclick={() => choose(null)}>show all sessions</button>
    </p>
  {/if}
  <div class="diff" class:filtered={selection.session !== null}>
    <ul class="files" aria-label="Files the agent changed, newest first">
      {#each shown as v (v.path)}
        <li>
          <button
            class:active={current?.path === v.path}
            onclick={() => pick(v.path)}
            title={v.path}
          >
            <span class="path">{v.display_path}</span>
            <span class="meta">
              <span class="stat" class:muted={!v.unified}>{label(v)}</span>
              <span class="when">{v.tool ?? "edit"} · {ago(v.at_ms)}</span>
            </span>
          </button>
        </li>
      {/each}
    </ul>
    <section class="view" bind:this={viewEl}>
      {#if current}
        <header>
          <span class="path" title={current.path}>{current.display_path}</span>
          {#if mode !== "hunks"}
            <span class="root">working copy · read-only</span>
          {:else if current.repo_root}
            <span class="root" title={current.repo_root}>{current.repo_root}</span>
          {/if}
          {#if current.baseline && mode !== "new" && (mode === "file" ? file?.hunks.length : current.unified)}
            <span class="root">{since(current.baseline)}</span>
          {/if}
          <span class="controls">
            {#if mode === "file" && file && file.hunks.length > 0}
              <span class="nav">
                <button onclick={() => step(-1)} title="Previous change" aria-label="Previous change">‹</button>
                <span class="count">{at + 1}/{file.hunks.length}</span>
                <button onclick={() => step(1)} title="Next change" aria-label="Next change">›</button>
              </span>
            {/if}
            <span class="toggle" role="group" aria-label="Hunks, the whole file, or the new code alone">
              <button class:active={mode === "hunks"} aria-pressed={mode === "hunks"} onclick={() => (mode = "hunks")}>
                Hunks
              </button>
              <button
                class:active={mode === "file"}
                aria-pressed={mode === "file"}
                onclick={() => void openFile()}
                title="The whole file, read-only, at its first change"
              >
                File
              </button>
              <button
                class:active={mode === "new"}
                aria-pressed={mode === "new"}
                onclick={() => void openNew()}
                title="Only the new code: the lines the latest edit brought, as they stand now"
              >
                New
              </button>
            </span>
          </span>
        </header>
        {#if fresh.size > 0}
          <!-- What the colours mean. The highlighter is ReviewGlass's own mark. -->
          <p class="legend">
            <span class="swatch fresh" aria-hidden="true"></span>
            {freshFirst ? "new — the first edit ReviewGlass saw of this file" : "new in the latest edit"}
            ({fresh.size} {fresh.size === 1 ? "line" : "lines"})
            {#if older > 0 && mode !== "new"}
              <span class="swatch older" aria-hidden="true"></span> earlier, not yet committed
            {/if}
          </p>
        {/if}
        {#if mode !== "hunks"}
          {#if fileFailure}
            <p class="state bad">The core is not answering: {fileFailure}</p>
          {:else if !file}
            <p class="state">Reading…</p>
          {:else if file.status !== "shown" || file.text === null}
            <p class="state">{file.reason ?? "Nothing to show."}</p>
          {:else if mode === "new"}
            {#if blocks.length === 0}
              <p class="state">
                {file.fresh_from_previous
                  ? "The latest edit brought no new lines: it only removed some, or wrote the same text again."
                  : "Nothing new to show in this file."}
                <button class="link" onclick={() => void openFile()}>Open the whole file</button>
              </p>
            {:else}
              <div class="file new" role="document" aria-label="The new code alone, read-only">
                {#each blocks as b, k (b.start)}
                  {#if k > 0}
                    <div class="gap" aria-hidden="true">⋯ lines {blocks[k - 1].start + blocks[k - 1].lines.length}–{b.start - 1} unchanged</div>
                  {/if}
                  {#each b.lines as text, j (b.start + j)}
                    <div class="ln fresh" data-line={b.start + j}>
                      <span class="n">{b.start + j}</span><span class="t">{text}</span>
                    </div>
                  {/each}
                {/each}
              </div>
            {/if}
          {:else}
            <div class="file" role="document" aria-label="The whole file, read-only">
              {#each lines as text, i (i)}
                <div
                  class="ln"
                  class:added={added.has(i + 1)}
                  class:fresh={fresh.has(i + 1)}
                  class:cut={cuts.has(i + 1)}
                  class:target={file.hunks[at]?.start === i + 1}
                  data-line={i + 1}
                  title={cuts.has(i + 1) ? "Lines removed above this one" : undefined}
                >
                  <span class="n">{i + 1}</span><span class="t">{text}</span>
                </div>
              {/each}
              {#if cuts.has(lines.length + 1)}
                <div class="ln cut end" data-line={lines.length + 1} title="Lines removed at the end"></div>
              {/if}
            </div>
          {/if}
        {:else if current.unified}
          <div class="rendered">{@html rendered}</div>
        {:else}
          <p class="state">{current.reason ?? "Nothing to show."}</p>
        {/if}
      {/if}
    </section>
  </div>
  {#if tab.unreadable > 0}
    <p class="state">{tab.unreadable} event file(s) could not be read on the last pass.</p>
  {/if}
{/if}

<style>
  .state {
    color: var(--muted);
    max-width: 60ch;
  }
  .state code {
    padding: 1px 4px;
    border-radius: 3px;
    background: var(--line);
  }
  .bad {
    color: var(--bad);
  }
  /* The file list and the hunk side by side, each scrolling on its own inside the
     drawer's height. The list stops growing at 260 px, so a drawer dragged wider
     (adr.rg.021) gives the width to the code. */
  .diff {
    display: grid;
    grid-template-columns: minmax(170px, min(30%, 260px)) minmax(0, 1fr);
    gap: 12px;
    height: 100%;
    min-height: 0;
  }
  .diff.filtered {
    height: calc(100% - 26px);
  }
  .filter {
    margin: 0 0 6px;
    font-size: 12px;
    color: var(--muted);
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent);
    font: inherit;
    cursor: pointer;
    text-decoration: underline;
  }
  .files {
    list-style: none;
    margin: 0;
    padding: 0;
    border-right: 1px solid var(--line);
    overflow: auto;
    min-height: 0;
  }
  .files button {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    text-align: left;
    padding: 6px 8px;
    border: 0;
    border-left: 3px solid transparent;
    background: transparent;
    color: var(--fg);
    font: inherit;
    cursor: pointer;
  }
  .files button:hover {
    background: var(--raised);
  }
  .files button.active {
    border-left-color: var(--accent);
    background: var(--raised);
  }
  .files .path {
    font-family: ui-monospace, Consolas, monospace;
    font-size: 12px;
    word-break: break-all;
  }
  .meta {
    display: flex;
    gap: 8px;
    font-size: 11px;
    color: var(--muted);
  }
  .stat {
    color: var(--fg);
    font-variant-numeric: tabular-nums;
  }
  .stat.muted {
    color: var(--muted);
  }
  .view {
    min-width: 0;
    overflow: auto;
    min-height: 0;
  }
  .view header {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 10px;
    align-items: center;
    margin-bottom: 8px;
    font-family: ui-monospace, Consolas, monospace;
    font-size: 12px;
    position: sticky;
    left: 0;
  }
  .view header .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }
  .view .root {
    color: var(--muted);
    font-size: 11px;
  }
  .controls {
    margin-left: auto;
    display: inline-flex;
    gap: 8px;
    align-items: center;
    font-family: system-ui, sans-serif;
  }
  .toggle {
    display: inline-flex;
    border: 1px solid var(--line);
    border-radius: 4px;
    overflow: hidden;
  }
  .toggle button,
  .nav button {
    border: 0;
    background: transparent;
    color: var(--fg);
    font: inherit;
    font-size: 11px;
    padding: 2px 8px;
    cursor: pointer;
  }
  .toggle button:hover,
  .nav button:hover {
    background: var(--raised);
  }
  .toggle button.active {
    background: var(--accent);
    color: #fff;
  }
  .nav {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    font-size: 11px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .nav button {
    font-size: 14px;
    line-height: 1;
    padding: 1px 6px;
  }
  .rendered :global(.d2h-file-header) {
    display: none;
  }
  .rendered :global(.d2h-wrapper) {
    font-size: 12px;
  }
  .rendered :global(.d2h-info) {
    cursor: pointer;
  }
  .rendered :global(tr[role="button"]:focus-visible) {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  /* ReviewGlass's highlighter on the latest edit's rows. diff2html's table is drawn in
     its light scheme whatever the theme, so the light highlighter is used here. */
  .rendered :global(tr.rg-fresh td),
  .rendered :global(tr.rg-fresh td .d2h-code-line),
  .rendered :global(tr.rg-fresh td .d2h-code-line ins) {
    background-color: #fff1a8 !important;
  }
  .rendered :global(tr.rg-fresh td.d2h-code-linenumber) {
    box-shadow: inset 3px 0 0 #d9a400;
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 6px;
    margin: -2px 0 8px;
    font-size: 11px;
    color: var(--muted);
    position: sticky;
    left: 0;
  }
  .swatch {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 2px;
  }
  .swatch.older {
    margin-left: 8px;
    background: #dfd;
    box-shadow: inset 0 0 0 1px #b4e2b4;
  }
  .swatch.fresh {
    background: var(--fresh-bg, #fff1a8);
    box-shadow: inset 2px 0 0 var(--fresh-bar, #d9a400);
  }
  /* The whole file: the working copy, one row per line, wide as its longest line so
     a tint spans the row when the view scrolls sideways. */
  .view {
    --fresh-bg: #fff1a8;
    --fresh-bar: #d9a400;
  }
  .file {
    --added-bg: #e6ffec;
    --cut: #cf222e;
    width: max-content;
    min-width: 100%;
    font-family: ui-monospace, Consolas, monospace;
    font-size: 12px;
    line-height: 1.45;
  }
  @media (prefers-color-scheme: dark) {
    .view {
      --fresh-bg: #4d4110;
      --fresh-bar: #e0b400;
    }
    .file {
      --added-bg: #1f3a28;
      --cut: #ff8080;
    }
  }
  .file .ln.fresh {
    background: var(--fresh-bg);
    box-shadow: inset 3px 0 0 var(--fresh-bar);
  }
  .file.new .gap {
    padding: 4px 0 4px 4.5em;
    color: var(--muted);
    font-style: italic;
  }
  .ln {
    display: flex;
    white-space: pre;
  }
  .ln .n {
    flex: 0 0 3.5em;
    text-align: right;
    padding-right: 1em;
    color: var(--muted);
    user-select: none;
  }
  .ln.added {
    background: var(--added-bg);
  }
  .ln.cut {
    border-top: 2px solid var(--cut);
  }
  .ln.end {
    height: 0;
  }
  .ln.target .n {
    color: var(--accent);
    font-weight: 600;
  }
</style>
