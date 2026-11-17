//! Files, buffers, minibuffer completion, dired, buffer menu, help, shell,
//! format-on-save.

use crate::buffer::{BufKind, Buffer};
use crate::commands::{self, abort_minibuffer};
use crate::editor::{BufId, Completion, Editor, MINI, Mini, PromptKind, YnAction};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// ======================= paths =======================

pub fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

/// Abbreviate home directory to ~.
pub fn abbreviate(p: &Path) -> String {
    let h = home();
    if let Ok(rest) = p.strip_prefix(&h) {
        if rest.as_os_str().is_empty() {
            return "~".into();
        }
        return format!("~/{}", rest.display());
    }
    p.display().to_string()
}

/// Emacs substitute-in-file-name + expand-file-name.
pub fn expand(input: &str, dir: &Path) -> PathBuf {
    let mut s = input.to_string();
    // "//" or "/~" restart the path
    if let Some(i) = s.rfind("//") {
        s = s[i + 1..].to_string();
    }
    if let Some(i) = s.rfind("/~") {
        s = s[i + 1..].to_string();
    }
    // $VAR expansion
    if s.contains('$') {
        let re = regex::Regex::new(r"\$\{?([A-Za-z_][A-Za-z0-9_]*)\}?").unwrap();
        s = re
            .replace_all(&s, |c: &regex::Captures| {
                std::env::var(&c[1]).unwrap_or_default()
            })
            .to_string();
    }
    let p = if s == "~" {
        home()
    } else if let Some(r) = s.strip_prefix("~/") {
        home().join(r)
    } else {
        let p = PathBuf::from(&s);
        if p.is_absolute() { p } else { dir.join(p) }
    };
    normalize(&p)
}

fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

fn dir_prompt_string(dir: &Path) -> String {
    let mut s = abbreviate(dir);
    if !s.ends_with('/') {
        s.push('/');
    }
    s
}

// ======================= visiting =======================

fn read_file(path: &Path) -> std::io::Result<(String, bool, bool)> {
    let bytes = std::fs::read(path)?;
    let (s, lossy) = match String::from_utf8(bytes) {
        Ok(s) => (s, false),
        Err(e) => (String::from_utf8_lossy(e.as_bytes()).into_owned(), true),
    };
    let crlf = s.contains("\r\n") && s.matches("\r\n").count() * 2 >= s.matches('\n').count();
    let s = if crlf { s.replace("\r\n", "\n") } else { s };
    Ok((s, crlf, lossy))
}

pub fn buffer_name_for(ed: &Editor, path: &Path) -> String {
    let base = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());
    if ed.find_buffer(&base).is_none() {
        return base;
    }
    // post-forward-angle-brackets uniquify style
    let parent = path
        .parent()
        .and_then(|p| p.file_name())
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let cand = format!("{base}<{parent}>");
    if ed.find_buffer(&cand).is_none() {
        cand
    } else {
        ed.unique_name(&base)
    }
}

pub fn set_mode(ed: &mut Editor, bid: BufId) {
    let (name, first) = {
        let b = ed.buf(bid);
        let name = b
            .path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| b.name.clone());
        let first = b.line_text(0);
        (name, first)
    };
    let ss = &crate::syntax::ctx().ss;
    let lang = ed.langs.detect(&name, &first, ss);
    let syn = ed.langs.list[lang].syntax.clone();
    let big = ed.buf(bid).len() > ed.cfg.max_highlight_size;
    let b = ed.bufm(bid);
    b.lang = lang;
    b.syntax = if big {
        None
    } else {
        syn.and_then(|s| crate::syntax::Cache::new(&s))
            .map(Box::new)
    };
    b.dirty_line = 0;
}

/// Visit a file, returning the buffer.
pub fn visit(ed: &mut Editor, path: &Path) -> Result<BufId, String> {
    let path = normalize(path);
    for &b in &ed.live_buffers() {
        if ed.buf(b).path.as_deref() == Some(path.as_path()) && ed.buf(b).kind == BufKind::Normal {
            return Ok(b);
        }
    }
    if path.is_dir() {
        return Ok(dired_buffer(ed, &path));
    }
    let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_else(home);
    let name = buffer_name_for(ed, &path);
    let mut b = Buffer::new(&name, dir);
    b.path = Some(path.clone());
    let mut msg = None;
    match read_file(&path) {
        Ok((s, crlf, lossy)) => {
            b.undo_disabled = true;
            b.insert(0, &s);
            b.undo_disabled = false;
            b.crlf = crlf;
            b.mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
            b.modified = false;
            b.set_mpos(b.pt, 0);
            b.set_mpos(b.ws, 0);
            if lossy {
                msg = Some("File is not valid UTF-8; invalid bytes were replaced".to_string());
            }
            if std::fs::metadata(&path)
                .map(|m| m.permissions().readonly())
                .unwrap_or(false)
            {
                b.read_only = true;
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            msg = Some("(New file)".into());
        }
        Err(e) => return Err(format!("Opening input file: {e}, {}", path.display())),
    }
    b.change_tick = 0;
    let id = ed.add_buffer(b);
    set_mode(ed, id);
    if let Some(m) = msg {
        ed.msg(m);
    }
    Ok(id)
}

pub fn open_path(ed: &mut Editor, path: &Path, line: Option<usize>) {
    match visit(ed, path) {
        Ok(b) => {
            ed.switch_to(b);
            if ed.b().kind == BufKind::Dired {
                dired_move_to_filename(ed);
            }
            if let Some(l) = line {
                commands::goto_line_n(ed, l as i64);
                let p = ed.pt();
                let m = ed.b().mark;
                ed.bm().set_mpos(m, p);
                ed.bm().mark_set = false;
            }
        }
        Err(e) => ed.error(e),
    }
}

fn cur_dir(ed: &Editor) -> PathBuf {
    let b = if ed.sel == MINI {
        ed.mini
            .as_ref()
            .map(|m| ed.win(m.ret_win).buf)
            .unwrap_or(ed.cur())
    } else {
        ed.cur()
    };
    ed.buf(b).dir.clone()
}

pub fn find_file(ed: &mut Editor) {
    let d = dir_prompt_string(&cur_dir(ed));
    ed.read_string(
        "Find file: ",
        PromptKind::FindFile {
            other: false,
            read_only: false,
        },
        &d,
        Completion::File,
        "file",
    );
}
pub fn find_file_read_only(ed: &mut Editor) {
    let d = dir_prompt_string(&cur_dir(ed));
    ed.read_string(
        "Find file read-only: ",
        PromptKind::FindFile {
            other: false,
            read_only: true,
        },
        &d,
        Completion::File,
        "file",
    );
}
pub fn find_file_other_window(ed: &mut Editor) {
    let d = dir_prompt_string(&cur_dir(ed));
    ed.read_string(
        "Find file in other window: ",
        PromptKind::FindFile {
            other: true,
            read_only: false,
        },
        &d,
        Completion::File,
        "file",
    );
}
pub fn find_alternate_file(ed: &mut Editor) {
    let init = match &ed.b().path {
        Some(p) => abbreviate(p),
        None => dir_prompt_string(&cur_dir(ed)),
    };
    ed.read_string(
        "Find alternate file: ",
        PromptKind::FindAlternate,
        &init,
        Completion::File,
        "file",
    );
}
pub fn insert_file(ed: &mut Editor) {
    if !ed.check_writable() {
        return;
    }
    let d = dir_prompt_string(&cur_dir(ed));
    ed.read_string(
        "Insert file: ",
        PromptKind::InsertFile,
        &d,
        Completion::File,
        "file",
    );
}
pub fn write_file(ed: &mut Editor) {
    let d = dir_prompt_string(&cur_dir(ed));
    ed.read_string(
        "Write file: ",
        PromptKind::WriteFile,
        &d,
        Completion::File,
        "file",
    );
}

pub fn dired(ed: &mut Editor) {
    let d = dir_prompt_string(&cur_dir(ed));
    ed.read_string(
        "Dired (directory): ",
        PromptKind::Dired { other: false },
        &d,
        Completion::File,
        "file",
    );
}
pub fn dired_other_window(ed: &mut Editor) {
    let d = dir_prompt_string(&cur_dir(ed));
    ed.read_string(
        "Dired in other window (directory): ",
        PromptKind::Dired { other: true },
        &d,
        Completion::File,
        "file",
    );
}

// ======================= saving =======================

pub fn save_buffer(ed: &mut Editor) {
    let b = ed.cur();
    if ed.buf(b).kind != BufKind::Normal {
        ed.error("Buffer is not visiting a file");
        return;
    }
    if !ed.buf(b).modified {
        ed.msg("(No changes need to be saved)");
        return;
    }
    if ed.buf(b).path.is_none() {
        write_file(ed);
        return;
    }
    save_buffer_id(ed, b, false);
}

/// Check modification on disk then save.
pub fn save_buffer_id(ed: &mut Editor, bid: BufId, force: bool) {
    let Some(path) = ed.buf(bid).path.clone() else {
        return;
    };
    if !force {
        if let (Some(old), Ok(now)) = (
            ed.buf(bid).mtime,
            std::fs::metadata(&path).and_then(|m| m.modified()),
        ) {
            if now != old {
                let name = abbreviate(&path);
                ed.yes_or_no(
                    format!("{name} has changed since visited or saved.  Save anyway? "),
                    YnAction::SaveAnyway(bid),
                );
                return;
            }
        }
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                let p = parent.to_path_buf();
                ed.yes_or_no(
                    format!("Directory `{}' does not exist; create? ", abbreviate(&p)),
                    YnAction::MakeDirs(p, true),
                );
                return;
            }
        }
    }
    do_save(ed, bid);
}

