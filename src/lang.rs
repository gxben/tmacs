//! Major modes: detection, indentation rules, comment syntax, formatters.

use crate::buffer::Buffer;
use regex::Regex;
use serde::Deserialize;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum IndentStyle {
    /// Copy previous line indentation (text modes).
    #[default]
    Relative,
    /// C-family braces/brackets.
    Braces,
    /// Python-style: `:` opens a block, TAB cycles.
    Python,
    /// YAML: `:` / `- ` open blocks, TAB cycles.
    Yaml,
    /// Lisp: align with first argument.
    Lisp,
    /// Shell: then/do/fi/done keywords plus braces.
    Shell,
}

#[derive(Clone, Debug)]
pub struct Lang {
    pub name: String,
    pub mode_name: String,
    pub syntax: Option<String>,
    pub indent: usize,
    pub tabs: bool,
    pub tab_width: usize,
    pub comment_start: String,
    pub comment_end: String,
    pub style: IndentStyle,
    pub formatter: Option<Vec<String>>,
    pub format_on_save: bool,
    /// File names (regexps) never formatted on save, e.g. templated YAML.
    pub format_exclude: Vec<Regex>,
    pub prog: bool,
    pub require_final_newline: bool,
}

impl Lang {
    fn new(name: &str, mode_name: &str, syntax: Option<&str>) -> Lang {
        Lang {
            name: name.to_string(),
            mode_name: mode_name.to_string(),
            syntax: syntax.map(|s| s.to_string()),
            indent: 4,
            tabs: false,
            tab_width: 8,
            comment_start: String::new(),
            comment_end: String::new(),
            style: IndentStyle::Relative,
            formatter: None,
            format_on_save: false,
            format_exclude: Vec::new(),
            prog: true,
            require_final_newline: true,
        }
    }
}

/// Config overrides for a language (`[[language]]` in init.toml).
#[derive(Clone, Debug, Deserialize, Default)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct LangConfig {
    pub name: String,
    pub mode_name: Option<String>,
    pub syntax: Option<String>,
    pub patterns: Option<Vec<String>>,
    pub indent: Option<usize>,
    pub tabs: Option<bool>,
    pub tab_width: Option<usize>,
    pub comment_start: Option<String>,
    pub comment_end: Option<String>,
    pub indent_style: Option<IndentStyle>,
    pub formatter: Option<Vec<String>>,
    pub format_on_save: Option<bool>,
    pub format_exclude: Option<Vec<String>>,
    pub require_final_newline: Option<bool>,
}

pub struct Langs {
    pub list: Vec<Lang>,
    /// (regex on file name, lang index), checked in order (later config first).
    pub patterns: Vec<(Regex, usize)>,
}

/// name, mode-name, syntect syntax, indent, tabs, comment, style
type Def = (
    &'static str,
    &'static str,
    &'static str,
    usize,
    bool,
    &'static str,
    IndentStyle,
);

