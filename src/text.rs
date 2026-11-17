// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Benjamin Zores

//! Text helpers: display widths, word/sexp/paragraph motion, visual rows.

use crate::buffer::Buffer;
use unicode_width::UnicodeWidthChar;

pub fn char_width(c: char) -> usize {
    if (c as u32) < 0x20 || c == '\u{7f}' {
        2 // ^X
    } else {
        c.width().unwrap_or(1)
    }
}

pub fn advance_col(col: usize, c: char, tab_width: usize) -> usize {
    if c == '\t' {
        let tw = tab_width.max(1);
        (col / tw + 1) * tw
    } else {
        col + char_width(c)
    }
}

pub fn is_word(c: char) -> bool {
    c.is_alphanumeric()
}

pub fn forward_word(b: &Buffer, mut p: usize) -> usize {
    let end = b.zv();
    while p < end && !is_word(b.text.char(p)) {
        p += 1;
    }
    while p < end && is_word(b.text.char(p)) {
        p += 1;
    }
    p
}

pub fn backward_word(b: &Buffer, mut p: usize) -> usize {
    let start = b.begv();
    while p > start && !is_word(b.text.char(p - 1)) {
        p -= 1;
    }
    while p > start && is_word(b.text.char(p - 1)) {
        p -= 1;
    }
    p
}

fn is_blank_line(b: &Buffer, line: usize) -> bool {
    let s = b.line_start(line);
    let e = b.line_end(line);
    b.text.slice(s..e).chars().all(|c| c == ' ' || c == '\t')
}

pub fn forward_paragraph(b: &Buffer, p: usize) -> usize {
    let last = b.line_of(b.zv());
    let mut l = b.line_of(p);
    while l <= last && is_blank_line(b, l) {
        l += 1;
    }
    while l <= last && !is_blank_line(b, l) {
        l += 1;
    }
    if l > last {
        b.zv()
    } else {
        b.line_start(l).min(b.zv())
    }
}

pub fn backward_paragraph(b: &Buffer, p: usize) -> usize {
    let first = b.line_of(b.begv());
    let mut l = b.line_of(p);
    if l > first && b.line_start(l) == p {
        l -= 1;
    }
    while l > first && is_blank_line(b, l) {
        l -= 1;
    }
    while l > first && !is_blank_line(b, l) {
        l -= 1;
    }
    b.line_start(l).max(b.begv())
}

pub fn forward_sentence(b: &Buffer, mut p: usize) -> usize {
    let end = b.zv();
    let para_end = forward_paragraph(b, p);
    while p < end && p < para_end {
        let c = b.text.char(p);
        p += 1;
        if matches!(c, '.' | '?' | '!') {
            // sentence-end-double-space nil: one space or EOL ends it
            while p < end && matches!(b.text.char(p), '"' | '\'' | ')' | ']') {
                p += 1;
            }
            if p >= end || matches!(b.text.char(p), ' ' | '\n' | '\t') {
                return p;
            }
        }
    }
    let mut q = para_end;
    while q > p.min(para_end) && q > 0 && b.text.char(q - 1).is_whitespace() {
        q -= 1;
    }
    q.max(p.min(para_end))
}

pub fn backward_sentence(b: &Buffer, p: usize) -> usize {
    let start = b.begv();
    let para = backward_paragraph(b, p);
    let mut q = p;
    // skip whitespace backward
    while q > start && b.text.char(q - 1).is_whitespace() {
        q -= 1;
    }
    while q > para.max(start) {
        let c = b.text.char(q - 1);
        if matches!(c, '.' | '?' | '!')
            && q < p
            && b.char_at(q).map(|c| c.is_whitespace()).unwrap_or(true)
        {
            let mut r = q;
            while r < p && b.text.char(r).is_whitespace() {
                r += 1;
            }
            if r < p {
                return r;
            }
        }
        q -= 1;
    }
    let mut r = para.max(start);
    while r < p && b.text.char(r).is_whitespace() {
        r += 1;
    }
    r
}

pub fn open_of(c: char) -> Option<char> {
    match c {
        ')' => Some('('),
        ']' => Some('['),
        '}' => Some('{'),
        _ => None,
    }
}

pub fn close_of(c: char) -> Option<char> {
    match c {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        _ => None,
    }
}

/// Find matching bracket for the bracket at `p`; skips over string literals
/// naively. Returns None if unbalanced within `limit` chars.
pub fn match_paren(b: &Buffer, p: usize, limit: usize) -> Option<usize> {
    let c = b.char_at(p)?;
    if let Some(close) = close_of(c) {
        let mut depth = 0i32;
        let end = (p + limit).min(b.len());
        let mut q = p;
        while q < end {
            let d = b.text.char(q);
            if d == c {
                depth += 1;
            } else if d == close {
                depth -= 1;
                if depth == 0 {
                    return Some(q);
                }
            } else if open_of(d).is_some() || close_of(d).is_some() {
                // other bracket kinds: tracked loosely
            }
            q += 1;
        }
        None
    } else if let Some(open) = open_of(c) {
        let mut depth = 0i32;
        let start = p.saturating_sub(limit);
        let mut q = p + 1;
        while q > start {
            q -= 1;
            let d = b.text.char(q);
            if d == c {
                depth += 1;
            } else if d == open {
                depth -= 1;
                if depth == 0 {
                    return Some(q);
                }
            }
        }
        None
    } else {
        None
    }
}

