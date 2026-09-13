<script lang="ts">
  // The dock (rg.dock-window, adr.rg.018): the control panel and the fixed point. A
  // strip in a screen corner, present at every start while the glass starts hidden.
  // One button per glass mode: a click switches the glass on in that mode, a click on
  // the lit button switches it off, so the lit button always says whether the glass
  // is on and how. Beside them the account gauge in miniature — the same account-wide
  // figure as the panel (adr.rg.007), never a per-session one — and the panel button.
  // Dragged anywhere, the dock snaps to the nearest corner.
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  type GlassState = {
    visible: boolean;
    frozen: boolean;
    lens: boolean;
    build: string;
  };
  type QuotaWindow = {
    used_percentage: number;
    resets_at: number | null;
    stale: boolean;
  };
  type UsageView = {
    quota: {
      five_hour: QuotaWindow | null;
      absence: "no-cli-session" | "not-reported-yet" | null;
    };
  };
  type ModeName = "follow" | "lens" | "still";

  const win = getCurrentWindow();
  const USAGE_EVERY = 5000;

  let visible = $state(false);
  let frozen = $state(false);
  let lens = $state(false);
  let build = $state("");
  let usage = $state<UsageView | null | undefined>(undefined); // undefined: not asked yet
  let now = $state(Date.now());
  const mode = $derived<ModeName>(lens ? "lens" : frozen ? "still" : "follow");
  const lit = $derived<ModeName | null>(visible ? mode : null);
  const five = $derived(usage?.quota.five_hour ?? null);

  // The glass hangs from these three buttons: the lit one is the glass's mode, and a
  // click on it is the way off.
  function press(next: ModeName) {
    void invoke("dock_activate", { mode: lit === next ? "off" : next });
  }

  function pct(v: number): string {
    return `${v < 10 ? v.toFixed(1) : Math.round(v)}%`;
  }

  function untilReset(w: QuotaWindow): string | null {
    if (w.resets_at === null) return null;
    const ms = w.resets_at * 1000 - now;
    if (ms <= 0) return null;
    const h = Math.floor(ms / 3_600_000);
    const m = Math.round((ms % 3_600_000) / 60_000);
    return h > 0 ? `${h} h ${m} min` : `${m} min`;
  }

  function severity(v: number): "" | "warn" | "bad" {
    return v >= 90 ? "bad" : v >= 75 ? "warn" : "";
  }

  async function refreshUsage() {
    try {
      usage = await invoke<UsageView | null>("panel_usage");
      now = Date.now();
    } catch {
      // The core is not answering yet; the next tick asks again.
    }
  }

  function oncontextmenu(e: MouseEvent) {
    e.preventDefault();
    void invoke("dock_menu");
  }

  function startDrag(e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    void win.startDragging();
  }

  // A button must not also drag the strip.
  function control(e: Event, run: () => void) {
    e.stopPropagation();
    e.preventDefault();
    run();
  }

  onMount(() => {
    const unlisten: (() => void)[] = [];
    let snapTimer: ReturnType<typeof setTimeout> | undefined;
    let stopped = false;

    (async () => {
      // The first call can land before the core has registered its state; ask again.
      for (let attempt = 0; !stopped; attempt++) {
        try {
          const s = await invoke<GlassState>("glass_state");
          visible = s.visible;
          frozen = s.frozen;
          lens = s.lens;
          build = s.build;
          break;
        } catch {
          await new Promise((r) => setTimeout(r, 250));
          if (attempt > 40) break;
        }
      }
      unlisten.push(
        await listen<GlassState>("glass:state", (ev) => {
          visible = ev.payload.visible;
          frozen = ev.payload.frozen;
          lens = ev.payload.lens;
          build = ev.payload.build;
        }),
      );
      // Dragged: once the drag has settled, snap to the nearest corner.
      unlisten.push(
        await win.onMoved(() => {
          clearTimeout(snapTimer);
          snapTimer = setTimeout(() => void invoke("dock_snap"), 350);
        }),
      );
      await refreshUsage();
    })();
    const usageTimer = setInterval(() => void refreshUsage(), USAGE_EVERY);
    const clock = setInterval(() => (now = Date.now()), 30_000);

    return () => {
      stopped = true;
      clearInterval(usageTimer);
      clearInterval(clock);
      unlisten.forEach((u) => u());
    };
  });
</script>

