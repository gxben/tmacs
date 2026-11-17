//! Window geometry: visual rows (continuation lines), vertical motion, scrolling.

use crate::buffer::Buffer;
use crate::editor::{Editor, MINI, WinId};
use crate::text;

#[derive(Clone, Copy)]
pub struct Geo {
    /// Text columns available (excluding the continuation glyph column).
    pub width: usize,
    pub tab_width: usize,
    pub truncate: bool,
}

pub fn geo(ed: &Editor, wid: WinId) -> Geo {
    let w = ed.text_width(wid);
    let b = ed.buf(ed.win(wid).buf);
    let tw = if b.lang == 0 {
        ed.cfg.tab_width
    } else {
        ed.langs.list[b.lang].tab_width
    };
    let truncate = wid == MINI || ed.cfg.truncate_lines;
    Geo {
        width: w.saturating_sub(1).max(1),
        tab_width: tw,
        truncate,
    }
}

/// Accessible span (start, end) of a line (clipped to narrowing).
pub fn line_span(b: &Buffer, line: usize) -> (usize, usize) {
    let s = b.line_start(line).max(b.begv());
    let e = b.line_end(line).min(b.zv());
    (s, e.max(s))
}

pub fn line_chars(b: &Buffer, line: usize) -> Vec<char> {
    let (s, e) = line_span(b, line);
    b.text.slice(s..e).chars().collect()
}

/// Row start offsets (chars from line start) for a line.
pub fn rows_of(b: &Buffer, line: usize, g: Geo) -> Vec<usize> {
    if g.truncate {
        return vec![0];
    }
    let (s, e) = line_span(b, line);
    // Fast path: short ASCII-ish lines without tabs.
    if e - s <= g.width {
        let sl = b.text.slice(s..e);
        if sl
            .chars()
            .all(|c| c != '\t' && (c as u32) >= 0x20 && (c as u32) < 0x1100)
        {
            return vec![0];
        }
    }
    text::wrap_rows(&line_chars(b, line), g.width, g.tab_width)
}

pub fn first_line(b: &Buffer) -> usize {
    b.line_of(b.begv())
}

pub fn last_line(b: &Buffer) -> usize {
    b.line_of(b.zv())
}

/// (line, row) of a buffer position.
pub fn vpos(b: &Buffer, pos: usize, g: Geo) -> (usize, usize) {
    let line = b.line_of(pos);
    let (s, _) = line_span(b, line);
    let off = pos.saturating_sub(s);
    let rows = rows_of(b, line, g);
    let r = rows.iter().rposition(|&r| r <= off).unwrap_or(0);
    (line, r)
}

pub fn next_row(b: &Buffer, (l, r): (usize, usize), g: Geo) -> Option<(usize, usize)> {
    if r + 1 < rows_of(b, l, g).len() {
        Some((l, r + 1))
    } else if l < last_line(b) {
        Some((l + 1, 0))
    } else {
        None
    }
}

pub fn prev_row(b: &Buffer, (l, r): (usize, usize), g: Geo) -> Option<(usize, usize)> {
    if r > 0 {
        Some((l, r - 1))
    } else if l > first_line(b) {
        let n = rows_of(b, l - 1, g).len();
        Some((l - 1, n - 1))
    } else {
        None
    }
}

/// Display x of `pos` within its visual row.
pub fn x_of(b: &Buffer, pos: usize, g: Geo) -> usize {
    let (line, r) = vpos(b, pos, g);
    let (s, _) = line_span(b, line);
    let rows = rows_of(b, line, g);
    let rs = s + rows[r];
    let mut col = 0;
    for c in b.text.slice(rs..pos.max(rs)).chars() {
        col = text::advance_col(col, c, g.tab_width);
    }
    col
}

/// Buffer position at display column `x` in visual row (line, r).
pub fn pos_at(b: &Buffer, (line, r): (usize, usize), x: usize, g: Geo) -> usize {
    let (s, e) = line_span(b, line);
    let rows = rows_of(b, line, g);
    let rs = s + rows[r.min(rows.len() - 1)];
    let re = if r + 1 < rows.len() {
        s + rows[r + 1]
    } else {
        e
    };
    let mut col = 0;
    let mut p = rs;
    while p < re {
        let c = b.text.char(p);
        let n = text::advance_col(col, c, g.tab_width);
        if n > x {
            break;
        }
        col = n;
        p += 1;
    }
    // Cannot sit at the end of a continued row (that's the next row's start).
    if p == re && r + 1 < rows.len() && p > rs {
        p -= 1;
    }
    p
}

