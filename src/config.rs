// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Benjamin Zores

//! User configuration: `~/.config/tmacs/init.toml` (or `$TMACS_INIT`).

use crate::lang::LangConfig;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct Settings {
    pub tab_width: usize,
    pub indent_tabs_mode: bool,
    pub fill_column: usize,
    pub scroll_step: usize,
    pub scroll_preserve_screen_position: bool,
    pub next_screen_context_lines: usize,
    pub show_trailing_whitespace: bool,
    pub show_paren_mode: bool,
    pub transient_mark_mode: bool,
    pub search_highlight: bool,
    pub case_fold_search: bool,
    pub visible_bell: bool,
    pub display_time: bool,
    /// strftime-style format; `%L` = load average like Emacs' `load`.
    pub display_time_format: String,
    pub line_number_mode: bool,
    pub column_number_mode: bool,
    pub truncate_lines: bool,
    pub make_backup_files: bool,
    pub require_final_newline: bool,
    pub xterm_mouse_mode: bool,
    pub mouse_wheel_lines: usize,
    pub kitty_keyboard: bool,
    pub cursor_color: Option<String>,
    pub frame_title_format: String,
    pub highlight_tabs: bool,
    pub kill_whole_line: bool,
    pub format_on_save: bool,
    pub format_timeout_ms: u64,
    pub max_highlight_size: usize,
    pub delete_selection_mode: bool,
    /// "auto" (pbcopy locally, OSC 52 over SSH), "pbcopy", "osc52",
    /// "wayland", "xclip" or "none".
    pub clipboard: String,
    pub yes_or_no_short: bool,
    /// Reopen last session's files when started without file arguments.
    pub desktop_save_mode: bool,
    /// Persist minibuffer and search histories.
    pub savehist_mode: bool,
}

impl Default for Settings {
    fn default() -> Self {
        // Mirrors ~/.emacs.cli.d/init.el
        Settings {
            tab_width: 8,
            indent_tabs_mode: false,
            fill_column: 70,
            scroll_step: 1,
            scroll_preserve_screen_position: true,
            next_screen_context_lines: 1,
            show_trailing_whitespace: true,
            show_paren_mode: true,
            transient_mark_mode: true,
            search_highlight: true,
            case_fold_search: false,
            visible_bell: true,
            display_time: true,
            display_time_format: "[%-d/%-m]-[%H:%M]%L".into(),
            line_number_mode: true,
            column_number_mode: false,
            truncate_lines: false,
            make_backup_files: false,
            require_final_newline: true,
            xterm_mouse_mode: true,
            mouse_wheel_lines: 1,
            kitty_keyboard: true,
            cursor_color: Some("yellow".into()),
            frame_title_format: "Emacs - %b (%f)".into(),
            highlight_tabs: false,
            kill_whole_line: false,
            format_on_save: true,
            format_timeout_ms: 5000,
            max_highlight_size: 20 * 1024 * 1024,
            delete_selection_mode: false,
            clipboard: "auto".into(),
            yes_or_no_short: true,
            desktop_save_mode: true,
            savehist_mode: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct Config {
    pub settings: Settings,
    pub faces: HashMap<String, HashMap<String, toml::Value>>,
    /// Global key bindings: "C-c f" = "format-buffer". Use "" to unbind.
    pub keys: HashMap<String, String>,
    pub language: Vec<LangConfig>,
}

pub fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("TMACS_INIT") {
        return PathBuf::from(p);
    }
    dirs::home_dir()
        .unwrap_or_default()
        .join(".config")
        .join("tmacs")
        .join("init.toml")
}

pub fn load() -> (Config, Option<String>) {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => match toml::from_str::<Config>(&s) {
            Ok(c) => (c, None),
            Err(e) => (
                Config::default(),
                Some(format!("Error in {}: {}", path.display(), e.message())),
            ),
        },
        Err(_) => (Config::default(), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_config_parses() {
        let s = include_str!("../examples/init.toml");
        let c: Config = toml::from_str(s).unwrap();
        assert!(!c.settings.case_fold_search);
        assert!(c.language.iter().any(|l| l.name == "go"));
    }
}
