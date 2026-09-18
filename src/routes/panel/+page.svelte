<script lang="ts">
  // Panel window (rg.panel-window). Sessions is the default tab; the rest arrive with
  // their phases.
  //
  // The governing rule is spec section 10: absent data hides its element, it never
  // renders a zero. That is not only honesty about the data — it is what lets the panel
  // say something useful about WHY a figure is missing, and "no CLI session is running"
  // is a sentence the user can act on where a grey "0%" is not.
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Sessions from "./Sessions.svelte";
  import Settings from "./Settings.svelte";
  import Diff from "./Diff.svelte";
  import type { UsageView } from "./types";

  const tabs = ["Sessions", "Diff", "Cache", "PR", "Settings"] as const;
  type Tab = (typeof tabs)[number];
  const PHASE: Partial<Record<Tab, string>> = {
    Cache: "Cache health arrives in phase 5.",
    PR: "Pull-request state arrives in phase 5.",
  };

  let active = $state<Tab>("Sessions");
  let usage = $state<UsageView | null>(null);
  // Null for the first couple of seconds after start, before the usage loop's first pass.

  let failure = $state<string | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;

  async function refresh() {
    try {
      usage = await invoke<UsageView | null>("panel_usage");
      failure = null;
    } catch (e) {
      failure = e instanceof Error ? e.message : String(e);
    }
    timer = setTimeout(refresh, 2000);
  }

  // The panel opens beside the dock at its remembered size (dock.rs places it); a
  // resize by the user is what changes that size, so it is stored from here.
  let unlistenResize: (() => void) | undefined;
  let sizeTimer: ReturnType<typeof setTimeout> | undefined;
  onMount(() => {
    void refresh();
    void getCurrentWindow()
      .onResized(async (ev) => {
        clearTimeout(sizeTimer);
        const { width, height } = ev.payload;
        sizeTimer = setTimeout(() => void invoke("panel_save_size", { width, height }), 400);
      })
      .then((u) => (unlistenResize = u));
  });
  onDestroy(() => {
    clearTimeout(timer);
    clearTimeout(sizeTimer);
    unlistenResize?.();
  });
</script>

<div class="panel">
  <nav>
    {#each tabs as tab (tab)}
      <button class:active={active === tab} onclick={() => (active = tab)}>
        {tab}
        {#if tab === "Sessions" && usage}<span class="count">{usage.sessions.length}</span>{/if}
      </button>
    {/each}
  </nav>

  <section>
    {#if active === "Sessions"}
      <Sessions {usage} {failure} />
    {:else if active === "Diff"}
      <Diff />
    {:else if active === "Settings"}
      <Settings />
    {:else}
      <p class="empty">{PHASE[active]}</p>
    {/if}
  </section>

  <footer>
    Closing this window hides it. The tray icon or the ▤ button on the glass brings it back; the tray menu quits for real.
  </footer>
</div>

<style>
  :global(:root) {
    --bg: #ffffff;
    --fg: #1b1b1b;
    --muted: #6a6a6a;
    --line: #e0e0e0;
    --raised: #f6f6f6;
    --accent: #2f6feb;
    --warn: #b06000;
    --bad: #b32020;
  }
  @media (prefers-color-scheme: dark) {
    :global(:root) {
      --bg: #1a1a1c;
      --fg: #ececec;
      --muted: #9a9a9a;
      --line: #333336;
      --raised: #232326;
      --accent: #6ba1ff;
      --warn: #e0a34a;
      --bad: #ff8080;
    }
  }
  :global(html, body) {
    margin: 0;
    background: var(--bg);
    color: var(--fg);
  }

  .panel {
    display: flex;
    flex-direction: column;
    height: 100vh;
    font: 13px/1.5 system-ui, sans-serif;
  }
  nav {
    display: flex;
    gap: 2px;
    padding: 8px 10px 0;
    border-bottom: 1px solid var(--line);
  }
  nav button {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border: 1px solid transparent;
    border-bottom: none;
    border-radius: 7px 7px 0 0;
    background: none;
    color: var(--muted);
    font: inherit;
    cursor: pointer;
  }
  nav button:hover {
    color: var(--fg);
  }
  nav button.active {
    border-color: var(--line);
    background: var(--raised);
    color: var(--fg);
  }
  .count {
    padding: 0 6px;
    border-radius: 9px;
    background: var(--line);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }
  section {
    flex: 1;
    overflow: auto;
    padding: 14px;
  }
  .empty {
    color: var(--muted);
  }
  footer {
    padding: 6px 14px;
    border-top: 1px solid var(--line);
    color: var(--muted);
    font-size: 11px;
  }
</style>
