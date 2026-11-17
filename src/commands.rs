// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Benjamin Zores

//! Interactive commands, named after their Emacs counterparts.

use crate::editor::{Cmd, Completion, Editor, MINI, PromptKind, ReadKey, RegOp, Register};
use crate::lang::{self, IndentStyle};
use crate::text;
use crate::view;
use std::collections::HashMap;
use std::sync::OnceLock;

macro_rules! commands {
    ($($name:literal => $f:path, $doc:literal;)*) => {
        pub const COMMANDS: &[(&str, Cmd, &str)] = &[$(($name, $f, $doc),)*];
    };
}

commands! {
    // motion
    "forward-char" => forward_char, "Move point N characters forward.";
    "backward-char" => backward_char, "Move point N characters backward.";
    "next-line" => next_line, "Move cursor vertically down N screen lines.";
    "previous-line" => previous_line, "Move cursor vertically up N screen lines.";
    "move-beginning-of-line" => beginning_of_line, "Move point to beginning of current line.";
    "move-end-of-line" => end_of_line, "Move point to end of current line.";
    "beginning-of-line" => beginning_of_line, "Move point to beginning of current line.";
    "end-of-line" => end_of_line, "Move point to end of current line.";
    "forward-word" => forward_word, "Move point forward N words.";
    "backward-word" => backward_word, "Move backward until encountering the beginning of a word.";
    "beginning-of-buffer" => beginning_of_buffer, "Move point to the beginning of the buffer, leaving mark at previous position.";
    "end-of-buffer" => end_of_buffer, "Move point to the end of the buffer, leaving mark at previous position.";
    "scroll-up-command" => scroll_up_command, "Scroll text of selected window upward a screenful.";
    "scroll-down-command" => scroll_down_command, "Scroll text of selected window down a screenful.";
    "scroll-up-line" => scroll_up_line, "Scroll text of selected window upward N lines.";
    "scroll-down-line" => scroll_down_line, "Scroll text of selected window down N lines.";
    "scroll-other-window" => scroll_other_window, "Scroll next window upward a screenful.";
    "scroll-other-window-down" => scroll_other_window_down, "Scroll next window downward a screenful.";
    "scroll-left" => scroll_left, "Scroll selected window display N columns left.";
    "scroll-right" => scroll_right, "Scroll selected window display N columns right.";
    "recenter-top-bottom" => recenter_top_bottom, "Scroll the window so that current line is in the middle, top or bottom.";
    "move-to-window-line-top-bottom" => move_to_window_line, "Position point relative to window: middle, top, bottom.";
    "back-to-indentation" => back_to_indentation, "Move point to the first non-whitespace character on this line.";
    "forward-paragraph" => forward_paragraph, "Move forward to end of paragraph.";
    "backward-paragraph" => backward_paragraph, "Move backward to start of paragraph.";
    "forward-sentence" => forward_sentence, "Move forward to next end of sentence.";
    "backward-sentence" => backward_sentence, "Move backward to start of sentence.";
    "forward-sexp" => forward_sexp, "Move forward across one balanced expression.";
    "backward-sexp" => backward_sexp, "Move backward across one balanced expression.";
    "forward-list" => forward_list, "Move forward across one balanced group of parentheses.";
    "backward-list" => backward_list, "Move backward across one balanced group of parentheses.";
    "backward-up-list" => backward_up_list, "Move backward out of one level of parentheses.";
    "down-list" => down_list, "Move forward down one level of parentheses.";
    "beginning-of-defun" => beginning_of_defun, "Move backward to the beginning of a defun.";
    "end-of-defun" => end_of_defun, "Move forward to next end of defun.";
    "goto-line" => goto_line, "Go to LINE, counting from line 1 at beginning of buffer.";
    "goto-char" => goto_char, "Set point to POSITION.";
    // editing
    "self-insert-command" => self_insert_cmd, "Insert the character you type.";
    "newline" => newline, "Insert a newline, and indent (electric-indent-mode).";
    "newline-and-indent" => newline_and_indent, "Insert a newline, then indent according to major mode.";
    "electric-newline-and-maybe-indent" => plain_newline, "Insert a newline without indenting.";
    "open-line" => open_line, "Insert a newline and leave point before it.";
    "split-line" => split_line, "Split current line, moving portion beyond point vertically down.";
    "delete-char" => delete_char, "Delete the following N characters.";
    "delete-forward-char" => delete_forward_char, "Delete the following N characters (or the active region).";
    "delete-backward-char" => delete_backward_char, "Delete the previous N characters (or the active region).";
    "kill-word" => kill_word, "Kill characters forward until encountering the end of a word.";
    "backward-kill-word" => backward_kill_word, "Kill characters backward until encountering the beginning of a word.";
    "kill-line" => kill_line, "Kill the rest of the current line; if no nonblanks there, kill thru newline.";
    "kill-whole-line" => kill_whole_line, "Kill current line.";
    "kill-region" => kill_region, "Kill (\"cut\") text between point and mark.";
    "kill-ring-save" => kill_ring_save, "Save the region as if killed, but don't kill it.";
    "yank" => yank, "Reinsert (\"paste\") the last stretch of killed text.";
    "yank-pop" => yank_pop, "Replace just-yanked stretch of killed text with a different stretch.";
    "set-mark-command" => set_mark_command, "Set the mark where point is, or jump to the mark.";
    "exchange-point-and-mark" => exchange_point_and_mark, "Put the mark where point is now, and point where the mark is now.";
    "mark-whole-buffer" => mark_whole_buffer, "Put point at beginning and mark at end of buffer.";
    "mark-word" => mark_word, "Set mark ARG words away from point.";
    "mark-paragraph" => mark_paragraph, "Put point at beginning of this paragraph, mark at end.";
    "mark-sexp" => mark_sexp, "Set mark ARG sexps from point.";
    "mark-defun" => mark_defun, "Put mark at end of this defun, point at beginning.";
    "mark-page" => mark_whole_buffer, "Put mark at end of page, point at beginning.";
    "undo" => undo, "Undo some previous changes.";
    "undo-redo" => undo_redo, "Undo the last ARG undos.";
    "transpose-chars" => transpose_chars, "Interchange characters around point, moving forward one character.";
    "transpose-words" => transpose_words, "Interchange words around point, leaving point at end of them.";
    "transpose-lines" => transpose_lines, "Exchange current line and previous line, leaving point after both.";
    "transpose-sexps" => transpose_words, "Like transpose-words but applies to sexps.";
    "upcase-word" => upcase_word, "Convert to upper case from point to end of word, moving over.";
    "downcase-word" => downcase_word, "Convert to lower case from point to end of word, moving over.";
    "capitalize-word" => capitalize_word, "Capitalize from point to the end of word, moving over.";
    "upcase-region" => upcase_region, "Convert the region to upper case.";
    "downcase-region" => downcase_region, "Convert the region to lower case.";
    "capitalize-region" => capitalize_region, "Convert the region to capitalized form.";
    "delete-horizontal-space" => delete_horizontal_space, "Delete all spaces and tabs around point.";
    "just-one-space" => just_one_space, "Delete all spaces and tabs around point, leaving one space.";
    "cycle-spacing" => just_one_space, "Manipulate whitespace around point.";
    "delete-indentation" => delete_indentation, "Join this line to previous and fix up whitespace at join.";
    "join-line" => delete_indentation, "Join this line to previous and fix up whitespace at join.";
    "delete-blank-lines" => delete_blank_lines, "On blank line, delete all surrounding blank lines, leaving just one.";
    "zap-to-char" => zap_to_char_cmd, "Kill up to and including ARGth occurrence of CHAR.";
    "indent-for-tab-command" => indent_for_tab, "Indent the current line or region, or insert a tab, as appropriate.";
    "indent-region" => indent_region, "Indent each nonblank line in the region.";
    "indent-rigidly" => indent_rigidly, "Indent all lines starting in the region.";
    "indent-relative" => indent_relative_cmd, "Space out to under next indent point in previous nonblank line.";
    "tab-to-tab-stop" => tab_to_tab_stop, "Insert spaces or tabs to next defined tab-stop column.";
    "comment-dwim" => comment_dwim, "Call the comment command you want (Do What I Mean).";
    "comment-line" => comment_line, "Comment or uncomment current line and leave point after it.";
    "comment-region" => comment_region, "Comment or uncomment each line in the region.";
    "uncomment-region" => uncomment_region, "Uncomment each line in the region.";
    "fill-paragraph" => fill_paragraph, "Fill paragraph at or after point.";
    "set-fill-column" => set_fill_column, "Set fill-column to specified argument (or current column).";
    "quoted-insert" => quoted_insert, "Read next input character and insert it.";
    "overwrite-mode" => overwrite_mode, "Toggle Overwrite mode.";
    "dabbrev-expand" => dabbrev_expand, "Expand previous word \"dynamically\".";
    "dabbrev-completion" => dabbrev_expand, "Completion on current word.";
    "count-words-region" => count_words, "Count the number of words in the region or buffer.";
    "count-words" => count_words, "Count the number of words in the region or buffer.";
    "what-cursor-position" => what_cursor_position, "Print info on cursor position.";
    "what-line" => what_line, "Print the current buffer line number.";
    "count-lines-page" => count_lines_page, "Report number of lines on current page.";
    "kill-sexp" => kill_sexp, "Kill the sexp following point.";
    "backward-kill-sexp" => backward_kill_sexp, "Kill the sexp preceding point.";
    "kill-sentence" => kill_sentence, "Kill from point to end of sentence.";
    "insert-current-time-string" => insert_time, "Insert (current-time-string) at point (init.el my-print-date).";
    "my-print-date" => insert_time, "Insert (current-time-string) at point.";
    "delete-trailing-whitespace" => delete_trailing_whitespace, "Delete trailing whitespace between START and END.";
    "whitespace-cleanup" => delete_trailing_whitespace, "Cleanup trailing whitespace.";
    "untabify" => untabify, "Convert all tabs in region to multiple spaces, preserving columns.";
    "tabify" => tabify, "Convert multiple spaces in region to tabs when possible.";
    "sort-lines" => sort_lines, "Sort lines in region alphabetically; ARG means descending order.";
    "reverse-region" => reverse_region, "Reverse the order of lines in a region.";
    "delete-duplicate-lines" => delete_duplicate_lines, "Delete all but one copy of any identical lines in the region.";
    "flush-lines" => flush_lines, "Delete lines containing matches for REGEXP.";
    "keep-lines" => keep_lines, "Delete all lines except those containing matches for REGEXP.";
    "repeat" => repeat, "Repeat most recently executed command.";
    "upcase-initials-region" => capitalize_region, "Upcase the initial of each word in the region.";
    // rectangles / registers / bookmarks
    "kill-rectangle" => kill_rectangle, "Delete the region-rectangle and save it as the last killed one.";
    "delete-rectangle" => delete_rectangle, "Delete (don't save) text in the region-rectangle.";
    "yank-rectangle" => yank_rectangle, "Yank the last killed rectangle with upper left corner at point.";
    "open-rectangle" => open_rectangle, "Blank out the region-rectangle, shifting text right.";
    "clear-rectangle" => clear_rectangle, "Blank out the region-rectangle.";
    "string-rectangle" => string_rectangle, "Replace rectangle contents with STRING on each line.";
    "copy-rectangle-as-kill" => copy_rectangle_as_kill, "Copy the region-rectangle and save it as the last killed one.";
    "rectangle-number-lines" => rectangle_number_lines, "Insert numbers in front of the region-rectangle.";
    "copy-to-register" => copy_to_register, "Copy region into register REGISTER.";
    "insert-register" => insert_register, "Insert contents of register REGISTER.";
    "point-to-register" => point_to_register, "Store current location of point in REGISTER.";
    "jump-to-register" => jump_to_register, "Jump to register REGISTER.";
    "bookmark-set" => crate::files::bookmark_set, "Set a bookmark named NAME at the current location.";
    "bookmark-jump" => crate::files::bookmark_jump, "Jump to bookmark BOOKMARK.";
    "bookmark-bmenu-list" => crate::files::bookmark_list, "Display a list of existing bookmarks.";
    // search
    "isearch-forward" => crate::search::isearch_forward, "Do incremental search forward.";
    "isearch-backward" => crate::search::isearch_backward, "Do incremental search backward.";
    "isearch-forward-regexp" => crate::search::isearch_forward_regexp, "Do incremental search forward for regular expression.";
    "isearch-backward-regexp" => crate::search::isearch_backward_regexp, "Do incremental search backward for regular expression.";
    "isearch-forward-symbol-at-point" => crate::search::isearch_symbol_at_point, "Do incremental search forward for a symbol found near point.";
    "isearch-forward-word" => crate::search::isearch_forward, "Do incremental search forward for a sequence of words.";
    "isearch-forward-symbol" => crate::search::isearch_forward_regexp, "Do incremental search forward for a symbol.";
    "query-replace" => crate::search::query_replace, "Replace some occurrences of FROM-STRING with TO-STRING.";
    "query-replace-regexp" => crate::search::query_replace_regexp, "Replace some things after point matching REGEXP with TO-STRING.";
    "replace-string" => crate::search::replace_string, "Replace occurrences of FROM-STRING with TO-STRING.";
    "replace-regexp" => crate::search::replace_regexp, "Replace things after point matching REGEXP with TO-STRING.";
    "occur" => crate::search::occur, "Show all lines in the current buffer containing a match for REGEXP.";
    "highlight-regexp" => crate::search::highlight_regexp, "Set face of each match of REGEXP.";
    "unhighlight-regexp" => crate::search::unhighlight_regexp, "Remove highlighting of each match to REGEXP.";
    "next-error" => crate::search::next_error, "Visit next occur/compilation hit.";
    "previous-error" => crate::search::previous_error, "Visit previous occur/compilation hit.";
    // files & buffers
    "find-file" => crate::files::find_file, "Edit file FILENAME.";
    "find-file-read-only" => crate::files::find_file_read_only, "Edit file FILENAME but don't allow changes.";
    "find-alternate-file" => crate::files::find_alternate_file, "Find file FILENAME, select its buffer, kill previous buffer.";
    "find-file-other-window" => crate::files::find_file_other_window, "Edit file FILENAME, in another window.";
    "save-buffer" => crate::files::save_buffer, "Save current buffer in visited file if modified.";
    "write-file" => crate::files::write_file, "Write current buffer into file FILENAME.";
    "save-some-buffers" => crate::files::save_some_buffers, "Save some modified file-visiting buffers.";
    "insert-file" => crate::files::insert_file, "Insert contents of file FILENAME into buffer after point.";
    "save-buffers-kill-terminal" => crate::files::save_buffers_kill_terminal, "Offer to save each buffer, then kill the current connection.";
    "save-buffers-kill-emacs" => crate::files::save_buffers_kill_terminal, "Offer to save each buffer, then kill this Emacs process.";
    "kill-emacs" => crate::files::kill_emacs, "Exit immediately without saving.";
    "read-only-mode" => crate::files::read_only_mode, "Change whether the current buffer is read-only.";
    "toggle-read-only" => crate::files::read_only_mode, "Change whether the current buffer is read-only.";
    "revert-buffer" => crate::files::revert_buffer, "Replace current buffer text with the text of the visited file on disk.";
    "format-buffer" => crate::files::format_buffer, "Format the current buffer with the configured formatter.";
    "format-all-buffer" => crate::files::format_buffer, "Format the current buffer with the configured formatter.";
    "toggle-format-on-save" => crate::files::toggle_format_on_save, "Toggle formatting on save for the current major mode.";
    "dired" => crate::files::dired, "Edit directory DIRNAME.";
    "list-directory" => crate::files::dired, "Display a list of files in DIRNAME.";
    "dired-other-window" => crate::files::dired_other_window, "Edit directory DIRNAME in another window.";
    "suspend-frame" => crate::files::suspend_frame, "Stop Emacs and return to superior process.";
    "switch-to-buffer" => crate::files::switch_to_buffer, "Display buffer BUFFER-OR-NAME in the selected window.";
    "switch-to-buffer-other-window" => crate::files::switch_to_buffer_other_window, "Select the buffer specified by BUFFER-OR-NAME in another window.";
    "list-buffers" => crate::files::list_buffers, "Display a list of existing buffers.";
    "buffer-menu" => crate::files::buffer_menu, "Switch to the Buffer Menu.";
    "kill-buffer" => crate::files::kill_buffer_cmd, "Kill the buffer specified by BUFFER-OR-NAME.";
    "kill-current-buffer" => crate::files::kill_current_buffer, "Kill the current buffer (init.el my-kill-buffer).";
    "my-kill-buffer" => crate::files::kill_current_buffer, "Kill the current buffer without confirmation.";
    "next-buffer" => crate::files::next_buffer, "In selected window switch to ARGth next buffer.";
    "previous-buffer" => crate::files::previous_buffer, "In selected window switch to ARGth previous buffer.";
    "rename-buffer" => crate::files::rename_buffer, "Change current buffer's name to NEWNAME.";
    "Buffer-menu-this-window" => crate::files::bm_this_window, "Select this line's buffer in this window.";
    "Buffer-menu-other-window" => crate::files::bm_other_window, "Select this line's buffer in other window, leaving buffer menu visible.";
    "Buffer-menu-switch-other-window" => crate::files::bm_switch_other_window, "Make the other window select this line's buffer.";
    "Buffer-menu-1-window" => crate::files::bm_one_window, "Select this line's buffer, alone, in full frame.";
    "Buffer-menu-delete" => crate::files::bm_delete, "Mark the buffer on this line to be deleted by x command.";
    "Buffer-menu-delete-backwards" => crate::files::bm_delete, "Mark buffer on this line to be deleted by x command.";
    "Buffer-menu-save" => crate::files::bm_save, "Mark the buffer on this line to be saved by x command.";
    "Buffer-menu-unmark" => crate::files::bm_unmark, "Cancel all requested operations on buffer on this line.";
    "Buffer-menu-backup-unmark" => crate::files::bm_backup_unmark, "Move up and cancel all requested operations on buffer on line above.";
    "Buffer-menu-execute" => crate::files::bm_execute, "Save and/or delete marked buffers in the Buffer Menu.";
    "Buffer-menu-not-modified" => crate::files::bm_not_modified, "Mark the buffer on this line as unmodified (no changes to save).";
    "Buffer-menu-toggle-read-only" => crate::files::bm_toggle_ro, "Toggle read-only status of buffer on this line.";
    "dired-find-file" => crate::files::dired_find_file, "In Dired, visit the file or directory named on this line.";
    "dired-find-file-other-window" => crate::files::dired_find_file_other_window, "In Dired, visit this file or directory in another window.";
    "dired-view-file" => crate::files::dired_view_file, "In Dired, examine a file in view mode.";
    "dired-up-directory" => crate::files::dired_up_directory, "Run Dired on parent directory of current directory.";
    "dired-next-line" => crate::files::dired_next_line, "Move down lines then position at filename.";
    "dired-previous-line" => crate::files::dired_previous_line, "Move up lines then position at filename.";
    "dired-flag-file-deletion" => crate::files::dired_flag, "In Dired, flag the current line's file for deletion.";
    "dired-unmark" => crate::files::dired_unmark, "Unmark the file at point in the Dired buffer.";
    "dired-do-flagged-delete" => crate::files::dired_do_delete, "In Dired, delete the files flagged for deletion.";
    "dired-create-directory" => crate::files::dired_create_directory, "Create a directory called DIRECTORY.";
    "dired-do-rename" => crate::files::dired_rename, "Rename current file.";
    "dired-do-copy" => crate::files::dired_copy, "Copy current file.";
    "quit-window" => crate::files::quit_window, "Quit the current buffer.";
    "special-mode-follow" => crate::search::occur_follow, "Visit the location on the current line.";
    "occur-mode-goto-occurrence" => crate::search::occur_follow, "Go to the occurrence on the current line.";
    "shell-command" => crate::files::shell_command, "Execute string COMMAND in inferior shell; display output, if any.";
    "async-shell-command" => crate::files::shell_command, "Execute string COMMAND in inferior shell.";
    "shell-command-on-region" => crate::files::shell_command_on_region, "Execute string COMMAND in inferior shell with region as input.";
    // windows
    "split-window-below" => split_window_below, "Split the selected window into two windows, one above the other.";
    "split-window-vertically" => split_window_below, "Split the selected window into two windows, one above the other.";
    "split-window-right" => split_window_right, "Split the selected window into two side-by-side windows.";
    "split-window-horizontally" => split_window_right, "Split the selected window into two side-by-side windows.";
    "delete-window" => delete_window, "Delete the selected window.";
    "delete-other-windows" => delete_other_windows, "Make the selected window fill its frame.";
    "other-window" => other_window, "Select another window in cyclic ordering of windows.";
    "other-window-backward" => other_window_backward, "Select the previous window (init.el C-S-<tab>).";
    "enlarge-window" => enlarge_window, "Make the selected window SIZE lines taller.";
    "shrink-window" => shrink_window, "Make the selected window SIZE lines smaller.";
    "enlarge-window-horizontally" => enlarge_window_h, "Make selected window DELTA columns wider.";
    "shrink-window-horizontally" => shrink_window_h, "Make selected window DELTA columns narrower.";
    "shrink-window-if-larger-than-buffer" => noop, "Shrink height of WINDOW if its buffer doesn't need so many lines.";
    "balance-windows" => balance_windows, "Balance the sizes of windows.";
    "kill-buffer-and-window" => kill_buffer_and_window, "Kill the current buffer and delete the selected window.";
    // misc
    "keyboard-quit" => keyboard_quit, "Signal a quit condition.";
    "keyboard-escape-quit" => keyboard_escape_quit, "Exit the current \"mode\" (in a generalized sense of the word).";
    "execute-extended-command" => execute_extended_command, "Read a command name, then read the arguments and call the command.";
    "universal-argument" => universal_argument, "Begin a numeric argument for the following command.";
    "negative-argument" => negative_argument, "Begin a negative numeric argument for the next command.";
    "kmacro-start-macro" => kmacro_start, "Record subsequent keyboard input, defining a keyboard macro.";
    "kmacro-end-macro" => kmacro_end, "Finish defining a keyboard macro.";
    "kmacro-end-and-call-macro" => kmacro_end_and_call, "Call last keyboard macro, ending it first if currently being defined.";
    "kmacro-start-macro-or-insert-counter" => kmacro_start_or_counter, "Record subsequent keyboard input, or insert macro counter.";
    "kmacro-end-or-call-macro" => kmacro_end_or_call, "End kbd macro if currently being defined; else call last kbd macro.";
    "call-last-kbd-macro" => kmacro_end_and_call, "Call the last keyboard macro that you defined.";
    "narrow-to-region" => narrow_to_region, "Restrict editing in this buffer to the current region.";
    "widen" => widen, "Remove restrictions (narrowing) from current buffer.";
    "toggle-truncate-lines" => toggle_truncate_lines, "Toggle truncating of long lines.";
    "describe-key" => describe_key, "Display documentation of the function invoked by KEY-LIST.";
    "describe-key-briefly" => describe_key_briefly, "Print the name of the function KEY invokes.";
    "describe-bindings" => crate::files::describe_bindings, "Display a buffer showing a list of all defined keys.";
    "describe-function" => describe_function, "Display the full documentation of FUNCTION.";
    "describe-mode" => crate::files::describe_mode, "Display documentation of current major mode.";
    "where-is" => where_is, "Print message listing key sequences that invoke the command DEFINITION.";
    "view-echo-area-messages" => crate::files::view_messages, "View the log of recent echo-area messages.";
    "help-for-help" => help_for_help, "Help command.";
    "text-scale-adjust" => noop, "Adjust the font size (no-op in a terminal).";
    "menu-bar-open" => menu_bar_open, "Open the menu bar (not available).";
    "flyspell-auto-correct-word" => flyspell_auto_correct, "Correct the current word (spell checking not available yet).";
    "redraw-display" => redraw_display, "Clear and redisplay all visible frames.";
    "set-variable" => set_variable, "Set VARIABLE to VALUE (limited to a few settings).";
    "abbrev-prefix-mark" => noop, "Mark current point as the beginning of an abbrev.";
    "eval-expression" => eval_unsupported, "Evaluate EXP (Lisp is not available).";
    "eval-last-sexp" => eval_unsupported, "Evaluate sexp before point (Lisp is not available).";
    // minibuffer
    "exit-minibuffer" => crate::files::exit_minibuffer, "Terminate this minibuffer argument.";
    "minibuffer-complete" => crate::files::minibuffer_complete, "Complete the minibuffer contents as far as possible.";
    "minibuffer-completion-help" => crate::files::minibuffer_completion_help, "Display a list of possible completions of the current minibuffer contents.";
    "abort-recursive-edit" => abort_minibuffer, "Abort the command that requested this recursive edit or minibuffer input.";
    "previous-history-element" => crate::files::previous_history, "Put previous minibuffer history element in the minibuffer.";
    "next-history-element" => crate::files::next_history, "Put next minibuffer history element in the minibuffer.";
    "switch-to-completions" => noop, "Select the completion list window.";
}

