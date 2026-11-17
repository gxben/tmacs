//! Text buffer: rope storage, markers, Emacs-style undo, narrowing.

use ropey::Rope;
use std::path::PathBuf;
use std::time::SystemTime;

pub type MarkerId = usize;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BufKind {
    Normal,
    Mini,
    BufferList,
    Dired,
    Special,
}

#[derive(Clone, Copy, Debug)]
pub struct Marker {
    pub pos: usize,
    /// Insertion-type: advance when text is inserted exactly at the marker.
    pub advance: bool,
}

#[derive(Clone, Debug)]
pub enum Change {
    Ins(usize, String),
    Del(usize, String),
}

#[derive(Clone, Debug)]
pub struct UndoGroup {
    pub changes: Vec<Change>,
    pub point: usize,
    pub unmodified_before: bool,
    /// Number of self-inserted chars amalgamated into this group.
    pub amalgamated: usize,
}

pub struct Buffer {
    pub name: String,
    pub path: Option<PathBuf>,
    pub dir: PathBuf,
    pub text: Rope,
    pub kind: BufKind,
    pub modified: bool,
    pub read_only: bool,
    pub crlf: bool,
    pub mtime: Option<SystemTime>,
    pub lang: usize,
    markers: Vec<Option<Marker>>,
    /// The buffer's own point (used when no window shows it).
    pub pt: MarkerId,
    /// The buffer's own window-start (restored when redisplayed).
    pub ws: MarkerId,
    pub mark: MarkerId,
    pub mark_set: bool,
    pub mark_active: bool,
    pub mark_ring: Vec<MarkerId>,
    pub narrow: Option<(MarkerId, MarkerId)>,
    pub undo: Vec<UndoGroup>,
    undo_cur: Vec<Change>,
    undo_cur_point: usize,
    undo_cur_unmod: bool,
    pub undo_disabled: bool,
    pub undo_ptr: Option<usize>,
    /// Point at the start of the current command (for undo records).
    pub cmd_point: usize,
    /// First line whose syntax state is stale.
    pub dirty_line: usize,
    /// An edit happened during this command (deactivates the mark).
    pub deactivate: bool,
    pub overwrite: bool,
    /// Entries for special buffers (buffer list, dired): one per line.
    pub entries: Vec<String>,
    pub change_tick: u64,
    pub syntax: Option<Box<crate::syntax::Cache>>,
    pub indent_override: Option<usize>,
}

impl Buffer {
    pub fn new(name: &str, dir: PathBuf) -> Buffer {
        let mut b = Buffer {
            name: name.to_string(),
            path: None,
            dir,
            text: Rope::new(),
            kind: BufKind::Normal,
            modified: false,
            read_only: false,
            crlf: false,
            mtime: None,
            lang: 0,
            markers: Vec::new(),
            pt: 0,
            ws: 0,
            mark: 0,
            mark_set: false,
            mark_active: false,
            mark_ring: Vec::new(),
            narrow: None,
            undo: Vec::new(),
            undo_cur: Vec::new(),
            undo_cur_point: 0,
            undo_cur_unmod: true,
            undo_disabled: false,
            undo_ptr: None,
            cmd_point: 0,
            dirty_line: 0,
            deactivate: false,
            overwrite: false,
            entries: Vec::new(),
            change_tick: 0,
            syntax: None,
            indent_override: None,
        };
        b.pt = b.new_marker(0, false);
        b.ws = b.new_marker(0, false);
        b.mark = b.new_marker(0, false);
        b
    }

    // ---------- markers ----------

    pub fn new_marker(&mut self, pos: usize, advance: bool) -> MarkerId {
        let m = Marker { pos, advance };
        if let Some(i) = self.markers.iter().position(|m| m.is_none()) {
            self.markers[i] = Some(m);
            i
        } else {
            self.markers.push(Some(m));
            self.markers.len() - 1
        }
    }

    pub fn free_marker(&mut self, id: MarkerId) {
        if id < self.markers.len() {
            self.markers[id] = None;
        }
    }

