//! Pane detection (adr.rg.017): the column under the cursor, read from pixels.
//!
//! Structure, never content. A band of rows around the cursor is scanned column by
//! column; each column is either *uniform* (every sampled pixel the same colour) or
//! *textured* (text, icons, anything drawn). A run of uniform columns is a boundary
//! when it is wide enough to be a gutter between panes, or when it contains a colour
//! change — a border line drawn inside padding. The pane is the textured span between
//! the nearest boundary on each side of the cursor. No character is recognised and the
//! band is discarded after the scan.
//!
//! Where it is wrong — a band in which every line is deeply indented, a wide empty
//! stripe inside a pane, a caret — the finder window shows it, and the user can turn
//! the lock off. It never invents a width: no boundary pair around the cursor means no
//! pane.

/// Per-channel tolerance for "the same colour". Captured UI pixels are exact, but a
/// small tolerance survives dithering and subpixel-rendered edges of a border.
const SAME: i32 = 6;
/// A uniform run at least this wide is a gutter between panes. Narrower blank runs are
/// word gaps, the space between line numbers and code, or an indentation step.
const MIN_GUTTER: usize = 28;
/// A uniform run at least this wide that changes colour inside itself carries a border
/// line. Two pixels of padding on each side of a 1 px line is the least any toolkit
/// draws; a lone 1–2 px uniform stripe (a caret) does not qualify.
const MIN_LINED_RUN: usize = 4;
/// Narrower than this is not a pane worth locking to: a scrollbar, an icon rail.
const MIN_PANE: usize = 120;

/// A pane as an x-range in the coordinates of the scanned band: `[x0, x1)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pane {
    pub x0: i32,
    pub x1: i32,
}