fn table() -> &'static HashMap<&'static str, (Cmd, &'static str)> {
    static T: OnceLock<HashMap<&'static str, (Cmd, &'static str)>> = OnceLock::new();
    T.get_or_init(|| COMMANDS.iter().map(|(n, f, d)| (*n, (*f, *d))).collect())
}

pub fn lookup(name: &str) -> Option<Cmd> {
    table().get(name).map(|(f, _)| *f)
}

pub fn doc(name: &str) -> &'static str {
    table()
        .get(name)
        .map(|(_, d)| *d)
        .unwrap_or("Not documented.")
}

pub fn names() -> Vec<String> {
    let mut v: Vec<String> = COMMANDS.iter().map(|(n, _, _)| n.to_string()).collect();
    v.sort();
    v.dedup();
    v
}

// ======================= helpers =======================

fn noop(_: &mut Editor) {}

pub fn region(ed: &mut Editor) -> Option<(usize, usize)> {
    let b = ed.b();
    match b.mark_pos() {
        Some(m) => {
            let p = ed.pt();
            Some((m.min(p), m.max(p)))
        }
        None => {
            ed.error("The mark is not set now, so there is no region");
            None
        }
    }
}

fn active_region(ed: &Editor) -> Option<(usize, usize)> {
    let p = ed.pt();
    ed.b().region(p)
}

fn signed_count(ed: &Editor) -> i64 {
    ed.count()
}

// ======================= motion =======================

fn forward_char(ed: &mut Editor) {
    move_chars(ed, signed_count(ed));
}

fn backward_char(ed: &mut Editor) {
    move_chars(ed, -signed_count(ed));
}

fn move_chars(ed: &mut Editor, n: i64) {
    let p = ed.pt() as i64 + n;
    let (bv, zv) = (ed.b().begv() as i64, ed.b().zv() as i64);
    if p < bv {
        ed.goto(bv as usize);
        ed.error("Beginning of buffer");
    } else if p > zv {
        ed.goto(zv as usize);
        ed.error("End of buffer");
    } else {
        ed.goto(p as usize);
    }
}

fn next_line(ed: &mut Editor) {
    vertical(ed, signed_count(ed));
}

fn previous_line(ed: &mut Editor) {
    vertical(ed, -signed_count(ed));
}

pub fn vertical(ed: &mut Editor, n: i64) {
    let wid = ed.sel;
    if wid == MINI {
        // Minibuffer: history navigation.
        if n > 0 {
            crate::files::next_history(ed)
        } else {
            crate::files::previous_history(ed)
        }
        return;
    }
    let g = view::geo(ed, wid);
    let pt = ed.pt();
    let b = ed.b();
    let goal = ed.w().goal.unwrap_or_else(|| view::x_of(b, pt, g));
    let pv = view::vpos(b, pt, g);
    let (target, moved) = view::move_rows(b, pv, n as isize, g);
    let p = view::pos_at(b, target, goal, g);
    if moved != n as isize {
        if n > 0 {
            let z = ed.b().zv();
            ed.goto(z);
            ed.error("End of buffer");
        } else {
            let a = ed.b().begv();
            ed.goto(a);
            ed.error("Beginning of buffer");
        }
    } else {
        ed.goto(p);
    }
    ed.wm().goal = Some(goal);
}