<div class="dock" {oncontextmenu} role="toolbar" tabindex="-1" aria-label="ReviewGlass dock">
  <button class="grab" title="Drag to another corner" aria-label="Move the dock" onpointerdown={startDrag}
    >✥</button
  >
  <span class="name" title={build ? `ReviewGlass build ${build}` : "ReviewGlass"}>RG</span>

  <div class="modes" role="group" aria-label="Glass">
    <button
      class="follow"
      class:on={lit === "follow"}
      aria-pressed={lit === "follow"}
      title={lit === "follow"
        ? "The glass is on, following the cursor — click to switch it off"
        : "Switch the glass on: it stays where it is and shows what is around the cursor"}
      onpointerdown={(e) => control(e, () => press("follow"))}>Follow</button
    >
    <button
      class="lens"
      class:on={lit === "lens"}
      aria-pressed={lit === "lens"}
      title={lit === "lens"
        ? "The lens is on the cursor — click to switch it off"
        : "Switch the lens on: the glass rides on the cursor"}
      onpointerdown={(e) => control(e, () => press("lens"))}>Lens</button
    >
    <button
      class="still"
      class:on={lit === "still"}
      aria-pressed={lit === "still"}
      title={lit === "still"
        ? "A still is on screen — click to switch it off"
        : "Keep what the glass shows as a still"}
      onpointerdown={(e) => control(e, () => press("still"))}>Still</button
    >
  </div>

  <span class="sep"></span>

  <!-- The account gauge, or the reason there is none. Never a zero, never a dash. -->
  {#if five}
    <span
      class="gauge {severity(five.used_percentage)}"
      class:stale={five.stale}
      title={five.stale
        ? "5-hour window, account-wide — the reset time has passed with no fresher sample; this is the last figure seen"
        : "5-hour window, account-wide — the same figure in every session (from the panel)"}
    >
      <b>{pct(five.used_percentage)}</b>
      {#if untilReset(five)}<small>resets in {untilReset(five)}</small>{/if}
    </span>
  {:else if usage?.quota.absence === "no-cli-session"}
    <span class="gauge absent" title="The quota reaches ReviewGlass only through a terminal claude session. Start one anywhere and the gauge appears.">no CLI session</span>
  {:else if usage?.quota.absence === "not-reported-yet"}
    <span class="gauge absent" title="A CLI session is running but has not reported a quota yet">not reported yet</span>
  {/if}

  <span class="spacer"></span>

  <button
    title="Open the sessions panel"
    aria-label="Open the sessions panel"
    onpointerdown={(e) => control(e, () => invoke("panel_show"))}>▤</button
  >
</div>

<style>
  :global(html, body) {
    margin: 0;
    background: transparent;
    overflow: hidden;
  }
  .dock {
    --follow: #ffc800;
    --lens: #ffffff;
    --still: #5ab4ff;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 0.25em;
    width: 100vw;
    height: 100vh;
    padding: 0 0.5em 0 0.3em;
    border-radius: 8px;
    background: #202020;
    border: 1px solid rgba(255, 255, 255, 0.16);
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.45);
    font: 12px system-ui, sans-serif;
    color: #eee;
    user-select: none;
  }
  button {
    min-width: 1.85em;
    height: 1.9em;
    padding: 0 0.5em;
    border: 0;
    border-radius: 5px;
    background: transparent;
    color: inherit;
    font: inherit;
    line-height: 1;
    cursor: pointer;
  }
  button:hover:not(.on) {
    background: rgba(255, 255, 255, 0.14);
  }
  .grab {
    cursor: move;
    opacity: 0.75;
    font-size: 1.1em;
    padding: 0 0.3em;
  }
  .name {
    font-weight: 700;
    letter-spacing: 0.04em;
    opacity: 0.85;
    margin-right: 0.35em;
  }
  .modes {
    display: flex;
    gap: 2px;
    padding: 2px;
    border-radius: 7px;
    background: rgba(255, 255, 255, 0.08);
  }
  .modes button {
    padding: 0.15em 0.75em;
  }
  /* The lit button carries its mode's colour — the same colour as the glass's border,
     so the dock says at a glance what is on screen. */
  .modes button.follow.on {
    background: var(--follow);
    color: #111;
    font-weight: 600;
  }
  .modes button.lens.on {
    background: var(--lens);
    color: #111;
    font-weight: 600;
  }
  .modes button.still.on {
    background: var(--still);
    color: #111;
    font-weight: 600;
  }
  .sep {
    width: 1px;
    height: 1.3em;
    margin: 0 0.35em;
    background: rgba(255, 255, 255, 0.22);
  }
  .gauge {
    display: inline-flex;
    align-items: baseline;
    gap: 0.4em;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  .gauge b {
    font-size: 1.15em;
  }
  .gauge small {
    opacity: 0.7;
  }
  .gauge.warn b {
    color: #ffb545;
  }
  .gauge.bad b {
    color: #ff6b6b;
  }
  .gauge.stale {
    opacity: 0.55;
  }
  .gauge.absent {
    opacity: 0.6;
    font-style: italic;
  }
  .spacer {
    flex: 1;
  }
</style>
