// The chosen session: one, or all. The owner's wish of 20.9.2026: pick a session on
// the Sessions tab and see only its edits on the Diff tab. Module state, so the
// choice reaches both tabs and survives a tab switch; not persisted, since sessions
// come and go. Nothing clears it but the user: a chosen session that has left the
// list is still named, with "show all" one click away.
//
// Follow can make the choice too (adr.rg.022): while the glass follows, the Code pane
// under the cursor names its session and that session is chosen. `by` says who chose,
// so the tabs can say so; a choice by hand stands until Follow's pane changes.
export const selection = $state<{
  session: string | null;
  name: string | null;
  by: "hand" | "follow";
}>({
  session: null,
  name: null,
  by: "hand",
});

/** Choose one session by id (with its label), or `null` for all. */
export function choose(session: string | null, name: string | null = null, by: "hand" | "follow" = "hand") {
  selection.session = session;
  selection.name = session === null ? null : name;
  selection.by = by;
}

/** What Follow last saw on a Code pane's header (mirrors follow_session.rs). */
export type FollowSaw = { title: string; session_id: string | null; name: string | null };

/** The switch and the latest sighting (mirrors follow_session.rs's Status). */
export type FollowStatus = {
  on: boolean;
  active: boolean;
  saw: FollowSaw | null;
  reads: number;
  last_read_ms: number | null;
  unavailable: string | null;
};

export const follow = $state<FollowStatus>({
  on: true,
  active: false,
  saw: null,
  reads: 0,
  last_read_ms: null,
  unavailable: null,
});

/** A sighting from the core: remembered, and made the choice when it names a session. */
export function followSaw(saw: FollowSaw) {
  follow.saw = saw;
  if (saw.session_id) choose(saw.session_id, saw.name ?? saw.title, "follow");
}
