<script lang="ts">
  // Settings tab. For now: the alert thresholds, and a way to see what an alert looks
  // like without burning 75 % of a window to find out.
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  type AlertConfig = { enabled: boolean; thresholds: number[] };

  let cfg = $state<AlertConfig | null>(null);
  let hotkey = $state("");
  let hotkeyDraft = $state("");
  let hotkeyError = $state<string | null>(null);
  let hotkeySaved = $state(false);
  let saving = $state(false);
  let testResult = $state<"idle" | "sent" | "failed">("idle");
  let testError = $state<string | null>(null);
  // The Follow measurement recorder (temporary tooling for tuning the column detector
  // and Fit together): Record starts a new file, Stop ends it, like any recorder.
  type LogState = {
    follow_log: boolean;
    follow_log_path: string | null;
    follow_log_since: number | null;
    follow_log_lines: number;
  };
  let rec = $state<LogState>({ follow_log: false, follow_log_path: null, follow_log_since: null, follow_log_lines: 0 });
  let recError = $state<string | null>(null);
  let now = $state(Math.floor(Date.now() / 1000));
  const elapsed = $derived(rec.follow_log && rec.follow_log_since ? Math.max(0, now - rec.follow_log_since) : 0);

  function mmss(s: number) {
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  }

  async function load() {
    cfg = await invoke<AlertConfig>("alerts_get");
    const s = await invoke<{ hotkey_toggle: string } & LogState>("glass_state");
    hotkey = s.hotkey_toggle;
    hotkeyDraft = hotkey;
    rec = s;
  }

  async function setRecording(on: boolean) {
    recError = null;
    try {
      rec = await invoke<LogState>("follow_log_set", { on });
    } catch (e) {
      recError = e instanceof Error ? e.message : String(e);
    }
  }

  // While recording, the elapsed time and the line count tick once a second.
  $effect(() => {
    if (!rec.follow_log) return;
    const t = setInterval(async () => {
      now = Math.floor(Date.now() / 1000);
      try {
        rec = await invoke<LogState>("glass_state");
      } catch {
        /* the next tick asks again */
      }
    }, 1000);
    return () => clearInterval(t);
  });

  // The glass on/off shortcut. Applied on purpose, not on every keystroke: a half-typed
  // combination must not be registered.
  async function applyHotkey() {
    hotkeyError = null;
    hotkeySaved = false;
    try {
      hotkey = await invoke<string>("hotkey_set_toggle", { shortcut: hotkeyDraft });
      hotkeyDraft = hotkey;
      hotkeySaved = true;
    } catch (e) {
      hotkeyError = e instanceof Error ? e.message : String(e);
    }
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
    <h2>Glass on / off</h2>
    <p class="help">
      The shortcut that switches the glass on (as it last was) and off, from any
      application — the same as the <b>RG</b> mark on the dock. Modifiers and a key, e.g.
      <code>Ctrl+Alt+G</code>, <code>Ctrl+Shift+Space</code>, <code>F9</code>. A combination
      another application already holds cannot be taken, and the old one stays.
    </p>
    <div class="hotkey">
      <input
        type="text"
        value={hotkeyDraft}
        aria-label="Glass on/off shortcut"
        spellcheck="false"
        oninput={(e) => (hotkeyDraft = e.currentTarget.value)}
        onkeydown={(e) => e.key === "Enter" && applyHotkey()}
      />
      <button onclick={applyHotkey} disabled={hotkeyDraft.trim() === hotkey}>Apply</button>
      {#if hotkeySaved}
        <span class="ok">Now {hotkey}.</span>
      {:else if hotkeyError}
        <span class="bad">{hotkeyError}</span>
      {/if}
    </div>
  </section>

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

  <section>
    <h2>Measurements</h2>
    <p class="help">
      A recording of what the column detector reads and what Fit does with it, for tuning
      the two together: the cursor, the column's edges, the source rectangle, the glass's
      size and zoom, and Fit's decisions. Structure only — never pixels, never text. Each
      recording is its own file under <code>%TEMP%</code>, named by its start time; a
      recording ends with Stop or with the app, and stops itself after 200 000 lines.
    </p>
    <div class="record">
      {#if rec.follow_log}
        <button class="stop" onclick={() => setRecording(false)} aria-label="Stop recording">
          <span class="square" aria-hidden="true"></span> Stop
        </button>
        <span class="live"><span class="dot" aria-hidden="true"></span> Recording {mmss(elapsed)} · {rec.follow_log_lines} lines</span>
      {:else}
        <button class="rec" onclick={() => setRecording(true)} aria-label="Start recording">
          <span class="dot" aria-hidden="true"></span> Record
        </button>
      {/if}
    </div>
    <p class="help">
      {#if rec.follow_log_path}
        {rec.follow_log ? "Writing to" : "Last recording:"} <code>{rec.follow_log_path}</code>
      {:else}
        No recording yet.
      {/if}
    </p>
    {#if recError}
      <p class="bad">{recError}</p>
    {/if}
  </section>
{/if}

<style>
  section + section {
    margin-top: 22px;
  }
  .hotkey {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .hotkey input {
    width: 14em;
    font: inherit;
    padding: 3px 6px;
  }
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
  .record {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 10px;
  }
  .record button {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 5px 12px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--raised);
    color: var(--fg);
    font: inherit;
    cursor: pointer;
  }
  .record button:hover {
    border-color: var(--accent);
  }
  .dot {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #d23b3b;
  }
  .square {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 2px;
    background: var(--fg);
  }
  .live {
    color: #d23b3b;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-variant-numeric: tabular-nums;
  }
</style>
