//! Headless end-to-end tests: key sequences go through the real dispatcher,
//! keymaps and commands, and the resulting buffer/point is checked.
//!
//! Buffer text uses `|` to mark point, e.g. `"foo |bar"`.

use crate::config::Config;
use crate::editor::Editor;
use crate::key::{K, Key};
use crate::lang::LangConfig;
use crate::render;
use crate::screen::Screen;

fn editor_with(cfg: Config) -> Editor {
    let mut ed = Editor::new(cfg, None);
    ed.resize(80, 24);
    ed
}

fn test_config() -> Config {
    let mut cfg = Config::default();
    cfg.settings.clipboard = "none".into();
    cfg
}

/// Editor whose current buffer holds `text` ('|' = point), in `lang` mode.
fn ed_lang(text: &str, lang: &str) -> Editor {
    let mut ed = editor_with(test_config());
    let p = text
        .find('|')
        .map(|i| text[..i].chars().count())
        .unwrap_or(0);
    let clean = text.replacen('|', "", 1);
    let b = ed.cur();
    let buf = ed.bufm(b);
    buf.undo_disabled = true;
    buf.insert(0, &clean);
    buf.undo_disabled = false;
    buf.clear_undo();
    buf.modified = false;
    if !lang.is_empty() {
        let i = ed.langs.find(lang).expect("lang");
        ed.bufm(b).lang = i;
    }
    ed.goto(p);
    ed
}

fn ed(text: &str) -> Editor {
    ed_lang(text, "")
}

/// Send Emacs key notation, e.g. "C-x C-f".
fn k(ed: &mut Editor, keys: &str) {
    for key in Key::parse_seq(keys).unwrap_or_else(|| panic!("bad keys {keys}")) {
        ed.handle_key(key);
    }
}

/// Type literal text ('\n' = RET).
fn t(ed: &mut Editor, s: &str) {
    for c in s.chars() {
        let key = if c == '\n' {
            Key::new(K::Enter)
        } else {
            Key::new(K::Char(c))
        };
        ed.handle_key(key);
    }
}

/// Buffer text with '|' at point.
fn show(ed: &Editor) -> String {
    let s = ed.b().text.to_string();
    let p = ed.pt();
    let i = s.char_indices().nth(p).map(|(i, _)| i).unwrap_or(s.len());
    format!("{}|{}", &s[..i], &s[i..])
}

fn redisplay(ed: &mut Editor) {
    let mut scr = Screen::new(ed.width, ed.height, ed.faces.default);
    render::redisplay(ed, &mut scr);
}

