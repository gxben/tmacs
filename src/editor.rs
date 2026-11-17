//! Editor state, command loop, windows, minibuffer and prompts.

use crate::buffer::{BufKind, Buffer, MarkerId};
use crate::config::{Config, Settings};
use crate::face::Faces;
use crate::key::{K, Key, seq_to_string};
use crate::keymap::{self, Keymap, Lookup};
use crate::lang::{Lang, Langs};
use crate::search::{ISearch, QueryReplace};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub type BufId = usize;
pub type WinId = usize;
pub const MINI: WinId = 0;

pub struct Window {
    pub buf: BufId,
    pub pt: MarkerId,
    pub start: MarkerId,
    pub start_row: usize,
    pub hscroll: usize,
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
    /// Screen row -> (start pos, end pos) of the buffer text shown there.
    pub rows: Vec<Option<(usize, usize)>>,
    pub goal: Option<usize>,
}

#[derive(Clone, Debug)]
pub enum Layout {
    Leaf(WinId),
    Split {
        side: bool,
        ratio: f32,
        a: Box<Layout>,
        b: Box<Layout>,
    },
}

impl Layout {
    pub fn leaves(&self, out: &mut Vec<WinId>) {
        match self {
            Layout::Leaf(w) => out.push(*w),
            Layout::Split { a, b, .. } => {
                a.leaves(out);
                b.leaves(out);
            }
        }
    }

    fn split(&mut self, target: WinId, new: WinId, side: bool) -> bool {
        match self {
            Layout::Leaf(w) if *w == target => {
                *self = Layout::Split {
                    side,
                    ratio: 0.5,
                    a: Box::new(Layout::Leaf(target)),
                    b: Box::new(Layout::Leaf(new)),
                };
                true
            }
            Layout::Leaf(_) => false,
            Layout::Split { a, b, .. } => a.split(target, new, side) || b.split(target, new, side),
        }
    }

    fn remove(&mut self, target: WinId) -> bool {
        if let Layout::Split { a, b, .. } = self {
            if matches!(**a, Layout::Leaf(w) if w == target) {
                *self = (**b).clone();
                return true;
            }
            if matches!(**b, Layout::Leaf(w) if w == target) {
                *self = (**a).clone();
                return true;
            }
            return a.remove(target) || b.remove(target);
        }
        false
    }

    /// Adjust the ratio of the split directly containing `target`.
    pub fn resize(&mut self, target: WinId, side: bool, delta: f32) -> bool {
        if let Layout::Split {
            side: s,
            ratio,
            a,
            b,
        } = self
        {
            let in_a = {
                let mut v = Vec::new();
                a.leaves(&mut v);
                v.contains(&target)
            };
            let deeper = if in_a {
                a.resize(target, side, delta)
            } else {
                b.resize(target, side, delta)
            };
            if deeper {
                return true;
            }
            if *s == side {
                *ratio = (*ratio + if in_a { delta } else { -delta }).clamp(0.1, 0.9);
                return true;
            }
        }
        false
    }

    pub fn balance(&mut self) {
        if let Layout::Split { ratio, a, b, .. } = self {
            *ratio = 0.5;
            a.balance();
            b.balance();
        }
    }
}

#[derive(Clone, Debug)]
pub enum PromptKind {
    FindFile {
        other: bool,
        read_only: bool,
    },
    FindAlternate,
    InsertFile,
    WriteFile,
    SwitchBuffer {
        other: bool,
    },
    KillBuffer,
    MX,
    GotoLine,
    QueryReplaceFrom {
        regex: bool,
        query: bool,
    },
    QueryReplaceTo {
        regex: bool,
        query: bool,
        from: String,
    },
    ShellCommand,
    ShellCommandRegion {
        replace: bool,
    },
    StringRectangle,
    RenameBuffer,
    Dired {
        other: bool,
    },
    DescribeFunction,
    WhereIs,
    Occur,
    HighlightRegexp,
    FlushLines {
        keep: bool,
    },
    GotoChar,
    DiredCreateDir,
    DiredRename(PathBuf),
    DiredCopy(PathBuf),
    BookmarkSet,
    BookmarkJump,
}

#[derive(Clone, Debug)]
pub enum Completion {
    None,
    File,
    Buffer,
    Command,
    List(Vec<String>),
}

pub struct Mini {
    pub prompt: String,
    pub kind: PromptKind,
    pub comp: Completion,
    pub hist: &'static str,
    pub hist_idx: Option<usize>,
    pub ret_win: WinId,
    pub default: Option<String>,
    pub saved_input: String,
}

#[derive(Clone, Debug)]
pub enum YnAction {
    KillBuffer(BufId),
    SaveAnyway(BufId),
    ExitSave(Vec<BufId>),
    ExitAnyway,
    SaveSome(Vec<BufId>),
    Revert(BufId),
    DiredDelete(Vec<PathBuf>),
    MakeDirs(PathBuf, bool),
}

#[derive(Clone, Debug)]
pub enum RegOp {
    CopyTo,
    Insert,
    PointTo,
    Jump,
}

#[derive(Clone, Debug)]
pub enum ReadKey {
    YesNo(YnAction, String),
    DescribeKey(Vec<Key>, bool),
    ZapToChar,
    QuotedInsert,
    Register(RegOp),
    IndentRigidly,
}

#[derive(Clone, Debug)]
pub enum Register {
    Text(String),
    Pos(BufId, MarkerId),
}