fn beginning_of_line(ed: &mut Editor) {
    let n = ed.count();
    let b = ed.b();
    let line = (b.line_of(ed.pt()) as i64 + n - 1).max(0) as usize;
    let p = b.line_start(line).max(b.begv());
    ed.goto(p);
}

fn end_of_line(ed: &mut Editor) {
    let n = ed.count();
    let b = ed.b();
    let line = (b.line_of(ed.pt()) as i64 + n - 1).max(0) as usize;
    let p = b.line_end(line.min(b.line_count() - 1)).min(b.zv());
    ed.goto(p);
}

fn forward_word(ed: &mut Editor) {
    let n = signed_count(ed);
    let mut p = ed.pt();
    for _ in 0..n.abs() {
        p = if n > 0 {
            text::forward_word(ed.b(), p)
        } else {
            text::backward_word(ed.b(), p)
        };
    }
    ed.goto(p);
}

fn backward_word(ed: &mut Editor) {
    ed.arg = Some(-ed.count());
    forward_word(ed);
}

fn beginning_of_buffer(ed: &mut Editor) {
    let p = ed.pt();
    if !ed.b().mark_active {
        ed.bm().push_mark(p, false);
    }
    let b = ed.b();
    let target = match ed.arg {
        Some(n) if ed.arg_universal || ed.arg_digits => {
            b.begv() + (b.zv() - b.begv()) * (n.clamp(0, 10) as usize) / 10
        }
        _ => b.begv(),
    };
    ed.goto(target);
    if ed.message.is_none() && !ed.executing_macro {
        ed.msg("Mark set");
    }
}

fn end_of_buffer(ed: &mut Editor) {
    let p = ed.pt();
    if !ed.b().mark_active {
        ed.bm().push_mark(p, false);
    }
    let b = ed.b();
    let target = match ed.arg {
        Some(n) if ed.arg_universal || ed.arg_digits => {
            let p = b.zv() - (b.zv() - b.begv()) * (n.clamp(0, 10) as usize) / 10;
            b.bol(p)
        }
        _ => b.zv(),
    };
    ed.goto(target);
    if ed.message.is_none() && !ed.executing_macro {
        ed.msg("Mark set");
    }
}

fn scroll_amount(ed: &Editor, wid: usize) -> isize {
    let h = ed.text_height(wid) as isize;
    match ed.arg {
        Some(n) if ed.arg_digits || ed.arg_universal => n as isize,
        Some(-1) => -(h - ed.cfg.next_screen_context_lines as isize).max(1),
        _ => (h - ed.cfg.next_screen_context_lines as isize).max(1),
    }
}

fn scroll_window(ed: &mut Editor, wid: usize, n: isize) {
    let g = view::geo(ed, wid);
    let preserve = ed.cfg.scroll_preserve_screen_position;
    let row = view::point_screen_row(ed, wid);
    let pt = ed.win_pt(wid);
    let b = ed.buf(ed.win(wid).buf);
    let x = ed.win(wid).goal.unwrap_or_else(|| view::x_of(b, pt, g));
    let start = view::start_vpos(ed, wid);
    let b = ed.buf(ed.win(wid).buf);
    let (ns, moved) = view::move_rows(b, start, n, g);
    if moved == 0 {
        if n > 0 {
            // At the end: move point to end like scroll-error-top-bottom would.
            let z = b.zv();
            if ed.win_pt(wid) == z {
                ed.error("End of buffer");
            } else {
                view::set_win_point(ed, wid, z);
                ed.error("End of buffer");
            }
        } else {
            let a = b.begv();
            if ed.win_pt(wid) == a {
                ed.error("Beginning of buffer");
            } else {
                view::set_win_point(ed, wid, a);
                ed.error("Beginning of buffer");
            }
        }
        return;
    }
    view::set_start(ed, wid, ns);
    let h = ed.text_height(wid);
    let b = ed.buf(ed.win(wid).buf);
    let target_row = if preserve {
        row.unwrap_or(0).min(h - 1)
    } else if n > 0 {
        0
    } else {
        h - 1
    };
    let (t, _) = view::move_rows(b, ns, target_row as isize, g);
    let p = view::pos_at(b, t, if preserve { x } else { 0 }, g);
    view::set_win_point(ed, wid, p);
    if preserve {
        ed.winm(wid).goal = Some(x);
    }
}

fn scroll_up_command(ed: &mut Editor) {
    let wid = ed.sel;
    let n = scroll_amount(ed, wid);
    scroll_window(ed, wid, n);
}

fn scroll_down_command(ed: &mut Editor) {
    let wid = ed.sel;
    let n = scroll_amount(ed, wid);
    scroll_window(ed, wid, -n);
}

fn scroll_up_line(ed: &mut Editor) {
    let wid = ed.sel;
    let n = ed.count() as isize;
    view::scroll_rows(ed, wid, n);
}

fn scroll_down_line(ed: &mut Editor) {
    let wid = ed.sel;
    let n = ed.count() as isize;
    view::scroll_rows(ed, wid, -n);
}

fn scroll_other_window(ed: &mut Editor) {
    let wins = ed.windows();
    if wins.len() < 2 {
        ed.error("There is no other window");
        return;
    }
    let i = wins.iter().position(|&w| w == ed.sel).unwrap_or(0);
    let other = wins[(i + 1) % wins.len()];
    let n = scroll_amount(ed, other);
    scroll_window(ed, other, n);
}

fn scroll_other_window_down(ed: &mut Editor) {
    let wins = ed.windows();
    if wins.len() < 2 {
        ed.error("There is no other window");
        return;
    }
    let i = wins.iter().position(|&w| w == ed.sel).unwrap_or(0);
    let other = wins[(i + 1) % wins.len()];
    let n = scroll_amount(ed, other);
    scroll_window(ed, other, -n);
}

fn scroll_left(ed: &mut Editor) {
    let n = if ed.arg.is_some() {
        ed.count() as usize
    } else {
        ed.text_width(ed.sel).saturating_sub(2)
    };
    let w = ed.wm();
    w.hscroll += n;
}

fn scroll_right(ed: &mut Editor) {
    let n = if ed.arg.is_some() {
        ed.count() as usize
    } else {
        ed.text_width(ed.sel).saturating_sub(2)
    };
    let w = ed.wm();
    w.hscroll = w.hscroll.saturating_sub(n);
}

fn recenter_top_bottom(ed: &mut Editor) {
    let wid = ed.sel;
    let h = ed.text_height(wid);
    if let Some(n) = ed.arg {
        let row = if n >= 0 {
            n as usize
        } else {
            h.saturating_sub((-n) as usize)
        };
        view::recenter(ed, wid, row.min(h - 1));
        return;
    }
    let cycle = if ed.last_cmd == "recenter-top-bottom" {
        (ed.recenter_cycle + 1) % 3
    } else {
        0
    };
    ed.recenter_cycle = cycle;
    let row = match cycle {
        0 => h / 2,
        1 => 0,
        _ => h - 1,
    };
    view::recenter(ed, wid, row);
    ed.redraw_all = cycle == 0;
}

fn move_to_window_line(ed: &mut Editor) {
    let wid = ed.sel;
    let h = ed.text_height(wid);
    let cycle = if ed.last_cmd == "move-to-window-line-top-bottom" {
        (ed.recenter_cycle + 1) % 3
    } else {
        0
    };
    let row = match cycle {
        0 => (h - 1) / 2,
        1 => 0,
        _ => h - 1,
    };
    let g = view::geo(ed, wid);
    let start = view::start_vpos(ed, wid);
    let b = ed.b();
    let (t, _) = view::move_rows(b, start, row as isize, g);
    let p = view::pos_at(b, t, 0, g);
    ed.goto(p);
    ed.keep_arg = false;
    ed.recenter_cycle = cycle;
}

fn indentation_end(ed: &Editor, pos: usize) -> usize {
    let b = ed.b();
    let mut p = b.bol(pos);
    let e = b.eol(pos);
    while p < e && matches!(b.text.char(p), ' ' | '\t') {
        p += 1;
    }
    p
}

fn back_to_indentation(ed: &mut Editor) {
    let p = indentation_end(ed, ed.pt());
    ed.goto(p);
}

fn forward_paragraph(ed: &mut Editor) {
    let n = ed.count();
    let mut p = ed.pt();
    for _ in 0..n.abs() {
        p = if n > 0 {
            text::forward_paragraph(ed.b(), p)
        } else {
            text::backward_paragraph(ed.b(), p)
        };
    }
    ed.goto(p);
}

fn backward_paragraph(ed: &mut Editor) {
    ed.arg = Some(-ed.count());
    forward_paragraph(ed);
}

fn forward_sentence(ed: &mut Editor) {
    let mut p = ed.pt();
    for _ in 0..ed.count().max(1) {
        p = text::forward_sentence(ed.b(), p);
    }
    ed.goto(p);
}

fn backward_sentence(ed: &mut Editor) {
    let mut p = ed.pt();
    for _ in 0..ed.count().max(1) {
        p = text::backward_sentence(ed.b(), p);
    }
    ed.goto(p);
}

fn forward_sexp(ed: &mut Editor) {
    let n = ed.count();
    let mut p = ed.pt();
    for _ in 0..n.abs() {
        let r = if n > 0 {
            text::forward_sexp(ed.b(), p)
        } else {
            text::backward_sexp(ed.b(), p)
        };
        match r {
            Some(q) => p = q,
            None => {
                ed.goto(p);
                ed.error("Scan error: \"Containing expression ends prematurely\"");
                return;
            }
        }
    }
    ed.goto(p);
}

fn backward_sexp(ed: &mut Editor) {
    ed.arg = Some(-ed.count());
    forward_sexp(ed);
}

fn forward_list(ed: &mut Editor) {
    let b = ed.b();
    let mut p = ed.pt();
    while p < b.zv() && text::close_of(b.text.char(p)).is_none() {
        p += 1;
    }
    match text::match_paren(b, p, 1_000_000) {
        Some(q) => ed.goto(q + 1),
        None => ed.error("No next group"),
    }
}

fn backward_list(ed: &mut Editor) {
    let b = ed.b();
    let mut p = ed.pt();
    while p > b.begv() && text::open_of(b.text.char(p - 1)).is_none() {
        p -= 1;
    }
    if p == 0 {
        ed.error("No previous group");
        return;
    }
    match text::match_paren(b, p - 1, 1_000_000) {
        Some(q) => ed.goto(q),
        None => ed.error("No previous group"),
    }
}

fn backward_up_list(ed: &mut Editor) {
    let b = ed.b();
    let mut depth = 0;
    let mut p = ed.pt();
    while p > b.begv() {
        p -= 1;
        let c = b.text.char(p);
        if text::open_of(c).is_some() {
            depth += 1;
        } else if text::close_of(c).is_some() {
            if depth == 0 {
                ed.goto(p);
                return;
            }
            depth -= 1;
        }
    }
    ed.error("Scan error: \"Unbalanced parentheses\"");
}

fn down_list(ed: &mut Editor) {
    let b = ed.b();
    let mut p = ed.pt();
    while p < b.zv() {
        let c = b.text.char(p);
        if text::close_of(c).is_some() {
            ed.goto(p + 1);
            return;
        }
        if text::open_of(c).is_some() {
            break;
        }
        p += 1;
    }
    ed.error("Scan error: \"Containing expression ends prematurely\"");
}

fn is_defun_start(line: &str) -> bool {
    let Some(c) = line.chars().next() else {
        return false;
    };
    !(c.is_whitespace() || matches!(c, '}' | ')' | ']' | '#' | '/' | '*' | ';' | '-'))
        || line.starts_with("(def")
        || line.starts_with("(use-package")
}

fn beginning_of_defun(ed: &mut Editor) {
    let b = ed.b();
    let mut l = b.line_of(ed.pt());
    if b.line_start(l) == ed.pt() && l > 0 {
        l -= 1;
    }
    loop {
        if is_defun_start(&b.line_text(l)) {
            let p = b.line_start(l);
            ed.goto(p);
            return;
        }
        if l == 0 {
            ed.goto(0);
            return;
        }
        l -= 1;
    }
}

fn end_of_defun(ed: &mut Editor) {
    let b = ed.b();
    let last = b.line_count() - 1;
    let mut l = b.line_of(ed.pt());
    // Move to the next defun start, then to the line before it ends.
    let mut found_close = None;
    let mut ll = l;
    while ll <= last {
        let t = b.line_text(ll);
        if t.starts_with('}') || t.starts_with(')') {
            found_close = Some(ll);
            break;
        }
        if ll > l && is_defun_start(&t) {
            break;
        }
        ll += 1;
    }
    if let Some(cl) = found_close {
        l = cl + 1;
    } else {
        l = ll;
    }
    let p = b.line_start(l.min(last + 1));
    ed.goto(p);
}

fn goto_line(ed: &mut Editor) {
    if let Some(n) = ed.arg.filter(|_| ed.arg_digits || !ed.arg_universal) {
        goto_line_n(ed, n);
        return;
    }
    let def = {
        let b = ed.b();
        let p = ed.pt();
        let mut s = p;
        while s > 0 && b.text.char(s - 1).is_ascii_digit() {
            s -= 1;
        }
        let mut e = p;
        while e < b.len() && b.text.char(e).is_ascii_digit() {
            e += 1;
        }
        if e > s { Some(b.slice(s, e)) } else { None }
    };
    let prompt = match &def {
        Some(d) => format!("Goto line (default {d}): "),
        None => "Goto line: ".to_string(),
    };
    ed.read_string_default(
        &prompt,
        PromptKind::GotoLine,
        "",
        Completion::None,
        "goto-line",
        def,
    );
}

pub fn goto_line_n(ed: &mut Editor, n: i64) {
    let p = ed.pt();
    if !ed.b().mark_active {
        ed.bm().push_mark(p, false);
    }
    let b = ed.b();
    let line = (n.max(1) as usize - 1).min(b.line_count() - 1);
    let target = b.line_start(line);
    if target < b.begv() || target > b.zv() {
        ed.bm().narrow = None;
    }
    ed.goto(target);
}

