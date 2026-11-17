// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Benjamin Zores

//! Incremental search, query-replace, occur, highlight-regexp.

use crate::buffer::{BufKind, Buffer};
use crate::editor::{BufId, Completion, Editor, PromptKind};
use crate::key::{K, Key};
use regex::Regex;
use std::rc::Rc;

pub struct ISearch {
    pub forward: bool,
    pub regex: bool,
    pub string: String,
    pub origin: usize,
    pub found: Option<(usize, usize)>,
    pub failing: bool,
    pub wrapped: bool,
    pub case_fold: bool,
    stack: Vec<IState>,
}

/// Saved isearch state: (string, match, failing, wrapped, forward).
type IState = (String, Option<(usize, usize)>, bool, bool, bool);

pub struct QueryReplace {
    pub from: String,
    pub to: String,
    pub regex: bool,
    pub query: bool,
    pub cur: Option<(usize, usize)>,
    pub count: usize,
    pub end: Option<usize>,
    pub origin: usize,
    pub caps: Vec<Option<String>>,
}

/// Cached full text of a buffer (for regex searching).
pub fn buffer_text(ed: &mut Editor, bid: BufId) -> Rc<String> {
    let tick = ed.buf(bid).change_tick;
    if let Some((b, t, s)) = &ed.text_cache {
        if *b == bid && *t == tick {
            return s.clone();
        }
    }
    let s = Rc::new(ed.buf(bid).text.to_string());
    ed.text_cache = Some((bid, tick, s.clone()));
    s
}

pub fn make_regex(pat: &str, regex: bool, case_fold: bool) -> Result<Regex, String> {
    let p = if regex {
        emacs_to_rust_regex(pat)
    } else {
        regex::escape(pat)
    };
    // Emacs "smart case": fold only if the pattern is all lower case.
    let fold = case_fold && !pat.chars().any(|c| c.is_uppercase());
    regex::RegexBuilder::new(&p)
        .case_insensitive(fold)
        .multi_line(true)
        .build()
        .map_err(|e| e.to_string())
}

/// Translate common Emacs regexp syntax to Rust regex syntax.
pub fn emacs_to_rust_regex(p: &str) -> String {
    let mut out = String::new();
    let mut it = p.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\\' => match it.next() {
                Some('(') => {
                    if it.peek() == Some(&'?') {
                        it.next();
                        if it.peek() == Some(&':') {
                            it.next();
                        }
                        out.push_str("(?:");
                    } else {
                        out.push('(');
                    }
                }
                Some(')') => out.push(')'),
                Some('|') => out.push('|'),
                Some('{') => out.push('{'),
                Some('}') => out.push('}'),
                Some('<') | Some('>') | Some('b') => out.push_str("\\b"),
                Some('_') => {
                    it.next();
                    out.push_str("\\b");
                }
                Some('`') => out.push_str("\\A"),
                Some('\'') => out.push_str("\\z"),
                Some('w') => out.push_str("\\w"),
                Some('W') => out.push_str("\\W"),
                Some('s') => match it.next() {
                    Some('-') | Some(' ') => out.push_str("\\s"),
                    Some('w') => out.push_str("\\w"),
                    Some('_') => out.push_str("[\\w_]"),
                    Some(_) => out.push_str("\\s"),
                    None => out.push_str("\\\\s"),
                },
                Some('S') => {
                    it.next();
                    out.push_str("\\S");
                }
                Some(d) if d.is_ascii_digit() => {
                    out.push('\\');
                    out.push(d);
                }
                Some(o) => {
                    out.push('\\');
                    out.push(o);
                }
                None => out.push_str("\\\\"),
            },
            '(' | ')' | '|' | '{' | '}' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

fn byte_to_char(b: &Buffer, byte: usize) -> usize {
    b.text.byte_to_char(byte)
}

fn char_to_byte(b: &Buffer, ch: usize) -> usize {
    b.text.char_to_byte(ch)
}

