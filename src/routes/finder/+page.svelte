<script lang="ts">
  // The viewfinder (rg.finder-window, adr.rg.017): a frame on the real screen around
  // the rectangle the glass is showing, while the glass is in Follow mode with the pane
  // lock on. It says two things the parked glass cannot: what the pane detector found,
  // and where exactly the glass is looking. The rider thread moves and sizes this
  // window to the engine's source rectangle; the page is a border and nothing else.
  // Click-through and excluded from capture (see lib.rs), so it never takes a click and
  // never lands in the picture.
</script>

<div class="frame" aria-hidden="true"></div>

<style>
  :global(html, body) {
    margin: 0;
    background: transparent;
    overflow: hidden;
  }
  /* The Follow colour, the same as the glass's own border: the frame on screen and
     the glass are one rectangle. Drawn inside the window so it lies exactly on the
     source rectangle's edge; a faint outer halo lifts it off light and dark alike. */
  .frame {
    box-sizing: border-box;
    width: 100vw;
    height: 100vh;
    border: 2px solid rgba(255, 200, 0, 0.85);
    border-radius: 3px;
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.45);
    pointer-events: none;
  }
</style>
