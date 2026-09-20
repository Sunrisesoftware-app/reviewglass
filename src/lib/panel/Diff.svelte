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
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { html as diffHtml } from "diff2html";
  import "diff2html/bundles/css/diff2html.min.css";
  import type { Baseline, DiffTab, DiffView, FileView } from "./types";

  let tab = $state<DiffTab | null>(null);
  let selected = $state<string | null>(null);
  let failure = $state<string | null>(null);
  let now = $state(Date.now());

  /** "hunks" (the default) or "file" (P4b). */
  let mode = $state<"hunks" | "file">("hunks");
  let file = $state<FileView | null>(null);
  let fileFailure = $state<string | null>(null);
  /** Which of the file's changes the view last went to (0-based), for ‹ ›. */
  let at = $state(0);
  let viewEl = $state<HTMLElement | null>(null);

  const current = $derived<DiffView | null>(
    tab ? (tab.views.find((v) => v.path === selected) ?? tab.views[0] ?? null) : null,
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

  /** Open the whole file at `line`, or at its first change. */
  async function openFile(line?: number) {
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

  function step(d: number) {
    if (!file || file.hunks.length === 0) return;
    at = (at + d + file.hunks.length) % file.hunks.length;
    void goTo(file.hunks[at].start);
  }

  /** The list changed: re-read an open file, or fall back to the hunks if it is gone. */
  async function onUpdate() {
    await refresh();
    if (mode !== "file") return;
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
    viewEl.querySelectorAll("div.d2h-info").forEach((info) => {
      const row = (info.closest("tr") ?? info) as HTMLElement;
      const m = /\+(\d+)/.exec(info.textContent ?? "");
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
{:else}
  <div class="diff">
    <ul class="files" aria-label="Files the agent changed, newest first">
      {#each tab.views as v (v.path)}
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
          {#if mode === "file"}
            <span class="root">working copy · read-only</span>
          {:else if current.repo_root}
            <span class="root" title={current.repo_root}>{current.repo_root}</span>
          {/if}
          {#if current.baseline && (mode === "file" ? file?.hunks.length : current.unified)}
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
            <span class="toggle" role="group" aria-label="Hunks or the whole file">
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
            </span>
          </span>
        </header>
        {#if mode === "file"}
          {#if fileFailure}
            <p class="state bad">The core is not answering: {fileFailure}</p>
          {:else if !file}
            <p class="state">Reading…</p>
          {:else if file.status !== "shown" || file.text === null}
            <p class="state">{file.reason ?? "Nothing to show."}</p>
          {:else}
            <div class="file" role="document" aria-label="The whole file, read-only">
              {#each lines as text, i (i)}
                <div
                  class="ln"
                  class:added={added.has(i + 1)}
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
  /* Sized for the drawer's 640 px (adr.rg.020): the file list and the hunk side by
     side, each scrolling on its own inside the drawer's height. */
  .diff {
    display: grid;
    grid-template-columns: minmax(170px, 30%) minmax(0, 1fr);
    gap: 12px;
    height: 100%;
    min-height: 0;
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
  /* The whole file: the working copy, one row per line, wide as its longest line so
     a tint spans the row when the view scrolls sideways. */
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
    .file {
      --added-bg: #1f3a28;
      --cut: #ff8080;
    }
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
