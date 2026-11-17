# tmacs

A small, fast, terminal-only text editor written in Rust that behaves like
`emacs -nw` with the configuration from `~/.emacs.cli.d/init.el`.

One ~5 MB binary, starts instantly, no Lisp runtime: the Emacs behaviour
used day to day (keys, kill ring, undo, windows, isearch, dired, …)
re-implemented natively and configured with a small TOML file.

- [Features](#features)
- [Install](#install)
- [Usage](#usage)
- [Configuration](#configuration)
  - [Settings](#settings)
  - [Faces (colors)](#faces-colors)
  - [Custom key bindings](#custom-key-bindings)
  - [Languages, indentation and formatters](#languages-indentation-and-formatters)
- [Key binding reference](#key-binding-reference)
- [Differences from Emacs](#differences-from-emacs)
- [Troubleshooting](#troubleshooting)
- [Development](#development)

## Features

**Editing like Emacs**
- Emacs key bindings and command names (`M-x` with completion), prefix
  arguments (`C-u`, `C-u C-u`, `C-u 8`, `M-5`, `M--`), `ESC` as Meta.
- Kill ring with appending consecutive kills, `C-y`/`M-y`; mark ring
  (`C-u C-SPC`), transient-mark region highlighting.
- Emacs undo model: undo is itself undoable (any other command, e.g.
  `C-g`, breaks the chain so the next undo redoes), 20-character
  self-insert grouping, `undo-redo` (`C-?`).
- Words, sentences, paragraphs, sexps/lists, defuns; transpose, case
  conversion, whitespace commands (`M-SPC`, `M-\`, `M-^`, `C-x C-o`),
  `M-z`, `M-q` fill-paragraph, `M-;`/`C-x C-;` comments.
- Rectangles (`C-x r k/y/o/c/t/d/N`), registers (`C-x r s/i/SPC/j`),
  bookmarks (`C-x r m/b/l`), keyboard macros (`C-x (`, `C-x )`,
  `C-x e e e`, `F3`/`F4`), `C-x z z z` repeat, `M-/` dabbrev.
- Narrowing (`C-x n n`, `C-x n w`), overwrite mode, quoted insert.

**Search**
- Incremental search (`C-s`, `C-r`, regexp with `C-M-s`), lazy highlight,
  wrapping, `C-w`/`C-y` to extend, `M-%` from isearch; search ring.
- `M-%` query-replace, `C-M-%` with regexps (`\1`, `\&` in replacements),
  `replace-string`, `replace-regexp`, `occur` (`M-s o`) with ``C-x ` ``
  next-error, `flush-lines`, `keep-lines`, `highlight-regexp`.
- Emacs regexp syntax (`\(…\)`, `\|`, `\{n\}`, `\<`, `\_<`) is accepted.

**Windows, buffers, files**
- Window splits (`C-x 2`, `C-x 3`, `C-x 0`, `C-x 1`, `C-x o`, `C-z`,
  `C-<tab>`), resizing (`C-x ^`, `C-x }`, `C-x +`), `C-M-v`.
- Minibuffer with file/buffer/command completion (`TAB`, `?`) and history
  (`M-p`/`M-n`), `y-or-n` prompts.
- Buffer menu (`C-x C-b`), dired (`C-x d`: open, `^`, flag/delete, rename,
  copy, mkdir), `C-x b`, `C-x k`, `C-x <left>/<right>`.
- Warns when a file changed on disk before saving, offers to create
  missing directories, keeps CRLF files CRLF, `revert-buffer`, read-only
  mode (`C-x C-q`).
- `M-!` / `M-|` shell commands, `C-x C-z` suspend.

**Display**
- Syntax highlighting for ~200 languages (Sublime/bat grammars via
  [syntect](https://github.com/trishume/syntect)) mapped to Emacs font-lock
  faces. Fontification is lazy like jit-lock: a 300k-line file opens and
  jumps to its end instantly while exact highlighting finishes in idle time.
- Continuation lines with `\`, or `toggle-truncate-lines`; `scroll-step 1`,
  `scroll-preserve-screen-position`, `C-l` cycling.
- Emacs mode line (`-UUU:**--  name   Top L12    (Go) [17/11]-[14:05] 0.53`),
  show-paren, trailing whitespace, visible bell (mode-line flash).
- Truecolor with fallback to the 256-color palette; custom cursor color.

**Around the editor**
- Format on save (`gofmt`, `terraform fmt`, `rustfmt`, `black`, `shfmt`,
  `clang-format`, `prettier`), like `format-all-mode`.
- System clipboard: kills go to `pbcopy` (OSC 52 over SSH, `wl-copy` or
  `xclip` on Linux); `C-y` picks up text copied in other apps.
- Mouse (xterm-mouse-mode): click, drag to select, double/triple click,
  wheel scrolls one line, click to select a window.
- `desktop-save-mode`: reopen last session's files and positions;
  `savehist-mode`: minibuffer and search history persist.
- Help: `C-h k`, `C-h c`, `C-h b`, `C-h f`, `C-h w`, `C-h m`, `C-h e`.

## Install

### Homebrew (macOS, Linux)

```sh
brew install gxben/tap/tmacs
```

The formula is published to
[gxben/homebrew-tap](https://github.com/gxben/homebrew-tap) on every
release. The example config
is installed to `$(brew --prefix)/share/tmacs/init.toml.example`.

### Prebuilt binaries

Each [GitHub release](../../releases) has
`tmacs_<version>_{darwin,linux}_{amd64,arm64}.tar.gz` archives and a
`tmacs_<version>_SHA256SUMS` file. Linux binaries are built on Ubuntu
22.04 (glibc 2.35+).

### From source

Requires a Rust toolchain (1.85+, edition 2024).

```sh
make install                    # release build, installs ~/.local/bin/tmacs
make install PREFIX=/usr/local  # → /usr/local/bin/tmacs
```

or `cargo install --path .` (installs to `~/.cargo/bin`).

Use it as your editor:

```sh
export EDITOR=tmacs VISUAL=tmacs
```

## Usage

```
tmacs [OPTION]... [+LINE[:COLUMN]] [FILE]...

  -q, --no-init-file   do not load ~/.config/tmacs/init.toml
  -nw                  ignored (tmacs is always terminal-only)
  --version            print version
  -h, --help           help
```

- `tmacs` with no file reopens the last session (`desktop-save-mode`).
- `tmacs +42 main.go` / `tmacs +42:7 main.go` open at a line/column.
- Two files open in a split window, like Emacs.
- Opening a directory starts dired.

Exit with `C-x C-c` (offers to save modified buffers).

## Configuration

tmacs reads `~/.config/tmacs/init.toml`, or the file named by
`$TMACS_INIT`. **Every setting is optional**: the built-in defaults already
reproduce `~/.emacs.cli.d/init.el`. Start from the annotated example:

```sh
mkdir -p ~/.config/tmacs
cp examples/init.toml ~/.config/tmacs/init.toml
```

Settings take effect on restart. If the file has an error, tmacs starts
with the defaults and shows the error in the echo area.

The file has four parts:

```toml
[settings]          # editor variables
[faces]             # colors
[keys]              # global key bindings
[[language]]        # one table per major mode to tweak (repeatable)
```

### Settings

| Setting | Default | Meaning (Emacs equivalent) |
| --- | --- | --- |
| `tab-width` | `8` | `tab-width` in text/fundamental modes |
| `indent-tabs-mode` | `false` | `indent-tabs-mode` in text/fundamental modes |
| `fill-column` | `70` | `fill-column` for `M-q` |
| `scroll-step` | `1` | when point leaves the window, scroll this many lines if enough, else recenter |
| `scroll-preserve-screen-position` | `true` | keep point's screen row on `C-v`/`M-v` |
| `next-screen-context-lines` | `1` | lines kept when paging |
| `show-trailing-whitespace` | `true` | highlight trailing blanks (also: `C-k` kills only to end of line) |
| `show-paren-mode` | `true` | highlight the matching bracket |
| `transient-mark-mode` | `true` | highlight the active region |
| `search-highlight` | `true` | highlight isearch matches |
| `case-fold-search` | `false` | case-insensitive search (smart case when `true`) |
| `visible-bell` | `true` | flash the mode line instead of beeping |
| `display-time` | `true` | clock in the mode line |
| `display-time-format` | `"[%-d/%-m]-[%H:%M]%L"` | strftime format; `%L` is the load average |
| `line-number-mode` | `true` | `L12` in the mode line |
| `column-number-mode` | `false` | `(12,4)` in the mode line |
| `truncate-lines` | `false` | truncate long lines instead of continuing them |
| `require-final-newline` | `true` | add a final newline on save (programming modes) |
| `xterm-mouse-mode` | `true` | mouse support |
| `mouse-wheel-lines` | `1` | lines per wheel step |
| `kitty-keyboard` | `true` | use the kitty keyboard protocol when the terminal supports it |
| `cursor-color` | `"yellow"` | `set-cursor-color`; remove the line to keep the terminal's |
| `frame-title-format` | `"tmacs - %b (%f)"` | terminal title; `%b` buffer, `%f` file; `""` disables |
| `highlight-tabs` | `false` | show tab characters with `tab-face` |
| `kill-whole-line` | `false` | `C-k` at line start also kills the newline |
| `delete-selection-mode` | `false` | typing replaces the active region |
| `format-on-save` | `true` | master switch for formatters on save |
| `format-timeout-ms` | `5000` | formatter time limit |
| `max-highlight-size` | `20971520` | no syntax highlighting above this many characters |
| `clipboard` | `"auto"` | `auto`, `pbcopy`, `osc52`, `wayland`, `xclip`, `none` |
| `yes-or-no-short` | `true` | `(fset 'yes-or-no-p 'y-or-n-p)` |
| `desktop-save-mode` | `true` | reopen the last session when started without files |
| `savehist-mode` | `true` | persist minibuffer and search history |
| `make-backup-files` | `false` | accepted for compatibility; tmacs never writes backups |

Example:

```toml
[settings]
fill-column = 80
case-fold-search = true
display-time-format = "%H:%M"
clipboard = "osc52"
```

`clipboard = "auto"` picks OSC 52 inside SSH sessions, `pbcopy`/`pbpaste`
on macOS, `wl-copy` on Wayland, `xclip` on X11, else none.

### Faces (colors)

Each face accepts `fg`, `bg`, `bold`, `italic`, `underline`, `reverse`.
Colors are X11/Emacs names (`DarkSlateGray`, `gray80`, `seagreen3`,
`light slate blue`) or `#rrggbb`/`#rgb`. Entries are merged over the
defaults, so list only what changes.

```toml
[faces]
default = { fg = "wheat", bg = "DarkSlateGray" }
region = { bg = "black" }
font-lock-keyword-face = { fg = "orange", bold = true }
mode-line = { fg = "black", bg = "grey75" }
```

Available faces:

- **UI:** `default`, `region`, `highlight`, `mode-line`,
  `mode-line-inactive`, `mode-line-buffer-id`, `minibuffer-prompt`,
  `isearch`, `isearch-fail`, `lazy-highlight`, `show-paren-match`,
  `show-paren-mismatch`, `trailing-whitespace`, `tab-face`,
  `vertical-border`, `escape-glyph`, `error`, `warning`, `success`,
  `completions-common-part`, `dired-directory`.
- **Syntax:** `font-lock-comment-face`, `font-lock-doc-face`,
  `font-lock-string-face`, `font-lock-keyword-face`,
  `font-lock-builtin-face`, `font-lock-function-name-face`,
  `font-lock-variable-name-face`, `font-lock-type-face`,
  `font-lock-constant-face`, `font-lock-preprocessor-face`,
  `font-lock-warning-face`.
- **Markup:** `bold`, `italic`, `underline`, `bold-italic`, `link`,
  `header`, `diff-added`, `diff-removed`.

### Custom key bindings

`[keys]` maps Emacs key notation to command names (anything `M-x`
offers; `C-h f` documents them). An empty string unbinds a key.

```toml
[keys]
"C-c f" = "format-buffer"
"C-c l" = "toggle-truncate-lines"
"<f5>" = "revert-buffer"
"C-x C-b" = "buffer-menu"     # list buffers in the current window
"C-x C-z" = ""                # unbind suspend
```

Notation: `C-` control, `M-` meta, `S-` shift, `RET`, `TAB`, `DEL`,
`ESC`, `SPC`, `<f1>`…`<f12>`, `<left>`, `<home>`, `<prior>`/`<next>`
(page up/down), `<deletechar>`, `<insert>`, `C-<tab>`, `C-S-<tab>`.
Sequences are space separated: `"C-x r t"`.

### Languages, indentation and formatters

The major mode is chosen from the file name (`patterns`), then an Emacs
`-*- mode: x -*-` cookie, then the extension/shebang known to the bundled
grammars. Built-in modes: c, c++, objc, go, gomod, rust, java, kotlin,
scala, swift, csharp, javascript, typescript, tsx, json, css, scss, less,
php, dart, zig, proto, groovy, graphql, terraform, nix, jsonnet, rego,
python, yaml, toml, ini, sh, fish, makefile, cmake, dockerfile, ruby, perl,
lua, sql, haskell, elisp, clojure, racket, erlang, elixir, ocaml, r, julia,
html, xml, vue, svelte, markdown, latex, diff, git-commit, powershell, vim,
awk, nginx, jinja, csv, asm, text. Any other bundled grammar (≈200) gets a
generic mode automatically. `C-h m` shows the active mode's settings.

Each `[[language]]` entry overrides an existing mode (by `name`) or
defines a new one:

| Key | Meaning |
| --- | --- |
| `name` | mode name (`go`, `yaml`, …), required |
| `patterns` | file-name regexps selecting this mode (Emacs `\\'` accepted) |
| `syntax` | syntect grammar name (e.g. `"Bourne Again Shell (bash)"`) |
| `mode-name` | text shown in the mode line |
| `indent` | indentation width (`c-basic-offset`, …) |
| `tabs` | indent with tabs |
| `tab-width` | display width of a tab |
| `indent-style` | `braces`, `python`, `yaml`, `lisp`, `shell`, `relative` |
| `comment-start`, `comment-end` | used by `M-;`, `C-x C-;`, `M-q` |
| `formatter` | argv; buffer text on stdin, result on stdout; `{file}` = file path |
| `format-on-save` | run the formatter before saving |
| `format-exclude` | file-name regexps never formatted on save |
| `require-final-newline` | add a final newline on save |

Examples:

```toml
# Use ruff instead of black for Python
[[language]]
name = "python"
formatter = ["ruff", "format", "--stdin-filename", "{file}", "-"]

# Helm charts: YAML mode, never formatted
[[language]]
name = "yaml"
patterns = ['/templates/.*\.ya?ml$', '\.ya?ml\.gotmpl$']
format-exclude = ['/templates/', '\.gotmpl$']

# Don't format Markdown on save
[[language]]
name = "markdown"
format-on-save = false

# A new mode for .jenkins files
[[language]]
name = "jenkins"
syntax = "Groovy"
patterns = ['\.jenkins$']
indent = 4
comment-start = "//"
indent-style = "braces"
```

Default formatters:

| Modes | Formatter | On save |
| --- | --- | --- |
| go | `gofmt` | yes |
| terraform (`.tf`, `.tfvars`, `.hcl`) | `terraform fmt -` | yes |
| rust | `rustfmt --edition 2024` | yes |
| python | `black -q -` | yes |
| sh | `shfmt` | yes |
| c, c++, objc | `clang-format` (honours `.clang-format`) | yes |
| typescript, javascript, tsx, json, yaml, markdown, css, scss, less, html, vue, svelte, graphql | `prettier --stdin-filepath {file}` | yes |
| java, proto, csharp | `clang-format` | no |
| lua, nix, zig, toml, dart, elixir, swift, jsonnet, rego | stylua, nixfmt, zig fmt, taplo, … | no |

Templated YAML (`.gotmpl`, `.j2`, `.tpl`) is never formatted. When a
formatter is missing or fails, the file is saved unformatted and the
error is shown in the echo area. `M-x format-buffer` formats on demand;
`M-x toggle-format-on-save` toggles the current mode for this session.
Formatting is a single undo step.

Indentation: `TAB` indents the line (or the active region) for the mode;
in Python and YAML repeated `TAB` cycles through indentation levels; in
text modes it indents relative to the previous line. `RET` indents the
new line in programming modes (`electric-indent-mode`) and removes
trailing whitespace from the line it leaves; `C-j` inserts a plain
newline. Closing brackets re-indent their line as they are typed.

### Files written by tmacs

Next to the config file (`~/.config/tmacs/`):

| File | Content |
| --- | --- |
| `desktop.toml` | buffers and positions of the last session |
| `history.toml` | minibuffer and search history |
| `bookmarks` | `C-x r m` bookmarks |

Sessions started with file arguments (e.g. tmacs as `$EDITOR` for a git
commit) don't touch `desktop.toml`.

## Key binding reference

Standard Emacs bindings apply. Highlights, plus the `init.el` ones (★):

| Keys | Command |
| --- | --- |
| `C-f` `C-b` `C-n` `C-p` `C-a` `C-e` | character / line motion (visual lines) |
| `M-f` `M-b` `M-a` `M-e` `M-{` `M-}` | words, sentences, paragraphs |
| `C-M-f` `C-M-b` `C-M-u` `C-M-d` `C-M-a` `C-M-e` | sexps, lists, defuns |
| `M-<` `M->` `C-v` `M-v` `C-l` `M-r` | buffer ends, paging, recenter |
| `M-g` ★ | goto-line |
| `<home>` `<end>` ★ | beginning / end of line |
| `C-SPC` `C-x C-x` `C-x h` `M-h` `M-@` | mark and region |
| `C-w` `M-w` `C-y` `M-y` `C-k` `M-d` | kill, copy, yank |
| `M-DEL` ★ | backward-kill-word |
| `C-_` `C-/` `C-x u` | undo |
| `C-t` `M-t` `C-x C-t` | transpose |
| `M-u` `M-l` `M-c` `C-x C-u` `C-x C-l` | case |
| `M-;` `C-x C-;` | comments |
| `TAB` `C-M-\` `C-x TAB` | indentation |
| `M-/` `S-<tab>` ★ | dabbrev-expand |
| `C-x p` ★ | insert `current-time-string` |
| `C-s` `C-r` `C-M-s` `M-%` `C-M-%` `M-s o` | search, replace, occur |
| `C-x C-f` `C-x C-s` `C-x C-w` `C-x s` `C-x i` `C-x C-v` | files |
| `C-x b` `C-x C-b` `C-x <left>` `C-x <right>` | buffers |
| `C-x k` ★ | kill current buffer (asks only if modified) |
| `C-x 2` `C-x 3` `C-x 0` `C-x 1` `C-x o` | windows |
| `C-z` `C-<tab>` ★ / `C-S-<tab>` ★ | next / previous window |
| `C-x (` `C-x )` `C-x e` `<f3>` `<f4>` | keyboard macros |
| `C-x r …` | rectangles, registers, bookmarks |
| `M-x` `C-u` `C-g` `M-!` `M-\|` | misc |
| `C-h k` `C-h c` `C-h b` `C-h f` `C-h w` `C-h m` | help |

`C-h b` lists every binding.

## Differences from Emacs

- No Emacs Lisp: configuration is TOML and commands are built in;
  `M-x set-variable` / `M-:` are not available.
- Not implemented: Gnus, Copilot, flyspell/ispell (`C-M-RET` says so),
  `M-x shell` / `compile`, LSP, window layout in the desktop.
- `*Completions*` is a popup above the echo area, not a buffer.
- Indentation is rule-based per mode rather than cc-mode/SMIE;
  formatters normalize code on save.

## Troubleshooting

- **`C-<tab>`, `C-S-<tab>`, `C-M-RET` do nothing.** They need the kitty
  keyboard protocol (kitty, WezTerm, Ghostty, foot, iTerm2 with "Report
  keys using CSI u"). tmacs detects support at startup. `C-z` and `C-x o`
  always work.
- **Meta keys insert odd characters.** Set the terminal's Option key to
  act as Meta / Esc+ (iTerm2: Profiles › Keys › Left Option key = Esc+),
  or type `ESC` then the key.
- **What does a key do?** `C-h c` then the key prints its command;
  `C-h k` shows its documentation.
- **Colors look off.** Truecolor is used when `$COLORTERM` is
  `truecolor`/`24bit` or the terminal is known to support it, otherwise
  the 256-color palette (Terminal.app).
- **Selecting text with the mouse for the terminal's own copy.** Mouse
  reporting captures drags: hold Option (macOS terminals) or Shift (most
  Linux terminals), or set `xterm-mouse-mode = false`. Kills already go to
  the system clipboard.

## Development

```sh
make help        # list targets
make build       # debug build
make test        # unit + end-to-end tests
make lint        # clippy, warnings are errors
make check       # fmt-check + lint + test
make release     # optimized binary in target/release/
make install     # install to ~/.local/bin (PREFIX=...)
make package     # dist/tmacs_<version>_<os>_<arch>.tar.gz + .sha256
make dist        # check + package
make version V=1.2.3   # bump Cargo.toml/Cargo.lock (release CI does this)
make bench BENCH_FILE=big.go   # syntax highlighting throughput
```

### Continuous integration

| Workflow | Trigger | What it does |
| --- | --- | --- |
| `ci.yml` | push/PR on `main` | rustfmt, clippy (`-D warnings`), tests and release build on Linux amd64/arm64 and macOS |
| `sec.yml` | push/PR, weekly | CodeQL analysis of the Rust code and the workflows |
| `vuln.yml` | `Cargo.*` changes, weekly | `cargo audit` against the RustSec advisory database |
| `release.yml` | manual (Actions › Trigger Release › Run workflow) | release pipeline below |

### Releasing

Releases are cut from `main` by running the **Trigger Release** workflow:

1. [semantic-release](https://semantic-release.gitbook.io/) reads the
   Conventional Commits since the last `v*` tag: `feat` → minor,
   `fix`/`perf`/`chore` → patch, `!`/`BREAKING CHANGE` → major (`docs`,
   `test`, `build`, `ci` don't release). Scopes (`feat(dired): …`) are
   supported.
2. It sets the version in `Cargo.toml`/`Cargo.lock`
   (`scripts/set-version.sh`), updates `CHANGELOG.md`, commits
   `chore: version X.Y.Z [skip ci]`, tags `vX.Y.Z` and creates the GitHub
   release with generated notes.
3. Binaries are built with `make package` on Linux (amd64, arm64) and
   macOS (Intel, Apple Silicon), smoke-tested, and attached to the release
   with a `tmacs_X.Y.Z_SHA256SUMS` file.
4. `scripts/homebrew-formula.sh` renders `Formula/tmacs.rb` from the
   checksums and pushes it to the tap repository.

Repository setup:

| Kind | Name | Purpose |
| --- | --- | --- |
| secret | `PAT_HOMEBREW_TAP_RELEASER` | token with `contents: write` on the tap repo (formula push is skipped with a warning when unset) |
| variable | `HOMEBREW_TAP_REPO` | optional, tap repository (default `<owner>/homebrew-tap`) |
| variable | `RELEASE_BOT_CLIENT_ID` | optional GitHub App client id, to push the release commit through branch protection |
| secret | `RELEASE_BOT_PRIVATE_KEY` | private key of that GitHub App |

Create the tap once: an empty `<owner>/homebrew-tap` repository on
GitHub. Without the GitHub App, the workflow uses `GITHUB_TOKEN`, which
needs `main` to accept pushes from GitHub Actions. The first release is
`1.0.0` unless a `v0.x.y` tag exists (e.g. `git tag v0.1.0` to start in
0.x).

See [CLAUDE.md](CLAUDE.md) for the architecture and contribution notes.

## License

Copyright 2026 Benjamin Zores.

Licensed under the [Apache License, Version 2.0](LICENSE). Unless required
by applicable law or agreed to in writing, software distributed under the
License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
CONDITIONS OF ANY KIND, either express or implied.