fn do_save(ed: &mut Editor, bid: BufId) {
    let Some(path) = ed.buf(bid).path.clone() else {
        return;
    };
    // before-save-hook: format on save
    let lang = ed.langs.list[ed.buf(bid).lang].clone();
    let mut fmt_msg = None;
    let excluded = lang
        .format_exclude
        .iter()
        .any(|r| r.is_match(&path.to_string_lossy()));
    if ed.cfg.format_on_save && lang.format_on_save && !excluded {
        if let Some(cmd) = &lang.formatter {
            if let Err(e) = run_formatter(ed, bid, cmd) {
                fmt_msg = Some(e);
            }
        }
    }
    // require-final-newline
    if ed.cfg.require_final_newline && lang.require_final_newline {
        let b = ed.buf(bid);
        if b.len() > 0 && b.char_before(b.len()) != Some('\n') {
            let n = b.len();
            ed.bufm(bid).insert(n, "\n");
        }
    }
    let b = ed.buf(bid);
    let mut s = b.text.to_string();
    if b.crlf {
        s = s.replace('\n', "\r\n");
    }
    match std::fs::write(&path, s.as_bytes()) {
        Ok(()) => {
            let b = ed.bufm(bid);
            b.mark_saved();
            b.mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
            match fmt_msg {
                Some(m) => ed.msg(format!("Wrote {} ({m})", path.display())),
                None => ed.msg(format!("Wrote {}", path.display())),
            }
        }
        Err(e) => ed.error(format!("Write error: {e}, {}", path.display())),
    }
}

/// Run an external formatter on the buffer contents (stdin -> stdout).
pub fn run_formatter(ed: &mut Editor, bid: BufId, cmd: &[String]) -> Result<(), String> {
    let b = ed.buf(bid);
    let file = b
        .path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let args: Vec<String> = cmd.iter().map(|a| a.replace("{file}", &file)).collect();
    let (prog, rest) = args.split_first().ok_or("empty formatter")?;
    let input = b.text.to_string();
    let dir = b.dir.clone();
    let mut child = match Command::new(prog)
        .args(rest)
        .current_dir(if dir.is_dir() { dir } else { home() })
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!("formatter {prog} not found"));
        }
        Err(e) => return Err(format!("formatter {prog}: {e}")),
    };
    let mut stdin = child.stdin.take().unwrap();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(input.as_bytes());
    });
    let mut out = child.stdout.take().unwrap();
    let mut err = child.stderr.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut s = Vec::new();
        let _ = out.read_to_end(&mut s);
        s
    });
    let ereader = std::thread::spawn(move || {
        let mut s = Vec::new();
        let _ = err.read_to_end(&mut s);
        s
    });
    let deadline = Instant::now() + Duration::from_millis(ed.cfg.format_timeout_ms);
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) if Instant::now() > deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("formatter {prog} timed out"));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(e) => return Err(e.to_string()),
        }
    };
    let _ = writer.join();
    let out = reader.join().unwrap_or_default();
    let errs = ereader.join().unwrap_or_default();
    if !status.success() {
        let e = String::from_utf8_lossy(&errs);
        let first = e
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim()
            .to_string();
        return Err(format!("{prog} failed: {first}"));
    }
    let Ok(new) = String::from_utf8(out) else {
        return Err(format!("{prog}: invalid UTF-8 output"));
    };
    if new.is_empty() && ed.buf(bid).len() > 0 {
        return Err(format!("{prog} produced no output"));
    }
    ed.bufm(bid).replace_all_minimal(&new);
    Ok(())
}

pub fn format_buffer(ed: &mut Editor) {
    let bid = ed.cur();
    let lang = ed.lang().clone();
    let Some(cmd) = lang.formatter else {
        ed.error(format!("No formatter configured for {}", lang.mode_name));
        return;
    };
    if !ed.check_writable() {
        return;
    }
    match run_formatter(ed, bid, &cmd) {
        Ok(()) => ed.msg(format!("Formatted with {}", cmd[0])),
        Err(e) => ed.error(e),
    }
}

pub fn toggle_format_on_save(ed: &mut Editor) {
    let i = ed.b().lang;
    let l = &mut ed.langs.list[i];
    l.format_on_save = !l.format_on_save;
    let (on, name) = (l.format_on_save, l.mode_name.clone());
    ed.msg(format!(
        "Format on save {} for {name}",
        if on { "enabled" } else { "disabled" }
    ));
}

pub fn save_some_buffers(ed: &mut Editor) {
    let list: Vec<BufId> = ed
        .live_buffers()
        .into_iter()
        .filter(|&b| ed.buf(b).modified && ed.buf(b).path.is_some())
        .collect();
    if list.is_empty() {
        ed.msg("(No files need saving)");
        return;
    }
    if ed.arg.is_some() {
        for b in list {
            save_buffer_id(ed, b, true);
        }
        return;
    }
    ask_save(ed, list, false);
}

fn ask_save(ed: &mut Editor, list: Vec<BufId>, exit: bool) {
    let Some(&b) = list.first() else {
        if exit {
            exit_check_rest(ed);
        }
        return;
    };
    let p = abbreviate(ed.buf(b).path.as_ref().unwrap());
    let action = if exit {
        YnAction::ExitSave(list)
    } else {
        YnAction::SaveSome(list)
    };
    let prompt = format!("Save file {p}? (y, n, !, ., q) ");
    ed.message = Some(prompt.clone());
    ed.read_key = Some(crate::editor::ReadKey::YesNo(action, prompt));
}

pub fn save_buffers_kill_terminal(ed: &mut Editor) {
    let list: Vec<BufId> = ed
        .live_buffers()
        .into_iter()
        .filter(|&b| ed.buf(b).modified && ed.buf(b).path.is_some())
        .collect();
    if list.is_empty() {
        ed.quit = true;
        return;
    }
    ask_save(ed, list, true);
}

