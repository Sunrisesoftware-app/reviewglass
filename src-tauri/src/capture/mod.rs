//! Capture engine (rg.capture-engine).
//!
//! Captures the monitor under the source rectangle through Windows.Graphics.Capture
//! (via the `windows-capture` crate), crops the rectangle out of each frame and keeps
//! only the newest crop in memory. Pixels never touch the disk and never leave the
//! process. Scaling is left to the glass window's canvas: the engine hands over the
//! source pixels at 1:1 and the frontend draws them at the chosen zoom.
//!
//! Coordinates are physical pixels in the virtual-desktop space, which is what
//! `GetCursorPos` and `GetMonitorInfoW` return for a per-monitor-DPI-aware process
//! (tao declares PerMonitorV2 awareness).

pub mod pane;
pub mod track;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, HMONITOR, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
use windows_capture::capture::{CaptureControl, Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::monitor::Monitor;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};

pub use pane::Pane;

/// Zoom bounds from the spec (section 6.3, capture-engine): above roughly 400 %
/// bitmap scaling visibly degrades, so higher factors are not offered.
pub const ZOOM_MIN: f32 = 1.5;
pub const ZOOM_MAX: f32 = 4.0;

/// Frame delivery cap while the source is changing. Idle cost is governed by
/// Windows.Graphics.Capture itself, which delivers nothing while the screen is static.
const MAX_FPS: u64 = 30;

/// Height of the band scanned for pane boundaries, centred on the cursor. Tall enough
/// that a block of indented code rarely fills it, short enough to stay cheap.
const PANE_BAND: i32 = 400;
/// Pane detection runs at most this often while the cursor moves (adr.rg.017) …
const PANE_SCAN_EVERY: Duration = Duration::from_millis(250);
/// … and this often while it rests: the layout under a resting cursor changes when
/// the user switches applications, which a second's delay does not hurt.
const PANE_SCAN_RESTING: Duration = Duration::from_millis(1000);
/// A pane whose edges moved less than this is the same pane: the scan is not allowed
/// to nudge the picture by a pixel or two between frames.
const PANE_JITTER: i32 = 8;
/// After a menu over the glass closes, publishing waits this long: the compositor
/// may still deliver a frame composed while the menu was up, and a still taken from
/// the menu must not carry the menu (the frame after the grace is forced through).
const HOLD_GRACE: Duration = Duration::from_millis(120);

/// A rectangle in virtual-desktop physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SourceRect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl SourceRect {
    fn centered_on(cx: i32, cy: i32, w: u32, h: u32) -> Self {
        Self {
            x: cx - (w / 2) as i32,
            y: cy - (h / 2) as i32,
            w,
            h,
        }
    }
}

