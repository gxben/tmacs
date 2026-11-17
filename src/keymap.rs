//! Prefix keymaps and default Emacs bindings (plus the user's init.el ones).

use crate::key::Key;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub enum Binding {
    Cmd(String),
    Prefix(Keymap),
}

#[derive(Clone, Debug, Default)]
pub struct Keymap {
    pub map: HashMap<Key, Binding>,
}

pub enum Lookup<'a> {
    Cmd(&'a str),
    Prefix,
    None,
}

impl Keymap {
    pub fn bind(&mut self, keys: &[Key], cmd: &str) {
        let (first, rest) = keys.split_first().expect("empty key sequence");
        if rest.is_empty() {
            if cmd.is_empty() {
                self.map.remove(first);
            } else {
                self.map.insert(*first, Binding::Cmd(cmd.to_string()));
            }
            return;
        }
        let entry = self
            .map
            .entry(*first)
            .or_insert_with(|| Binding::Prefix(Keymap::default()));
        if let Binding::Cmd(_) = entry {
            *entry = Binding::Prefix(Keymap::default());
        }
        if let Binding::Prefix(m) = entry {
            m.bind(rest, cmd);
        }
    }

    pub fn bind_str(&mut self, keys: &str, cmd: &str) -> bool {
        match Key::parse_seq(keys) {
            Some(seq) => {
                self.bind(&seq, cmd);
                true
            }
            None => false,
        }
    }

    pub fn lookup(&self, keys: &[Key]) -> Lookup<'_> {
        let mut m = self;
        for (i, k) in keys.iter().enumerate() {
            match m.map.get(k) {
                Some(Binding::Cmd(c)) => {
                    return if i + 1 == keys.len() {
                        Lookup::Cmd(c)
                    } else {
                        Lookup::None
                    };
                }
                Some(Binding::Prefix(p)) => {
                    if i + 1 == keys.len() {
                        return Lookup::Prefix;
                    }
                    m = p;
                }
                None => return Lookup::None,
            }
        }
        Lookup::None
    }

    /// All (key sequence, command) pairs, for describe-bindings / where-is.
    pub fn flatten(&self) -> Vec<(Vec<Key>, String)> {
        let mut out = Vec::new();
        fn walk(m: &Keymap, prefix: &mut Vec<Key>, out: &mut Vec<(Vec<Key>, String)>) {
            for (k, b) in &m.map {
                prefix.push(*k);
                match b {
                    Binding::Cmd(c) => out.push((prefix.clone(), c.clone())),
                    Binding::Prefix(p) => walk(p, prefix, out),
                }
                prefix.pop();
            }
        }
        walk(self, &mut Vec::new(), &mut out);
        out.sort_by_key(|(k, _)| crate::key::seq_to_string(k));
        out
    }
}