/// Search forward from `from` (chars); returns char range.
pub fn search_forward(
    ed: &mut Editor,
    bid: BufId,
    re: &Regex,
    from: usize,
    limit: Option<usize>,
) -> Option<(usize, usize)> {
    let text = buffer_text(ed, bid);
    let b = ed.buf(bid);
    let from_b = char_to_byte(b, from.min(b.len()));
    let lim_b = limit
        .map(|l| char_to_byte(b, l.min(b.len())))
        .unwrap_or(text.len());
    if from_b > lim_b {
        return None;
    }
    let m = re.find_at(&text[..lim_b], from_b)?;
    Some((byte_to_char(b, m.start()), byte_to_char(b, m.end())))
}

/// Search backward: last match starting before `before` (chars).
pub fn search_backward(
    ed: &mut Editor,
    bid: BufId,
    re: &Regex,
    before: usize,
    limit: usize,
) -> Option<(usize, usize)> {
    let text = buffer_text(ed, bid);
    let b = ed.buf(bid);
    let before_b = char_to_byte(b, before.min(b.len()));
    let lim_b = char_to_byte(b, limit.min(b.len()));
    // Scan backwards in chunks for efficiency.
    let mut chunk = 4096usize;
    loop {
        let lo = before_b.saturating_sub(chunk).max(lim_b);
        let lo = (lo..=before_b)
            .find(|&i| text.is_char_boundary(i))
            .unwrap_or(before_b);
        let mut best = None;
        let mut pos = lo;
        while pos <= before_b {
            let Some(m) = re.find_at(&text, pos) else {
                break;
            };
            if m.start() >= before_b {
                break;
            }
            best = Some((m.start(), m.end()));
            pos = m.start()
                + text[m.start()..]
                    .chars()
                    .next()
                    .map(|c| c.len_utf8())
                    .unwrap_or(1);
        }
        if let Some((s, e)) = best {
            return Some((byte_to_char(b, s), byte_to_char(b, e)));
        }
        if lo <= lim_b {
            return None;
        }
        chunk *= 4;
    }
}

// ======================= isearch =======================

fn start_isearch(ed: &mut Editor, forward: bool, regex: bool) {
    let p = ed.pt();
    ed.isearch = Some(ISearch {
        forward,
        regex,
        string: String::new(),
        origin: p,
        found: None,
        failing: false,
        wrapped: false,
        case_fold: ed.cfg.case_fold_search,
        stack: Vec::new(),
    });
    isearch_message(ed);
}

pub fn isearch_forward(ed: &mut Editor) {
    let re = ed.arg.is_some();
    start_isearch(ed, true, re);
}
pub fn isearch_backward(ed: &mut Editor) {
    let re = ed.arg.is_some();
    start_isearch(ed, false, re);
}
pub fn isearch_forward_regexp(ed: &mut Editor) {
    start_isearch(ed, true, true);
}
pub fn isearch_backward_regexp(ed: &mut Editor) {
    start_isearch(ed, false, true);
}

pub fn isearch_symbol_at_point(ed: &mut Editor) {
    let b = ed.b();
    let p = ed.pt();
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    let mut s = p;
    while s > 0 && ident(b.text.char(s - 1)) {
        s -= 1;
    }
    let mut e = p;
    while e < b.len() && ident(b.text.char(e)) {
        e += 1;
    }
    if s == e {
        ed.error("No symbol at point");
        return;
    }
    let sym = b.slice(s, e);
    ed.goto(s);
    start_isearch(ed, true, true);
    let is = ed.isearch.as_mut().unwrap();
    is.string = format!("\\_<{}\\_>", regex::escape(&sym));
    is.origin = s;
    isearch_update(ed, s);
}

