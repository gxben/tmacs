//! Syntax highlighting via syntect grammars, mapped onto Emacs font-lock faces.
//!
//! Parse state is checkpointed every `CHECKPOINT` lines and highlighted spans
//! are cached per line; edits invalidate from the first changed line.

use crate::face::{Faces, Style};
use ropey::Rope;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use syntect::highlighting::{
    Color, HighlightIterator, HighlightState, Highlighter, ScopeSelectors, StyleModifier, Theme,
    ThemeItem, ThemeSettings,
};
use syntect::parsing::{ParseState, ScopeStack, SyntaxSet};

const CHECKPOINT: usize = 32;
const MAX_LINE_LEN: usize = 10_000;
/// Lines of context parsed before the window when approximating.
const APPROX_CONTEXT: usize = 200;

/// Face ids encoded in theme colors (`r` channel).
pub const F_DEFAULT: u8 = 0;
const FACE_NAMES: &[&str] = &[
    "default",
    "font-lock-comment-face",
    "font-lock-doc-face",
    "font-lock-string-face",
    "font-lock-keyword-face",
    "font-lock-builtin-face",
    "font-lock-function-name-face",
    "font-lock-variable-name-face",
    "font-lock-type-face",
    "font-lock-constant-face",
    "font-lock-preprocessor-face",
    "font-lock-warning-face",
    "bold",
    "italic",
    "underline",
    "header",
    "link",
    "diff-added",
    "diff-removed",
];

