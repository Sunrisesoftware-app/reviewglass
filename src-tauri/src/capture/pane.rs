//! Pane detection (adr.rg.017): the column under the cursor, read from pixels.
//!
//! Structure, never content. A band of rows around the cursor is cut into horizontal
//! slices and scanned column by column; in each slice a column is either *uniform*
//! (every sampled pixel the same colour) or *textured* (text, icons, anything drawn).
//! A column is *blank* when it is uniform in the slice the cursor is in and in most
//! of the others — most, not all, because a toolbar, a tab strip, an input box or a
//! hover highlight crosses every gutter on the rows it occupies, and a rule that asked
//! for every row lost the pane whenever the cursor came near the top or bottom of the
//! screen (measured 13.9.2026: the top and bottom fifth). A run of *strong* blank
//! columns — blank in most slices — is a boundary when it is wide enough to be a
//! gutter between panes, or when it contains a colour change, a border line drawn
//! inside padding. Only the strong core counts: the end of a short line leaves the
//! text area blank on the cursor's row too, and judging the whole blank run by its
//! weakest column lost every boundary next to ragged text (measured the same day). The
//! pane is the span between the nearest boundary on each side of the cursor. No
//! character is recognised and the band is discarded after the scan.
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
/// Height of one slice of the band. Two or three text lines: a slice is small enough
/// that a toolbar occupies few of them, large enough that a word gap does not make a
/// column uniform by accident.
const SLICE_H: usize = 25;
/// A column is *strong* when uniform in this share of the slices, and a run of strong
/// columns with a border line in it is a boundary: the line runs the pane's full
/// height, a toolbar crosses it only at the top.
const BLANK_LINED: f32 = 0.6;
/// A plain gutter, with no line to vouch for it, must be uniform in this share: an
/// indentation gap is uniform in the slices where every line is indented, and a
/// stricter share keeps a deeply indented block from passing as a gutter. 0.8 lets a
/// hover highlight or a one-line crossing through (two slices of sixteen) and stops
/// at a block indented for four fifths of the band.
const BLANK_PLAIN: f32 = 0.8;

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

/// One column of the band: blank, with its colour in the cursor's slice and the share
/// of slices it is uniform in; or textured.
#[derive(Clone, Copy, PartialEq)]
enum Column {
    Blank(Rgb, f32),
    Textured,
}

/// Classify every column of a tightly packed RGBA band of `width × height` pixels,
/// sampling every `row_step`-th row of each slice. `cy` is the cursor's row within the
/// band: a column must be uniform in that slice to be blank at all.
fn classify(rgba: &[u8], width: usize, height: usize, row_step: usize, cy: usize) -> Vec<Column> {
    let px = |x: usize, y: usize| {
        let i = (y * width + x) * 4;
        Rgb(rgba[i], rgba[i + 1], rgba[i + 2])
    };
    let slices: Vec<(usize, usize)> = (0..height)
        .step_by(SLICE_H)
        .map(|y0| (y0, (y0 + SLICE_H).min(height)))
        .collect();
    let cursor_slice = (cy.min(height - 1) / SLICE_H).min(slices.len() - 1);
    (0..width)
        .map(|x| {
            let uniform_in = |&(y0, y1): &(usize, usize)| {
                let first = px(x, y0);
                (y0..y1)
                    .step_by(row_step.max(1))
                    .all(|y| same(px(x, y), first))
            };
            if !uniform_in(&slices[cursor_slice]) {
                return Column::Textured;
            }
            let uniform = slices.iter().filter(|s| uniform_in(s)).count();
            Column::Blank(
                px(x, slices[cursor_slice].0),
                uniform as f32 / slices.len() as f32,
            )
        })
        .collect()
}

fn share(c: &Column) -> f32 {
    match c {
        Column::Blank(_, f) => *f,
        Column::Textured => 0.0,
    }
}

fn is_strong(c: &Column) -> bool {
    share(c) >= BLANK_LINED
}

/// Is the strong run `cols[a..b]` a pane boundary?
fn is_boundary(cols: &[Column], a: usize, b: usize) -> bool {
    let w = b - a;
    // A colour change inside the run: a border line drawn inside its padding.
    let lined = cols[a..b].windows(2).any(|p| match (p[0], p[1]) {
        (Column::Blank(x, _), Column::Blank(y, _)) => !same(x, y),
        _ => false,
    });
    if lined && w >= MIN_LINED_RUN {
        return true;
    }
    let weakest = cols[a..b].iter().map(share).fold(1.0f32, f32::min);
    w >= MIN_GUTTER && weakest >= BLANK_PLAIN
}