fn isearch_message(ed: &mut Editor) {
    let Some(is) = &ed.isearch else { return };
    let mut m = String::new();
    if is.failing {
        m.push_str("Failing ");
    }
    if is.wrapped {
        m.push_str(if is.failing {
            "overwrapped "
        } else {
            "Wrapped "
        });
    }
    if is.regex {
        m.push_str("Regexp ");
    }
    m.push_str("I-search");
    if !is.forward {
        m.push_str(" backward");
    }
    m.push_str(": ");
    m.push_str(&is.string);
    ed.message = Some(capitalize_first(&m));
}

fn capitalize_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Re-run search for the current string from `from`.
fn isearch_update(ed: &mut Editor, from: usize) {
    let bid = ed.cur();
    let (string, regex, fold, forward) = {
        let is = ed.isearch.as_ref().unwrap();
        (is.string.clone(), is.regex, is.case_fold, is.forward)
    };
    if string.is_empty() {
        let is = ed.isearch.as_mut().unwrap();
        is.found = None;
        is.failing = false;
        let o = is.origin;
        ed.goto(o);
        isearch_message(ed);
        return;
    }
    let re = match make_regex(&string, regex, fold) {
        Ok(r) => r,
        Err(_) => {
            // incomplete regexp: keep state
            let is = ed.isearch.as_mut().unwrap();
            is.failing = false;
            isearch_message(ed);
            if let Some(m) = &mut ed.message {
                m.push_str(" [incomplete input]");
            }
            return;
        }
    };
    let (bv, zv) = (ed.b().begv(), ed.b().zv());
    let r = if forward {
        search_forward(ed, bid, &re, from, Some(zv))
    } else {
        search_backward(ed, bid, &re, from + 1, bv)
    };
    let r = r.filter(|&(s, _)| if forward { s >= from } else { s <= from });
    let is = ed.isearch.as_mut().unwrap();
    match r {
        Some((s, e)) => {
            is.found = Some((s, e));
            is.failing = false;
            let p = if is.forward { e } else { s };
            ed.goto(p);
        }
        None => {
            is.failing = true;
        }
    }
    isearch_message(ed);
}

fn isearch_repeat(ed: &mut Editor, forward: bool) {
    let bid = ed.cur();
    let is = ed.isearch.as_mut().unwrap();
    if is.string.is_empty() {
        let ring = if is.regex {
            &ed.regex_search_ring
        } else {
            &ed.search_ring
        };
        if let Some(s) = ring.first().cloned() {
            let is = ed.isearch.as_mut().unwrap();
            is.forward = forward;
            is.string = s;
            let o = is.origin;
            isearch_update(ed, o);
        } else {
            let is = ed.isearch.as_mut().unwrap();
            is.forward = forward;
            isearch_message(ed);
        }
        return;
    }
    if is.forward != forward {
        is.forward = forward;
        // Reverse direction: stay at the current match.
        isearch_message(ed);
        return;
    }
    let snapshot = (
        is.string.clone(),
        is.found,
        is.failing,
        is.wrapped,
        is.forward,
    );
    is.stack.push(snapshot);
    let (bv, zv) = (ed.b().begv(), ed.b().zv());
    let cur_pt = ed.pt();
    let is = ed.isearch.as_mut().unwrap();
    let from = if is.failing {
        is.wrapped = true;
        if forward { bv } else { zv }
    } else {
        match is.found {
            Some((s, e)) => {
                if forward {
                    if e == s { e + 1 } else { e }
                } else if s == 0 {
                    0
                } else {
                    s - 1
                }
            }
            None => cur_pt,
        }
    };
    let re = match make_regex(&is.string, is.regex, is.case_fold) {
        Ok(r) => r,
        Err(_) => return,
    };
    let r = if forward {
        search_forward(ed, bid, &re, from, Some(zv))
    } else {
        search_backward(ed, bid, &re, from + 1, bv)
    };
    let is = ed.isearch.as_mut().unwrap();
    match r {
        Some((s, e)) => {
            is.found = Some((s, e));
            is.failing = false;
            let p = if forward { e } else { s };
            ed.goto(p);
        }
        None => {
            is.failing = true;
            ed.ding();
        }
    }
    isearch_message(ed);
}