pub fn exit_check_rest(ed: &mut Editor) {
    let any = ed
        .live_buffers()
        .into_iter()
        .any(|b| ed.buf(b).modified && ed.buf(b).path.is_some());
    if any {
        ed.yes_or_no(
            "Modified buffers exist; exit anyway? ",
            YnAction::ExitAnyway,
        );
    } else {
        ed.quit = true;
    }
}

pub fn kill_emacs(ed: &mut Editor) {
    ed.quit = true;
}

pub fn yn_answer(ed: &mut Editor, action: YnAction, yes: bool) {
    match action {
        YnAction::KillBuffer(b) => {
            if yes {
                ed.kill_buffer(b);
            }
        }
        YnAction::SaveAnyway(b) => {
            if yes {
                do_save(ed, b);
            }
        }
        YnAction::ExitSave(mut list) => {
            let b = list.remove(0);
            if yes {
                do_save(ed, b);
            }
            ask_save(ed, list, true);
        }
        YnAction::SaveSome(mut list) => {
            let b = list.remove(0);
            if yes {
                do_save(ed, b);
            }
            if !list.is_empty() {
                ask_save(ed, list, false);
            }
        }
        YnAction::ExitAnyway => {
            if yes {
                ed.quit = true;
            }
        }
        YnAction::Revert(b) => {
            if yes {
                revert(ed, b);
            }
        }
        YnAction::DiredDelete(paths) => {
            if yes {
                let mut n = 0;
                for p in &paths {
                    let r = if p.is_dir() {
                        std::fs::remove_dir(p)
                    } else {
                        std::fs::remove_file(p)
                    };
                    match r {
                        Ok(()) => n += 1,
                        Err(e) => ed.error(format!("Deleting {}: {e}", p.display())),
                    }
                }
                revert_buffer_now(ed);
                ed.msg(format!(
                    "{n} deletion{} done",
                    if n == 1 { "" } else { "s" }
                ));
            }
        }
        YnAction::MakeDirs(p, save) => {
            if yes {
                if let Err(e) = std::fs::create_dir_all(&p) {
                    ed.error(format!("Creating directory: {e}"));
                    return;
                }
                if save {
                    let b = ed.cur();
                    do_save(ed, b);
                }
            }
        }
    }
}

pub fn read_only_mode(ed: &mut Editor) {
    let b = ed.bm();
    b.read_only = !b.read_only;
    let ro = b.read_only;
    ed.msg(if ro {
        "Read-Only mode enabled in current buffer"
    } else {
        "Read-Only mode disabled in current buffer"
    });
}

pub fn revert_buffer(ed: &mut Editor) {
    let b = ed.cur();
    match ed.buf(b).kind {
        BufKind::BufferList => {
            refresh_buffer_list(ed, b);
            return;
        }
        BufKind::Dired => {
            revert_buffer_now(ed);
            return;
        }
        _ => {}
    }
    let Some(p) = ed.buf(b).path.clone() else {
        ed.error("Buffer does not seem to be associated with any file");
        return;
    };
    ed.yes_or_no(
        format!("Revert buffer from file {}? ", abbreviate(&p)),
        YnAction::Revert(b),
    );
}

fn revert(ed: &mut Editor, b: BufId) {
    let Some(p) = ed.buf(b).path.clone() else {
        return;
    };
    match read_file(&p) {
        Ok((s, crlf, _)) => {
            let buf = ed.bufm(b);
            buf.replace_all_minimal(&s);
            buf.crlf = crlf;
            buf.mark_saved();
            buf.mtime = std::fs::metadata(&p).and_then(|m| m.modified()).ok();
            ed.msg(format!("Reverted {}", abbreviate(&p)));
        }
        Err(e) => ed.error(format!("Reverting: {e}")),
    }
}

fn revert_buffer_now(ed: &mut Editor) {
    let b = ed.cur();
    if ed.buf(b).kind == BufKind::Dired {
        let dir = ed.buf(b).dir.clone();
        let line = ed.b().line_of(ed.pt());
        fill_dired(ed, b, &dir);
        let p = ed.buf(b).line_start(line).min(ed.buf(b).len());
        ed.goto(p);
        dired_move_to_filename(ed);
    }
}

pub fn suspend_frame(ed: &mut Editor) {
    ed.suspend = true;
}

// ======================= buffers =======================

pub fn switch_to_buffer(ed: &mut Editor) {
    let cur = ed.cur();
    let def = ed.buf(ed.other_buffer(cur)).name.clone();
    ed.read_string_default(
        &format!("Switch to buffer (default {def}): "),
        PromptKind::SwitchBuffer { other: false },
        "",
        Completion::Buffer,
        "buffer",
        Some(def),
    );
}

pub fn switch_to_buffer_other_window(ed: &mut Editor) {
    let cur = ed.cur();
    let def = ed.buf(ed.other_buffer(cur)).name.clone();
    ed.read_string_default(
        &format!("Switch to buffer in other window (default {def}): "),
        PromptKind::SwitchBuffer { other: true },
        "",
        Completion::Buffer,
        "buffer",
        Some(def),
    );
}

pub fn kill_buffer_cmd(ed: &mut Editor) {
    let def = ed.b().name.clone();
    ed.read_string_default(
        &format!("Kill buffer (default {def}): "),
        PromptKind::KillBuffer,
        "",
        Completion::Buffer,
        "buffer",
        Some(def),
    );
}

pub fn kill_buffer_maybe(ed: &mut Editor, b: BufId) {
    let buf = ed.buf(b);
    if buf.modified && buf.path.is_some() && buf.kind == BufKind::Normal {
        let n = buf.name.clone();
        ed.yes_or_no(
            format!("Buffer {n} modified; kill anyway? "),
            YnAction::KillBuffer(b),
        );
    } else {
        ed.kill_buffer(b);
    }
}

pub fn kill_current_buffer(ed: &mut Editor) {
    let b = ed.cur();
    kill_buffer_maybe(ed, b);
}

fn cycle_buffer(ed: &mut Editor, forward: bool) {
    let cur = ed.cur();
    let list: Vec<BufId> = ed
        .live_buffers()
        .into_iter()
        .filter(|&b| !ed.buf(b).name.starts_with(' '))
        .collect();
    if list.len() < 2 {
        return;
    }
    let target = if forward {
        *list.last().unwrap()
    } else {
        list.iter().copied().find(|&b| b != cur).unwrap()
    };
    ed.switch_to(target);
    if forward {
        // move current to the end so repeated next-buffer cycles
        ed.buf_order.retain(|&x| x != cur);
        ed.buf_order.push(cur);
    } else {
        ed.buf_order.retain(|&x| x != cur);
        ed.buf_order.push(cur);
    }
}

pub fn next_buffer(ed: &mut Editor) {
    cycle_buffer(ed, true);
}
pub fn previous_buffer(ed: &mut Editor) {
    cycle_buffer(ed, false);
}

pub fn rename_buffer(ed: &mut Editor) {
    let n = ed.b().name.clone();
    ed.read_string(
        "Rename buffer (to new name): ",
        PromptKind::RenameBuffer,
        &n,
        Completion::None,
        "buffer",
    );
}

/// Create (or reuse) a special read-only buffer with text.
pub fn special_buffer(ed: &mut Editor, name: &str, text: &str, kind: BufKind) -> BufId {
    let dir = cur_dir(ed);
    let id = match ed.find_buffer(name) {
        Some(b) => b,
        None => {
            let mut b = Buffer::new(name, dir);
            b.kind = kind;
            ed.add_buffer(b)
        }
    };
    let b = ed.bufm(id);
    b.kind = kind;
    b.read_only = false;
    b.undo_disabled = true;
    b.set_text(text);
    b.undo_disabled = false;
    b.clear_undo();
    b.modified = false;
    b.read_only = true;
    b.set_mpos(b.pt, 0);
    b.set_mpos(b.ws, 0);
    id
}