fn tmpdir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("tmacs-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

// ---------------- motion ----------------

#[test]
fn char_and_line_motion() {
    let mut e = ed("|abc\ndef\nghi");
    k(&mut e, "C-f C-f C-n");
    assert_eq!(show(&e), "abc\nde|f\nghi");
    k(&mut e, "C-a");
    assert_eq!(show(&e), "abc\n|def\nghi");
    k(&mut e, "C-e C-p");
    assert_eq!(show(&e), "abc|\ndef\nghi");
    k(&mut e, "C-b");
    assert_eq!(show(&e), "ab|c\ndef\nghi");
}

#[test]
fn goal_column_is_kept() {
    let mut e = ed("abcdef|\nx\nabcdef");
    k(&mut e, "C-n C-n");
    assert_eq!(show(&e), "abcdef\nx\nabcdef|");
}

#[test]
fn end_of_buffer_errors_and_dings() {
    let mut e = ed("a\n|b");
    k(&mut e, "C-n");
    assert_eq!(e.message.as_deref(), Some("End of buffer"));
    assert!(e.bell.is_some());
}

#[test]
fn word_motion_and_buffer_ends() {
    let mut e = ed("|foo bar_baz qux");
    k(&mut e, "M-f M-f");
    assert_eq!(show(&e), "foo bar|_baz qux");
    k(&mut e, "M-b");
    assert_eq!(show(&e), "foo |bar_baz qux");
    k(&mut e, "M->");
    assert_eq!(show(&e), "foo bar_baz qux|");
    assert_eq!(e.b().mark_pos(), Some(4));
    k(&mut e, "M-<");
    assert_eq!(show(&e), "|foo bar_baz qux");
}

#[test]
fn esc_is_meta_prefix() {
    let mut e = ed("|foo bar");
    k(&mut e, "ESC f");
    assert_eq!(show(&e), "foo| bar");
}

#[test]
fn back_to_indentation_and_paragraphs() {
    let mut e = ed("a\nb\n\n   c|d\ne");
    k(&mut e, "M-m");
    assert_eq!(show(&e), "a\nb\n\n   |cd\ne");
    k(&mut e, "M-{");
    assert_eq!(show(&e), "a\nb\n|\n   cd\ne");
    k(&mut e, "M-}");
    assert_eq!(show(&e), "a\nb\n\n   cd\ne|");
}

#[test]
fn sexp_motion() {
    let mut e = ed("|(a (b) c) d");
    k(&mut e, "C-M-f");
    assert_eq!(show(&e), "(a (b) c)| d");
    k(&mut e, "C-M-b");
    assert_eq!(show(&e), "|(a (b) c) d");
}

#[test]
fn goto_line_via_minibuffer() {
    let mut e = ed("|1\n2\n3\n4");
    k(&mut e, "M-g");
    assert!(e.mini.is_some());
    t(&mut e, "3\n");
    assert_eq!(show(&e), "1\n2\n|3\n4");
    assert!(e.mini.is_none());
}

// ---------------- prefix arguments ----------------

#[test]
fn universal_argument_repeats() {
    let mut e = ed("|");
    k(&mut e, "C-u x");
    assert_eq!(show(&e), "xxxx|");
    k(&mut e, "C-u C-u y");
    assert_eq!(e.b().text.to_string(), format!("xxxx{}", "y".repeat(16)));
    k(&mut e, "C-u 3 z");
    assert!(show(&e).ends_with("zzz|"));
    k(&mut e, "M-2 w");
    assert!(show(&e).ends_with("zzzww|"));
}

#[test]
fn negative_argument() {
    let mut e = ed("abc|");
    k(&mut e, "M-- C-d");
    assert_eq!(show(&e), "ab|");
}

// ---------------- killing and yanking ----------------

#[test]
fn kill_line_appends_and_yanks() {
    let mut e = ed("|one\ntwo\nthree");
    k(&mut e, "C-k C-k C-k");
    assert_eq!(show(&e), "|\nthree");
    assert_eq!(e.kill_ring[0], "one\ntwo");
    k(&mut e, "C-y");
    assert_eq!(show(&e), "one\ntwo|\nthree");
}

#[test]
fn yank_pop_cycles() {
    let mut e = ed("|a b c");
    k(&mut e, "M-d M-f C-f M-d C-e");
    // two separate kills: "a" then "c" (non-consecutive)
    assert_eq!(e.kill_ring[..2], ["c".to_string(), "a".to_string()]);
    k(&mut e, "C-y");
    assert!(show(&e).ends_with("c|"));
    k(&mut e, "M-y");
    assert!(show(&e).ends_with("a|"));
}

#[test]
fn backward_kill_word_prepends() {
    let mut e = ed("foo bar|");
    k(&mut e, "M-DEL M-DEL");
    assert_eq!(e.kill_ring[0], "foo bar");
    assert_eq!(show(&e), "|");
}

#[test]
fn region_kill_and_copy() {
    let mut e = ed("|hello world");
    k(&mut e, "C-SPC M-f M-w");
    assert_eq!(e.kill_ring[0], "hello");
    assert!(!e.b().mark_active);
    k(&mut e, "C-x C-x C-w");
    assert_eq!(show(&e), "| world");
    k(&mut e, "C-e C-y");
    assert_eq!(show(&e), " worldhello|");
}

#[test]
fn set_mark_twice_deactivates() {
    let mut e = ed("|abc");
    k(&mut e, "C-SPC");
    assert!(e.b().mark_active);
    k(&mut e, "C-SPC");
    assert!(!e.b().mark_active);
}

#[test]
fn delete_backward_deletes_active_region() {
    let mut e = ed("|abc def");
    k(&mut e, "C-SPC M-f DEL");
    assert_eq!(show(&e), "| def");
    assert!(e.kill_ring.is_empty());
}

#[test]
fn zap_to_char() {
    let mut e = ed("|abc,def,ghi");
    k(&mut e, "M-z ,");
    assert_eq!(show(&e), "|def,ghi");
    assert_eq!(e.kill_ring[0], "abc,");
}

// ---------------- undo ----------------

#[test]
fn undo_and_redo_after_break() {
    let mut e = ed("|");
    t(&mut e, "abc");
    k(&mut e, "C-x u");
    assert_eq!(show(&e), "|");
    assert!(!e.b().modified);
    k(&mut e, "C-f C-g C-_");
    assert_eq!(e.b().text.to_string(), "abc");
}

#[test]
fn self_insert_amalgamates_twenty_chars() {
    let mut e = ed("|");
    t(&mut e, &"x".repeat(25));
    k(&mut e, "C-/");
    assert_eq!(e.b().text.to_string(), "x".repeat(20));
}

// ---------------- editing commands ----------------

#[test]
fn transpose_commands() {
    let mut e = ed("ab|c");
    k(&mut e, "C-t");
    assert_eq!(show(&e), "acb|");
    let mut e = ed("foo |bar");
    k(&mut e, "M-t");
    assert_eq!(show(&e), "bar foo|");
    let mut e = ed("one\n|two\n");
    k(&mut e, "C-x C-t");
    assert_eq!(show(&e), "two\none\n|");
}

#[test]
fn case_commands() {
    let mut e = ed("|hello world again");
    k(&mut e, "M-u M-c M-l");
    assert_eq!(show(&e), "HELLO World again|");
}

#[test]
fn whitespace_commands() {
    let mut e = ed("a   |   b");
    k(&mut e, "M-SPC");
    assert_eq!(show(&e), "a |b");
    let mut e = ed("a   |   b");
    k(&mut e, "M-\\");
    assert_eq!(show(&e), "a|b");
    let mut e = ed("foo\n    |bar");
    k(&mut e, "M-^");
    assert_eq!(show(&e), "foo| bar");
    let mut e = ed("a\n\n|\n\nb");
    k(&mut e, "C-x C-o");
    assert_eq!(e.b().text.to_string(), "a\n\nb");
}

#[test]
fn open_line_keeps_point() {
    let mut e = ed("ab|cd");
    k(&mut e, "C-o");
    assert_eq!(show(&e), "ab|\ncd");
}

#[test]
fn quoted_insert_control_char() {
    let mut e = ed("|");
    k(&mut e, "C-q C-a");
    assert_eq!(e.b().text.to_string(), "\u{1}");
}

#[test]
fn insert_date_from_init_el() {
    let mut e = ed("|");
    k(&mut e, "C-x p");
    let s = e.b().text.to_string();
    assert_eq!(s.split_whitespace().count(), 5, "{s}");
}

#[test]
fn dabbrev_expands_previous_word() {
    let mut e = ed("configuration other conf|");
    k(&mut e, "M-/");
    assert_eq!(show(&e), "configuration other configuration|");
    let mut e = ed("alpha beta al|");
    k(&mut e, "S-<tab>");
    assert_eq!(show(&e), "alpha beta alpha|");
}

#[test]
fn fill_paragraph_wraps_text() {
    let words = "word ".repeat(30);
    let mut e = ed(&format!("|{}", words.trim_end()));
    e.cfg.fill_column = 20;
    k(&mut e, "M-q");
    let s = e.b().text.to_string();
    assert!(s.lines().all(|l| l.len() <= 20), "{s}");
    assert_eq!(s.split_whitespace().count(), 30);
}

#[test]
fn rectangles() {
    let mut e = ed("|abcd\nefgh\nijkl");
    k(&mut e, "C-f C-SPC C-n C-n C-f C-f C-x r k");
    assert_eq!(e.b().text.to_string(), "ad\neh\nil");
    k(&mut e, "M-< C-x r y");
    assert_eq!(e.b().text.to_string(), "bcad\nfgeh\njkil");
    let mut e = ed("|ab\ncd");
    k(&mut e, "C-SPC C-n C-f C-x r t");
    t(&mut e, "X\n");
    assert_eq!(e.b().text.to_string(), "Xb\nXd");
}

#[test]
fn registers() {
    let mut e = ed("|hello world");
    k(&mut e, "C-SPC M-f C-x r s a C-e C-x r i a");
    assert_eq!(e.b().text.to_string(), "hello worldhello");
    k(&mut e, "M-< C-x r SPC b C-e C-x r j b");
    assert_eq!(e.pt(), 0);
}

#[test]
fn keyboard_macro_and_repeat_key() {
    let mut e = ed("|");
    k(&mut e, "C-x (");
    t(&mut e, "ab");
    k(&mut e, "C-x ) C-x e e");
    assert_eq!(e.b().text.to_string(), "ababab");
    k(&mut e, "C-u 2 C-x e");
    assert_eq!(e.b().text.to_string(), "ababababab");
}

#[test]
fn repeat_command() {
    let mut e = ed("|a b c d e");
    k(&mut e, "M-f C-x z z");
    assert_eq!(show(&e), "a b c| d e");
}

#[test]
fn narrowing() {
    let mut e = ed("aa\n|bb\ncc");
    k(&mut e, "C-SPC C-e C-x n n M-<");
    assert_eq!(e.pt(), 3);
    k(&mut e, "M->");
    assert_eq!(e.pt(), 5);
    k(&mut e, "C-x n w M->");
    assert_eq!(e.pt(), 8);
}

// ---------------- indentation and comments ----------------

#[test]
fn c_newline_and_indent_k_and_r() {
    let mut e = ed_lang("int main() {|", "c");
    t(&mut e, "\nfoo();\n}");
    assert_eq!(e.b().text.to_string(), "int main() {\n  foo();\n}");
    let mut e = ed_lang("void f() {|", "c++");
    t(&mut e, "\nx;");
    assert_eq!(e.b().text.to_string(), "void f() {\n    x;");
}

#[test]
fn go_indents_with_tabs() {
    let mut e = ed_lang("func f() {|", "go");
    t(&mut e, "\nif x {\ny()\n}");
    assert_eq!(e.b().text.to_string(), "func f() {\n\tif x {\n\t\ty()\n\t}");
}

#[test]
fn tab_reindents_line() {
    let mut e = ed_lang("int f() {\n|      x;\n}", "c");
    k(&mut e, "TAB");
    assert_eq!(show(&e), "int f() {\n  |x;\n}");
}

#[test]
fn newline_strips_trailing_whitespace_in_prog_modes() {
    let mut e = ed_lang("x = 1   |", "python");
    t(&mut e, "\n");
    assert_eq!(e.b().text.to_string(), "x = 1\n");
}

#[test]
fn python_tab_cycles_indentation() {
    let mut e = ed_lang("def f():\n    if x:\n|y", "python");
    k(&mut e, "TAB");
    assert_eq!(current_line(&e), "        y");
    k(&mut e, "TAB");
    assert_eq!(current_line(&e), "    y");
}

fn current_line(e: &Editor) -> String {
    let b = e.b();
    b.line_text(b.line_of(e.pt()))
}

#[test]
fn comment_dwim_region_toggles() {
    let mut e = ed_lang("|a = 1\nb = 2\n", "python");
    k(&mut e, "C-SPC M-> M-;");
    assert_eq!(e.b().text.to_string(), "# a = 1\n# b = 2\n");
    k(&mut e, "C-x C-x M-;");
    assert_eq!(e.b().text.to_string(), "a = 1\nb = 2\n");
}

#[test]
fn comment_dwim_adds_end_of_line_comment() {
    let mut e = ed_lang("|x := 1", "go");
    k(&mut e, "M-;");
    assert_eq!(show(&e), format!("x := 1{}// |", " ".repeat(26)));
}

#[test]
fn comment_line_moves_down() {
    let mut e = ed_lang("|a\nb", "sh");
    k(&mut e, "C-x C-;");
    assert_eq!(show(&e), "# a\n|b");
}

// ---------------- search and replace ----------------

#[test]
fn isearch_forward_backward_and_quit() {
    let mut e = ed("|one foo two foo three");
    k(&mut e, "C-s");
    t(&mut e, "foo");
    assert_eq!(e.pt(), 7);
    k(&mut e, "C-s");
    assert_eq!(e.pt(), 15);
    k(&mut e, "C-r");
    k(&mut e, "C-r");
    assert_eq!(e.pt(), 4);
    k(&mut e, "C-g");
    assert_eq!(e.pt(), 0);
    assert!(e.isearch.is_none());
}

#[test]
fn isearch_exit_sets_mark_and_runs_key() {
    let mut e = ed("|abc def");
    k(&mut e, "C-s");
    t(&mut e, "def");
    k(&mut e, "C-a");
    assert_eq!(e.pt(), 0);
    assert_eq!(e.b().mark_pos(), Some(0));
    assert_eq!(e.search_ring[0], "def");
}

#[test]
fn isearch_is_case_sensitive_like_init_el() {
    let mut e = ed("|Foo foo");
    k(&mut e, "C-s");
    t(&mut e, "foo");
    assert_eq!(e.pt(), 7);
}

#[test]
fn isearch_regexp() {
    let mut e = ed("|a1 b22 c333");
    k(&mut e, "C-M-s");
    t(&mut e, "[0-9]\\{3\\}");
    k(&mut e, "RET");
    assert_eq!(e.pt(), 11, "point after \"333\"");
}

#[test]
fn query_replace_answers() {
    let mut e = ed("|foo foo foo foo");
    k(&mut e, "M-%");
    t(&mut e, "foo\nbar\n");
    t(&mut e, "yn!");
    assert_eq!(e.b().text.to_string(), "bar foo bar bar");
    assert!(e.qr.is_none());
}

#[test]
fn replace_regexp_with_groups() {
    let mut e = ed("|k1=v1 k2=v2");
    k(&mut e, "M-x");
    t(&mut e, "replace-regexp\n");
    t(&mut e, "\\([a-z0-9]+\\)=\\([a-z0-9]+\\)\n\\2:\\1\n");
    assert_eq!(e.b().text.to_string(), "v1:k1 v2:k2");
}

#[test]
fn occur_lists_matches() {
    let mut e = ed("|a\nfoo 1\nb\nfoo 2\n");
    k(&mut e, "M-s o");
    t(&mut e, "foo\n");
    let occ = e.find_buffer("*Occur*").expect("occur buffer");
    let txt = e.buf(occ).text.to_string();
    assert!(txt.contains("2 matches"), "{txt}");
    k(&mut e, "C-x `");
    assert_eq!(current_line(&e), "foo 1");
}

// ---------------- minibuffer ----------------

#[test]
fn mx_completes_command_names() {
    let mut e = ed("|hello");
    k(&mut e, "M-x");
    t(&mut e, "upcase-wo");
    k(&mut e, "TAB");
    assert_eq!(e.mini_text(), "upcase-word");
    k(&mut e, "RET");
    assert_eq!(e.b().text.to_string(), "HELLO");
}

#[test]
fn minibuffer_history() {
    let mut e = ed("|1\n2\n3");
    k(&mut e, "M-g");
    t(&mut e, "2\n");
    k(&mut e, "M-g M-p");
    assert_eq!(e.mini_text(), "2");
    k(&mut e, "C-g");
    assert!(e.mini.is_none());
}

#[test]
fn describe_key_reports_command() {
    let mut e = ed("|");
    k(&mut e, "C-h c C-x C-f");
    assert_eq!(
        e.message.as_deref(),
        Some("C-x C-f runs the command find-file")
    );
    k(&mut e, "C-h c M-g");
    assert_eq!(e.message.as_deref(), Some("M-g runs the command goto-line"));
}

#[test]
fn undefined_key_message() {
    let mut e = ed("|");
    k(&mut e, "C-x C-y");
    assert_eq!(e.message.as_deref(), Some("C-x C-y is undefined"));
}

// ---------------- windows and buffers ----------------

#[test]
fn window_split_cycle_delete() {
    let mut e = ed("|x");
    k(&mut e, "C-x 2 C-x 3");
    assert_eq!(e.windows().len(), 3);
    let first = e.sel;
    k(&mut e, "C-z");
    assert_ne!(e.sel, first);
    k(&mut e, "C-x 0");
    assert_eq!(e.windows().len(), 2);
    k(&mut e, "C-x 1");
    assert_eq!(e.windows().len(), 1);
    k(&mut e, "C-x 0");
    assert_eq!(
        e.message.as_deref(),
        Some("Attempt to delete minibuffer or sole ordinary window")
    );
}

#[test]
fn switch_create_and_kill_buffer() {
    let mut e = ed("|x");
    k(&mut e, "C-x b");
    t(&mut e, "notes\n");
    assert_eq!(e.b().name, "notes");
    t(&mut e, "hi");
    k(&mut e, "C-x b RET");
    assert_eq!(e.b().name, "*scratch*");
    k(&mut e, "C-x b");
    t(&mut e, "notes\n");
    k(&mut e, "C-x k");
    assert!(
        e.find_buffer("notes").is_none(),
        "non-file buffers are killed without asking"
    );
}

#[test]
fn kill_modified_file_buffer_asks() {
    let d = tmpdir("killask");
    let f = d.join("a.txt");
    std::fs::write(&f, "x\n").unwrap();
    let mut e = ed("|");
    crate::files::open_path(&mut e, &f, None);
    t(&mut e, "y");
    k(&mut e, "C-x k");
    assert!(e.read_key.is_some());
    t(&mut e, "n");
    assert!(e.find_buffer("a.txt").is_some());
    k(&mut e, "C-x k");
    t(&mut e, "y");
    assert!(e.find_buffer("a.txt").is_none());
}

#[test]
fn scroll_step_scrolls_one_line() {
    let text: String = (0..100).map(|i| format!("line {i}\n")).collect();
    let mut e = ed(&format!("|{text}"));
    redisplay(&mut e);
    let h = e.text_height(e.sel);
    for _ in 0..h {
        k(&mut e, "C-n");
    }
    redisplay(&mut e);
    let (start, _) = crate::view::start_vpos(&e, e.sel);
    assert_eq!(start, 1, "scroll-step 1 scrolls by a single line");
    k(&mut e, "C-v");
    redisplay(&mut e);
    let (start2, _) = crate::view::start_vpos(&e, e.sel);
    assert_eq!(start2, start + h - 1, "next-screen-context-lines 1");
}

// ---------------- files ----------------

#[test]
fn visit_edit_save_roundtrip() {
    let d = tmpdir("save");
    let f = d.join("x.txt");
    std::fs::write(&f, "hello\n").unwrap();
    let mut e = ed("|");
    crate::files::open_path(&mut e, &f, None);
    assert_eq!(e.b().name, "x.txt");
    k(&mut e, "C-e");
    t(&mut e, " world");
    assert!(e.b().modified);
    k(&mut e, "C-x C-s");
    assert!(!e.b().modified);
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "hello world\n");
    k(&mut e, "C-x C-s");
    assert_eq!(e.message.as_deref(), Some("(No changes need to be saved)"));
}