pub fn isearch_exit(ed: &mut Editor) {
    let Some(is) = ed.isearch.take() else { return };
    if !is.string.is_empty() {
        let ring = if is.regex {
            &mut ed.regex_search_ring
        } else {
            &mut ed.search_ring
        };
        ring.retain(|s| s != &is.string);
        ring.insert(0, is.string.clone());
        ring.truncate(16);
    }
    let p = ed.pt();
    if p != is.origin {
        let o = is.origin;
        let b = ed.bm();
        if !b.mark_active {
            b.push_mark(o, false);
            ed.msg("Mark saved where search started");
            return;
        }
    }
    ed.message = None;
}

fn isearch_add(ed: &mut Editor, s: &str) {
    let is = ed.isearch.as_mut().unwrap();
    is.stack.push((
        is.string.clone(),
        is.found,
        is.failing,
        is.wrapped,
        is.forward,
    ));
    is.string.push_str(s);
    // The match may extend in place: search again from its start.
    let from = is.found.map(|(st, _)| st).unwrap_or(is.origin);
    if is.failing && !is.regex {
        isearch_message(ed);
        ed.ding();
        return;
    }
    isearch_update(ed, from);
    if ed.isearch.as_ref().map(|i| i.failing).unwrap_or(false) {
        ed.ding();
    }
}

/// Handle a key during isearch. Returns false if the key should be
/// processed as a normal command (isearch exited).
pub fn isearch_key(ed: &mut Editor, key: Key) -> bool {
    if let Some(c) = key.plain_char() {
        isearch_add(ed, &c.to_string());
        return true;
    }
    match (key.k, key.ctrl, key.meta) {
        (K::Char('s'), true, false) => isearch_repeat(ed, true),
        (K::Char('r'), true, false) => isearch_repeat(ed, false),
        (K::Char('s'), true, true) => isearch_repeat(ed, true),
        (K::Char('r'), true, true) => isearch_repeat(ed, false),
        (K::Backspace, false, false) => {
            let is = ed.isearch.as_mut().unwrap();
            if let Some((s, f, fail, wr, fw)) = is.stack.pop() {
                is.string = s;
                is.found = f;
                is.failing = fail;
                is.wrapped = wr;
                is.forward = fw;
                let p = match f {
                    Some((a, b)) => {
                        if fw {
                            b
                        } else {
                            a
                        }
                    }
                    None => is.origin,
                };
                ed.goto(p);
            } else {
                ed.ding();
            }
            isearch_message(ed);
        }
        (K::Char('g'), true, false) => {
            let is = ed.isearch.as_mut().unwrap();
            if is.failing && !is.string.is_empty() {
                // Remove failing part.
                while let Some((s, f, fail, wr, fw)) = is.stack.pop() {
                    if !fail {
                        is.string = s;
                        is.found = f;
                        is.failing = false;
                        is.wrapped = wr;
                        is.forward = fw;
                        break;
                    }
                }
                if is.failing {
                    is.string.clear();
                    is.failing = false;
                    is.found = None;
                }
                let p = match is.found {
                    Some((a, b)) => {
                        if is.forward {
                            b
                        } else {
                            a
                        }
                    }
                    None => is.origin,
                };
                ed.goto(p);
                isearch_message(ed);
            } else {
                let o = is.origin;
                ed.isearch = None;
                ed.goto(o);
                ed.msg("Quit");
                ed.ding();
            }
        }
        (K::Enter, false, false) => {
            let empty = ed.isearch.as_ref().unwrap().string.is_empty();
            if empty {
                ed.isearch = None;
                ed.message = None;
                let fw = true;
                let _ = fw;
                ed.msg("Search aborted");
            } else {
                isearch_exit(ed);
            }
        }
        (K::Char('w'), true, false) => {
            // yank next word from buffer
            let p = ed.pt();
            let q = crate::text::forward_word(ed.b(), p);
            let s = ed.b().slice(p, q);
            let s = if ed.isearch.as_ref().unwrap().regex {
                regex::escape(&s)
            } else {
                s
            };
            isearch_add(ed, &s);
        }
        (K::Char('y'), true, false) | (K::Char('y'), false, true) => {
            if let Some(s) = ed.kill_ring.first().cloned() {
                isearch_add(ed, &s);
            }
        }
        (K::Char('c'), false, true) => {
            let is = ed.isearch.as_mut().unwrap();
            is.case_fold = !is.case_fold;
            let f = is.found.map(|f| f.0).unwrap_or(is.origin);
            isearch_update(ed, f);
            let fold = ed.isearch.as_ref().unwrap().case_fold;
            ed.msg(if fold {
                "case insensitive"
            } else {
                "case sensitive"
            });
        }
        (K::Char('r'), false, true) => {
            let is = ed.isearch.as_mut().unwrap();
            is.regex = !is.regex;
            let f = is.found.map(|f| f.0).unwrap_or(is.origin);
            isearch_update(ed, f);
        }
        (K::Char('%'), false, true) => {
            let is = ed.isearch.take().unwrap();
            if let Some((s, _)) = is.found {
                ed.goto(s);
            }
            let regex = is.regex;
            ed.read_string(
                &format!(
                    "Query replace{} {} with: ",
                    if regex { " regexp" } else { "" },
                    is.string
                ),
                PromptKind::QueryReplaceTo {
                    regex,
                    query: true,
                    from: is.string.clone(),
                },
                "",
                Completion::None,
                "replace",
            );
        }
        (K::Char('q'), true, false) => {
            // quoted char: next key literally — simplified: insert C-q's char via read
            ed.ding();
        }
        (K::Tab, false, false) => isearch_add(ed, "\t"),
        (K::Char('j'), true, false) => isearch_add(ed, "\n"),
        _ => {
            isearch_exit(ed);
            return false;
        }
    }
    true
}

