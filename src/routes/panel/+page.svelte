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
  import Sessions from "./Sessions.svelte";
  import type { UsageView } from "./types";

  const tabs = ["Sessions", "Diff", "Cache", "PR", "Settings"] as const;
  type Tab = (typeof tabs)[number];
  const PHASE: Partial<Record<Tab, string>> = {
    Diff: "Live diff arrives in phase 4.",
    Cache: "Cache health arrives in phase 5.",
    PR: "Pull-request state arrives in phase 5.",
    Settings: "Settings arrive alongside the installer in phase 7.",
  };

  let active = $state<Tab>("Sessions");
  let usage = $state<UsageView | null>(null);
  let failure = $state<string | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;

  async function refresh() {
    try {
      usage = await invoke<UsageView>("panel_usage");
      failure = null;
    } catch (e) {
      failure = e instanceof Error ? e.message : String(e);
    }
    timer = setTimeout(refresh, 2000);
  }

  onMount(refresh);
  onDestroy(() => clearTimeout(timer));
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
    {:else}
      <p class="empty">{PHASE[active]}</p>
    {/if}
  </section>
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
</style>
