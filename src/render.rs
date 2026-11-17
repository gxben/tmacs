//! Redisplay: windows, mode lines, echo area, completions.

use crate::buffer::BufKind;
use crate::editor::{Editor, MINI, WinId};
use crate::face::Style;
use crate::screen::Screen;
use crate::syntax::{self, Spans};
use crate::text;
use crate::view::{self, Geo};
use std::rc::Rc;

pub fn redisplay(ed: &mut Editor, scr: &mut Screen) {
    if ed.width != scr.w || ed.height != scr.h {
        ed.resize(scr.w, scr.h);
    }
    if ed.redraw_all {
        scr.invalidate();
        ed.redraw_all = false;
    }
    scr.clear(ed.faces.default);
    scr.cursor = None;
    for wid in ed.windows() {
        view::ensure_visible(ed, wid);
        draw_window(ed, scr, wid);
    }
    draw_echo(ed, scr);
    draw_completions(ed, scr);
    if ed.bell.is_some() {
        // Visible bell: briefly invert the selected window's mode line only
        // (a full-screen flash is too aggressive).
        let wid = if ed.sel == MINI {
            ed.mini.as_ref().map(|m| m.ret_win).unwrap_or(1)
        } else {
            ed.sel
        };
        if let Some(Some(w)) = ed.wins.get(wid) {
            let y = w.y + w.h - 1;
            for x in w.x..w.x + w.w {
                scr.set_style(
                    x,
                    y,
                    Style {
                        reverse: true,
                        ..Default::default()
                    },
                );
            }
        }
    }
}

struct Overlay {
    a: usize,
    b: usize,
    style: Style,
}

fn visible_range(ed: &Editor, wid: WinId) -> (usize, usize) {
    let b = ed.buf(ed.win(wid).buf);
    let (l, _) = view::start_vpos(ed, wid);
    let h = ed.text_height(wid);
    let s = b.line_start(l);
    let e = b.line_end((l + h).min(b.line_count().saturating_sub(1)));
    (s, e)
}

fn collect_overlays(ed: &mut Editor, wid: WinId) -> Vec<Overlay> {
    let mut ov = Vec::new();
    let bid = ed.win(wid).buf;
    let selected = wid == ed.sel;
    let pt = ed.win_pt(wid);
    let (vs, ve) = visible_range(ed, wid);
    // highlight-regexp
    if !ed.highlights.is_empty() && bid != ed.mini_buf {
        let text = crate::search::buffer_text(ed, bid);
        let b = ed.buf(bid);
        let (sb, eb) = (b.text.char_to_byte(vs), b.text.char_to_byte(ve));
        for (re, st) in &ed.highlights {
            for m in re.find_iter(&text[sb..eb]) {
                ov.push(Overlay {
                    a: b.text.byte_to_char(sb + m.start()),
                    b: b.text.byte_to_char(sb + m.end()),
                    style: *st,
                });
            }
        }
    }
    // isearch lazy highlight + current match
    if selected && ed.cfg.search_highlight {
        if let Some(is) = &ed.isearch {
            if !is.string.is_empty() {
                if let Ok(re) = crate::search::make_regex(&is.string, is.regex, is.case_fold) {
                    let found = is.found;
                    let lazy = ed.faces.lazy_highlight;
                    let text = crate::search::buffer_text(ed, bid);
                    let b = ed.buf(bid);
                    let (sb, eb) = (b.text.char_to_byte(vs), b.text.char_to_byte(ve));
                    let mut n = 0;
                    for m in re.find_iter(&text[sb..eb]) {
                        if m.start() == m.end() {
                            continue;
                        }
                        ov.push(Overlay {
                            a: b.text.byte_to_char(sb + m.start()),
                            b: b.text.byte_to_char(sb + m.end()),
                            style: lazy,
                        });
                        n += 1;
                        if n > 2000 {
                            break;
                        }
                    }
                    if let Some((a, b)) = found {
                        let st = if ed.isearch.as_ref().unwrap().failing {
                            ed.faces.isearch_fail
                        } else {
                            ed.faces.isearch
                        };
                        ov.push(Overlay { a, b, style: st });
                    }
                }
            }
        }
        if let Some(q) = &ed.qr {
            if let Some((a, b)) = q.cur {
                ov.push(Overlay {
                    a,
                    b,
                    style: ed.faces.isearch,
                });
            }
        }
    }
    // region
    if selected && ed.cfg.transient_mark_mode {
        if let Some((a, b)) = ed.buf(bid).region(pt) {
            ov.push(Overlay {
                a,
                b,
                style: ed.faces.region,
            });
        }
    }
    // show-paren
    if selected && ed.cfg.show_paren_mode && wid != MINI {
        let b = ed.buf(bid);
        let mut done = false;
        if let Some(c) = b.char_at(pt) {
            if text::close_of(c).is_some() {
                if let Some((q, ok)) = text::scan_pair(b, pt, true, 50_000) {
                    let st = if ok {
                        ed.faces.show_paren_match
                    } else {
                        ed.faces.show_paren_mismatch
                    };
                    ov.push(Overlay {
                        a: pt,
                        b: pt + 1,
                        style: st,
                    });
                    ov.push(Overlay {
                        a: q,
                        b: q + 1,
                        style: st,
                    });
                    done = true;
                }
            }
        }
        if !done {
            if let Some(c) = b.char_before(pt) {
                if text::open_of(c).is_some() {
                    if let Some((q, ok)) = text::scan_pair(b, pt, false, 50_000) {
                        let st = if ok {
                            ed.faces.show_paren_match
                        } else {
                            ed.faces.show_paren_mismatch
                        };
                        ov.push(Overlay {
                            a: pt - 1,
                            b: pt,
                            style: st,
                        });
                        ov.push(Overlay {
                            a: q,
                            b: q + 1,
                            style: st,
                        });
                    }
                }
            }
        }
    }
    ov
}

