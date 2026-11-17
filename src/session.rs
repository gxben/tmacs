// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Benjamin Zores

//! desktop-save-mode and savehist-mode: persist open files and histories.

use crate::buffer::BufKind;
use crate::editor::Editor;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Default)]
struct Desktop {
    #[serde(default)]
    buffers: Vec<Entry>,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    path: String,
    point: usize,
    #[serde(default)]
    dired: bool,
}

#[derive(Serialize, Deserialize, Default)]
struct History {
    #[serde(default)]
    minibuffer: HashMap<String, Vec<String>>,
    #[serde(default)]
    search: Vec<String>,
    #[serde(default)]
    regexp_search: Vec<String>,
}

/// History names used by the minibuffer (must be 'static in the editor).
const HIST_NAMES: &[&str] = &[
    "file",
    "buffer",
    "extended-command",
    "goto-line",
    "replace",
    "shell",
    "regexp",
    "string-rectangle",
    "bookmark",
    "goto-char",
];

fn dir() -> PathBuf {
    crate::config::config_path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
}

fn write(d: &Path, name: &str, contents: &str) {
    let _ = std::fs::create_dir_all(d);
    let tmp = d.join(format!(".{name}.tmp"));
    if std::fs::write(&tmp, contents).is_ok() {
        let _ = std::fs::rename(&tmp, d.join(name));
    }
}

/// Save open file buffers (most recent first) to the desktop file.
pub fn save_desktop(ed: &Editor) {
    save_desktop_in(ed, &dir());
}

fn save_desktop_in(ed: &Editor, d: &Path) {
    let mut buffers = Vec::new();
    // Selected window's buffer first, then MRU order.
    let mut order = vec![ed.cur()];
    order.extend(ed.live_buffers().into_iter().filter(|&b| b != ed.cur()));
    for b in order {
        let buf = ed.buf(b);
        let shown = ed.windows().into_iter().find(|&w| ed.win(w).buf == b);
        let point = match shown {
            Some(w) => ed.win_pt(w),
            None => buf.mpos(buf.pt),
        };
        match buf.kind {
            BufKind::Normal => {
                if let Some(p) = &buf.path {
                    buffers.push(Entry {
                        path: p.to_string_lossy().into(),
                        point,
                        dired: false,
                    });
                }
            }
            BufKind::Dired => {
                buffers.push(Entry {
                    path: buf.dir.to_string_lossy().into(),
                    point,
                    dired: true,
                });
            }
            _ => {}
        }
    }
    if let Ok(s) = toml::to_string(&Desktop { buffers }) {
        write(d, "desktop.toml", &s);
    }
}

/// Reopen the buffers recorded in the desktop file.
pub fn restore_desktop(ed: &mut Editor) {
    restore_desktop_in(ed, &dir());
}

fn restore_desktop_in(ed: &mut Editor, d: &Path) {
    let Ok(s) = std::fs::read_to_string(d.join("desktop.toml")) else {
        return;
    };
    let Ok(d) = toml::from_str::<Desktop>(&s) else {
        return;
    };
    let mut opened = Vec::new();
    for e in d.buffers.iter().rev() {
        let p = PathBuf::from(&e.path);
        if !p.exists() {
            continue;
        }
        if let Ok(b) = crate::files::visit(ed, &p) {
            let bf = ed.bufm(b);
            let pos = e.point.min(bf.len());
            bf.set_mpos(bf.pt, pos);
            opened.push(b);
        }
    }
    if let Some(&first) = opened.last() {
        ed.switch_to(first);
        // Buffer MRU order follows the saved desktop order.
        let mut order: Vec<_> = opened.iter().rev().copied().collect();
        order.extend(ed.buf_order.iter().copied().filter(|b| !opened.contains(b)));
        ed.buf_order = order;
        let n = opened.len();
        ed.msg(format!(
            "Desktop: {n} buffer{} restored",
            if n == 1 { "" } else { "s" }
        ));
    }
}

pub fn save_history(ed: &Editor) {
    save_history_in(ed, &dir());
}

fn save_history_in(ed: &Editor, d: &Path) {
    // Merge with what other sessions saved, newest first.
    let mut h = load_history_file(d);
    for (k, v) in &ed.hist {
        let e = h.minibuffer.entry(k.to_string()).or_default();
        let mut merged = v.clone();
        merged.extend(e.iter().filter(|x| !v.contains(x)).cloned());
        merged.truncate(100);
        *e = merged;
    }
    let merge = |a: &[String], b: &[String]| {
        let mut m = a.to_vec();
        m.extend(b.iter().filter(|x| !a.contains(x)).cloned());
        m.truncate(16);
        m
    };
    h.search = merge(&ed.search_ring, &h.search);
    h.regexp_search = merge(&ed.regex_search_ring, &h.regexp_search);
    if let Ok(s) = toml::to_string(&h) {
        write(d, "history.toml", &s);
    }
}

fn load_history_file(d: &Path) -> History {
    std::fs::read_to_string(d.join("history.toml"))
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn restore_history(ed: &mut Editor) {
    restore_history_in(ed, &dir());
}

fn restore_history_in(ed: &mut Editor, d: &Path) {
    let h = load_history_file(d);
    for (k, v) in h.minibuffer {
        if let Some(&name) = HIST_NAMES.iter().find(|n| **n == k) {
            ed.hist.insert(name, v);
        }
    }
    ed.search_ring = h.search;
    ed.regex_search_ring = h.regexp_search;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn editor() -> Editor {
        let mut c = Config::default();
        c.settings.clipboard = "none".into();
        let mut ed = Editor::new(c, None);
        ed.resize(80, 24);
        ed
    }

    #[test]
    fn desktop_roundtrip_keeps_order_and_point() {
        let d = std::env::temp_dir().join(format!("tmacs-session-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let (a, b) = (d.join("a.txt"), d.join("b.txt"));
        std::fs::write(&a, "aaaa\n").unwrap();
        std::fs::write(&b, "bbbb\n").unwrap();
        let mut ed = editor();
        crate::files::open_path(&mut ed, &a, None);
        ed.goto(2);
        crate::files::open_path(&mut ed, &b, None);
        ed.goto(3);
        ed.push_history("file", "x");
        save_desktop_in(&ed, &d);
        save_history_in(&ed, &d);

        let mut ed2 = editor();
        restore_desktop_in(&mut ed2, &d);
        restore_history_in(&mut ed2, &d);
        assert_eq!(ed2.b().name, "b.txt");
        assert_eq!(ed2.pt(), 3);
        let other = ed2.other_buffer(ed2.cur());
        assert_eq!(ed2.buf(other).name, "a.txt");
        assert_eq!(ed2.hist.get("file").unwrap()[0], "x");
    }
}