pub const GLOBAL: &[(&str, &str)] = &[
    // --- motion ---
    ("C-f", "forward-char"),
    ("<right>", "forward-char"),
    ("C-b", "backward-char"),
    ("<left>", "backward-char"),
    ("C-n", "next-line"),
    ("<down>", "next-line"),
    ("C-p", "previous-line"),
    ("<up>", "previous-line"),
    ("C-a", "move-beginning-of-line"),
    ("C-e", "move-end-of-line"),
    ("<home>", "beginning-of-line"),
    ("<end>", "end-of-line"),
    ("M-f", "forward-word"),
    ("M-b", "backward-word"),
    ("C-<right>", "forward-word"),
    ("M-<right>", "forward-word"),
    ("C-<left>", "backward-word"),
    ("M-<left>", "backward-word"),
    ("M-<", "beginning-of-buffer"),
    ("M->", "end-of-buffer"),
    ("C-<home>", "beginning-of-buffer"),
    ("C-<end>", "end-of-buffer"),
    ("C-v", "scroll-up-command"),
    ("<next>", "scroll-up-command"),
    ("M-v", "scroll-down-command"),
    ("<prior>", "scroll-down-command"),
    ("C-M-v", "scroll-other-window"),
    ("C-M-S-v", "scroll-other-window-down"),
    ("C-l", "recenter-top-bottom"),
    ("M-r", "move-to-window-line-top-bottom"),
    ("M-m", "back-to-indentation"),
    ("M-}", "forward-paragraph"),
    ("M-{", "backward-paragraph"),
    ("C-<down>", "forward-paragraph"),
    ("C-<up>", "backward-paragraph"),
    ("M-e", "forward-sentence"),
    ("M-a", "backward-sentence"),
    ("C-M-f", "forward-sexp"),
    ("C-M-b", "backward-sexp"),
    ("C-M-u", "backward-up-list"),
    ("C-M-d", "down-list"),
    ("C-M-n", "forward-list"),
    ("C-M-p", "backward-list"),
    ("C-M-a", "beginning-of-defun"),
    ("C-M-e", "end-of-defun"),
    // User: (define-key global-map [(meta g)] 'goto-line)
    ("M-g", "goto-line"),
    ("C-x <", "scroll-left"),
    ("C-x >", "scroll-right"),
    // --- editing ---
    ("RET", "newline"),
    ("C-j", "electric-newline-and-maybe-indent"),
    ("C-o", "open-line"),
    ("C-M-o", "split-line"),
    ("C-d", "delete-char"),
    ("<deletechar>", "delete-char"),
    ("DEL", "delete-backward-char"),
    ("S-DEL", "delete-backward-char"),
    ("M-d", "kill-word"),
    ("C-<deletechar>", "kill-word"),
    ("M-DEL", "backward-kill-word"),
    ("C-DEL", "backward-kill-word"),
    ("C-<backspace>", "backward-kill-word"),
    ("C-S-<backspace>", "kill-whole-line"),
    ("C-k", "kill-line"),
    ("C-w", "kill-region"),
    ("M-w", "kill-ring-save"),
    ("C-y", "yank"),
    ("M-y", "yank-pop"),
    ("C-SPC", "set-mark-command"),
    ("C-x C-x", "exchange-point-and-mark"),
    ("C-x h", "mark-whole-buffer"),
    ("M-@", "mark-word"),
    ("M-h", "mark-paragraph"),
    ("C-M-SPC", "mark-sexp"),
    ("C-M-@", "mark-sexp"),
    ("C-M-h", "mark-defun"),
    ("C-x C-p", "mark-page"),
    ("C-_", "undo"),
    ("C-/", "undo"),
    ("C-x u", "undo"),
    ("C-?", "undo-redo"),
    ("C-M-_", "undo-redo"),
    ("C-t", "transpose-chars"),
    ("M-t", "transpose-words"),
    ("C-x C-t", "transpose-lines"),
    ("C-M-t", "transpose-sexps"),
    ("M-u", "upcase-word"),
    ("M-l", "downcase-word"),
    ("M-c", "capitalize-word"),
    ("C-x C-u", "upcase-region"),
    ("C-x C-l", "downcase-region"),
    ("M-\\", "delete-horizontal-space"),
    ("M-SPC", "just-one-space"),
    ("M-^", "delete-indentation"),
    ("C-x C-o", "delete-blank-lines"),
    ("M-z", "zap-to-char"),
    ("TAB", "indent-for-tab-command"),
    ("C-M-\\", "indent-region"),
    ("C-x TAB", "indent-rigidly"),
    ("M-;", "comment-dwim"),
    ("C-x C-;", "comment-line"),
    ("M-q", "fill-paragraph"),
    ("C-x f", "set-fill-column"),
    ("C-q", "quoted-insert"),
    ("<insert>", "overwrite-mode"),
    ("M-/", "dabbrev-expand"),
    ("C-M-/", "dabbrev-completion"),
    ("M-=", "count-words-region"),
    ("C-x =", "what-cursor-position"),
    ("C-x l", "count-lines-page"),
    ("C-M-k", "kill-sexp"),
    ("C-M-DEL", "backward-kill-sexp"),
    ("M-k", "kill-sentence"),
    ("M-'", "abbrev-prefix-mark"),
    ("C-x z", "repeat"),
    ("M-!", "shell-command"),
    ("M-|", "shell-command-on-region"),
    ("M-&", "async-shell-command"),
    ("C-x C-e", "eval-last-sexp"),
    // --- rectangles & registers ---
    ("C-x r k", "kill-rectangle"),
    ("C-x r d", "delete-rectangle"),
    ("C-x r y", "yank-rectangle"),
    ("C-x r o", "open-rectangle"),
    ("C-x r c", "clear-rectangle"),
    ("C-x r t", "string-rectangle"),
    ("C-x r M-w", "copy-rectangle-as-kill"),
    ("C-x r N", "rectangle-number-lines"),
    ("C-x r s", "copy-to-register"),
    ("C-x r x", "copy-to-register"),
    ("C-x r i", "insert-register"),
    ("C-x r g", "insert-register"),
    ("C-x r SPC", "point-to-register"),
    ("C-x r j", "jump-to-register"),
    ("C-x r m", "bookmark-set"),
    ("C-x r b", "bookmark-jump"),
    ("C-x r l", "bookmark-bmenu-list"),
    // --- search ---
    ("C-s", "isearch-forward"),
    ("C-r", "isearch-backward"),
    ("C-M-s", "isearch-forward-regexp"),
    ("C-M-r", "isearch-backward-regexp"),
    ("M-%", "query-replace"),
    ("C-M-%", "query-replace-regexp"),
    ("M-s o", "occur"),
    ("M-s .", "isearch-forward-symbol-at-point"),
    ("M-s w", "isearch-forward-word"),
    ("M-s _", "isearch-forward-symbol"),
    ("M-s h r", "highlight-regexp"),
    ("M-s h u", "unhighlight-regexp"),
    ("C-x `", "next-error"),
    // --- files ---
    ("C-x C-f", "find-file"),
    ("C-x C-r", "find-file-read-only"),
    ("C-x C-v", "find-alternate-file"),
    ("C-x 4 f", "find-file-other-window"),
    ("C-x 4 C-f", "find-file-other-window"),
    ("C-x C-s", "save-buffer"),
    ("C-x C-w", "write-file"),
    ("C-x s", "save-some-buffers"),
    ("C-x i", "insert-file"),
    ("C-x C-c", "save-buffers-kill-terminal"),
    ("C-x C-q", "read-only-mode"),
    ("C-x d", "dired"),
    ("C-x C-d", "list-directory"),
    ("C-x 4 d", "dired-other-window"),
    ("C-x C-z", "suspend-frame"),
    // --- buffers ---
    ("C-x b", "switch-to-buffer"),
    ("C-x 4 b", "switch-to-buffer-other-window"),
    ("C-x C-b", "list-buffers"),
    // User: (global-set-key [(control x) ?k] 'my-kill-buffer)
    ("C-x k", "kill-current-buffer"),
    ("C-x <right>", "next-buffer"),
    ("C-x <left>", "previous-buffer"),
    ("C-x C-<right>", "next-buffer"),
    ("C-x C-<left>", "previous-buffer"),
    // --- windows ---
    ("C-x 2", "split-window-below"),
    ("C-x 3", "split-window-right"),
    ("C-x 0", "delete-window"),
    ("C-x 1", "delete-other-windows"),
    ("C-x o", "other-window"),
    ("C-x ^", "enlarge-window"),
    ("C-x }", "enlarge-window-horizontally"),
    ("C-x {", "shrink-window-horizontally"),
    ("C-x -", "shrink-window-if-larger-than-buffer"),
    ("C-x +", "balance-windows"),
    ("C-x 4 0", "kill-buffer-and-window"),
    // User: C-z / C-<tab> other-window, C-S-<tab> previous window
    ("C-z", "other-window"),
    ("C-<tab>", "other-window"),
    ("C-S-<tab>", "other-window-backward"),
    // --- user init.el extras ---
    ("C-x p", "insert-current-time-string"),
    ("S-<tab>", "dabbrev-expand"),
    ("C-M-RET", "flyspell-auto-correct-word"),
    // --- misc ---
    ("C-g", "keyboard-quit"),
    ("M-ESC ESC", "keyboard-escape-quit"),
    ("M-x", "execute-extended-command"),
    ("M-:", "eval-expression"),
    ("C-u", "universal-argument"),
    ("M--", "negative-argument"),
    ("C--", "negative-argument"),
    ("C-x (", "kmacro-start-macro"),
    ("C-x )", "kmacro-end-macro"),
    ("C-x e", "kmacro-end-and-call-macro"),
    ("<f3>", "kmacro-start-macro-or-insert-counter"),
    ("<f4>", "kmacro-end-or-call-macro"),
    ("C-x n n", "narrow-to-region"),
    ("C-x n w", "widen"),
    ("C-x C-+", "text-scale-adjust"),
    ("C-h k", "describe-key"),
    ("C-h c", "describe-key-briefly"),
    ("C-h b", "describe-bindings"),
    ("C-h f", "describe-function"),
    ("C-h w", "where-is"),
    ("C-h e", "view-echo-area-messages"),
    ("C-h m", "describe-mode"),
    ("C-h ?", "help-for-help"),
    ("<f1> k", "describe-key"),
    ("<f1> b", "describe-bindings"),
    ("<f10>", "menu-bar-open"),
];

