//! The Claude Code integration (adr.rg.038): the one place ReviewGlass writes into the
//! user's Claude Code settings, and only when the user presses Connect in Settings >
//! Claude Code, after being shown what will change.
//!
//! Connect copies the two collectors (adr.rg.010) from beside the app into
//! `~/.reviewglass/bin` (adr.rg.019), keeps the settings file as it was in
//! `settings.json.bak-reviewglass`, and adds ReviewGlass's `statusLine` and
//! `PostToolUse` hook with an atomic write that keeps every other key, in its order. An
//! existing status line of the user's own is kept in `~/.reviewglass/integration.json`:
//! chained, the statusLine collector runs it and prints its line; replaced, it is only
//! remembered. Disconnect removes exactly ReviewGlass's entries and puts the user's own
//! status line back. `reviewglass.exe --remove-integration` (the uninstaller's call) does
//! the same before Tauri starts, and removes the collectors.
//!
//! The planning functions are pure (settings in, settings out) and tested on every shape
//! the contract names; the file work around them is thin.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

/// The collectors, as they are named beside the app and in `~/.reviewglass/bin`.
pub const STATUSLINE_EXE: &str = "reviewglass-statusline.exe";
pub const HOOK_EXE: &str = "reviewglass-hook.exe";
const COLLECTORS: [&str; 2] = [STATUSLINE_EXE, HOOK_EXE];
/// The edits the hook reports (spec 5.3).
const HOOK_MATCHER: &str = "Edit|Write|MultiEdit|NotebookEdit";
/// Seconds Claude Code gives the hook; it exits in milliseconds.
const HOOK_TIMEOUT_S: u64 = 5;
const BACKUP_NAME: &str = "settings.json.bak-reviewglass";

// ---- paths -------------------------------------------------------------------

/// `~/.reviewglass`, under the profile root, never AppData (adr.rg.019).
pub fn home_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".reviewglass"))
}

pub fn bin_dir() -> Option<PathBuf> {
    home_dir().map(|d| d.join("bin"))
}

/// What ReviewGlass remembers of the user's own status line.
pub fn record_path() -> Option<PathBuf> {
    home_dir().map(|d| d.join("integration.json"))
}

/// Claude Code's user settings: `$CLAUDE_CONFIG_DIR/settings.json`, else
/// `~/.claude/settings.json`.
pub fn settings_path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(dir).join("settings.json"));
    }
    dirs::home_dir().map(|h| h.join(".claude").join("settings.json"))
}

/// The folder the running app ships its collectors in: its own.
fn shipped_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(Path::to_path_buf)
}

/// Every file the integration touches. The real ones come from the profile; a test
/// gives a temporary folder's, so the user's own files are never part of a test.
#[derive(Clone, Debug)]
pub struct Paths {
    pub settings: PathBuf,
    pub bin: PathBuf,
    pub record: PathBuf,
    /// Left by the uninstaller's disconnect, so an upgrade that uninstalls first can
    /// put the connection back (`--restore-integration`).
    pub reconnect: PathBuf,
    /// Where the collectors to install are: beside the running app.
    pub shipped: PathBuf,
}

impl Paths {
    pub fn real() -> Result<Paths, String> {
        Ok(Paths {
            settings: settings_path().ok_or("no home folder")?,
            bin: bin_dir().ok_or("no home folder")?,
            record: record_path().ok_or("no home folder")?,
            reconnect: home_dir().ok_or("no home folder")?.join("reconnect.json"),
            shipped: shipped_dir().ok_or("the app's own folder is unknown")?,
        })
    }
}

/// A path as Claude Code is given it: forward slashes (CLAUDE.md, spec 10).
fn command_path(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

// ---- the record of the user's own status line ----------------------------------

/// `~/.reviewglass/integration.json`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Record {
    /// The user's `statusLine` value as it was before Connect, to put back.
    pub previous_status_line: Option<Value>,
    /// Whether the collector runs the previous command and prints its line.
    pub chain: bool,
}

impl Record {
    /// The command the collector chains to, if the user chose to chain one.
    pub fn chained_command(&self) -> Option<String> {
        if !self.chain {
            return None;
        }
        self.previous_status_line
            .as_ref()?
            .get("command")?
            .as_str()
            .map(str::to_owned)
            .filter(|c| !c.trim().is_empty())
    }
}