const DEFS: &[Def] = {
    use IndentStyle::*;
    &[
        ("c", "C/*l", "C", 2, false, "/* */", Braces),
        ("c++", "C++//l", "C++", 4, false, "//", Braces),
        ("objc", "ObjC", "Objective-C", 4, false, "//", Braces),
        ("go", "Go", "Go", 8, true, "//", Braces),
        ("gomod", "Go.mod", "Gomod", 8, true, "//", Braces),
        ("rust", "Rust", "Rust", 4, false, "//", Braces),
        ("java", "Java", "Java", 4, false, "//", Braces),
        ("kotlin", "Kotlin", "Kotlin", 4, false, "//", Braces),
        ("scala", "Scala", "Scala", 2, false, "//", Braces),
        ("swift", "Swift", "Swift", 4, false, "//", Braces),
        ("csharp", "C#", "C#", 4, false, "//", Braces),
        (
            "javascript",
            "JavaScript",
            "JavaScript (Babel)",
            4,
            false,
            "//",
            Braces,
        ),
        (
            "typescript",
            "TypeScript",
            "TypeScript",
            4,
            false,
            "//",
            Braces,
        ),
        ("tsx", "TSX", "TypeScriptReact", 4, false, "//", Braces),
        ("json", "JSON", "JSON", 4, false, "//", Braces),
        ("css", "CSS", "CSS", 4, false, "/* */", Braces),
        ("scss", "SCSS", "SCSS", 4, false, "//", Braces),
        ("less", "Less", "Less", 4, false, "//", Braces),
        ("php", "PHP", "PHP", 4, false, "//", Braces),
        ("dart", "Dart", "Dart", 2, false, "//", Braces),
        ("zig", "Zig", "Zig", 4, false, "//", Braces),
        (
            "proto",
            "Protobuf",
            "Protocol Buffer",
            2,
            false,
            "//",
            Braces,
        ),
        ("groovy", "Groovy", "Groovy", 4, false, "//", Braces),
        ("graphql", "GraphQL", "GraphQL", 2, false, "#", Braces),
        ("terraform", "Terraform", "Terraform", 2, false, "#", Braces),
        ("nix", "Nix", "Nix", 2, false, "#", Braces),
        ("jsonnet", "Jsonnet", "jsonnet", 2, false, "//", Braces),
        ("rego", "Rego", "Rego", 4, true, "#", Braces),
        ("python", "Python", "Python", 4, false, "#", Python),
        ("yaml", "YAML", "YAML", 2, false, "#", Yaml),
        ("toml", "Conf[TOML]", "TOML", 2, false, "#", Relative),
        ("ini", "Conf[INI]", "INI", 4, false, ";", Relative),
        (
            "sh",
            "Shell-script[bash]",
            "Bourne Again Shell (bash)",
            4,
            false,
            "#",
            Shell,
        ),
        ("fish", "Fish", "Fish", 4, false, "#", Shell),
        ("makefile", "Makefile", "Makefile", 8, true, "#", Relative),
        ("cmake", "CMake", "CMake", 2, false, "#", Braces),
        (
            "dockerfile",
            "Dockerfile",
            "Dockerfile",
            4,
            false,
            "#",
            Relative,
        ),
        ("ruby", "Ruby", "Ruby", 2, false, "#", Relative),
        ("perl", "Perl", "Perl", 4, false, "#", Braces),
        ("lua", "Lua", "Lua", 3, false, "--", Relative),
        ("sql", "SQL", "SQL", 4, false, "--", Relative),
        ("haskell", "Haskell", "Haskell", 2, false, "--", Relative),
        ("elisp", "ELisp", "Lisp", 2, false, ";", Lisp),
        ("clojure", "Clojure", "Clojure", 2, false, ";", Lisp),
        ("racket", "Racket", "Racket", 2, false, ";", Lisp),
        ("erlang", "Erlang", "Erlang", 4, false, "%", Relative),
        ("elixir", "Elixir", "Elixir", 2, false, "#", Relative),
        ("ocaml", "Tuareg", "OCaml", 2, false, "(* *)", Relative),
        ("r", "ESS[R]", "R", 2, false, "#", Braces),
        ("julia", "Julia", "Julia", 4, false, "#", Relative),
        ("html", "HTML", "HTML", 2, false, "<!-- -->", Relative),
        ("xml", "nXML", "XML", 2, false, "<!-- -->", Relative),
        (
            "vue",
            "Vue",
            "Vue Component",
            2,
            false,
            "<!-- -->",
            Relative,
        ),
        ("svelte", "Svelte", "Svelte", 2, false, "<!-- -->", Relative),
        (
            "markdown", "Markdown", "Markdown", 4, false, "<!-- -->", Relative,
        ),
        ("latex", "LaTeX", "LaTeX", 2, false, "%", Relative),
        ("diff", "Diff", "Diff", 4, false, "#", Relative),
        (
            "git-commit",
            "Git-Commit",
            "Git Commit",
            4,
            false,
            "#",
            Relative,
        ),
        (
            "powershell",
            "PowerShell",
            "PowerShell",
            4,
            false,
            "#",
            Braces,
        ),
        ("vim", "VimL", "VimL", 2, false, "\"", Relative),
        ("awk", "AWK", "AWK", 4, false, "#", Braces),
        ("nginx", "Nginx", "nginx", 4, false, "#", Braces),
        ("jinja", "Jinja2", "Jinja2", 2, false, "{# #}", Relative),
        ("csv", "CSV", "Separated Values", 4, false, "#", Relative),
        ("asm", "Asm", "x86_64 Assembly", 8, true, ";", Relative),
        ("text", "Text", "Plain Text", 4, false, "", Relative),
    ]
};

