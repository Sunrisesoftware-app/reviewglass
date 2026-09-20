// The chosen session: one, or all. The owner's wish of 20.9.2026: pick a session on
// the Sessions tab and see only its edits on the Diff tab. Module state, so the
// choice reaches both tabs and survives a tab switch; not persisted, since sessions
// come and go. Nothing clears it but the user: a chosen session that has left the
// list is still named, with "show all" one click away.
export const selection = $state<{ session: string | null; name: string | null }>({
  session: null,
  name: null,
});

/** Choose one session by id (with its label), or `null` for all. */
export function choose(session: string | null, name: string | null = null) {
  selection.session = session;
  selection.name = session === null ? null : name;
}