    pub fn mpos(&self, id: MarkerId) -> usize {
        self.markers
            .get(id)
            .and_then(|m| *m)
            .map(|m| m.pos)
            .unwrap_or(0)
            .min(self.len())
    }

    pub fn set_mpos(&mut self, id: MarkerId, pos: usize) {
        let pos = pos.min(self.len());
        if let Some(Some(m)) = self.markers.get_mut(id) {
            m.pos = pos;
        }
    }

    pub fn mark_pos(&self) -> Option<usize> {
        if self.mark_set {
            Some(self.mpos(self.mark))
        } else {
            None
        }
    }

    /// Active region bounds (transient-mark-mode).
    pub fn region(&self, point: usize) -> Option<(usize, usize)> {
        if self.mark_active {
            let m = self.mpos(self.mark);
            Some((m.min(point), m.max(point)))
        } else {
            None
        }
    }

    /// Push current mark onto the ring and set a new one.
    pub fn push_mark(&mut self, pos: usize, activate: bool) {
        if self.mark_set {
            let old = self.mpos(self.mark);
            let id = self.new_marker(old, false);
            self.mark_ring.insert(0, id);
            if self.mark_ring.len() > 16 {
                let id = self.mark_ring.pop().unwrap();
                self.free_marker(id);
            }
        }
        self.set_mpos(self.mark, pos);
        self.mark_set = true;
        self.mark_active = activate;
    }

    // ---------- text access ----------

    pub fn len(&self) -> usize {
        self.text.len_chars()
    }

    /// Accessible-region start (narrowing).
    pub fn begv(&self) -> usize {
        self.narrow.map(|(a, _)| self.mpos(a)).unwrap_or(0)
    }

    /// Accessible-region end (narrowing).
    pub fn zv(&self) -> usize {
        self.narrow.map(|(_, b)| self.mpos(b)).unwrap_or(self.len())
    }

    pub fn clamp(&self, pos: usize) -> usize {
        pos.max(self.begv()).min(self.zv())
    }

    pub fn char_at(&self, pos: usize) -> Option<char> {
        if pos < self.len() {
            Some(self.text.char(pos))
        } else {
            None
        }
    }

    pub fn char_before(&self, pos: usize) -> Option<char> {
        if pos > 0 && pos <= self.len() {
            Some(self.text.char(pos - 1))
        } else {
            None
        }
    }

    pub fn slice(&self, a: usize, b: usize) -> String {
        let (a, b) = (a.min(self.len()), b.min(self.len()));
        self.text.slice(a.min(b)..b.max(a)).to_string()
    }

    pub fn line_of(&self, pos: usize) -> usize {
        self.text.char_to_line(pos.min(self.len()))
    }

    pub fn line_count(&self) -> usize {
        self.text.len_lines()
    }

    pub fn line_start(&self, line: usize) -> usize {
        if line >= self.text.len_lines() {
            return self.len();
        }
        self.text.line_to_char(line)
    }

    /// Position of end of `line` (before the newline).
    pub fn line_end(&self, line: usize) -> usize {
        if line + 1 >= self.text.len_lines() {
            return self.len();
        }
        self.text.line_to_char(line + 1) - 1
    }

    pub fn bol(&self, pos: usize) -> usize {
        self.line_start(self.line_of(pos)).max(self.begv())
    }

    pub fn eol(&self, pos: usize) -> usize {
        self.line_end(self.line_of(pos)).min(self.zv())
    }

    /// Text of a line without the trailing newline.
    pub fn line_text(&self, line: usize) -> String {
        self.slice(self.line_start(line), self.line_end(line))
    }

    pub fn column(&self, pos: usize, tab_width: usize) -> usize {
        let start = self.bol(pos);
        let mut col = 0;
        for c in self.text.slice(start..pos).chars() {
            col = crate::text::advance_col(col, c, tab_width);
        }
        col
    }