/// Default auto-mode patterns (Emacs auto-mode-alist style regexes on file names).
const PATTERNS: &[(&str, &str)] = &[
    // From ~/.emacs.cli.d/init.el
    (r"\.ya?ml$", "yaml"),
    (r"\.ya?ml\.j2$", "yaml"),
    (r"\.ya?ml\.gotmpl$", "yaml"),
    (r"\.tf$", "terraform"),
    (r"\.tfvars$", "terraform"),
    (r"\.hcl$", "terraform"),
    (r"\.ts$", "typescript"),
    (r"\.go$", "go"),
    (r"\.md$", "markdown"),
    // Other common ones syntect gets wrong or ambiguous.
    (r"\.h$", "c"),
    (r"\.(cc|cpp|cxx|hh|hpp|hxx)$", "c++"),
    (r"\.el$", "elisp"),
    (r"(^|/)\.emacs$", "elisp"),
    (r"\.(mjs|cjs|jsx|js)$", "javascript"),
    (r"\.tsx$", "tsx"),
    (r"\.(zsh|bash|sh)$", "sh"),
    (
        r"(^|/)\.(bashrc|zshrc|profile|bash_profile|zprofile)$",
        "sh",
    ),
    (r"(^|/)(Dockerfile|Containerfile)(\..*)?$", "dockerfile"),
    (r"(^|/)(GNUmakefile|[Mm]akefile)$", "makefile"),
    (r"\.mk$", "makefile"),
    (r"(^|/)go\.(mod|work)$", "gomod"),
    (r"(^|/)Jenkinsfile$", "groovy"),
    (r"\.txt$", "text"),
];

/// Formatter defaults, similar to format-all. `{file}` is the buffer file path.
pub fn default_formatter(name: &str) -> Option<(Vec<&'static str>, bool)> {
    // (command, format_on_save by default)
    Some(match name {
        "go" => (vec!["gofmt"], true),
        "terraform" => (vec!["terraform", "fmt", "-"], true),
        // format-all style: prettier for the web/data languages
        "typescript" | "tsx" | "javascript" | "css" | "scss" | "less" | "html" | "vue"
        | "svelte" | "graphql" | "json" | "markdown" | "yaml" => {
            (vec!["prettier", "--stdin-filepath", "{file}"], true)
        }
        "rust" => (
            vec![
                "rustfmt",
                "--edition",
                "2024",
                "--emit",
                "stdout",
                "--quiet",
            ],
            true,
        ),
        "python" => (vec!["black", "-q", "-"], true),
        "c" | "c++" | "objc" => (vec!["clang-format", "--assume-filename={file}"], true),
        "java" | "proto" | "csharp" => (vec!["clang-format", "--assume-filename={file}"], false),
        "sh" => (vec!["shfmt", "-filename", "{file}"], true),
        "lua" => (vec!["stylua", "-"], false),
        "nix" => (vec!["nixfmt"], false),
        "zig" => (vec!["zig", "fmt", "--stdin"], false),
        "toml" => (vec!["taplo", "fmt", "-"], false),
        "dart" => (vec!["dart", "format"], false),
        "elixir" => (vec!["mix", "format", "-"], false),
        "swift" => (vec!["swiftformat"], false),
        "jsonnet" => (vec!["jsonnetfmt", "-"], false),
        "rego" => (vec!["opa", "fmt"], false),
        _ => return None,
    })
}

impl Langs {
    #[allow(clippy::regex_creation_in_loops)]
    pub fn new() -> Langs {
        let mut list = vec![Lang {
            prog: false,
            require_final_newline: false,
            ..Lang::new("fundamental", "Fundamental", None)
        }];
        for &(name, mode, syn, indent, tabs, comment, style) in DEFS {
            let mut l = Lang::new(name, mode, Some(syn));
            l.indent = indent;
            l.tabs = tabs;
            l.style = style;
            let mut parts = comment.split(' ');
            l.comment_start = parts.next().unwrap_or("").to_string();
            l.comment_end = parts.next().unwrap_or("").to_string();
            if matches!(name, "text" | "markdown" | "git-commit" | "csv" | "diff") {
                l.prog = false;
            }
            if let Some((cmd, on)) = default_formatter(name) {
                l.formatter = Some(cmd.into_iter().map(String::from).collect());
                l.format_on_save = on;
            }
            if name == "yaml" {
                // Helm/Ansible templates are not valid YAML for prettier.
                l.format_exclude = vec![Regex::new(r"\.(gotmpl|j2|tpl)$").unwrap()];
            }
            list.push(l);
        }
        let mut langs = Langs {
            list,
            patterns: Vec::new(),
        };
        for (re, name) in PATTERNS {
            if let Some(i) = langs.find(name) {
                langs.patterns.push((Regex::new(re).unwrap(), i));
            }
        }
        langs
    }

