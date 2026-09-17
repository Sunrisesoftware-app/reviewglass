<script lang="ts">
  // Glass window (rg.glass-window): frameless, transparent, always on top.
  //
  // The Rust engine hands over the source pixels at 1:1; this canvas draws them at the
  // chosen zoom. Polling is adaptive: ~30 fps while frames change, ~5 fps once a frozen
  // source has been static, so an idle glass costs almost nothing.
  //
  // Three modes. Follow: the window stays put and the source rides the cursor. Lens: the
  // window itself rides the cursor at its own smaller size. Frozen: a still — the
  // picture stops updating so a captured instruction survives the user switching to
  // another application underneath it; drag the still wherever it should live.
  //
  // The controls live in a title bar that is always there, like any other window's:
  // the first hands-on session found a bar that comes and goes more confusing than a
  // strip of chrome above the picture. In the lens the cursor is always at the window's
  // centre, so the title bar's buttons cannot be reached by mouse; there it shows what
  // the keys and the right button do instead. Every control also has a keyboard, wheel
  // or right-click equivalent.
  //
  // Pane lock and Fit (adr.rg.017): the engine detects the column under the cursor
  // from pixels. Locked, Follow tracks the cursor vertically only; Fit lets this
  // window's width follow the column at the current zoom, capped at the monitor. The
  // finder window frames the source rectangle on screen so a wrong guess is visible.
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { PhysicalPosition, PhysicalSize } from "@tauri-apps/api/dpi";

  type GlassState = {
    zoom: number;
    frozen: boolean;
    lens: boolean;
    halo: boolean;
    ui_scale: number;
    pane_lock: boolean;
    pane_fit: boolean;
    pane_width: number | null;
    build: string;
    follow_log: boolean;
    follow_log_path: string | null;
    follow_log_since: number | null;
    follow_log_lines: number;
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
  const FIT_MIN_WIDTH = 480; // narrower than this the bar itself would not fit
  const FIT_MARGIN = 40; // kept free at the monitor's edges when fitting
  const FIT_SLACK = 16; // a pane that changed by less does not move the window
  const FIT_SHRINK_AFTER = 2000; // ms a narrower reading must hold before the glass narrows
  const BORDER_CSS = 3; // the glass's border, per side, in CSS px

  let canvas: HTMLCanvasElement;
  let zoom = $state(2); // in effect
  let userZoom = $state(2); // the setting; the fit may show less, never more
  const zoomDerived = $derived(Math.abs(zoom - userZoom) > 0.001);
  let frozen = $state(false);
  let lens = $state(false);
  let halo = $state(true);
  let uiScale = $state(1);
  let paneLock = $state(true);
  let paneFit = $state(true);
  let paneWidth = $state<number | null>(null);
  let build = $state("");
  let followLog = $state(false);

  // The measurement log (Settings, Measurements): Fit's side of the story, so the
  // detector's readings and the glass's reaction to them read in one file.
  function mlog(line: string) {
    if (followLog) void invoke("glass_log", { line });
  }
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let haveFrame = $state(false);
  let hovering = $state(false);
  type ModeName = "follow" | "lens" | "still";
  const mode = $derived<ModeName>(lens ? "lens" : frozen ? "still" : "follow");
  // The bar reflows when the mode or the column value changes: keep the picture area's
  // reported size in step. Not before the saved state is in: a report sent earlier
  // would write the defaults over the settings.
  let ready = $state(false);
  $effect(() => {
    void mode;
    void paneLock;
    void paneWidth;
    if (ready) void relayout();
  });

  async function setHalo(next: boolean) {
    halo = next;
    await invoke("glass_set_halo", { halo: next });
  }

  async function setPaneLock(next: boolean) {
    paneLock = next;
    if (!next) paneWidth = null;
    await invoke("glass_set_pane_lock", { lock: next });
  }

  async function setPaneFit(next: boolean) {
    paneFit = next;
    await invoke("glass_set_pane_fit", { fit: next });
    if (next) await fitToPane();
  }

  // Resize this window so the pane fills it at the current zoom. When even the lowest
  // zoom that still fits the monitor is above the minimum, the zoom comes down to it:
  // Fit promises the whole line, not the number on the zoom control.
  //
  // Widen at once, narrow reluctantly: a wider reading means a line was being cut,
  // a narrower one is as likely a band with short lines as a narrower column, and a
  // glass that breathes with every row is tiring (the owner's word). A narrower
  // reading has to hold for FIT_SHRINK_AFTER before the window follows it.
  let fitting = false;
  let shrinkTimer: ReturnType<typeof setTimeout> | undefined;
  let shrinkDue = false;
  async function fitToPane() {
    if (fitting) return;
    if (!paneFit || !paneLock || mode !== "follow" || !paneWidth) {
      mlog(`fit skip fit=${paneFit ? 1 : 0} lock=${paneLock ? 1 : 0} mode=${mode} pane=${paneWidth ?? "none"} zoomBack=${zoomDerived ? 1 : 0}`);
      if (zoomDerived) {
        zoom = userZoom; // no column to fit: the user's own zoom is back
        await reportView();
      }
      return;
    }
    fitting = true;
    try {
      const scale = dpr();
      const border = Math.round(BORDER_CSS * 2 * scale);
      const maxW = Math.round(screen.availWidth * scale) - FIT_MARGIN;
      let z = userZoom;
      let want = Math.round(paneWidth * z) + border;
      if (want > maxW) {
        z = Math.max(ZOOM_MIN, Math.floor((maxW - border) / paneWidth / ZOOM_STEP) * ZOOM_STEP);
        want = Math.round(paneWidth * z) + border;
      }
      want = Math.max(FIT_MIN_WIDTH, Math.min(maxW, want));
      const size = await win.innerSize();
      const zoomChanged = Math.abs(z - zoom) > 0.001;
      const facts = `fit pane=${paneWidth} z=${z} want=${want} size=${size.width} max=${maxW}`;
      if (want < size.width - FIT_SLACK && !shrinkDue) {
        // Narrower: wait and see whether it holds.
        mlog(`${facts} -> shrink-wait ${FIT_SHRINK_AFTER}ms`);
        clearTimeout(shrinkTimer);
        shrinkTimer = setTimeout(() => {
          shrinkDue = true;
          void fitToPane().finally(() => (shrinkDue = false));
        }, FIT_SHRINK_AFTER);
        return;
      }
      clearTimeout(shrinkTimer);
      zoom = z;
      mlog(
        `${facts} -> ${Math.abs(want - size.width) > FIT_SLACK ? (want > size.width ? "widen" : "shrink") : "none"}${shrinkDue ? " after-wait" : ""}${zoomChanged ? " zoom-changed" : ""}`,
      );
      if (Math.abs(want - size.width) > FIT_SLACK) {
        // Keep the window on its monitor: a glass that grew past the right edge would
        // show its picture off screen.
        const pos = await win.outerPosition();
        // availLeft is non-standard but every Chromium ships it: the monitor's left
        // edge in the virtual desktop, CSS px.
        const availLeft = (screen as Screen & { availLeft?: number }).availLeft ?? 0;
        const left = Math.round(availLeft * scale);
        const right = left + Math.round(screen.availWidth * scale);
        const x = Math.max(left, Math.min(pos.x, right - want - Math.round(FIT_MARGIN / 2)));
        if (x !== pos.x) await win.setPosition(new PhysicalPosition(x, pos.y));
        await win.setSize(new PhysicalSize(want, size.height)); // onResized reports the view
      } else if (zoomChanged) {
        await reportView();
      }
    } finally {
      fitting = false;
    }
  }

  // The chrome grows in steps, picked from a menu. The glass exists because things
  // are too small to read; its own bar must not be one of them. The bar wraps to a
  // second row when its buttons no longer fit the window, so a larger size never
  // pushes a control out of sight.
  function uiScaleMenu() {
    void invoke("glass_ui_scale_menu");
  }

  // After a state change that can reflow the bar (its size, a toggle appearing), the
  // picture area may have changed height: re-measure once the DOM has settled.
  async function relayout() {
    await tick();
    await reportView();
  }

  function setMode(next: ModeName) {
    if (next === mode) return;
    if (next === "lens") void setLens(true);
    else if (next === "still") void setFrozen(true);
    else if (lens) void setLens(false);
    else void setFrozen(false);
  }

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
    if (canvas.width !== w || canvas.height !== h) {
      // Assigning a size wipes the canvas, and a still gets no frame to refill it:
      // size it only when the picture area really changed, and put the last picture
      // back at the new size.
      canvas.width = w;
      canvas.height = h;
      repaint();
    }
    zoom = await invoke<number>("glass_set_view", {
      widthPx: w,
      heightPx: h,
      zoom,
      derived: zoomDerived,
    });
  }

  /** The last frame, scaled onto the canvas. A still is redrawn from here whenever the
   *  canvas is resized, since no new frame will arrive to do it. */
  function repaint() {
    if (!offscreen) return;
    const ctx = canvas.getContext("2d")!;
    ctx.imageSmoothingEnabled = true;
    ctx.imageSmoothingQuality = "high";
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.drawImage(offscreen, 0, 0, canvas.width, canvas.height);
  }

  function draw(w: number, h: number, rgba: Uint8ClampedArray<ArrayBuffer>) {
    if (!offscreen || offscreen.width !== w || offscreen.height !== h) {
      offscreen = new OffscreenCanvas(w, h);
    }
    const octx = offscreen.getContext("2d")!;
    octx.putImageData(new ImageData(rgba, w, h), 0, 0);
    repaint();
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
    if (frozen) return; // a still has fixed pixels; zoom returns when it resumes
    userZoom = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, Math.round(next / ZOOM_STEP) * ZOOM_STEP));
    zoom = userZoom;
    await reportView();
    await fitToPane(); // the same pane at a new zoom is a new width
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
    await invoke("glass_set_lens", { lens: next });
  }

  // While the pointer is over the glass the source holds still and the picture dims,
  // so the controls read clearly and nothing jumps underneath them.
  function setHover(next: boolean) {
    hovering = next;
    // In the lens the pointer is always over the glass; holding the source there would
    // freeze the lens, so the hold applies to the parked glass only.
    void invoke("glass_set_hovered", { hovered: next && !lens });
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
    if (frozen) return; // a still neither scrolls nor zooms
    await setZoom(zoom + (e.deltaY < 0 ? ZOOM_STEP : -ZOOM_STEP));
  }

  function oncontextmenu(e: MouseEvent) {
    e.preventDefault();
    void invoke("glass_menu");
  }

  // The title bar and the picture both drag the window: the title bar because that is
  // where every other window is dragged, the picture because a still is a thing one
  // grabs and moves.
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
      if (lens) return; // the lens moves every few ms; its position is not a setting
      clearTimeout(saveTimer);
      saveTimer = setTimeout(async () => {
        const p = await win.outerPosition();
        await invoke("glass_save_position", { x: p.x, y: p.y });
      }, 400);
    };

    // The webview can be up before the core has registered its state (the windows
    // in tauri.conf.json are created before setup runs), and the first invoke is
    // then rejected. Ask again until it answers, and say so meanwhile rather than
    // sit on "waiting for the first frame" forever.
    async function stateWhenReady(): Promise<GlassState> {
      for (let attempt = 0; ; attempt++) {
        try {
          return await invoke<GlassState>("glass_state");
        } catch (e) {
          if (stopped) throw e;
          if (attempt >= 4) error = `The app core is not answering yet (${e instanceof Error ? e.message : String(e)})`;
          await new Promise((r) => setTimeout(r, 250));
        }
      }
    }

    (async () => {
      const s = await stateWhenReady();
      error = null;
      zoom = s.zoom;
      userZoom = s.zoom;
      frozen = s.frozen;
      lens = s.lens;
      halo = s.halo;
      uiScale = s.ui_scale;
      paneLock = s.pane_lock;
      paneFit = s.pane_fit;
      paneWidth = s.pane_width;
      build = s.build;
      followLog = s.follow_log;
      if (s.config === "reset-corrupt") {
        notice = "Settings were unreadable; defaults are in effect (the old file is kept as config.json.bak).";
        setTimeout(() => (notice = null), 8000);
      }
      await reportView();
      unlisten.push(await win.onMoved(savePosition));
      unlisten.push(
        await win.onResized(async () => {
          await reportView();
          const s = await win.innerSize();
          mlog(`resized ${s.width}x${s.height}`);
          await invoke("glass_save_size", { width: s.width, height: s.height });
        }),
      );
      unlisten.push(
        await listen<number>("glass:zoom", (ev) => void setZoom(zoom + ev.payload)),
      );
      unlisten.push(
        await listen<GlassState>("glass:state", (ev) => {
          zoom = ev.payload.zoom;
          if (!zoomDerived) userZoom = zoom;
          frozen = ev.payload.frozen;
          lens = ev.payload.lens;
          halo = ev.payload.halo;
          uiScale = ev.payload.ui_scale;
          paneLock = ev.payload.pane_lock;
          paneFit = ev.payload.pane_fit;
          paneWidth = ev.payload.pane_width;
          followLog = ev.payload.follow_log;
          void relayout();
        }),
      );
      unlisten.push(
        await listen<{ width: number | null }>("glass:pane", (ev) => {
          paneWidth = ev.payload.width;
          void fitToPane();
        }),
      );
      ready = true;
      void poll();
    })();

    return () => {
      stopped = true;
      unlisten.forEach((u) => u());
    };
  });
