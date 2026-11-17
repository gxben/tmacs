//! tmacs — a lightweight terminal Emacs replacement.

#![allow(clippy::collapsible_if, clippy::rc_clone_in_vec_init)]

mod buffer;
mod clipboard;
mod color;
mod commands;
mod config;
mod editor;
mod face;
mod files;
mod key;
mod keymap;
mod lang;
mod render;
mod screen;
mod search;
mod syntax;
mod text;
mod view;

use crossterm::event::{
    self, Event, KeyboardEnhancementFlags, MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::{cursor, execute, terminal};
use editor::{Editor, MINI};
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const USAGE: &str = "Usage: tmacs [OPTION]... [+LINE[:COLUMN]] [FILE]...

  -q, --no-init-file   do not load ~/.config/tmacs/init.toml
  -nw                  ignored (tmacs is always terminal-only)
  --version            print version
  -h, --help           this help
";

struct Term {
    kitty: bool,
    mouse: bool,
    cursor_color: Option<String>,
}

impl Term {
    fn setup(&self) -> io::Result<()> {
        let mut out = io::stdout();
        terminal::enable_raw_mode()?;
        execute!(
            out,
            terminal::EnterAlternateScreen,
            event::EnableBracketedPaste
        )?;
        if self.mouse {
            // Button + drag tracking with SGR encoding (no any-motion events).
            out.write_all(b"\x1b[?1000h\x1b[?1002h\x1b[?1006h")?;
        }
        if self.kitty {
            execute!(
                out,
                event::PushKeyboardEnhancementFlags(
                    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                )
            )?;
        }
        if let Some(c) = &self.cursor_color {
            if let Some(rgb) = color::parse(c) {
                write!(out, "\x1b]12;#{:02x}{:02x}{:02x}\x07", rgb.0, rgb.1, rgb.2)?;
            }
        }
        out.flush()
    }

    fn restore(&self) {
        let mut out = io::stdout();
        if self.kitty {
            let _ = execute!(out, event::PopKeyboardEnhancementFlags);
        }
        if self.mouse {
            let _ = out.write_all(b"\x1b[?1006l\x1b[?1002l\x1b[?1000l");
        }
        if self.cursor_color.is_some() {
            let _ = out.write_all(b"\x1b]112\x07");
        }
        let _ = execute!(
            out,
            event::DisableBracketedPaste,
            terminal::LeaveAlternateScreen,
            cursor::Show
        );
        let _ = terminal::disable_raw_mode();
        let _ = out.flush();
    }
}

#[derive(Default)]
struct Mouse {
    drag: Option<(usize, usize)>,
    last_click: Option<(Instant, u16, u16, u8)>,
}

impl Mouse {
    fn handle(&mut self, ed: &mut Editor, ev: MouseEvent) {
        let lines = ed.cfg.mouse_wheel_lines.max(1) as isize;
        match ev.kind {
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                if let Some((wid, _, _)) = render::pos_at_screen(ed, ev.column, ev.row) {
                    let n = if ev.kind == MouseEventKind::ScrollDown {
                        lines
                    } else {
                        -lines
                    };
                    view::scroll_rows(ed, wid, n);
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let Some((wid, pos, _)) = render::pos_at_screen(ed, ev.column, ev.row) else {
                    return;
                };
                if ed.mini.is_some() && ed.sel == MINI {
                    return;
                }
                if ed.isearch.is_some() {
                    search::isearch_exit(ed);
                }
                ed.sel = wid;
                let Some(p) = pos else { return };
                let count = match self.last_click {
                    Some((t, x, y, n))
                        if t.elapsed() < Duration::from_millis(400)
                            && x == ev.column
                            && y == ev.row =>
                    {
                        n % 3 + 1
                    }
                    _ => 1,
                };
                self.last_click = Some((Instant::now(), ev.column, ev.row, count));
                ed.bm().mark_active = false;
                ed.goto(p);
                match count {
                    2 => {
                        let b = ed.b();
                        let s = text::backward_word(b, (p + 1).min(b.len()));
                        let e = text::forward_word(b, s);
                        ed.bm().push_mark(s, true);
                        ed.goto(e);
                    }
                    3 => {
                        let b = ed.b();
                        let s = b.bol(p);
                        let e = (b.eol(p) + 1).min(b.zv());
                        ed.bm().push_mark(s, true);
                        ed.goto(e);
                    }
                    _ => self.drag = Some((wid, p)),
                }
                ed.message = None;
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                let Some((wid, start)) = self.drag else {
                    return;
                };
                let Some((w2, pos, _)) = render::pos_at_screen(ed, ev.column, ev.row) else {
                    return;
                };
                if w2 != wid {
                    return;
                }
                let Some(p) = pos else { return };
                if !ed.b().mark_active {
                    ed.bm().push_mark(start, true);
                }
                ed.goto(p);
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if let Some((_, start)) = self.drag.take() {
                    if ed.pt() == start && ed.b().mark_active {
                        ed.bm().mark_active = false;
                    }
                }
            }
            _ => {}
        }
    }
}

fn frame_title(ed: &Editor, fmt: &str) -> String {
    let b = ed.buf(
        ed.win(if ed.sel == MINI {
            ed.mini.as_ref().map(|m| m.ret_win).unwrap_or(1)
        } else {
            ed.sel
        })
        .buf,
    );
    let name = b.name.clone();
    match &b.path {
        Some(p) => fmt
            .replace("%b", &name)
            .replace("%f", &p.display().to_string()),
        None => fmt
            .replace(" (%f)", "")
            .replace("%f", "")
            .replace("%b", &name),
    }
}

fn main() {
    let mut files: Vec<(PathBuf, Option<usize>, Option<usize>)> = Vec::new();
    let mut pending_line: Option<(usize, Option<usize>)> = None;
    let mut no_init = false;
    for a in std::env::args().skip(1) {
        match a.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return;
            }
            "--version" => {
                println!("tmacs {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "-q" | "--no-init-file" | "-Q" => no_init = true,
            "-nw" | "--no-window-system" => {}
            s if s.starts_with('+') => {
                let mut it = s[1..].splitn(2, ':');
                let l = it.next().and_then(|x| x.parse().ok()).unwrap_or(1);
                let c = it.next().and_then(|x| x.parse().ok());
                pending_line = Some((l, c));
            }
            s => {
                let (l, c) = match pending_line.take() {
                    Some((l, c)) => (Some(l), c),
                    None => (None, None),
                };
                files.push((PathBuf::from(s), l, c));
            }
        }
    }

    let (config, err) = if no_init {
        (config::Config::default(), None)
    } else {
        config::load()
    };
    syntax::preload();
    let mut ed = Editor::new(config, err);
    files::load_bookmarks(&mut ed);
    let cwd = std::env::current_dir().unwrap_or_default();
    let n_files = files.len();
    for (i, (p, line, col)) in files.iter().enumerate() {
        let p = files::expand(&p.to_string_lossy(), &cwd);
        files::open_path(&mut ed, &p, *line);
        if let Some(c) = col {
            let tw = ed.tab_width();
            let pt = ed.pt();
            let np = ed.b().pos_at_column(pt, c.saturating_sub(1), tw);
            ed.goto(np);
        }
        if n_files == 2 && i == 0 {
            ed.split_window(false);
            let w = ed.windows()[1];
            ed.sel = w;
        }
    }
    if n_files == 0 {
        ed.message = None;
    }
    if let Some(e) = ed.init_error.take() {
        ed.error(e);
    }

    let kitty = ed.cfg.kitty_keyboard && terminal::supports_keyboard_enhancement().unwrap_or(false);
    let term = Term {
        kitty,
        mouse: ed.cfg.xterm_mouse_mode,
        cursor_color: ed.cfg.cursor_color.clone(),
    };

    let default_hook = std::panic::take_hook();
    let mouse_on = term.mouse;
    let kitty_on = term.kitty;
    std::panic::set_hook(Box::new(move |info| {
        Term {
            kitty: kitty_on,
            mouse: mouse_on,
            cursor_color: Some(String::new()),
        }
        .restore();
        default_hook(info);
    }));

    if let Err(e) = term.setup() {
        term.restore();
        eprintln!("tmacs: cannot initialize terminal: {e}");
        std::process::exit(1);
    }
    let (w, h) = terminal::size().unwrap_or((80, 24));
    let mut scr = screen::Screen::new(w, h, ed.faces.default);
    ed.resize(w, h);
    let mut out = io::BufWriter::with_capacity(1 << 16, io::stdout());
    let mut mouse = Mouse::default();
    let mut title = String::new();
    let title_fmt = ed.cfg.frame_title_format.clone();

    let result: io::Result<()> = (|| {
        loop {
            ed.tick();
            if ed.quit {
                break;
            }
            if ed.suspend {
                ed.suspend = false;
                term.restore();
                unsafe {
                    libc::raise(libc::SIGTSTP);
                }
                term.setup()?;
                scr.invalidate();
                title.clear();
            }
            render::redisplay(&mut ed, &mut scr);
            scr.flush(&mut out)?;
            if !title_fmt.is_empty() {
                let t = frame_title(&ed, &title_fmt);
                if t != title {
                    execute!(out, terminal::SetTitle(&t))?;
                    title = t;
                }
            }
            let pending = ed.fontify_pending();
            let timeout = if pending {
                Duration::ZERO
            } else {
                ed.next_timeout()
            };
            if !event::poll(timeout)? {
                if pending {
                    ed.fontify_step(Duration::from_millis(30));
                }
                continue;
            }
            loop {
                match event::read()? {
                    Event::Key(k) => {
                        if let Some(key) = key::Key::from_event(&k) {
                            ed.handle_key(key);
                        }
                    }
                    Event::Mouse(m) => mouse.handle(&mut ed, m),
                    Event::Resize(w, h) => {
                        scr.resize(w, h);
                        ed.resize(w, h);
                    }
                    Event::Paste(s) => ed.paste(&s),
                    _ => {}
                }
                if ed.quit || ed.suspend || !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }
        Ok(())
    })();
    term.restore();
    if let Err(e) = result {
        eprintln!("tmacs: {e}");
        std::process::exit(1);
    }
}