/// Maximal runs of strong columns, as `[a, b)`.
fn strong_runs(cols: &[Column]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut x = 0;
    while x < cols.len() {
        if is_strong(&cols[x]) {
            let a = x;
            while x < cols.len() && is_strong(&cols[x]) {
                x += 1;
            }
            out.push((a, x));
        } else {
            x += 1;
        }
    }
    out
}

/// Find the pane around column `cx` from classified columns. Band edges count as
/// boundaries: a pane against the monitor's edge is still a pane.
fn find(cols: &[Column], cx: usize) -> Option<Pane> {
    let n = cols.len();
    if n == 0 || cx >= n {
        return None;
    }
    let boundaries: Vec<(usize, usize)> = strong_runs(cols)
        .into_iter()
        .filter(|&(a, b)| is_boundary(cols, a, b))
        .collect();
    if boundaries.iter().any(|&(a, b)| a <= cx && cx < b) {
        return None; // the cursor is in a gutter, not in a pane
    }
    let x0 = boundaries
        .iter()
        .filter(|&&(_, b)| b <= cx)
        .map(|&(_, b)| b)
        .max()
        .unwrap_or(0);
    let x1 = boundaries
        .iter()
        .filter(|&&(a, _)| a > cx)
        .map(|&(a, _)| a)
        .min()
        .unwrap_or(n);
    if x1 - x0 < MIN_PANE {
        return None;
    }
    Some(Pane {
        x0: x0 as i32,
        x1: x1 as i32,
    })
}