fn spans_for(ed: &mut Editor, wid: WinId, first: usize, last: usize) -> Vec<Spans> {
    let bid = ed.win(wid).buf;
    let b = ed.bufm(bid);
    let Some(mut cache) = b.syntax.take() else {
        return vec![Rc::new(Vec::new()); last - first + 1];
    };
    if b.dirty_line != usize::MAX {
        cache.invalidate(b.dirty_line, b.text.len_lines());
        b.dirty_line = usize::MAX;
    }
    let sp = cache.spans(&b.text, first, last, std::time::Duration::from_millis(20));
    b.syntax = Some(cache);
    sp
}

fn draw_window(ed: &mut Editor, scr: &mut Screen, wid: WinId) {
    let g = view::geo(ed, wid);
    let h = ed.text_height(wid);
    let (wx, wy, ww) = {
        let w = ed.win(wid);
        (w.x, w.y, w.w)
    };
    let tw = ed.text_width(wid);
    let bid = ed.win(wid).buf;
    let selected = wid == ed.sel;
    let pt = ed.win_pt(wid);
    let (mut line, mut row) = view::start_vpos(ed, wid);
    let last = view::last_line(ed.buf(bid));
    let first_line = line;
    let span_last = (line + h).min(last);
    let spans = spans_for(ed, wid, line, span_last);
    let overlays = collect_overlays(ed, wid);
    let faces = ed.faces.clone();
    let show_tw = ed.cfg.show_trailing_whitespace && ed.buf(bid).kind == BufKind::Normal;
    let hl_tabs = ed.cfg.highlight_tabs;
    let hscroll = ed.win(wid).hscroll;
    let mut rows_map = vec![None; h];
    let mut cursor = None;
    let mut srow = 0;
    let b = ed.buf(bid);
    let mut end_visible = false;
    while srow < h {
        if line > last {
            end_visible = true;
            break;
        }
        let (ls, le) = view::line_span(b, line);
        let chars: Vec<char> = b.text.slice(ls..le).chars().collect();
        let rows = view::rows_of(b, line, g);
        let sp = spans.get(line - first_line).cloned().unwrap_or_default();
        let trail = chars
            .iter()
            .rposition(|c| *c != ' ' && *c != '\t')
            .map(|i| i + 1)
            .unwrap_or(0);
        let hide_trail = selected && pt == le;
        let mut si = 0usize;
        for r in row..rows.len() {
            if srow >= h {
                break;
            }
            let cs = rows[r];
            let ce = if r + 1 < rows.len() {
                rows[r + 1]
            } else {
                chars.len()
            };
            let y = wy + srow as u16;
            rows_map[srow] = Some((ls + cs, ls + ce));
            let row_ov: Vec<&Overlay> = overlays
                .iter()
                .filter(|o| o.a < ls + ce + 1 && o.b > ls + cs)
                .collect();
            let mut col = 0usize; // display column within row (or line when truncating)
            let max_col = if g.truncate {
                hscroll + tw.saturating_sub(1)
            } else {
                g.width
            };
            for (i, &c) in chars.iter().enumerate().take(ce).skip(cs) {
                let pos = ls + i;
                while si < sp.len() && (sp[si].end as usize) <= i {
                    si += 1;
                }
                let mut st = Style::default();
                if si < sp.len() && (sp[si].start as usize) <= i {
                    st = syntax::face_style(&faces, sp[si].face);
                }
                if show_tw && i >= trail && !hide_trail {
                    st = st.over(faces.trailing_whitespace);
                }
                for o in &row_ov {
                    if pos >= o.a && pos < o.b {
                        st = st.over(o.style);
                    }
                }
                if selected && pos == pt {
                    let cx = col.saturating_sub(if g.truncate { hscroll } else { 0 });
                    cursor = Some((wx + cx as u16, y));
                }
                let next = text::advance_col(col, c, g.tab_width);
                if g.truncate && next > max_col {
                    break;
                }
                let vis = |cc: usize| -> Option<u16> {
                    if g.truncate {
                        if cc < hscroll {
                            None
                        } else {
                            Some(wx + (cc - hscroll) as u16)
                        }
                    } else {
                        Some(wx + cc as u16)
                    }
                };
                if c == '\t' {
                    let tst = if hl_tabs { st.over(faces.tab) } else { st };
                    for cc in col..next {
                        if let Some(x) = vis(cc) {
                            scr.put(x, y, ' ', tst);
                        }
                    }
                } else if (c as u32) < 0x20 || c == '\u{7f}' {
                    let e = st.over(faces.escape_glyph);
                    let d = if c == '\u{7f}' {
                        '?'
                    } else {
                        char::from_u32(c as u32 + 64).unwrap_or('?')
                    };
                    if let Some(x) = vis(col) {
                        scr.put(x, y, '^', e);
                    }
                    if let Some(x) = vis(col + 1) {
                        scr.put(x, y, d, e);
                    }
                } else if let Some(x) = vis(col) {
                    scr.put(x, y, c, st);
                }
                col = next;
            }
            let is_last_row = r + 1 == rows.len();
            // end-of-line cursor / region extension
            if selected && pt == ls + ce && is_last_row {
                let cx = col.saturating_sub(if g.truncate { hscroll } else { 0 });
                cursor = Some((wx + (cx as u16).min(tw.saturating_sub(1) as u16), y));
            }
            // region covering the newline: extend highlight one column like Emacs
            for o in &row_ov {
                if is_last_row
                    && le >= o.a
                    && le < o.b
                    && o.style.bg.is_some()
                    && o.style == faces.region
                {
                    let cx = col.saturating_sub(if g.truncate { hscroll } else { 0 });
                    if cx < tw {
                        scr.put(wx + cx as u16, y, ' ', o.style);
                    }
                }
            }
            if !is_last_row {
                scr.put(wx + g.width as u16, y, '\\', Style::default());
            } else if g.truncate {
                let total: usize = chars
                    .iter()
                    .fold(0, |c, ch| text::advance_col(c, *ch, g.tab_width));
                if total > hscroll + tw.saturating_sub(1) {
                    scr.put(wx + tw.saturating_sub(1) as u16, y, '$', Style::default());
                }
                if hscroll > 0 {
                    scr.put(wx, y, '$', Style::default());
                }
            }
            srow += 1;
        }
        line += 1;
        row = 0;
        if line > last && srow < h {
            end_visible = true;
        }
    }
    if srow >= h && line > last {
        end_visible = true;
    }
    if let Some(c) = cursor {
        if selected {
            scr.cursor = Some(c);
        }
    }
    ed.winm(wid).rows = rows_map;
    // vertical border
    if ed.has_right_border(wid) {
        for y in 0..=h {
            scr.put(wx + ww - 1, wy + y as u16, '|', faces.vertical_border);
        }
    }
    draw_mode_line(ed, scr, wid, end_visible);
}