pub struct Editor {
    pub bufs: Vec<Option<Buffer>>,
    pub buf_order: Vec<BufId>,
    pub wins: Vec<Option<Window>>,
    pub layout: Layout,
    pub sel: WinId,
    pub mini_buf: BufId,
    pub cfg: Settings,
    pub faces: Faces,
    pub langs: Langs,
    pub global: Keymap,
    pub mini_map: Keymap,
    pub buflist_map: Keymap,
    pub dired_map: Keymap,
    pub special_map: Keymap,
    pub kill_ring: Vec<String>,
    pub kill_idx: usize,
    pub yank_range: Option<(usize, usize)>,
    pub message: Option<String>,
    pub messages: Vec<String>,
    pub mini: Option<Mini>,
    pub read_key: Option<ReadKey>,
    pub isearch: Option<ISearch>,
    pub qr: Option<QueryReplace>,
    pub completions: Option<Vec<String>>,
    pub arg: Option<i64>,
    pub arg_universal: bool,
    pub arg_digits: bool,
    pub keep_arg: bool,
    pub last_cmd: String,
    pub this_cmd: String,
    pub last_repeatable: Option<(String, Option<i64>)>,
    pub prefix: Vec<Key>,
    pub prefix_time: Option<Instant>,
    pub meta_pending: bool,
    pub macro_rec: Option<Vec<Key>>,
    pub last_macro: Option<Vec<Key>>,
    pub macro_counter: i64,
    pub executing_macro: bool,
    pub quit: bool,
    pub suspend: bool,
    pub bell: Option<Instant>,
    pub errored: bool,
    pub width: u16,
    pub height: u16,
    pub hist: HashMap<&'static str, Vec<String>>,
    pub search_ring: Vec<String>,
    pub regex_search_ring: Vec<String>,
    pub replace_hist: Vec<(String, String)>,
    pub dabbrev: Option<crate::commands::Dabbrev>,
    pub registers: HashMap<char, Register>,
    pub bookmarks: HashMap<String, (PathBuf, usize)>,
    pub this_kill: bool,
    pub last_kill: bool,
    pub this_yank: bool,
    pub last_yank: bool,
    pub highlights: Vec<(regex::Regex, crate::face::Style)>,
    pub text_cache: Option<(BufId, u64, std::rc::Rc<String>)>,
    pub redraw_all: bool,
    pub recenter_cycle: usize,
    pub next_error: Option<(BufId, usize)>,
    pub init_error: Option<String>,
    pub clipboard: crate::clipboard::Clipboard,
    /// Last text exchanged with the system clipboard.
    pub clip_last: Option<String>,
    /// Key that repeats the last command (z after C-x z, e after C-x e).
    pub repeat_key: Option<(Key, String)>,
    pub last_insert: Option<char>,
}

pub type Cmd = fn(&mut Editor);

impl Editor {
    pub fn new(config: Config, init_error: Option<String>) -> Editor {
        let mut faces = Faces::default();
        let mut errors: Vec<String> = init_error.into_iter().collect();
        for (name, spec) in &config.faces {
            match (faces.get_mut(name), crate::face::style_from_table(spec)) {
                (Some(f), Ok(s)) => *f = f.over(s),
                (None, _) => errors.push(format!("unknown face {name}")),
                (_, Err(e)) => errors.push(format!("face {name}: {e}")),
            }
        }
        let mut langs = Langs::new();
        errors.extend(langs.apply_config(&config.language));
        let mut global = keymap::build(keymap::GLOBAL);
        for (k, c) in &config.keys {
            if !global.bind_str(k, c) {
                errors.push(format!("bad key sequence {k:?}"));
            }
        }
        let cwd = std::env::current_dir().unwrap_or_default();
        let mut ed = Editor {
            bufs: Vec::new(),
            buf_order: Vec::new(),
            wins: Vec::new(),
            layout: Layout::Leaf(1),
            sel: 1,
            mini_buf: 0,
            cfg: config.settings,
            faces,
            langs,
            global,
            mini_map: keymap::build(keymap::MINIBUFFER),
            buflist_map: keymap::build(keymap::BUFFER_LIST),
            dired_map: keymap::build(keymap::DIRED),
            special_map: keymap::build(keymap::SPECIAL),
            kill_ring: Vec::new(),
            kill_idx: 0,
            yank_range: None,
            message: None,
            messages: Vec::new(),
            mini: None,
            read_key: None,
            isearch: None,
            qr: None,
            completions: None,
            arg: None,
            arg_universal: false,
            arg_digits: false,
            keep_arg: false,
            last_cmd: String::new(),
            this_cmd: String::new(),
            last_repeatable: None,
            prefix: Vec::new(),
            prefix_time: None,
            meta_pending: false,
            macro_rec: None,
            last_macro: None,
            macro_counter: 0,
            executing_macro: false,
            quit: false,
            suspend: false,
            bell: None,
            errored: false,
            width: 80,
            height: 24,
            hist: HashMap::new(),
            search_ring: Vec::new(),
            regex_search_ring: Vec::new(),
            replace_hist: Vec::new(),
            dabbrev: None,
            registers: HashMap::new(),
            bookmarks: HashMap::new(),
            this_kill: false,
            last_kill: false,
            this_yank: false,
            last_yank: false,
            highlights: Vec::new(),
            text_cache: None,
            redraw_all: false,
            recenter_cycle: 0,
            next_error: None,
            init_error: None,
            clipboard: crate::clipboard::Clipboard::None,
            clip_last: None,
            repeat_key: None,
            last_insert: None,
        };
        ed.clipboard = crate::clipboard::Clipboard::from_setting(&ed.cfg.clipboard);
        let mut mb = Buffer::new(" *Minibuf-0*", cwd.clone());
        mb.kind = BufKind::Mini;
        ed.bufs.push(Some(mb));
        ed.mini_buf = 0;
        let pt = ed.bufs[0].as_mut().unwrap().new_marker(0, false);
        let st = ed.bufs[0].as_mut().unwrap().new_marker(0, false);
        ed.wins.push(Some(Window::blank(0, pt, st)));
        let scratch = ed.add_buffer(Buffer::new("*scratch*", cwd));
        ed.wins.push(None);
        ed.attach_window(1, scratch);
        if !errors.is_empty() {
            ed.init_error = Some(errors.join("; "));
        }
        ed
    }