// ======================= query-replace =======================

pub fn query_replace(ed: &mut Editor) {
    start_replace_prompt(ed, false, true);
}
pub fn query_replace_regexp(ed: &mut Editor) {
    start_replace_prompt(ed, true, true);
}
pub fn replace_string(ed: &mut Editor) {
    start_replace_prompt(ed, false, false);
}
pub fn replace_regexp(ed: &mut Editor) {
    start_replace_prompt(ed, true, false);
}

fn start_replace_prompt(ed: &mut Editor, regex: bool, query: bool) {
    if !ed.check_writable() {
        return;
    }
    let what = match (query, regex) {
        (true, false) => "Query replace",
        (true, true) => "Query replace regexp",
        (false, false) => "Replace string",
        (false, true) => "Replace regexp",
    };
    let def = ed.replace_hist.first().cloned();
    let prompt = match &def {
        Some((f, t)) => format!("{what} (default {f} -> {t}): "),
        None => format!("{what}: "),
    };
    ed.read_string(
        &prompt,
        PromptKind::QueryReplaceFrom { regex, query },
        "",
        Completion::None,
        "replace",
    );
}

pub fn replace_from_entered(ed: &mut Editor, from: String, regex: bool, query: bool) {
    if from.is_empty() {
        if let Some((f, t)) = ed.replace_hist.first().cloned() {
            start_replace(ed, f, t, regex, query);
        }
        return;
    }
    let what = match (query, regex) {
        (true, false) => "Query replace",
        (true, true) => "Query replace regexp",
        (false, false) => "Replace string",
        (false, true) => "Replace regexp",
    };
    ed.read_string(
        &format!("{what} {from} with: "),
        PromptKind::QueryReplaceTo { regex, query, from },
        "",
        Completion::None,
        "replace",
    );
}