    pub fn find(&self, name: &str) -> Option<usize> {
        self.list.iter().position(|l| l.name == name)
    }

    pub fn find_by_syntax(&self, syntax: &str) -> Option<usize> {
        self.list
            .iter()
            .position(|l| l.syntax.as_deref() == Some(syntax))
    }

    /// Apply `[[language]]` config entries.
    pub fn apply_config(&mut self, cfgs: &[LangConfig]) -> Vec<String> {
        let mut errors = Vec::new();
        let mut new_patterns = Vec::new();
        for c in cfgs {
            let i = match self.find(&c.name) {
                Some(i) => i,
                None => {
                    let syn = c.syntax.clone();
                    self.list.push(Lang::new(&c.name, &c.name, syn.as_deref()));
                    self.list.len() - 1
                }
            };
            let l = &mut self.list[i];
            if let Some(v) = &c.mode_name {
                l.mode_name = v.clone();
            }
            if let Some(v) = &c.syntax {
                l.syntax = Some(v.clone());
            }
            if let Some(v) = c.indent {
                l.indent = v;
            }
            if let Some(v) = c.tabs {
                l.tabs = v;
            }
            if let Some(v) = c.tab_width {
                l.tab_width = v;
            }
            if let Some(v) = &c.comment_start {
                l.comment_start = v.clone();
            }
            if let Some(v) = &c.comment_end {
                l.comment_end = v.clone();
            }
            if let Some(v) = c.indent_style {
                l.style = v;
            }
            if let Some(v) = &c.formatter {
                l.formatter = if v.is_empty() { None } else { Some(v.clone()) };
            }
            if let Some(v) = c.format_on_save {
                l.format_on_save = v;
            }
            if let Some(v) = c.require_final_newline {
                l.require_final_newline = v;
            }
            if let Some(v) = &c.format_exclude {
                l.format_exclude.clear();
                for p in v {
                    match Regex::new(&p.replace("\\'", "$")) {
                        Ok(r) => l.format_exclude.push(r),
                        Err(e) => errors.push(format!("bad format-exclude {p}: {e}")),
                    }
                }
            }
            for p in c.patterns.iter().flatten() {
                // Accept Emacs regex flavour: \\' end anchor, \\. literal dot.
                let re = p.replace("\\'", "$").replace("\\`", "^");
                match Regex::new(&re) {
                    Ok(r) => new_patterns.push((r, i)),
                    Err(e) => errors.push(format!("bad pattern {p}: {e}")),
                }
            }
        }
        new_patterns.append(&mut self.patterns);
        self.patterns = new_patterns;
        errors
    }

    /// Detect language for a file name and first line.
    pub fn detect(
        &mut self,
        file_name: &str,
        first_line: &str,
        syntaxes: &syntect::parsing::SyntaxSet,
    ) -> usize {
        for (re, i) in &self.patterns {
            if re.is_match(file_name) {
                return *i;
            }
        }
        // Emacs -*- mode: x -*- cookie
        if let Some(m) = first_line.find("-*-") {
            let rest = &first_line[m + 3..];
            if let Some(e) = rest.find("-*-") {
                let cookie = rest[..e].trim();
                let mode = cookie
                    .split(';')
                    .find_map(|kv| kv.trim().strip_prefix("mode:").map(|s| s.trim()))
                    .unwrap_or(if cookie.contains(':') { "" } else { cookie });
                let mode = mode.trim_end_matches("-mode").to_lowercase();
                if let Some(i) = self.find(&mode) {
                    return i;
                }
            }
        }
        let base = std::path::Path::new(file_name)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(file_name);
        let ext = base.rsplit_once('.').map(|(_, e)| e).unwrap_or(base);
        let syn = syntaxes
            .find_syntax_by_extension(base)
            .or_else(|| syntaxes.find_syntax_by_extension(ext))
            .or_else(|| syntaxes.find_syntax_by_first_line(first_line));
        match syn {
            Some(s) if s.name != "Plain Text" => match self.find_by_syntax(&s.name) {
                Some(i) => i,
                None => {
                    let mut l = Lang::new(&s.name.to_lowercase(), &s.name, Some(&s.name));
                    l.comment_start = "#".into();
                    self.list.push(l);
                    self.list.len() - 1
                }
            },
            _ => 0,
        }
    }
}

