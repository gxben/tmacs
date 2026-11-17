// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Benjamin Zores

//! Faces (display styles), defaults mirror the user's Emacs configuration.

use crate::color::{self, Rgb};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub struct Style {
    pub fg: Option<Rgb>,
    pub bg: Option<Rgb>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub reverse: bool,
}

impl Style {
    pub fn fg(c: &str) -> Style {
        Style {
            fg: color::parse(c),
            ..Default::default()
        }
    }
    pub fn bg(c: &str) -> Style {
        Style {
            bg: color::parse(c),
            ..Default::default()
        }
    }
    pub fn fgbg(f: &str, b: &str) -> Style {
        Style {
            fg: color::parse(f),
            bg: color::parse(b),
            ..Default::default()
        }
    }
    pub fn bold(mut self) -> Style {
        self.bold = true;
        self
    }
    pub fn italic(mut self) -> Style {
        self.italic = true;
        self
    }
    pub fn underline(mut self) -> Style {
        self.underline = true;
        self
    }

    /// Layer `top` over `self`: set fields of `top` win.
    pub fn over(self, top: Style) -> Style {
        Style {
            fg: top.fg.or(self.fg),
            bg: top.bg.or(self.bg),
            bold: self.bold || top.bold,
            italic: self.italic || top.italic,
            underline: self.underline || top.underline,
            reverse: self.reverse ^ top.reverse,
        }
    }

    /// Resolve reverse video into explicit colors.
    pub fn resolved(self, default: Style) -> Style {
        let mut s = default.over(self);
        if s.reverse {
            std::mem::swap(&mut s.fg, &mut s.bg);
            s.reverse = false;
        }
        s
    }
}

macro_rules! faces {
    ($($name:ident = $lisp:literal),* $(,)?) => {
        #[derive(Clone, Debug)]
        pub struct Faces { $(pub $name: Style,)* }
        impl Faces {
            pub fn get_mut(&mut self, name: &str) -> Option<&mut Style> {
                match name { $($lisp => Some(&mut self.$name),)* _ => None }
            }
            pub fn get(&self, name: &str) -> Option<Style> {
                match name { $($lisp => Some(self.$name),)* _ => None }
            }
        }
    };
}

faces! {
    default = "default",
    region = "region",
    highlight = "highlight",
    mode_line = "mode-line",
    mode_line_inactive = "mode-line-inactive",
    mode_line_buffer_id = "mode-line-buffer-id",
    minibuffer_prompt = "minibuffer-prompt",
    isearch = "isearch",
    isearch_fail = "isearch-fail",
    lazy_highlight = "lazy-highlight",
    show_paren_match = "show-paren-match",
    show_paren_mismatch = "show-paren-mismatch",
    trailing_whitespace = "trailing-whitespace",
    tab = "tab-face",
    vertical_border = "vertical-border",
    escape_glyph = "escape-glyph",
    error = "error",
    warning = "warning",
    success = "success",
    completions = "completions-common-part",
    comment = "font-lock-comment-face",
    doc = "font-lock-doc-face",
    string = "font-lock-string-face",
    keyword = "font-lock-keyword-face",
    builtin = "font-lock-builtin-face",
    function = "font-lock-function-name-face",
    variable = "font-lock-variable-name-face",
    type_ = "font-lock-type-face",
    constant = "font-lock-constant-face",
    preprocessor = "font-lock-preprocessor-face",
    warning_kw = "font-lock-warning-face",
    bold = "bold",
    italic = "italic",
    underline = "underline",
    bold_italic = "bold-italic",
    link = "link",
    header = "header",
    diff_added = "diff-added",
    diff_removed = "diff-removed",
    dired_directory = "dired-directory",
}

impl Default for Faces {
    fn default() -> Self {
        // From ~/.emacs.cli.d/init.el + Emacs dark-background defaults.
        Faces {
            default: Style::fgbg("wheat", "DarkSlateGray"),
            region: Style::bg("black"),
            highlight: Style::bg("slateblue"),
            mode_line: Style::fgbg("black", "grey75"),
            mode_line_inactive: Style::fgbg("gray80", "gray30"),
            mode_line_buffer_id: Style::default().bold(),
            minibuffer_prompt: Style::fg("cyan"),
            isearch: Style::fgbg("brown4", "palevioletred2"),
            isearch_fail: Style::bg("red4"),
            lazy_highlight: Style::bg("paleturquoise4"),
            show_paren_match: Style::bg("steelblue3"),
            show_paren_mismatch: Style::fgbg("white", "purple"),
            trailing_whitespace: Style::bg("red1"),
            tab: Style::bg("LightGrey"),
            vertical_border: Style::default(),
            escape_glyph: Style::fg("cyan"),
            error: Style::fg("Pink").bold(),
            warning: Style::fg("DarkOrange").bold(),
            success: Style::fg("Green1").bold(),
            completions: Style::fg("lightblue"),
            comment: Style::fg("gray"),
            doc: Style::fg("green3"),
            string: Style::fg("green3"),
            keyword: Style::fg("orange"),
            builtin: Style::fg("LightSteelBlue"),
            function: Style::fg("lightblue2"),
            variable: Style::fg("lightblue3"),
            type_: Style::fg("palegreen"),
            constant: Style::fg("Aquamarine"),
            preprocessor: Style::fg("LightSteelBlue"),
            warning_kw: Style::fg("Pink").bold(),
            bold: Style::fg("lightcoral").bold(),
            italic: Style::fg("Orchid").italic(),
            underline: Style::fg("seagreen3").underline(),
            bold_italic: Style::fg("orange").bold().italic(),
            link: Style::fg("cyan1").underline(),
            header: Style::fg("lightcoral").bold(),
            diff_added: Style::fg("green3"),
            diff_removed: Style::fg("lightcoral"),
            dired_directory: Style::fg("LightSkyBlue"),
        }
    }
}

/// Parse a face spec from config: `{ fg = "x", bg = "y", bold = true, ... }`.
pub fn style_from_table(t: &HashMap<String, toml::Value>) -> Result<Style, String> {
    let mut s = Style::default();
    for (k, v) in t {
        match (k.as_str(), v) {
            ("fg" | "foreground", toml::Value::String(c)) => {
                s.fg = Some(color::parse(c).ok_or_else(|| format!("unknown color {c}"))?)
            }
            ("bg" | "background", toml::Value::String(c)) => {
                s.bg = Some(color::parse(c).ok_or_else(|| format!("unknown color {c}"))?)
            }
            ("bold", toml::Value::Boolean(b)) => s.bold = *b,
            ("italic", toml::Value::Boolean(b)) => s.italic = *b,
            ("underline", toml::Value::Boolean(b)) => s.underline = *b,
            ("reverse" | "inverse-video", toml::Value::Boolean(b)) => s.reverse = *b,
            _ => return Err(format!("bad face attribute {k}")),
        }
    }
    Ok(s)
}
