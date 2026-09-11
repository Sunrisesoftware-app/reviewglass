<script lang="ts">
  // Glass window: frameless, transparent, always on top. Draggable by its body.
  // P1 fills this with the capture stream; until then it is an outlined rectangle.
  import { getCurrentWindow } from "@tauri-apps/api/window";

  function onpointerdown(e: PointerEvent) {
    if (e.button === 0) void getCurrentWindow().startDragging();
  }
</script>

<div class="glass" {onpointerdown} role="presentation">
  <span class="hint">ReviewGlass — capture engine not wired yet (P1)</span>
</div>

<style>
  :global(html, body) { margin: 0; background: transparent; }
  .glass {
    box-sizing: border-box; width: 100vw; height: 100vh;
    border: 2px solid rgba(255, 200, 0, 0.9); border-radius: 4px;
    background: rgba(0, 0, 0, 0.15); cursor: move; user-select: none;
    display: grid; place-items: center;
  }
  .hint { font: 12px system-ui, sans-serif; color: #fff; text-shadow: 0 0 3px #000; }
</style>