    // ---------------- accessors ----------------

    pub fn buf(&self, id: BufId) -> &Buffer {
        self.bufs[id].as_ref().expect("dead buffer")
    }
    pub fn bufm(&mut self, id: BufId) -> &mut Buffer {
        self.bufs[id].as_mut().expect("dead buffer")
    }
    pub fn win(&self, id: WinId) -> &Window {
        self.wins[id].as_ref().expect("dead window")
    }
    pub fn winm(&mut self, id: WinId) -> &mut Window {
        self.wins[id].as_mut().expect("dead window")
    }
    pub fn cur(&self) -> BufId {
        self.win(self.sel).buf
    }
    pub fn b(&self) -> &Buffer {
        self.buf(self.cur())
    }
    pub fn bm(&mut self) -> &mut Buffer {
        let id = self.cur();
        self.bufm(id)
    }
    pub fn w(&self) -> &Window {
        self.win(self.sel)
    }
    pub fn wm(&mut self) -> &mut Window {
        let s = self.sel;
        self.winm(s)
    }
    pub fn pt(&self) -> usize {
        let w = self.w();
        self.buf(w.buf).mpos(w.pt)
    }
    pub fn win_pt(&self, wid: WinId) -> usize {
        let w = self.win(wid);
        self.buf(w.buf).mpos(w.pt)
    }
    pub fn goto(&mut self, p: usize) {
        let (bid, m) = (self.w().buf, self.w().pt);
        let b = self.bufm(bid);
        let p = b.clamp(p);
        b.set_mpos(m, p);
    }
    pub fn lang(&self) -> &Lang {
        &self.langs.list[self.b().lang]
    }
    pub fn tab_width(&self) -> usize {
        let l = self.lang();
        if self.b().lang == 0 {
            self.cfg.tab_width
        } else {
            l.tab_width
        }
    }
    pub fn indent_width(&self) -> usize {
        self.b().indent_override.unwrap_or(self.lang().indent)
    }
    pub fn use_tabs(&self) -> bool {
        if self.b().lang == 0 {
            self.cfg.indent_tabs_mode
        } else {
            self.lang().tabs
        }
    }
    pub fn count(&self) -> i64 {
        self.arg.unwrap_or(1)
    }
    pub fn windows(&self) -> Vec<WinId> {
        let mut v = Vec::new();
        self.layout.leaves(&mut v);
        v
    }

    // ---------------- messages ----------------

    pub fn msg(&mut self, s: impl Into<String>) {
        let s = s.into();
        if !s.is_empty() {
            self.messages.push(s.clone());
            if self.messages.len() > 1000 {
                self.messages.drain(..200);
            }
        }
        self.message = Some(s);
    }

    pub fn ding(&mut self) {
        if self.cfg.visible_bell {
            self.bell = Some(Instant::now() + Duration::from_millis(80));
        } else {
            use std::io::Write;
            let _ = std::io::stdout().write_all(b"\x07");
        }
        if self.executing_macro {
            self.errored = true;
        }
    }

    pub fn error(&mut self, s: impl Into<String>) {
        self.msg(s);
        self.ding();
        self.errored = true;
    }

    // ---------------- buffers & windows ----------------

    pub fn add_buffer(&mut self, mut b: Buffer) -> BufId {
        b.name = self.unique_name(&b.name);
        let id = match self.bufs.iter().position(|b| b.is_none()) {
            Some(i) => {
                self.bufs[i] = Some(b);
                i
            }
            None => {
                self.bufs.push(Some(b));
                self.bufs.len() - 1
            }
        };
        self.buf_order.push(id);
        id
    }

    pub fn unique_name(&self, name: &str) -> String {
        if self.find_buffer(name).is_none() {
            return name.to_string();
        }
        let mut n = 2;
        loop {
            let cand = format!("{name}<{n}>");
            if self.find_buffer(&cand).is_none() {
                return cand;
            }
            n += 1;
        }
    }

    pub fn find_buffer(&self, name: &str) -> Option<BufId> {
        self.bufs.iter().position(|b| {
            b.as_ref()
                .map(|b| b.name == name && b.kind != BufKind::Mini)
                .unwrap_or(false)
        })
    }

    pub fn live_buffers(&self) -> Vec<BufId> {
        self.buf_order
            .iter()
            .copied()
            .filter(|&b| self.bufs[b].is_some() && b != self.mini_buf)
            .collect()
    }

    fn touch_buffer(&mut self, id: BufId) {
        self.buf_order.retain(|&b| b != id);
        self.buf_order.insert(0, id);
    }

    /// Most recently used buffer other than `not` (and not shown elsewhere if possible).
    pub fn other_buffer(&self, not: BufId) -> BufId {
        let shown: Vec<BufId> = self.windows().iter().map(|&w| self.win(w).buf).collect();
        let live = self.live_buffers();
        live.iter()
            .copied()
            .find(|&b| b != not && !shown.contains(&b) && !self.buf(b).name.starts_with(' '))
            .or_else(|| live.iter().copied().find(|&b| b != not))
            .unwrap_or(not)
    }

