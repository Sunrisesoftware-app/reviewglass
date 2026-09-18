<script lang="ts">
  // The Diff tab (P4): what the agent changed, as git sees it, within a second of the
  // edit. A list of the files touched since ReviewGlass started, newest first, and the
  // selected file's diff rendered by diff2html (an existing renderer, spec 6.2). Every
  // empty state names its reason: no hook, no edit yet, a file git cannot show.
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { html as diffHtml } from "diff2html";
  import "diff2html/bundles/css/diff2html.min.css";
  import type { DiffTab, DiffView } from "./types";

  let tab = $state<DiffTab | null>(null);
  let selected = $state<string | null>(null);
  let failure = $state<string | null>(null);
  let now = $state(Date.now());

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

  async function refresh() {
    try {
      tab = await invoke<DiffTab>("panel_diffs");
      failure = null;
    } catch (e) {
      failure = e instanceof Error ? e.message : String(e);
    }
  }

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
    switch (v.status) {
      case "changed":
        return `+${v.added ?? 0} −${v.removed ?? 0}`;
      case "untracked":
        return `new, +${v.added ?? 0}`;
      case "unchanged":
        return "unchanged";
      case "not-in-repo":
        return "not in a repository";
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

  onMount(() => {
    void refresh();
    const tick = setInterval(() => (now = Date.now()), 5000);
    let unlisten: (() => void) | undefined;
    void listen("diff:update", () => void refresh()).then((u) => (unlisten = u));
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
            onclick={() => (selected = v.path)}
            title={v.path}
          >
            <span class="path">{v.display_path}</span>
            <span class="meta">
              <span class="stat" class:muted={v.status !== "changed" && v.status !== "untracked"}>{label(v)}</span>
              <span class="when">{v.tool ?? "edit"} · {ago(v.at_ms)}</span>
            </span>
          </button>
        </li>
      {/each}
    </ul>
    <section class="view">
      {#if current}
        <header>
          <span class="path" title={current.path}>{current.display_path}</span>
          {#if current.repo_root}
            <span class="root" title={current.repo_root}>{current.repo_root}</span>
          {/if}
        </header>
        {#if current.unified}
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
  .diff {
    display: grid;
    grid-template-columns: minmax(180px, 32%) 1fr;
    gap: 12px;
    min-height: 0;
  }
  .files {
    list-style: none;
    margin: 0;
    padding: 0;
    border-right: 1px solid var(--line);
    overflow: auto;
    max-height: 70vh;
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
    max-height: 70vh;
  }
  .view header {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    align-items: baseline;
    margin-bottom: 8px;
    font-family: ui-monospace, Consolas, monospace;
    font-size: 12px;
  }
  .view .root {
    color: var(--muted);
    font-size: 11px;
  }
  .rendered :global(.d2h-file-header) {
    display: none;
  }
  .rendered :global(.d2h-wrapper) {
    font-size: 12px;
  }
</style>