pub fn read_record() -> Record {
    record_path()
        .map(|p| read_record_at(&p))
        .unwrap_or_default()
}

fn read_record_at(path: &Path) -> Record {
    fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

// ---- reading the settings ----------------------------------------------------------

/// The settings file as found.
#[derive(Clone, Debug, PartialEq)]
pub enum Found {
    Absent,
    /// Present and a JSON object (an empty file counts as `{}`).
    Object(Map<String, Value>),
    /// Present but not a JSON object: ReviewGlass will not write over it.
    Unreadable(String),
}

pub fn parse_settings(bytes: Option<&[u8]>) -> Found {
    let Some(bytes) = bytes else {
        return Found::Absent;
    };
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim_start_matches('\u{feff}');
    if text.trim().is_empty() {
        return Found::Object(Map::new());
    }
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(m)) => Found::Object(m),
        Ok(_) => Found::Unreadable("the file is JSON but not an object".into()),
        Err(e) => Found::Unreadable(format!("the file is not valid JSON ({e})")),
    }
}

fn is_ours(command: &str, exe: &str) -> bool {
    let c = command.to_ascii_lowercase();
    c.contains(&exe.to_ascii_lowercase())
        || c.contains(&exe.trim_end_matches(".exe").to_ascii_lowercase())
}

/// What the status line is.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind", content = "command")]
pub enum StatusLineState {
    None,
    Ours,
    Other(String),
}

fn status_line_state(m: &Map<String, Value>) -> StatusLineState {
    match m.get("statusLine") {
        None | Some(Value::Null) => StatusLineState::None,
        Some(v) => {
            let cmd = v.get("command").and_then(Value::as_str).unwrap_or("");
            if is_ours(cmd, STATUSLINE_EXE) {
                StatusLineState::Ours
            } else if cmd.is_empty() {
                StatusLineState::Other(v.to_string())
            } else {
                StatusLineState::Other(cmd.to_owned())
            }
        }
    }
}

/// Every hook command in `hooks.PostToolUse`, as found.
fn post_tool_use_commands(m: &Map<String, Value>) -> Vec<String> {
    let mut out = Vec::new();
    let groups = m
        .get("hooks")
        .and_then(|h| h.get("PostToolUse"))
        .and_then(Value::as_array);
    for g in groups.into_iter().flatten() {
        for h in g
            .get("hooks")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(c) = h.get("command").and_then(Value::as_str) {
                out.push(c.to_owned());
            }
        }
    }
    out
}

fn has_our_hook(m: &Map<String, Value>) -> bool {
    post_tool_use_commands(m)
        .iter()
        .any(|c| is_ours(c, HOOK_EXE))
}

// ---- planning a connect and a disconnect (pure) --------------------------------------

/// What to do with a status line of the user's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Existing {
    /// The collector runs it and prints its line.
    Chain,
    /// ReviewGlass's line replaces it; it is remembered for Disconnect.
    Replace,
}

/// The settings after Connect, and the record to keep.
#[derive(Debug, PartialEq)]
pub struct Plan {
    pub settings: Map<String, Value>,
    pub record: Record,
    pub changed: bool,
}

/// Add ReviewGlass's entries to `settings`, keeping every other key and its order.
/// `bin` is where the collectors are; `existing` says what to do with a status line of
/// the user's own; `record` is what is already remembered (a second Connect keeps it).
pub fn plan_connect(
    mut settings: Map<String, Value>,
    bin: &Path,
    existing: Existing,
    record: &Record,
) -> Plan {
    let mut record = record.clone();
    let mut changed = false;
    let ours = json!({
        "type": "command",
        "command": command_path(&bin.join(STATUSLINE_EXE)),
    });
    match status_line_state(&settings) {
        StatusLineState::Ours => {}
        StatusLineState::None => {
            settings.insert("statusLine".into(), ours);
            changed = true;
        }
        StatusLineState::Other(_) => {
            record.previous_status_line = settings.get("statusLine").cloned();
            record.chain = existing == Existing::Chain;
            settings.insert("statusLine".into(), ours);
            changed = true;
        }
    }
    if !has_our_hook(&settings) {
        let group = json!({
            "matcher": HOOK_MATCHER,
            "hooks": [{
                "type": "command",
                "command": command_path(&bin.join(HOOK_EXE)),
                "timeout": HOOK_TIMEOUT_S,
            }],
        });
        let hooks = settings
            .entry("hooks")
            .or_insert_with(|| Value::Object(Map::new()));
        if !hooks.is_object() {
            *hooks = Value::Object(Map::new());
        }
        let hooks = hooks.as_object_mut().expect("just made an object");
        let ptu = hooks
            .entry("PostToolUse")
            .or_insert_with(|| Value::Array(Vec::new()));
        if !ptu.is_array() {
            *ptu = Value::Array(Vec::new());
        }
        ptu.as_array_mut().expect("just made an array").push(group);
        changed = true;
    }
    Plan {
        settings,
        record,
        changed,
    }
}