    fn attach_window(&mut self, wid: WinId, bid: BufId) {
        let (pt, ws) = {
            let b = self.buf(bid);
            (b.mpos(b.pt), b.mpos(b.ws))
        };
        let b = self.bufm(bid);
        let pm = b.new_marker(pt, false);
        let sm = b.new_marker(ws, false);
        let w = Window::blank(bid, pm, sm);
        if wid >= self.wins.len() {
            self.wins.resize_with(wid + 1, || None);
        }
        self.wins[wid] = Some(w);
        if bid != self.mini_buf {
            self.touch_buffer(bid);
        }
    }

    fn detach_window(&mut self, wid: WinId) {
        let Some(w) = self.wins[wid].take() else {
            return;
        };
        if let Some(b) = self.bufs[w.buf].as_mut() {
            let p = b.mpos(w.pt);
            let s = b.mpos(w.start);
            b.set_mpos(b.pt, p);
            b.set_mpos(b.ws, s);
            b.free_marker(w.pt);
            b.free_marker(w.start);
        }
    }

    /// Display buffer `bid` in window `wid`.
    pub fn set_window_buffer(&mut self, wid: WinId, bid: BufId) {
        let geom = self.wins[wid].as_ref().map(|w| (w.x, w.y, w.w, w.h));
        if self.wins[wid].as_ref().map(|w| w.buf) == Some(bid) {
            self.touch_buffer(bid);
            return;
        }
        self.detach_window(wid);
        self.attach_window(wid, bid);
        if let Some((x, y, ww, h)) = geom {
            let w = self.winm(wid);
            w.x = x;
            w.y = y;
            w.w = ww;
            w.h = h;
        }
    }

    pub fn switch_to(&mut self, bid: BufId) {
        let s = self.sel;
        self.set_window_buffer(s, bid);
    }

    /// Pop to a window other than the selected one, splitting if needed.
    pub fn other_window_for(&mut self) -> WinId {
        let wins = self.windows();
        if wins.len() > 1 {
            let i = wins.iter().position(|&w| w == self.sel).unwrap_or(0);
            return wins[(i + 1) % wins.len()];
        }
        self.split_window(false)
    }

    pub fn display_buffer_other(&mut self, bid: BufId, select: bool) {
        let w = self.other_window_for();
        self.set_window_buffer(w, bid);
        if select {
            self.sel = w;
        }
    }

    pub fn split_window(&mut self, side: bool) -> WinId {
        let cur = if self.sel == MINI {
            self.mini.as_ref().map(|m| m.ret_win).unwrap_or(1)
        } else {
            self.sel
        };
        let bid = self.win(cur).buf;
        let new = self
            .wins
            .iter()
            .skip(1)
            .position(|w| w.is_none())
            .map(|i| i + 1)
            .unwrap_or(self.wins.len());
        // New window shows the same buffer at the same point/start.
        let (pt, st, row) = {
            let w = self.win(cur);
            let b = self.buf(bid);
            (b.mpos(w.pt), b.mpos(w.start), w.start_row)
        };
        let b = self.bufm(bid);
        let pm = b.new_marker(pt, false);
        let sm = b.new_marker(st, false);
        let mut w = Window::blank(bid, pm, sm);
        w.start_row = row;
        if new >= self.wins.len() {
            self.wins.resize_with(new + 1, || None);
        }
        self.wins[new] = Some(w);
        self.layout.split(cur, new, side);
        self.relayout();
        new
    }

    pub fn delete_window(&mut self, wid: WinId) -> bool {
        if self.windows().len() <= 1 || wid == MINI {
            return false;
        }
        let wins = self.windows();
        let i = wins.iter().position(|&w| w == wid).unwrap_or(0);
        self.layout.remove(wid);
        self.detach_window(wid);
        let rest = self.windows();
        let next = rest[i
            .min(rest.len() - 1)
            .saturating_sub(if i > 0 { 1 } else { 0 })];
        if self.sel == wid {
            self.sel = next;
        }
        // The minibuffer must return to a live window.
        if let Some(m) = self.mini.as_mut() {
            if m.ret_win == wid {
                m.ret_win = next;
            }
        }
        self.relayout();
        true
    }

    pub fn kill_buffer(&mut self, bid: BufId) {
        if bid == self.mini_buf {
            return;
        }
        let other = self.other_buffer(bid);
        let other = if other == bid {
            let cwd = self.buf(bid).dir.clone();
            self.add_buffer(Buffer::new("*scratch*", cwd))
        } else {
            other
        };
        for w in self.windows() {
            if self.win(w).buf == bid {
                self.set_window_buffer(w, other);
            }
        }
        self.bufs[bid] = None;
        self.buf_order.retain(|&b| b != bid);
        if self.text_cache.as_ref().map(|c| c.0) == Some(bid) {
            self.text_cache = None;
        }
    }

    pub fn resize(&mut self, w: u16, h: u16) {
        self.width = w;
        self.height = h;
        self.relayout();
    }

    pub fn relayout(&mut self) {
        let h = self.height.saturating_sub(1).max(2);
        let layout = self.layout.clone();
        self.place(&layout, 0, 0, self.width, h);
        let (w, hh) = (self.width, self.height);
        let mw = self.winm(MINI);
        mw.x = 0;
        mw.y = hh.saturating_sub(1);
        mw.w = w;
        mw.h = 1;
    }

