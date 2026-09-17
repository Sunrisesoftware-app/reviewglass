//! Column tracking between scans (adr.rg.017, tuned from the first recording of
//! 17.9.2026). The detector reads one 400 px band at a time; this keeps what a band
//! cannot see.
//!
//! - **The right edge is the furthest line seen in this column within
//!   `EDGE_MEMORY`**, not the furthest in the current band. Per band the edge
//!   flickered by up to 38 px as lines scrolled through it, while the left edge never
//!   moved; every flicker crossed the jitter and reached Fit.
//! - **A `none` inside a column holds the column for up to `HOLD_ON_NONE`.** Twelve
//!   of 218 scans read `none` with the cursor inside a column just read, and each let
//!   the lock go and the picture jump sideways to the cursor's x. A found column
//!   replaces the held one at once, so a column switch goes column to column with no
//!   cursor-centred picture in between.
//!
//! A pause in scanning (the pointer over the glass or the dock, a menu) keeps the
//! memory, which ages out by itself; only the lock going off forgets the column.
//!
//! Pure: no frames, no clock of its own. The capture thread feeds it every scan.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use super::pane::Pane;

/// How long a right-edge reading counts: the furthest line over this window is the
/// column's edge. Ten seconds covers a scroll through a paragraph of long lines.
pub const EDGE_MEMORY: Duration = Duration::from_secs(10);
/// How long a lost column is held before the lock lets go. Fit's wait for a much
/// wider column is longer than this on purpose, so a reading that only persisted
/// through a hold never widens the glass.
pub const HOLD_ON_NONE: Duration = Duration::from_secs(2);
/// Left edges this close are the same column (the detector's own jitter).
const SAME_COLUMN: i32 = 8;

#[derive(Debug, Default)]
pub struct ColumnTracker {
    column: Option<Tracked>,
}

#[derive(Debug)]
struct Tracked {
    x0: i32,
    /// Right-edge readings still in memory, oldest first.
    edges: VecDeque<(Instant, i32)>,
    /// When the detector first read `none` while this column was held.
    none_since: Option<Instant>,
}

impl Tracked {
    fn furthest(&self) -> Option<i32> {
        self.edges.iter().map(|&(_, x1)| x1).max()
    }
}