/// The newest cropped frame. `seq` increases only when the pixels changed, so a
/// consumer polling with its last seen `seq` can skip redraws on a static source.
#[derive(Default)]
pub struct FrameData {
    pub seq: u64,
    pub width: u32,
    pub height: u32,
    /// RGBA8, tightly packed, `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
struct MonitorGeom {
    handle: isize,
    /// Top-left of the monitor in virtual-desktop coordinates.
    left: i32,
    top: i32,
}

/// State shared between the capture thread and the command handlers.
struct Shared {
    /// A buffer handed back by the reader, reused by the capture thread so a frame
    /// costs no allocation and no copy beyond the one the crop itself needs.
    spare: Mutex<Vec<u8>>,
    /// The rectangle to crop, in virtual-desktop coordinates.
    source: Mutex<SourceRect>,
    /// The monitor the running capture is attached to.
    monitor: Mutex<Option<MonitorGeom>>,
    latest: Mutex<FrameData>,
    seq: AtomicU64,
    last_hash: AtomicU64,
    /// The rectangle the last published crop came from. A rectangle that moved is new
    /// content even when the compositor reports nothing dirty inside it.
    last_rect: Mutex<Option<SourceRect>>,
    /// Frozen: hold the picture. No frame is published until this clears, so the glass
    /// keeps showing what it showed at the moment of freezing — a still, not a live
    /// view of a locked region. That is what lets a captured instruction survive the
    /// user switching to another application underneath it.
    still: AtomicBool,
    /// Set when a freeze was asked for with no frame in hand (a still restored from
    /// disk, or the glass switched on straight into Still): the next published frame
    /// becomes the still.
    freeze_after_publish: AtomicBool,
    /// A menu is open over the glass: no frame is published and the source rectangle
    /// holds, so what the user right-clicked on is what the menu's pick applies to.
    held: AtomicBool,
    /// After a hold is released, no frame is published before this instant.
    resume_at: Mutex<Option<Instant>>,
    /// `seq` at the last attach: a frame is in hand once `seq` has gone past it.
    attached_seq: AtomicU64,
    /// Follow mode with the pane lock on: the handler scans for the pane under the
    /// cursor. Off in every other state, so the scan costs nothing there.
    want_pane: AtomicBool,
    /// The pane under the cursor in virtual-desktop coordinates, or none found.
    pane: Mutex<Option<Pane>>,
    /// Bumped whenever `pane` changes, so a consumer can notice without comparing.
    pane_seq: AtomicU64,
}

struct Handler {
    shared: Arc<Shared>,
    scratch: Vec<u8>,
    band: Vec<u8>,
    last_pane_scan: Instant,
    last_pane_cursor: (i32, i32),
    /// What the band cannot see: the furthest line over time and a lost column held
    /// (see `track`).
    tracker: track::ColumnTracker,
}

impl Handler {
    /// Scan a band around the cursor for the pane it is in (adr.rg.017). One crop of
    /// full width and `PANE_BAND` height, at most four times a second; the band is
    /// classified and dropped.
    fn scan_pane(&mut self, frame: &mut Frame, mon: MonitorGeom) {
        if !self.shared.want_pane.load(Ordering::Relaxed) {
            // The lock is off, or the glass is hovered or held: the next scan starts
            // from a fresh reading, not from a column remembered across the gap.
            self.tracker.reset();
            return;
        }
        let (cx, cy) = cursor_pos();
        let due = if (cx, cy) == self.last_pane_cursor {
            PANE_SCAN_RESTING
        } else {
            PANE_SCAN_EVERY
        };
        if self.last_pane_scan.elapsed() < due {
            return;
        }
        self.last_pane_scan = Instant::now();
        self.last_pane_cursor = (cx, cy);
        let fw = frame.width() as i32;
        let fh = frame.height() as i32;
        let lx = cx - mon.left;
        let ly = cy - mon.top;
        if lx < 0 || lx >= fw || ly < 0 || ly >= fh {
            return; // the cursor is on another monitor; the capture re-attaches there
        }
        // The band keeps its full height at the screen's edges by sliding onto the
        // screen rather than being cut: near the top the rows below the cursor are
        // the pane, and a clipped band would be mostly toolbar.
        let y0 = (ly - PANE_BAND / 2).max(0);
        let y1 = (y0 + PANE_BAND).min(fh);
        let y0 = (y1 - PANE_BAND).max(0);
        if y1 - y0 < 8 {
            return;
        }
        let found = frame
            .buffer_crop(0, y0 as u32, fw as u32, y1 as u32)
            .ok()
            .and_then(|b| {
                let w = b.width() as usize;
                let h = b.height() as usize;
                let bytes = b.as_nopadding_buffer(&mut self.band);
                pane::detect(bytes, w, h, lx, ly - y0)
            })
            .map(|p| Pane {
                x0: p.x0 + mon.left,
                x1: p.x1 + mon.left,
            });
        // The band's reading, then what the lock holds: the furthest line in memory
        // for the right edge, and a lost column held for a moment.
        let held = self.tracker.observe(found, Instant::now());
        let mut current = self.shared.pane.lock();
        let changed = match (*current, held) {
            (Some(a), Some(b)) => {
                (a.x0 - b.x0).abs() > PANE_JITTER || (a.x1 - b.x1).abs() > PANE_JITTER
            }
            (None, None) => false,
            _ => true,
        };
        crate::measure::log(|| {
            let fmt = |p: Option<Pane>| match p {
                Some(p) => format!("{}..{}/{}", p.x0, p.x1, p.width()),
                None => "none".into(),
            };
            format!(
                "scan cur={cx},{cy} pane={} lock={} changed={}",
                fmt(found),
                fmt(held),
                changed as u8
            )
        });
        if changed {
            *current = held;
            self.shared.pane_seq.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl GraphicsCaptureApiHandler for Handler {
    type Flags = Arc<Shared>;
    type Error = CaptureError;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self {
            shared: ctx.flags,
            scratch: Vec::new(),
            band: Vec::new(),
            last_pane_scan: Instant::now() - PANE_SCAN_RESTING,
            last_pane_cursor: (i32::MIN, i32::MIN),
            tracker: track::ColumnTracker::new(),
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        _control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        if self.shared.still.load(Ordering::Relaxed) || self.shared.held.load(Ordering::Relaxed) {
            return Ok(());
        }
        {
            let mut resume_at = self.shared.resume_at.lock();
            if let Some(t) = *resume_at {
                if Instant::now() < t {
                    return Ok(());
                }
                *resume_at = None;
            }
        }
        let Some(mon) = *self.shared.monitor.lock() else {
            return Ok(());
        };
        self.scan_pane(frame, mon);
        let src = *self.shared.source.lock();

        // Monitor-local crop box, clamped to the frame so a rectangle hanging off the
        // edge still yields whatever part of it is on screen.
        let fw = frame.width() as i32;
        let fh = frame.height() as i32;
        let x0 = (src.x - mon.left).clamp(0, fw);
        let y0 = (src.y - mon.top).clamp(0, fh);
        let x1 = (src.x - mon.left + src.w as i32).clamp(0, fw);
        let y1 = (src.y - mon.top + src.h as i32).clamp(0, fh);
        if x1 - x0 < 2 || y1 - y0 < 2 {
            return Ok(());
        }

        // Skip the crop entirely when the compositor tells us nothing inside our box
        // changed. Cropping allocates a staging texture and copies through it, which is
        // by far the most expensive thing this handler does, and a magnifier parked over
        // a still region would otherwise pay it thirty times a second for no new pixels.
        // An empty list means "the whole frame is dirty" as far as we are concerned: the
        // API reports no regions when it cannot track them, and skipping on that would
        // freeze the glass.
        // Only while the rectangle itself has not moved: a rectangle that moved shows
        // different pixels regardless of what changed on screen, and skipping there is
        // what made the picture lag behind the cursor.
        let moved = *self.shared.last_rect.lock() != Some(src);
        if !moved {
            if let Ok(regions) = frame.dirty_regions() {
                if !regions.is_empty()
                    && !regions
                        .iter()
                        .any(|r| r.x < x1 && r.x + r.width > x0 && r.y < y1 && r.y + r.height > y0)
                {
                    return Ok(());
                }
            }
        }

        let buffer = frame
            .buffer_crop(x0 as u32, y0 as u32, x1 as u32, y1 as u32)
            .map_err(|_| CaptureError::Frame)?;
        let w = buffer.width();
        let h = buffer.height();
        let bytes = buffer.as_nopadding_buffer(&mut self.scratch);

        // Skip the publish when nothing changed. A sampled FNV over the crop is far
        // cheaper than a full compare and good enough to keep a static source idle.
        let hash = sampled_hash(bytes);
        if !moved && hash == self.shared.last_hash.load(Ordering::Relaxed) {
            return Ok(());
        }
        self.shared.last_hash.store(hash, Ordering::Relaxed);
        *self.shared.last_rect.lock() = Some(src);

        let seq = self.shared.seq.fetch_add(1, Ordering::Relaxed) + 1;
        if self
            .shared
            .freeze_after_publish
            .swap(false, Ordering::Relaxed)
        {
            self.shared.still.store(true, Ordering::Relaxed);
        }
        let mut latest = self.shared.latest.lock();
        latest.seq = seq;
        latest.width = w;
        latest.height = h;
        latest.rgba.clear();
        latest.rgba.extend_from_slice(bytes);
        Ok(())
    }
}

fn sampled_hash(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let step = (bytes.len() / 4096).max(1);
    for b in bytes.iter().step_by(step) {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h ^ bytes.len() as u64
}

#[derive(Debug)]
pub enum CaptureError {
    /// Windows.Graphics.Capture could not be started on the target monitor.
    Start(String),
    /// A frame could not be read.
    Frame,
    /// No monitor contains the requested point.
    NoMonitor,
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Start(e) => write!(f, "capture could not start: {e}"),
            Self::Frame => f.write_str("frame could not be read"),
            Self::NoMonitor => f.write_str("no monitor at that position"),
        }
    }
}

/// How the source rectangle is chosen each tick.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum Mode {
    /// The window stays put; the source rectangle follows the cursor.
    Follow,
    /// The window stays put; the rectangle is locked and wheel scrolling moves it.
    Frozen,
    /// The window itself rides on the cursor, showing what is under it — the classic
    /// magnifier lens. Clicks pass through to whatever is beneath.
    Lens,
}

/// The engine: owns the running capture and the shared state.
pub struct Engine {
    shared: Arc<Shared>,
    /// False while the glass is hidden. A hidden magnifier has nothing to show, so it
    /// holds no capture session and reads no pixels.
    enabled: Mutex<bool>,
    control: Mutex<Option<CaptureControl<Handler, CaptureError>>>,
    view: Mutex<View>,
    /// When the source rectangle last went to the measurement log (at most 10/s).
    src_logged: Mutex<Instant>,
}

/// What the glass wants to show: its own size in physical pixels and the zoom.
#[derive(Clone, Copy, Debug)]
struct View {
    width_px: u32,
    height_px: u32,
    zoom: f32,
    mode: Mode,
    /// Origin of the frozen rectangle (only meaningful in `Mode::Frozen`).
    frozen_origin: (i32, i32),
    /// The pointer is over the glass. While following, the source holds still so the
    /// picture does not jump to "what is under the glass" the moment the user reaches
    /// for a control.
    hovered: bool,
    /// Follow tracks the cursor vertically only and holds the detected pane
    /// horizontally (adr.rg.017).
    pane_lock: bool,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Self {
            enabled: Mutex::new(true),
            shared: Arc::new(Shared {
                spare: Mutex::new(Vec::new()),
                last_rect: Mutex::new(None),
                still: AtomicBool::new(false),
                freeze_after_publish: AtomicBool::new(false),
                held: AtomicBool::new(false),
                resume_at: Mutex::new(None),
                attached_seq: AtomicU64::new(0),
                want_pane: AtomicBool::new(false),
                pane: Mutex::new(None),
                pane_seq: AtomicU64::new(0),
                source: Mutex::new(SourceRect {
                    x: 0,
                    y: 0,
                    w: 2,
                    h: 2,
                }),
                monitor: Mutex::new(None),
                latest: Mutex::new(FrameData::default()),
                seq: AtomicU64::new(0),
                last_hash: AtomicU64::new(0),
            }),
            control: Mutex::new(None),
            src_logged: Mutex::new(Instant::now()),
            view: Mutex::new(View {
                width_px: 640,
                height_px: 240,
                zoom: 2.0,
                mode: Mode::Follow,
                frozen_origin: (0, 0),
                hovered: false,
                pane_lock: false,
            }),
        }
    }

