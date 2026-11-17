// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Benjamin Zores

//! Key representation using Emacs notation (`C-x`, `M-f`, `C-M-<return>`...).

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum K {
    Char(char),
    Enter,
    Tab,
    Backspace,
    Delete,
    Esc,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    F(u8),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Key {
    pub k: K,
    pub ctrl: bool,
    pub meta: bool,
    pub shift: bool,
}

impl Key {
    pub const fn new(k: K) -> Key {
        Key {
            k,
            ctrl: false,
            meta: false,
            shift: false,
        }
    }
    pub fn with_meta(mut self) -> Key {
        self.meta = true;
        self
    }

    /// Printable character with no modifiers (self-insert candidate).
    pub fn plain_char(&self) -> Option<char> {
        match self.k {
            K::Char(c) if !self.ctrl && !self.meta => Some(c),
            _ => None,
        }
    }

    pub fn from_event(ev: &KeyEvent) -> Option<Key> {
        if ev.kind == KeyEventKind::Release {
            return None;
        }
        let m = ev.modifiers;
        let mut ctrl = m.contains(KeyModifiers::CONTROL);
        let meta = m.contains(KeyModifiers::ALT) || m.contains(KeyModifiers::META);
        let mut shift = m.contains(KeyModifiers::SHIFT);
        let k = match ev.code {
            KeyCode::Char(c) => {
                let mut c = c;
                // Kitty keyboard protocol reports the unshifted key plus Shift
                // (C-S-- for C-_, M-S-, for M-<): apply the US shift layer.
                if shift && (ctrl || meta) {
                    if let Some(s) = shifted(c) {
                        c = s;
                        shift = false;
                    }
                }
                if ctrl {
                    // Legacy terminals encode C-\ C-] C-^ C-_ as C-4..C-7.
                    c = match c {
                        '4' => '\\',
                        '5' => ']',
                        '6' => '^',
                        '7' => '_',
                        '@' => ' ',
                        _ => c,
                    };
                    if c.is_ascii_uppercase() {
                        c = c.to_ascii_lowercase();
                    } else {
                        shift = false;
                    }
                } else {
                    shift = false;
                }
                if c == '\u{7f}' {
                    ctrl = false;
                    K::Backspace
                } else if ctrl && c == 'i' && !shift {
                    // Kitty protocol reports C-i/C-m/C-[ distinctly; Emacs treats
                    // them as TAB/RET/ESC.
                    ctrl = false;
                    K::Tab
                } else if ctrl && c == 'm' && !shift {
                    ctrl = false;
                    K::Enter
                } else if ctrl && c == '[' {
                    ctrl = false;
                    K::Esc
                } else {
                    K::Char(c)
                }
            }
            KeyCode::Enter => K::Enter,
            KeyCode::Tab => K::Tab,
            KeyCode::BackTab => {
                shift = true;
                K::Tab
            }
            KeyCode::Backspace => K::Backspace,
            KeyCode::Delete => K::Delete,
            KeyCode::Esc => K::Esc,
            KeyCode::Left => K::Left,
            KeyCode::Right => K::Right,
            KeyCode::Up => K::Up,
            KeyCode::Down => K::Down,
            KeyCode::Home => K::Home,
            KeyCode::End => K::End,
            KeyCode::PageUp => K::PageUp,
            KeyCode::PageDown => K::PageDown,
            KeyCode::Insert => K::Insert,
            KeyCode::F(n) => K::F(n),
            _ => return None,
        };
        Some(Key {
            k,
            ctrl,
            meta,
            shift,
        })
    }

    /// Parse a single key in Emacs notation: `C-x`, `M-<backspace>`, `RET`, `C-M-%`.
    pub fn parse(s: &str) -> Option<Key> {
        let mut key = Key::new(K::Char(' '));
        let mut rest = s;
        loop {
            if rest.len() > 2
                && (rest.starts_with("C-") || rest.starts_with("M-") || rest.starts_with("S-"))
            {
                match &rest[..1] {
                    "C" => key.ctrl = true,
                    "M" => key.meta = true,
                    _ => key.shift = true,
                }
                rest = &rest[2..];
            } else {
                break;
            }
        }
        let name = if rest.len() > 2 && rest.starts_with('<') && rest.ends_with('>') {
            &rest[1..rest.len() - 1]
        } else {
            rest
        };
        key.k = match name {
            "RET" | "return" => K::Enter,
            "TAB" | "tab" => K::Tab,
            "DEL" | "backspace" => K::Backspace,
            "deletechar" | "delete" => K::Delete,
            "ESC" | "escape" => K::Esc,
            "SPC" => K::Char(' '),
            "left" => K::Left,
            "right" => K::Right,
            "up" => K::Up,
            "down" => K::Down,
            "home" => K::Home,
            "end" => K::End,
            "prior" | "pageup" => K::PageUp,
            "next" | "pagedown" => K::PageDown,
            "insert" | "insertchar" => K::Insert,
            "backtab" | "iso-lefttab" => {
                key.shift = true;
                K::Tab
            }
            n if n.len() > 1 && n.starts_with('f') && n[1..].parse::<u8>().is_ok() => {
                K::F(n[1..].parse().unwrap())
            }
            n => {
                let mut it = n.chars();
                let c = it.next()?;
                if it.next().is_some() {
                    return None;
                }
                K::Char(c)
            }
        };
        if key.ctrl {
            if let K::Char(c) = key.k {
                if c.is_ascii_uppercase() {
                    key.k = K::Char(c.to_ascii_lowercase());
                    key.shift = true;
                }
            }
        }
        Some(key)
    }

    pub fn parse_seq(s: &str) -> Option<Vec<Key>> {
        let v: Option<Vec<Key>> = s.split_whitespace().map(Key::parse).collect();
        v.filter(|v| !v.is_empty())
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl {
            write!(f, "C-")?;
        }
        if self.meta {
            write!(f, "M-")?;
        }
        let named = |f: &mut fmt::Formatter<'_>, n: &str| {
            if self.shift {
                write!(f, "S-")?;
            }
            write!(f, "{}", n)
        };
        match self.k {
            K::Char(' ') => write!(f, "SPC"),
            K::Char(c) => {
                if self.shift && self.ctrl {
                    write!(f, "{}", c.to_ascii_uppercase())
                } else {
                    write!(f, "{}", c)
                }
            }
            K::Enter if !self.shift => write!(f, "RET"),
            K::Tab if !self.shift && !self.ctrl => write!(f, "TAB"),
            K::Backspace if !self.shift => write!(f, "DEL"),
            K::Esc if !self.shift => write!(f, "ESC"),
            K::Enter => named(f, "<return>"),
            K::Tab => named(f, "<tab>"),
            K::Backspace => named(f, "<backspace>"),
            K::Esc => named(f, "<escape>"),
            K::Delete => named(f, "<deletechar>"),
            K::Left => named(f, "<left>"),
            K::Right => named(f, "<right>"),
            K::Up => named(f, "<up>"),
            K::Down => named(f, "<down>"),
            K::Home => named(f, "<home>"),
            K::End => named(f, "<end>"),
            K::PageUp => named(f, "<prior>"),
            K::PageDown => named(f, "<next>"),
            K::Insert => named(f, "<insert>"),
            K::F(n) => named(f, &format!("<f{}>", n)),
        }
    }
}