/// Detect the pane under column `cx` of a tightly packed RGBA band, with the cursor
/// on row `cy` of the band. Both are relative to the band's top-left; the result is in
/// the same coordinates.
pub fn detect(rgba: &[u8], width: usize, height: usize, cx: i32, cy: i32) -> Option<Pane> {
    if width == 0 || height == 0 || rgba.len() < width * height * 4 || cx < 0 || cy < 0 {
        return None;
    }
    // Every second row is plenty: a glyph is never one pixel tall, and halving the
    // reads keeps the scan well under a millisecond on a 2560 px wide band.
    let cols = classify(rgba, width, height, 2, cy as usize);
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
    /// Rows below `toolbar` are drawn as text across the whole width, like a tab strip
    /// or an input box crossing every gutter.
    fn band_with(spec: &[(usize, char)], height: usize, toolbar: usize) -> (Vec<u8>, usize) {
        let width: usize = spec.iter().map(|s| s.0).sum();
        let mut px = vec![0u8; width * height * 4];
        let mut x = 0;
        for &(w, kind) in spec {
            for col in x..x + w {
                for y in 0..height {
                    let c = match kind {
                        _ if y < toolbar && y % 3 == 1 => INK,
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

    fn band(spec: &[(usize, char)], height: usize) -> (Vec<u8>, usize) {
        band_with(spec, height, 0)
    }

    #[test]
    fn two_panes_split_by_a_gutter() {
        // pane A 400 | gutter 40 | pane B 600
        let (px, w) = band(&[(400, 't'), (40, 'b'), (600, 't')], 60);
        assert_eq!(detect(&px, w, 60, 100, 30), Some(Pane { x0: 0, x1: 400 }));
        assert_eq!(
            detect(&px, w, 60, 700, 30),
            Some(Pane { x0: 440, x1: 1040 })
        );
    }

    #[test]
    fn a_toolbar_across_the_top_does_not_hide_a_lined_boundary() {
        // 400 rows, the top 100 a tab strip across everything; the cursor below it.
        let (px, w) = band_with(
            &[(400, 't'), (4, 'b'), (1, 'l'), (4, 'b'), (300, 't')],
            400,
            100,
        );
        assert_eq!(detect(&px, w, 400, 50, 250), Some(Pane { x0: 0, x1: 400 }));
        assert_eq!(
            detect(&px, w, 400, 500, 250),
            Some(Pane { x0: 409, x1: 709 })
        );
    }

    #[test]
    fn a_thin_crossing_does_not_hide_a_plain_gutter() {
        // A 30 px hover highlight across a 400 px band: 7.5 %, under the plain gutter's
        // allowance.
        let (px, w) = band_with(&[(400, 't'), (40, 'b'), (600, 't')], 400, 30);
        assert_eq!(detect(&px, w, 400, 100, 250), Some(Pane { x0: 0, x1: 400 }));
    }

    #[test]
    fn a_thick_crossing_hides_a_plain_gutter_but_not_a_lined_one() {
        // A toolbar over a quarter of the band: a plain gutter no longer vouches for
        // itself, a border line still does.
        let (plain, w1) = band_with(&[(400, 't'), (40, 'b'), (600, 't')], 400, 100);
        assert_eq!(
            detect(&plain, w1, 400, 100, 250),
            Some(Pane { x0: 0, x1: 1040 })
        );
        let (lined, w2) = band_with(
            &[(400, 't'), (20, 'b'), (1, 'l'), (19, 'b'), (600, 't')],
            400,
            100,
        );
        assert_eq!(
            detect(&lined, w2, 400, 100, 250),
            Some(Pane { x0: 0, x1: 400 })
        );
    }

    #[test]
    fn ragged_text_beside_a_gutter_does_not_hide_it() {
        // The middle pane's lines are short on the cursor's row: its text area is
        // blank there but textured in most slices, so it is weak; the gutter beside
        // it, with its border line, is strong and stays a boundary. Rows 0..400,
        // text in the left pane only on rows outside 190..215 for columns 300..400.
        let (px, w) = band(
            &[(400, 't'), (20, 'b'), (1, 'l'), (19, 'b'), (600, 't')],
            400,
        );
        let mut px = px;
        for y in 190..215 {
            for x in 300..400 {
                let i = (y * w + x) * 4;
                px[i..i + 3].copy_from_slice(&[BG.0, BG.1, BG.2]);
            }
        }
        assert_eq!(
            detect(&px, w, 400, 700, 200),
            Some(Pane { x0: 440, x1: 1040 })
        );
        assert_eq!(detect(&px, w, 400, 100, 200), Some(Pane { x0: 0, x1: 400 }));
    }

    #[test]
    fn the_cursor_slice_must_be_clear() {
        // A gutter clear everywhere except in the cursor's own slice is not a
        // boundary there: whatever crosses it is what the user is pointing at.
        let (px, w) = band(&[(400, 't'), (40, 'b'), (600, 't')], 400);
        // Draw text across the gutter on rows 200..225 (the cursor's slice).
        let mut px = px;
        for y in (200..225).filter(|y| y % 3 == 1) {
            for x in 400..440 {
                let i = (y * w + x) * 4;
                px[i..i + 3].copy_from_slice(&[INK.0, INK.1, INK.2]);
            }
        }
        assert_eq!(
            detect(&px, w, 400, 100, 210),
            Some(Pane { x0: 0, x1: 1040 })
        );
        assert_eq!(detect(&px, w, 400, 100, 300), Some(Pane { x0: 0, x1: 400 }));
    }

    #[test]
    fn a_border_line_inside_padding_is_a_boundary() {
        // pane A 400 | 4 bg | 1 line | 4 bg | pane B 300 — a 9 px run, narrower than
        // a gutter, but it carries a line.
        let (px, w) = band(&[(400, 't'), (4, 'b'), (1, 'l'), (4, 'b'), (300, 't')], 60);
        assert_eq!(detect(&px, w, 60, 50, 30), Some(Pane { x0: 0, x1: 400 }));
        assert_eq!(detect(&px, w, 60, 500, 30), Some(Pane { x0: 409, x1: 709 }));
    }

    #[test]
    fn an_indentation_gap_is_not_a_boundary() {
        // line numbers 40 | gap 20 | code 500: one pane, the gap is too narrow.
        let (px, w) = band(&[(40, 't'), (20, 'b'), (500, 't')], 60);
        assert_eq!(detect(&px, w, 60, 300, 30), Some(Pane { x0: 0, x1: 560 }));
    }

    #[test]
    fn a_caret_is_not_a_boundary() {
        // text | 2 px caret | text: a lone thin stripe carries no line and is too
        // narrow to be a gutter.
        let (px, w) = band(&[(300, 't'), (2, 'l'), (300, 't')], 60);
        assert_eq!(detect(&px, w, 60, 100, 30), Some(Pane { x0: 0, x1: 602 }));
    }

    #[test]
    fn cursor_in_a_gutter_finds_no_pane() {
        let (px, w) = band(&[(400, 't'), (40, 'b'), (600, 't')], 60);
        assert_eq!(detect(&px, w, 60, 420, 30), None);
    }

    #[test]
    fn a_narrow_strip_is_not_a_pane() {
        // 60 px of icons between two gutters: too narrow to lock to.
        let (px, w) = band(&[(40, 'b'), (60, 't'), (40, 'b'), (600, 't')], 60);
        assert_eq!(detect(&px, w, 60, 70, 30), None);
    }

    #[test]
    fn an_empty_band_finds_no_pane() {
        let (px, w) = band(&[(800, 'b')], 60);
        assert_eq!(detect(&px, w, 60, 400, 30), None);
        assert_eq!(detect(&[], 0, 0, 0, 0), None);
        assert_eq!(detect(&px, w, 60, -1, 30), None);
        assert_eq!(detect(&px, w, 60, 5000, 30), None);
    }
}
