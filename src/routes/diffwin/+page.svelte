<script lang="ts">
  // The diff window (adr.rg.028): the Diff tab's view of one session in a frameless
  // window of its own. It shows the session of the pane locked by a click and follows
  // the lock (Follow's sighting, `follow:session`); with nothing locked, the drawer's
  // choice made by hand (`selection:changed`). Its title row names the session, moves
  // the window and closes it; its edges resize it.
  //
  // Attached to a side of the glass (adr.rg.034): four side buttons in the title row
  // attach it with one click, the lit one naming the side. A move or a resize the user
  // starts here is reported when it ends: dropped beside the glass it attaches to that
  // side (a resize of the free edge sets its thickness), dropped away from it it is
  // free and stays there. ReviewGlass's own placement is never reported.
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Diff from "$lib/panel/Diff.svelte";
  import { selection, follow, followSaw, type FollowSaw, type FollowStatus } from "$lib/panel/selection.svelte";

  const win = getCurrentWindow();

  /** A move or resize the user started: the geometry that follows is theirs to keep. */
  let byHand = false;
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  function saveSoon() {
    if (!byHand) return;
    clearTimeout(saveTimer);
    saveTimer = setTimeout(async () => {
      byHand = false;
      const p = await win.outerPosition();
      const s = await win.outerSize();
      await invoke("diffwin_dropped", { x: p.x, y: p.y, width: s.width, height: s.height });
    }, 400);
  }

  type Side = "left" | "right" | "above" | "below";
  /** The side of the glass the window is attached to; null: free. */
  let attached = $state<Side | null>(null);
  const SIDES: { side: Side; glyph: string; name: string }[] = [
    { side: "left", glyph: "◧", name: "left of" },
    { side: "right", glyph: "◨", name: "right of" },
    { side: "above", glyph: "⬒", name: "above" },
    { side: "below", glyph: "⬓", name: "below" },
  ];
  function attach(side: Side) {
    void invoke("diffwin_attach", { side });
  }

  function startMove(e: PointerEvent) {
    if (e.button !== 0) return;
    byHand = true;
    void win.startDragging();
  }

  const GRIPS = [
    ["n", "North"],
    ["s", "South"],
    ["e", "East"],
    ["w", "West"],
    ["ne", "NorthEast"],
    ["nw", "NorthWest"],
    ["se", "SouthEast"],
    ["sw", "SouthWest"],
  ] as const;

  function startResize(e: PointerEvent, dir: (typeof GRIPS)[number][1]) {
    if (e.button !== 0) return;
    e.stopPropagation();
    byHand = true;
    void win.startResizeDragging(dir);
  }

  function close() {
    void invoke("diffwin_toggle");
  }

  onMount(() => {
    const unlisten: (() => void)[] = [];
    void (async () => {
      // What Follow (or the lock) last saw, so the window opens on that session.
      try {
        const s = await invoke<FollowStatus>("follow_session_state");
        if (s.saw) followSaw(s.saw);
      } catch {
        // The core is not answering yet: the next sighting will do.
      }
      unlisten.push(await listen<FollowSaw>("follow:session", (ev) => followSaw(ev.payload)));
      unlisten.push(await listen<Side | null>("diffwin:attached", (ev) => (attached = ev.payload)));
      attached = await invoke<Side | null>("diffwin_attached");
      // A choice made by hand in the drawer. Set directly, not through choose(), which
      // would send it on again.
      unlisten.push(
        await listen<{ session: string | null; name: string | null }>("selection:changed", (ev) => {
          selection.session = ev.payload.session;
          selection.name = ev.payload.name;
          selection.by = "hand";
        }),
      );
      unlisten.push(await win.onMoved(saveSoon));
      unlisten.push(await win.onResized(saveSoon));
    })();
    return () => unlisten.forEach((u) => u());
  });
</script>