/// Remove exactly ReviewGlass's entries and put the user's own status line back.
/// Returns the settings and whether anything changed.
pub fn plan_disconnect(
    mut settings: Map<String, Value>,
    record: &Record,
) -> (Map<String, Value>, bool) {
    let mut changed = false;
    if status_line_state(&settings) == StatusLineState::Ours {
        match &record.previous_status_line {
            Some(prev) => settings.insert("statusLine".into(), prev.clone()),
            None => settings.remove("statusLine"),
        };
        changed = true;
    }
    let mut empty_hooks = false;
    if let Some(hooks) = settings.get_mut("hooks").and_then(Value::as_object_mut) {
        let mut empty_ptu = false;
        if let Some(groups) = hooks.get_mut("PostToolUse").and_then(Value::as_array_mut) {
            for g in groups.iter_mut() {
                if let Some(list) = g.get_mut("hooks").and_then(Value::as_array_mut) {
                    let before = list.len();
                    list.retain(|h| {
                        !h.get("command")
                            .and_then(Value::as_str)
                            .is_some_and(|c| is_ours(c, HOOK_EXE))
                    });
                    changed |= list.len() != before;
                }
            }
            // A group left with no hooks of its own was ReviewGlass's: it goes.
            groups.retain(|g| {
                g.get("hooks")
                    .and_then(Value::as_array)
                    .is_none_or(|l| !l.is_empty())
            });
            empty_ptu = groups.is_empty();
        }
        if empty_ptu {
            hooks.remove("PostToolUse");
        }
        empty_hooks = hooks.is_empty();
    }
    if empty_hooks {
        settings.remove("hooks");
    }
    (settings, changed)
}

// ---- the state Settings > Claude Code shows ------------------------------------------

