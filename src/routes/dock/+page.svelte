<script lang="ts">
  // The dock (rg.dock-window, adr.rg.018): the control panel and the fixed point. A
  // strip in a screen corner, present at every start while the glass starts hidden.
  // One button per glass mode: a click switches the glass on in that mode, a click on
  // the lit button switches it off, so the lit button always says whether the glass
  // is on and how. Beside them the account gauge in miniature — the same account-wide
  // figure as the panel (adr.rg.007), never a per-session one — and the drawer button.
  // Dragged anywhere, the dock snaps to the nearest corner.
  //
  // The panel is the dock's drawer, in this same window (adr.rg.020): the ▤ button
  // unfolds it under the strip (over it, in a bottom corner) at 640 px, with the tabs
  // — Sessions, Diff, Settings — in the drawer's first row. The Rust side sizes the
  // window; this page lays out whichever way the corner says.
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Sessions from "$lib/panel/Sessions.svelte";
  import Diff from "$lib/panel/Diff.svelte";
  import Settings from "$lib/panel/Settings.svelte";
  import type { UsageView } from "$lib/panel/types";

  type GlassState = {
    visible: boolean;
    frozen: boolean;
    lens: boolean;
    build: string;
    hotkey_toggle: string;
  };
  type Corner = "top-left" | "top-right" | "bottom-left" | "bottom-right";
  type DockView = {
    open: boolean;
    corner: Corner;
    drawer_height: number;
    drawer_tab: string;
  };
  type ModeName = "follow" | "lens" | "still";
  type Tab = "sessions" | "diff" | "settings";
  const TABS: { id: Tab; label: string }[] = [
    { id: "sessions", label: "Sessions" },
    { id: "diff", label: "Diff" },
    { id: "settings", label: "Settings" },
  ];

  const win = getCurrentWindow();
  const USAGE_CLOSED = 5000;
  const USAGE_OPEN = 2000;

  let visible = $state(false);
  let frozen = $state(false);
  let lens = $state(false);
  let build = $state("");
  let hotkey = $state("");
  let usage = $state<UsageView | null | undefined>(undefined); // undefined: not asked yet
  let failure = $state<string | null>(null);
  let now = $state(Date.now());
  let open = $state(false);
  let corner = $state<Corner>("top-left");
  let tab = $state<Tab>("sessions");
  const mode = $derived<ModeName>(lens ? "lens" : frozen ? "still" : "follow");
  const lit = $derived<ModeName | null>(visible ? mode : null);
  const five = $derived(usage?.quota.five_hour ?? null);
  const bottom = $derived(corner === "bottom-left" || corner === "bottom-right");

  // The glass hangs from these three buttons: the lit one is the glass's mode, and a
  // click on it is the way off.
  function press(next: ModeName) {
    void invoke("dock_activate", { mode: lit === next ? "off" : next });
  }

  function applyDock(d: DockView) {
    open = d.open;
    corner = d.corner;
    if (d.drawer_tab === "sessions" || d.drawer_tab === "diff" || d.drawer_tab === "settings") tab = d.drawer_tab;
  }

  async function toggleDrawer() {
    try {
      applyDock(await invoke<DockView>("dock_drawer", { open: !open }));
      if (open) void refreshUsage();
    } catch {
      // The core is not answering yet.
    }
  }

  function pick(next: Tab) {
    tab = next;
    void invoke("dock_set_tab", { tab: next });
  }

  function pct(v: number): string {
    return `${v < 10 ? v.toFixed(1) : Math.round(v)}%`;
  }

  function untilReset(w: { resets_at: number | null }): string | null {
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
      failure = null;
      now = Date.now();
    } catch (e) {
      // The core is not answering yet; the next tick asks again. The Sessions tab
      // says so meanwhile.
      failure = e instanceof Error ? e.message : String(e);
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
    let usageTimer: ReturnType<typeof setTimeout> | undefined;
    let stopped = false;

    // The usage poll: brisk while the drawer is open, unhurried while it is a strip.
    const schedule = () => {
      clearTimeout(usageTimer);
      usageTimer = setTimeout(async () => {
        if (stopped) return;
        await refreshUsage();
        schedule();
      }, open ? USAGE_OPEN : USAGE_CLOSED);
    };

    (async () => {
      // The first call can land before the core has registered its state; ask again.
      for (let attempt = 0; !stopped; attempt++) {
        try {
          const s = await invoke<GlassState>("glass_state");
          visible = s.visible;
          frozen = s.frozen;
          lens = s.lens;
          build = s.build;
          hotkey = s.hotkey_toggle;
          applyDock(await invoke<DockView>("dock_state"));
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
          hotkey = ev.payload.hotkey_toggle;
        }),
      );
      // The drawer opened from the tray or the glass, or the dock changed corner.
      unlisten.push(
        await listen<DockView>("dock:state", (ev) => {
          const was = open;
          applyDock(ev.payload);
          if (open && !was) {
            void refreshUsage();
            schedule();
          }
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
      schedule();
    })();
    const clock = setInterval(() => (now = Date.now()), 30_000);

    return () => {
      stopped = true;
      clearTimeout(usageTimer);
      clearInterval(clock);
      unlisten.forEach((u) => u());
    };
  });
</script>

<div class="dock" class:open class:bottom {oncontextmenu} role="toolbar" tabindex="-1" aria-label="ReviewGlass dock">
  <div class="strip">
    <button class="grab" title="Drag to another corner" aria-label="Move the dock" onpointerdown={startDrag}
      >✥</button
    >
    <!-- The RG mark is the one-click switch: the glass on as it last was, or off. The
         hotkey does the same from anywhere, and this is where it is said. -->
    <button
      class="name"
      class:on={visible}
      aria-pressed={visible}
      title={(visible ? "Glass off" : "Glass on") +
        (hotkey ? ` — ${hotkey} from anywhere` : "") +
        (build ? `\nReviewGlass build ${build}` : "")}
      onpointerdown={(e) => control(e, () => invoke("dock_activate", { mode: visible ? "off" : "last" }))}
      >RG</button
    >

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
      class="drawer-toggle"
      class:on={open}
      aria-pressed={open}
      aria-expanded={open}
      title={open ? "Close the drawer" : "Open the drawer: sessions, diff and settings"}
      aria-label={open ? "Close the drawer" : "Open the drawer"}
      onpointerdown={(e) => control(e, () => toggleDrawer())}>{open ? (bottom ? "▾" : "▴") : "▤"}</button
    >
    <!-- The way out, in sight (CLAUDE.md: ship the affordance with the mechanism). The
         menus and the tray still quit too; this is the one a user finds without being
         told. It quits for real: the glass, the drawer and the process. -->
    <button
      class="quit"
      title="Quit ReviewGlass"
      aria-label="Quit ReviewGlass"
      onpointerdown={(e) => control(e, () => invoke("app_quit"))}>✕</button
    >
  </div>

  {#if open}
    <div class="drawer" role="region" aria-label="Sessions, diff and settings">
      <nav aria-label="Drawer tabs">
        {#each TABS as t (t.id)}
          <button class:active={tab === t.id} onclick={() => pick(t.id)} aria-pressed={tab === t.id}>
            {t.label}
            {#if t.id === "sessions" && usage}<span class="count">{usage.sessions.length}</span>{/if}
          </button>
        {/each}
      </nav>
      <section>
        {#if tab === "sessions"}
          <Sessions usage={usage ?? null} {failure} />
        {:else if tab === "diff"}
          <Diff />
        {:else}
          <Settings />
        {/if}
      </section>
    </div>
  {/if}
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
    --strip: #202020;
    /* The drawer's theme: the panel's variables, scoped here. */
    --bg: #ffffff;
    --fg: #1b1b1b;
    --muted: #6a6a6a;
    --line: #e0e0e0;
    --raised: #f6f6f6;
    --accent: #2f6feb;
    --warn: #b06000;
    --bad: #b32020;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    width: 100vw;
    height: 100vh;
    border-radius: 8px;
    background: var(--strip);
    border: 1px solid rgba(255, 255, 255, 0.16);
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.45);
    font: 12px system-ui, sans-serif;
    color: #eee;
    user-select: none;
    overflow: hidden;
  }
  @media (prefers-color-scheme: dark) {
    .dock {
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
  /* In a bottom corner the strip is the drawer's bottom row: the drawer unfolds up. */
  .dock.bottom {
    flex-direction: column-reverse;
  }
  .strip {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 0.25em;
    flex: 0 0 42px;
    height: 42px;
    padding: 0 0.5em 0 0.3em;
  }
  .strip button {
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
  .strip button:hover:not(.on) {
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
    padding: 0 0.55em;
    border: 1px solid rgba(255, 255, 255, 0.25) !important;
  }
  .name.on {
    background: #eee;
    color: #111;
    opacity: 1;
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
  .drawer-toggle.on {
    background: rgba(255, 255, 255, 0.2);
  }
  .quit {
    margin-left: 0.15em;
    opacity: 0.6;
  }
  .quit:hover {
    opacity: 1;
    background: #b32020 !important;
    color: #fff;
  }

  /* The drawer: the panel, in the panel's own colours, under (or over) the strip. */
  .drawer {
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    min-height: 0;
    background: var(--bg);
    color: var(--fg);
    font: 13px/1.5 system-ui, sans-serif;
    user-select: text;
  }
  .dock:not(.bottom) .drawer {
    border-top: 1px solid rgba(255, 255, 255, 0.16);
  }
  .dock.bottom .drawer {
    border-bottom: 1px solid rgba(255, 255, 255, 0.16);
  }
  nav {
    display: flex;
    gap: 2px;
    padding: 8px 10px 0;
    border-bottom: 1px solid var(--line);
    flex: 0 0 auto;
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
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
    padding: 12px 14px;
  }
</style>