fn goto_char(ed: &mut Editor) {
    if let Some(n) = ed.arg {
        ed.goto((n.max(1) - 1) as usize);
        return;
    }
    ed.read_string(
        "Go to char: ",
        PromptKind::GotoChar,
        "",
        Completion::None,
        "goto-char",
    );
}

// ======================= insertion =======================

fn self_insert_cmd(ed: &mut Editor) {
    // Invoked via M-x: nothing sensible to insert.
    ed.error("self-insert-command must be bound to a key");
}

pub fn self_insert(ed: &mut Editor, c: char) {
    if ed.sel == MINI && ed.mini.is_none() {
        return;
    }
    if !ed.check_writable() {
        return;
    }
    let n = ed.count().max(1) as usize;
    if ed.cfg.delete_selection_mode {
        if let Some((a, b)) = active_region(ed) {
            ed.delete(a, b);
            ed.goto(a);
        }
    }
    ed.last_insert = Some(c);
    let s: String = std::iter::repeat_n(c, n).collect();
    if ed.b().overwrite {
        let p = ed.pt();
        let e = ed.b().eol(p);
        let del = n.min(e - p);
        if del > 0 && ed.b().char_at(p) != Some('\t') {
            ed.delete(p, p + del);
        }
    }
    ed.insert(&s);
    electric_after_insert(ed, c);
}

fn electric_after_insert(ed: &mut Editor, c: char) {
    if ed.sel == MINI {
        return;
    }
    let style = ed.lang().style;
    let p = ed.pt();
    let b = ed.b();
    let bol = b.bol(p);
    let before = b.slice(bol, p);
    let trimmed = before.trim_start();
    let reindent = match style {
        IndentStyle::Braces => {
            matches!(c, '}' | ']' | ')') && trimmed.len() == 1
                || (c == ':' && (trimmed.starts_with("case ") || trimmed.starts_with("default")))
        }
        IndentStyle::Python => {
            c == ':'
                && ["else:", "elif ", "except", "finally:"]
                    .iter()
                    .any(|k| trimmed.starts_with(k))
        }
        IndentStyle::Shell => {
            trimmed == "fi"
                || trimmed == "done"
                || trimmed == "esac"
                || trimmed == "else"
                || trimmed == "}"
        }
        IndentStyle::Lisp => false,
        _ => false,
    };
    if reindent {
        let line = ed.b().line_of(p);
        indent_line(ed, line);
    }
}

fn newline(ed: &mut Editor) {
    if !ed.check_writable() {
        return;
    }
    let n = ed.count().max(1);
    let lang = ed.lang().clone();
    let electric = lang.prog && ed.sel != MINI;
    for _ in 0..n {
        if electric && matches!(lang.style, IndentStyle::Braces | IndentStyle::Shell) {
            let line = ed.b().line_of(ed.pt());
            reindent_if_changed(ed, line);
        }
        // electric-indent deletes trailing whitespace on the line we leave.
        let p = ed.pt();
        if electric {
            let b = ed.b();
            let mut s = p;
            while s > b.bol(p) && matches!(b.text.char(s - 1), ' ' | '\t') {
                s -= 1;
            }
            if s < p && b.eol(p) == p {
                ed.delete(s, p);
            }
        }
        ed.insert("\n");
        if electric {
            let line = ed.b().line_of(ed.pt());
            indent_line(ed, line);
        }
    }
}

fn newline_and_indent(ed: &mut Editor) {
    if !ed.check_writable() {
        return;
    }
    delete_horizontal_space(ed);
    ed.insert("\n");
    let line = ed.b().line_of(ed.pt());
    indent_line(ed, line);
}

fn plain_newline(ed: &mut Editor) {
    if !ed.check_writable() {
        return;
    }
    let n = ed.count().max(1) as usize;
    ed.insert(&"\n".repeat(n));
}

fn open_line(ed: &mut Editor) {
    if !ed.check_writable() {
        return;
    }
    let p = ed.pt();
    let n = ed.count().max(1) as usize;
    ed.insert(&"\n".repeat(n));
    ed.goto(p);
}

fn split_line(ed: &mut Editor) {
    if !ed.check_writable() {
        return;
    }
    let p = ed.pt();
    let b = ed.b();
    let mut q = p;
    while q < b.zv() && matches!(b.text.char(q), ' ' | '\t') {
        q += 1;
    }
    let tw = ed.tab_width();
    let col = ed.b().column(q, tw);
    ed.goto(q);
    let ind = lang::indent_string(col, ed.use_tabs(), tw);
    ed.insert(&format!("\n{ind}"));
    ed.goto(q.min(p.max(q)));
}

fn delete_char(ed: &mut Editor) {
    if !ed.check_writable() {
        return;
    }
    let n = ed.count();
    let p = ed.pt();
    let (bv, zv) = (ed.b().begv(), ed.b().zv());
    if n >= 0 {
        if p + n as usize > zv {
            ed.error("End of buffer");
            return;
        }
        if ed.arg.is_some() {
            ed.kill(p, p + n as usize);
        } else {
            ed.delete(p, p + n as usize);
        }
    } else {
        let k = (-n) as usize;
        if p < bv + k {
            ed.error("Beginning of buffer");
            return;
        }
        ed.delete(p - k, p);
    }
}

fn delete_forward_char(ed: &mut Editor) {
    if let Some((a, b)) = active_region(ed) {
        if ed.check_writable() {
            ed.delete(a, b);
        }
        return;
    }
    delete_char(ed);
}

fn delete_backward_char(ed: &mut Editor) {
    if !ed.check_writable() {
        return;
    }
    if let Some((a, b)) = active_region(ed) {
        if ed.arg.is_none() {
            ed.delete(a, b);
            return;
        }
    }
    let n = ed.count();
    if n < 0 {
        ed.arg = Some(-n);
        delete_char(ed);
        return;
    }
    let p = ed.pt();
    let n = n as usize;
    if p < ed.b().begv() + n {
        ed.error("Beginning of buffer");
        return;
    }
    if ed.b().overwrite {
        let s = " ".repeat(n);
        ed.delete(p - n, p);
        ed.bm().insert(p - n, &s);
        ed.goto(p - n);
        return;
    }
    if ed.arg.is_some() {
        ed.kill_backward(p - n, p);
    } else {
        ed.delete(p - n, p);
    }
}

fn kill_word(ed: &mut Editor) {
    let n = ed.count();
    let p = ed.pt();
    let mut q = p;
    for _ in 0..n.abs() {
        q = if n > 0 {
            text::forward_word(ed.b(), q)
        } else {
            text::backward_word(ed.b(), q)
        };
    }
    if !ed.check_writable() {
        ed.copy_as_kill(ed.b().slice(p.min(q), p.max(q)));
        return;
    }
    if q >= p {
        ed.kill(p, q)
    } else {
        ed.kill_backward(q, p)
    }
}

fn backward_kill_word(ed: &mut Editor) {
    ed.arg = Some(-ed.count());
    kill_word(ed);
}

fn kill_line(ed: &mut Editor) {
    let p = ed.pt();
    let b = ed.b();
    let end = match ed.arg {
        Some(0) => {
            let s = b.bol(p);
            if ed.check_writable() {
                ed.kill_backward(s, p);
            }
            return;
        }
        Some(n) if n > 0 => {
            let line = b.line_of(p) + n as usize;
            b.line_start(line).min(b.zv())
        }
        Some(n) => {
            let line = b.line_of(p).saturating_sub((-n) as usize);
            let s = b.line_start(line).max(b.begv());
            if ed.check_writable() {
                ed.kill_backward(s, p);
            }
            return;
        }
        None => {
            if p >= b.zv() {
                ed.error("End of buffer");
                return;
            }
            let e = b.eol(p);
            let rest_blank = !ed.cfg.show_trailing_whitespace
                && b.slice(p, e).chars().all(|c| c == ' ' || c == '\t');
            if e == p || rest_blank || (ed.cfg.kill_whole_line && p == b.bol(p)) {
                (e + 1).min(b.zv())
            } else {
                e
            }
        }
    };
    if !ed.check_writable() {
        let s = ed.b().slice(p, end);
        ed.copy_as_kill(s);
        return;
    }
    ed.kill(p, end);
}

fn kill_whole_line(ed: &mut Editor) {
    if !ed.check_writable() {
        return;
    }
    let p = ed.pt();
    let b = ed.b();
    let s = b.bol(p);
    let e = (b.eol(p) + 1).min(b.zv());
    ed.kill(s, e);
    ed.goto(s);
}

fn kill_region(ed: &mut Editor) {
    let Some((a, b)) = region(ed) else { return };
    if ed.b().read_only {
        let s = ed.b().slice(a, b);
        ed.copy_as_kill(s);
        ed.check_writable();
        return;
    }
    if ed.pt() == a && ed.pt() != b {
        // point at start: kill forward semantic (append)
        ed.kill(a, b);
    } else {
        ed.kill_backward(a, b);
    }
    ed.bm().mark_active = false;
}

fn kill_ring_save(ed: &mut Editor) {
    let Some((a, b)) = region(ed) else { return };
    let s = ed.b().slice(a, b);
    if ed.last_kill && !ed.kill_ring.is_empty() {
        ed.add_kill(s, false);
    } else {
        ed.copy_as_kill(s);
    }
    ed.bm().mark_active = false;
}

fn yank(ed: &mut Editor) {
    ed.sync_clipboard();
    if ed.kill_ring.is_empty() {
        ed.error("Kill ring is empty");
        return;
    }
    if !ed.check_writable() {
        return;
    }
    let idx = match ed.arg {
        Some(n) if ed.arg_digits => ((n - 1).max(0) as usize) % ed.kill_ring.len(),
        _ => 0,
    };
    ed.kill_idx = idx;
    let s = ed.kill_ring[idx].clone();
    let p = ed.pt();
    ed.bm().push_mark(p, false);
    ed.insert(&s);
    let e = ed.pt();
    ed.yank_range = Some((p, e));
    if ed.arg_universal && !ed.arg_digits {
        // C-u C-y: point before, mark after.
        ed.goto(p);
        let bm = ed.b().mark;
        ed.bm().set_mpos(bm, e);
    }
    ed.this_yank = true;
}

fn yank_pop(ed: &mut Editor) {
    if !ed.last_yank || ed.yank_range.is_none() {
        ed.error("Previous command was not a yank");
        return;
    }
    if !ed.check_writable() {
        return;
    }
    let (a, b) = ed.yank_range.unwrap();
    let n = ed.count();
    let len = ed.kill_ring.len() as i64;
    ed.kill_idx = (((ed.kill_idx as i64 + n) % len + len) % len) as usize;
    let s = ed.kill_ring[ed.kill_idx].clone();
    ed.delete(a, b);
    ed.goto(a);
    ed.insert(&s);
    ed.yank_range = Some((a, ed.pt()));
    let m = ed.b().mark;
    ed.bm().set_mpos(m, a);
    ed.this_yank = true;
}

fn set_mark_command(ed: &mut Editor) {
    let p = ed.pt();
    if ed.arg_universal && !ed.arg_digits {
        // C-u C-SPC: jump to mark, pop mark ring.
        let Some(m) = ed.b().mark_pos() else {
            ed.error("No mark set in this buffer");
            return;
        };
        ed.goto(m);
        let b = ed.bm();
        if !b.mark_ring.is_empty() {
            let id = b.mark_ring.remove(0);
            let np = b.mpos(id);
            b.mark_ring.push(id);
            b.set_mpos(id, p);
            let mm = b.mark;
            b.set_mpos(mm, np);
        }
        ed.bm().mark_active = false;
        return;
    }
    if ed.last_cmd == "set-mark-command" && ed.b().mark_active && ed.b().mark_pos() == Some(p) {
        ed.bm().mark_active = false;
        ed.msg("Mark deactivated");
        return;
    }
    let tmm = ed.cfg.transient_mark_mode;
    ed.bm().push_mark(p, tmm);
    ed.msg("Mark set");
}

fn exchange_point_and_mark(ed: &mut Editor) {
    let Some(m) = ed.b().mark_pos() else {
        ed.error("No mark set in this buffer");
        return;
    };
    let p = ed.pt();
    let mk = ed.b().mark;
    ed.bm().set_mpos(mk, p);
    ed.goto(m);
    if !ed.arg_universal || ed.arg_digits {
        ed.bm().mark_active = true;
    }
}

fn mark_whole_buffer(ed: &mut Editor) {
    let (a, z) = (ed.b().begv(), ed.b().zv());
    ed.bm().push_mark(z, true);
    ed.goto(a);
    ed.msg("Mark set");
}

fn mark_word(ed: &mut Editor) {
    let start = if ed.b().mark_active {
        ed.b().mark_pos().unwrap_or(ed.pt())
    } else {
        ed.pt()
    };
    let mut q = start;
    for _ in 0..ed.count().max(1) {
        q = text::forward_word(ed.b(), q);
    }
    if ed.b().mark_active {
        let m = ed.b().mark;
        ed.bm().set_mpos(m, q);
    } else {
        ed.bm().push_mark(q, true);
    }
}

fn mark_paragraph(ed: &mut Editor) {
    let p = ed.pt();
    let e = text::forward_paragraph(ed.b(), p);
    let s = text::backward_paragraph(ed.b(), e);
    ed.bm().push_mark(e, true);
    ed.goto(s);
}

fn mark_sexp(ed: &mut Editor) {
    let start = if ed.b().mark_active {
        ed.b().mark_pos().unwrap_or(ed.pt())
    } else {
        ed.pt()
    };
    match text::forward_sexp(ed.b(), start) {
        Some(q) => {
            if ed.b().mark_active {
                let m = ed.b().mark;
                ed.bm().set_mpos(m, q);
            } else {
                ed.bm().push_mark(q, true);
            }
        }
        None => ed.error("No next sexp"),
    }
}

fn mark_defun(ed: &mut Editor) {
    beginning_of_defun(ed);
    let s = ed.pt();
    end_of_defun(ed);
    let e = ed.pt();
    ed.bm().push_mark(e, true);
    ed.goto(s);
}

fn undo(ed: &mut Editor) {
    if !ed.check_writable() {
        return;
    }
    let continuing = matches!(ed.last_cmd.as_str(), "undo" | "undo-redo");
    let n = ed.count().max(1);
    for _ in 0..n {
        match ed.bm().undo_step(continuing) {
            Ok(p) => ed.goto(p),
            Err(e) => {
                ed.error(e);
                return;
            }
        }
    }
    if ed.sel != MINI {
        ed.msg("Undo");
    }
}