/// Bracket-aware matching used by show-paren (checks mismatched kinds).
pub fn scan_pair(b: &Buffer, p: usize, forward: bool, limit: usize) -> Option<(usize, bool)> {
    let mut stack: Vec<char> = Vec::new();
    if forward {
        let end = (p + limit).min(b.len());
        let mut q = p;
        while q < end {
            let d = b.text.char(q);
            if close_of(d).is_some() {
                stack.push(d);
            } else if let Some(o) = open_of(d) {
                let top = stack.pop()?;
                if stack.is_empty() {
                    return Some((q, top == o));
                }
            }
            q += 1;
        }
    } else {
        let start = p.saturating_sub(limit);
        let mut q = p;
        while q > start {
            q -= 1;
            let d = b.text.char(q);
            if open_of(d).is_some() {
                stack.push(d);
            } else if let Some(c) = close_of(d) {
                let top = stack.pop()?;
                if stack.is_empty() {
                    return Some((q, top == c));
                }
            }
        }
    }
    None
}

fn is_sexp_char(c: char) -> bool {
    c.is_alphanumeric()
        || matches!(
            c,
            '_' | '-'
                | '.'
                | '$'
                | '@'
                | '!'
                | '?'
                | '*'
                | '+'
                | '/'
                | '<'
                | '>'
                | '='
                | '&'
                | '%'
                | ':'
        )
}

pub fn forward_sexp(b: &Buffer, mut p: usize) -> Option<usize> {
    let end = b.zv();
    while p < end && (b.text.char(p).is_whitespace() || matches!(b.text.char(p), ',' | ';')) {
        p += 1;
    }
    if p >= end {
        return None;
    }
    let c = b.text.char(p);
    if close_of(c).is_some() {
        return match_paren(b, p, 1_000_000).map(|q| q + 1);
    }
    if open_of(c).is_some() {
        return None;
    }
    if c == '"' || c == '\'' || c == '`' {
        let mut q = p + 1;
        while q < end {
            let d = b.text.char(q);
            if d == '\\' {
                q += 2;
                continue;
            }
            if d == c {
                return Some(q + 1);
            }
            q += 1;
        }
        return Some(end);
    }
    if !is_sexp_char(c) {
        return Some(p + 1);
    }
    while p < end && is_sexp_char(b.text.char(p)) {
        p += 1;
    }
    Some(p)
}

pub fn backward_sexp(b: &Buffer, mut p: usize) -> Option<usize> {
    let start = b.begv();
    while p > start
        && (b.text.char(p - 1).is_whitespace() || matches!(b.text.char(p - 1), ',' | ';'))
    {
        p -= 1;
    }
    if p <= start {
        return None;
    }
    let c = b.text.char(p - 1);
    if open_of(c).is_some() {
        return match_paren(b, p - 1, 1_000_000);
    }
    if close_of(c).is_some() {
        return None;
    }
    if c == '"' || c == '\'' || c == '`' {
        let mut q = p - 1;
        while q > start {
            q -= 1;
            if b.text.char(q) == c && (q == 0 || b.text.char(q - 1) != '\\') {
                return Some(q);
            }
        }
        return Some(start);
    }
    if !is_sexp_char(c) {
        return Some(p - 1);
    }
    while p > start && is_sexp_char(b.text.char(p - 1)) {
        p -= 1;
    }
    Some(p)
}

/// Split a logical line (as chars) into visual rows for a given text width.
/// Returns the char offsets at which each row starts (first is always 0).
pub fn wrap_rows(chars: &[char], width: usize, tab_width: usize) -> Vec<usize> {
    let mut rows = vec![0];
    if width == 0 {
        return rows;
    }
    let mut col = 0;
    for (i, &c) in chars.iter().enumerate() {
        let w = if c == '\t' {
            advance_col(col, c, tab_width) - col
        } else {
            char_width(c)
        };
        if col + w > width && col > 0 {
            rows.push(i);
            col = 0;
            let w = if c == '\t' {
                advance_col(0, c, tab_width)
            } else {
                char_width(c)
            };
            col += w;
        } else {
            col += w;
        }
    }
    rows
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

    #[test]
    fn words() {
        let b = buf("foo_bar  baz");
        assert_eq!(forward_word(&b, 0), 3);
        assert_eq!(forward_word(&b, 3), 7);
        assert_eq!(backward_word(&b, 12), 9);
    }

    #[test]
    fn parens() {
        let b = buf("(a [b] {c})");
        assert_eq!(match_paren(&b, 0, 100), Some(10));
        assert_eq!(match_paren(&b, 10, 100), Some(0));
        assert_eq!(forward_sexp(&b, 0), Some(11));
    }

    #[test]
    fn wrap() {
        let c: Vec<char> = "abcdefghij".chars().collect();
        assert_eq!(wrap_rows(&c, 4, 8), vec![0, 4, 8]);
        assert_eq!(wrap_rows(&c, 10, 8), vec![0]);
    }

    #[test]
    fn paragraphs() {
        let b = buf("a\nb\n\nc\nd\n");
        assert_eq!(forward_paragraph(&b, 0), 4);
        assert_eq!(backward_paragraph(&b, 8), 4);
    }
}
