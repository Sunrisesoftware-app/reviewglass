<script lang="ts">
  // The viewfinder (rg.finder-window, adr.rg.017): a frame on the real screen around
  // the rectangle the glass is showing, while the glass is in Follow mode with the pane
  // lock on. It says two things the parked glass cannot: what the pane detector found,
  // and where exactly the glass is looking. The rider thread moves and sizes this
  // window; the page is borders and nothing else. Click-through and excluded from
  // capture (see lib.rs), so it never takes a click and never lands in the picture.
  //
  // With a pane locked by a click (adr.rg.026) the window is the pane: a quiet outline
  // around it is the frame, and the box — the glass's source rectangle — moves inside
  // it (`finder:layout`). While another application's window covers the box the glass
  // holds, and the box says so by going dashed.
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";

  type Layout = { framed: boolean; covered: boolean; x: number; y: number; w: number; h: number };
  let layout = $state<Layout>({ framed: false, covered: false, x: 0, y: 0, w: 0, h: 0 });

  onMount(() => {
    let unlisten: (() => void) | undefined;
    void listen<Layout>("finder:layout", (ev) => (layout = ev.payload)).then((u) => (unlisten = u));
    return () => unlisten?.();
  });

  // The rider speaks in physical pixels; the page lays out in CSS pixels.
  const px = (v: number) => `${v / (window.devicePixelRatio || 1)}px`;
</script>

{#if layout.framed}
  <div class="pane" aria-hidden="true"></div>
  <div
    class="box"
    class:covered={layout.covered}
    style="left: {px(layout.x)}; top: {px(layout.y)}; width: {px(layout.w)}; height: {px(layout.h)}"
    aria-hidden="true"
  ></div>
{:else}
  <div class="frame" aria-hidden="true"></div>
{/if}

<style>
  :global(html, body) {
    margin: 0;
    background: transparent;
    overflow: hidden;
  }
  /* The Follow colour, the same as the glass's own border: the frame on screen and
     the glass are one rectangle. Drawn inside the window so it lies exactly on the
     source rectangle's edge; a faint outer halo lifts it off light and dark alike. */
  .frame,
  .box {
    box-sizing: border-box;
    border: 2px solid rgba(255, 200, 0, 0.85);
    border-radius: 3px;
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.45);
    pointer-events: none;
  }
  .frame {
    width: 100vw;
    height: 100vh;
  }
  /* The locked pane: quieter than the box, so the eye finds the box first. */
  .pane {
    position: fixed;
    inset: 0;
    box-sizing: border-box;
    border: 1px solid rgba(255, 200, 0, 0.6);
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.25);
    pointer-events: none;
  }
  .box {
    position: fixed;
  }
  .box.covered {
    border-style: dashed;
    border-color: rgba(255, 200, 0, 0.5);
  }
</style>