fn undo_redo(ed: &mut Editor) {
    if !matches!(ed.last_cmd.as_str(), "undo" | "undo-redo") {
        ed.error("No undo to undo");
        return;
    }
    match ed.bm().undo_step(false) {
        Ok(p) => {
            ed.goto(p);
            ed.msg("Redo");
        }
        Err(e) => ed.error(e),
    }
}

fn transpose_chars(ed: &mut Editor) {
    if !ed.check_writable() {
        return;
    }
    let mut p = ed.pt();
    let b = ed.b();
    if p == b.eol(p) && ed.arg.is_none() {
        // at end of line: transpose the two previous chars
        if p >= b.begv() + 2 {
            p -= 1;
        }
    }
    if p <= b.begv() || p >= b.zv() {
        ed.error(if p <= b.begv() {
            "Beginning of buffer"
        } else {
            "End of buffer"
        });
        return;
    }
    let a = b.slice(p - 1, p);
    let c = b.slice(p, p + 1);
    ed.delete(p - 1, p + 1);
    ed.bm().insert(p - 1, &(c + &a));
    ed.goto(p + 1);
}

fn transpose_words(ed: &mut Editor) {
    if !ed.check_writable() {
        return;
    }
    let p = ed.pt();
    let b = ed.b();
    let e2 = text::forward_word(b, p);
    let s2 = text::backward_word(b, e2);
    let s1 = text::backward_word(b, s2.min(p));
    let e1 = text::forward_word(b, s1);
    if s1 >= s2 || e1 > s2 || s2 == e2 {
        ed.error("Don't have two things to transpose");
        return;
    }
    let w1 = b.slice(s1, e1);
    let mid = b.slice(e1, s2);
    let w2 = b.slice(s2, e2);
    ed.delete(s1, e2);
    ed.bm().insert(s1, &format!("{w2}{mid}{w1}"));
    ed.goto(e2);
}

fn transpose_lines(ed: &mut Editor) {
    if !ed.check_writable() {
        return;
    }
    let p = ed.pt();
    let b = ed.b();
    let line = b.line_of(p);
    if line == 0 {
        ed.error("Don't have two things to transpose");
        return;
    }
    let mut cur = b.line_text(line);
    let prev = b.line_text(line - 1);
    let s = b.line_start(line - 1);
    let e = b.line_end(line);
    let at_end = e == b.len();
    if at_end {
        cur.push('\n');
    }
    let replacement = if at_end {
        format!("{cur}{prev}\n")
    } else {
        format!("{cur}\n{prev}")
    };
    ed.delete(s, e);
    ed.bm().insert(s, &replacement);
    let np = ed.b().line_start(line + 1).min(ed.b().len());
    ed.goto(np);
}

fn case_word(ed: &mut Editor, f: fn(&str) -> String) {
    if !ed.check_writable() {
        return;
    }
    let p = ed.pt();
    let n = ed.count();
    let mut q = p;
    for _ in 0..n.abs() {
        q = if n > 0 {
            text::forward_word(ed.b(), q)
        } else {
            text::backward_word(ed.b(), q)
        };
    }
    let (a, b) = (p.min(q), p.max(q));
    let s = ed.b().slice(a, b);
    let r = f(&s);
    if r != s {
        ed.delete(a, b);
        ed.bm().insert(a, &r);
    }
    ed.goto(if n > 0 { b } else { p });
}

fn capitalize(s: &str) -> String {
    let mut out = String::new();
    let mut in_word = false;
    for c in s.chars() {
        if text::is_word(c) {
            if in_word {
                out.extend(c.to_lowercase());
            } else {
                out.extend(c.to_uppercase());
            }
            in_word = true;
        } else {
            out.push(c);
            in_word = false;
        }
    }
    out
}

fn upcase_word(ed: &mut Editor) {
    case_word(ed, |s| s.to_uppercase());
}
fn downcase_word(ed: &mut Editor) {
    case_word(ed, |s| s.to_lowercase());
}
fn capitalize_word(ed: &mut Editor) {
    case_word(ed, capitalize);
}

fn case_region(ed: &mut Editor, f: fn(&str) -> String) {
    let Some((a, b)) = region(ed) else { return };
    if !ed.check_writable() {
        return;
    }
    let p = ed.pt();
    let s = ed.b().slice(a, b);
    let r = f(&s);
    if r != s {
        ed.delete(a, b);
        ed.bm().insert(a, &r);
    }
    ed.goto(p);
}

fn upcase_region(ed: &mut Editor) {
    case_region(ed, |s| s.to_uppercase());
}
fn downcase_region(ed: &mut Editor) {
    case_region(ed, |s| s.to_lowercase());
}
fn capitalize_region(ed: &mut Editor) {
    case_region(ed, capitalize);
}

fn delete_horizontal_space(ed: &mut Editor) {
    if !ed.check_writable() {
        return;
    }
    let p = ed.pt();
    let b = ed.b();
    let mut s = p;
    while s > b.bol(p) && matches!(b.text.char(s - 1), ' ' | '\t') {
        s -= 1;
    }
    let mut e = p;
    if !(ed.arg_universal) {
        while e < b.eol(p) && matches!(b.text.char(e), ' ' | '\t') {
            e += 1;
        }
    }
    ed.delete(s, e);
    ed.goto(s);
}

fn just_one_space(ed: &mut Editor) {
    if !ed.check_writable() {
        return;
    }
    let n = ed.count().max(0) as usize;
    let p = ed.pt();
    let b = ed.b();
    let mut s = p;
    while s > b.begv() && matches!(b.text.char(s - 1), ' ' | '\t') {
        s -= 1;
    }
    let mut e = p;
    while e < b.zv() && matches!(b.text.char(e), ' ' | '\t') {
        e += 1;
    }
    ed.delete(s, e);
    ed.bm().insert(s, &" ".repeat(n));
    ed.goto(s + n);
}

fn fixup_whitespace(ed: &mut Editor) {
    // Called with point at a join; leave one space or none.
    let p = ed.pt();
    let b = ed.b();
    let mut s = p;
    while s > b.begv() && matches!(b.text.char(s - 1), ' ' | '\t') {
        s -= 1;
    }
    let mut e = p;
    while e < b.zv() && matches!(b.text.char(e), ' ' | '\t') {
        e += 1;
    }
    ed.delete(s, e);
    ed.goto(s);
    let b = ed.b();
    let next = b.char_at(s);
    let prev = b.char_before(s);
    let none =
        next.is_none_or(|c| c == ')' || c == '\n') || prev.is_none_or(|c| c == '(' || c == '\n');
    if !none {
        ed.insert(" ");
        ed.goto(s);
    }
}

fn delete_indentation(ed: &mut Editor) {
    if !ed.check_writable() {
        return;
    }
    let p = ed.pt();
    let b = ed.b();
    let line = b.line_of(p);
    let line = if ed.arg.is_some() { line + 1 } else { line };
    if line == 0 || line >= b.line_count() {
        return;
    }
    let s = b.line_start(line);
    // delete newline and leading whitespace
    let mut e = s;
    while e < b.zv() && matches!(b.text.char(e), ' ' | '\t') {
        e += 1;
    }
    ed.delete(s - 1, e);
    ed.goto(s - 1);
    fixup_whitespace(ed);
}

fn delete_blank_lines(ed: &mut Editor) {
    if !ed.check_writable() {
        return;
    }
    let b = ed.b();
    let blank = |l: usize| b.line_text(l).trim().is_empty();
    let line = b.line_of(ed.pt());
    let last = b.line_count() - 1;
    if blank(line) {
        let mut a = line;
        while a > 0 && blank(a - 1) {
            a -= 1;
        }
        let mut z = line;
        while z < last && blank(z + 1) {
            z += 1;
        }
        let s = b.line_start(a);
        if a == z {
            // isolated blank line: delete it
            let e = (b.line_end(z) + 1).min(b.len());
            ed.delete(s, e);
            ed.goto(s);
        } else {
            let e = b.line_end(z);
            ed.delete(s, e);
            ed.goto(s);
        }
    } else {
        let mut z = line + 1;
        while z <= last && blank(z) {
            z += 1;
        }
        if z > line + 1 {
            let s = b.line_start(line + 1);
            let e = if z > last { b.len() } else { b.line_start(z) };
            ed.delete(s, e);
        }
    }
}

fn zap_to_char_cmd(ed: &mut Editor) {
    ed.message = Some("Zap to char: ".into());
    ed.read_key = Some(ReadKey::ZapToChar);
    ed.keep_arg = true;
}

pub fn zap_to_char(ed: &mut Editor, c: char) {
    if !ed.check_writable() {
        ed.reset_arg();
        return;
    }
    let n = ed.count();
    let p = ed.pt();
    let b = ed.b();
    let mut q = p;
    let mut found = 0;
    if n > 0 {
        while q < b.zv() {
            let d = b.text.char(q);
            q += 1;
            if d == c {
                found += 1;
                if found == n {
                    break;
                }
            }
        }
    } else {
        while q > b.begv() {
            q -= 1;
            if b.text.char(q) == c {
                found += 1;
                if found == -n {
                    break;
                }
            }
        }
    }
    ed.reset_arg();
    if found < n.abs() {
        ed.error(format!("Search failed: \"{c}\""));
        return;
    }
    if q > p {
        ed.kill(p, q)
    } else {
        ed.kill_backward(q, p)
    }
}

// ======================= indentation =======================

pub fn current_indentation(ed: &Editor, line: usize) -> usize {
    lang::indentation_of(&ed.b().line_text(line), ed.tab_width())
}

/// Re-indent `line` to `col`, keeping point sensible.
pub fn indent_line_to(ed: &mut Editor, line: usize, col: usize) {
    let tw = ed.tab_width();
    let tabs = ed.use_tabs();
    let b = ed.b();
    let s = b.line_start(line);
    let mut e = s;
    while e < b.len() && matches!(b.text.char(e), ' ' | '\t') {
        e += 1;
    }
    let want = lang::indent_string(col, tabs, tw);
    if b.slice(s, e) == want {
        let p = ed.pt();
        if p >= s && p < e {
            ed.goto(e);
        }
        return;
    }
    let p = ed.pt();
    let in_indent = p >= s && p <= e;
    ed.delete(s, e);
    ed.bm().insert(s, &want);
    if in_indent {
        ed.goto(s + want.chars().count());
    }
}

/// Indent a line according to the major mode.
pub fn indent_line(ed: &mut Editor, line: usize) {
    let lang = ed.lang().clone();
    let mut l = lang.clone();
    l.indent = ed.indent_width();
    l.tab_width = ed.tab_width();
    if let Some(col) = lang::compute_indent(ed.b(), &l, line) {
        indent_line_to(ed, line, col);
    }
}

fn reindent_if_changed(ed: &mut Editor, line: usize) {
    let mut l = ed.lang().clone();
    l.indent = ed.indent_width();
    l.tab_width = ed.tab_width();
    if ed.b().line_text(line).trim().is_empty() {
        return;
    }
    if let Some(col) = lang::compute_indent(ed.b(), &l, line) {
        if col != current_indentation(ed, line) {
            let p = ed.pt();
            let s = ed.b().line_start(line);
            let old_end = indentation_end(ed, s);
            indent_line_to(ed, line, col);
            if p > old_end {
                // keep point at same text position (markers already shifted)
            }
        }
    }
}

fn indent_for_tab(ed: &mut Editor) {
    if ed.sel == MINI {
        return;
    }
    if !ed.check_writable() {
        return;
    }
    if active_region(ed).is_some() {
        indent_region(ed);
        return;
    }
    let lang = ed.lang().clone();
    let line = ed.b().line_of(ed.pt());
    match lang.style {
        IndentStyle::Relative if lang.tabs && lang.prog => {
            ed.insert("\t");
        }
        IndentStyle::Relative if !lang.prog => indent_relative(ed),
        IndentStyle::Relative => {
            let mut l = lang.clone();
            l.indent = ed.indent_width();
            let want = lang::compute_indent(ed.b(), &l, line).unwrap_or(0);
            let cur = current_indentation(ed, line);
            let p = ed.pt();
            let ie = indentation_end(ed, p);
            if cur != want && p <= ie {
                indent_line_to(ed, line, want);
            } else if p <= ie && ed.last_cmd != "indent-for-tab-command" {
                ed.goto(ie);
            } else {
                indent_relative(ed);
            }
        }
        IndentStyle::Python | IndentStyle::Yaml => {
            let mut l = lang.clone();
            l.indent = ed.indent_width();
            l.tab_width = ed.tab_width();
            let want = lang::compute_indent(ed.b(), &l, line).unwrap_or(0);
            let cur = current_indentation(ed, line);
            let col = if ed.last_cmd == "indent-for-tab-command" && cur <= want {
                // cycle: dedent one level, wrapping to the deepest
                if cur == 0 {
                    want
                } else {
                    cur.saturating_sub(l.indent)
                }
            } else {
                want
            };
            indent_line_to(ed, line, col);
        }
        _ => indent_line(ed, line),
    }
}

fn indent_relative(ed: &mut Editor) {
    let p = ed.pt();
    let tw = ed.tab_width();
    let b = ed.b();
    let col = b.column(p, tw);
    let line = b.line_of(p);
    let mut target = None;
    let mut l = line;
    while l > 0 {
        l -= 1;
        let t = b.line_text(l);
        if t.trim().is_empty() {
            continue;
        }
        // next indent point: start of a word after whitespace, beyond col
        let mut c = 0;
        let mut prev_ws = true;
        for ch in t.chars() {
            let is_ws = ch == ' ' || ch == '\t';
            if !is_ws && prev_ws && c > col {
                target = Some(c);
                break;
            }
            prev_ws = is_ws;
            c = text::advance_col(c, ch, tw);
        }
        break;
    }
    match target {
        Some(t) => {
            // delete whitespace before point first (indent-relative does that)
            let mut s = p;
            while s > b.bol(p) && matches!(b.text.char(s - 1), ' ' | '\t') {
                s -= 1;
            }
            let base = b.column(s, tw);
            ed.delete(s, p);
            ed.goto(s);
            let ws = if ed.use_tabs() {
                lang::indent_string(t, true, tw)[..].to_string()
            } else {
                " ".repeat(t - base)
            };
            let ws = if ed.use_tabs() && base == 0 {
                ws
            } else {
                " ".repeat(t - base)
            };
            ed.insert(&ws);
        }
        None => tab_to_tab_stop(ed),
    }
}