#[test]
fn format_on_save_runs_formatter() {
    let d = tmpdir("fmt");
    let f = d.join("u.txt");
    std::fs::write(&f, "abc\n").unwrap();
    let mut cfg = test_config();
    cfg.language.push(LangConfig {
        name: "text".into(),
        formatter: Some(vec!["tr".into(), "a-z".into(), "A-Z".into()]),
        format_on_save: Some(true),
        ..Default::default()
    });
    let mut e = editor_with(cfg);
    crate::files::open_path(&mut e, &f, None);
    t(&mut e, "x");
    k(&mut e, "C-x C-s");
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "XABC\n");
    // Undo restores the pre-format text and marks the buffer modified.
    k(&mut e, "C-_");
    assert!(e.b().modified);
}

#[test]
fn failing_formatter_still_saves() {
    let d = tmpdir("fmtfail");
    let f = d.join("u.txt");
    std::fs::write(&f, "abc\n").unwrap();
    let mut cfg = test_config();
    cfg.language.push(LangConfig {
        name: "text".into(),
        formatter: Some(vec!["false".into()]),
        format_on_save: Some(true),
        ..Default::default()
    });
    let mut e = editor_with(cfg);
    crate::files::open_path(&mut e, &f, None);
    t(&mut e, "x");
    k(&mut e, "C-x C-s");
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "xabc\n");
    assert!(e.message.as_deref().unwrap_or("").contains("failed"));
}