impl Pane {
    pub fn width(&self) -> u32 {
        (self.x1 - self.x0).max(0) as u32
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Rgb(u8, u8, u8);

fn same(a: Rgb, b: Rgb) -> bool {
    (a.0 as i32 - b.0 as i32).abs() <= SAME
        && (a.1 as i32 - b.1 as i32).abs() <= SAME
        && (a.2 as i32 - b.2 as i32).abs() <= SAME
}

/// One column of the band: uniform with its colour, or textured.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Column {
    Uniform(Rgb),
    Textured,
}

/// Classify every column of a tightly packed RGBA band of `width × height` pixels,
/// sampling every `row_step`-th row.
fn classify(rgba: &[u8], width: usize, height: usize, row_step: usize) -> Vec<Column> {
    let px = |x: usize, y: usize| {
        let i = (y * width + x) * 4;
        Rgb(rgba[i], rgba[i + 1], rgba[i + 2])
    };
    (0..width)
        .map(|x| {
            let first = px(x, 0);
            let uniform = (0..height)
                .step_by(row_step.max(1))
                .all(|y| same(px(x, y), first));
            if uniform {
                Column::Uniform(first)
            } else {
                Column::Textured
            }
        })
        .collect()
}

/// Is the uniform run `cols[a..b]` a pane boundary?
fn is_boundary(cols: &[Column], a: usize, b: usize) -> bool {
    let w = b - a;
    if w >= MIN_GUTTER {
        return true;
    }
    if w < MIN_LINED_RUN {
        return false;
    }
    // A colour change inside the run: a border line drawn inside its padding.
    cols[a..b].windows(2).any(|p| match (p[0], p[1]) {
        (Column::Uniform(x), Column::Uniform(y)) => !same(x, y),
        _ => false,
    })
}

/// Find the pane around column `cx` from classified columns. Band edges count as
/// boundaries: a pane against the monitor's edge is still a pane.
fn find(cols: &[Column], cx: usize) -> Option<Pane> {
    let n = cols.len();
    if n == 0 || cx >= n {
        return None;
    }
    let is_uniform = |x: usize| matches!(cols[x], Column::Uniform(_));
    // The maximal uniform run containing x, as [a, b).
    let run_around = |x: usize| {
        let mut a = x;
        while a > 0 && is_uniform(a - 1) {
            a -= 1;
        }
        let mut b = x + 1;
        while b < n && is_uniform(b) {
            b += 1;
        }
        (a, b)
    };
    if is_uniform(cx) {
        let (a, b) = run_around(cx);
        if is_boundary(cols, a, b) {
            return None; // the cursor is in a gutter, not in a pane
        }
    }
    // Walk left to the first boundary run.
    let mut x0 = 0;
    let mut x = cx;
    while x > 0 {
        x -= 1;
        if is_uniform(x) {
            let (a, b) = run_around(x);
            if is_boundary(cols, a, b) {
                x0 = b;
                break;
            }
            x = a;
        }
    }
    // Walk right likewise.
    let mut x1 = n;
    let mut x = cx + 1;
    while x < n {
        if is_uniform(x) {
            let (a, b) = run_around(x);
            if is_boundary(cols, a, b) {
                x1 = a;
                break;
            }
            x = b;
        } else {
            x += 1;
        }
    }
    if x1 - x0 < MIN_PANE {
        return None;
    }
    Some(Pane {
        x0: x0 as i32,
        x1: x1 as i32,
    })
}

/// Detect the pane under column `cx` of a tightly packed RGBA band. `cx` is relative
/// to the band's left edge; the result is in the same coordinates.
pub fn detect(rgba: &[u8], width: usize, height: usize, cx: i32) -> Option<Pane> {
    if width == 0 || height == 0 || rgba.len() < width * height * 4 || cx < 0 {
        return None;
    }
    // Every second row is plenty: a glyph is never one pixel tall, and halving the
    // reads keeps the scan well under a millisecond on a 2560 px wide band.
    let cols = classify(rgba, width, height, 2);
    find(&cols, cx as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BG: Rgb = Rgb(30, 30, 30);
    const LINE: Rgb = Rgb(70, 70, 70);
    const INK: Rgb = Rgb(220, 220, 220);

    /// A synthetic band: `spec` lists (width, kind) from left to right, where kind is
    /// 'b' background, 'l' border line, 't' text (a textured column: ink on some rows).
    fn band(spec: &[(usize, char)], height: usize) -> (Vec<u8>, usize) {
        let width: usize = spec.iter().map(|s| s.0).sum();
        let mut px = vec![0u8; width * height * 4];
        let mut x = 0;
        for &(w, kind) in spec {
            for col in x..x + w {
                for y in 0..height {
                    let c = match kind {
                        'l' => LINE,
                        // Ink on every third row, like glyph rows in a line of text.
                        't' if y % 3 == 1 => INK,
                        _ => BG,
                    };
                    let i = (y * width + col) * 4;
                    px[i..i + 4].copy_from_slice(&[c.0, c.1, c.2, 255]);
                }
            }
            x += w;
        }
        (px, width)
    }

    #[test]
    fn two_panes_split_by_a_gutter() {
        // pane A 400 | gutter 40 | pane B 600
        let (px, w) = band(&[(400, 't'), (40, 'b'), (600, 't')], 60);
        assert_eq!(detect(&px, w, 60, 100), Some(Pane { x0: 0, x1: 400 }));
        assert_eq!(detect(&px, w, 60, 700), Some(Pane { x0: 440, x1: 1040 }));
    }

    #[test]
    fn a_border_line_inside_padding_is_a_boundary() {
        // pane A 400 | 4 bg | 1 line | 4 bg | pane B 300 — a 9 px run, narrower than
        // a gutter, but it carries a line.
        let (px, w) = band(&[(400, 't'), (4, 'b'), (1, 'l'), (4, 'b'), (300, 't')], 60);
        assert_eq!(detect(&px, w, 60, 50), Some(Pane { x0: 0, x1: 400 }));
        assert_eq!(detect(&px, w, 60, 500), Some(Pane { x0: 409, x1: 709 }));
    }

    #[test]
    fn an_indentation_gap_is_not_a_boundary() {
        // line numbers 40 | gap 20 | code 500: one pane, the gap is too narrow.
        let (px, w) = band(&[(40, 't'), (20, 'b'), (500, 't')], 60);
        assert_eq!(detect(&px, w, 60, 300), Some(Pane { x0: 0, x1: 560 }));
    }

    #[test]
    fn a_caret_is_not_a_boundary() {
        // text | 2 px caret | text: a lone thin stripe carries no line and is too
        // narrow to be a gutter.
        let (px, w) = band(&[(300, 't'), (2, 'l'), (300, 't')], 60);
        assert_eq!(detect(&px, w, 60, 100), Some(Pane { x0: 0, x1: 602 }));
    }

    #[test]
    fn cursor_in_a_gutter_finds_no_pane() {
        let (px, w) = band(&[(400, 't'), (40, 'b'), (600, 't')], 60);
        assert_eq!(detect(&px, w, 60, 420), None);
    }

    #[test]
    fn a_narrow_strip_is_not_a_pane() {
        // 60 px of icons between two gutters: too narrow to lock to.
        let (px, w) = band(&[(40, 'b'), (60, 't'), (40, 'b'), (600, 't')], 60);
        assert_eq!(detect(&px, w, 60, 70), None);
    }

    #[test]
    fn an_empty_band_finds_no_pane() {
        let (px, w) = band(&[(800, 'b')], 60);
        assert_eq!(detect(&px, w, 60, 400), None);
        assert_eq!(detect(&[], 0, 0, 0), None);
        assert_eq!(detect(&px, w, 60, -1), None);
        assert_eq!(detect(&px, w, 60, 5000), None);
    }
}