/// (scope selector, face name). syntect picks the most specific match.
const SCOPE_FACES: &[(&str, &str)] = &[
    ("comment", "font-lock-comment-face"),
    ("punctuation.definition.comment", "font-lock-comment-face"),
    ("comment.block.documentation", "font-lock-doc-face"),
    ("string.quoted.docstring", "font-lock-doc-face"),
    ("comment.line.documentation", "font-lock-comment-face"),
    ("string", "font-lock-string-face"),
    ("punctuation.definition.string", "font-lock-string-face"),
    ("constant.character", "font-lock-string-face"),
    ("string.regexp", "font-lock-string-face"),
    ("keyword", "font-lock-keyword-face"),
    ("keyword.operator", "default"),
    ("keyword.operator.word", "font-lock-keyword-face"),
    ("keyword.operator.logical.python", "font-lock-keyword-face"),
    ("keyword.operator.new", "font-lock-keyword-face"),
    ("storage.modifier", "font-lock-keyword-face"),
    ("storage.type", "font-lock-type-face"),
    ("storage.type.function", "font-lock-keyword-face"),
    ("storage.type.class", "font-lock-keyword-face"),
    ("storage.type.struct", "font-lock-keyword-face"),
    ("storage.type.enum", "font-lock-keyword-face"),
    ("storage.type.trait", "font-lock-keyword-face"),
    ("storage.type.impl", "font-lock-keyword-face"),
    ("storage.type.interface", "font-lock-keyword-face"),
    ("storage.type.namespace", "font-lock-keyword-face"),
    ("storage.type.module", "font-lock-keyword-face"),
    ("storage.type.type", "font-lock-keyword-face"),
    ("storage.type.keyword", "font-lock-keyword-face"),
    ("storage.type.rust", "font-lock-keyword-face"),
    ("storage.type.terraform", "font-lock-keyword-face"),
    ("storage.type.js", "font-lock-keyword-face"),
    ("storage.type.ts", "font-lock-keyword-face"),
    ("storage.type.variable", "font-lock-keyword-face"),
    ("storage.type.function.arrow", "default"),
    (
        "meta.class.identifier storage.type",
        "font-lock-keyword-face",
    ),
    (
        "meta.interface.identifier storage.type",
        "font-lock-keyword-face",
    ),
    (
        "meta.enum.identifier storage.type",
        "font-lock-keyword-face",
    ),
    ("support.type", "font-lock-type-face"),
    ("support.class", "font-lock-type-face"),
    ("entity.name.type", "font-lock-type-face"),
    ("entity.name.class", "font-lock-type-face"),
    ("entity.name.struct", "font-lock-type-face"),
    ("entity.name.enum", "font-lock-type-face"),
    ("entity.name.interface", "font-lock-type-face"),
    ("entity.name.trait", "font-lock-type-face"),
    ("entity.name.impl", "font-lock-type-face"),
    ("entity.other.inherited-class", "font-lock-type-face"),
    ("entity.name.function", "font-lock-function-name-face"),
    ("entity.name.method", "font-lock-function-name-face"),
    ("entity.name.macro", "font-lock-function-name-face"),
    ("support.function", "font-lock-builtin-face"),
    ("support.function.builtin", "font-lock-builtin-face"),
    ("variable.function", "default"),
    ("variable.parameter", "font-lock-variable-name-face"),
    ("entity.name.variable", "font-lock-variable-name-face"),
    ("entity.name.constant", "font-lock-constant-face"),
    (
        "variable.other.constant.declaration",
        "font-lock-variable-name-face",
    ),
    ("constant.language", "font-lock-constant-face"),
    ("constant.other", "font-lock-constant-face"),
    ("support.constant", "font-lock-constant-face"),
    ("variable.language", "font-lock-keyword-face"),
    ("constant.numeric", "default"),
    ("meta.preprocessor", "font-lock-preprocessor-face"),
    ("meta.preprocessor keyword", "font-lock-preprocessor-face"),
    (
        "keyword.control.import.include",
        "font-lock-preprocessor-face",
    ),
    ("meta.preprocessor string", "font-lock-string-face"),
    ("meta.annotation", "font-lock-preprocessor-face"),
    ("meta.attribute", "font-lock-preprocessor-face"),
    ("entity.name.tag", "font-lock-function-name-face"),
    ("entity.name.tag.yaml", "font-lock-variable-name-face"),
    (
        "entity.other.attribute-name",
        "font-lock-variable-name-face",
    ),
    ("entity.name.label", "font-lock-constant-face"),
    ("markup.heading", "header"),
    ("markup.bold", "bold"),
    ("markup.italic", "italic"),
    ("markup.underline", "underline"),
    ("markup.underline.link", "link"),
    ("markup.inserted", "diff-added"),
    ("markup.deleted", "diff-removed"),
    ("meta.diff.range", "font-lock-constant-face"),
    ("meta.diff.header", "header"),
    ("message.error", "font-lock-warning-face"),
    ("invalid.deprecated", "font-lock-warning-face"),
];

pub struct Ctx {
    pub ss: SyntaxSet,
    pub hl: Highlighter<'static>,
}

static THEME: OnceLock<Theme> = OnceLock::new();
static CTX: OnceLock<Ctx> = OnceLock::new();

fn face_color(name: &str) -> Color {
    let i = FACE_NAMES.iter().position(|n| *n == name).unwrap_or(0);
    Color {
        r: i as u8,
        g: 0,
        b: 0,
        a: 255,
    }
}

fn theme() -> &'static Theme {
    THEME.get_or_init(|| {
        let scopes = SCOPE_FACES
            .iter()
            .map(|(sel, face)| ThemeItem {
                scope: ScopeSelectors::from_str(sel).expect("selector"),
                style: StyleModifier {
                    foreground: Some(face_color(face)),
                    background: None,
                    font_style: None,
                },
            })
            .collect();
        Theme {
            name: Some("emacs-font-lock".into()),
            author: None,
            settings: ThemeSettings {
                foreground: Some(face_color("default")),
                ..Default::default()
            },
            scopes,
        }
    })
}

pub fn ctx() -> &'static Ctx {
    CTX.get_or_init(|| Ctx {
        ss: two_face::syntax::extra_newlines(),
        hl: Highlighter::new(theme()),
    })
}