#[derive(Clone, Debug, Serialize)]
pub struct CollectorState {
    pub name: &'static str,
    /// In `~/.reviewglass/bin`.
    pub installed: bool,
    /// Beside the running app, so it can be installed or refreshed.
    pub shipped: bool,
    /// Installed and the same as the one beside the app.
    pub current: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct IntegrationState {
    pub settings_path: Option<String>,
    /// `absent`, `ok` or `unreadable`.
    pub settings: &'static str,
    pub unreadable_reason: Option<String>,
    pub status_line: StatusLineState,
    /// The user's own status line, remembered at Connect, and whether it is chained.
    pub remembered_status_line: Option<String>,
    pub chained: bool,
    pub hook: bool,
    /// `disableAllHooks` in the user settings: neither the hook nor the status line runs.
    pub disable_all_hooks: bool,
    /// `allowManagedHooksOnly` in the user settings: the hook does not run.
    pub managed_hooks_only: bool,
    pub collectors: Vec<CollectorState>,
    pub bin_dir: Option<String>,
    pub backup_path: Option<String>,
    pub backup_exists: bool,
    /// The status line and the hook are ReviewGlass's and the collectors are installed.
    pub connected: bool,
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::read(a), fs::read(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

fn collectors_state(bin: &Path, shipped: &Path) -> Vec<CollectorState> {
    COLLECTORS
        .iter()
        .map(|name| {
            let inst = bin.join(name);
            let ship = shipped.join(name);
            let installed = inst.is_file();
            let shipped_ok = ship.is_file();
            CollectorState {
                name,
                installed,
                shipped: shipped_ok,
                current: installed && shipped_ok && same_file(&inst, &ship),
            }
        })
        .collect()
}

fn backup_path_of(settings: &Path) -> PathBuf {
    settings.with_file_name(BACKUP_NAME)
}

pub fn state() -> IntegrationState {
    match Paths::real() {
        Ok(p) => state_at(&p),
        Err(e) => IntegrationState {
            settings_path: None,
            settings: "unreadable",
            unreadable_reason: Some(e),
            status_line: StatusLineState::None,
            remembered_status_line: None,
            chained: false,
            hook: false,
            disable_all_hooks: false,
            managed_hooks_only: false,
            collectors: Vec::new(),
            bin_dir: None,
            backup_path: None,
            backup_exists: false,
            connected: false,
        },
    }
}

pub fn state_at(p: &Paths) -> IntegrationState {
    let found = parse_settings(fs::read(&p.settings).ok().as_deref());
    let record = read_record_at(&p.record);
    let collectors = collectors_state(&p.bin, &p.shipped);
    let (settings, unreadable_reason, map) = match found {
        Found::Absent => ("absent", None, Map::new()),
        Found::Object(m) => ("ok", None, m),
        Found::Unreadable(r) => ("unreadable", Some(r), Map::new()),
    };
    let status_line = status_line_state(&map);
    let hook = has_our_hook(&map);
    let flag = |k: &str| map.get(k).and_then(Value::as_bool).unwrap_or(false);
    let installed = collectors.iter().all(|c| c.installed);
    let backup = backup_path_of(&p.settings);
    IntegrationState {
        settings_path: Some(command_path(&p.settings)),
        settings,
        unreadable_reason,
        connected: status_line == StatusLineState::Ours && hook && installed,
        remembered_status_line: record
            .previous_status_line
            .as_ref()
            .and_then(|v| v.get("command"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        chained: record.chained_command().is_some() && status_line == StatusLineState::Ours,
        status_line,
        hook,
        disable_all_hooks: flag("disableAllHooks"),
        managed_hooks_only: flag("allowManagedHooksOnly"),
        collectors,
        bin_dir: Some(command_path(&p.bin)),
        backup_exists: backup.is_file(),
        backup_path: Some(command_path(&backup)),
    }
}

// ---- the file work -------------------------------------------------------------------

/// Write through a temporary file beside the target and a rename, so a reader never
/// sees half a file.
fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("reviewglass-{}.tmp", std::process::id()));
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

fn pretty(m: &Map<String, Value>) -> Vec<u8> {
    let mut s = serde_json::to_string_pretty(&Value::Object(m.clone())).unwrap_or_default();
    s.push('\n');
    s.into_bytes()
}

/// Put a collector in place: the new file beside, the old one renamed out of the way
/// (a running copy can be renamed, not overwritten), then the new one renamed in.
fn install_collector(from: &Path, to: &Path) -> io::Result<()> {
    if let Some(dir) = to.parent() {
        fs::create_dir_all(dir)?;
    }
    let new = to.with_extension("exe.new");
    let old = to.with_extension("exe.old");
    fs::copy(from, &new)?;
    let _ = fs::remove_file(&old);
    if to.exists() {
        fs::rename(to, &old)?;
    }
    fs::rename(&new, to)?;
    let _ = fs::remove_file(&old);
    Ok(())
}

/// Copy every shipped collector that is missing or differs into `~/.reviewglass/bin`.
/// `only_installed`: refresh the ones already there and add none (the start-up pass).
pub fn refresh_collectors(only_installed: bool) -> Result<Vec<&'static str>, String> {
    refresh_collectors_at(&Paths::real()?, only_installed)
}

fn refresh_collectors_at(p: &Paths, only_installed: bool) -> Result<Vec<&'static str>, String> {
    let mut done = Vec::new();
    for name in COLLECTORS {
        let from = p.shipped.join(name);
        let to = p.bin.join(name);
        if !from.is_file() || (only_installed && !to.is_file()) || same_file(&from, &to) {
            continue;
        }
        install_collector(&from, &to).map_err(|e| format!("{name}: {e}"))?;
        done.push(name);
    }
    Ok(done)
}

/// Connect: collectors in place, the backup, ReviewGlass's entries added.
pub fn connect(existing: Existing) -> Result<IntegrationState, String> {
    connect_at(&Paths::real()?, existing)
}

pub fn connect_at(p: &Paths, existing: Existing) -> Result<IntegrationState, String> {
    let bytes = fs::read(&p.settings).ok();
    let map = match parse_settings(bytes.as_deref()) {
        Found::Absent => Map::new(),
        Found::Object(m) => m,
        Found::Unreadable(r) => {
            return Err(format!(
                "{r}. ReviewGlass will not write over it; fix the file or move it away first."
            ))
        }
    };
    if let Some(missing) = COLLECTORS.iter().find(|n| !p.shipped.join(n).is_file()) {
        return Err(format!(
            "{missing} is not beside the app, so it cannot be installed"
        ));
    }
    refresh_collectors_at(p, false)?;
    let plan = plan_connect(map, &p.bin, existing, &read_record_at(&p.record));
    if plan.changed {
        if let Some(b) = &bytes {
            write_atomic(&backup_path_of(&p.settings), b).map_err(|e| format!("backup: {e}"))?;
        }
        let rec = serde_json::to_vec_pretty(&plan.record).unwrap_or_default();
        write_atomic(&p.record, &rec).map_err(|e| format!("record: {e}"))?;
        write_atomic(&p.settings, &pretty(&plan.settings)).map_err(|e| format!("settings: {e}"))?;
    }
    Ok(state_at(p))
}

/// Disconnect: ReviewGlass's entries out, the user's status line back. The collectors
/// stay (a running session may still call them); `remove_collectors` takes them too.
pub fn disconnect(remove_collectors: bool) -> Result<IntegrationState, String> {
    disconnect_at(&Paths::real()?, remove_collectors)
}

pub fn disconnect_at(p: &Paths, remove_collectors: bool) -> Result<IntegrationState, String> {
    let bytes = fs::read(&p.settings).ok();
    if let Found::Object(map) = parse_settings(bytes.as_deref()) {
        let record = read_record_at(&p.record);
        let (next, changed) = plan_disconnect(map, &record);
        if changed {
            if let Some(b) = &bytes {
                write_atomic(&backup_path_of(&p.settings), b)
                    .map_err(|e| format!("backup: {e}"))?;
            }
            write_atomic(&p.settings, &pretty(&next)).map_err(|e| format!("settings: {e}"))?;
        }
    }
    let _ = fs::remove_file(&p.record);
    if remove_collectors {
        for name in COLLECTORS {
            for f in [p.bin.join(name), p.bin.join(name).with_extension("exe.old")] {
                let _ = fs::remove_file(f);
            }
        }
        let _ = fs::remove_dir(&p.bin);
    }
    Ok(state_at(p))
}

/// How long the uninstaller's marker stands: an upgrade that uninstalls the old version
/// first installs the new one within minutes; a reinstall a day later is a new start.
const RECONNECT_WITHIN_S: u64 = 3600;

/// What `--remove-integration` leaves when it disconnects a connected ReviewGlass.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Reconnect {
    at: u64,
    record: Record,
}

fn now_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The uninstaller's disconnect. A connected ReviewGlass leaves a marker first, because
/// Tauri's installer offers to uninstall an older version before installing the new
/// one, without telling the uninstaller it is an update; `restore_at` reads it.
fn remove_at(p: &Paths, now: u64) -> Result<IntegrationState, String> {
    if state_at(p).connected {
        let marker = Reconnect {
            at: now,
            record: read_record_at(&p.record),
        };
        let bytes = serde_json::to_vec_pretty(&marker).unwrap_or_default();
        write_atomic(&p.reconnect, &bytes).map_err(|e| format!("marker: {e}"))?;
    }
    disconnect_at(p, true)
}

/// The installer's last step: put back a connection the uninstaller took down within
/// the hour, with the user's own status line chained or replaced as it was. Without a
/// fresh marker it does nothing: a first install connects only on the user's Connect.
fn restore_at(p: &Paths, now: u64) -> Result<bool, String> {
    let Ok(bytes) = fs::read(&p.reconnect) else {
        return Ok(false);
    };
    let _ = fs::remove_file(&p.reconnect);
    let Ok(marker) = serde_json::from_slice::<Reconnect>(&bytes) else {
        return Ok(false);
    };
    if now.saturating_sub(marker.at) > RECONNECT_WITHIN_S {
        return Ok(false);
    }
    let existing = if marker.record.chain {
        Existing::Chain
    } else {
        Existing::Replace
    };
    connect_at(p, existing).map(|s| s.connected)
}

/// `reviewglass.exe --remove-integration`: the uninstaller's call, handled before Tauri
/// starts (adr.rg.038). Always exits 0: an uninstall must not stop on it.
pub fn remove_from_command_line() -> i32 {
    if let Err(e) = Paths::real().and_then(|p| remove_at(&p, now_s())) {
        eprintln!("reviewglass: {e}");
    }
    0
}

/// `reviewglass.exe --restore-integration`: the installer's last step. Always exits 0.
pub fn restore_from_command_line() -> i32 {
    if let Err(e) = Paths::real().and_then(|p| restore_at(&p, now_s())) {
        eprintln!("reviewglass: {e}");
    }
    0
}

// ---- commands --------------------------------------------------------------------------

#[tauri::command]
pub async fn integration_state() -> IntegrationState {
    state()
}

#[tauri::command]
pub async fn integration_connect(existing: Existing) -> Result<IntegrationState, String> {
    connect(existing)
}

#[tauri::command]
pub async fn integration_disconnect() -> Result<IntegrationState, String> {
    disconnect(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    fn bin() -> PathBuf {
        PathBuf::from(r"C:\Users\u\.reviewglass\bin")
    }

    #[test]
    fn an_absent_or_empty_file_is_an_empty_object_and_junk_is_left_alone() {
        assert_eq!(parse_settings(None), Found::Absent);
        assert_eq!(parse_settings(Some(b"")), Found::Object(Map::new()));
        assert_eq!(
            parse_settings(Some(b"\xef\xbb\xbf{}")),
            Found::Object(Map::new())
        );
        assert!(matches!(
            parse_settings(Some(b"{ nope")),
            Found::Unreadable(_)
        ));
        assert!(matches!(parse_settings(Some(b"[1]")), Found::Unreadable(_)));
    }

    #[test]
    fn connect_on_an_empty_file_adds_both_with_forward_slashes() {
        let p = plan_connect(Map::new(), &bin(), Existing::Chain, &Record::default());
        assert!(p.changed);
        assert_eq!(
            p.settings["statusLine"]["command"],
            "C:/Users/u/.reviewglass/bin/reviewglass-statusline.exe"
        );
        let h = &p.settings["hooks"]["PostToolUse"][0];
        assert_eq!(h["matcher"], HOOK_MATCHER);
        assert_eq!(
            h["hooks"][0]["command"],
            "C:/Users/u/.reviewglass/bin/reviewglass-hook.exe"
        );
        assert_eq!(p.record, Record::default());
    }

    #[test]
    fn a_hand_made_configuration_is_already_connected_and_not_doubled() {
        // The owner's file, as configured by hand on 11.9.2026.
        let s = obj(json!({
            "permissions": {"allow": ["x"]},
            "hooks": {"PostToolUse": [{"matcher": "Edit|Write|MultiEdit|NotebookEdit",
                "hooks": [{"type": "command", "command": "C:/Users/Koti/.reviewglass/bin/reviewglass-hook.exe", "timeout": 5}]}]},
            "statusLine": {"type": "command", "command": "C:/Users/Koti/.reviewglass/bin/reviewglass-statusline.exe"},
            "theme": "dark"
        }));
        let p = plan_connect(s.clone(), &bin(), Existing::Chain, &Record::default());
        assert!(!p.changed);
        assert_eq!(p.settings, s);
    }

    #[test]
    fn every_other_key_keeps_its_place() {
        let s = obj(json!({"zeta": 1, "alpha": {"b": 2, "a": 1}, "theme": "dark"}));
        let p = plan_connect(s, &bin(), Existing::Chain, &Record::default());
        let keys: Vec<&str> = p.settings.keys().map(String::as_str).collect();
        assert_eq!(keys, ["zeta", "alpha", "theme", "statusLine", "hooks"]);
        let inner: Vec<&str> = p.settings["alpha"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(inner, ["b", "a"]);
    }

    #[test]
    fn a_status_line_of_the_users_own_is_chained_or_replaced_and_comes_back() {
        let mine = json!({"type": "command", "command": "bash ~/my-line.sh", "padding": 1});
        let s = obj(json!({"statusLine": mine.clone()}));
        for (choice, chain) in [(Existing::Chain, true), (Existing::Replace, false)] {
            let p = plan_connect(s.clone(), &bin(), choice, &Record::default());
            assert_eq!(p.record.previous_status_line, Some(mine.clone()));
            assert_eq!(p.record.chain, chain);
            assert_eq!(p.record.chained_command().is_some(), chain);
            let (back, changed) = plan_disconnect(p.settings, &p.record);
            assert!(changed);
            assert_eq!(back["statusLine"], mine);
            assert!(back.get("hooks").is_none());
        }
    }

    #[test]
    fn hooks_of_the_users_own_stay_through_connect_and_disconnect() {
        let s = obj(json!({"hooks": {
            "PostToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "lint.sh"}]}],
            "Stop": [{"hooks": [{"type": "command", "command": "notify.sh"}]}]
        }}));
        let p = plan_connect(s.clone(), &bin(), Existing::Chain, &Record::default());
        assert_eq!(
            p.settings["hooks"]["PostToolUse"].as_array().unwrap().len(),
            2
        );
        let (back, _) = plan_disconnect(p.settings, &p.record);
        assert_eq!(back, s);
    }

    #[test]
    fn our_hook_inside_a_group_of_the_users_own_is_taken_out_alone() {
        let s = obj(
            json!({"hooks": {"PostToolUse": [{"matcher": "Edit|Write", "hooks": [
                {"type": "command", "command": "fmt.sh"},
                {"type": "command", "command": "C:/x/reviewglass-hook.exe"}
            ]}]}}),
        );
        let (back, changed) = plan_disconnect(s, &Record::default());
        assert!(changed);
        assert_eq!(
            back["hooks"]["PostToolUse"][0]["hooks"],
            json!([{"type": "command", "command": "fmt.sh"}])
        );
    }

    #[test]
    fn disconnect_without_a_record_removes_the_status_line_and_touches_nothing_else() {
        let s = obj(
            json!({"statusLine": {"type": "command", "command": "C:/x/reviewglass-statusline.exe"}, "theme": "dark"}),
        );
        let (back, changed) = plan_disconnect(s, &Record::default());
        assert!(changed);
        assert_eq!(back, obj(json!({"theme": "dark"})));
        let (same, changed) = plan_disconnect(back.clone(), &Record::default());
        assert!(!changed);
        assert_eq!(same, back);
    }

    #[test]
    fn a_hooks_key_of_the_wrong_shape_is_replaced_only_where_ours_goes() {
        let s = obj(json!({"hooks": {"PostToolUse": "oops", "Stop": []}}));
        let p = plan_connect(s, &bin(), Existing::Chain, &Record::default());
        assert_eq!(p.settings["hooks"]["Stop"], json!([]));
        assert_eq!(
            p.settings["hooks"]["PostToolUse"].as_array().unwrap().len(),
            1
        );
    }

    #[test]
    fn a_second_connect_keeps_what_the_first_remembered() {
        let mine = json!({"type": "command", "command": "my-line"});
        let first = plan_connect(
            obj(json!({"statusLine": mine.clone()})),
            &bin(),
            Existing::Chain,
            &Record::default(),
        );
        let again = plan_connect(
            first.settings.clone(),
            &bin(),
            Existing::Replace,
            &first.record,
        );
        assert!(!again.changed);
        assert_eq!(again.record, first.record);
    }

    /// A temporary home: settings, bin, record and the "shipped" collectors (two small
    /// files standing in for the executables).
    fn temp_paths(tag: &str) -> Paths {
        let root =
            std::env::temp_dir().join(format!("rg-integration-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let shipped = root.join("app");
        fs::create_dir_all(&shipped).unwrap();
        fs::write(shipped.join(STATUSLINE_EXE), b"statusline v2").unwrap();
        fs::write(shipped.join(HOOK_EXE), b"hook v2").unwrap();
        Paths {
            settings: root.join("claude").join("settings.json"),
            bin: root.join("reviewglass").join("bin"),
            record: root.join("reviewglass").join("integration.json"),
            reconnect: root.join("reviewglass").join("reconnect.json"),
            shipped,
        }
    }

    #[test]
    fn connect_and_disconnect_on_disk_keep_the_users_file_and_take_back_only_ours() {
        let p = temp_paths("roundtrip");
        fs::create_dir_all(p.settings.parent().unwrap()).unwrap();
        let original = "{\n  \"theme\": \"dark\",\n  \"statusLine\": {\"type\": \"command\", \"command\": \"my-line\"}\n}\n";
        fs::write(&p.settings, original).unwrap();

        let s = connect_at(&p, Existing::Chain).unwrap();
        assert!(s.connected);
        assert!(s.chained);
        assert_eq!(
            fs::read_to_string(backup_path_of(&p.settings)).unwrap(),
            original
        );
        assert_eq!(fs::read(p.bin.join(HOOK_EXE)).unwrap(), b"hook v2");
        assert_eq!(
            read_record_at(&p.record).chained_command().as_deref(),
            Some("my-line")
        );
        let written: Value = serde_json::from_slice(&fs::read(&p.settings).unwrap()).unwrap();
        assert_eq!(written["theme"], "dark");
        assert!(written["statusLine"]["command"]
            .as_str()
            .unwrap()
            .ends_with("/reviewglass-statusline.exe"));

        // A second Connect changes nothing.
        let before = fs::read(&p.settings).unwrap();
        connect_at(&p, Existing::Replace).unwrap();
        assert_eq!(fs::read(&p.settings).unwrap(), before);

        // An upgrade: a newer collector beside the app replaces the installed one.
        fs::write(p.shipped.join(HOOK_EXE), b"hook v3").unwrap();
        assert_eq!(refresh_collectors_at(&p, true).unwrap(), vec![HOOK_EXE]);
        assert_eq!(fs::read(p.bin.join(HOOK_EXE)).unwrap(), b"hook v3");

        // The uninstaller's disconnect.
        let s = disconnect_at(&p, true).unwrap();
        assert!(!s.connected);
        let back: Value = serde_json::from_slice(&fs::read(&p.settings).unwrap()).unwrap();
        assert_eq!(back, serde_json::from_str::<Value>(original).unwrap());
        assert!(!p.bin.exists());
        assert!(!p.record.exists());
        let _ = fs::remove_dir_all(p.settings.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn connect_refuses_a_file_it_cannot_read_and_writes_nothing() {
        let p = temp_paths("junk");
        fs::create_dir_all(p.settings.parent().unwrap()).unwrap();
        fs::write(&p.settings, "{ not json").unwrap();
        assert!(connect_at(&p, Existing::Chain).is_err());
        assert_eq!(fs::read_to_string(&p.settings).unwrap(), "{ not json");
        assert!(!p.bin.exists());
        assert!(!backup_path_of(&p.settings).exists());
        let _ = fs::remove_dir_all(p.settings.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn connect_with_no_settings_file_makes_one() {
        let p = temp_paths("absent");
        let s = connect_at(&p, Existing::Chain).unwrap();
        assert!(s.connected);
        assert!(!backup_path_of(&p.settings).exists());
        let _ = fs::remove_dir_all(p.settings.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn an_upgrade_that_uninstalls_first_gets_its_connection_back_within_the_hour() {
        let p = temp_paths("upgrade");
        fs::create_dir_all(p.settings.parent().unwrap()).unwrap();
        let original = "{\"statusLine\": {\"type\": \"command\", \"command\": \"my-line\"}}";
        fs::write(&p.settings, original).unwrap();
        connect_at(&p, Existing::Replace).unwrap();

        // The old uninstaller, then the new installer ten minutes later.
        assert!(!remove_at(&p, 1_000).unwrap().connected);
        assert!(p.reconnect.exists());
        assert!(restore_at(&p, 1_600).unwrap());
        let s = state_at(&p);
        assert!(s.connected);
        assert!(!s.chained, "the choice to replace comes back as it was");
        assert!(!p.reconnect.exists());

        // A real uninstall, and an install two hours later: nothing comes back.
        remove_at(&p, 10_000).unwrap();
        assert!(!restore_at(&p, 10_000 + 7_200).unwrap());
        assert!(!state_at(&p).connected);
        assert!(!p.reconnect.exists());

        // A first install with no marker does nothing.
        assert!(!restore_at(&p, 20_000).unwrap());
        let _ = fs::remove_dir_all(p.settings.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn the_settings_path_honours_claude_config_dir() {
        // Only the shape is checked: the variable is the user's.
        let p = settings_path().unwrap();
        assert!(p.ends_with("settings.json"));
    }
}
