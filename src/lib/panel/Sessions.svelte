<script lang="ts">
  // Sessions tab: one shared quota gauge above, N relative shares below.
  //
  // The two are deliberately not the same thing and are not drawn as though they were
  // (adr.rg.007). The gauge is one account-wide figure that every session reports
  // identically; the shares are a derived ranking in a different unit. There is no
  // single blended number anywhere on this screen, because there is no honest one.
  import type { UsageView, SessionAttribution, QuotaWindow } from "./types";
  import { selection, choose } from "./selection.svelte";

  let { usage, failure }: { usage: UsageView | null; failure: string | null } = $props();

  /** The chosen session is not in the table now (ended, or its record aged out). */
  const chosenUnlisted = $derived(
    selection.session !== null &&
      !(usage?.sessions ?? []).some((s) => s.session_id === selection.session),
  );

  /** The two account windows as rows, skipping whichever is absent. */
  const windows = $derived(
    (
      [
        { name: "5-hour", w: usage?.quota.five_hour ?? null },
        { name: "7-day", w: usage?.quota.seven_day ?? null },
      ] satisfies { name: string; w: QuotaWindow | null }[]
    ).filter((r): r is { name: string; w: QuotaWindow } => r.w !== null),
  );

  function pct(v: number): string {
    return `${v < 10 ? v.toFixed(1) : Math.round(v)}%`;
  }

  function untilReset(w: QuotaWindow): string | null {
    if (w.resets_at === null) return null;
    const ms = w.resets_at * 1000 - Date.now();
    if (ms <= 0) return null;
    const h = Math.floor(ms / 3_600_000);
    const m = Math.round((ms % 3_600_000) / 60_000);
    return h > 0 ? `${h} h ${m} min` : `${m} min`;
  }

  function estimate(w: QuotaWindow): string | null {
    if (w.hours_to_limit === null) return null;
    const h = w.hours_to_limit;
    const text = h >= 1 ? `${h.toFixed(1)} h` : `${Math.round(h * 60)} min`;
    return `~${text} to the limit at the current rate (estimate)`;
  }

  function severity(v: number): "" | "warn" | "bad" {
    return v >= 90 ? "bad" : v >= 75 ? "warn" : "";
  }

  function label(s: SessionAttribution): string {
    if (s.session_name) return s.session_name;
    const dir = s.cwd?.split(/[\\/]/).filter(Boolean).pop();
    return dir ?? s.session_id.slice(0, 8);
  }

  const SURFACE: Record<string, string> = {
    desktop: "Desktop",
    cli: "CLI",
    unknown: "unknown",
  };

  function ago(ms: number): string {
    const s = Math.max(0, Math.round((Date.now() - ms) / 1000));
    if (s < 60) return `${s}s ago`;
    const m = Math.round(s / 60);
    return m < 60 ? `${m}m ago` : `${Math.round(m / 60)}h ago`;
  }
</script>