    fn place(&mut self, l: &Layout, x: u16, y: u16, w: u16, h: u16) {
        match l {
            Layout::Leaf(id) => {
                let win = self.winm(*id);
                win.x = x;
                win.y = y;
                win.w = w;
                win.h = h;
            }
            Layout::Split { side, ratio, a, b } => {
                if *side {
                    let wa =
                        ((w as f32 * ratio).round() as u16).clamp(1, w.saturating_sub(1).max(1));
                    self.place(a, x, y, wa, h);
                    self.place(b, x + wa, y, w - wa, h);
                } else {
                    let ha =
                        ((h as f32 * ratio).round() as u16).clamp(2, h.saturating_sub(2).max(2));
                    self.place(a, x, y, w, ha);
                    self.place(b, x, y + ha, w, h - ha);
                }
            }
        }
    }

    /// Does window `wid` have a window to its right (needs a border column)?
    pub fn has_right_border(&self, wid: WinId) -> bool {
        let w = self.win(wid);
        wid != MINI && w.x + w.w < self.width
    }

    pub fn text_width(&self, wid: WinId) -> usize {
        let w = self.win(wid);
        let b = if self.has_right_border(wid) { 1 } else { 0 };
        (w.w as usize).saturating_sub(b)
    }

    pub fn text_height(&self, wid: WinId) -> usize {
        if wid == MINI {
            1
        } else {
            (self.win(wid).h as usize).saturating_sub(1).max(1)
        }
    }

    // ---------------- editing primitives ----------------

    pub fn check_writable(&mut self) -> bool {
        if self.b().read_only {
            let n = self.b().name.clone();
            self.error(format!("Buffer is read-only: #<buffer {n}>"));
            return false;
        }
        true
    }

    /// Insert at point, moving point after the text.
    pub fn insert(&mut self, s: &str) {
        let p = self.pt();
        self.bm().insert(p, s);
        self.goto(p + s.chars().count());
    }

    pub fn delete(&mut self, a: usize, b: usize) -> String {
        self.bm().delete(a, b)
    }

    /// Kill text into the kill ring (appending on consecutive kills).
    pub fn kill(&mut self, a: usize, b: usize) {
        let (a, b) = (a.min(b), a.max(b));
        let s = self.delete(a, b);
        self.add_kill(s, false);
    }

    pub fn kill_backward(&mut self, a: usize, b: usize) {
        let s = self.delete(a.min(b), a.max(b));
        self.add_kill(s, true);
    }

    pub fn add_kill(&mut self, s: String, prepend: bool) {
        if self.last_kill && !self.kill_ring.is_empty() {
            if prepend {
                self.kill_ring[0].insert_str(0, &s);
            } else {
                self.kill_ring[0].push_str(&s);
            }
        } else {
            self.kill_ring.insert(0, s);
            if self.kill_ring.len() > 120 {
                self.kill_ring.pop();
            }
        }
        self.kill_idx = 0;
        self.this_kill = true;
        let s = self.kill_ring[0].clone();
        self.clipboard.copy(&s);
        self.clip_last = Some(s);
    }

    /// Emacs interprogram-paste: if another program put new text on the
    /// clipboard, push it onto the kill ring before yanking.
    pub fn sync_clipboard(&mut self) {
        let Some(t) = self.clipboard.paste() else {
            return;
        };
        if self.clip_last.as_deref() == Some(t.as_str()) || self.kill_ring.first() == Some(&t) {
            return;
        }
        self.kill_ring.insert(0, t.clone());
        self.kill_ring.truncate(120);
        self.kill_idx = 0;
        self.clip_last = Some(t);
    }

    pub fn copy_as_kill(&mut self, s: String) {
        let was = self.last_kill;
        self.last_kill = false;
        self.add_kill(s, false);
        self.last_kill = was;
        self.this_kill = false;
    }

    // ---------------- key dispatch ----------------

    pub fn handle_key(&mut self, mut key: Key) {
        if let Some(rec) = self.macro_rec.as_mut() {
            rec.push(key);
        }
        self.errored = false;

        // Esc as Meta prefix.
        if key.k == K::Esc && !key.ctrl && !key.meta && !key.shift {
            let after_meta_esc = self
                .prefix
                .last()
                .map(|k| k.k == K::Esc && k.meta)
                .unwrap_or(false);
            if self.meta_pending {
                self.meta_pending = false;
                key = key.with_meta();
            } else if !after_meta_esc && self.read_key.is_none() {
                self.meta_pending = true;
                return;
            }
        } else if self.meta_pending {
            self.meta_pending = false;
            key.meta = true;
        }

        if let Some(rk) = self.read_key.take() {
            self.handle_read_key(rk, key);
            return;
        }
        // `C-x z z z...` and `C-x e e e...`: the last key repeats the command.
        if let Some((k, cmd)) = self.repeat_key.take() {
            if key == k && self.prefix.is_empty() {
                self.message = None;
                self.run_command(&cmd);
                return;
            }
        }
        if self.qr.is_some() {
            crate::search::qr_key(self, key);
            return;
        }
        if self.isearch.is_some() && self.prefix.is_empty() && crate::search::isearch_key(self, key)
        {
            return;
        }

        // Numeric argument digits after C-u.
        if self.prefix.is_empty() && (self.arg_universal || self.arg_digits) {
            if let Some(c) = key.plain_char().or(if key.meta || key.ctrl {
                match key.k {
                    K::Char(c) if c.is_ascii_digit() || c == '-' => Some(c),
                    _ => None,
                }
            } else {
                None
            }) {
                if c.is_ascii_digit() {
                    let d = c.to_digit(10).unwrap() as i64;
                    let cur = if self.arg_digits {
                        self.arg.unwrap_or(0)
                    } else {
                        0
                    };
                    let neg = cur < 0 || (self.arg == Some(-1) && !self.arg_digits);
                    let v = cur.abs() * 10 + d;
                    self.arg = Some(if neg { -v } else { v });
                    self.arg_digits = true;
                    self.arg_universal = true;
                    self.msg(format!("C-u {}-", self.arg.unwrap()));
                    return;
                }
                if c == '-' && !self.arg_digits {
                    self.arg = Some(-1);
                    self.msg("C-u -");
                    return;
                }
            }
        }

        self.prefix.push(key);
        let lookup = self.lookup_key(&self.prefix.clone());
        match lookup {
            Some(Ok(cmd)) => {
                self.prefix.clear();
                self.prefix_time = None;
                if self.mini.is_none() || self.sel != MINI {
                    if !matches!(
                        cmd.as_str(),
                        "universal-argument" | "digit-argument" | "negative-argument"
                    ) {
                        self.message = None;
                    }
                } else {
                    self.message = None;
                }
                self.run_command(&cmd);
            }
            Some(Err(())) => {
                if self.prefix_time.is_none() {
                    self.prefix_time = Some(Instant::now());
                }
            }
            None => {
                let seq = std::mem::take(&mut self.prefix);
                self.prefix_time = None;
                if seq.len() == 1 {
                    if let Some(c) = key.plain_char() {
                        self.message = None;
                        self.self_insert(c);
                        return;
                    }
                    // M-digit: numeric argument
                    if key.meta || key.ctrl {
                        if let K::Char(c) = key.k {
                            if c.is_ascii_digit() {
                                let d = c.to_digit(10).unwrap() as i64;
                                let cur = if self.arg_digits {
                                    self.arg.unwrap_or(0)
                                } else {
                                    0
                                };
                                self.arg = Some(cur * 10 + if cur < 0 { -d } else { d });
                                self.arg_digits = true;
                                return;
                            }
                        }
                    }
                }
                self.error(format!("{} is undefined", seq_to_string(&seq)));
                self.reset_arg();
            }
        }
    }