/// Shifted character of a US-layout key, for non-letters.
fn shifted(c: char) -> Option<char> {
    const PAIRS: &str = "1!2@3#4$5%6^7&8*9(0)-_=+[{]}\\|;:'\",<.>/?`~";
    let v: Vec<char> = PAIRS.chars().collect();
    v.chunks(2).find(|p| p[0] == c).map(|p| p[1])
}

pub fn seq_to_string(keys: &[Key]) -> String {
    keys.iter()
        .map(|k| k.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_roundtrip() {
        for s in [
            "C-x", "M-f", "C-M-f", "RET", "TAB", "DEL", "M-DEL", "C-x C-f", "S-<tab>", "C-<tab>",
            "<f3>", "M-%", "C-SPC", "M-<",
        ] {
            let seq = Key::parse_seq(s).unwrap();
            assert_eq!(seq_to_string(&seq), s);
        }
    }

    #[test]
    fn kitty_shifted_punctuation() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let ev = |c, m| {
            Key::from_event(&KeyEvent::new(KeyCode::Char(c), m))
                .unwrap()
                .to_string()
        };
        assert_eq!(ev('-', KeyModifiers::CONTROL | KeyModifiers::SHIFT), "C-_");
        assert_eq!(ev('7', KeyModifiers::CONTROL), "C-_");
        assert_eq!(ev(',', KeyModifiers::ALT | KeyModifiers::SHIFT), "M-<");
        assert_eq!(ev('5', KeyModifiers::ALT | KeyModifiers::SHIFT), "M-%");
        assert_eq!(ev('A', KeyModifiers::CONTROL | KeyModifiers::SHIFT), "C-A");
    }

    #[test]
    fn parse_aliases() {
        assert_eq!(Key::parse("M-<backspace>"), Key::parse("M-DEL"));
        assert_eq!(Key::parse("<backtab>"), Key::parse("S-<tab>"));
        assert_eq!(
            Key::parse("C-S-<iso-lefttab>").unwrap().to_string(),
            "C-S-<tab>"
        );
    }
}