fn indent_relative_cmd(ed: &mut Editor) {
    if ed.check_writable() {
        indent_relative(ed);
    }
}

fn tab_to_tab_stop(ed: &mut Editor) {
    if !ed.check_writable() {
        return;
    }
    let tw = ed.tab_width();
    let p = ed.pt();
    let col = ed.b().column(p, tw);
    let next = (col / tw + 1) * tw;
    if ed.use_tabs() {
        ed.insert("\t");
    } else {
        ed.insert(&" ".repeat(next - col));
    }
}

fn indent_region(ed: &mut Editor) {
    let Some((a, b)) = region(ed) else { return };
    if !ed.check_writable() {
        return;
    }
    let (l0, l1) = {
        let bf = ed.b();
        let l1 = bf.line_of(b);
        let l1 = if bf.line_start(l1) == b && l1 > bf.line_of(a) {
            l1 - 1
        } else {
            l1
        };
        (bf.line_of(a), l1)
    };
    let p = ed.pt();
    let pm = ed.bm().new_marker(p, false);
    for l in l0..=l1 {
        if ed.b().line_text(l).trim().is_empty() {
            let s = ed.b().line_start(l);
            let e = ed.b().line_end(l);
            ed.delete(s, e);
        } else {
            indent_line(ed, l);
        }
    }
    let np = ed.b().mpos(pm);
    ed.bm().free_marker(pm);
    ed.goto(np);
    ed.msg("Indenting region...done");
}

fn indent_rigidly(ed: &mut Editor) {
    if region(ed).is_none() {
        return;
    }
    if let Some(n) = ed.arg {
        indent_rigidly_by(ed, n);
        return;
    }
    ed.bm().mark_active = true;
    ed.message = Some("Indent region with <left>, <right>, <S-left>, or <S-right>.".into());
    ed.read_key = Some(ReadKey::IndentRigidly);
}

pub fn indent_rigidly_by(ed: &mut Editor, delta: i64) {
    let Some(m) = ed.b().mark_pos() else { return };
    if !ed.check_writable() {
        return;
    }
    let p = ed.pt();
    let (a, b) = (m.min(p), m.max(p));
    let (l0, l1) = {
        let bf = ed.b();
        let l1 = bf.line_of(b);
        let l1 = if bf.line_start(l1) == b && l1 > bf.line_of(a) {
            l1 - 1
        } else {
            l1
        };
        (bf.line_of(a), l1)
    };
    for l in l0..=l1 {
        if ed.b().line_text(l).trim().is_empty() {
            continue;
        }
        let cur = current_indentation(ed, l) as i64;
        indent_line_to(ed, l, (cur + delta).max(0) as usize);
    }
    ed.bm().mark_active = true;
    ed.bm().deactivate = false;
}

// ======================= comments =======================

fn comment_parts(ed: &Editor) -> (String, String) {
    let l = ed.lang();
    let mut s = l.comment_start.clone();
    if s.is_empty() {
        s = "#".into();
    }
    if s == ";" {
        s = ";;".into();
    }
    (s, l.comment_end.clone())
}

fn line_is_commented(t: &str, cs: &str) -> bool {
    let tt = t.trim_start();
    tt.starts_with(cs) || (cs == ";;" && tt.starts_with(';'))
}

fn comment_lines(ed: &mut Editor, l0: usize, l1: usize) {
    let (cs, ce) = comment_parts(ed);
    let tw = ed.tab_width();
    let lines: Vec<usize> = (l0..=l1)
        .filter(|&l| !ed.b().line_text(l).trim().is_empty())
        .collect();
    if lines.is_empty() {
        return;
    }
    let all_commented = lines
        .iter()
        .all(|&l| line_is_commented(&ed.b().line_text(l), &cs));
    if all_commented {
        for &l in &lines {
            let b = ed.b();
            let t = b.line_text(l);
            let s = b.line_start(l);
            let lead = t.len() - t.trim_start().len();
            let lead_chars = t[..lead].chars().count();
            let rest = &t[lead..];
            let marker = if rest.starts_with(&cs) {
                cs.clone()
            } else {
                ";".into()
            };
            let mut n = marker.chars().count();
            if rest[marker.len()..].starts_with(' ') {
                n += 1;
            }
            ed.delete(s + lead_chars, s + lead_chars + n);
            if !ce.is_empty() {
                let t = ed.b().line_text(l);
                let tt = t.trim_end();
                if tt.ends_with(&ce) {
                    let s = ed.b().line_start(l);
                    let mut cut = tt.chars().count() - ce.chars().count();
                    if tt[..tt.len() - ce.len()].ends_with(' ') {
                        cut -= 1;
                    }
                    let e = s + t.chars().count();
                    ed.delete(s + cut, e);
                }
            }
        }
    } else {
        let min_ind = lines
            .iter()
            .map(|&l| lang::indentation_of(&ed.b().line_text(l), tw))
            .min()
            .unwrap_or(0);
        for &l in &lines {
            let pos = ed.b().pos_at_column(ed.b().line_start(l), min_ind, tw);
            ed.bm().insert(pos, &format!("{cs} "));
            if !ce.is_empty() {
                let e = ed.b().line_end(l);
                ed.bm().insert(e, &format!(" {ce}"));
            }
        }
    }
}

fn region_lines(ed: &Editor, a: usize, b: usize) -> (usize, usize) {
    let bf = ed.b();
    let l0 = bf.line_of(a);
    let mut l1 = bf.line_of(b);
    if bf.line_start(l1) == b && l1 > l0 {
        l1 -= 1;
    }
    (l0, l1)
}

fn comment_dwim(ed: &mut Editor) {
    if !ed.check_writable() {
        return;
    }
    if let Some((a, b)) = active_region(ed) {
        let (l0, l1) = region_lines(ed, a, b);
        comment_lines(ed, l0, l1);
        return;
    }
    let (cs, ce) = comment_parts(ed);
    let p = ed.pt();
    let line = ed.b().line_of(p);
    let t = ed.b().line_text(line);
    let s = ed.b().line_start(line);
    if t.trim().is_empty() {
        indent_line(ed, line);
        let e = ed.b().line_end(line);
        ed.goto(e);
        ed.insert(&format!("{cs} "));
        let q = ed.pt();
        if !ce.is_empty() {
            ed.insert(&format!(" {ce}"));
            ed.goto(q);
        }
        return;
    }
    if let Some(i) = find_comment_start(&t, &cs) {
        let mut q = s + t[..i].chars().count() + cs.chars().count();
        if t[i + cs.len()..].starts_with(' ') {
            q += 1;
        }
        ed.goto(q);
        return;
    }
    // Add an end-of-line comment at comment-column (32).
    let e = ed.b().line_end(line);
    ed.goto(e);
    delete_horizontal_space(ed);
    let tw = ed.tab_width();
    let col = ed.b().column(ed.pt(), tw);
    let target = if col < 32 { 32 } else { col + 1 };
    ed.insert(&" ".repeat(target - col));
    ed.insert(&format!("{cs} "));
    if !ce.is_empty() {
        let q = ed.pt();
        ed.insert(&format!(" {ce}"));
        ed.goto(q);
    }
}

fn find_comment_start(t: &str, cs: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    let mut prev = ' ';
    for (i, c) in t.char_indices() {
        if let Some(q) = quote {
            if c == q && prev != '\\' {
                quote = None;
            }
        } else if c == '"' || (c == '\'' && cs != "#") {
            quote = Some(c);
        } else if t[i..].starts_with(cs) {
            return Some(i);
        }
        prev = c;
    }
    None
}

fn comment_line(ed: &mut Editor) {
    if !ed.check_writable() {
        return;
    }
    let (l0, l1) = match active_region(ed) {
        Some((a, b)) => region_lines(ed, a, b),
        None => {
            let l = ed.b().line_of(ed.pt());
            let n = ed.count().max(1) as usize;
            (l, (l + n - 1).min(ed.b().line_count() - 1))
        }
    };
    comment_lines(ed, l0, l1);
    if active_region(ed).is_none() {
        let p = ed.b().line_start(l1 + 1).min(ed.b().len());
        ed.goto(p);
    }
}

fn comment_region(ed: &mut Editor) {
    let Some((a, b)) = region(ed) else { return };
    if ed.check_writable() {
        let (l0, l1) = region_lines(ed, a, b);
        comment_lines(ed, l0, l1);
    }
}

fn uncomment_region(ed: &mut Editor) {
    comment_region(ed);
}

fn fill_paragraph(ed: &mut Editor) {
    if !ed.check_writable() {
        return;
    }
    let fill = ed.cfg.fill_column;
    let (cs, _) = comment_parts(ed);
    let prog = ed.lang().prog;
    let b = ed.b();
    let line = b.line_of(ed.pt());
    let t = b.line_text(line);
    let lead: String = t.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
    let rest = &t[lead.len()..];
    let prefix = if line_is_commented(&t, &cs) {
        let marker: String = rest
            .chars()
            .take_while(|c| !c.is_alphanumeric() && *c != ' ')
            .collect();
        let sp: String = rest[marker.len()..]
            .chars()
            .take_while(|c| *c == ' ')
            .collect();
        format!("{lead}{marker}{sp}")
    } else if prog {
        ed.msg("Not inside a comment; nothing to fill");
        return;
    } else {
        lead.clone()
    };
    let core = prefix.trim_end().to_string();
    let belongs = |t: &str| -> bool {
        if core.is_empty() {
            !t.trim().is_empty()
        } else {
            t.starts_with(&core) && !t[core.len()..].trim().is_empty()
        }
    };
    if !belongs(&t) {
        return;
    }
    let mut l0 = line;
    while l0 > 0 && belongs(&b.line_text(l0 - 1)) {
        l0 -= 1;
    }
    let mut l1 = line;
    while l1 + 1 < b.line_count() && belongs(&b.line_text(l1 + 1)) {
        l1 += 1;
    }
    let words: Vec<String> = (l0..=l1)
        .flat_map(|l| {
            let t = b.line_text(l);
            let body = if t.len() >= prefix.len() && t.starts_with(&prefix) {
                t[prefix.len()..].to_string()
            } else {
                t.trim_start().trim_start_matches(core.trim()).to_string()
            };
            body.split_whitespace()
                .map(String::from)
                .collect::<Vec<_>>()
        })
        .collect();
    let mut out = String::new();
    let mut cur = prefix.clone();
    let mut has = false;
    for w in words {
        if has && cur.chars().count() + 1 + w.chars().count() > fill {
            out.push_str(cur.trim_end());
            out.push('\n');
            cur = prefix.clone();
            has = false;
        }
        if has {
            cur.push(' ');
        }
        cur.push_str(&w);
        has = true;
    }
    out.push_str(cur.trim_end());
    let s = b.line_start(l0);
    let e = b.line_end(l1);
    if b.slice(s, e) != out {
        let p = ed.pt();
        ed.delete(s, e);
        ed.bm().insert(s, &out);
        ed.goto(p);
    }
}

fn set_fill_column(ed: &mut Editor) {
    let n = match ed.arg {
        Some(n) if ed.arg_digits => n.max(1) as usize,
        _ => {
            let tw = ed.tab_width();
            ed.b().column(ed.pt(), tw)
        }
    };
    let old = ed.cfg.fill_column;
    ed.cfg.fill_column = n;
    ed.msg(format!("Fill column set to {n} (was {old})"));
}

fn quoted_insert(ed: &mut Editor) {
    ed.read_key = Some(ReadKey::QuotedInsert);
    ed.keep_arg = true;
    ed.message = Some("C-q-".into());
}

fn overwrite_mode(ed: &mut Editor) {
    let b = ed.bm();
    b.overwrite = !b.overwrite;
    let on = b.overwrite;
    ed.msg(if on {
        "Overwrite mode enabled in current buffer"
    } else {
        "Overwrite mode disabled in current buffer"
    });
}

// ======================= dabbrev =======================