<div class="diffwin">
  <header role="toolbar" aria-label="Diff window" tabindex="-1" onpointerdown={startMove}>
    <span class="title" title="Drag to move the window; drop it beside the glass to attach it there">
      Diff{" — "}{#if selection.name}<b>{selection.name}</b>{:else}every session{/if}
    </span>
    {#if attached === null}
      <span class="hint">free — drag beside the glass to attach</span>
    {/if}
    <span class="sides" role="group" aria-label="Attach to a side of the glass">
      {#each SIDES as s (s.side)}
        <button
          class="side"
          class:on={attached === s.side}
          aria-pressed={attached === s.side}
          title={attached === s.side
            ? `Attached ${s.name} the glass: it follows the glass's place and size`
            : `Attach ${s.name} the glass`}
          aria-label={`Attach ${s.name} the glass`}
          onpointerdown={(e) => {
            e.stopPropagation();
            if (e.button === 0) attach(s.side);
          }}>{s.glyph}</button
        >
      {/each}
    </span>
    <button
      class="close"
      title="Close the diff window (Ctrl+Alt+D, or the glass's ⧉)"
      aria-label="Close the diff window"
      onpointerdown={(e) => {
        e.stopPropagation();
        close();
      }}>✕</button
    >
  </header>
  {#if follow.saw && !follow.saw.session_id}
    <p class="unmatched" role="status">
      The pane "{follow.saw.title}" names no session ReviewGlass can find (live, or in the
      last three days' transcripts), so the diff still shows {selection.name ? `"${selection.name}"` : "every session"}.
    </p>
  {/if}
  <section>
    <Diff />
  </section>
  {#each GRIPS as [name, dir] (name)}
    <div class="grip {name}" role="presentation" onpointerdown={(e) => startResize(e, dir)}></div>
  {/each}
</div>

<style>
  :global(html, body) {
    margin: 0;
    overflow: hidden;
  }
  /* The drawer's theme, so the two read alike. */
  .diffwin {
    --bg: #ffffff;
    --fg: #1b1b1b;
    --muted: #6a6a6a;
    --line: #e0e0e0;
    --raised: #f6f6f6;
    --accent: #2f6feb;
    --warn: #b06000;
    --bad: #b32020;
    position: relative;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    width: 100vw;
    height: 100vh;
    border: 1px solid var(--line);
    background: var(--bg);
    color: var(--fg);
    font: 13px system-ui, sans-serif;
  }
  @media (prefers-color-scheme: dark) {
    .diffwin {
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
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 0 0 auto;
    padding: 4px 6px 4px 12px;
    background: #202020;
    color: #eee;
    cursor: move;
    user-select: none;
  }
  .title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
  }
  .hint {
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11px;
    color: #aaa;
  }
  .sides {
    display: flex;
    gap: 2px;
    flex: 0 0 auto;
  }
  .side {
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: #bbb;
    font: inherit;
    font-size: 14px;
    line-height: 1;
    padding: 3px 5px;
    cursor: pointer;
  }
  .side:hover {
    background: #3a3a3a;
    color: #fff;
  }
  .side.on {
    background: #2f6feb;
    color: #fff;
  }
  .close {
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: #eee;
    font: inherit;
    padding: 2px 8px;
    cursor: pointer;
  }
  .close:hover {
    background: #b32020;
  }
  .unmatched {
    margin: 0;
    padding: 6px 12px;
    font-size: 12px;
    color: var(--warn);
    border-bottom: 1px solid var(--line);
  }
  section {
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
    padding: 10px 12px;
  }
  .grip {
    position: absolute;
  }
  .grip.n { top: 0; left: 10px; right: 10px; height: 4px; cursor: ns-resize; }
  .grip.s { bottom: 0; left: 10px; right: 10px; height: 6px; cursor: ns-resize; }
  .grip.w { left: 0; top: 10px; bottom: 10px; width: 5px; cursor: ew-resize; }
  .grip.e { right: 0; top: 10px; bottom: 10px; width: 5px; cursor: ew-resize; }
  .grip.nw { top: 0; left: 0; width: 10px; height: 4px; cursor: nwse-resize; }
  .grip.ne { top: 0; right: 0; width: 10px; height: 4px; cursor: nesw-resize; }
  .grip.se { bottom: 0; right: 0; width: 14px; height: 14px; cursor: nwse-resize; }
  .grip.sw { bottom: 0; left: 0; width: 14px; height: 14px; cursor: nesw-resize; }
</style>
