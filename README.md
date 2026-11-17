# tmacs

A small, fast, terminal-only text editor written in Rust that behaves like
Emacs (`emacs -nw`) with the configuration from `~/.emacs.cli.d/init.el`.

- Emacs key bindings, prefix arguments, kill ring, mark ring, undo (with
  undo-of-undo redo), keyboard macros, registers, rectangles, isearch,
  query-replace, occur, dired, buffer menu, dabbrev, M-x with completion.
- Syntax highlighting for ~200 languages (Sublime/bat grammars through
  [syntect](https://github.com/trishume/syntect)), mapped to Emacs font-lock
  faces and fontified lazily like jit-lock, so huge files open instantly.
- Format-on-save hooks per language (`gofmt`, `terraform fmt`, `prettier`,
  `rustfmt`, `black`, `clang-format`, `shfmt`, ...).
- Mouse support (xterm-mouse-mode): click, drag to select, double/triple
  click, wheel scrolls one line.

## Build and install

```sh
cargo install --path .
# or: cargo build --release && cp target/release/tmacs ~/bin/
```

Usage: `tmacs [-q] [+LINE[:COL]] [FILE]...`

To use it as your editor: `export EDITOR=tmacs VISUAL=tmacs`.

## Configuration

`~/.config/tmacs/init.toml` (or `$TMACS_INIT`). Defaults already mirror
`init.el`; see [`examples/init.toml`](examples/init.toml) for every option.

```toml
[settings]
case-fold-search = false
display-time-format = "[%-d/%-m]-[%H:%M]%L"

[faces]
default = { fg = "wheat", bg = "DarkSlateGray" }
font-lock-keyword-face = { fg = "orange" }

[keys]
"C-c f" = "format-buffer"

[[language]]
name = "python"
formatter = ["ruff", "format", "-"]
format-on-save = true
```

Colors accept X11/Emacs names (`DarkSlateGray`, `gray80`, `seagreen3`) or
`#rrggbb`. Truecolor is used when the terminal supports it, otherwise colors
are mapped to the 256-color palette.

### Languages

`[[language]]` entries can override: `patterns` (regexps on the file name,
Emacs `\\'` anchors accepted), `syntax` (syntect grammar name), `mode-name`,
`indent`, `tabs`, `tab-width`, `indent-style` (`braces`, `python`, `yaml`,
`lisp`, `shell`, `relative`), `comment-start`, `comment-end`, `formatter`
(argv, `{file}` is replaced by the file path; the buffer is sent on stdin),
`format-on-save`, `require-final-newline`.

Format on save is enabled by default for Go, Terraform/HCL and TypeScript
(the hooks in `init.el`); other languages have a formatter configured that
runs with `M-x format-buffer`, toggle per mode with
`M-x toggle-format-on-save`.

## Key bindings

Standard Emacs bindings, plus the ones from `init.el`:

| Key | Command |
| --- | --- |
| `M-g` | goto-line |
| `C-x k` | kill current buffer (no buffer-name prompt) |
| `C-x p` | insert `current-time-string` |
| `C-z`, `C-<tab>` | other-window |
| `C-S-<tab>` | previous window |
| `S-<tab>`, `M-/` | dabbrev-expand |
| `M-DEL` | backward-kill-word |
| `<home>` / `<end>` | beginning / end of line |

`C-h k`, `C-h b`, `C-h f`, `C-h w` describe keys and commands.
`C-<tab>`, `C-S-<tab>` and `C-M-RET` need a terminal implementing the kitty
keyboard protocol (kitty, WezTerm, Ghostty, foot, recent iTerm2 with CSI u);
`ESC` works as Meta everywhere.

## Not (yet) supported

Emacs Lisp, Gnus, Copilot, flyspell/ispell, `M-x shell`/`compile`, LSP.
