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

// The shapes `panel_diffs` returns. Mirrors src-tauri/src/diff/mod.rs.

export type DiffStatus =
  | "changed"
  | "unchanged"
  | "untracked"
  | "not-in-repo"
  | "denied"
  | "missing"
  | "too-large"
  | "binary"
  | "git-failed";

/** What a diff is against: git HEAD, the previous edit ReviewGlass saw, or nothing yet. */
export type Baseline = "head" | "last-edit" | "whole-file";

export type DiffView = {
  path: string;
  display_path: string;
  repo_root: string | null;
  status: DiffStatus;
  /** Null when there is no diff to show. */
  baseline: Baseline | null;
  /** The unified diff as git printed it; null when the status says why. */
  unified: string | null;
  added: number | null;
  removed: number | null;
  /** The lines the latest edit brought, 1-based in the working copy: ReviewGlass's own mark. */
  fresh: number[];
  /** Measured against the previous edit (true), or the first sighting's every added line (false). */
  fresh_from_previous: boolean;
  at_ms: number;
  session_id: string | null;
  tool: string | null;
  reason: string | null;
};

export type DiffTab = {
  views: DiffView[];
  /** False is a different empty list: the hook collector has never run. */
  hook_installed: boolean;
  unreadable: number;
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

// The shapes `panel_file_view` returns (P4b). Mirrors src-tauri/src/diff/file.rs.

export type FileStatus = "shown" | "not-offered" | "denied" | "missing" | "too-large" | "binary";

/** One hunk of the last diff by its new-side lines: where the file view scrolls to. */
export type Hunk = {
  /** The first line to look at, 1-based. */
  start: number;
  /** New-side lines in the hunk; 0 when it only removed. */
  lines: number;
};

export type FileView = {
  path: string;
  display_path: string;
  repo_root: string | null;
  status: FileStatus;
  /** The working copy as text; null when the status says why. */
  text: string | null;
  /** Lines the last diff added, 1-based, ascending. */
  added: number[];
  /** Line numbers before which the last diff removed lines; one past the last line for a removal at the end. */
  removed_before: number[];
  hunks: Hunk[];
  /** The lines the latest edit brought, 1-based: ReviewGlass's own mark. */
  fresh: number[];
  fresh_from_previous: boolean;
  reason: string | null;
};
