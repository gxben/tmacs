# CLAUDE.md

Guidance for working on **tmacs**, a terminal-only Rust editor that
replaces the user's `emacs -nw` setup (`~/.emacs.cli.d/init.el`).

## Goal and ground rules

- **Emacs fidelity first.** Keys, command names, messages ("End of
  buffer", "Mark set", "Quit"), prompts and edge-case behaviour should match
  Emacs. When unsure, check what Emacs does; don't invent behaviour.
- The user's `init.el` customizations are the defaults (colors, `M-g`
  goto-line, `C-x k`, `C-x p`, `C-z`, case-sensitive search,
  `scroll-step 1`, …). Don't change them without being asked.
- Terminal only, lightweight, fast. Avoid heavy dependencies; no async
  runtime. Large files must stay responsive (see jit-lock in `syntax.rs`).
- Work on local `main`, one Conventional Commit per change/feature
  (`feat(scope):`, `fix(scope):`, `test:`, `docs:`, `build:`, `perf:`).
  Never push. Keep fixes and features in separate commits.

## License headers

Apache-2.0. Every source file starts with:

```rust
// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Benjamin Zores
```

(`#` comments in shell scripts and the Makefile). Add it to new files.

## Commands

```sh
make build     # debug build
make test      # all tests (~0.2s)
make lint      # clippy --all-targets -D warnings
make check     # fmt-check + lint + test — run before every commit
make release   # target/release/tmacs
```

`cargo fmt` before committing; clippy must stay at zero warnings.

## Releases and CI

- Never edit the version by hand: `.github/workflows/release.yml`
  (manual dispatch) runs semantic-release, which derives the version from
  commit types, runs `scripts/set-version.sh`, updates `CHANGELOG.md`, tags
  `vX.Y.Z`, builds `make package` archives for linux/darwin × amd64/arm64
  and publishes the Homebrew formula (`scripts/homebrew-formula.sh`) to
  the tap. Commit types therefore matter: `feat` = minor, `fix`/`perf`/
  `chore` = patch, `docs`/`test`/`build`/`ci` = no release.
- Release asset names `tmacs_<version>_<os>_<arch>.tar.gz` are shared by
  the Makefile (`package`), the workflow and the formula script; change
  them together. The archive contains `tmacs`, `README.md`, `LICENSE` and
  `init.toml.example`.
- `ci.yml` mirrors `make fmt-check lint test`; keep the Makefile targets
  as the single source of truth for CI commands.

## Architecture (src/)

