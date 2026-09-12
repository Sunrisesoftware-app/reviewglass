<script lang="ts">
  // Settings tab. For now: the alert thresholds, and a way to see what an alert looks
  // like without burning 75 % of a window to find out.
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  type AlertConfig = { enabled: boolean; thresholds: number[] };

  let cfg = $state<AlertConfig | null>(null);
  let saving = $state(false);
  let testResult = $state<"idle" | "sent" | "failed">("idle");
  let testError = $state<string | null>(null);

  async function load() {
    cfg = await invoke<AlertConfig>("alerts_get");
  }

  async function save(next: AlertConfig) {
    saving = true;
    try {
      cfg = await invoke<AlertConfig>("alerts_set", {
        enabled: next.enabled,
        thresholds: next.thresholds,
      });
    } finally {
      saving = false;
    }
  }

  function setThreshold(i: number, value: number) {
    if (!cfg) return;
    const t = [...cfg.thresholds];
    t[i] = value;
    void save({ ...cfg, thresholds: t });
  }

  function removeThreshold(i: number) {
    if (!cfg) return;
    void save({ ...cfg, thresholds: cfg.thresholds.filter((_, j) => j !== i) });
  }

  function addThreshold() {
    if (!cfg) return;
    // Halfway between the highest existing threshold and 100, or 50 for an empty list;
    // whatever is chosen can be edited in place.
    const top = Math.max(0, ...cfg.thresholds);
    const next = cfg.thresholds.length === 0 ? 50 : Math.min(99, Math.round((top + 100) / 2));
    void save({ ...cfg, thresholds: [...cfg.thresholds, next] });
  }

  async function test() {
    testResult = "idle";
    testError = null;
    try {
      await invoke("alerts_test");
      testResult = "sent";
    } catch (e) {
      testResult = "failed";
      testError = e instanceof Error ? e.message : String(e);
    }
  }

  onMount(load);
</script>

{#if !cfg}
  <p class="state">Reading settings…</p>
{:else}
  <section>
    <h2>Quota alerts</h2>
    <p class="help">
      A Windows notification when the account's 5-hour or 7-day window crosses a threshold.
      Each threshold fires once per window and re-arms when the window resets. Alerts need
      the quota, which reaches ReviewGlass only through a terminal <code>claude</code>
      session.
    </p>

    <label class="row">
      <input
        type="checkbox"
        checked={cfg.enabled}
        disabled={saving}
        onchange={(e) => save({ ...cfg!, enabled: e.currentTarget.checked })}
      />
      <span>Notify me when a window crosses a threshold</span>
    </label>

    <div class="thresholds" class:off={!cfg.enabled}>
      {#each cfg.thresholds as t, i (i)}
        <div class="threshold">
          <input
            type="number"
            min="1"
            max="99"
            step="1"
            value={t}
            disabled={saving}
            aria-label="Threshold {i + 1}, percent"
            onchange={(e) => setThreshold(i, Number(e.currentTarget.value))}
          />
          <span class="unit">%</span>
          <button
            class="remove"
            title="Remove this threshold"
            aria-label="Remove threshold {t}%"
            disabled={saving}
            onclick={() => removeThreshold(i)}>✕</button
          >
        </div>
      {/each}
      <button class="add" disabled={saving || cfg.thresholds.length >= 6} onclick={addThreshold}>
        + Add a threshold
      </button>
    </div>
    {#if cfg.thresholds.length === 0}
      <p class="help">No thresholds: nothing will fire. Add one, or turn alerts off to say so.</p>
    {/if}

    <div class="test">
      <button onclick={test}>Show a test notification</button>
      {#if testResult === "sent"}
        <span class="ok">Sent. If nothing appeared, check Windows notification settings for ReviewGlass.</span>
      {:else if testResult === "failed"}
        <span class="bad">Could not show it: {testError}</span>
      {/if}
    </div>
  </section>
{/if}

<style>
  h2 {
    margin: 0 0 4px;
    font-size: 14px;
    font-weight: 600;
  }
  .help {
    margin: 0 0 12px;
    max-width: 60ch;
    color: var(--muted);
  }
  .help code {
    padding: 1px 4px;
    border-radius: 3px;
    background: var(--line);
  }
  .state {
    color: var(--muted);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 12px;
  }
  .thresholds {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-bottom: 14px;
    transition: opacity 120ms;
  }
  .thresholds.off {
    opacity: 0.45;
  }
  .threshold {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 2px 4px 2px 6px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--raised);
  }
  .threshold input {
    width: 3.2em;
    border: 0;
    background: transparent;
    color: var(--fg);
    font: inherit;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .unit {
    color: var(--muted);
  }
  .remove {
    margin-left: 4px;
    border: 0;
    border-radius: 4px;
    padding: 0 5px;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
  }
  .remove:hover {
    background: var(--line);
    color: var(--fg);
  }
  .add,
  .test button {
    padding: 4px 10px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--raised);
    color: var(--fg);
    font: inherit;
    cursor: pointer;
  }
  .add:hover,
  .test button:hover {
    border-color: var(--accent);
  }
  .add:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .test {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    padding-top: 12px;
    border-top: 1px solid var(--line);
  }
  .ok {
    color: var(--muted);
  }
  .bad {
    color: var(--bad);
  }
</style>