    /// Ok(cmd) for complete binding, Err(()) for prefix, None for undefined.
    fn lookup_key(&self, keys: &[Key]) -> Option<Result<String, ()>> {
        let mut maps: Vec<&Keymap> = Vec::new();
        if self.sel == MINI {
            maps.push(&self.mini_map);
        } else {
            match self.b().kind {
                BufKind::BufferList => maps.push(&self.buflist_map),
                BufKind::Dired => maps.push(&self.dired_map),
                BufKind::Special => maps.push(&self.special_map),
                _ => {}
            }
        }
        maps.push(&self.global);
        let mut prefix = false;
        for m in maps {
            match m.lookup(keys) {
                Lookup::Cmd(c) => {
                    if prefix {
                        continue;
                    }
                    return Some(Ok(c.to_string()));
                }
                Lookup::Prefix => prefix = true,
                Lookup::None => {}
            }
        }
        if prefix { Some(Err(())) } else { None }
    }

    pub fn reset_arg(&mut self) {
        self.arg = None;
        self.arg_universal = false;
        self.arg_digits = false;
    }

    pub fn run_command(&mut self, name: &str) {
        let Some(f) = crate::commands::lookup(name) else {
            self.error(format!("Command not implemented: {name}"));
            self.reset_arg();
            return;
        };
        self.this_cmd = name.to_string();
        self.keep_arg = false;
        self.this_kill = false;
        self.this_yank = false;
        let bid = self.cur();
        let p = self.pt();
        self.bufm(bid).cmd_point = p;
        self.begin_command();
        f(self);
        self.post_command(name);
    }

    fn post_command(&mut self, name: &str) {
        let amalg = if name == "self-insert-command" {
            if self.last_cmd == "self-insert-command" {
                2
            } else {
                1
            }
        } else {
            0
        };
        for b in self.bufs.iter_mut().flatten() {
            b.undo_boundary(amalg);
            if b.deactivate {
                b.deactivate = false;
                b.mark_active = false;
            }
        }
        if !matches!(
            name,
            "previous-line" | "next-line" | "scroll-up-command" | "scroll-down-command"
        ) && !self.keep_arg
        {
            if let Some(w) = self.wins.get_mut(self.sel).and_then(|w| w.as_mut()) {
                w.goal = None;
            }
        }
        if !matches!(name, "recenter-top-bottom") {
            self.recenter_cycle = 0;
        }
        if !self.keep_arg {
            if name != "repeat" && !name.starts_with("kmacro") {
                self.last_repeatable = Some((name.to_string(), self.arg));
            }
            self.reset_arg();
            self.last_cmd = name.to_string();
            self.last_kill = self.this_kill;
            self.last_yank = self.this_yank;
        }
    }

    /// Like Emacs resetting `deactivate-mark` before each command: only
    /// changes made by this command may deactivate the mark.
    fn begin_command(&mut self) {
        for b in self.bufs.iter_mut().flatten() {
            b.deactivate = false;
        }
    }

    pub fn self_insert(&mut self, c: char) {
        self.this_cmd = "self-insert-command".into();
        let bid = self.cur();
        let p = self.pt();
        self.bufm(bid).cmd_point = p;
        self.keep_arg = false;
        self.begin_command();
        crate::commands::self_insert(self, c);
        self.post_command("self-insert-command");
    }

    /// Bracketed paste: insert text literally as one undoable change.
    pub fn paste(&mut self, s: &str) {
        if self.isearch.is_some() {
            crate::search::isearch_exit(self);
        }
        let s = s.replace("\r\n", "\n").replace('\r', "\n");
        let s = if self.sel == MINI {
            s.replace('\n', " ")
        } else {
            s
        };
        if !self.check_writable() {
            return;
        }
        let bid = self.cur();
        let p = self.pt();
        self.bufm(bid).cmd_point = p;
        self.bufm(bid).push_mark(p, false);
        self.insert(&s);
        self.post_command("yank");
    }