pub fn display_time(ed: &Editor) -> String {
    if !ed.cfg.display_time {
        return String::new();
    }
    let now = chrono::Local::now();
    let mut loads = [0f64; 3];
    let load = unsafe {
        if libc::getloadavg(loads.as_mut_ptr(), 3) > 0 && loads[0] > 0.0 {
            format!(" {:.2}", loads[0])
        } else {
            String::new()
        }
    };
    let parts: Vec<&str> = ed.cfg.display_time_format.split("%L").collect();
    let mut out = String::new();
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            out.push_str(&load);
        }
        let items: Vec<chrono::format::Item> = chrono::format::StrftimeItems::new(p).collect();
        if items
            .iter()
            .any(|it| matches!(it, chrono::format::Item::Error))
        {
            out.push_str(p);
        } else {
            out.push_str(&now.format_with_items(items.into_iter()).to_string());
        }
    }
    out
}

fn draw_mode_line(ed: &Editor, scr: &mut Screen, wid: WinId, end_visible: bool) {
    let w = ed.win(wid);
    let y = w.y + w.h - 1;
    let active =
        wid == ed.sel || (ed.sel == MINI && ed.mini.as_ref().map(|m| m.ret_win) == Some(wid));
    let face = if active {
        ed.faces.mode_line
    } else {
        ed.faces.mode_line_inactive
    };
    let b = ed.buf(w.buf);
    let eol = if b.crlf { '\\' } else { ':' };
    let flags = match (b.read_only, b.modified && b.kind == BufKind::Normal) {
        (true, true) => "%*",
        (true, false) => "%%",
        (false, true) => "**",
        (false, false) => "--",
    };
    let start_pos = b.mpos(w.start);
    let at_top = start_pos <= b.begv();
    let pos = if at_top && end_visible {
        "All".to_string()
    } else if at_top {
        "Top".to_string()
    } else if end_visible {
        "Bot".to_string()
    } else {
        let range = (b.zv() - b.begv()).max(1);
        format!("{}%", ((start_pos - b.begv()) * 100 / range).min(99))
    };
    let pt = ed.win_pt(wid);
    let line = b.line_of(pt) + 1;
    let mut lc = String::new();
    if ed.cfg.line_number_mode {
        lc.push_str(&format!("L{line}"));
    }
    if ed.cfg.column_number_mode {
        let tw = if b.lang == 0 {
            ed.cfg.tab_width
        } else {
            ed.langs.list[b.lang].tab_width
        };
        lc = format!("({line},{})", b.column(pt, tw));
    }
    let mode = match b.kind {
        BufKind::BufferList => "Buffer Menu".to_string(),
        BufKind::Dired => "Dired by name".to_string(),
        BufKind::Special => {
            if b.name == "*Occur*" {
                "Occur".into()
            } else {
                "Help".into()
            }
        }
        _ => ed.langs.list[b.lang].mode_name.clone(),
    };
    let mut minor = String::new();
    if b.overwrite {
        minor.push_str(" Ovwrt");
    }
    if b.narrow.is_some() {
        minor.push_str(" Narrow");
    }
    if ed.macro_rec.is_some() && active {
        minor.push_str(" Def");
    }
    let name = format!("{:<12}", b.name);
    let left = format!("-UUU{eol}{flags}--  ");
    let mid = format!("   {pos} {lc}    ({mode}{minor}) {}", display_time(ed));
    scr.fill(w.x, y, w.x + w.w, face);
    let max = w.x + w.w;
    let mut x = scr.put_str(w.x, y, &left, face, max);
    x = scr.put_str(x, y, &name, face.over(ed.faces.mode_line_buffer_id), max);
    x = scr.put_str(x, y, &mid, face, max);
    x = scr.put_str(x, y, " ", face, max);
    while x < max {
        x += scr.put(x, y, '-', face);
    }
}