    /// Update the glass geometry and zoom. Zoom is clamped, never rejected.
    pub fn set_view(&self, width_px: u32, height_px: u32, zoom: f32) -> f32 {
        let zoom = if zoom.is_finite() {
            zoom.clamp(ZOOM_MIN, ZOOM_MAX)
        } else {
            2.0
        };
        let mut v = self.view.lock();
        v.width_px = width_px.max(2);
        v.height_px = height_px.max(2);
        v.zoom = zoom;
        zoom
    }

    pub fn zoom(&self) -> f32 {
        self.view.lock().zoom
    }

    pub fn mode(&self) -> Mode {
        self.view.lock().mode
    }

    /// Freeze on the current source rectangle, or resume following the cursor.
    pub fn set_mode(&self, mode: Mode) {
        let mut v = self.view.lock();
        if mode == Mode::Frozen && v.mode != Mode::Frozen {
            let s = *self.shared.source.lock();
            v.frozen_origin = (s.x, s.y);
        }
        v.mode = mode;
        drop(v);
        crate::measure::log(|| format!("mode {}", mode_name(mode)));
        if mode == Mode::Frozen {
            self.freeze();
        } else {
            // Leaving a still forces the next frame through, whatever the dirty
            // regions say, so the picture goes live again immediately. A freeze that
            // was still waiting for its frame is cancelled with it, or the next frame
            // to arrive would freeze a glass that is following.
            self.shared.still.store(false, Ordering::Relaxed);
            self.shared
                .freeze_after_publish
                .store(false, Ordering::Relaxed);
            *self.shared.last_rect.lock() = None;
        }
    }

