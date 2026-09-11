<script lang="ts">
  // Panel window: Sessions (default), Diff, Cache, PR, Settings.
  // Every tab hides an element whose data source is absent; it never renders a zero.
  const tabs = ["Sessions", "Diff", "Cache", "PR", "Settings"] as const;
  let active: (typeof tabs)[number] = $state("Sessions");
</script>

<div class="panel">
  <nav>
    {#each tabs as tab}
      <button class:active={active === tab} onclick={() => (active = tab)}>{tab}</button>
    {/each}
  </nav>
  <section>
    {#if active === "Sessions"}
      <p class="empty">No live sessions. Collectors are not installed yet (P2).</p>
    {:else if active === "Diff"}
      <p class="empty">No edits observed yet (P4).</p>
    {:else if active === "Cache"}
      <p class="empty">Cache statistics need Claude Code 2.1.251 or newer (P5).</p>
    {:else if active === "PR"}
      <p class="empty">No pull request bound to a live session (P5).</p>
    {:else}
      <p class="empty">Defaults in effect (P1).</p>
    {/if}
  </section>
</div>

<style>
  .panel { display: flex; flex-direction: column; height: 100vh; font: 14px system-ui, sans-serif; }
  nav { display: flex; gap: 2px; padding: 6px 8px 0; border-bottom: 1px solid #ccc; }
  nav button { border: 1px solid transparent; border-bottom: none; background: none; padding: 6px 12px; cursor: pointer; border-radius: 6px 6px 0 0; }
  nav button.active { border-color: #ccc; background: #fff; }
  section { flex: 1; padding: 16px; }
  .empty { color: #666; }
</style>