</script>

<svelte:window {onkeydown} />

<div
  class="glass {mode}"
  class:dimmed={hovering && mode === "follow"}
  style="--ui: {uiScale}"
  onpointerenter={() => setHover(true)}
  onpointerleave={() => setHover(false)}
  role="presentation"
>
  <header class="titlebar" {onpointerdown} {oncontextmenu} role="toolbar" tabindex="-1" aria-label="ReviewGlass">
    <button
      class="grab"
      title="Drag to move the glass"
      aria-label="Move the glass"
      onpointerdown={(e) => {
        if (e.button !== 0) return;
        e.stopPropagation();
        void win.startDragging();
      }}>✥</button
    >
    <span class="name" title={build ? `Build ${build} — version, commit; a + means uncommitted changes` : ""}
      >ReviewGlass{#if build}<span class="build">{build}</span>{/if}</span
    >

    {#if mode === "lens"}
      <span class="hint">
        Lens — <b>double-click</b> to keep as a still · <b>right-click</b> for options ·
        <b>L</b> to leave
      </span>
    {:else}
      <div class="modes" role="radiogroup" aria-label="Mode">
        <button
          class:on={mode === "follow"}
          role="radio"
          aria-checked={mode === "follow"}
          title="The glass stays here and shows what is around the cursor"
          onpointerdown={(e) => control(e, () => setMode("follow"))}>Follow</button
        >
        <button
          role="radio"
          aria-checked="false"
          title="The glass rides on the cursor like a lens (L)"
          onpointerdown={(e) => control(e, () => setMode("lens"))}>Lens</button
        >
        <button
          class:on={mode === "still"}
          role="radio"
          aria-checked={mode === "still"}
          title="Keep this picture as a still; drag it anywhere (F)"
          onpointerdown={(e) => control(e, () => setMode("still"))}>Still</button
        >
      </div>

      <span class="sep"></span>

      <button
        title="Zoom out (− or wheel down)"
        aria-label="Zoom out"
        disabled={frozen || zoom <= ZOOM_MIN}
        onpointerdown={(e) => control(e, () => setZoom(zoom - ZOOM_STEP))}>−</button
      >
      <span
        class="value"
        class:derived={zoomDerived}
        aria-live="polite"
        title={zoomDerived
          ? `Lowered from ${Math.round(userZoom * 100)}% so the whole column fits the screen (Fit)`
          : "Zoom"}>{Math.round(zoom * 100)}%{zoomDerived ? "↓" : ""}</span
      >
      <button
        title={zoomDerived ? "The column would not fit the screen at a higher zoom (Fit is on)" : "Zoom in (+ or wheel up)"}
        aria-label="Zoom in"
        disabled={frozen || zoom >= ZOOM_MAX || zoomDerived}
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
        class:on={halo}
        title={halo ? "Cursor halo is on: a ring marks the pointer on screen" : "Show a ring around the pointer on screen"}
        aria-label="Cursor halo"
        aria-pressed={halo}
        onpointerdown={(e) => control(e, () => setHalo(!halo))}>◉</button
      >
      <button
        title="Bar size ({Math.round(uiScale * 100)}%) — click for a menu of sizes"
        aria-label="Bar size"
        aria-haspopup="menu"
        onpointerdown={(e) => control(e, () => uiScaleMenu())}>Aa</button
      >

      <span class="sep"></span>

      <button
        class:on={paneLock}
        title={paneLock
          ? "Column lock is on: the picture holds the column under the cursor and follows it up and down; the frame on screen shows the column"
          : "Lock the picture to the column under the cursor; a frame on screen shows what was found"}
        aria-label="Column lock"
        aria-pressed={paneLock}
        onpointerdown={(e) => control(e, () => setPaneLock(!paneLock))}>Column</button
      >
      <button
        class:on={paneFit && paneLock}
        disabled={!paneLock}
        title={!paneLock
          ? "Fit needs the column lock"
          : paneFit
            ? "Fit is on: the window's width follows the column at this zoom; the zoom comes down when a column is too wide for the screen"
            : "Let the window's width follow the column at this zoom"}
        aria-label="Fit width to the column"
        aria-pressed={paneFit && paneLock}
        onpointerdown={(e) => control(e, () => setPaneFit(!paneFit))}>Fit</button
      >
      {#if paneLock && mode === "follow"}
        <span class="value pane" aria-live="polite" title={paneWidth ? "Width of the column under the cursor, in screen pixels" : "No column boundaries were found around the cursor; the picture follows the cursor in both directions meanwhile"}>
          {paneWidth ? `${paneWidth} px` : "no column here"}
        </span>
      {/if}

      <span class="spacer"></span>

      <button
        title="Open the sessions panel"
        aria-label="Open the sessions panel"
        onpointerdown={(e) => control(e, () => invoke("panel_show"))}>▤</button
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
    {/if}
  </header>

  <div class="picture" {onpointerdown} {ondblclick} {onwheel} {oncontextmenu} role="presentation">
    <canvas bind:this={canvas}></canvas>
    <div class="dim" aria-hidden="true"></div>

    {#if error}
      <div class="overlay error"><span>{error}</span></div>
    {:else if !haveFrame}
      <div class="overlay hint"><span>Waiting for the first frame…</span></div>
    {/if}
    {#if notice}
      <div class="overlay notice">{notice}</div>
    {/if}
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

  /* One colour per mode, on the border and on the active mode button, so the state
     reads from across the room. */
  .glass {
    --mode: #ffc800;
    --ui: 1;
    position: relative;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    width: 100vw;
    height: 100vh;
    border: 3px solid var(--mode);
    border-radius: 6px;
    overflow: hidden;
    background: #161616;
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.7);
    font: calc(12px * var(--ui)) system-ui, sans-serif;
    color: #eee;
    user-select: none;
  }
  .glass.lens {
    --mode: #ffffff;
  }
  .glass.still {
    --mode: #5ab4ff;
  }

  .titlebar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.17em;
    min-height: calc(28px * var(--ui));
    padding: 0 0.5em 0 0.35em;
    background: #202020;
    border-bottom: 1px solid rgba(255, 255, 255, 0.12);
    cursor: move;
    flex: none;
  }
  .titlebar button.grab {
    margin-right: 0.3em;
    font-size: 1.15em;
    cursor: move;
    opacity: 0.8;
  }
  .name {
    margin-right: 0.8em;
    font-weight: 600;
    letter-spacing: 0.01em;
    opacity: 0.9;
    white-space: nowrap;
  }
  .build {
    margin-left: 0.5em;
    font-weight: 400;
    font-size: 0.85em;
    opacity: 0.6;
    font-variant-numeric: tabular-nums;
  }
  .hint {
    opacity: 0.85;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .hint b {
    color: #fff;
  }
  .modes {
    display: flex;
    padding: 2px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.08);
  }
  .modes button {
    padding: 0.15em 0.75em;
    border-radius: 4px;
  }
  .modes button.on {
    background: var(--mode);
    color: #111;
    font-weight: 600;
  }
  .titlebar button {
    min-width: 1.85em;
    height: 1.7em;
    padding: 0 0.4em;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    line-height: 1;
    cursor: pointer;
  }
  .titlebar button:hover:not(:disabled):not(.on) {
    background: rgba(255, 255, 255, 0.16);
  }
  .titlebar button:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .titlebar button.quit:hover {
    background: rgba(200, 40, 40, 0.85);
    color: #fff;
  }
  .titlebar button.on:not(.modes button) {
    background: rgba(255, 140, 0, 0.35);
  }
  .value {
    min-width: 3.2em;
    text-align: center;
    opacity: 0.85;
    font-variant-numeric: tabular-nums;
  }
  .value.derived {
    color: #ffc800;
  }
  .value.pane {
    min-width: 0;
    padding: 0 0.3em;
    white-space: nowrap;
    opacity: 0.7;
  }
  .sep {
    width: 1px;
    height: 1.2em;
    margin: 0 0.35em;
    background: rgba(255, 255, 255, 0.22);
  }
  .spacer {
    flex: 1;
  }

  .picture {
    position: relative;
    flex: 1;
    min-height: 0;
    background: rgba(0, 0, 0, 0.35);
    cursor: move;
  }
  .glass.lens .picture {
    cursor: default;
  }
  canvas {
    display: block;
    width: 100%;
    height: 100%;
  }
  /* Darken the picture while the pointer is over the parked glass: the source holds
     still meanwhile, and the dim says so. */
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

  .overlay {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    padding: 12px;
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

  .grip {
    position: absolute;
  }
  /* The top edge and corners are thin strips on the border itself, so they never sit
     on a title-bar button; the bottom corners can afford to be generous. */
  .grip.n { top: 0; left: 10px; right: 10px; height: 4px; cursor: ns-resize; }
  .grip.s { bottom: 0; left: 10px; right: 10px; height: 6px; cursor: ns-resize; }
  .grip.w { left: 0; top: 10px; bottom: 10px; width: 4px; cursor: ew-resize; }
  .grip.e { right: 0; top: 10px; bottom: 10px; width: 4px; cursor: ew-resize; }
  .grip.nw { top: 0; left: 0; width: 10px; height: 4px; cursor: nwse-resize; }
  .grip.ne { top: 0; right: 0; width: 10px; height: 4px; cursor: nesw-resize; }
  .grip.se { bottom: 0; right: 0; width: 14px; height: 14px; cursor: nwse-resize; }
  .grip.sw { bottom: 0; left: 0; width: 14px; height: 14px; cursor: nesw-resize; }
</style>