    /// Become a still. With a frame of the running capture in hand the picture holds
    /// at once: it is the picture the user is looking at. Without one (the glass
    /// switched on straight into Still from the dock, or a still restored from disk)
    /// holding at once would hold nothing and the glass would stay black, so one frame
    /// is let through and the hold starts after it.
    fn freeze(&self) {
        if self.frame_in_hand() {
            self.shared.still.store(true, Ordering::Relaxed);
            self.shared
                .freeze_after_publish
                .store(false, Ordering::Relaxed);
        } else {
            self.shared.still.store(false, Ordering::Relaxed);
            self.shared
                .freeze_after_publish
                .store(true, Ordering::Relaxed);
        }
    }

    /// The capture is running and has published at least one frame since it attached.
    fn frame_in_hand(&self) -> bool {
        self.shared.monitor.lock().is_some()
            && self.shared.seq.load(Ordering::Relaxed)
                > self.shared.attached_seq.load(Ordering::Relaxed)
    }

    pub fn is_still(&self) -> bool {
        self.shared.still.load(Ordering::Relaxed)
    }

    /// Restore a frozen rectangle from persisted state.
    pub fn freeze_at(&self, x: i32, y: i32) {
        let mut v = self.view.lock();
        v.mode = Mode::Frozen;
        v.frozen_origin = (x, y);
        drop(v);
        self.freeze();
    }