fn draw_echo(ed: &mut Editor, scr: &mut Screen) {
    let y = scr.h - 1;
    let w = scr.w;
    if let Some(m) = &ed.mini {
        let prompt = m.prompt.clone();
        let pw: usize = prompt.chars().map(text::char_width).sum();
        let x0 = scr.put_str(0, y, &prompt, ed.faces.minibuffer_prompt, w);
        let mb = ed.mini_buf;
        let text: Vec<char> = ed.buf(mb).text.chars().collect();
        let pt = ed.win_pt(MINI);
        // horizontal scroll so cursor is visible
        let avail = (w as usize).saturating_sub(pw).max(1);
        let cur_col: usize = text[..pt.min(text.len())]
            .iter()
            .map(|c| text::char_width(*c))
            .sum();
        let hs = if cur_col >= avail {
            cur_col + 1 - avail
        } else {
            0
        };
        let mut col = 0;
        let mut x = x0;
        let mut cursor_x = None;
        let region = ed.buf(mb).region(pt);
        for (i, &c) in text.iter().enumerate() {
            if i == pt {
                cursor_x = Some(x);
            }
            let cw = text::char_width(c);
            if col + cw <= hs {
                col += cw;
                continue;
            }
            col += cw;
            let st = match region {
                Some((a, b)) if i >= a && i < b && ed.sel == MINI => ed.faces.region,
                _ => Style::default(),
            };
            if (c as u32) < 0x20 {
                x = scr.put_str(
                    x,
                    y,
                    &format!("^{}", char::from_u32(c as u32 + 64).unwrap_or('?')),
                    ed.faces.escape_glyph,
                    w,
                );
            } else {
                x += scr.put(x, y, c, st);
            }
            if x >= w {
                break;
            }
        }
        if pt >= text.len() {
            cursor_x = Some(x.min(w - 1));
        }
        // transient message after input, like " [No match]"
        if let Some(msg) = &ed.message {
            if msg.starts_with(" [") {
                scr.put_str(x, y, msg, Style::default(), w);
            }
        }
        if ed.sel == MINI {
            scr.cursor = cursor_x.map(|cx| (cx, y));
        }
        return;
    }
    if let Some(msg) = &ed.message {
        let first = msg.lines().next().unwrap_or("");
        let x = scr.put_str(0, y, first, Style::default(), w);
        if ed.read_key.is_some() {
            scr.cursor = Some((x.min(w - 1), y));
        }
    }
}