pub fn start_replace(ed: &mut Editor, from: String, to: String, regex: bool, query: bool) {
    ed.replace_hist.retain(|(f, t)| !(f == &from && t == &to));
    ed.replace_hist.insert(0, (from.clone(), to.clone()));
    let p = ed.pt();
    let (start, end) = match ed.b().region(p) {
        Some((a, b)) => (a, Some(b)),
        None => (p, None),
    };
    ed.bm().mark_active = false;
    ed.bm().push_mark(p, false);
    ed.goto(start);
    ed.qr = Some(QueryReplace {
        from,
        to,
        regex,
        query,
        cur: None,
        count: 0,
        end,
        origin: start,
        caps: Vec::new(),
    });
    qr_next(ed, start);
    if !query {
        while ed.qr.as_ref().map(|q| q.cur.is_some()).unwrap_or(false) {
            qr_replace_current(ed);
            let p = ed.pt();
            let p = if ed
                .qr
                .as_ref()
                .unwrap()
                .cur
                .map(|(s, e)| s == e)
                .unwrap_or(false)
            {
                p + 1
            } else {
                p
            };
            qr_next(ed, p);
        }
    }
}

fn qr_regex(ed: &Editor) -> Option<Regex> {
    let q = ed.qr.as_ref()?;
    make_regex(&q.from, q.regex, ed.cfg.case_fold_search).ok()
}