    pub fn execute_keys(&mut self, keys: &[Key]) {
        let was = self.executing_macro;
        self.executing_macro = true;
        for &k in keys {
            self.handle_key(k);
            if self.errored || self.quit {
                break;
            }
        }
        self.executing_macro = was;
    }

    // ---------------- read-key prompts ----------------

    pub fn yes_or_no(&mut self, prompt: impl Into<String>, action: YnAction) {
        let p = prompt.into();
        let suffix = if self.cfg.yes_or_no_short {
            "(y or n) "
        } else {
            "(yes or no) "
        };
        let p = format!("{p}{suffix}");
        self.message = Some(p.clone());
        self.read_key = Some(ReadKey::YesNo(action, p));
    }

    fn handle_read_key(&mut self, rk: ReadKey, key: Key) {
        match rk {
            ReadKey::YesNo(action, prompt) => {
                let ans = match key.k {
                    K::Char('y') | K::Char('Y') | K::Char(' ') if !key.ctrl && !key.meta => {
                        Some(true)
                    }
                    K::Char('n') | K::Char('N') | K::Backspace if !key.ctrl && !key.meta => {
                        Some(false)
                    }
                    K::Char('!')
                        if matches!(action, YnAction::ExitSave(..) | YnAction::SaveSome(_)) =>
                    {
                        // save all remaining
                        let is_exit = matches!(action, YnAction::ExitSave(..));
                        match action {
                            YnAction::ExitSave(list) | YnAction::SaveSome(list)
                                if !list.is_empty() =>
                            {
                                for b in list {
                                    crate::files::save_buffer_id(self, b, true);
                                }
                                if is_exit {
                                    crate::files::exit_check_rest(self);
                                }
                            }
                            _ => {}
                        }
                        self.message = None;
                        return;
                    }
                    K::Char('q')
                        if matches!(action, YnAction::ExitSave(..) | YnAction::SaveSome(_)) =>
                    {
                        self.message = None;
                        if matches!(action, YnAction::ExitSave(..)) {
                            crate::files::exit_check_rest(self);
                        }
                        return;
                    }
                    K::Char('g') if key.ctrl => {
                        self.message = Some("Quit".into());
                        self.ding();
                        return;
                    }
                    _ => None,
                };
                match ans {
                    Some(a) => {
                        self.message = None;
                        crate::files::yn_answer(self, action, a);
                    }
                    None => {
                        self.message = Some(format!("Please answer y or n.  {prompt}"));
                        self.read_key = Some(ReadKey::YesNo(action, prompt));
                    }
                }
            }
            ReadKey::DescribeKey(mut seq, brief) => {
                if key.k == K::Esc && !key.meta && seq.last().map(|k| k.k != K::Esc).unwrap_or(true)
                {
                    // allow ESC as meta in describe-key
                }
                seq.push(key);
                match self.lookup_key(&seq) {
                    Some(Ok(cmd)) => {
                        let s = seq_to_string(&seq);
                        if brief {
                            self.msg(format!("{s} runs the command {cmd}"));
                        } else {
                            let doc = crate::commands::doc(&cmd);
                            let text = format!("{s} runs the command {cmd}.\n\n{doc}\n");
                            crate::files::show_help(self, &text);
                        }
                    }
                    Some(Err(())) => {
                        self.message = Some(format!("Describe key: {}-", seq_to_string(&seq)));
                        self.read_key = Some(ReadKey::DescribeKey(seq, brief));
                    }
                    None => {
                        if seq.len() == 1 && key.plain_char().is_some() {
                            self.msg(format!(
                                "{} runs the command self-insert-command",
                                seq_to_string(&seq)
                            ));
                        } else {
                            self.msg(format!("{} is undefined", seq_to_string(&seq)));
                        }
                    }
                }
            }
            ReadKey::ZapToChar => {
                if let K::Char(c) = key.k {
                    if key.ctrl && c == 'g' {
                        self.msg("Quit");
                        return;
                    }
                    crate::commands::zap_to_char(self, c);
                } else if key.k == K::Enter {
                    crate::commands::zap_to_char(self, '\n');
                } else if key.k == K::Tab {
                    crate::commands::zap_to_char(self, '\t');
                }
                self.message = None;
                self.post_command("zap-to-char");
            }
            ReadKey::QuotedInsert => {
                self.message = None;
                let c = match key.k {
                    K::Char(c) if key.ctrl => {
                        if c == ' ' || c == '@' {
                            Some('\0')
                        } else if c.is_ascii_lowercase() {
                            char::from_u32(c as u32 - 'a' as u32 + 1)
                        } else {
                            match c {
                                '[' => Some('\x1b'),
                                '\\' => Some('\x1c'),
                                ']' => Some('\x1d'),
                                '^' => Some('\x1e'),
                                '_' => Some('\x1f'),
                                _ => Some(c),
                            }
                        }
                    }
                    K::Char(c) => Some(c),
                    K::Enter => Some('\r'),
                    K::Tab => Some('\t'),
                    K::Esc => Some('\x1b'),
                    K::Backspace => Some('\x7f'),
                    _ => None,
                };
                if let Some(c) = c {
                    if self.check_writable() {
                        let n = self.count().max(1) as usize;
                        let s: String = std::iter::repeat_n(c, n).collect();
                        self.insert(&s);
                    }
                }
                self.post_command("quoted-insert");
            }
            ReadKey::Register(op) => {
                self.message = None;
                if let K::Char(c) = key.k {
                    if key.ctrl && c == 'g' {
                        self.msg("Quit");
                        return;
                    }
                    crate::commands::register_op(self, op, c);
                }
                self.post_command("register");
            }
            ReadKey::IndentRigidly => {
                let delta: i64 = match (key.k, key.shift) {
                    (K::Left, false) => -1,
                    (K::Right, false) => 1,
                    (K::Left, true) => -(self.tab_width() as i64),
                    (K::Right, true) => self.tab_width() as i64,
                    _ => {
                        self.message = None;
                        self.handle_key(key);
                        return;
                    }
                };
                crate::commands::indent_rigidly_by(self, delta);
                self.read_key = Some(ReadKey::IndentRigidly);
                self.message =
                    Some("Indent region with <left>, <right>, <S-left>, or <S-right>.".into());
                self.post_command("indent-rigidly");
            }
        }
    }