// ---------------- indentation ----------------

pub fn indentation_of(line: &str, tab_width: usize) -> usize {
    let mut col = 0;
    for c in line.chars() {
        match c {
            ' ' => col += 1,
            '\t' => col = (col / tab_width + 1) * tab_width,
            _ => break,
        }
    }
    col
}

/// Remove string literals and line comments, for bracket counting.
fn strip_code(line: &str, comment: &str) -> String {
    let mut out = String::new();
    let mut chars = line.chars().peekable();
    let mut quote: Option<char> = None;
    let cstart: Vec<char> = comment.chars().collect();
    let all: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < all.len() {
        let c = all[i];
        if let Some(q) = quote {
            if c == '\\' {
                i += 2;
                continue;
            }
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if c == '"' || c == '\'' || c == '`' {
            // Lone apostrophes in Rust lifetimes / Lisp quotes: only treat as
            // string if closed later on the line.
            if all[i + 1..].contains(&c) {
                quote = Some(c);
                i += 1;
                continue;
            }
        }
        if !cstart.is_empty() && all[i..].starts_with(&cstart) {
            break;
        }
        if c == '/' && all.get(i + 1) == Some(&'/') {
            break;
        }
        if c == '/' && all.get(i + 1) == Some(&'*') {
            // skip block comment on this line
            if let Some(end) =
                (i + 2..all.len().saturating_sub(1)).find(|&j| all[j] == '*' && all[j + 1] == '/')
            {
                i = end + 2;
                continue;
            }
            break;
        }
        out.push(c);
        i += 1;
    }
    let _ = chars.peek();
    out
}

fn net_open(code: &str) -> i32 {
    // Count unmatched openers, ignoring leading closers (handled separately).
    let mut depth = 0i32;
    let mut min = 0i32;
    for c in code.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                min = min.min(depth);
            }
            _ => {}
        }
    }
    // Leading closers already dedented this line; don't double count.
    let leading = code
        .trim_start()
        .chars()
        .take_while(|c| matches!(c, ')' | ']' | '}'))
        .count() as i32;
    depth + leading.min(-min)
}

fn prev_nonblank(b: &Buffer, line: usize) -> Option<usize> {
    let mut l = line;
    while l > 0 {
        l -= 1;
        if !b.line_text(l).trim().is_empty() {
            return Some(l);
        }
    }
    None
}

fn is_braceless_control(code: &str) -> bool {
    let t = code.trim();
    if t.ends_with('{') || t.ends_with(';') || t.ends_with('}') {
        return false;
    }
    let starts = [
        "if ", "if(", "else", "for ", "for(", "while ", "while(", "} else",
    ];
    starts.iter().any(|s| t.starts_with(s))
        && (t.ends_with(')') || t == "else" || t.ends_with("else"))
}

fn is_case_label(t: &str) -> bool {
    (t.starts_with("case ") || t.starts_with("default:") || t.starts_with("default :"))
        && t.ends_with(':')
}