pub fn show_help(ed: &mut Editor, text: &str) {
    let b = special_buffer(ed, "*Help*", text, BufKind::Special);
    if ed.windows().iter().any(|&w| ed.win(w).buf == b) {
        for w in ed.windows() {
            if ed.win(w).buf == b {
                crate::view::set_win_point(ed, w, 0);
            }
        }
    } else {
        ed.display_buffer_other(b, false);
    }
    ed.msg("Type \"q\" in help window to delete it.");
}

fn buffer_list_text(ed: &Editor, exclude: Option<BufId>) -> (String, Vec<String>) {
    let mut lines = vec!["CRM Buffer                Size Mode             File".to_string()];
    let mut entries = vec![String::new()];
    let cur = exclude.unwrap_or(ed.cur());
    for b in ed.live_buffers() {
        let buf = ed.buf(b);
        if buf.name.starts_with(' ') {
            continue;
        }
        let c = if b == cur { '.' } else { ' ' };
        let r = if buf.read_only { '%' } else { ' ' };
        let m = if buf.modified && buf.kind == BufKind::Normal {
            '*'
        } else {
            ' '
        };
        let mode = match buf.kind {
            BufKind::BufferList => "Buffer Menu".to_string(),
            BufKind::Dired => "Dired by name".to_string(),
            BufKind::Special => "Special".to_string(),
            _ => ed.langs.list[buf.lang].mode_name.clone(),
        };
        let file = buf.path.as_ref().map(|p| abbreviate(p)).unwrap_or_default();
        let name: String = if buf.name.chars().count() > 20 {
            buf.name.chars().take(17).collect::<String>() + "..."
        } else {
            buf.name.clone()
        };
        lines.push(
            format!(
                "{c}{r}{m} {:<20} {:>7} {:<16} {}",
                name,
                buf.len(),
                mode,
                file
            )
            .trim_end()
            .to_string(),
        );
        entries.push(b.to_string());
    }
    (lines.join("\n") + "\n", entries)
}

fn refresh_buffer_list(ed: &mut Editor, bid: BufId) {
    let line = ed.buf(bid).line_of(ed.buf(bid).mpos(ed.buf(bid).pt));
    let (text, entries) = buffer_list_text(ed, None);
    special_buffer(ed, "*Buffer List*", &text, BufKind::BufferList);
    ed.bufm(bid).entries = entries;
    let p = ed.buf(bid).line_start(line.max(1));
    ed.goto(p);
}

pub fn list_buffers(ed: &mut Editor) {
    let (text, entries) = buffer_list_text(ed, None);
    let b = special_buffer(ed, "*Buffer List*", &text, BufKind::BufferList);
    ed.bufm(b).entries = entries;
    let p = ed.buf(b).line_start(1);
    let pm = ed.buf(b).pt;
    ed.bufm(b).set_mpos(pm, p);
    if !ed.windows().iter().any(|&w| ed.win(w).buf == b) {
        ed.display_buffer_other(b, false);
    }
    for w in ed.windows() {
        if ed.win(w).buf == b {
            crate::view::set_win_point(ed, w, p);
        }
    }
}

pub fn buffer_menu(ed: &mut Editor) {
    let (text, entries) = buffer_list_text(ed, None);
    let b = special_buffer(ed, "*Buffer List*", &text, BufKind::BufferList);
    ed.bufm(b).entries = entries;
    ed.switch_to(b);
    let p = ed.buf(b).line_start(1);
    ed.goto(p);
}

fn bm_entry(ed: &Editor) -> Option<BufId> {
    let line = ed.b().line_of(ed.pt());
    ed.b()
        .entries
        .get(line)?
        .parse::<usize>()
        .ok()
        .filter(|&b| ed.bufs.get(b).map(|x| x.is_some()).unwrap_or(false))
}

pub fn bm_this_window(ed: &mut Editor) {
    if let Some(b) = bm_entry(ed) {
        ed.switch_to(b);
    }
}
pub fn bm_other_window(ed: &mut Editor) {
    if let Some(b) = bm_entry(ed) {
        ed.display_buffer_other(b, true);
    }
}
pub fn bm_switch_other_window(ed: &mut Editor) {
    if let Some(b) = bm_entry(ed) {
        ed.display_buffer_other(b, false);
    }
}
pub fn bm_one_window(ed: &mut Editor) {
    if let Some(b) = bm_entry(ed) {
        ed.switch_to(b);
        let s = ed.sel;
        for w in ed.windows() {
            if w != s {
                ed.delete_window(w);
            }
        }
    }
}

fn bm_set_flag(ed: &mut Editor, col: usize, ch: char, advance: bool) {
    let line = ed.b().line_of(ed.pt());
    if line == 0 || line >= ed.b().entries.len() {
        return;
    }
    let s = ed.b().line_start(line) + col;
    let bm = ed.bm();
    bm.read_only = false;
    bm.undo_disabled = true;
    bm.replace(s, s + 1, &ch.to_string());
    bm.undo_disabled = false;
    bm.read_only = true;
    bm.modified = false;
    if advance {
        let n = (line + 1).min(ed.b().line_count() - 1);
        let p = ed.b().line_start(n);
        ed.goto(p);
    }
}

pub fn bm_delete(ed: &mut Editor) {
    bm_set_flag(ed, 0, 'D', true);
}
pub fn bm_save(ed: &mut Editor) {
    bm_set_flag(ed, 2, 'S', true);
}
pub fn bm_unmark(ed: &mut Editor) {
    let line = ed.b().line_of(ed.pt());
    if let Some(b) = bm_entry(ed) {
        let cur = if ed.buf_order.first() == Some(&b) {
            '.'
        } else {
            ' '
        };
        let m = if ed.buf(b).modified { '*' } else { ' ' };
        bm_set_flag(ed, 0, cur, false);
        bm_set_flag(ed, 2, m, false);
    }
    let n = (line + 1).min(ed.b().line_count() - 1);
    let p = ed.b().line_start(n);
    ed.goto(p);
}
pub fn bm_backup_unmark(ed: &mut Editor) {
    let line = ed.b().line_of(ed.pt());
    if line > 1 {
        let p = ed.b().line_start(line - 1);
        ed.goto(p);
        bm_unmark(ed);
        let p = ed.b().line_start(line - 1);
        ed.goto(p);
    }
}
pub fn bm_execute(ed: &mut Editor) {
    let me = ed.cur();
    let lines: Vec<(usize, String)> = (1..ed.b().entries.len())
        .map(|l| (l, ed.b().line_text(l)))
        .collect();
    let mut to_kill = Vec::new();
    for (l, t) in lines {
        let Some(b) = ed
            .buf(me)
            .entries
            .get(l)
            .and_then(|e| e.parse::<usize>().ok())
        else {
            continue;
        };
        if ed.bufs.get(b).map(|x| x.is_none()).unwrap_or(true) {
            continue;
        }
        let chars: Vec<char> = t.chars().collect();
        if chars.get(2) == Some(&'S') {
            save_buffer_id(ed, b, true);
        }
        if chars.first() == Some(&'D') {
            to_kill.push(b);
        }
    }
    for b in to_kill {
        if b != me {
            ed.kill_buffer(b);
        }
    }
    if ed.bufs.get(me).map(|b| b.is_some()).unwrap_or(false) {
        refresh_buffer_list(ed, me);
    }
}
pub fn bm_not_modified(ed: &mut Editor) {
    if let Some(b) = bm_entry(ed) {
        ed.bufm(b).modified = false;
        bm_set_flag(ed, 2, ' ', true);
    }
}
pub fn bm_toggle_ro(ed: &mut Editor) {
    if let Some(b) = bm_entry(ed) {
        let ro = !ed.buf(b).read_only;
        ed.bufm(b).read_only = ro;
        bm_set_flag(ed, 1, if ro { '%' } else { ' ' }, true);
    }
}