    /// Position on the line of `pos` at display column `col` (or line end).
    pub fn pos_at_column(&self, pos: usize, target: usize, tab_width: usize) -> usize {
        let mut p = self.bol(pos);
        let end = self.eol(pos);
        let mut col = 0;
        while p < end {
            let c = self.text.char(p);
            let n = crate::text::advance_col(col, c, tab_width);
            if n > target {
                break;
            }
            col = n;
            p += 1;
        }
        p
    }

    // ---------- modification ----------

    fn note_change(&mut self, pos: usize) {
        if self.undo_cur.is_empty() && !self.undo_disabled {
            self.undo_cur_point = self.cmd_point;
            self.undo_cur_unmod = !self.modified;
        }
        let line = self.line_of(pos);
        self.dirty_line = self.dirty_line.min(line);
        self.modified = true;
        self.deactivate = true;
        self.change_tick += 1;
    }

    pub fn insert(&mut self, pos: usize, s: &str) {
        if s.is_empty() {
            return;
        }
        let pos = pos.min(self.len());
        self.note_change(pos);
        self.text.insert(pos, s);
        let n = s.chars().count();
        for m in self.markers.iter_mut().flatten() {
            if m.pos > pos || (m.pos == pos && m.advance) {
                m.pos += n;
            }
        }
        if !self.undo_disabled {
            if let Some(Change::Ins(p, t)) = self.undo_cur.last_mut() {
                if *p + t.chars().count() == pos {
                    t.push_str(s);
                    return;
                }
            }
            self.undo_cur.push(Change::Ins(pos, s.to_string()));
        }
    }

    pub fn delete(&mut self, a: usize, b: usize) -> String {
        let (a, b) = (a.min(b).min(self.len()), b.max(a).min(self.len()));
        if a == b {
            return String::new();
        }
        self.note_change(a);
        let s = self.text.slice(a..b).to_string();
        self.text.remove(a..b);
        let n = b - a;
        for m in self.markers.iter_mut().flatten() {
            if m.pos >= b {
                m.pos -= n;
            } else if m.pos > a {
                m.pos = a;
            }
        }
        if !self.undo_disabled {
            // Merge consecutive backward deletions (DEL DEL DEL).
            if let Some(Change::Del(p, t)) = self.undo_cur.last_mut() {
                if b == *p {
                    *p = a;
                    t.insert_str(0, &s);
                    return s;
                }
                if a == *p {
                    t.push_str(&s);
                    return s;
                }
            }
            self.undo_cur.push(Change::Del(a, s.clone()));
        }
        s
    }

    pub fn replace(&mut self, a: usize, b: usize, s: &str) {
        self.delete(a, b);
        self.insert(a, s);
    }

    /// Replace whole text keeping markers roughly in place (used by formatters).
    pub fn replace_all_minimal(&mut self, new: &str) {
        let old = self.text.to_string();
        if old == new {
            return;
        }
        // Trim common prefix/suffix (by chars) to keep the edit small.
        let oc: Vec<char> = old.chars().collect();
        let nc: Vec<char> = new.chars().collect();
        let mut pre = 0;
        while pre < oc.len() && pre < nc.len() && oc[pre] == nc[pre] {
            pre += 1;
        }
        let mut suf = 0;
        while suf < oc.len() - pre
            && suf < nc.len() - pre
            && oc[oc.len() - 1 - suf] == nc[nc.len() - 1 - suf]
        {
            suf += 1;
        }
        let mid: String = nc[pre..nc.len() - suf].iter().collect();
        self.delete(pre, oc.len() - suf);
        self.insert(pre, &mid);
    }

    pub fn set_text(&mut self, s: &str) {
        let n = self.len();
        self.delete(0, n);
        self.insert(0, s);
    }

    // ---------- undo ----------