/// Desired indentation column for `line`, or None to leave as-is.
pub fn compute_indent(b: &Buffer, lang: &Lang, line: usize) -> Option<usize> {
    let tw = lang.tab_width;
    let ind = lang.indent;
    let cur = b.line_text(line);
    let cur_t = cur.trim_start();
    let Some(pl) = prev_nonblank(b, line) else {
        return Some(0);
    };
    let prev = b.line_text(pl);
    let prev_ind = indentation_of(&prev, tw);
    let prev_code = strip_code(&prev, &lang.comment_start);
    let prev_t = prev_code.trim();
    match lang.style {
        IndentStyle::Relative => Some(prev_ind),
        IndentStyle::Braces => {
            let mut col = prev_ind as i64;
            let open = net_open(&prev_code);
            if open > 0 {
                col += ind as i64;
            } else if open < 0 {
                // Closed a multi-line paren group: align with the line that opened it.
                if let Some(ol) = find_opener_line(b, lang, pl) {
                    col = indentation_of(&b.line_text(ol), tw) as i64;
                }
            }
            if is_braceless_control(prev_t) && !cur_t.starts_with('{') {
                col += ind as i64;
            } else if let Some(pp) = prev_nonblank(b, pl) {
                let pp_code = strip_code(&b.line_text(pp), &lang.comment_start);
                if is_braceless_control(pp_code.trim()) && open == 0 && !prev_t.starts_with('{') {
                    col = indentation_of(&b.line_text(pp), tw) as i64;
                }
            }
            if is_case_label(prev_t) && open == 0 {
                col += ind as i64;
            }
            if is_case_label(cur_t.trim_end()) {
                col -= ind as i64;
            }
            let lead_close = cur_t
                .chars()
                .take_while(|c| matches!(c, ')' | ']' | '}'))
                .count() as i64;
            if lead_close > 0 {
                col -= ind as i64;
            }
            Some(col.max(0) as usize)
        }
        IndentStyle::Python => {
            let mut col = prev_ind as i64;
            if prev_t.ends_with(':') || net_open(&prev_code) > 0 {
                col += ind as i64;
            } else if ["return", "pass", "break", "continue", "raise"]
                .iter()
                .any(|k| prev_t == *k || prev_t.starts_with(&format!("{k} ")))
            {
                col -= ind as i64;
            }
            if ["else:", "elif ", "except", "finally:"]
                .iter()
                .any(|k| cur_t.starts_with(k))
            {
                col -= ind as i64;
            }
            if cur_t.starts_with(')') || cur_t.starts_with(']') || cur_t.starts_with('}') {
                col -= ind as i64;
            }
            Some(col.max(0) as usize)
        }
        IndentStyle::Yaml => {
            let mut col = prev_ind as i64;
            if prev_t.ends_with(':')
                || prev_t.ends_with('|')
                || prev_t.ends_with('>')
                || prev_t.ends_with("|-")
            {
                col += ind as i64;
            } else if let Some(rest) = prev_t.strip_prefix("- ") {
                // list item: continue at the item's content column
                let _ = rest;
                col = prev_ind as i64 + 2;
            }
            if prev_t.starts_with("- ") && prev_t.ends_with(':') {
                col = prev_ind as i64 + 2 + ind as i64;
            }
            Some(col.max(0) as usize)
        }
        IndentStyle::Shell => {
            let mut col = prev_ind as i64;
            let opens = ["then", "do", "else", "{", "(", "in"];
            if opens.iter().any(|k| prev_t.ends_with(k))
                || prev_t.ends_with(")") && !prev_t.contains("case") && is_case_pattern(prev_t)
            {
                col += ind as i64;
            }
            if prev_t.ends_with(";;") {
                col -= ind as i64;
            }
            let closes = ["fi", "done", "esac", "}", "else", "elif", ")"];
            if closes.iter().any(|k| {
                cur_t == *k
                    || cur_t.starts_with(&format!("{k} "))
                    || cur_t.starts_with(&format!("{k};"))
            }) {
                col -= ind as i64;
            }
            Some(col.max(0) as usize)
        }
        IndentStyle::Lisp => Some(lisp_indent(b, line, ind).unwrap_or(0)),
    }
}

fn is_case_pattern(t: &str) -> bool {
    !t.starts_with('(') && t.ends_with(')') && !t.contains('$') && !t.contains(' ')
}

fn find_opener_line(b: &Buffer, lang: &Lang, line: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut l = line as i64;
    let mut first = true;
    while l >= 0 && (line as i64 - l) < 200 {
        let code = strip_code(&b.line_text(l as usize), &lang.comment_start);
        let chars: Vec<char> = code.chars().collect();
        for &c in chars.iter().rev() {
            match c {
                ')' | ']' | '}' => depth += 1,
                '(' | '[' | '{' => {
                    depth -= 1;
                }
                _ => {}
            }
        }
        if !first && depth <= 0 {
            return Some(l as usize);
        }
        if first && depth <= 0 {
            return None;
        }
        first = false;
        l -= 1;
    }
    None
}

const LISP_SPECIAL: &[&str] = &[
    "defun",
    "defmacro",
    "defvar",
    "defcustom",
    "defconst",
    "let",
    "let*",
    "lambda",
    "when",
    "unless",
    "while",
    "dolist",
    "dotimes",
    "progn",
    "save-excursion",
    "with-current-buffer",
    "condition-case",
    "defn",
    "fn",
    "def",
    "use-package",
    "add-hook",
    "define",
    "if-let",
    "when-let",
    "cl-defun",
    "defface",
    "defgroup",
];

