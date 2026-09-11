<script lang="ts">
  // Glass window (rg.glass-window): frameless, transparent, always on top.
  //
  // The Rust engine hands over the source pixels at 1:1; this canvas draws them at
  // the chosen zoom. Polling is adaptive: ~30 fps while frames change, ~5 fps once
  // the source has been static for a moment, so an idle glass costs almost nothing.
  //
  // Interaction:
  //   drag body      move the glass; drag an edge or corner to resize
  //   wheel          zoom (following) / scroll the source (frozen); shift = horizontal
  //   double-click   toggle freeze
  //   F              toggle freeze     Esc  hide (Ctrl+Alt+G shows it again)
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  type GlassState = { zoom: number; frozen: boolean; config: "loaded" | "fresh" | "reset-corrupt" };

  const win = getCurrentWindow();
  const HEADER = 16;
  const ACTIVE_MS = 1000 / 30;
  const IDLE_MS = 1000 / 5;
  const IDLE_AFTER = 20; // unchanged polls before dropping to the idle rate

  let canvas: HTMLCanvasElement;
  let zoom = $state(2);
  let frozen = $state(false);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let haveFrame = $state(false);

  let seq = 0;
  let unchanged = 0;
  let stopped = false;
  let offscreen: OffscreenCanvas | null = null;

  function dpr() {
    return window.devicePixelRatio || 1;
  }

  async function reportView() {
    const w = Math.max(2, Math.round(canvas.clientWidth * dpr()));
    const h = Math.max(2, Math.round(canvas.clientHeight * dpr()));
    canvas.width = w;
    canvas.height = h;
    zoom = await invoke<number>("glass_set_view", { widthPx: w, heightPx: h, zoom });
  }

  function draw(w: number, h: number, rgba: Uint8ClampedArray<ArrayBuffer>) {
    if (!offscreen || offscreen.width !== w || offscreen.height !== h) {
      offscreen = new OffscreenCanvas(w, h);
    }
    const octx = offscreen.getContext("2d")!;
    octx.putImageData(new ImageData(rgba, w, h), 0, 0);
    const ctx = canvas.getContext("2d")!;
    ctx.imageSmoothingEnabled = true;
    ctx.imageSmoothingQuality = "high";
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.drawImage(offscreen, 0, 0, canvas.width, canvas.height);
    haveFrame = true;
  }

  async function poll() {
    if (stopped) return;
    let delay = ACTIVE_MS;
    try {
      const buf = await invoke<ArrayBuffer>("glass_frame", { since: seq });
      const view = new DataView(buf);
      const newSeq = Number(view.getBigUint64(0, true));
      const w = view.getUint32(8, true);
      const h = view.getUint32(12, true);
      if (w > 0 && h > 0 && buf.byteLength >= HEADER + w * h * 4) {
        seq = newSeq;
        unchanged = 0;
        draw(w, h, new Uint8ClampedArray(buf, HEADER, w * h * 4));
      } else {
        unchanged++;
      }
      error = null;
      // While following the cursor the source moves even when the screen does not,
      // so stay at the active rate; only a frozen static source is truly idle.
      if (frozen && unchanged >= IDLE_AFTER) delay = IDLE_MS;
    } catch (e) {
      error = String(e);
      delay = IDLE_MS;
    }
    setTimeout(poll, delay);
  }

  async function setFrozen(next: boolean) {
    frozen = next;
    await invoke("glass_set_frozen", { frozen: next });
  }

  async function onwheel(e: WheelEvent) {
    e.preventDefault();
    if (frozen) {
      // Scroll the source rectangle in source pixels; a wheel notch is ~3 lines.
      const step = Math.round(Math.max(1, 40 / zoom));
      const dir = Math.sign(e.deltaY) * step;
      const dx = e.shiftKey ? dir : Math.sign(e.deltaX) * step;
      const dy = e.shiftKey ? 0 : dir;
      await invoke("glass_scroll", { dx, dy });
    } else {
      zoom = Math.round((zoom + (e.deltaY < 0 ? 0.25 : -0.25)) * 4) / 4;
      await reportView();
    }
  }

  function onpointerdown(e: PointerEvent) {
    if (e.button === 0 && e.detail === 1) void win.startDragging();
  }

  // A frameless transparent window has no system resize border, so the grips are
  // explicit: a strip on each edge and a wider patch in each corner.
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
    void win.startResizeDragging(dir);
  }

  function ondblclick() {
    void setFrozen(!frozen);
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "f" || e.key === "F") void setFrozen(!frozen);
    else if (e.key === "Escape") void win.hide();
  }

  onMount(() => {
    let unlisten: (() => void)[] = [];
    let saveTimer: ReturnType<typeof setTimeout> | undefined;
    const savePosition = () => {
      clearTimeout(saveTimer);
      saveTimer = setTimeout(async () => {
        const p = await win.outerPosition();
        await invoke("glass_save_position", { x: p.x, y: p.y });
      }, 400);
    };

    (async () => {
      const s = await invoke<GlassState>("glass_state");
      zoom = s.zoom;
      frozen = s.frozen;
      if (s.config === "reset-corrupt") {
        notice = "Settings file was unreadable; defaults are in effect (the old file is kept as config.json.bak).";
        setTimeout(() => (notice = null), 8000);
      }
      await reportView();
      unlisten.push(await win.onMoved(savePosition));
      unlisten.push(await win.onResized(() => void reportView()));
      unlisten.push(
        await listen<GlassState>("glass:state", (ev) => {
          zoom = ev.payload.zoom;
          frozen = ev.payload.frozen;
        }),
      );
      void poll();
    })();

    return () => {
      stopped = true;
      unlisten.forEach((u) => u());
    };
  });