/// Window start as (line, row).
pub fn start_vpos(ed: &Editor, wid: WinId) -> (usize, usize) {
    let w = ed.win(wid);
    let b = ed.buf(w.buf);
    let g = geo(ed, wid);
    let line = b
        .line_of(b.mpos(w.start))
        .max(first_line(b))
        .min(last_line(b));
    let n = rows_of(b, line, g).len();
    (line, w.start_row.min(n - 1))
}

pub fn set_start(ed: &mut Editor, wid: WinId, (line, row): (usize, usize)) {
    let w = ed.win(wid);
    let (bid, sm) = (w.buf, w.start);
    let b = ed.bufm(bid);
    let p = line_span(b, line).0;
    b.set_mpos(sm, p);
    ed.winm(wid).start_row = row;
}

/// Count rows from `a` to `b` (b >= a), giving up after `max`.
pub fn rows_between(
    b: &Buffer,
    a: (usize, usize),
    t: (usize, usize),
    g: Geo,
    max: usize,
) -> Option<usize> {
    if t < a {
        return None;
    }
    let mut cur = a;
    let mut n = 0;
    while cur < t {
        cur = next_row(b, cur, g)?;
        n += 1;
        if n > max {
            return None;
        }
    }
    Some(n)
}

pub fn move_rows(b: &Buffer, mut p: (usize, usize), n: isize, g: Geo) -> ((usize, usize), isize) {
    let mut moved = 0;
    if n > 0 {
        for _ in 0..n {
            match next_row(b, p, g) {
                Some(q) => {
                    p = q;
                    moved += 1;
                }
                None => break,
            }
        }
    } else {
        for _ in 0..(-n) {
            match prev_row(b, p, g) {
                Some(q) => {
                    p = q;
                    moved -= 1;
                }
                None => break,
            }
        }
    }
    (p, moved)
}

/// Put point's row at screen row `target` (recenter).
pub fn recenter(ed: &mut Editor, wid: WinId, target: usize) {
    let g = geo(ed, wid);
    let pt = ed.win_pt(wid);
    let b = ed.buf(ed.win(wid).buf);
    let pv = vpos(b, pt, g);
    let (start, _) = move_rows(b, pv, -(target as isize), g);
    set_start(ed, wid, start);
}

/// Scroll so that point is visible (scroll-step semantics), and keep
/// horizontal scrolling in truncate mode.
pub fn ensure_visible(ed: &mut Editor, wid: WinId) {
    let g = geo(ed, wid);
    let h = ed.text_height(wid);
    let pt = ed.win_pt(wid);
    let step = ed.cfg.scroll_step;
    if g.truncate {
        let b = ed.buf(ed.win(wid).buf);
        let col = b.column(pt, g.tab_width);
        let w = ed.win(wid);
        let mut hs = w.hscroll;
        let width = g.width;
        if col < hs || col >= hs + width {
            hs = if wid == MINI {
                col.saturating_sub(width * 3 / 4)
            } else {
                col.saturating_sub(width / 2)
            };
        }
        ed.winm(wid).hscroll = hs;
    } else {
        ed.winm(wid).hscroll = 0;
    }
    if wid == MINI {
        return;
    }
    let start = start_vpos(ed, wid);
    let b = ed.buf(ed.win(wid).buf);
    let pv = vpos(b, pt, g);
    if pv < start {
        let d = rows_between(b, pv, start, g, step.max(1));
        match d {
            Some(d) if d <= step && step > 0 => set_start(ed, wid, pv),
            _ => recenter(ed, wid, h / 2),
        }
        return;
    }
    match rows_between(b, start, pv, g, h + step) {
        Some(n) if n < h => {
            // Also normalise start into canonical form.
            set_start(ed, wid, start);
        }
        Some(n) if step > 0 && n + 1 - h <= step => {
            let (ns, _) = move_rows(b, start, (n + 1 - h) as isize, g);
            set_start(ed, wid, ns);
        }
        _ => recenter(ed, wid, h / 2),
    }
}