/// Preload grammars in a background thread so the first file opens fast.
pub fn preload() {
    std::thread::spawn(|| {
        let _ = ctx();
    });
}

/// Resolve a face id to a style using the user's faces.
pub fn face_style(faces: &Faces, id: u8) -> Style {
    let name = FACE_NAMES.get(id as usize).copied().unwrap_or("default");
    if name == "default" {
        Style::default()
    } else {
        faces.get(name).unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
    pub face: u8,
}

pub type Spans = Rc<Vec<Span>>;

#[derive(Clone)]
struct State {
    parse: ParseState,
    hl: HighlightState,
}

pub struct Cache {
    /// states[k] = state at start of line k*CHECKPOINT
    states: Vec<State>,
    valid: usize,
    spans: Vec<Option<Spans>>,
    nlines: usize,
    failed: bool,
    /// Line for which an exact parse state is wanted (background target).
    want: usize,
}

impl Cache {
    pub fn new(syntax: &str) -> Option<Cache> {
        let c = ctx();
        let syn = c.ss.find_syntax_by_name(syntax)?;
        let st = State {
            parse: ParseState::new(syn),
            hl: HighlightState::new(&c.hl, ScopeStack::new()),
        };
        Some(Cache {
            states: vec![st],
            valid: 1,
            spans: Vec::new(),
            nlines: 0,
            failed: false,
            want: 0,
        })
    }

    /// Invalidate from `line` (first modified line).
    pub fn invalidate(&mut self, line: usize, nlines: usize) {
        if line == usize::MAX {
            return;
        }
        let k = line / CHECKPOINT + 1;
        if nlines != self.nlines {
            self.states.truncate(k.max(1));
        }
        self.valid = self.valid.min(k).max(1).min(self.states.len());
        self.spans.truncate(line);
        self.nlines = nlines;
        self.failed = false;
    }

    fn line_str(text: &Rope, line: usize) -> String {
        let mut s = text.line(line).to_string();
        if !s.ends_with('\n') {
            s.push('\n');
        }
        s
    }

    fn parse_line(st: &mut State, line: &str, want: bool) -> Option<Vec<Span>> {
        let c = ctx();
        if line.len() > MAX_LINE_LEN {
            return Some(Vec::new());
        }
        let ops = st.parse.parse_line(line, &c.ss).ok()?;
        if !want {
            // Still need to advance highlight state.
            for _ in HighlightIterator::new(&mut st.hl, &ops, line, &c.hl) {}
            return Some(Vec::new());
        }
        let mut out = Vec::new();
        let mut pos = 0u32;
        for (style, piece) in HighlightIterator::new(&mut st.hl, &ops, line, &c.hl) {
            let n = piece.chars().count() as u32;
            let face = style.foreground.r;
            if face != F_DEFAULT && n > 0 {
                match out.last_mut() {
                    Some(Span { end, face: f, .. }) if *end == pos && *f == face => *end += n,
                    _ => out.push(Span {
                        start: pos,
                        end: pos + n,
                        face,
                    }),
                }
            }
            pos += n;
        }
        Some(out)
    }

    /// Advance exact checkpoints towards `self.want` until `deadline`.
    /// Returns true when the checkpoint needed for `want` is available.
    pub fn advance(&mut self, text: &Rope, deadline: Instant) -> bool {
        let nlines = text.len_lines();
        if self.nlines != nlines {
            self.invalidate(0, nlines);
        }
        let k_target = self.want.min(nlines.saturating_sub(1)) / CHECKPOINT;
        while self.valid <= k_target {
            if self.failed {
                return true;
            }
            let k = self.valid - 1;
            let mut st = self.states[k].clone();
            for l in k * CHECKPOINT..((k + 1) * CHECKPOINT).min(nlines) {
                if Self::parse_line(&mut st, &Self::line_str(text, l), false).is_none() {
                    self.failed = true;
                    return true;
                }
            }
            self.store_checkpoint(k + 1, st);
            if Instant::now() >= deadline && self.valid <= k_target {
                return false;
            }
        }
        true
    }

    /// Exact fontification still needs background work.
    pub fn pending(&self) -> bool {
        !self.failed && self.valid <= self.want.min(self.nlines.saturating_sub(1)) / CHECKPOINT
    }

    /// Highlight spans for lines `first..=last`. If the exact parse state is
    /// too far away to compute within `budget`, an approximation (parsing
    /// from a little before `first`) is returned and the exact state is
    /// computed later via `advance` (like jit-lock's stealth fontification).
    pub fn spans(
        &mut self,
        text: &Rope,
        first: usize,
        last: usize,
        budget: Duration,
    ) -> Vec<Spans> {
        let nlines = text.len_lines();
        let last = last.min(nlines.saturating_sub(1));
        if self.nlines != nlines {
            self.invalidate(0, nlines);
        }
        let empty = || vec![Rc::new(Vec::new()); last.saturating_sub(first) + 1];
        if self.failed || first > last {
            return empty();
        }
        if (first..=last).all(|l| matches!(self.spans.get(l), Some(Some(_)))) {
            return (first..=last)
                .map(|l| self.spans[l].clone().unwrap())
                .collect();
        }
        self.want = first;
        let exact = self.advance(text, Instant::now() + budget);
        if self.failed {
            return empty();
        }
        let k_target = first / CHECKPOINT;
        let (start, mut st) = if exact {
            let k = k_target.min(self.valid - 1);
            (k * CHECKPOINT, self.states[k].clone())
        } else {
            let from = first.saturating_sub(APPROX_CONTEXT);
            let k = from / CHECKPOINT;
            if k < self.valid {
                (k * CHECKPOINT, self.states[k].clone())
            } else {
                (from, self.states[0].clone())
            }
        };
        let mut out = Vec::with_capacity(last - first + 1);
        for l in start..=last {
            if exact && l % CHECKPOINT == 0 && l / CHECKPOINT >= self.valid {
                self.store_checkpoint(l / CHECKPOINT, st.clone());
            }
            let want = l >= first;
            let Some(sp) = Self::parse_line(&mut st, &Self::line_str(text, l), want) else {
                self.failed = true;
                return empty();
            };
            if want {
                let sp = Rc::new(sp);
                if exact {
                    if self.spans.len() <= l {
                        self.spans.resize(l + 1, None);
                    }
                    self.spans[l] = Some(sp.clone());
                }
                out.push(sp);
            }
        }
        out
    }

    fn store_checkpoint(&mut self, k: usize, st: State) {
        if k < self.states.len() {
            if k >= self.valid {
                let same = self.states[k].parse == st.parse && self.states[k].hl == st.hl;
                self.states[k] = st;
                self.valid = if same { self.states.len() } else { k + 1 };
            } else {
                self.states[k] = st;
            }
        } else {
            self.states.push(st);
            self.valid = self.states.len();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn go_keywords_and_strings() {
        let mut c = Cache::new("Go").unwrap();
        let text = Rope::from_str("package main\n\n// hi\nfunc main() { x := \"s\" }\n");
        let sp = c.spans(&text, 0, 3, Duration::from_secs(5));
        let faces = |l: usize| {
            sp[l]
                .iter()
                .map(|s| FACE_NAMES[s.face as usize])
                .collect::<Vec<_>>()
        };
        assert!(
            faces(0).contains(&"font-lock-keyword-face"),
            "{:?}",
            faces(0)
        );
        assert_eq!(faces(2), vec!["font-lock-comment-face"]);
        assert!(faces(3).contains(&"font-lock-string-face"));
        assert!(
            faces(3).contains(&"font-lock-function-name-face"),
            "{:?}",
            faces(3)
        );
    }

    #[test]
    fn invalidation_multiline_comment() {
        let mut c = Cache::new("C").unwrap();
        let mut text = Rope::from_str("int a;\nint b;\n");
        let _ = c.spans(&text, 0, 1, Duration::from_secs(5));
        text.insert(0, "/*\n");
        c.invalidate(0, text.len_lines());
        let sp = c.spans(&text, 1, 2, Duration::from_secs(5));
        assert!(
            sp[0]
                .iter()
                .all(|s| FACE_NAMES[s.face as usize] == "font-lock-comment-face")
        );
    }

    #[test]
    fn far_lines_checkpoints() {
        let mut c = Cache::new("Rust").unwrap();
        let src: String = (0..500).map(|i| format!("let x{i} = \"{i}\";\n")).collect();
        let text = Rope::from_str(&src);
        let sp = c.spans(&text, 400, 401, Duration::from_secs(5));
        assert!(!sp[0].is_empty());
        assert!(c.states.len() > 10);
    }

    #[test]
    fn approximate_then_exact() {
        let mut c = Cache::new("C").unwrap();
        let src: String = std::iter::once("/*\n".to_string())
            .chain((0..5000).map(|i| format!("int x{i};\n")))
            .collect();
        let text = Rope::from_str(&src);
        // Zero budget: approximation (comment opener is out of context).
        let sp = c.spans(&text, 4000, 4000, Duration::ZERO);
        assert!(c.pending());
        assert!(
            sp[0]
                .iter()
                .all(|s| FACE_NAMES[s.face as usize] != "font-lock-comment-face")
        );
        while !c.advance(&text, Instant::now() + Duration::from_millis(50)) {}
        assert!(!c.pending());
        let sp = c.spans(&text, 4000, 4000, Duration::ZERO);
        assert!(
            sp[0]
                .iter()
                .all(|s| FACE_NAMES[s.face as usize] == "font-lock-comment-face")
        );
    }
}

#[cfg(test)]
mod bench {
    use super::*;
    #[test]
    #[ignore]
    fn parse_speed() {
        let src = std::fs::read_to_string(std::env::var("BENCH_FILE").unwrap()).unwrap();
        let text = Rope::from_str(&src);
        let mut c = Cache::new("Go").unwrap();
        let t = std::time::Instant::now();
        let n = text.len_lines() - 2;
        let _ = c.spans(&text, n, n, Duration::from_secs(60));
        eprintln!("parsed {} lines in {:?}", n, t.elapsed());
    }
}

#[cfg(test)]
mod languages {
    use super::*;

    fn faces_of(syntax: &str, src: &str) -> Vec<(String, &'static str)> {
        let mut c = Cache::new(syntax).unwrap_or_else(|| panic!("no syntax {syntax}"));
        let text = Rope::from_str(src);
        let n = text.len_lines() - 1;
        let sp = c.spans(&text, 0, n, Duration::from_secs(5));
        let mut out = Vec::new();
        for (l, spans) in sp.iter().enumerate() {
            let line: Vec<char> = text.line(l).chars().collect();
            for s in spans.iter() {
                let word: String = line[s.start as usize..s.end as usize].iter().collect();
                out.push((word.trim().to_string(), FACE_NAMES[s.face as usize]));
            }
        }
        out
    }

    fn has(f: &[(String, &str)], word: &str, face: &str) -> bool {
        f.iter().any(|(w, fc)| w.contains(word) && *fc == face)
    }

    #[test]
    fn major_languages() {
        let cases: &[(&str, &str, &[(&str, &str)])] = &[
            (
                "Python",
                "# c\ndef foo(x):\n    return \"s\"\n",
                &[
                    ("# c", "font-lock-comment-face"),
                    ("def", "font-lock-keyword-face"),
                    ("foo", "font-lock-function-name-face"),
                    ("\"s\"", "font-lock-string-face"),
                ],
            ),
            (
                "Rust",
                "// c\nfn main() { let s = \"x\"; }\n",
                &[
                    ("// c", "font-lock-comment-face"),
                    ("fn", "font-lock-keyword-face"),
                    ("main", "font-lock-function-name-face"),
                    ("let", "font-lock-keyword-face"),
                ],
            ),
            (
                "TypeScript",
                "// c\nfunction f(a: number): string { return 'x'; }\n",
                &[
                    ("function", "font-lock-keyword-face"),
                    ("f", "font-lock-function-name-face"),
                    ("number", "font-lock-type-face"),
                    ("'x'", "font-lock-string-face"),
                ],
            ),
            (
                "C",
                "#include <stdio.h>\nint main(void) { return 0; }\n",
                &[
                    ("#include", "font-lock-preprocessor-face"),
                    ("int", "font-lock-type-face"),
                    ("main", "font-lock-function-name-face"),
                    ("return", "font-lock-keyword-face"),
                ],
            ),
            (
                "YAML",
                "# c\nkey: \"v\"\n",
                &[
                    ("# c", "font-lock-comment-face"),
                    ("key", "font-lock-variable-name-face"),
                    ("\"v\"", "font-lock-string-face"),
                ],
            ),
            (
                "Terraform",
                "resource \"a\" \"b\" {\n  x = \"y\" # c\n}\n",
                &[
                    ("resource", "font-lock-keyword-face"),
                    ("\"y\"", "font-lock-string-face"),
                    ("# c", "font-lock-comment-face"),
                ],
            ),
            (
                "Bourne Again Shell (bash)",
                "# c\nif [ -f x ]; then echo \"s\"; fi\n",
                &[
                    ("# c", "font-lock-comment-face"),
                    ("if", "font-lock-keyword-face"),
                    ("\"s\"", "font-lock-string-face"),
                ],
            ),
            (
                "JSON",
                "{\"k\": \"v\", \"n\": true}\n",
                &[
                    ("\"v\"", "font-lock-string-face"),
                    ("true", "font-lock-constant-face"),
                ],
            ),
            (
                "Markdown",
                "# Title\n*it* **b**\n",
                &[("Title", "header"), ("b", "bold")],
            ),
            (
                "Dockerfile",
                "FROM alpine\n# c\nRUN echo hi\n",
                &[
                    ("FROM", "font-lock-keyword-face"),
                    ("# c", "font-lock-comment-face"),
                ],
            ),
            (
                "Java",
                "class A { void f() { String s = \"x\"; } }\n",
                &[
                    ("class", "font-lock-keyword-face"),
                    ("A", "font-lock-type-face"),
                    ("\"x\"", "font-lock-string-face"),
                ],
            ),
            (
                "Lisp",
                ";; c\n(defun foo () \"d\")\n",
                &[
                    (";; c", "font-lock-comment-face"),
                    ("defun", "font-lock-keyword-face"),
                ],
            ),
        ];
        let mut failures = Vec::new();
        for (syn, src, expects) in cases {
            let f = faces_of(syn, src);
            for (w, face) in *expects {
                if !has(&f, w, face) {
                    failures.push(format!("{syn}: {w:?} not {face}; got {f:?}"));
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
#[cfg(test)]
mod dump {
    #[test]
    #[ignore]
    fn dump_scopes() {
        use syntect::parsing::{ParseState, ScopeStack};
        let c = super::ctx();
        let syn =
            c.ss.find_syntax_by_name(&std::env::var("SYN").unwrap())
                .unwrap();
        let mut ps = ParseState::new(syn);
        let line = std::env::var("SRC").unwrap() + "\n";
        let ops = ps.parse_line(&line, &c.ss).unwrap();
        let mut st = ScopeStack::new();
        let mut last = 0;
        for (i, op) in ops {
            if i > last {
                eprintln!(
                    "{:?} => {:?}",
                    &line[last..i],
                    st.as_slice()
                        .iter()
                        .map(|s| s.build_string())
                        .collect::<Vec<_>>()
                );
            }
            last = i;
            st.apply(&op).unwrap();
        }
    }
}