pub const MINIBUFFER: &[(&str, &str)] = &[
    ("RET", "exit-minibuffer"),
    ("C-j", "exit-minibuffer"),
    ("TAB", "minibuffer-complete"),
    ("?", "minibuffer-completion-help"),
    ("C-g", "abort-recursive-edit"),
    ("M-ESC ESC", "abort-recursive-edit"),
    ("M-p", "previous-history-element"),
    ("M-n", "next-history-element"),
    ("<up>", "previous-history-element"),
    ("<down>", "next-history-element"),
    ("M-v", "switch-to-completions"),
];

pub const BUFFER_LIST: &[(&str, &str)] = &[
    ("RET", "Buffer-menu-this-window"),
    ("f", "Buffer-menu-this-window"),
    ("o", "Buffer-menu-other-window"),
    ("C-o", "Buffer-menu-switch-other-window"),
    ("1", "Buffer-menu-1-window"),
    ("d", "Buffer-menu-delete"),
    ("k", "Buffer-menu-delete"),
    ("C-d", "Buffer-menu-delete-backwards"),
    ("s", "Buffer-menu-save"),
    ("u", "Buffer-menu-unmark"),
    ("DEL", "Buffer-menu-backup-unmark"),
    ("x", "Buffer-menu-execute"),
    ("~", "Buffer-menu-not-modified"),
    ("%", "Buffer-menu-toggle-read-only"),
    ("g", "revert-buffer"),
    ("n", "next-line"),
    ("p", "previous-line"),
    ("SPC", "next-line"),
    ("q", "quit-window"),
];

