<script lang="ts">
  // Glass window (rg.glass-window): frameless, transparent, always on top.
  //
  // The Rust engine hands over the source pixels at 1:1; this canvas draws them at the
  // chosen zoom. Polling is adaptive: ~30 fps while frames change, ~5 fps once a frozen
  // source has been static, so an idle glass costs almost nothing.
  //
  // The controls live in a bar that appears on hover and fades when the pointer leaves,
  // because the glass is a reading surface: chrome sitting over magnified text defeats
  // the point of magnifying it. Every control also has a keyboard or mouse equivalent,
  // so nothing is reachable only by hunting for a button.
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { PhysicalSize } from "@tauri-apps/api/dpi";

  type GlassState = {
    zoom: number;
    frozen: boolean;
    lens: boolean;
    config: "loaded" | "fresh" | "reset-corrupt";
  };

  const win = getCurrentWindow();
  const HEADER = 16;
  const ACTIVE_MS = 1000 / 30;
  const IDLE_MS = 1000 / 5;
  const IDLE_AFTER = 20; // unchanged polls before dropping to the idle rate
  const ZOOM_MIN = 1.5;
  const ZOOM_MAX = 4.0;
  const ZOOM_STEP = 0.25;
  const SIZE_STEP = 1.25; // one press grows or shrinks the window by a quarter

  let canvas: HTMLCanvasElement;
  let zoom = $state(2);
  let frozen = $state(false);
  let lens = $state(false);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let haveFrame = $state(false);
  let barPinned = $state(true); // shown at startup so the controls are discoverable
  let hovering = $state(false);
  const barVisible = $derived(barPinned || hovering);

  let seq = 0;
  let unchanged = 0;
  let stopped = false;
  let offscreen: OffscreenCanvas | null = null;

  function dpr() {
    return window.devicePixelRatio || 1;
  }

  /** `invoke` hands back a Uint8Array on some platforms and an ArrayBuffer on others. */
  function asArrayBuffer(v: unknown): ArrayBuffer {
    if (v instanceof ArrayBuffer) return v;
    if (ArrayBuffer.isView(v)) {
      const view = v as ArrayBufferView;
      return view.buffer.slice(view.byteOffset, view.byteOffset + view.byteLength) as ArrayBuffer;
    }
    if (Array.isArray(v)) return new Uint8Array(v).buffer;
    throw new TypeError(`unexpected frame payload: ${Object.prototype.toString.call(v)}`);
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
      const buf = asArrayBuffer(await invoke("glass_frame", { since: seq }));
      if (buf.byteLength >= HEADER) {
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
      }
      error = null;
      // While following the cursor the source moves even when the screen does not, so
      // stay at the active rate; only a frozen static source is truly idle.
      if (frozen && unchanged >= IDLE_AFTER) delay = IDLE_MS;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      delay = IDLE_MS;
    }
    setTimeout(poll, delay);
  }

  async function setZoom(next: number) {
    zoom = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, Math.round(next / ZOOM_STEP) * ZOOM_STEP));
    await reportView();
  }

  async function setFrozen(next: boolean) {
    frozen = next;
    if (next) lens = false;
    await invoke("glass_set_frozen", { frozen: next });
  }

  // The lens rides on the cursor and passes clicks through, so once it is on, its own
  // bar cannot switch it off; the hotkey and the tray can, and the badge says so.
  async function setLens(next: boolean) {
    lens = next;
    if (next) frozen = false;
    hovering = false;
    await invoke("glass_set_lens", { lens: next });
  }

  // While the pointer is over the glass the source holds still and the picture dims,
  // so the controls read clearly and nothing jumps underneath them.
  function setHover(next: boolean) {
    if (lens) return;
    hovering = next;
    void invoke("glass_set_hovered", { hovered: next });
  }

  async function resizeBy(factor: number) {
    const s = await win.innerSize();
    const w = Math.round(Math.min(4000, Math.max(160, s.width * factor)));
    const h = Math.round(Math.min(2000, Math.max(60, s.height * factor)));
    await win.setSize(new PhysicalSize(w, h));
    await reportView();
  }

  async function onwheel(e: WheelEvent) {
    e.preventDefault();
    if (frozen) {
      // Scroll the source rectangle in source pixels; a notch is roughly three lines.
      const step = Math.round(Math.max(1, 40 / zoom));
      const dir = Math.sign(e.deltaY) * step;
      const dx = e.shiftKey ? dir : Math.sign(e.deltaX) * step;
      const dy = e.shiftKey ? 0 : dir;
      await invoke("glass_scroll", { dx, dy });
    } else {
      await setZoom(zoom + (e.deltaY < 0 ? ZOOM_STEP : -ZOOM_STEP));
    }
  }

  function onpointerdown(e: PointerEvent) {
    if (e.button === 0 && e.detail === 1) void win.startDragging();
  }

  function ondblclick() {
    void setFrozen(!frozen);
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "f" || e.key === "F") void setFrozen(!frozen);
    else if (e.key === "l" || e.key === "L") void setLens(!lens);
    else if (e.key === "Escape") void invoke("glass_hide");
    else if (e.key === "+" || e.key === "=") void setZoom(zoom + ZOOM_STEP);
    else if (e.key === "-" || e.key === "_") void setZoom(zoom - ZOOM_STEP);
  }

  // A control must not also drag or freeze the window, so each swallows its own event.
  function control(e: Event, run: () => void) {
    e.stopPropagation();
    e.preventDefault();
    run();
  }

  // Grips resize the window: a frameless transparent window has no system border.
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

  onMount(() => {
    const unlisten: (() => void)[] = [];
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
      lens = s.lens;
      if (s.config === "reset-corrupt") {
        notice = "Settings were unreadable; defaults are in effect (the old file is kept as config.json.bak).";
        setTimeout(() => (notice = null), 8000);
      }
      await reportView();
      unlisten.push(await win.onMoved(savePosition));
      unlisten.push(await win.onResized(() => void reportView()));
      unlisten.push(
        await listen<GlassState>("glass:state", (ev) => {
          zoom = ev.payload.zoom;
          frozen = ev.payload.frozen;
          lens = ev.payload.lens;
          if (lens) hovering = false;
        }),
      );
      void poll();
    })();

    const unpin = setTimeout(() => (barPinned = false), 4000);
    return () => {
      stopped = true;
      clearTimeout(unpin);
      unlisten.forEach((u) => u());
    };
  });
