// The shapes `panel_usage` returns. Mirrors src-tauri/src/usage.rs.
//
// Everything optional here is optional in earnest: `null` means the figure does not
// exist, and the panel must hide its element rather than render a zero.

export type Surface = "desktop" | "cli" | "unknown";
export type Origin = "status-line" | "transcript" | "both";
export type AttributionBasis = "cost" | "tokens" | "none";

/** Why the quota gauge has nothing to show. The two have different remedies. */
export type QuotaAbsence = "no-cli-session" | "not-reported-yet";

export type QuotaWindow = {
  used_percentage: number;
  resets_at: number | null;
  /** Percentage points per hour. Null until two samples exist far enough apart. */
  burn_per_hour: number | null;
  /** Hours to 100 % at the current rate. An estimate, always labelled as one. */
  hours_to_limit: number | null;
  /** resets_at has passed with no fresher sample: this figure is not current. */
  stale: boolean;
};

export type AccountQuota = {
  five_hour: QuotaWindow | null;
  seven_day: QuotaWindow | null;
  absence: QuotaAbsence | null;
  /** Which session the account-wide figures were read from — the gauge is borrowed. */
  source_session: string | null;
};

export type SessionAttribution = {
  session_id: string;
  session_name: string | null;
  surface: Surface;
  origin: Origin;
  cwd: string | null;
  model_name: string | null;
  context_used_pct: number | null;
  cost_usd: number | null;
  lines_added: number | null;
  lines_removed: number | null;
  basis: AttributionBasis;
  /** Share of the measurable total, relative only — never a share of the quota. */
  share_pct: number | null;
  observed_at_ms: number;
};

export type UsageView = {
  quota: AccountQuota;
  sessions: SessionAttribution[];
  /** False when the collector has never written: an empty table with a different cause. */
  collector_installed: boolean;
  unreadable: number;
  /** Shares come from more than one basis, so only within-basis ranking is meaningful. */
  mixed_basis: boolean;
};