fn qr_next(ed: &mut Editor, from: usize) {
    let bid = ed.cur();
    let Some(re) = qr_regex(ed) else {
        ed.qr = None;
        ed.error("Invalid regexp");
        return;
    };
    let zv = ed.b().zv();
    let end = ed.qr.as_ref().unwrap().end.map(|e| e.min(zv)).unwrap_or(zv);
    let r = if from > end {
        None
    } else {
        search_forward(ed, bid, &re, from, Some(end))
    };
    match r {
        Some((s, e)) => {
            let text = buffer_text(ed, bid);
            let b = ed.buf(bid);
            let sb = b.text.char_to_byte(s);
            let caps: Vec<Option<String>> = re
                .captures_at(&text, sb)
                .map(|c| {
                    c.iter()
                        .map(|m| m.map(|m| m.as_str().to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let q = ed.qr.as_mut().unwrap();
            q.cur = Some((s, e));
            q.caps = caps;
            let m = if q.query {
                Some(format!(
                    "Query replacing {} with {}: (? for help) ",
                    q.from, q.to
                ))
            } else {
                None
            };
            ed.goto(e);
            if m.is_some() {
                ed.message = m;
            }
        }
        None => qr_finish(ed),
    }
}

fn expand_replacement(to: &str, caps: &[Option<String>], regex: bool) -> String {
    if !regex {
        return to.to_string();
    }
    let mut out = String::new();
    let mut it = to.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('&') => out.push_str(caps.first().and_then(|c| c.as_deref()).unwrap_or("")),
                Some(d) if d.is_ascii_digit() => {
                    let i = d.to_digit(10).unwrap() as usize;
                    out.push_str(caps.get(i).and_then(|c| c.as_deref()).unwrap_or(""));
                }
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('\\') => out.push('\\'),
                Some(o) => out.push(o),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn qr_replace_current(ed: &mut Editor) {
    let q = ed.qr.as_ref().unwrap();
    let Some((s, e)) = q.cur else { return };
    let rep = expand_replacement(&q.to, &q.caps, q.regex);
    let n = rep.chars().count();
    let delta = n as i64 - (e - s) as i64;
    ed.delete(s, e);
    ed.bm().insert(s, &rep);
    ed.goto(s + n);
    let q = ed.qr.as_mut().unwrap();
    q.count += 1;
    if let Some(end) = q.end.as_mut() {
        *end = (*end as i64 + delta).max(0) as usize;
    }
}

fn qr_finish(ed: &mut Editor) {
    if let Some(q) = ed.qr.take() {
        ed.msg(format!(
            "Replaced {} occurrence{}",
            q.count,
            if q.count == 1 { "" } else { "s" }
        ));
    }
}

pub fn qr_key(ed: &mut Editor, key: Key) {
    let next_from = |ed: &Editor| {
        let p = ed.pt();
        match ed.qr.as_ref().and_then(|q| q.cur) {
            Some((s, e)) if s == e => p + 1,
            _ => p,
        }
    };
    match (key.k, key.ctrl, key.meta) {
        (K::Char('y'), false, false) | (K::Char(' '), false, false) => {
            qr_replace_current(ed);
            let f = next_from(ed);
            qr_next(ed, f);
        }
        (K::Char('n'), false, false) | (K::Backspace, false, false) => {
            let f = next_from(ed);
            qr_next(ed, f);
        }
        (K::Char('!'), false, false) => {
            while ed.qr.as_ref().map(|q| q.cur.is_some()).unwrap_or(false) {
                qr_replace_current(ed);
                let f = next_from(ed);
                qr_next(ed, f);
            }
        }
        (K::Char('.'), false, false) => {
            qr_replace_current(ed);
            qr_finish(ed);
        }
        (K::Char(','), false, false) => {
            qr_replace_current(ed);
            let p = ed.pt();
            let q = ed.qr.as_mut().unwrap();
            q.cur = Some((p, p));
            ed.message = Some("Replaced; type SPC or y to continue, n to skip".into());
        }
        (K::Char('^'), false, false) => {
            let o = ed.qr.as_ref().unwrap().origin;
            ed.goto(o);
        }
        (K::Char('?'), false, false) => {
            ed.message = Some(
                "SPC/y replace, DEL/n skip, RET/q exit, ! replace all, . replace & exit, , replace & stay, ^ back".into(),
            );
        }
        (K::Char('q'), false, false) | (K::Enter, false, false) | (K::Char('g'), true, false) => {
            qr_finish(ed)
        }
        _ => {
            qr_finish(ed);
            ed.handle_key(key);
        }
    }
}

// ======================= occur / highlight / next-error =======================

pub fn occur(ed: &mut Editor) {
    ed.read_string(
        "List lines matching regexp: ",
        PromptKind::Occur,
        "",
        Completion::None,
        "regexp",
    );
}

pub fn occur_run(ed: &mut Editor, pat: &str) {
    let re = match make_regex(pat, true, ed.cfg.case_fold_search) {
        Ok(r) => r,
        Err(e) => {
            ed.error(format!("Invalid regexp: {e}"));
            return;
        }
    };
    let src = ed.cur();
    let b = ed.buf(src);
    let mut lines = Vec::new();
    let mut entries = Vec::new();
    for l in 0..b.line_count() {
        let t = b.line_text(l);
        if re.is_match(&t) {
            lines.push(format!("{:>7}:{}", l + 1, t));
            entries.push(format!("{src}:{}", b.line_start(l)));
        }
    }
    let n = lines.len();
    let header = format!(
        "{n} match{} for \"{pat}\" in buffer: {}",
        if n == 1 { "" } else { "es" },
        b.name
    );
    let mut text = header;
    for l in &lines {
        text.push('\n');
        text.push_str(l);
    }
    text.push('\n');
    let mut ents = vec![String::new()];
    ents.extend(entries);
    let bid = crate::files::special_buffer(ed, "*Occur*", &text, BufKind::Special);
    ed.bufm(bid).entries = ents;
    ed.display_buffer_other(bid, false);
    ed.next_error = Some((bid, 0));
    if n == 0 {
        ed.msg("Searched 1 buffer; no matches");
    }
}

pub fn occur_follow(ed: &mut Editor) {
    let line = ed.b().line_of(ed.pt());
    let entry = ed.b().entries.get(line).cloned().unwrap_or_default();
    let cur = ed.cur();
    if jump_entry(ed, &entry) {
        ed.next_error = Some((cur, line));
    }
}

fn jump_entry(ed: &mut Editor, entry: &str) -> bool {
    let Some((b, p)) = entry.split_once(':') else {
        return false;
    };
    let (Ok(b), Ok(p)) = (b.parse::<usize>(), p.parse::<usize>()) else {
        return false;
    };
    if ed.bufs.get(b).map(|x| x.is_none()).unwrap_or(true) {
        ed.error("Buffer no longer exists");
        return false;
    }
    if ed.windows().iter().any(|&w| ed.win(w).buf == b) {
        let w = *ed.windows().iter().find(|&&w| ed.win(w).buf == b).unwrap();
        ed.sel = w;
    } else {
        ed.display_buffer_other(b, true);
    }
    ed.goto(p);
    true
}

fn step_error(ed: &mut Editor, dir: i64) {
    let Some((ob, line)) = ed.next_error else {
        ed.error("No next-error capable buffer found");
        return;
    };
    if ed.bufs.get(ob).map(|b| b.is_none()).unwrap_or(true) {
        ed.next_error = None;
        ed.error("No next-error capable buffer found");
        return;
    }
    let n = ed.buf(ob).entries.len();
    let mut l = line as i64 + dir;
    while l >= 0 && (l as usize) < n {
        if !ed.buf(ob).entries[l as usize].is_empty() {
            break;
        }
        l += dir;
    }
    if l < 0 || l as usize >= n {
        ed.error(if dir > 0 {
            "Moved past last match"
        } else {
            "Moved back before first match"
        });
        return;
    }
    let entry = ed.buf(ob).entries[l as usize].clone();
    // move point in occur buffer too
    for w in ed.windows() {
        if ed.win(w).buf == ob {
            let p = ed.buf(ob).line_start(l as usize);
            crate::view::set_win_point(ed, w, p);
        }
    }
    let p = ed.buf(ob).line_start(l as usize);
    let pm = ed.buf(ob).pt;
    ed.bufm(ob).set_mpos(pm, p);
    ed.next_error = Some((ob, l as usize));
    jump_entry(ed, &entry);
}

pub fn next_error(ed: &mut Editor) {
    step_error(ed, 1);
}
pub fn previous_error(ed: &mut Editor) {
    step_error(ed, -1);
}

pub fn highlight_regexp(ed: &mut Editor) {
    ed.read_string(
        "Regexp to highlight: ",
        PromptKind::HighlightRegexp,
        "",
        Completion::None,
        "regexp",
    );
}

pub fn highlight_apply(ed: &mut Editor, pat: &str) {
    match make_regex(pat, true, false) {
        Ok(r) => {
            let colors = ["yellow1", "pink", "green1", "lightblue", "salmon"];
            let c = colors[ed.highlights.len() % colors.len()];
            ed.highlights
                .push((r, crate::face::Style::fgbg("black", c)));
        }
        Err(e) => ed.error(format!("Invalid regexp: {e}")),
    }
}

pub fn unhighlight_regexp(ed: &mut Editor) {
    if ed.highlights.pop().is_none() {
        ed.error("No highlighting to remove");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn emacs_regex() {
        assert_eq!(emacs_to_rust_regex(r"\(foo\|bar\)"), "(foo|bar)");
        assert_eq!(emacs_to_rust_regex(r"a(b)"), r"a\(b\)");
        assert_eq!(emacs_to_rust_regex(r"\<w\>"), r"\bw\b");
        assert!(Regex::new(&emacs_to_rust_regex(r"^\s-*$")).is_ok());
    }
    #[test]
    fn replacement() {
        let caps = vec![Some("ab".to_string()), Some("a".to_string())];
        assert_eq!(expand_replacement(r"<\1|\&>", &caps, true), "<a|ab>");
    }
}