fn draw_completions(ed: &Editor, scr: &mut Screen) {
    let Some(c) = &ed.completions else { return };
    if c.is_empty() || scr.h < 4 {
        return;
    }
    let w = scr.w as usize;
    let maxlen = c.iter().map(|s| s.chars().count()).max().unwrap_or(1) + 2;
    let cols = (w / maxlen).max(1);
    let nrows = c.len().div_ceil(cols);
    let avail = (scr.h as usize).saturating_sub(3);
    let shown_rows = nrows.min(avail.max(1));
    let top = scr.h as usize - 1 - shown_rows - 1;
    let face = Style::default();
    scr.fill(0, top as u16, scr.w, ed.faces.mode_line_inactive);
    scr.put_str(
        0,
        top as u16,
        &format!("Possible completions are ({}):", c.len()),
        ed.faces.mode_line_inactive,
        scr.w,
    );
    for r in 0..shown_rows {
        let y = (top + 1 + r) as u16;
        scr.fill(0, y, scr.w, face);
        for col in 0..cols {
            // column-major like Emacs' vertical completions
            let i = col * nrows + r;
            if let Some(s) = c.get(i) {
                let x = (col * maxlen) as u16;
                scr.put_str(x, y, s, ed.faces.completions, scr.w);
            }
        }
    }
}

/// Map a screen coordinate to (window, buffer position).
pub fn pos_at_screen(ed: &Editor, x: u16, y: u16) -> Option<(WinId, Option<usize>, bool)> {
    for wid in ed.windows() {
        let w = ed.win(wid);
        if x >= w.x && x < w.x + w.w && y >= w.y && y < w.y + w.h {
            let mode_line = y == w.y + w.h - 1;
            if mode_line {
                return Some((wid, None, true));
            }
            let row = (y - w.y) as usize;
            let Some(Some((rs, re))) = w.rows.get(row).copied() else {
                // below end of buffer
                let b = ed.buf(w.buf);
                return Some((wid, Some(b.zv()), false));
            };
            let b = ed.buf(w.buf);
            let g: Geo = view::geo(ed, wid);
            let target = (x - w.x) as usize + if g.truncate { w.hscroll } else { 0 };
            let mut col = 0;
            let mut p = if g.truncate { b.bol(rs) } else { rs };
            let end = re;
            while p < end {
                let c = b.text.char(p);
                let n = text::advance_col(col, c, g.tab_width);
                if n > target {
                    break;
                }
                col = n;
                p += 1;
            }
            return Some((wid, Some(p), false));
        }
    }
    None
}