fn lisp_indent(b: &Buffer, line: usize, ind: usize) -> Option<usize> {
    let start = b.line_start(line);
    // find innermost unclosed '('
    let mut depth = 0i32;
    let mut p = start;
    let limit = start.saturating_sub(100_000);
    while p > limit {
        p -= 1;
        let c = b.text.char(p);
        match c {
            ')' | ']' => depth += 1,
            '(' | '[' => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
        if p == limit {
            return Some(0);
        }
    }
    if p == 0 && b.char_at(0) != Some('(') {
        return Some(0);
    }
    let open_col = b.column(p, 8);
    let line_end = b.eol(p);
    let after: String = b.slice(p + 1, line_end);
    let mut words = after.split_whitespace();
    let head = words.next().unwrap_or("");
    if LISP_SPECIAL.contains(&head) || head.starts_with("def") {
        return Some(open_col + ind);
    }
    if head.is_empty() || head.starts_with('(') {
        return Some(open_col + 1);
    }
    // align with first argument if on the same line
    if let Some(off) = after.find(head).map(|i| i + head.len()) {
        let rest = &after[off..];
        if let Some(ws) = rest.find(|c: char| !c.is_whitespace()) {
            let arg_col = open_col + 1 + after[..off].chars().count() + rest[..ws].chars().count();
            return Some(arg_col);
        }
    }
    Some(open_col + 1)
}

/// Build indentation whitespace for a column.
pub fn indent_string(col: usize, tabs: bool, tab_width: usize) -> String {
    if tabs && tab_width > 0 {
        let mut s = "\t".repeat(col / tab_width);
        s.push_str(&" ".repeat(col % tab_width));
        s
    } else {
        " ".repeat(col)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn buf(s: &str) -> Buffer {
        let mut b = Buffer::new("t", PathBuf::from("/"));
        b.insert(0, s);
        b
    }

    fn lang(name: &str) -> Lang {
        let l = Langs::new();
        l.list[l.find(name).unwrap()].clone()
    }

    #[test]
    fn c_braces() {
        let l = lang("c");
        let b = buf("int main() {\nfoo();\n}\n");
        assert_eq!(compute_indent(&b, &l, 1), Some(2));
        assert_eq!(compute_indent(&b, &l, 2), Some(0));
    }

    #[test]
    fn c_braceless_if() {
        let l = lang("c++");
        let b = buf("void f() {\n    if (x)\n        y();\n    z();\n}\n");
        assert_eq!(compute_indent(&b, &l, 2), Some(8));
        assert_eq!(compute_indent(&b, &l, 3), Some(4));
    }

    #[test]
    fn substatement_open_zero() {
        let l = lang("c");
        let b = buf("  if (x)\n  {\n");
        assert_eq!(compute_indent(&b, &l, 1), Some(2));
    }

    #[test]
    fn python_colon() {
        let l = lang("python");
        let b = buf("def f():\n    return 1\nx\n");
        assert_eq!(compute_indent(&b, &l, 1), Some(4));
        assert_eq!(compute_indent(&b, &l, 2), Some(0));
    }

    #[test]
    fn yaml_nesting() {
        let l = lang("yaml");
        let b = buf("a:\n  b: 1\n  c:\n- x\n");
        assert_eq!(compute_indent(&b, &l, 1), Some(2));
        assert_eq!(compute_indent(&b, &l, 3), Some(4));
    }

    #[test]
    fn multiline_call_close() {
        let l = lang("go");
        let b = buf("\tfoo(a,\n\t\tb)\n\tbar()\n");
        assert_eq!(compute_indent(&b, &l, 2), Some(8));
    }

    #[test]
    fn detect_patterns() {
        let mut l = Langs::new();
        let ss = syntect::parsing::SyntaxSet::load_defaults_newlines();
        let i = l.detect("values.yaml.gotmpl", "", &ss);
        assert_eq!(l.list[i].name, "yaml");
        let i = l.detect("main.tf", "", &ss);
        assert_eq!(l.list[i].name, "terraform");
        let i = l.detect("script", "#!/bin/bash", &ss);
        assert_eq!(l.list[i].name, "sh");
    }
}