#[test]
fn format_exclude_skips_templates() {
    let d = tmpdir("fmtexcl");
    let f = d.join("v.yaml.gotmpl");
    std::fs::write(&f, "a: 1\n").unwrap();
    let mut cfg = test_config();
    cfg.language.push(LangConfig {
        name: "yaml".into(),
        formatter: Some(vec!["false".into()]),
        ..Default::default()
    });
    let mut e = editor_with(cfg);
    crate::files::open_path(&mut e, &f, None);
    t(&mut e, "#");
    k(&mut e, "C-x C-s");
    assert!(!e.message.as_deref().unwrap_or("").contains("failed"));
}

#[test]
fn find_file_with_completion() {
    let d = tmpdir("ff");
    std::fs::write(d.join("unique_name.rs"), "fn x() {}\n").unwrap();
    let mut e = ed("|");
    k(&mut e, "C-x C-f C-a C-k");
    t(&mut e, &format!("{}/uniq", d.display()));
    k(&mut e, "TAB RET");
    assert_eq!(e.b().name, "unique_name.rs");
    assert_eq!(e.lang().name, "rust");
}

#[test]
fn save_buffers_kill_terminal_prompts() {
    let d = tmpdir("exit");
    let f = d.join("m.txt");
    std::fs::write(&f, "a\n").unwrap();
    let mut e = ed("|");
    crate::files::open_path(&mut e, &f, None);
    t(&mut e, "b");
    k(&mut e, "C-x C-c");
    assert!(e.message.as_deref().unwrap().starts_with("Save file"));
    t(&mut e, "n");
    assert!(
        e.message
            .as_deref()
            .unwrap()
            .starts_with("Modified buffers exist")
    );
    t(&mut e, "y");
    assert!(e.quit);
    let mut e = ed("|");
    k(&mut e, "C-x C-c");
    assert!(e.quit, "nothing to save: quit immediately");
}

#[test]
fn dired_opens_entries() {
    let d = tmpdir("dired");
    std::fs::write(d.join("f.txt"), "content\n").unwrap();
    let mut e = ed("|");
    crate::files::open_path(&mut e, &d, None);
    assert_eq!(e.b().kind, crate::buffer::BufKind::Dired);
    // Point starts on the first entry after "." and "..".
    k(&mut e, "RET");
    assert_eq!(e.b().name, "f.txt");
}