/// Screen row of point in the window, if visible.
pub fn point_screen_row(ed: &Editor, wid: WinId) -> Option<usize> {
    let g = geo(ed, wid);
    let b = ed.buf(ed.win(wid).buf);
    let pv = vpos(b, ed.win_pt(wid), g);
    rows_between(b, start_vpos(ed, wid), pv, g, ed.text_height(wid))
}

/// Scroll window by `n` rows (positive = text moves up). Returns rows moved.
/// Point is moved into the window if it would leave it.
pub fn scroll_rows(ed: &mut Editor, wid: WinId, n: isize) -> isize {
    let g = geo(ed, wid);
    let h = ed.text_height(wid);
    let start = start_vpos(ed, wid);
    let b = ed.buf(ed.win(wid).buf);
    // Don't scroll past the last line being at the top.
    let (ns, moved) = move_rows(b, start, n, g);
    set_start(ed, wid, ns);
    let pt = ed.win_pt(wid);
    let b = ed.buf(ed.win(wid).buf);
    let pv = vpos(b, pt, g);
    let x = x_of(b, pt, g);
    if pv < ns {
        let p = pos_at(b, ns, x, g);
        set_win_point(ed, wid, p);
    } else if rows_between(b, ns, pv, g, h)
        .map(|r| r >= h)
        .unwrap_or(true)
    {
        let (last, _) = move_rows(b, ns, h as isize - 1, g);
        let p = pos_at(b, last, x, g);
        set_win_point(ed, wid, p);
    }
    moved
}

pub fn set_win_point(ed: &mut Editor, wid: WinId, p: usize) {
    let w = ed.win(wid);
    let (bid, m) = (w.buf, w.pt);
    let b = ed.bufm(bid);
    let p = b.clamp(p);
    b.set_mpos(m, p);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn buf(s: &str) -> Buffer {
        let mut b = Buffer::new("t", PathBuf::from("/"));
        b.insert(0, s);
        b
    }

    const G: Geo = Geo {
        width: 4,
        tab_width: 8,
        truncate: false,
    };

    #[test]
    fn vpos_and_rows_with_wrapping() {
        let b = buf("abcdefghij\nxy");
        assert_eq!(rows_of(&b, 0, G), vec![0, 4, 8]);
        assert_eq!(vpos(&b, 5, G), (0, 1));
        assert_eq!(vpos(&b, 11, G), (1, 0));
        assert_eq!(next_row(&b, (0, 2), G), Some((1, 0)));
        assert_eq!(prev_row(&b, (1, 0), G), Some((0, 2)));
        assert_eq!(next_row(&b, (1, 0), G), None);
    }

    #[test]
    fn x_and_pos_at_are_inverse() {
        let b = buf("abcdefghij");
        for p in 0..10 {
            let (l, r) = vpos(&b, p, G);
            assert_eq!(pos_at(&b, (l, r), x_of(&b, p, G), G), p);
        }
        // Can't land on the end of a continued row.
        assert_eq!(pos_at(&b, (0, 0), 99, G), 3);
    }

    #[test]
    fn tabs_count_display_columns() {
        let b = buf("\tx");
        let g = Geo {
            width: 80,
            tab_width: 8,
            truncate: false,
        };
        assert_eq!(x_of(&b, 1, g), 8);
        assert_eq!(pos_at(&b, (0, 0), 5, g), 0);
    }

    #[test]
    fn rows_between_and_move_rows() {
        let b = buf("a\nb\nc\nd");
        assert_eq!(rows_between(&b, (0, 0), (3, 0), G, 10), Some(3));
        assert_eq!(rows_between(&b, (0, 0), (3, 0), G, 2), None);
        assert_eq!(move_rows(&b, (1, 0), 10, G), ((3, 0), 2));
        assert_eq!(move_rows(&b, (1, 0), -10, G), ((0, 0), -1));
    }

    #[test]
    fn narrowing_clips_lines() {
        let mut b = buf("hello\nworld");
        let s = b.new_marker(2, false);
        let e = b.new_marker(8, true);
        b.narrow = Some((s, e));
        assert_eq!(line_span(&b, 0), (2, 5));
        assert_eq!(line_span(&b, 1), (6, 8));
        assert_eq!(first_line(&b), 0);
    }
}