</script>

<svelte:window {onkeydown} />

<div
  class="glass"
  class:frozen
  class:lens
  class:dimmed={hovering && !lens}
  {onpointerdown}
  {ondblclick}
  {onwheel}
  onpointerenter={() => setHover(true)}
  onpointerleave={() => setHover(false)}
  role="presentation"
>
  <canvas bind:this={canvas}></canvas>
  <div class="dim" aria-hidden="true"></div>

  {#if lens}
    <div class="lens-badge">Lens · Ctrl+Alt+L or the tray icon to release</div>
  {/if}

  {#if error}
    <div class="overlay error"><span>{error}</span></div>
  {:else if !haveFrame}
    <div class="overlay hint"><span>Waiting for the first frame…</span></div>
  {/if}
  {#if notice}
    <div class="overlay notice">{notice}</div>
  {/if}

  <div class="bar" class:visible={barVisible && !lens} role="toolbar" tabindex="-1" aria-label="Glass controls">
    <button
      class="move"
      title="Drag to move the glass"
      aria-label="Move the glass"
      onpointerdown={(e) => {
        if (e.button !== 0) return;
        e.stopPropagation();
        void win.startDragging();
      }}>✥</button
    >

    <span class="sep"></span>

    <button
      title="Zoom out (− or wheel down)"
      aria-label="Zoom out"
      disabled={zoom <= ZOOM_MIN}
      onpointerdown={(e) => control(e, () => setZoom(zoom - ZOOM_STEP))}>−</button
    >
    <span class="value" aria-live="polite">{Math.round(zoom * 100)}%</span>
    <button
      title="Zoom in (+ or wheel up)"
      aria-label="Zoom in"
      disabled={zoom >= ZOOM_MAX}
      onpointerdown={(e) => control(e, () => setZoom(zoom + ZOOM_STEP))}>+</button
    >

    <span class="sep"></span>

    <button
      title="Smaller window"
      aria-label="Smaller window"
      onpointerdown={(e) => control(e, () => resizeBy(1 / SIZE_STEP))}>▭−</button
    >
    <button
      title="Larger window"
      aria-label="Larger window"
      onpointerdown={(e) => control(e, () => resizeBy(SIZE_STEP))}>▭+</button
    >

    <span class="sep"></span>

    <button
      title="Open the sessions panel"
      aria-label="Open the sessions panel"
      onpointerdown={(e) => control(e, () => invoke("panel_show"))}>▤</button
    >
    <button
      class:active={frozen}
      title={frozen ? "Follow the cursor (F)" : "Freeze this region (F)"}
      aria-label="Freeze"
      aria-pressed={frozen}
      onpointerdown={(e) => control(e, () => setFrozen(!frozen))}>{frozen ? "❄" : "⌖"}</button
    >
    <button
      title="Lens: ride on the cursor, clicks pass through (L, Ctrl+Alt+L to release)"
      aria-label="Lens mode"
      onpointerdown={(e) => control(e, () => setLens(true))}>◎</button
    >
    <button
      title="Hide — Ctrl+Alt+G or the tray icon brings it back (Esc)"
      aria-label="Hide"
      onpointerdown={(e) => control(e, () => invoke("glass_hide"))}>▁</button
    >
    <button
      class="quit"
      title="Quit ReviewGlass"
      aria-label="Quit"
      onpointerdown={(e) => control(e, () => invoke("app_quit"))}>✕</button
    >
  </div>

  {#each GRIPS as [name, dir] (name)}
    <div class="grip {name}" role="presentation" onpointerdown={(e) => startResize(e, dir)}></div>
  {/each}
</div>

<style>
  :global(html, body) {
    margin: 0;
    background: transparent;
    overflow: hidden;
  }
  .glass {
    position: relative;
    box-sizing: border-box;
    width: 100vw;
    height: 100vh;
    border: 2px solid rgba(255, 200, 0, 0.9);
    border-radius: 4px;
    overflow: hidden;
    background: rgba(0, 0, 0, 0.35);
    cursor: move;
    user-select: none;
  }
  .glass.frozen {
    border-color: rgba(80, 180, 255, 0.95);
  }
  .glass.lens {
    border-color: rgba(255, 255, 255, 0.85);
    border-radius: 10px;
    cursor: none;
  }
  /* Darken the picture while the controls are up, so they read as controls and not as
     part of what is being magnified. */
  .dim {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    opacity: 0;
    transition: opacity 140ms ease;
    pointer-events: none;
  }
  .glass.dimmed .dim {
    opacity: 1;
  }
  .lens-badge {
    position: absolute;
    left: 50%;
    bottom: 6px;
    transform: translateX(-50%);
    padding: 3px 8px;
    border-radius: 6px;
    background: rgba(20, 20, 20, 0.82);
    font: 11px system-ui, sans-serif;
    color: #eee;
    white-space: nowrap;
    pointer-events: none;
  }
  canvas {
    display: block;
    width: 100%;
    height: 100%;
  }

  .overlay {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    padding: 12px;
    font: 12px system-ui, sans-serif;
    color: #fff;
    text-align: center;
    pointer-events: none;
  }
  .overlay span {
    max-width: 90%;
    padding: 6px 10px;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.75);
  }
  .overlay.error span {
    background: rgba(150, 20, 20, 0.9);
  }
  .overlay.notice {
    inset: auto 0 0 0;
    padding: 6px 10px;
    background: rgba(0, 0, 0, 0.75);
  }

  .bar {
    position: absolute;
    top: 4px;
    right: 6px;
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 2px 4px;
    border-radius: 6px;
    background: rgba(20, 20, 20, 0.82);
    font: 12px system-ui, sans-serif;
    color: #eee;
    opacity: 0;
    transition: opacity 140ms ease;
    pointer-events: none;
  }
  .bar.visible {
    opacity: 1;
    pointer-events: auto;
  }
  .bar button {
    min-width: 22px;
    height: 20px;
    padding: 0 4px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    line-height: 1;
    cursor: pointer;
  }
  .bar button:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.18);
  }
  .bar button:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .bar button.active {
    background: rgba(80, 180, 255, 0.35);
  }
  .bar button.quit:hover {
    background: rgba(200, 40, 40, 0.85);
  }
  .bar button.move {
    cursor: move;
    font-size: 14px;
  }
  .value {
    min-width: 38px;
    text-align: center;
    opacity: 0.85;
    font-variant-numeric: tabular-nums;
  }
  .sep {
    width: 1px;
    height: 14px;
    margin: 0 3px;
    background: rgba(255, 255, 255, 0.22);
  }

  .grip {
    position: absolute;
  }
  .grip.n { top: 0; left: 8px; right: 8px; height: 6px; cursor: ns-resize; }
  .grip.s { bottom: 0; left: 8px; right: 8px; height: 6px; cursor: ns-resize; }
  .grip.w { left: 0; top: 8px; bottom: 8px; width: 6px; cursor: ew-resize; }
  .grip.e { right: 0; top: 8px; bottom: 8px; width: 6px; cursor: ew-resize; }
  .grip.nw { top: 0; left: 0; width: 12px; height: 12px; cursor: nwse-resize; }
  .grip.se { bottom: 0; right: 0; width: 12px; height: 12px; cursor: nwse-resize; }
  .grip.ne { top: 0; right: 0; width: 12px; height: 12px; cursor: nesw-resize; }
  .grip.sw { bottom: 0; left: 0; width: 12px; height: 12px; cursor: nesw-resize; }
</style>