pub struct Dabbrev {
    start: usize,
    abbrev: String,
    inserted: usize,
    /// next search position backward / forward
    back: Option<usize>,
    fwd: usize,
    seen: Vec<String>,
    other_bufs: Vec<usize>,
    other_texts: Vec<String>,
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn dabbrev_expand(ed: &mut Editor) {
    if !ed.check_writable() {
        return;
    }
    let p = ed.pt();
    let continuing = ed.last_cmd == "dabbrev-expand" && ed.dabbrev.is_some();
    if !continuing {
        let b = ed.b();
        let mut s = p;
        while s > b.begv() && is_ident(b.text.char(s - 1)) {
            s -= 1;
        }
        if s == p {
            ed.error("No dynamic expansion for \"\" found");
            return;
        }
        let abbrev = b.slice(s, p);
        let cur = ed.cur();
        let others: Vec<usize> = ed
            .live_buffers()
            .into_iter()
            .filter(|&x| x != cur)
            .collect();
        ed.dabbrev = Some(Dabbrev {
            start: s,
            abbrev: abbrev.clone(),
            inserted: 0,
            back: Some(s),
            fwd: p,
            seen: vec![abbrev],
            other_bufs: others,
            other_texts: Vec::new(),
        });
    }
    let mut d = ed.dabbrev.take().unwrap();
    // remove previous expansion
    let exp_start = d.start + d.abbrev.chars().count();
    if d.inserted > 0 {
        ed.delete(exp_start, exp_start + d.inserted);
        ed.goto(exp_start);
    }
    let found = dabbrev_next(ed, &mut d);
    match found {
        Some(w) => {
            let suffix: String = w.chars().skip(d.abbrev.chars().count()).collect();
            d.inserted = suffix.chars().count();
            d.seen.push(w);
            ed.goto(exp_start);
            ed.insert(&suffix);
            ed.dabbrev = Some(d);
        }
        None => {
            d.inserted = 0;
            let a = d.abbrev.clone();
            ed.error(format!(
                "No {}dynamic expansion for `{a}' found",
                if continuing { "further " } else { "" }
            ));
            ed.dabbrev = None;
        }
    }
}

fn dabbrev_next(ed: &mut Editor, d: &mut Dabbrev) -> Option<String> {
    let ab: Vec<char> = d.abbrev.chars().collect();
    let check = |txt: &dyn Fn(usize) -> Option<char>, s: usize| -> Option<String> {
        // match at word start s
        if s > 0 && txt(s - 1).map(is_ident).unwrap_or(false) {
            return None;
        }
        for (i, &c) in ab.iter().enumerate() {
            if txt(s + i) != Some(c) {
                return None;
            }
        }
        let mut e = s + ab.len();
        let mut w: String = ab.iter().collect();
        while let Some(c) = txt(e) {
            if !is_ident(c) {
                break;
            }
            w.push(c);
            e += 1;
        }
        Some(w)
    };
    {
        let b = ed.b();
        let txt = |i: usize| b.char_at(i);
        // backward
        while let Some(pos) = d.back {
            if pos == 0 {
                d.back = None;
                break;
            }
            let s = pos - 1;
            d.back = Some(s);
            if let Some(w) = check(&txt, s) {
                if !d.seen.contains(&w) {
                    return Some(w);
                }
            }
        }
        // forward
        let exp_end = d.start + ab.len();
        if d.fwd < exp_end {
            d.fwd = exp_end;
        }
        while d.fwd < b.len() {
            let s = d.fwd;
            d.fwd += 1;
            if let Some(w) = check(&txt, s) {
                if !d.seen.contains(&w) {
                    return Some(w);
                }
            }
        }
    }
    // other buffers
    loop {
        if d.other_texts.is_empty() {
            let bid = d.other_bufs.pop()?;
            let t = ed.buf(bid).text.to_string();
            d.other_texts.push(t);
            d.fwd = 0;
        }
        let t: Vec<char> = d.other_texts[0].chars().collect();
        let txt = |i: usize| t.get(i).copied();
        while d.fwd < t.len() {
            let s = d.fwd;
            d.fwd += 1;
            if let Some(w) = check(&txt, s) {
                if !d.seen.contains(&w) {
                    return Some(w);
                }
            }
        }
        d.other_texts.clear();
    }
}

// ======================= info =======================

fn count_words(ed: &mut Editor) {
    let (a, b, what) = match active_region(ed) {
        Some((a, b)) => (a, b, "Region"),
        None => (ed.b().begv(), ed.b().zv(), "Buffer"),
    };
    let s = ed.b().slice(a, b);
    let lines = s.matches('\n').count()
        + if s.is_empty() || s.ends_with('\n') {
            0
        } else {
            1
        };
    let words = s
        .split(|c: char| !text::is_word(c))
        .filter(|w| !w.is_empty())
        .count();
    let chars = s.chars().count();
    ed.msg(format!(
        "{what} has {lines} line{}, {words} word{}, and {chars} character{}.",
        if lines == 1 { "" } else { "s" },
        if words == 1 { "" } else { "s" },
        if chars == 1 { "" } else { "s" }
    ));
}

fn what_cursor_position(ed: &mut Editor) {
    let p = ed.pt();
    let tw = ed.tab_width();
    let b = ed.b();
    let total = b.len();
    let pct = if total == 0 { 0 } else { p * 100 / total };
    let col = b.column(p, tw);
    match b.char_at(p) {
        Some(c) => {
            let n = c as u32;
            let disp = match c {
                '\n' => "C-j".to_string(),
                '\t' => "TAB".to_string(),
                ' ' => "SPC".to_string(),
                c if (c as u32) < 32 => format!("^{}", (c as u8 + 64) as char),
                c => c.to_string(),
            };
            let s = format!(
                "Char: {disp} ({n}, #o{n:o}, #x{n:x}) point={} of {} ({pct}%) column={col}",
                p + 1,
                total + 1
            );
            ed.msg(s);
        }
        None => {
            let s = format!("point={} of {} (EOB) column={col}", p + 1, total + 1);
            ed.msg(s);
        }
    }
}

fn what_line(ed: &mut Editor) {
    let l = ed.b().line_of(ed.pt()) + 1;
    ed.msg(format!("Line {l}"));
}

fn count_lines_page(ed: &mut Editor) {
    let b = ed.b();
    let l = b.line_of(ed.pt());
    let total = b.line_count();
    ed.msg(format!("Page has {} lines ({} + {})", total, l, total - l));
}

fn kill_sexp(ed: &mut Editor) {
    let p = ed.pt();
    match text::forward_sexp(ed.b(), p) {
        Some(q) => {
            if ed.check_writable() {
                ed.kill(p, q)
            }
        }
        None => ed.error("Scan error: \"Containing expression ends prematurely\""),
    }
}

fn backward_kill_sexp(ed: &mut Editor) {
    let p = ed.pt();
    match text::backward_sexp(ed.b(), p) {
        Some(q) => {
            if ed.check_writable() {
                ed.kill_backward(q, p)
            }
        }
        None => ed.error("Scan error: \"Containing expression ends prematurely\""),
    }
}

fn kill_sentence(ed: &mut Editor) {
    let p = ed.pt();
    let q = text::forward_sentence(ed.b(), p);
    if ed.check_writable() {
        ed.kill(p, q);
    }
}

fn insert_time(ed: &mut Editor) {
    if !ed.check_writable() {
        return;
    }
    let s = chrono::Local::now()
        .format("%a %b %e %H:%M:%S %Y")
        .to_string();
    ed.insert(&s);
}

fn region_or_buffer(ed: &Editor) -> (usize, usize) {
    match active_region(ed) {
        Some(r) => r,
        None => (ed.b().begv(), ed.b().zv()),
    }
}

fn map_lines(ed: &mut Editor, a: usize, b: usize, f: &dyn Fn(Vec<String>) -> Vec<String>) {
    if !ed.check_writable() {
        return;
    }
    let bf = ed.b();
    let l0 = bf.line_of(a);
    let mut l1 = bf.line_of(b);
    if bf.line_start(l1) == b && l1 > l0 {
        l1 -= 1;
    }
    let s = bf.line_start(l0);
    let e = bf.line_end(l1);
    let lines: Vec<String> = (l0..=l1).map(|l| bf.line_text(l)).collect();
    let new = f(lines).join("\n");
    if bf.slice(s, e) != new {
        let p = ed.pt();
        ed.delete(s, e);
        ed.bm().insert(s, &new);
        ed.goto(p);
    }
}

fn delete_trailing_whitespace(ed: &mut Editor) {
    let (a, b) = region_or_buffer(ed);
    map_lines(ed, a, b, &|ls| {
        ls.into_iter().map(|l| l.trim_end().to_string()).collect()
    });
    // Also delete trailing blank lines at end of buffer (Emacs does this when no region).
    if active_region(ed).is_none() {
        let bf = ed.b();
        let s = bf.slice(0, bf.len());
        let trimmed = s.trim_end_matches('\n');
        if trimmed.len() + 1 < s.len() {
            let keep = trimmed.chars().count() + 1;
            let len = bf.len();
            ed.delete(keep, len);
        }
    }
}

fn untabify(ed: &mut Editor) {
    let (a, b) = region_or_buffer(ed);
    let tw = ed.tab_width();
    map_lines(ed, a, b, &|ls| {
        ls.into_iter()
            .map(|l| {
                let mut out = String::new();
                let mut col = 0;
                for c in l.chars() {
                    if c == '\t' {
                        let n = text::advance_col(col, c, tw) - col;
                        out.push_str(&" ".repeat(n));
                        col += n;
                    } else {
                        out.push(c);
                        col = text::advance_col(col, c, tw);
                    }
                }
                out
            })
            .collect()
    });
}

fn tabify(ed: &mut Editor) {
    let (a, b) = region_or_buffer(ed);
    let tw = ed.tab_width();
    map_lines(ed, a, b, &|ls| {
        ls.into_iter()
            .map(|l| {
                let ind = lang::indentation_of(&l, tw);
                let body = l.trim_start_matches([' ', '\t']);
                format!("{}{}", lang::indent_string(ind, true, tw), body)
            })
            .collect()
    });
}

fn sort_lines(ed: &mut Editor) {
    let Some((a, b)) = region(ed) else { return };
    let rev = ed.arg.is_some();
    map_lines(ed, a, b, &|mut ls| {
        ls.sort();
        if rev {
            ls.reverse();
        }
        ls
    });
}

fn reverse_region(ed: &mut Editor) {
    let Some((a, b)) = region(ed) else { return };
    map_lines(ed, a, b, &|mut ls| {
        ls.reverse();
        ls
    });
}

fn delete_duplicate_lines(ed: &mut Editor) {
    let Some((a, b)) = region(ed) else { return };
    map_lines(ed, a, b, &|ls| {
        let mut seen = std::collections::HashSet::new();
        ls.into_iter().filter(|l| seen.insert(l.clone())).collect()
    });
}

fn flush_lines(ed: &mut Editor) {
    ed.read_string(
        "Flush lines containing match for regexp: ",
        PromptKind::FlushLines { keep: false },
        "",
        Completion::None,
        "regexp",
    );
}

fn keep_lines(ed: &mut Editor) {
    ed.read_string(
        "Keep lines containing match for regexp: ",
        PromptKind::FlushLines { keep: true },
        "",
        Completion::None,
        "regexp",
    );
}

pub fn flush_lines_apply(ed: &mut Editor, re: &str, keep: bool) {
    let re = match regex::Regex::new(re) {
        Ok(r) => r,
        Err(e) => {
            ed.error(format!("Invalid regexp: {e}"));
            return;
        }
    };
    let (a, b) = match active_region(ed) {
        Some(r) => r,
        None => (ed.b().bol(ed.pt()), ed.b().zv()),
    };
    let before = ed.b().line_count();
    map_lines(ed, a, b, &|ls| {
        ls.into_iter().filter(|l| re.is_match(l) == keep).collect()
    });
    let removed = before - ed.b().line_count();
    ed.msg(format!(
        "Deleted {removed} matching line{}",
        if removed == 1 { "" } else { "s" }
    ));
}

fn repeat(ed: &mut Editor) {
    let Some((cmd, arg)) = ed.last_repeatable.clone() else {
        ed.error("No repeatable command");
        return;
    };
    ed.repeat_key = Some((
        crate::key::Key::new(crate::key::K::Char('z')),
        "repeat".into(),
    ));
    ed.arg = arg;
    if cmd == "self-insert-command" {
        if let Some(c) = ed.last_insert {
            self_insert(ed, c);
        }
        return;
    }
    if let Some(f) = lookup(&cmd) {
        f(ed);
    }
}

// ======================= rectangles =======================

fn rect(ed: &mut Editor) -> Option<(usize, usize, usize, usize)> {
    let (a, b) = region(ed)?;
    let tw = ed.tab_width();
    let bf = ed.b();
    let (l0, l1) = (bf.line_of(a), bf.line_of(b));
    let (c0, c1) = (bf.column(a, tw), bf.column(b, tw));
    let p = ed.pt();
    let _ = p;
    let (c0, c1) = (c0.min(c1), c0.max(c1));
    Some((l0, l1, c0, c1))
}

fn rect_line_range(
    ed: &mut Editor,
    line: usize,
    c0: usize,
    c1: usize,
    pad: bool,
) -> (usize, usize) {
    let tw = ed.tab_width();
    let ls = ed.b().line_start(line);
    let le = ed.b().line_end(line);
    let col_end = ed.b().column(le, tw);
    if pad && col_end < c0 {
        ed.bm().insert(le, &" ".repeat(c0 - col_end));
    }
    let s = ed.b().pos_at_column(ls, c0, tw);
    let e = ed.b().pos_at_column(ls, c1, tw);
    (s, e.max(s))
}

fn extract_rect(ed: &mut Editor, delete: bool, replace: Option<&str>) -> Option<Vec<String>> {
    let (l0, l1, c0, c1) = rect(ed)?;
    if (delete || replace.is_some()) && !ed.check_writable() {
        return None;
    }
    let mut out = Vec::new();
    for l in l0..=l1 {
        let (s, e) = rect_line_range(ed, l, c0, c1, replace.is_some());
        let txt = ed.b().slice(s, e);
        let w: usize = txt.chars().map(text::char_width).sum();
        out.push(format!("{txt}{}", " ".repeat((c1 - c0).saturating_sub(w))));
        if delete {
            ed.delete(s, e);
        }
        if let Some(r) = replace {
            ed.delete(s, e);
            ed.bm().insert(s, r);
        }
    }
    Some(out)
}

thread_local! {
    static KILLED_RECT: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn kill_rectangle(ed: &mut Editor) {
    if let Some(r) = extract_rect(ed, true, None) {
        KILLED_RECT.with(|k| *k.borrow_mut() = r);
        ed.bm().mark_active = false;
    }
}

fn delete_rectangle(ed: &mut Editor) {
    extract_rect(ed, true, None);
    ed.bm().mark_active = false;
}

fn copy_rectangle_as_kill(ed: &mut Editor) {
    if let Some(r) = extract_rect(ed, false, None) {
        KILLED_RECT.with(|k| *k.borrow_mut() = r);
        ed.bm().mark_active = false;
    }
}

fn clear_rectangle(ed: &mut Editor) {
    let Some((_, _, c0, c1)) = rect(ed) else {
        return;
    };
    let blank = " ".repeat(c1 - c0);
    extract_rect(ed, false, Some(&blank));
    ed.bm().mark_active = false;
}

fn open_rectangle(ed: &mut Editor) {
    let Some((l0, l1, c0, c1)) = rect(ed) else {
        return;
    };
    if !ed.check_writable() {
        return;
    }
    for l in l0..=l1 {
        let (s, _) = rect_line_range(ed, l, c0, c0, true);
        ed.bm().insert(s, &" ".repeat(c1 - c0));
    }
    ed.bm().mark_active = false;
}

fn yank_rectangle(ed: &mut Editor) {
    if !ed.check_writable() {
        return;
    }
    let r = KILLED_RECT.with(|k| k.borrow().clone());
    if r.is_empty() {
        ed.error("No rectangle killed");
        return;
    }
    insert_rectangle(ed, &r);
}

fn insert_rectangle(ed: &mut Editor, r: &[String]) {
    let tw = ed.tab_width();
    let p = ed.pt();
    let col = ed.b().column(p, tw);
    let mut line = ed.b().line_of(p);
    let mut last = p;
    for (i, s) in r.iter().enumerate() {
        if i > 0 {
            line += 1;
            if line >= ed.b().line_count() {
                let n = ed.b().len();
                ed.bm().insert(n, "\n");
            }
        }
        let (pos, _) = rect_line_range(ed, line, col, col, true);
        ed.bm().insert(pos, s);
        last = pos + s.chars().count();
    }
    ed.bm().push_mark(p, false);
    ed.goto(last);
}

fn string_rectangle(ed: &mut Editor) {
    if rect(ed).is_none() {
        return;
    }
    ed.read_string(
        "String rectangle: ",
        PromptKind::StringRectangle,
        "",
        Completion::None,
        "string-rectangle",
    );
}

pub fn string_rectangle_apply(ed: &mut Editor, s: &str) {
    extract_rect(ed, false, Some(s));
    ed.bm().mark_active = false;
}

fn rectangle_number_lines(ed: &mut Editor) {
    let Some((l0, l1, c0, _)) = rect(ed) else {
        return;
    };
    if !ed.check_writable() {
        return;
    }
    let width = format!("{}", l1 - l0 + 1).len();
    for (i, l) in (l0..=l1).enumerate() {
        let (s, _) = rect_line_range(ed, l, c0, c0, true);
        ed.bm().insert(s, &format!("{:>width$} ", i + 1));
    }
    ed.bm().mark_active = false;
}

fn read_register(ed: &mut Editor, op: RegOp, prompt: &str) {
    ed.message = Some(prompt.to_string());
    ed.read_key = Some(ReadKey::Register(op));
    ed.keep_arg = true;
}

fn copy_to_register(ed: &mut Editor) {
    read_register(ed, RegOp::CopyTo, "Copy to register: ");
}
fn insert_register(ed: &mut Editor) {
    read_register(ed, RegOp::Insert, "Insert register: ");
}
fn point_to_register(ed: &mut Editor) {
    read_register(ed, RegOp::PointTo, "Point to register: ");
}
fn jump_to_register(ed: &mut Editor) {
    read_register(ed, RegOp::Jump, "Jump to register: ");
}

pub fn register_op(ed: &mut Editor, op: RegOp, c: char) {
    match op {
        RegOp::CopyTo => {
            let Some((a, b)) = region(ed) else { return };
            let s = ed.b().slice(a, b);
            ed.registers.insert(c, Register::Text(s));
            ed.bm().mark_active = false;
        }
        RegOp::Insert => match ed.registers.get(&c).cloned() {
            Some(Register::Text(s)) => {
                if ed.check_writable() {
                    let p = ed.pt();
                    ed.insert(&s);
                    if !ed.arg_universal {
                        let e = ed.pt();
                        ed.goto(p);
                        ed.bm().push_mark(e, false);
                    }
                }
            }
            Some(Register::Pos(..)) => ed.error("Register does not contain text"),
            None => ed.error("Register does not contain text"),
        },
        RegOp::PointTo => {
            let p = ed.pt();
            let bid = ed.cur();
            let m = ed.bm().new_marker(p, false);
            if let Some(Register::Pos(ob, om)) = ed.registers.insert(c, Register::Pos(bid, m)) {
                if let Some(b) = ed.bufs.get_mut(ob).and_then(|b| b.as_mut()) {
                    b.free_marker(om);
                }
            }
        }
        RegOp::Jump => match ed.registers.get(&c).cloned() {
            Some(Register::Pos(bid, m))
                if ed.bufs.get(bid).map(|b| b.is_some()).unwrap_or(false) =>
            {
                ed.switch_to(bid);
                let p = ed.buf(bid).mpos(m);
                ed.goto(p);
            }
            Some(Register::Text(_)) => ed.error("Register doesn't contain a buffer position"),
            _ => ed.error("Register doesn't contain a buffer position or configuration"),
        },
    }
}

// ======================= windows =======================

fn split_window_below(ed: &mut Editor) {
    if ed.win(ed.sel).h < 4 {
        ed.error("Window #<window> too small for splitting");
        return;
    }
    ed.split_window(false);
}

fn split_window_right(ed: &mut Editor) {
    if ed.win(ed.sel).w < 8 {
        ed.error("Window #<window> too small for splitting");
        return;
    }
    ed.split_window(true);
}

fn delete_window(ed: &mut Editor) {
    let s = ed.sel;
    if !ed.delete_window(s) {
        ed.error("Attempt to delete minibuffer or sole ordinary window");
    }
}

fn delete_other_windows(ed: &mut Editor) {
    let s = if ed.sel == MINI {
        ed.windows()[0]
    } else {
        ed.sel
    };
    for w in ed.windows() {
        if w != s {
            ed.delete_window(w);
        }
    }
    ed.sel = s;
}

fn other_window(ed: &mut Editor) {
    let n = ed.count();
    cycle_window(ed, n);
}

fn other_window_backward(ed: &mut Editor) {
    let n = ed.count();
    cycle_window(ed, -n);
}

fn cycle_window(ed: &mut Editor, n: i64) {
    let mut wins = ed.windows();
    if ed.mini.is_some() {
        wins.push(MINI);
    }
    if wins.len() < 2 {
        return;
    }
    let i = wins.iter().position(|&w| w == ed.sel).unwrap_or(0) as i64;
    let len = wins.len() as i64;
    ed.sel = wins[(((i + n) % len + len) % len) as usize];
}

fn enlarge_window(ed: &mut Editor) {
    let n = ed.count() as f32;
    let h = ed.height.max(1) as f32;
    let s = ed.sel;
    if !ed.layout.resize(s, false, n / h) {
        ed.error("No resizable window");
    }
    ed.relayout();
}

fn shrink_window(ed: &mut Editor) {
    ed.arg = Some(-ed.count());
    enlarge_window(ed);
}

fn enlarge_window_h(ed: &mut Editor) {
    let n = ed.count() as f32;
    let w = ed.width.max(1) as f32;
    let s = ed.sel;
    if !ed.layout.resize(s, true, n / w) {
        ed.error("No resizable window");
    }
    ed.relayout();
}

fn shrink_window_h(ed: &mut Editor) {
    ed.arg = Some(-ed.count());
    enlarge_window_h(ed);
}

fn balance_windows(ed: &mut Editor) {
    ed.layout.balance();
    ed.relayout();
}

fn kill_buffer_and_window(ed: &mut Editor) {
    let b = ed.cur();
    let w = ed.sel;
    ed.kill_buffer(b);
    ed.delete_window(w);
}

// ======================= misc =======================

fn keyboard_quit(ed: &mut Editor) {
    ed.bm().mark_active = false;
    ed.completions = None;
    ed.reset_arg();
    ed.msg("Quit");
    ed.ding();
    ed.errored = false;
}

fn keyboard_escape_quit(ed: &mut Editor) {
    if ed.mini.is_some() {
        abort_minibuffer(ed);
    } else if ed.b().mark_active {
        ed.bm().mark_active = false;
    } else if ed.windows().len() > 1 {
        delete_other_windows(ed);
    }
}

pub fn abort_minibuffer(ed: &mut Editor) {
    ed.mini_exit();
    ed.msg("Quit");
    ed.ding();
    ed.errored = false;
}

fn execute_extended_command(ed: &mut Editor) {
    let prompt = match ed.arg {
        Some(n) if ed.arg_universal && !ed.arg_digits && n == 4 => "C-u M-x ".to_string(),
        Some(n) => format!("{n} M-x "),
        None => "M-x ".to_string(),
    };
    let arg = (ed.arg, ed.arg_universal, ed.arg_digits);
    ed.read_string(
        &prompt,
        PromptKind::MX,
        "",
        Completion::Command,
        "extended-command",
    );
    MX_ARG.with(|m| *m.borrow_mut() = arg);
}

thread_local! {
    pub static MX_ARG: std::cell::RefCell<(Option<i64>, bool, bool)> = const { std::cell::RefCell::new((None, false, false)) };
}

fn universal_argument(ed: &mut Editor) {
    if ed.arg_universal && !ed.arg_digits {
        ed.arg = Some(ed.arg.unwrap_or(4) * 4);
    } else {
        ed.arg = Some(4);
        ed.arg_digits = false;
    }
    ed.arg_universal = true;
    ed.keep_arg = true;
    ed.prefix_time = Some(std::time::Instant::now());
    let n = ed.arg.unwrap();
    let mut s = String::from("C-u");
    let mut k = n;
    while k > 4 {
        s.push_str(" C-u");
        k /= 4;
    }
    ed.message = Some(format!("{s}-"));
}

fn negative_argument(ed: &mut Editor) {
    ed.arg = Some(match ed.arg {
        Some(n) if ed.arg_digits => -n,
        _ => -1,
    });
    ed.arg_universal = true;
    ed.keep_arg = true;
}

fn kmacro_start(ed: &mut Editor) {
    if ed.macro_rec.is_some() {
        ed.error("Already defining keyboard macro");
        return;
    }
    if ed.executing_macro {
        return;
    }
    ed.macro_rec = Some(Vec::new());
    ed.macro_counter = 0;
    ed.msg("Defining kbd macro...");
}

fn kmacro_end(ed: &mut Editor) {
    let Some(mut keys) = ed.macro_rec.take() else {
        if !ed.executing_macro {
            ed.error("Not defining kbd macro");
        }
        return;
    };
    // Drop the keys that invoked this command.
    let n = match ed.this_cmd.as_str() {
        "kmacro-end-or-call-macro" => 1,
        _ => 2,
    };
    keys.truncate(keys.len().saturating_sub(n));
    ed.last_macro = Some(keys);
    ed.msg("Keyboard macro defined");
}

fn call_macro(ed: &mut Editor) {
    let Some(keys) = ed.last_macro.clone() else {
        ed.error("No kbd macro has been defined");
        return;
    };
    let n = ed.count();
    ed.reset_arg();
    let mut i = 0;
    loop {
        if n > 0 && i >= n {
            break;
        }
        ed.execute_keys(&keys);
        if ed.errored {
            break;
        }
        i += 1;
        if n <= 0 && i > 100_000 {
            break;
        }
    }
}

fn kmacro_end_and_call(ed: &mut Editor) {
    if ed.macro_rec.is_some() {
        kmacro_end(ed);
        return;
    }
    call_macro(ed);
    ed.repeat_key = Some((
        crate::key::Key::new(crate::key::K::Char('e')),
        "kmacro-end-and-call-macro".into(),
    ));
}

fn kmacro_start_or_counter(ed: &mut Editor) {
    if ed.macro_rec.is_some() || ed.executing_macro {
        let c = ed.macro_counter;
        ed.insert(&c.to_string());
        ed.macro_counter += 1;
    } else {
        kmacro_start(ed);
    }
}

fn kmacro_end_or_call(ed: &mut Editor) {
    if ed.macro_rec.is_some() {
        kmacro_end(ed);
    } else {
        call_macro(ed);
    }
}

fn narrow_to_region(ed: &mut Editor) {
    let Some((a, b)) = region(ed) else { return };
    let bf = ed.bm();
    if let Some((x, y)) = bf.narrow.take() {
        bf.free_marker(x);
        bf.free_marker(y);
    }
    let ma = bf.new_marker(a, false);
    let mb = bf.new_marker(b, true);
    bf.narrow = Some((ma, mb));
    bf.mark_active = false;
    let p = ed.pt();
    ed.goto(p);
}

fn widen(ed: &mut Editor) {
    let bf = ed.bm();
    if let Some((x, y)) = bf.narrow.take() {
        bf.free_marker(x);
        bf.free_marker(y);
    }
}

fn toggle_truncate_lines(ed: &mut Editor) {
    ed.cfg.truncate_lines = !ed.cfg.truncate_lines;
    let on = ed.cfg.truncate_lines;
    ed.msg(format!(
        "Truncate long lines {}",
        if on { "enabled" } else { "disabled" }
    ));
}

fn describe_key(ed: &mut Editor) {
    ed.message = Some("Describe the following key, mouse click, or menu item: ".into());
    ed.read_key = Some(ReadKey::DescribeKey(Vec::new(), false));
}

fn describe_key_briefly(ed: &mut Editor) {
    ed.message = Some("Describe the following key, mouse click, or menu item: ".into());
    ed.read_key = Some(ReadKey::DescribeKey(Vec::new(), true));
}

fn describe_function(ed: &mut Editor) {
    ed.read_string(
        "Describe function: ",
        PromptKind::DescribeFunction,
        "",
        Completion::Command,
        "extended-command",
    );
}

fn where_is(ed: &mut Editor) {
    ed.read_string(
        "Where is command: ",
        PromptKind::WhereIs,
        "",
        Completion::Command,
        "extended-command",
    );
}

pub fn where_is_msg(ed: &mut Editor, cmd: &str) {
    let keys: Vec<String> = ed
        .global
        .flatten()
        .into_iter()
        .filter(|(_, c)| c == cmd)
        .map(|(k, _)| crate::key::seq_to_string(&k))
        .collect();
    if keys.is_empty() {
        ed.msg(format!("{cmd} is not on any key"));
    } else {
        ed.msg(format!("{cmd} is on {}", keys.join(", ")));
    }
}

fn help_for_help(ed: &mut Editor) {
    ed.msg("C-h k: describe key  C-h c: key briefly  C-h b: bindings  C-h f: function  C-h w: where-is  C-h e: messages  C-h m: mode");
}

fn menu_bar_open(ed: &mut Editor) {
    ed.msg("No menu bar in tmacs; use M-x");
}

fn flyspell_auto_correct(ed: &mut Editor) {
    ed.msg("Spell checking (flyspell) is not available in tmacs yet");
}

fn eval_unsupported(ed: &mut Editor) {
    ed.error("Emacs Lisp evaluation is not available in tmacs");
}

fn redraw_display(ed: &mut Editor) {
    ed.redraw_all = true;
}

fn set_variable(ed: &mut Editor) {
    ed.msg("Edit ~/.config/tmacs/init.toml to change settings");
}