    /// A menu opened over the glass (or the dock). Until it closes no frame is
    /// published, the source rectangle holds and the lens does not ride: what the
    /// user right-clicked on is what the menu's pick applies to, and the menu stays
    /// where it opened instead of having to be chased.
    pub fn hold(&self) {
        self.shared.held.store(true, Ordering::Relaxed);
        crate::measure::log(|| "hold".into());
    }

    /// The menu closed. Publishing resumes after `HOLD_GRACE`, and the frame after it
    /// is forced through: the picture may have changed under the menu. A freeze picked
    /// from the menu lands before the grace is over and keeps the frame from before
    /// the menu opened.
    pub fn release(&self) {
        *self.shared.resume_at.lock() = Some(Instant::now() + HOLD_GRACE);
        *self.shared.last_rect.lock() = None;
        self.shared.held.store(false, Ordering::Relaxed);
        crate::measure::log(|| "release".into());
    }

    pub fn is_held(&self) -> bool {
        self.shared.held.load(Ordering::Relaxed)
    }

    pub fn set_hovered(&self, hovered: bool) {
        self.view.lock().hovered = hovered;
        crate::measure::log(|| format!("hover {}", hovered as u8));
    }

    pub fn set_pane_lock(&self, lock: bool) {
        self.view.lock().pane_lock = lock;
        crate::measure::log(|| format!("lock {}", lock as u8));
        if !lock {
            // A lock switched off forgets its pane: the next lock starts from a scan.
            let mut p = self.shared.pane.lock();
            if p.take().is_some() {
                self.shared.pane_seq.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// The pane under the cursor, virtual-desktop coordinates; none when the lock is
    /// off or nothing was found.
    pub fn pane(&self) -> Option<Pane> {
        *self.shared.pane.lock()
    }

    /// Changes whenever `pane()` would answer differently.
    pub fn pane_seq(&self) -> u64 {
        self.shared.pane_seq.load(Ordering::Relaxed)
    }

    /// Scroll the frozen rectangle by a delta in source pixels. No-op while following.
    pub fn scroll(&self, dx: i32, dy: i32) {
        let mut v = self.view.lock();
        if v.mode == Mode::Frozen {
            v.frozen_origin.0 += dx;
            v.frozen_origin.1 += dy;
        }
    }

    pub fn source(&self) -> SourceRect {
        *self.shared.source.lock()
    }

    /// Turn capture on or off. Turning it off releases the capture session; the next
    /// tick after it is turned back on attaches a fresh one.
    pub fn set_enabled(&self, enabled: bool) {
        let changed = {
            let mut e = self.enabled.lock();
            let changed = *e != enabled;
            *e = enabled;
            changed
        };
        if changed && !enabled {
            self.stop();
        }
    }

    pub fn is_enabled(&self) -> bool {
        *self.enabled.lock()
    }

    /// Recompute the source rectangle and (re)attach the capture to the monitor
    /// under it. Called from the glass's poll loop, so it runs a few times a second.
    /// A no-op while disabled.
    pub fn tick(&self) -> Result<(), CaptureError> {
        if !self.is_enabled() {
            return Ok(());
        }
        let v = *self.view.lock();
        let src_w = ((v.width_px as f32 / v.zoom).round() as u32).max(2);
        let src_h = ((v.height_px as f32 / v.zoom).round() as u32).max(2);

        let held = self.shared.held.load(Ordering::Relaxed);
        let want_pane = v.mode == Mode::Follow && v.pane_lock && !v.hovered && !held;
        self.shared.want_pane.store(want_pane, Ordering::Relaxed);
        let pane = if want_pane { self.pane() } else { None };

        let rect = match v.mode {
            Mode::Follow | Mode::Lens if held || (v.mode == Mode::Follow && v.hovered) => {
                // Hold the last rectangle; only its size may change (zoom).
                let s = *self.shared.source.lock();
                SourceRect {
                    x: s.x + (s.w as i32 - src_w as i32) / 2,
                    y: s.y + (s.h as i32 - src_h as i32) / 2,
                    w: src_w,
                    h: src_h,
                }
            }
            Mode::Follow if pane.is_some() => {
                // Locked to the pane: the cursor sets the row, the pane sets the
                // column. A pane wider than the source shows its left part, where a
                // line of code begins; a narrower one sits centred.
                let p = pane.unwrap_or(Pane { x0: 0, x1: 0 });
                let (_, cy) = cursor_pos();
                pane_rect(p, cy, src_w, src_h)
            }
            Mode::Follow | Mode::Lens => {
                let (cx, cy) = cursor_pos();
                SourceRect::centered_on(cx, cy, src_w, src_h)
            }
            Mode::Frozen => SourceRect {
                x: v.frozen_origin.0,
                y: v.frozen_origin.1,
                w: src_w,
                h: src_h,
            },
        };
        let prev = std::mem::replace(&mut *self.shared.source.lock(), rect);
        if prev != rect && crate::measure::is_on() {
            let mut at = self.src_logged.lock();
            if at.elapsed() >= Duration::from_millis(100) {
                *at = Instant::now();
                let pane_s = pane
                    .map(|p| format!("{}..{}", p.x0, p.x1))
                    .unwrap_or_else(|| "none".into());
                crate::measure::log(|| {
                    format!(
                        "src {},{},{},{} mode={} hovered={} held={} pane={pane_s}",
                        rect.x,
                        rect.y,
                        rect.w,
                        rect.h,
                        mode_name(v.mode),
                        v.hovered as u8,
                        held as u8
                    )
                });
            }
        }

        let center = (rect.x + (rect.w / 2) as i32, rect.y + (rect.h / 2) as i32);
        let geom = monitor_at(center).ok_or(CaptureError::NoMonitor)?;
        let running = self.shared.monitor.lock().map(|m| m.handle);
        if running != Some(geom.handle) {
            self.attach(geom)?;
        }
        Ok(())
    }

    fn attach(&self, geom: MonitorGeom) -> Result<(), CaptureError> {
        let mut control = self.control.lock();
        if let Some(old) = control.take() {
            let _ = old.stop();
        }
        *self.shared.monitor.lock() = Some(geom);
        self.shared.last_hash.store(0, Ordering::Relaxed);
        *self.shared.last_rect.lock() = None;
        self.shared
            .attached_seq
            .store(self.shared.seq.load(Ordering::Relaxed), Ordering::Relaxed);

        let monitor = Monitor::from_raw_hmonitor(geom.handle as *mut std::ffi::c_void);
        let settings = Settings::new(
            monitor,
            CursorCaptureSettings::WithoutCursor,
            DrawBorderSettings::WithoutBorder,
            SecondaryWindowSettings::Default,
            MinimumUpdateIntervalSettings::Custom(Duration::from_millis(1000 / MAX_FPS)),
            DirtyRegionSettings::ReportOnly,
            ColorFormat::Rgba8,
            Arc::clone(&self.shared),
        );
        let started = Handler::start_free_threaded(settings)
            .map_err(|e| CaptureError::Start(format!("{e:?}")))?;
        *control = Some(started);
        Ok(())
    }

    /// Take the newest frame if it is newer than `since`; `None` when unchanged.
    ///
    /// The pixel buffer is moved out rather than copied, and the reader's previous
    /// allocation is left behind for the capture thread to refill, so a frame costs no
    /// copy beyond the crop itself. A frame the reader took is gone from the engine: a
    /// second call with the same `since` yields `None`, which the glass treats as
    /// "unchanged" and leaves its canvas alone.
    pub fn frame_since(&self, since: u64) -> Option<(u64, u32, u32, Vec<u8>)> {
        let mut latest = self.shared.latest.lock();
        if latest.seq == 0 || latest.seq == since || latest.rgba.is_empty() {
            return None;
        }
        let mut taken = std::mem::take(&mut *self.shared.spare.lock());
        taken.clear();
        std::mem::swap(&mut latest.rgba, &mut taken);
        Some((latest.seq, latest.width, latest.height, taken))
    }

    /// Hand a drained buffer back for the capture thread to refill. Keeping the larger
    /// of the two allocations is what makes a steady stream of frames allocation-free.
    pub fn recycle(&self, buf: Vec<u8>) {
        let mut spare = self.shared.spare.lock();
        if spare.capacity() < buf.capacity() {
            *spare = buf;
        }
    }

    pub fn stop(&self) {
        if let Some(c) = self.control.lock().take() {
            let _ = c.stop();
        }
        *self.shared.monitor.lock() = None;
    }
}

/// The source rectangle for a pane-locked Follow: row from the cursor, column from
/// the pane. A pane wider than the source shows its left part, where a line begins;
/// a narrower one sits centred in the source.
fn pane_rect(p: Pane, cy: i32, src_w: u32, src_h: u32) -> SourceRect {
    let pw = p.width() as i32;
    let x = if pw >= src_w as i32 {
        p.x0
    } else {
        p.x0 - (src_w as i32 - pw) / 2
    };
    SourceRect {
        x,
        y: cy - (src_h / 2) as i32,
        w: src_w,
        h: src_h,
    }
}

/// The cursor in virtual-desktop physical pixels.
/// The mode as the measurement log names it.
fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Follow => "follow",
        Mode::Lens => "lens",
        Mode::Frozen => "still",
    }
}

pub fn cursor_pos() -> (i32, i32) {
    let mut p = POINT::default();
    // SAFETY: GetCursorPos writes into a valid POINT.
    unsafe {
        let _ = GetCursorPos(&mut p);
    }
    (p.x, p.y)
}

fn monitor_at((x, y): (i32, i32)) -> Option<MonitorGeom> {
    // SAFETY: plain Win32 queries with valid out-pointers; DEFAULTTONEAREST never
    // returns a null handle while at least one monitor exists.
    unsafe {
        let h: HMONITOR = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        if h.is_invalid() {
            return None;
        }
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(h, &mut info).as_bool() {
            return None;
        }
        let r = info.rcMonitor;
        Some(MonitorGeom {
            handle: h.0 as isize,
            left: r.left,
            top: r.top,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_engine_does_not_tick() {
        let e = Engine::new();
        assert!(e.is_enabled());
        e.set_enabled(false);
        assert!(!e.is_enabled());
        // Disabled, tick is a no-op and never reports "no monitor at that position".
        assert!(e.tick().is_ok());
    }

    #[test]
    fn frame_is_taken_once() {
        let e = Engine::new();
        {
            let mut latest = e.shared.latest.lock();
            latest.seq = 4;
            latest.width = 2;
            latest.height = 1;
            latest.rgba = vec![9; 8];
        }
        let (seq, w, h, px) = e.frame_since(0).expect("a newer frame is available");
        assert_eq!((seq, w, h, px.len()), (4, 2, 1, 8));
        // The pixels moved out; asking again for the same seq yields nothing rather
        // than an empty frame the glass would draw as a black rectangle.
        assert!(e.frame_since(0).is_none());
        e.recycle(px);
        assert!(e.shared.spare.lock().capacity() >= 8);
    }

    #[test]
    fn zoom_is_clamped_not_rejected() {
        let e = Engine::new();
        assert_eq!(e.set_view(100, 100, 0.5), ZOOM_MIN);
        assert_eq!(e.set_view(100, 100, 9.0), ZOOM_MAX);
        assert_eq!(e.set_view(100, 100, f32::NAN), 2.0);
        assert_eq!(e.set_view(100, 100, 2.5), 2.5);
    }

    fn pretend_running(e: &Engine, frames_published: u64) {
        *e.shared.monitor.lock() = Some(MonitorGeom {
            handle: 1,
            left: 0,
            top: 0,
        });
        e.shared.attached_seq.store(0, Ordering::Relaxed);
        e.shared.seq.store(frames_published, Ordering::Relaxed);
    }

    #[test]
    fn freeze_without_a_frame_in_hand_waits_for_one() {
        // The glass switched on straight into Still: the capture has not attached.
        let e = Engine::new();
        e.set_mode(Mode::Frozen);
        assert!(!e.is_still(), "a hold with no frame would hold nothing");
        assert!(e.shared.freeze_after_publish.load(Ordering::Relaxed));
        // Attached, nothing published yet: still waiting.
        let e = Engine::new();
        pretend_running(&e, 0);
        e.set_mode(Mode::Frozen);
        assert!(!e.is_still());
        assert!(e.shared.freeze_after_publish.load(Ordering::Relaxed));
    }

    #[test]
    fn freeze_with_a_frame_in_hand_holds_at_once() {
        let e = Engine::new();
        pretend_running(&e, 3);
        e.set_mode(Mode::Frozen);
        assert!(e.is_still());
        assert!(!e.shared.freeze_after_publish.load(Ordering::Relaxed));
    }

    #[test]
    fn leaving_a_still_cancels_a_pending_freeze() {
        let e = Engine::new();
        e.set_mode(Mode::Frozen);
        assert!(e.shared.freeze_after_publish.load(Ordering::Relaxed));
        e.set_mode(Mode::Follow);
        assert!(!e.is_still());
        assert!(!e.shared.freeze_after_publish.load(Ordering::Relaxed));
        assert_eq!(*e.shared.last_rect.lock(), None);
    }

    #[test]
    fn a_hold_pauses_and_a_release_forces_the_next_frame() {
        let e = Engine::new();
        pretend_running(&e, 5);
        *e.shared.last_rect.lock() = Some(e.source());
        e.hold();
        assert!(e.is_held());
        e.release();
        assert!(!e.is_held());
        let resume_at = e.shared.resume_at.lock().expect("a grace period is set");
        assert!(resume_at > Instant::now());
        assert_eq!(
            *e.shared.last_rect.lock(),
            None,
            "the frame after the grace goes through"
        );
        // A freeze picked from the menu still counts the pre-menu frame as in hand.
        e.set_mode(Mode::Frozen);
        assert!(e.is_still());
    }

    #[test]
    fn scroll_only_moves_a_frozen_rect() {
        let e = Engine::new();
        e.scroll(10, 10);
        assert_eq!(e.view.lock().frozen_origin, (0, 0));
        e.freeze_at(5, 5);
        e.scroll(10, -3);
        assert_eq!(e.view.lock().frozen_origin, (15, 2));
    }

    #[test]
    fn sampled_hash_distinguishes_length_and_content() {
        assert_ne!(sampled_hash(&[0; 16]), sampled_hash(&[0; 32]));
        assert_ne!(sampled_hash(&[0; 16]), sampled_hash(&[1; 16]));
        assert_eq!(sampled_hash(&[7; 100]), sampled_hash(&[7; 100]));
    }

    #[test]
    fn pane_rect_left_aligns_a_wide_pane_and_centres_a_narrow_one() {
        let wide = Pane { x0: 100, x1: 1100 };
        let r = pane_rect(wide, 500, 600, 200);
        assert_eq!((r.x, r.y, r.w, r.h), (100, 400, 600, 200));
        let narrow = Pane { x0: 100, x1: 300 };
        let r = pane_rect(narrow, 500, 600, 200);
        assert_eq!((r.x, r.y), (-100, 400));
    }

    #[test]
    fn pane_lock_off_forgets_the_pane() {
        let e = Engine::new();
        *e.shared.pane.lock() = Some(Pane { x0: 0, x1: 500 });
        let seq = e.pane_seq();
        e.set_pane_lock(false);
        assert_eq!(e.pane(), None);
        assert_ne!(e.pane_seq(), seq);
    }

    #[test]
    fn centered_rect_is_centered() {
        let r = SourceRect::centered_on(100, 50, 20, 10);
        assert_eq!((r.x, r.y, r.w, r.h), (90, 45, 20, 10));
    }
}