</script>

<svelte:window {onkeydown} />

<div class="glass" class:frozen {onpointerdown} {ondblclick} {onwheel} role="presentation">
  <canvas bind:this={canvas}></canvas>
  {#if error}
    <div class="overlay error">{error}</div>
  {:else if !haveFrame}
    <div class="overlay hint">Waiting for the first frame…</div>
  {/if}
  {#if notice}
    <div class="overlay notice">{notice}</div>
  {/if}
  <div class="badge">{Math.round(zoom * 100)}%{frozen ? " · frozen" : ""}</div>
  {#each GRIPS as [name, dir]}
    <div
      class="grip {name}"
      role="presentation"
      onpointerdown={(e) => startResize(e, dir)}
    ></div>
  {/each}
</div>

<style>
  :global(html, body) { margin: 0; background: transparent; overflow: hidden; }
  .glass {
    position: relative; box-sizing: border-box; width: 100vw; height: 100vh;
    border: 2px solid rgba(255, 200, 0, 0.9); border-radius: 4px; overflow: hidden;
    background: rgba(0, 0, 0, 0.35); cursor: move; user-select: none;
  }
  .glass.frozen { border-color: rgba(80, 180, 255, 0.95); }
  canvas { display: block; width: 100%; height: 100%; }
  .overlay {
    position: absolute; inset: 0; display: grid; place-items: center; padding: 12px;
    font: 12px system-ui, sans-serif; color: #fff; text-shadow: 0 0 3px #000; text-align: center;
    pointer-events: none;
  }
  .overlay.error { background: rgba(120, 0, 0, 0.6); }
  .overlay.notice { inset: auto 0 0 0; background: rgba(0, 0, 0, 0.7); padding: 6px 10px; }
  .badge {
    position: absolute; top: 4px; right: 6px; font: 11px system-ui, sans-serif;
    color: #fff; text-shadow: 0 0 3px #000; opacity: 0.8; pointer-events: none;
  }
  .grip { position: absolute; }
  .grip.n { top: 0; left: 8px; right: 8px; height: 6px; cursor: ns-resize; }
  .grip.s { bottom: 0; left: 8px; right: 8px; height: 6px; cursor: ns-resize; }
  .grip.w { left: 0; top: 8px; bottom: 8px; width: 6px; cursor: ew-resize; }
  .grip.e { right: 0; top: 8px; bottom: 8px; width: 6px; cursor: ew-resize; }
  .grip.nw { top: 0; left: 0; width: 12px; height: 12px; cursor: nwse-resize; }
  .grip.se { bottom: 0; right: 0; width: 12px; height: 12px; cursor: nwse-resize; }
  .grip.ne { top: 0; right: 0; width: 12px; height: 12px; cursor: nesw-resize; }
  .grip.sw { bottom: 0; left: 0; width: 12px; height: 12px; cursor: nesw-resize; }
</style>