pub fn quit_window(ed: &mut Editor) {
    let b = ed.cur();
    let special = ed.buf(b).kind != BufKind::Normal;
    if special && ed.windows().len() > 1 {
        let w = ed.sel;
        ed.delete_window(w);
    } else {
        let o = ed.other_buffer(b);
        ed.switch_to(o);
    }
}

// ======================= dired =======================

fn fmt_mode(m: u32, is_dir: bool, is_link: bool) -> String {
    let t = if is_link {
        'l'
    } else if is_dir {
        'd'
    } else {
        '-'
    };
    let mut s = String::from(t);
    for shift in [6, 3, 0] {
        let v = (m >> shift) & 7;
        s.push(if v & 4 != 0 { 'r' } else { '-' });
        s.push(if v & 2 != 0 { 'w' } else { '-' });
        s.push(if v & 1 != 0 { 'x' } else { '-' });
    }
    s
}

fn fill_dired(ed: &mut Editor, bid: BufId, dir: &Path) {
    use std::os::unix::fs::MetadataExt;
    let mut names: Vec<(String, PathBuf)> = vec![
        (".".into(), dir.to_path_buf()),
        ("..".into(), dir.parent().unwrap_or(dir).to_path_buf()),
    ];
    match std::fs::read_dir(dir) {
        Ok(rd) => {
            let mut v: Vec<(String, PathBuf)> = rd
                .flatten()
                .map(|e| (e.file_name().to_string_lossy().to_string(), e.path()))
                .collect();
            v.sort_by(|a, b| a.0.cmp(&b.0));
            names.extend(v);
        }
        Err(e) => {
            ed.error(format!("Reading directory: {e}"));
        }
    }
    struct Row {
        mode: String,
        nlink: u64,
        owner: String,
        group: String,
        size: u64,
        date: String,
        name: String,
        link: Option<String>,
        path: PathBuf,
    }
    let mut rows = Vec::new();
    for (name, path) in names {
        match std::fs::symlink_metadata(&path) {
            Ok(m) => {
                let is_link = m.file_type().is_symlink();
                let is_dir = path.is_dir();
                let mtime = m
                    .modified()
                    .ok()
                    .map(chrono::DateTime::<chrono::Local>::from);
                rows.push(Row {
                    mode: fmt_mode(m.mode(), is_dir && !is_link, is_link),
                    nlink: m.nlink(),
                    owner: user_name(m.uid()),
                    group: group_name(m.gid()),
                    size: m.size(),
                    date: mtime
                        .map(|d| d.format("%b %e %H:%M").to_string())
                        .unwrap_or_default(),
                    name,
                    link: if is_link {
                        std::fs::read_link(&path)
                            .ok()
                            .map(|t| t.display().to_string())
                    } else {
                        None
                    },
                    path,
                });
            }
            Err(_) => rows.push(Row {
                mode: "??????????".into(),
                nlink: 0,
                owner: "?".into(),
                group: "?".into(),
                size: 0,
                date: String::new(),
                name,
                link: None,
                path,
            }),
        }
    }
    let wo = rows.iter().map(|r| r.owner.len()).max().unwrap_or(1);
    let wg = rows.iter().map(|r| r.group.len()).max().unwrap_or(1);
    let ws = rows
        .iter()
        .map(|r| r.size.to_string().len())
        .max()
        .unwrap_or(1);
    let wn = rows
        .iter()
        .map(|r| r.nlink.to_string().len())
        .max()
        .unwrap_or(1);
    let mut lines = vec![format!("  {}:", dir.display())];
    let mut entries = vec![String::new()];
    let mut cols = vec![0];
    for r in rows {
        let prefix = format!(
            "  {} {:>wn$} {:<wo$} {:<wg$} {:>ws$} {} ",
            r.mode, r.nlink, r.owner, r.group, r.size, r.date
        );
        cols.push(prefix.chars().count());
        let mut l = prefix + &r.name;
        if let Some(t) = r.link {
            l.push_str(&format!(" -> {t}"));
        }
        lines.push(l);
        entries.push(r.path.to_string_lossy().to_string());
    }
    let text = lines.join("\n") + "\n";
    let b = ed.bufm(bid);
    b.read_only = false;
    b.undo_disabled = true;
    b.set_text(&text);
    b.undo_disabled = false;
    b.clear_undo();
    b.read_only = true;
    b.modified = false;
    b.entries = entries;
    b.entry_cols = cols;
    b.dir = dir.to_path_buf();
}

pub fn dired_buffer(ed: &mut Editor, dir: &Path) -> BufId {
    for b in ed.live_buffers() {
        if ed.buf(b).kind == BufKind::Dired && ed.buf(b).dir == dir {
            fill_dired(ed, b, dir);
            return b;
        }
    }
    let name = dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "/".into());
    let mut b = Buffer::new(&name, dir.to_path_buf());
    b.kind = BufKind::Dired;
    b.path = None;
    let id = ed.add_buffer(b);
    fill_dired(ed, id, dir);
    let p = ed.buf(id).line_start(3.min(ed.buf(id).line_count() - 1));
    let pm = ed.buf(id).pt;
    ed.bufm(id).set_mpos(pm, p);
    id
}

fn dired_entry(ed: &Editor) -> Option<PathBuf> {
    let line = ed.b().line_of(ed.pt());
    let e = ed.b().entries.get(line)?;
    if e.is_empty() {
        None
    } else {
        Some(PathBuf::from(e))
    }
}

fn dired_move_to_filename(ed: &mut Editor) {
    let line = ed.b().line_of(ed.pt());
    if let Some(&c) = ed.b().entry_cols.get(line) {
        if c > 0 {
            let p = ed.b().line_start(line) + c;
            ed.goto(p);
        }
    }
}

fn user_name(uid: u32) -> String {
    // SAFETY: getpwuid returns a pointer to static storage or null.
    unsafe {
        let pw = libc::getpwuid(uid);
        if pw.is_null() {
            return uid.to_string();
        }
        std::ffi::CStr::from_ptr((*pw).pw_name)
            .to_string_lossy()
            .into_owned()
    }
}

fn group_name(gid: u32) -> String {
    // SAFETY: getgrgid returns a pointer to static storage or null.
    unsafe {
        let gr = libc::getgrgid(gid);
        if gr.is_null() {
            return gid.to_string();
        }
        std::ffi::CStr::from_ptr((*gr).gr_name)
            .to_string_lossy()
            .into_owned()
    }
}

