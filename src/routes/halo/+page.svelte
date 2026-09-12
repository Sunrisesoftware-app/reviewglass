<script lang="ts">
  // The cursor halo: a glowing ring that rides on the mouse pointer while the glass is
  // in Follow mode. Its whole purpose is peripheral: with the eyes on the glass, the
  // ring says where on the screen the pointer actually is. The window is click-through
  // and excluded from capture (see lib.rs), so it never lands in the picture.
</script>

<div class="stage" aria-hidden="true"><div class="halo"></div></div>

<style>
  :global(html, body) {
    margin: 0;
    background: transparent;
    overflow: hidden;
  }
  /* The window is larger than the ring and possibly not square: Windows imposes a
     minimum width on any top-level window. The ring is a fixed circle at the centre,
     which is where the rider puts the cursor. */
  .stage {
    display: grid;
    place-items: center;
    width: 100vw;
    height: 100vh;
  }
  .halo {
    box-sizing: border-box;
    width: 64px;
    height: 64px;
    border-radius: 50%;
    border: 3px solid rgba(255, 140, 0, 0.9);
    box-shadow:
      0 0 10px 3px rgba(255, 140, 0, 0.55),
      inset 0 0 8px 2px rgba(255, 140, 0, 0.35);
    animation: breathe 1.6s ease-in-out infinite;
    pointer-events: none;
  }
  @keyframes breathe {
    0%,
    100% {
      transform: scale(0.92);
      opacity: 0.85;
    }
    50% {
      transform: scale(1);
      opacity: 1;
    }
  }
</style>