pub const DIRED: &[(&str, &str)] = &[
    ("RET", "dired-find-file"),
    ("f", "dired-find-file"),
    ("e", "dired-find-file"),
    ("o", "dired-find-file-other-window"),
    ("v", "dired-view-file"),
    ("^", "dired-up-directory"),
    ("g", "revert-buffer"),
    ("n", "dired-next-line"),
    ("p", "dired-previous-line"),
    ("SPC", "dired-next-line"),
    ("d", "dired-flag-file-deletion"),
    ("u", "dired-unmark"),
    ("x", "dired-do-flagged-delete"),
    ("+", "dired-create-directory"),
    ("R", "dired-do-rename"),
    ("C", "dired-do-copy"),
    ("q", "quit-window"),
];

pub const SPECIAL: &[(&str, &str)] = &[
    ("q", "quit-window"),
    ("n", "next-line"),
    ("p", "previous-line"),
    ("SPC", "scroll-up-command"),
    ("DEL", "scroll-down-command"),
    ("RET", "special-mode-follow"),
    ("g", "revert-buffer"),
    ("<", "beginning-of-buffer"),
    (">", "end-of-buffer"),
];

pub fn build(defs: &[(&str, &str)]) -> Keymap {
    let mut m = Keymap::default();
    for (k, c) in defs {
        assert!(m.bind_str(k, c), "bad key {k}");
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_bound_commands_exist() {
        let mut missing = Vec::new();
        for table in [GLOBAL, MINIBUFFER, BUFFER_LIST, DIRED, SPECIAL] {
            for (k, c) in table {
                if crate::commands::lookup(c).is_none() {
                    missing.push(format!("{k} -> {c}"));
                }
            }
        }
        assert!(missing.is_empty(), "unimplemented: {missing:?}");
    }

    #[test]
    fn prefix_lookup() {
        let m = build(GLOBAL);
        let cx = Key::parse_seq("C-x").unwrap();
        assert!(matches!(m.lookup(&cx), Lookup::Prefix));
        let cxcf = Key::parse_seq("C-x C-f").unwrap();
        assert!(matches!(m.lookup(&cxcf), Lookup::Cmd("find-file")));
        let mg = Key::parse_seq("M-g").unwrap();
        assert!(matches!(m.lookup(&mg), Lookup::Cmd("goto-line")));
    }
}
