//! System clipboard integration (Emacs interprogram-cut/paste functions).

use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clipboard {
    None,
    /// macOS pbcopy/pbpaste.
    Pb,
    /// Wayland wl-copy/wl-paste.
    Wayland,
    /// X11 xclip.
    Xclip,
    /// OSC 52 escape (copy only; works over SSH).
    Osc52,
}

fn has(cmd: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(cmd).is_file()))
        .unwrap_or(false)
}

impl Clipboard {
    pub fn from_setting(s: &str) -> Clipboard {
        match s {
            "none" | "" => Clipboard::None,
            "pbcopy" => Clipboard::Pb,
            "wayland" | "wl-copy" => Clipboard::Wayland,
            "xclip" => Clipboard::Xclip,
            "osc52" => Clipboard::Osc52,
            _ => {
                if std::env::var_os("SSH_TTY").is_some()
                    || std::env::var_os("SSH_CONNECTION").is_some()
                {
                    Clipboard::Osc52
                } else if has("pbcopy") && has("pbpaste") {
                    Clipboard::Pb
                } else if std::env::var_os("WAYLAND_DISPLAY").is_some() && has("wl-copy") {
                    Clipboard::Wayland
                } else if std::env::var_os("DISPLAY").is_some() && has("xclip") {
                    Clipboard::Xclip
                } else {
                    Clipboard::None
                }
            }
        }
    }

    fn copy_cmd(self) -> Option<(&'static str, &'static [&'static str])> {
        match self {
            Clipboard::Pb => Some(("pbcopy", &[])),
            Clipboard::Wayland => Some(("wl-copy", &[])),
            Clipboard::Xclip => Some(("xclip", &["-selection", "clipboard", "-in"])),
            _ => None,
        }
    }

    fn paste_cmd(self) -> Option<(&'static str, &'static [&'static str])> {
        match self {
            Clipboard::Pb => Some(("pbpaste", &[])),
            Clipboard::Wayland => Some(("wl-paste", &["--no-newline"])),
            Clipboard::Xclip => Some(("xclip", &["-selection", "clipboard", "-out"])),
            _ => None,
        }
    }

    /// Copy text to the system clipboard (asynchronously).
    pub fn copy(self, s: &str) {
        if self == Clipboard::Osc52 {
            osc52_copy(s);
            return;
        }
        let Some((cmd, args)) = self.copy_cmd() else {
            return;
        };
        let s = s.to_string();
        std::thread::spawn(move || {
            if let Ok(mut child) = Command::new(cmd)
                .args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(s.as_bytes());
                }
                let _ = child.wait();
            }
        });
    }

    /// Current clipboard contents, if readable.
    pub fn paste(self) -> Option<String> {
        let (cmd, args) = self.paste_cmd()?;
        let out = Command::new(cmd)
            .args(args)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        String::from_utf8(out.stdout).ok().filter(|s| !s.is_empty())
    }
}

fn osc52_copy(s: &str) {
    let seq = format!("\x1b]52;c;{}\x07", base64(s.as_bytes()));
    let mut out = std::io::stdout();
    let _ = out.write_all(seq.as_bytes());
    let _ = out.flush();
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn b64() {
        assert_eq!(base64(b"hello"), "aGVsbG8=");
        assert_eq!(base64(b"hi!"), "aGkh");
    }
    #[test]
    fn settings() {
        assert_eq!(Clipboard::from_setting("none"), Clipboard::None);
        assert_eq!(Clipboard::from_setting("osc52"), Clipboard::Osc52);
    }
}