pub fn dired_find_file(ed: &mut Editor) {
    if let Some(p) = dired_entry(ed) {
        let p = normalize(&p);
        open_path(ed, &p, None);
        if ed.b().kind == BufKind::Dired {
            dired_move_to_filename(ed);
        }
    }
}
pub fn dired_find_file_other_window(ed: &mut Editor) {
    if let Some(p) = dired_entry(ed) {
        match visit(ed, &normalize(&p)) {
            Ok(b) => ed.display_buffer_other(b, true),
            Err(e) => ed.error(e),
        }
    }
}
pub fn dired_view_file(ed: &mut Editor) {
    dired_find_file(ed);
    if ed.b().kind == BufKind::Normal {
        ed.bm().read_only = true;
    }
}
pub fn dired_up_directory(ed: &mut Editor) {
    let dir = ed.b().dir.clone();
    if let Some(parent) = dir.parent() {
        let b = dired_buffer(ed, parent);
        ed.switch_to(b);
        // put point on the directory we came from
        let target = dir.to_string_lossy().to_string();
        if let Some(l) = ed.buf(b).entries.iter().position(|e| *e == target) {
            let p = ed.buf(b).line_start(l);
            ed.goto(p);
            dired_move_to_filename(ed);
        }
    }
}
pub fn dired_next_line(ed: &mut Editor) {
    commands::vertical(ed, ed.count());
    dired_move_to_filename(ed);
}
pub fn dired_previous_line(ed: &mut Editor) {
    commands::vertical(ed, -ed.count());
    dired_move_to_filename(ed);
}
pub fn dired_flag(ed: &mut Editor) {
    if dired_entry(ed).is_some() {
        bm_set_flag(ed, 0, 'D', true);
        dired_move_to_filename(ed);
    }
}
pub fn dired_unmark(ed: &mut Editor) {
    if dired_entry(ed).is_some() {
        bm_set_flag(ed, 0, ' ', true);
        dired_move_to_filename(ed);
    }
}
pub fn dired_do_delete(ed: &mut Editor) {
    let mut paths = Vec::new();
    for l in 0..ed.b().line_count() {
        if ed.b().line_text(l).starts_with('D') {
            if let Some(e) = ed.b().entries.get(l) {
                paths.push(PathBuf::from(e));
            }
        }
    }
    if paths.is_empty() {
        ed.msg("(No deletions requested)");
        return;
    }
    let names: Vec<String> = paths
        .iter()
        .map(|p| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        })
        .collect();
    ed.yes_or_no(
        format!("Delete {} ", names.join(" ")),
        YnAction::DiredDelete(paths),
    );
}
pub fn dired_create_directory(ed: &mut Editor) {
    let d = dir_prompt_string(&ed.b().dir.clone());
    ed.read_string(
        "Create directory: ",
        PromptKind::DiredCreateDir,
        &d,
        Completion::File,
        "file",
    );
}
pub fn dired_rename(ed: &mut Editor) {
    let Some(p) = dired_entry(ed) else { return };
    let d = dir_prompt_string(&ed.b().dir.clone());
    let n = p
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    ed.read_string(
        &format!("Rename {n} to: "),
        PromptKind::DiredRename(p),
        &d,
        Completion::File,
        "file",
    );
}
pub fn dired_copy(ed: &mut Editor) {
    let Some(p) = dired_entry(ed) else { return };
    let d = dir_prompt_string(&ed.b().dir.clone());
    let n = p
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    ed.read_string(
        &format!("Copy {n} to: "),
        PromptKind::DiredCopy(p),
        &d,
        Completion::File,
        "file",
    );
}

// ======================= help =======================

pub fn describe_bindings(ed: &mut Editor) {
    let mut out = String::from(
        "Key translations / bindings:\n\nkey             binding\n---             -------\n\n",
    );
    let local = match ed.b().kind {
        BufKind::BufferList => Some(("Buffer-menu-mode", &ed.buflist_map)),
        BufKind::Dired => Some(("dired-mode", &ed.dired_map)),
        BufKind::Special => Some(("special-mode", &ed.special_map)),
        _ => None,
    };
    if let Some((name, m)) = local {
        out.push_str(&format!("Major Mode Bindings ({name}):\n"));
        for (k, c) in m.flatten() {
            out.push_str(&format!("{:<24}{c}\n", crate::key::seq_to_string(&k)));
        }
        out.push('\n');
    }
    out.push_str("Global Bindings:\n");
    for (k, c) in ed.global.flatten() {
        out.push_str(&format!("{:<24}{c}\n", crate::key::seq_to_string(&k)));
    }
    show_help(ed, &out);
}

pub fn describe_mode(ed: &mut Editor) {
    let l = ed.lang().clone();
    let fmt = l
        .formatter
        .as_ref()
        .map(|f| f.join(" "))
        .unwrap_or_else(|| "none".into());
    let text = format!(
        "{} mode\n\nindent: {}  tabs: {}  tab-width: {}\nindent style: {:?}\ncomment: {} {}\nformatter: {}\nformat on save: {}\nsyntax: {}\n",
        l.mode_name,
        l.indent,
        l.tabs,
        l.tab_width,
        l.style,
        l.comment_start,
        l.comment_end,
        fmt,
        l.format_on_save && ed.cfg.format_on_save,
        l.syntax.unwrap_or_else(|| "none".into())
    );
    show_help(ed, &text);
}

pub fn view_messages(ed: &mut Editor) {
    let text = ed.messages.join("\n") + "\n";
    let b = special_buffer(ed, "*Messages*", &text, BufKind::Special);
    ed.display_buffer_other(b, true);
    let n = ed.b().len();
    ed.goto(n);
}

// ======================= shell =======================

pub fn shell_command(ed: &mut Editor) {
    let arg = ed.arg.is_some();
    SHELL_ARG.with(|s| *s.borrow_mut() = arg);
    ed.read_string(
        "Shell command: ",
        PromptKind::ShellCommand,
        "",
        Completion::None,
        "shell",
    );
}

pub fn shell_command_on_region(ed: &mut Editor) {
    if commands::region(ed).is_none() {
        return;
    }
    let replace = ed.arg.is_some();
    ed.read_string(
        "Shell command on region: ",
        PromptKind::ShellCommandRegion { replace },
        "",
        Completion::None,
        "shell",
    );
}

thread_local! {
    static SHELL_ARG: std::cell::RefCell<bool> = const { std::cell::RefCell::new(false) };
}

fn run_shell(dir: &Path, cmd: &str, input: Option<&str>) -> (String, bool) {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let child = Command::new(shell)
        .arg("-c")
        .arg(cmd)
        .current_dir(if dir.is_dir() {
            dir.to_path_buf()
        } else {
            home()
        })
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => return (e.to_string(), false),
    };
    if let Some(i) = input {
        let mut stdin = child.stdin.take().unwrap();
        let i = i.to_string();
        std::thread::spawn(move || {
            let _ = stdin.write_all(i.as_bytes());
        });
    }
    match child.wait_with_output() {
        Ok(o) => {
            let mut s = String::from_utf8_lossy(&o.stdout).to_string();
            s.push_str(&String::from_utf8_lossy(&o.stderr));
            (s, o.status.success())
        }
        Err(e) => (e.to_string(), false),
    }
}

fn show_shell_output(ed: &mut Editor, out: &str, ok: bool) {
    let trimmed = out.trim_end_matches('\n');
    if trimmed.is_empty() {
        ed.msg(if ok {
            "(Shell command succeeded with no output)"
        } else {
            "(Shell command failed with no output)"
        });
    } else if !trimmed.contains('\n') {
        ed.msg(trimmed.to_string());
    } else {
        let b = special_buffer(ed, "*Shell Command Output*", out, BufKind::Special);
        if !ed.windows().iter().any(|&w| ed.win(w).buf == b) {
            ed.display_buffer_other(b, false);
        }
        for w in ed.windows() {
            if ed.win(w).buf == b {
                crate::view::set_win_point(ed, w, 0);
            }
        }
    }
}

// ======================= bookmarks =======================

fn bookmark_file() -> PathBuf {
    crate::config::config_path().with_file_name("bookmarks")
}

pub fn load_bookmarks(ed: &mut Editor) {
    if let Ok(s) = std::fs::read_to_string(bookmark_file()) {
        for l in s.lines() {
            let mut it = l.splitn(3, '\t');
            if let (Some(n), Some(p), Some(f)) = (it.next(), it.next(), it.next()) {
                if let Ok(p) = p.parse() {
                    ed.bookmarks.insert(n.to_string(), (PathBuf::from(f), p));
                }
            }
        }
    }
}