impl ColumnTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one scan's reading; get what the lock should hold.
    pub fn observe(&mut self, found: Option<Pane>, now: Instant) -> Option<Pane> {
        match found {
            Some(p) => {
                let same = self
                    .column
                    .as_ref()
                    .is_some_and(|c| (c.x0 - p.x0).abs() <= SAME_COLUMN);
                if !same {
                    self.column = Some(Tracked {
                        x0: p.x0,
                        edges: VecDeque::new(),
                        none_since: None,
                    });
                }
                let c = self.column.as_mut().expect("just set");
                c.x0 = p.x0;
                c.none_since = None;
                while c
                    .edges
                    .front()
                    .is_some_and(|&(t, _)| now.duration_since(t) > EDGE_MEMORY)
                {
                    c.edges.pop_front();
                }
                c.edges.push_back((now, p.x1));
                Some(Pane {
                    x0: p.x0,
                    x1: c.furthest().unwrap_or(p.x1),
                })
            }
            None => {
                let c = self.column.as_mut()?;
                let since = *c.none_since.get_or_insert(now);
                if now.duration_since(since) >= HOLD_ON_NONE {
                    self.column = None;
                    return None;
                }
                let x0 = c.x0;
                c.furthest().map(|x1| Pane { x0, x1 })
            }
        }
    }

    /// Forget the column: the lock went off, or the scan stopped for another reason.
    pub fn reset(&mut self) {
        self.column = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(t0: Instant, ms: u64) -> Instant {
        t0 + Duration::from_millis(ms)
    }

    fn pane(x0: i32, x1: i32) -> Option<Pane> {
        Some(Pane { x0, x1 })
    }

    #[test]
    fn right_edge_is_the_furthest_line_in_memory() {
        let t0 = Instant::now();
        let mut tr = ColumnTracker::new();
        assert_eq!(tr.observe(pane(1463, 2010), at(t0, 0)), pane(1463, 2010));
        assert_eq!(tr.observe(pane(1463, 2001), at(t0, 250)), pane(1463, 2010));
        assert_eq!(tr.observe(pane(1463, 2036), at(t0, 500)), pane(1463, 2036));
        assert_eq!(tr.observe(pane(1463, 1998), at(t0, 750)), pane(1463, 2036));
        // The furthest line ages out of memory; the edge falls to what is still seen.
        assert_eq!(
            tr.observe(pane(1463, 2001), at(t0, 11_000)),
            pane(1463, 2001)
        );
    }

    #[test]
    fn a_new_left_edge_is_a_new_column_with_no_memory() {
        let t0 = Instant::now();
        let mut tr = ColumnTracker::new();
        tr.observe(pane(1463, 2036), at(t0, 0));
        assert_eq!(tr.observe(pane(2011, 2549), at(t0, 250)), pane(2011, 2549));
        // Within the jitter it is the same column and keeps its memory.
        assert_eq!(tr.observe(pane(2015, 2523), at(t0, 500)), pane(2015, 2549));
    }

    #[test]
    fn none_holds_the_column_and_lets_go_after_two_seconds() {
        let t0 = Instant::now();
        let mut tr = ColumnTracker::new();
        tr.observe(pane(2011, 2549), at(t0, 0));
        assert_eq!(tr.observe(None, at(t0, 300)), pane(2011, 2549));
        assert_eq!(tr.observe(None, at(t0, 1900)), pane(2011, 2549));
        assert_eq!(tr.observe(None, at(t0, 2400)), None);
        // Nothing to hold any more: none stays none until a column is found.
        assert_eq!(tr.observe(None, at(t0, 2700)), None);
        assert_eq!(tr.observe(pane(367, 915), at(t0, 3000)), pane(367, 915));
    }

    #[test]
    fn a_found_column_ends_a_hold_at_once() {
        let t0 = Instant::now();
        let mut tr = ColumnTracker::new();
        tr.observe(pane(1463, 2010), at(t0, 0));
        assert_eq!(tr.observe(None, at(t0, 250)), pane(1463, 2010));
        assert_eq!(tr.observe(pane(2011, 2549), at(t0, 500)), pane(2011, 2549));
        // And a hold that ended by a new column does not carry the old none_since.
        assert_eq!(tr.observe(None, at(t0, 2400)), pane(2011, 2549));
    }

    #[test]
    fn reset_forgets_everything() {
        let t0 = Instant::now();
        let mut tr = ColumnTracker::new();
        tr.observe(pane(1463, 2036), at(t0, 0));
        tr.reset();
        assert_eq!(tr.observe(None, at(t0, 100)), None);
        assert_eq!(tr.observe(pane(1463, 2001), at(t0, 200)), pane(1463, 2001));
    }

    /// The dock's window rect in the recordings (top-left corner, 470x44 at 8,8).
    /// Scans with the cursor over it are skipped, as the engine skips them now.
    fn over_dock(cx: i32, cy: i32) -> bool {
        (8..=478).contains(&cx) && (8..=52).contains(&cy)
    }

    struct Replay {
        scans: usize,
        raw_changes: u32,
        out_changes: u32,
        raw_none: u32,
        out_none: u32,
        /// Distinct right edges the lock held for the column at 1463, ascending.
        edges_1463: Vec<i32>,
    }

    /// A recording replayed: the band's raw readings go in, what the lock would have
    /// held comes out. The numbers are the ones the change was made for.
    fn replay(log: &str) -> Replay {
        let t0 = Instant::now();
        let mut scans: Vec<(u64, Option<Pane>)> = Vec::new();
        for line in log.lines() {
            let mut parts = line.splitn(3, ' ');
            let (Some(t), Some("scan"), Some(rest)) = (parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            let ms = (t.parse::<f64>().unwrap() * 1000.0) as u64;
            let cur = rest
                .split(' ')
                .find_map(|f| f.strip_prefix("cur="))
                .and_then(|c| {
                    let (x, y) = c.split_once(',')?;
                    Some((x.parse::<i32>().ok()?, y.parse::<i32>().ok()?))
                })
                .expect("a scan line carries the cursor");
            if over_dock(cur.0, cur.1) {
                continue;
            }
            let pane = rest
                .split(' ')
                .find_map(|f| f.strip_prefix("pane="))
                .and_then(|p| {
                    let (x0, rest) = p.split_once("..")?;
                    let (x1, _) = rest.split_once('/')?;
                    Some(Pane {
                        x0: x0.parse().ok()?,
                        x1: x1.parse().ok()?,
                    })
                });
            scans.push((ms, pane));
        }

        fn changed(a: Option<Pane>, b: Option<Pane>) -> bool {
            match (a, b) {
                (Some(a), Some(b)) => (a.x0 - b.x0).abs() > 8 || (a.x1 - b.x1).abs() > 8,
                (None, None) => false,
                _ => true,
            }
        }
        let mut tr = ColumnTracker::new();
        let (mut raw_prev, mut out_prev) = (None, None);
        let mut r = Replay {
            scans: scans.len(),
            raw_changes: 0,
            out_changes: 0,
            raw_none: 0,
            out_none: 0,
            edges_1463: Vec::new(),
        };
        for &(ms, raw) in &scans {
            let out = tr.observe(raw, at(t0, ms));
            r.raw_changes += changed(raw_prev, raw) as u32;
            r.out_changes += changed(out_prev, out) as u32;
            r.raw_none += raw.is_none() as u32;
            r.out_none += out.is_none() as u32;
            if let Some(p) = out {
                if p.x0 == 1463 {
                    r.edges_1463.push(p.x1);
                }
            }
            raw_prev = raw;
            out_prev = out;
        }
        r.edges_1463.sort_unstable();
        r.edges_1463.dedup();
        r
    }

    #[test]
    fn replay_of_the_first_recording() {
        let r = replay(include_str!(
            "../../../docs/measurements/follow-2026-09-17-owner-1.log"
        ));
        // 218 scans, 7 of them with the cursor over the dock.
        assert_eq!(r.scans, 211);
        // What the glass was told: 48 changes became 22, the owner's own column
        // switches and a right edge growing to a longer line.
        assert_eq!(r.raw_changes, 48);
        assert_eq!(r.out_changes, 22, "tracked changes");
        // The column at 1463 read four different right edges (1998..2036); the lock
        // held two: the furthest line, and the edge after the outlier aged out.
        assert_eq!(r.edges_1463, vec![2010, 2036]);
        // Twelve nones were read; none of them outlasted the hold, so the lock never
        // let go and the picture never fell back to the cursor's x.
        assert_eq!(r.raw_none, 12);
        assert_eq!(r.out_none, 0, "nones that reached the lock");
    }

    #[test]
    fn replay_of_the_second_recording() {
        // Recorded with the tracker already in place, 50 s of Follow: the band's
        // readings replayed the same way, 146 scans with 11 over the dock.
        let r = replay(include_str!(
            "../../../docs/measurements/follow-2026-09-17-owner-2-follow.log"
        ));
        assert_eq!(r.scans, 135);
        assert_eq!(r.raw_changes, 20);
        assert_eq!(r.out_changes, 11, "tracked changes");
        // The band read 1975 on twelve scans and 2010 on thirty; the lock held 2010.
        assert_eq!(r.edges_1463, vec![2010]);
        assert_eq!(r.raw_none, 3);
        assert!(
            r.out_none <= 2,
            "nones that reached the lock: {}",
            r.out_none
        );
    }
}