{#if failure}
  <p class="state bad">The session data could not be read: {failure}</p>
{:else if !usage}
  <p class="state">Reading sessions…</p>
{:else}
  <!-- The shared account gauge. One figure for the whole account, borrowed from
       whichever CLI session reported it, and absent with a reason when none has. -->
  <section class="quota">
    {#if windows.length > 0}
      <div class="windows">
        {#each windows as { name, w } (name)}
          <div class="window">
            <div class="head">
              <span class="name">{name}</span>
              <span class="figure {severity(w.used_percentage)}">{pct(w.used_percentage)}</span>
            </div>
            <div class="bar" role="img" aria-label="{name} window at {pct(w.used_percentage)}">
              <div
                class="fill {severity(w.used_percentage)}"
                style="width: {Math.min(100, w.used_percentage)}%"
              ></div>
            </div>
            <div class="sub">
              {#if w.stale}
                <span class="flag">reset — this figure is not current</span>
              {:else}
                {#if untilReset(w)}<span>resets in {untilReset(w)}</span>{/if}
                {#if estimate(w)}<span class="flag">{estimate(w)}</span>{/if}
              {/if}
            </div>
          </div>
        {/each}
      </div>
      <p class="note">
        Account-wide, identical in every session — not a per-session figure.
        {#if usage.quota.source_session}
          Reported by one CLI session.
        {/if}
      </p>
    {:else if usage.quota.absence === "no-cli-session"}
      <!-- Actionable: statusLine does not run in the Desktop Code tab (adr.rg.003),
           so the account quota only reaches ReviewGlass through a terminal session. -->
      <p class="state">
        <strong>No quota to show: no CLI session is running.</strong>
        The 5-hour and 7-day figures reach ReviewGlass only through a terminal
        <code>claude</code> session. Start one anywhere and the gauge appears here for every
        session, Desktop tabs included.
      </p>
    {:else if usage.quota.absence === "not-reported-yet"}
      <p class="state">
        A CLI session is running but has not reported a quota yet — it appears after the
        session's first API response, and only on a Claude Pro or Max account.
      </p>
    {/if}
  </section>

  {#if usage.sessions.length === 0}
    <p class="state">
      {#if usage.collector_installed}
        No live sessions. One appears here as soon as you send a message in Claude Code.
      {:else}
        <strong>The status-line collector is not installed yet.</strong>
        Desktop sessions still appear here from their transcripts; CLI sessions and the
        quota gauge need the collector, which the installer sets up in phase 7.
      {/if}
    </p>
  {:else}
    <!-- The first column chooses: all sessions (the header's button) or one, and the
         Diff tab shows the edits of the choice. The chosen row is bold. -->
    <table>
      <thead>
        <tr>
          <th class="pick">
            <input
              type="radio"
              name="session-pick"
              value=""
              checked={selection.session === null}
              onchange={() => choose(null)}
              aria-label="All sessions"
              title="All sessions: the Diff tab shows every session's edits"
            />
          </th>
          <th>Session</th>
          <th>Where</th>
          <th>Model</th>
          <th class="num">Context</th>
          <th class="num">Cost</th>
          <th class="num">Share</th>
          <th class="num">Seen</th>
        </tr>
      </thead>
      <tbody>
        {#each usage.sessions as s (s.session_id)}
          <tr class:chosen={selection.session === s.session_id}>
            <td class="pick">
              <input
                type="radio"
                name="session-pick"
                value={s.session_id}
                checked={selection.session === s.session_id}
                onchange={() => choose(s.session_id, label(s))}
                aria-label="Only {label(s)}"
                title="Only this session: the Diff tab shows its edits alone"
              />
            </td>
            <td>
              <span class="label">{label(s)}</span>
              {#if s.cwd && s.session_name}<span class="dim">{s.cwd.split(/[\\/]/).filter(Boolean).pop()}</span>{/if}
            </td>
            <td class:dim={s.surface === "unknown"}>{SURFACE[s.surface]}</td>
            <td>{s.model_name ?? ""}</td>
            <!-- Every cell below is blank when its figure is absent. Never a zero. -->
            <td class="num">{s.context_used_pct !== null ? pct(s.context_used_pct) : ""}</td>
            <td class="num">{s.cost_usd !== null ? `$${s.cost_usd.toFixed(2)}` : ""}</td>
            <td class="num">
              {#if s.share_pct !== null}
                {pct(s.share_pct)}<span class="basis" title="derived from {s.basis}">{s.basis === "cost" ? "$" : "t"}</span>
              {/if}
            </td>
            <td class="num dim">{ago(s.observed_at_ms)}</td>
          </tr>
        {/each}
      </tbody>
    </table>

    <p class="note">
      {#if selection.session === null}
        Pick a session to see only its edits on the Diff tab; the first button shows all.
      {:else if chosenUnlisted}
        <strong>{selection.name}</strong> is chosen for the Diff tab but is not in the table
        now. <button class="link" onclick={() => choose(null)}>Show all sessions</button>
      {:else}
        <strong>{selection.name}</strong> is chosen: the Diff tab shows its edits alone.
      {/if}
    </p>
    <p class="note">
      Share is relative between sessions, not a share of the quota — a different unit.
      {#if usage.mixed_basis}
        <span class="flag">
          Shares here come from two bases ($ = cost, t = tokens), so only sessions marked
          the same way are comparable.
        </span>
      {/if}
      {#if usage.sessions.some((s) => s.share_pct === null)}
        A session with no share has nothing measurable to report yet.
      {/if}
    </p>
  {/if}

  {#if usage.unreadable > 0}
    <p class="state warn">
      {usage.unreadable} session {usage.unreadable === 1 ? "record" : "records"} could not be
      read on this pass and {usage.unreadable === 1 ? "is" : "are"} missing from the table.
    </p>
  {/if}
{/if}

<style>
  .state {
    margin: 0 0 14px;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: var(--raised);
    color: var(--muted);
  }
  .state strong {
    display: block;
    color: var(--fg);
  }
  .state code {
    padding: 1px 4px;
    border-radius: 3px;
    background: var(--line);
  }
  .state.warn {
    color: var(--warn);
  }
  .state.bad {
    color: var(--bad);
  }

  .quota {
    margin-bottom: 18px;
  }
  .windows {
    display: flex;
    gap: 18px;
    flex-wrap: wrap;
  }
  .window {
    flex: 1 1 220px;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .name {
    color: var(--muted);
  }
  .figure {
    font-size: 19px;
    font-variant-numeric: tabular-nums;
  }
  .figure.warn {
    color: var(--warn);
  }
  .figure.bad {
    color: var(--bad);
  }
  .bar {
    height: 6px;
    margin: 5px 0;
    border-radius: 3px;
    background: var(--line);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--accent);
  }
  .fill.warn {
    background: var(--warn);
  }
  .fill.bad {
    background: var(--bad);
  }
  .sub {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
    font-size: 12px;
    color: var(--muted);
  }
  .flag {
    font-style: italic;
  }

  table {
    width: 100%;
    border-collapse: collapse;
  }
  th,
  td {
    padding: 6px 8px;
    text-align: left;
    border-bottom: 1px solid var(--line);
    white-space: nowrap;
  }
  th {
    font-weight: 500;
    color: var(--muted);
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .label {
    font-weight: 500;
  }
  .pick {
    width: 1.6em;
    padding-right: 0;
  }
  .pick input {
    margin: 0;
    accent-color: var(--accent);
    cursor: pointer;
  }
  tr.chosen td {
    background: var(--raised);
  }
  tr.chosen .label {
    font-weight: 700;
    color: var(--accent);
  }
  .note .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent);
    font: inherit;
    cursor: pointer;
    text-decoration: underline;
  }
  .dim {
    color: var(--muted);
  }
  td .dim {
    margin-left: 6px;
    font-size: 12px;
  }
  .basis {
    margin-left: 3px;
    font-size: 10px;
    color: var(--muted);
    vertical-align: super;
  }
  .note {
    margin: 10px 0 0;
    font-size: 12px;
    color: var(--muted);
  }
</style>