fn save_bookmarks(ed: &Editor) {
    let mut s = String::new();
    let mut v: Vec<_> = ed.bookmarks.iter().collect();
    v.sort();
    for (n, (f, p)) in v {
        s.push_str(&format!("{n}\t{p}\t{}\n", f.display()));
    }
    let f = bookmark_file();
    if let Some(d) = f.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(f, s);
}

pub fn bookmark_set(ed: &mut Editor) {
    let def = ed.b().name.clone();
    ed.read_string_default(
        &format!("Set bookmark named (default {def}): "),
        PromptKind::BookmarkSet,
        "",
        Completion::None,
        "bookmark",
        Some(def),
    );
}

pub fn bookmark_jump(ed: &mut Editor) {
    let mut names: Vec<String> = ed.bookmarks.keys().cloned().collect();
    names.sort();
    ed.read_string(
        "Jump to bookmark: ",
        PromptKind::BookmarkJump,
        "",
        Completion::List(names),
        "bookmark",
    );
}

pub fn bookmark_list(ed: &mut Editor) {
    let mut v: Vec<_> = ed
        .bookmarks
        .iter()
        .map(|(n, (f, p))| format!("{n:<24} {}:{p}", abbreviate(f)))
        .collect();
    v.sort();
    show_help(ed, &(String::from("Bookmarks:\n\n") + &v.join("\n") + "\n"));
}

// ======================= minibuffer =======================

pub fn exit_minibuffer(ed: &mut Editor) {
    let Some(m) = ed.mini.as_ref() else { return };
    let mut text = ed.mini_text();
    // Completion requirement for commands
    if matches!(m.comp, Completion::Command)
        && !text.is_empty()
        && commands::lookup(&text).is_none()
    {
        let cands = candidates(ed, &text);
        if cands.len() == 1 {
            text = cands[0].clone();
        } else {
            ed.message = Some(if cands.is_empty() {
                " [No match]".into()
            } else {
                " [Incomplete command]".into()
            });
            return;
        }
    }
    let m = ed.mini_exit().unwrap();
    let value = if text.is_empty() {
        m.default.clone().unwrap_or_default()
    } else {
        text.clone()
    };
    ed.push_history(m.hist, if text.is_empty() { &value } else { &text });
    accept(ed, m, value);
}

fn accept(ed: &mut Editor, m: Mini, v: String) {
    let dir = ed.b().dir.clone();
    match m.kind {
        PromptKind::FindFile { other, read_only } => {
            let p = expand(&v, &dir);
            match visit(ed, &p) {
                Ok(b) => {
                    if other {
                        ed.display_buffer_other(b, true);
                    } else {
                        ed.switch_to(b);
                    }
                    if read_only {
                        ed.bm().read_only = true;
                    }
                    if ed.b().kind == BufKind::Dired {
                        dired_move_to_filename(ed);
                    }
                }
                Err(e) => ed.error(e),
            }
        }
        PromptKind::FindAlternate => {
            let old = ed.cur();
            let p = expand(&v, &dir);
            match visit(ed, &p) {
                Ok(b) => {
                    ed.switch_to(b);
                    if old != b {
                        ed.kill_buffer(old);
                    }
                }
                Err(e) => ed.error(e),
            }
        }
        PromptKind::InsertFile => {
            let p = expand(&v, &dir);
            match read_file(&p) {
                Ok((s, _, _)) => {
                    let pt = ed.pt();
                    ed.bm().insert(pt, &s);
                    let e = pt + s.chars().count();
                    ed.bm().push_mark(e, false);
                    ed.goto(pt);
                }
                Err(e) => ed.error(format!("Opening input file: {e}")),
            }
        }
        PromptKind::WriteFile => {
            let mut p = expand(&v, &dir);
            if p.is_dir() {
                p = p.join(&ed.b().name);
            }
            let name = buffer_name_for(ed, &p);
            let b = ed.cur();
            {
                let buf = ed.bufm(b);
                buf.path = Some(p.clone());
                buf.dir = p.parent().map(|d| d.to_path_buf()).unwrap_or(dir);
                buf.mtime = None;
                buf.kind = BufKind::Normal;
                buf.modified = true;
            }
            if ed.buf(b).name != name {
                ed.bufm(b).name = name;
            }
            set_mode(ed, b);
            save_buffer_id(ed, b, false);
        }
        PromptKind::SwitchBuffer { other } => {
            let b = match ed.find_buffer(&v) {
                Some(b) => b,
                None => {
                    let nb = Buffer::new(&v, dir);
                    ed.add_buffer(nb)
                }
            };
            if other {
                ed.display_buffer_other(b, true);
            } else {
                ed.switch_to(b);
            }
        }
        PromptKind::KillBuffer => match ed.find_buffer(&v) {
            Some(b) => kill_buffer_maybe(ed, b),
            None => ed.error(format!("No such buffer {v}")),
        },
        PromptKind::MX => {
            if v.is_empty() {
                return;
            }
            let (arg, uni, dig) = commands::MX_ARG.with(|m| *m.borrow());
            ed.arg = arg;
            ed.arg_universal = uni;
            ed.arg_digits = dig;
            ed.run_command(&v);
            // Suggest key binding like Emacs.
            if ed.message.is_none() && !ed.errored {
                let keys: Vec<String> = ed
                    .global
                    .flatten()
                    .into_iter()
                    .filter(|(_, c)| *c == v)
                    .map(|(k, _)| crate::key::seq_to_string(&k))
                    .collect();
                if let Some(k) = keys.first() {
                    ed.msg(format!("You can run the command `{v}' with {k}"));
                }
            }
        }
        PromptKind::GotoLine => match v.trim().parse::<i64>() {
            Ok(n) => commands::goto_line_n(ed, n),
            Err(_) => ed.error("Invalid line number"),
        },
        PromptKind::GotoChar => match v.trim().parse::<i64>() {
            Ok(n) => ed.goto((n.max(1) - 1) as usize),
            Err(_) => ed.error("Invalid position"),
        },
        PromptKind::QueryReplaceFrom { regex, query } => {
            crate::search::replace_from_entered(ed, v, regex, query)
        }
        PromptKind::QueryReplaceTo { regex, query, from } => {
            crate::search::start_replace(ed, from, v, regex, query)
        }
        PromptKind::ShellCommand => {
            let insert = SHELL_ARG.with(|s| *s.borrow());
            let (out, ok) = run_shell(&dir, &v, None);
            if insert {
                if ed.check_writable() {
                    let p = ed.pt();
                    ed.bm().insert(p, &out);
                    ed.bm().push_mark(p + out.chars().count(), false);
                }
            } else {
                show_shell_output(ed, &out, ok);
            }
        }
        PromptKind::ShellCommandRegion { replace } => {
            let Some((a, b)) = commands::region(ed) else {
                return;
            };
            let input = ed.b().slice(a, b);
            let (out, ok) = run_shell(&dir, &v, Some(&input));
            if replace {
                if ed.check_writable() {
                    ed.delete(a, b);
                    ed.bm().insert(a, &out);
                    ed.goto(a);
                    ed.bm().push_mark(a + out.chars().count(), false);
                }
            } else {
                show_shell_output(ed, &out, ok);
            }
        }
        PromptKind::StringRectangle => commands::string_rectangle_apply(ed, &v),
        PromptKind::RenameBuffer => {
            if v.is_empty() {
                ed.error("Empty string is invalid as a buffer name");
            } else if ed.find_buffer(&v).is_some_and(|b| b != ed.cur()) {
                ed.error(format!("Buffer name `{v}' is in use"));
            } else {
                ed.bm().name = v;
            }
        }
        PromptKind::Dired { other } => {
            let p = expand(&v, &dir);
            if !p.is_dir() {
                ed.error(format!("{} is not a directory", p.display()));
                return;
            }
            let b = dired_buffer(ed, &p);
            if other {
                ed.display_buffer_other(b, true);
            } else {
                ed.switch_to(b);
            }
            dired_move_to_filename(ed);
        }
        PromptKind::DescribeFunction => {
            let text = format!("{v} is an interactive command.\n\n{}\n", commands::doc(&v));
            show_help(ed, &text);
        }
        PromptKind::WhereIs => commands::where_is_msg(ed, &v),
        PromptKind::Occur => crate::search::occur_run(ed, &v),
        PromptKind::HighlightRegexp => crate::search::highlight_apply(ed, &v),
        PromptKind::FlushLines { keep } => commands::flush_lines_apply(ed, &v, keep),
        PromptKind::DiredCreateDir => {
            let p = expand(&v, &dir);
            match std::fs::create_dir_all(&p) {
                Ok(()) => revert_buffer_now(ed),
                Err(e) => ed.error(format!("Creating directory: {e}")),
            }
        }
        PromptKind::DiredRename(from) => {
            let mut to = expand(&v, &dir);
            if to.is_dir() {
                to = to.join(from.file_name().unwrap_or_default());
            }
            match std::fs::rename(&from, &to) {
                Ok(()) => {
                    for b in ed.live_buffers() {
                        if ed.buf(b).path.as_deref() == Some(from.as_path()) {
                            ed.bufm(b).path = Some(to.clone());
                        }
                    }
                    revert_buffer_now(ed);
                }
                Err(e) => ed.error(format!("Renaming: {e}")),
            }
        }
        PromptKind::DiredCopy(from) => {
            let mut to = expand(&v, &dir);
            if to.is_dir() {
                to = to.join(from.file_name().unwrap_or_default());
            }
            match std::fs::copy(&from, &to) {
                Ok(_) => revert_buffer_now(ed),
                Err(e) => ed.error(format!("Copying: {e}")),
            }
        }
        PromptKind::BookmarkSet => {
            let Some(p) = ed.b().path.clone() else {
                ed.error("Buffer not visiting a file");
                return;
            };
            let pos = ed.pt();
            ed.bookmarks.insert(v, (p, pos));
            save_bookmarks(ed);
        }
        PromptKind::BookmarkJump => match ed.bookmarks.get(&v).cloned() {
            Some((f, p)) => {
                open_path(ed, &f, None);
                ed.goto(p);
            }
            None => ed.error(format!("Invalid bookmark {v}")),
        },
    }
}