| Module | Role |
| --- | --- |
| `main.rs` | CLI args, terminal setup/restore (raw mode, alt screen, mouse SGR 1000/1002/1006, kitty keyboard, bracketed paste, cursor color), event loop, mouse handling, idle fontification |
| `editor.rs` | `Editor` state: buffers, windows + `Layout` tree, minibuffer (`Mini`, `PromptKind`), read-key prompts (`ReadKey`, `YnAction`), key dispatch (`handle_key`), prefix args, command loop (`run_command` → `begin_command`/`post_command`), kill ring, macros |
| `commands.rs` | All interactive commands; `commands!` table maps Emacs names → `fn(&mut Editor)` + doc string. Editing, motion, indentation, comments, dabbrev, rectangles, registers, windows, misc |
| `files.rs` | Visiting/saving, format-on-save (`run_formatter`), minibuffer accept handlers (`accept`) and completion, buffer menu, dired, help buffers, shell commands, bookmarks, y/n answers |
| `search.rs` | isearch state machine, query-replace, occur/next-error, highlight-regexp, Emacs→Rust regex translation |
| `keymap.rs` | Prefix-key trie + default binding tables (`GLOBAL`, `MINIBUFFER`, `BUFFER_LIST`, `DIRED`, `SPECIAL`) |
| `key.rs` | `Key` type, crossterm event normalization (legacy C-4..C-7, kitty shift layer, C-i/C-m/C-[), Emacs notation parse/print |
| `buffer.rs` | Rope text, markers (`MarkerId`, insertion types), Emacs undo (groups, amalgamation, undo-of-undo), narrowing, mark ring |
| `view.rs` | Window geometry: visual rows/continuation lines, `vpos`, vertical motion helpers, scrolling (`ensure_visible` with scroll-step) |
| `render.rs` | Redisplay: windows, overlays (region, isearch, paren, highlight), mode line, echo area, completions popup, screen→buffer position |
| `screen.rs` | Cell grid with diffing flush, truecolor/256 output |
| `syntax.rs` | syntect grammars → font-lock face ids; checkpointed parse states, per-line span cache, time-budgeted exact parse + approximate fallback |
| `lang.rs` | Major modes table (`DEFS`, `PATTERNS`), detection, indentation rules (`compute_indent`), default formatters |
| `config.rs` | `init.toml` schema (`Settings`, faces, keys, `[[language]]`) |
| `face.rs` / `color.rs` | Faces (Emacs names) and X11 color names / 256-color mapping |
| `clipboard.rs` | pbcopy/pbpaste, wl-copy, xclip, OSC 52 |
| `session.rs` | desktop-save-mode / savehist-mode persistence |
| `text.rs` | Char widths, word/sentence/paragraph/sexp motion, bracket matching, line wrapping |
| `tests.rs` | Headless end-to-end tests (keys → buffer/point) |

### Key invariants

- **Point lives in window markers.** Use `ed.pt()` / `ed.goto(p)`;
  `goto` clamps to the accessible (narrowed) region. Other windows' points
  and window starts are markers that shift on edits automatically.
- **Edit through `Buffer::insert` / `delete`** (or `ed.insert`, `ed.delete`,
  `ed.kill`). They record undo, shift markers, invalidate syntax from the
  edited line and set `deactivate` (the region is dropped after the
  command, like `deactivate-mark`).
- Commands must call `ed.check_writable()` before modifying.
- **Undo boundaries** are added in `post_command`; self-inserts amalgamate.
  Don't call `undo_boundary` from commands unless you know why.
- **Prefix args:** read with `ed.arg` / `ed.count()`; `arg_universal`
  (C-u seen) and `arg_digits` (explicit number). Commands that wait for
  more input (`read_key`) set `ed.keep_arg = true`.
- **Errors:** `ed.error(msg)` shows the message, dings and stops keyboard
  macros. Use Emacs' exact wording.
- **Minibuffer:** `ed.read_string(prompt, PromptKind::X, initial,
  Completion::Y, "history")`; handle the result in `files::accept`.
  History names must be listed in `session::HIST_NAMES` to persist.
- **Single-key prompts:** set `ed.read_key = Some(ReadKey::…)` and handle
  it in `Editor::handle_read_key`; y/n via `ed.yes_or_no(prompt,
  YnAction::…)` + `files::yn_answer`.

## How to …

- **Add a command:** write `fn name(ed: &mut Editor)` in `commands.rs`
  (or `files.rs`/`search.rs`), register it in the `commands!` table with
  its Emacs name and doc, bind it in `keymap::GLOBAL` if Emacs binds it,
  add an end-to-end test in `tests.rs`. `keymap::tests` fails if a binding
  points to an unknown command.
- **Add a setting:** field in `config::Settings` (kebab-case in TOML) +
  default in `Default`, document it in README's settings table and
  `examples/init.toml`.
- **Add a language:** row in `lang::DEFS` (name, mode-name, syntect
  grammar, indent, tabs, comment, indent style), file patterns in
  `PATTERNS` if extension detection isn't enough, formatter in
  `default_formatter`. List grammar names with `SYN=… SRC=… cargo test
  dump_scopes -- --ignored --nocapture` (prints scopes of a line).
- **Fix highlighting colors:** adjust `SCOPE_FACES` in `syntax.rs`
  (most specific selector wins) and extend `languages::major_languages`.

## Testing

- `src/tests.rs` drives the real dispatcher: `ed("text with |point")`,
  `k(&mut e, "C-x C-f")` for keys, `t(&mut e, "text\n")` to type,
  `show(&e)` to compare text with `|` at point. Prefer these tests for
  behaviour; unit tests live next to the code they test.
- Tests must not touch the user's home: use `tmpdir()` and
  `test_config()` (clipboard `none`). Never set process-wide env vars in
  tests (they run in parallel).
- For interactive checks, run the binary in a pty (e.g. a small Python
  `pty` + `pyte` script) with `TMACS_INIT` pointing at a scratch config
  that sets `kitty-keyboard = false` (pyte doesn't answer the keyboard
  protocol query) and a scratch `HOME`.
- Formatter tests use stand-ins (`tr a-z A-Z`, `false`) so they don't
  depend on installed tools.