    /// Close the current undo group. `amalg`: 0 = normal, 1 = start an
    /// amalgamating (self-insert) group, 2 = continue it (max 20 chars).
    pub fn undo_boundary(&mut self, amalg: u8) {
        if self.undo_cur.is_empty() {
            return;
        }
        let changes = std::mem::take(&mut self.undo_cur);
        if amalg == 2 {
            if let Some(last) = self.undo.last_mut() {
                if last.amalgamated > 0 && last.amalgamated < 20 {
                    for c in changes {
                        match (last.changes.last_mut(), &c) {
                            (Some(Change::Ins(p, t)), Change::Ins(p2, t2))
                                if *p + t.chars().count() == *p2 =>
                            {
                                t.push_str(t2)
                            }
                            _ => last.changes.push(c),
                        }
                    }
                    last.amalgamated += 1;
                    return;
                }
            }
        }
        self.undo.push(UndoGroup {
            changes,
            point: self.undo_cur_point,
            unmodified_before: self.undo_cur_unmod,
            amalgamated: if amalg > 0 { 1 } else { 0 },
        });
        if self.undo.len() > 4000 {
            self.undo.drain(..1000);
            self.undo_ptr = None;
        }
    }

    /// Undo one group. `continuing` = previous command was undo.
    pub fn undo_step(&mut self, continuing: bool) -> Result<usize, &'static str> {
        self.undo_boundary(0);
        let ptr = match (continuing, self.undo_ptr) {
            (true, Some(p)) => p,
            _ => self.undo.len(),
        };
        if ptr == 0 {
            self.undo_ptr = Some(0);
            return Err("No further undo information");
        }
        let g = self.undo[ptr - 1].clone();
        self.undo_ptr = Some(ptr - 1);
        for c in g.changes.iter().rev() {
            match c {
                Change::Ins(p, t) => {
                    self.delete(*p, *p + t.chars().count());
                }
                Change::Del(p, t) => {
                    self.insert(*p, t);
                }
            }
        }
        self.undo_boundary(0);
        if g.unmodified_before {
            self.modified = false;
        }
        Ok(g.point.min(self.len()))
    }

    pub fn mark_saved(&mut self) {
        self.modified = false;
        for g in &mut self.undo {
            g.unmodified_before = false;
        }
        self.undo_cur_unmod = true;
    }

    pub fn clear_undo(&mut self) {
        self.undo.clear();
        self.undo_cur.clear();
        self.undo_ptr = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(s: &str) -> Buffer {
        let mut b = Buffer::new("t", PathBuf::from("/"));
        b.undo_disabled = true;
        b.insert(0, s);
        b.undo_disabled = false;
        b.modified = false;
        b
    }

    #[test]
    fn markers_shift() {
        let mut b = buf("hello world");
        let m = b.new_marker(6, false);
        b.insert(0, "XX");
        assert_eq!(b.mpos(m), 8);
        b.insert(8, "YY");
        assert_eq!(b.mpos(m), 8);
        b.delete(0, 10);
        assert_eq!(b.mpos(m), 0);
    }

    #[test]
    fn undo_redo_emacs_style() {
        let mut b = buf("abc");
        b.insert(3, "d");
        b.undo_boundary(0);
        b.insert(4, "e");
        b.undo_boundary(0);
        assert_eq!(b.text.to_string(), "abcde");
        b.undo_step(false).unwrap();
        assert_eq!(b.text.to_string(), "abcd");
        b.undo_step(true).unwrap();
        assert_eq!(b.text.to_string(), "abc");
        assert!(!b.modified);
        // Break the chain: next undo redoes.
        b.undo_step(false).unwrap();
        assert_eq!(b.text.to_string(), "abcd");
    }

    #[test]
    fn amalgamate_inserts() {
        let mut b = buf("");
        for (i, c) in "hello".chars().enumerate() {
            b.insert(i, &c.to_string());
            b.undo_boundary(if i == 0 { 1 } else { 2 });
        }
        assert_eq!(b.undo.len(), 1);
        b.undo_step(false).unwrap();
        assert_eq!(b.text.to_string(), "");
    }

    #[test]
    fn minimal_replace() {
        let mut b = buf("a\nb  \nc\n");
        let m = b.new_marker(6, false);
        b.replace_all_minimal("a\nb\nc\n");
        assert_eq!(b.text.to_string(), "a\nb\nc\n");
        assert_eq!(b.mpos(m), 4);
    }
}
