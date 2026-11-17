//! Double-buffered terminal cell grid with minimal diff output.

use crate::color::{self, Rgb};
use crate::face::Style;
use crossterm::style::{Attribute, Color, SetAttribute, SetBackgroundColor, SetForegroundColor};
use crossterm::{cursor, queue, terminal};
use std::io::{self, Write};
use unicode_width::UnicodeWidthChar;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cell {
    pub ch: char,
    pub style: Style,
    /// Second half of a double-width character.
    pub cont: bool,
}

pub struct Screen {
    pub w: u16,
    pub h: u16,
    cells: Vec<Cell>,
    prev: Vec<Cell>,
    pub cursor: Option<(u16, u16)>,
    pub truecolor: bool,
    full: bool,
    pub default: Style,
}

impl Screen {
    pub fn new(w: u16, h: u16, default: Style) -> Screen {
        let blank = Cell {
            ch: ' ',
            style: default,
            cont: false,
        };
        let n = w as usize * h as usize;
        Screen {
            w,
            h,
            cells: vec![blank; n],
            prev: vec![blank; n],
            cursor: None,
            truecolor: color::truecolor_supported(),
            full: true,
            default,
        }
    }

    pub fn resize(&mut self, w: u16, h: u16) {
        *self = Screen {
            truecolor: self.truecolor,
            ..Screen::new(w, h, self.default)
        };
    }

    pub fn invalidate(&mut self) {
        self.full = true;
    }

    pub fn clear(&mut self, default: Style) {
        self.default = default;
        let blank = Cell {
            ch: ' ',
            style: default,
            cont: false,
        };
        self.cells.fill(blank);
    }

    fn idx(&self, x: u16, y: u16) -> usize {
        y as usize * self.w as usize + x as usize
    }

    /// Put a char; returns the display width used (0 if clipped).
    pub fn put(&mut self, x: u16, y: u16, ch: char, style: Style) -> u16 {
        if x >= self.w || y >= self.h {
            return 0;
        }
        let wd = ch.width().unwrap_or(1).max(1) as u16;
        let style = style.resolved(self.default);
        if wd == 2 {
            if x + 1 >= self.w {
                let i = self.idx(x, y);
                self.cells[i] = Cell {
                    ch: ' ',
                    style,
                    cont: false,
                };
                return 1;
            }
            let i = self.idx(x, y);
            self.cells[i] = Cell {
                ch,
                style,
                cont: false,
            };
            self.cells[i + 1] = Cell {
                ch: ' ',
                style,
                cont: true,
            };
            2
        } else {
            let i = self.idx(x, y);
            self.cells[i] = Cell {
                ch,
                style,
                cont: false,
            };
            1
        }
    }

    pub fn put_str(&mut self, mut x: u16, y: u16, s: &str, style: Style, max_x: u16) -> u16 {
        for ch in s.chars() {
            let wd = ch.width().unwrap_or(0) as u16;
            if wd == 0 {
                continue;
            }
            if x + wd > max_x {
                break;
            }
            x += self.put(x, y, ch, style);
        }
        x
    }

    pub fn fill(&mut self, x0: u16, y: u16, x1: u16, style: Style) {
        for x in x0..x1.min(self.w) {
            self.put(x, y, ' ', style);
        }
    }

    pub fn set_style(&mut self, x: u16, y: u16, style: Style) {
        if x < self.w && y < self.h {
            let i = self.idx(x, y);
            let s = self.cells[i].style.over(style).resolved(self.default);
            self.cells[i].style = s;
        }
    }

    fn color(&self, c: Option<Rgb>) -> Color {
        match c {
            None => Color::Reset,
            Some(c) if self.truecolor => Color::Rgb {
                r: c.0,
                g: c.1,
                b: c.2,
            },
            Some(c) => Color::AnsiValue(color::to_ansi256(c)),
        }
    }

    pub fn flush<W: Write>(&mut self, out: &mut W) -> io::Result<()> {
        queue!(out, terminal::BeginSynchronizedUpdate, cursor::Hide)?;
        if self.full {
            queue!(
                out,
                SetAttribute(Attribute::Reset),
                SetBackgroundColor(self.color(self.default.bg))
            )?;
            queue!(out, terminal::Clear(terminal::ClearType::All))?;
        }
        let mut cur: Option<Style> = None;
        let mut pos: Option<(u16, u16)> = None;
        for y in 0..self.h {
            let mut x = 0u16;
            while x < self.w {
                let i = self.idx(x, y);
                let c = self.cells[i];
                let wide = !c.cont && x + 1 < self.w && self.cells[i + 1].cont;
                let changed = self.full
                    || c != self.prev[i]
                    || (wide && self.cells[i + 1] != self.prev[i + 1]);
                if c.cont || !changed {
                    x += 1;
                    continue;
                }
                if pos != Some((x, y)) {
                    queue!(out, cursor::MoveTo(x, y))?;
                }
                if cur != Some(c.style) {
                    queue!(out, SetAttribute(Attribute::Reset))?;
                    queue!(
                        out,
                        SetForegroundColor(self.color(c.style.fg)),
                        SetBackgroundColor(self.color(c.style.bg))
                    )?;
                    if c.style.bold {
                        queue!(out, SetAttribute(Attribute::Bold))?;
                    }
                    if c.style.italic {
                        queue!(out, SetAttribute(Attribute::Italic))?;
                    }
                    if c.style.underline {
                        queue!(out, SetAttribute(Attribute::Underlined))?;
                    }
                    cur = Some(c.style);
                }
                write!(out, "{}", c.ch)?;
                let adv = if wide { 2 } else { 1 };
                x += adv;
                pos = Some((x, y));
            }
        }
        queue!(out, SetAttribute(Attribute::Reset))?;
        if let Some((cx, cy)) = self.cursor {
            queue!(out, cursor::MoveTo(cx, cy), cursor::Show)?;
        }
        queue!(out, terminal::EndSynchronizedUpdate)?;
        out.flush()?;
        self.prev.copy_from_slice(&self.cells);
        self.full = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::face::Style;

    #[test]
    fn diff_only_redraws_changed_cells() {
        let def = Style::fgbg("white", "black");
        let mut scr = Screen::new(10, 2, def);
        scr.truecolor = true;
        scr.put_str(0, 0, "hello", Style::default(), 10);
        let mut out = Vec::new();
        scr.flush(&mut out).unwrap();
        assert!(String::from_utf8_lossy(&out).contains("hello"));
        // Same frame again: nothing printed.
        scr.clear(def);
        scr.put_str(0, 0, "hello", Style::default(), 10);
        let mut out = Vec::new();
        scr.flush(&mut out).unwrap();
        assert!(!String::from_utf8_lossy(&out).contains("hello"));
        // One change: only that cell.
        scr.clear(def);
        scr.put_str(0, 0, "hellO", Style::default(), 10);
        let mut out = Vec::new();
        scr.flush(&mut out).unwrap();
        let s = String::from_utf8_lossy(&out);
        assert!(s.contains('O') && !s.contains("hell"));
    }

    #[test]
    fn wide_chars_take_two_cells() {
        let mut scr = Screen::new(4, 1, Style::fgbg("white", "black"));
        assert_eq!(scr.put(0, 0, '漢', Style::default()), 2);
        // Doesn't fit at the last column: replaced by a blank.
        assert_eq!(scr.put(3, 0, '漢', Style::default()), 1);
        assert_eq!(scr.put_str(0, 0, "ab漢", Style::default(), 3), 2);
    }
}