/// Completion candidates for the current minibuffer input.
pub fn candidates(ed: &Editor, input: &str) -> Vec<String> {
    let Some(m) = &ed.mini else { return Vec::new() };
    match &m.comp {
        Completion::None => Vec::new(),
        Completion::Command => commands::names()
            .into_iter()
            .filter(|n| n.starts_with(input))
            .collect(),
        Completion::Buffer => {
            let mut v: Vec<String> = ed
                .live_buffers()
                .into_iter()
                .map(|b| ed.buf(b).name.clone())
                .filter(|n| n.starts_with(input) && !n.starts_with(' '))
                .collect();
            v.sort();
            v
        }
        Completion::List(l) => l.iter().filter(|n| n.starts_with(input)).cloned().collect(),
        Completion::File => {
            let dir = ed.buf(ed.win(m.ret_win).buf).dir.clone();
            let (dpart, fpart) = match input.rfind('/') {
                Some(i) => (&input[..=i], &input[i + 1..]),
                None => ("", input),
            };
            let d = if dpart.is_empty() {
                dir
            } else {
                expand(dpart, &dir)
            };
            let mut v: Vec<String> = match std::fs::read_dir(&d) {
                Ok(rd) => rd
                    .flatten()
                    .filter_map(|e| {
                        let n = e.file_name().to_string_lossy().to_string();
                        if !n.starts_with(fpart) {
                            return None;
                        }
                        let is_dir = e.path().is_dir();
                        Some(format!("{dpart}{n}{}", if is_dir { "/" } else { "" }))
                    })
                    .collect(),
                Err(_) => Vec::new(),
            };
            v.sort();
            v
        }
    }
}

fn common_prefix(v: &[String]) -> String {
    let Some(first) = v.first() else {
        return String::new();
    };
    let mut n = first.len();
    for s in v.iter().skip(1) {
        n = n.min(
            first
                .bytes()
                .zip(s.bytes())
                .take_while(|(a, b)| a == b)
                .count(),
        );
    }
    while !first.is_char_boundary(n) {
        n -= 1;
    }
    first[..n].to_string()
}

pub fn minibuffer_complete(ed: &mut Editor) {
    if ed.mini.is_none() {
        return;
    }
    if matches!(ed.mini.as_ref().unwrap().comp, Completion::None) {
        ed.insert("\t");
        return;
    }
    let input = ed.mini_text();
    let c = candidates(ed, &input);
    if c.is_empty() {
        ed.message = Some(" [No match]".into());
        ed.ding();
        return;
    }
    let pre = common_prefix(&c);
    if pre.len() > input.len() {
        ed.mini_set_text(&pre);
        if c.len() == 1 {
            ed.completions = None;
        }
        return;
    }
    if c.len() == 1 {
        ed.message = Some(" [Sole completion]".into());
        ed.completions = None;
        return;
    }
    let names: Vec<String> = c
        .iter()
        .map(|s| {
            s.trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or(s)
                .to_string()
                + if s.ends_with('/') { "/" } else { "" }
        })
        .collect();
    ed.completions = Some(names);
}

pub fn minibuffer_completion_help(ed: &mut Editor) {
    let comp = ed.mini.as_ref().map(|m| m.comp.clone());
    match comp {
        Some(Completion::None) | None => ed.insert("?"),
        Some(_) => {
            let input = ed.mini_text();
            let c = candidates(ed, &input);
            ed.completions = Some(
                c.iter()
                    .map(|s| {
                        s.trim_end_matches('/')
                            .rsplit('/')
                            .next()
                            .unwrap_or(s)
                            .to_string()
                    })
                    .collect(),
            );
        }
    }
}

fn history_step(ed: &mut Editor, dir: i64) {
    let Some(m) = ed.mini.as_ref() else { return };
    let h = ed.hist.get(m.hist).cloned().unwrap_or_default();
    let idx = m.hist_idx;
    let next = match (idx, dir) {
        (None, 1) => {
            // M-n with no history position: default value (future history)
            if let Some(d) = m.default.clone() {
                ed.mini_set_text(&d);
            } else {
                ed.error("End of history; no default available");
            }
            return;
        }
        (None, _) => 0i64,
        (Some(i), d) => i as i64 - d,
    };
    if next < 0 {
        let saved = ed.mini.as_ref().unwrap().saved_input.clone();
        ed.mini.as_mut().unwrap().hist_idx = None;
        ed.mini_set_text(&saved);
        return;
    }
    if next as usize >= h.len() {
        ed.error("Beginning of history; no preceding item");
        return;
    }
    if idx.is_none() {
        let t = ed.mini_text();
        ed.mini.as_mut().unwrap().saved_input = t;
    }
    ed.mini.as_mut().unwrap().hist_idx = Some(next as usize);
    ed.mini_set_text(&h[next as usize]);
}

pub fn previous_history(ed: &mut Editor) {
    history_step(ed, -1);
}
pub fn next_history(ed: &mut Editor) {
    history_step(ed, 1);
}

pub fn _abort(ed: &mut Editor) {
    abort_minibuffer(ed);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expand_paths() {
        let d = PathBuf::from("/a/b");
        assert_eq!(expand("c.txt", &d), PathBuf::from("/a/b/c.txt"));
        assert_eq!(expand("/a/b//etc/hosts", &d), PathBuf::from("/etc/hosts"));
        assert_eq!(expand("/a/b/~/x", &d), home().join("x"));
        assert_eq!(expand("../c", &d), PathBuf::from("/a/c"));
    }
}