    // ---------------- minibuffer ----------------

    pub fn read_string(
        &mut self,
        prompt: &str,
        kind: PromptKind,
        initial: &str,
        comp: Completion,
        hist: &'static str,
    ) {
        self.read_string_default(prompt, kind, initial, comp, hist, None);
    }

    pub fn read_string_default(
        &mut self,
        prompt: &str,
        kind: PromptKind,
        initial: &str,
        comp: Completion,
        hist: &'static str,
        default: Option<String>,
    ) {
        if self.mini.is_some() {
            self.error("Command attempted to use minibuffer while in minibuffer");
            return;
        }
        let ret = self.sel;
        let mb = self.mini_buf;
        {
            let b = self.bufm(mb);
            b.undo_disabled = true;
            b.set_text(initial);
            b.undo_disabled = false;
            b.clear_undo();
            b.modified = false;
            b.mark_active = false;
        }
        self.mini = Some(Mini {
            prompt: prompt.to_string(),
            kind,
            comp,
            hist,
            hist_idx: None,
            ret_win: ret,
            default,
            saved_input: String::new(),
        });
        self.sel = MINI;
        let n = self.buf(mb).len();
        self.goto(n);
        self.completions = None;
        self.message = None;
    }

    pub fn mini_text(&self) -> String {
        self.buf(self.mini_buf).text.to_string()
    }

    pub fn mini_set_text(&mut self, s: &str) {
        let mb = self.mini_buf;
        self.bufm(mb).set_text(s);
        let n = self.buf(mb).len();
        self.goto(n);
    }

    pub fn mini_exit(&mut self) -> Option<Mini> {
        let m = self.mini.take()?;
        self.sel = if self
            .wins
            .get(m.ret_win)
            .map(|w| w.is_some())
            .unwrap_or(false)
        {
            m.ret_win
        } else {
            self.windows()[0]
        };
        self.completions = None;
        Some(m)
    }

    pub fn push_history(&mut self, hist: &'static str, s: &str) {
        if s.is_empty() {
            return;
        }
        let h = self.hist.entry(hist).or_default();
        h.retain(|x| x != s);
        h.insert(0, s.to_string());
        h.truncate(100);
    }

    // ---------------- timers ----------------

    /// Time until the next timer-driven redisplay is needed.
    pub fn next_timeout(&self) -> Duration {
        let mut t = Duration::from_secs(60);
        if self.cfg.display_time {
            let now = chrono::Local::now();
            use chrono::Timelike;
            let secs = 60 - now.second() as u64;
            t = t.min(Duration::from_millis(secs * 1000 + 50));
        }
        if let Some(b) = self.bell {
            t = t.min(b.saturating_duration_since(Instant::now()));
        }
        if let Some(pt) = self.prefix_time {
            let d = (pt + Duration::from_secs(1)).saturating_duration_since(Instant::now());
            if !d.is_zero() {
                t = t.min(d);
            }
        }
        t
    }

    /// Some buffer still needs background (stealth) fontification.
    pub fn fontify_pending(&self) -> bool {
        self.bufs
            .iter()
            .flatten()
            .any(|b| b.syntax.as_ref().is_some_and(|c| c.pending()))
    }

    /// Do a slice of background fontification work.
    pub fn fontify_step(&mut self, budget: Duration) {
        let deadline = Instant::now() + budget;
        for b in self.bufs.iter_mut().flatten() {
            if let Some(mut cache) = b.syntax.take() {
                if b.dirty_line != usize::MAX {
                    cache.invalidate(b.dirty_line, b.text.len_lines());
                    b.dirty_line = usize::MAX;
                }
                if cache.pending() {
                    cache.advance(&b.text, deadline);
                }
                b.syntax = Some(cache);
            }
            if Instant::now() >= deadline {
                break;
            }
        }
    }

    pub fn tick(&mut self) {
        if let Some(b) = self.bell {
            if Instant::now() >= b {
                self.bell = None;
            }
        }
        if let Some(pt) = self.prefix_time {
            if pt.elapsed() >= Duration::from_secs(1) && !self.prefix.is_empty() {
                let mut s = String::new();
                if let Some(a) = self.arg {
                    if self.arg_universal {
                        s.push_str(&format!("C-u {a} "));
                    }
                }
                s.push_str(&seq_to_string(&self.prefix));
                s.push('-');
                self.message = Some(s);
            }
        }
    }
}

impl Window {
    pub fn blank(buf: BufId, pt: MarkerId, start: MarkerId) -> Window {
        Window {
            buf,
            pt,
            start,
            start_row: 0,
            hscroll: 0,
            x: 0,
            y: 0,
            w: 80,
            h: 23,
            rows: Vec::new(),
            goal: None,
        }
    }
}
