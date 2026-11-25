use std::collections::HashMap;

use crate::ast::{Align, Block, Env, Inline, Target};
use crate::diag::{did_you_mean, Diagnostics};
use crate::inline::{is_ident, parse_inline, take_label, Ctx, Span};
use crate::math::{self, MathEnv};

pub const LANGUAGE_VERSION: u32 = 1;

pub const ENV_KINDS: &[&str] = &[
    "theorem", "lemma", "corollary", "proposition", "definition", "example", "remark", "proof", "note", "figure", "quote",
];

pub const DOC_STYLES: &[&str] = &["quiet", "classic"];

const NUMBERED: &[&str] = &["theorem", "lemma", "corollary", "proposition", "definition", "example", "remark"];

#[derive(Debug, Clone)]
pub struct Settings {
    pub page: String,
    pub margin: String,
    pub text_size: String,
    pub font: Option<String>,
    pub numbering: bool,
    pub style: String,
    pub accent: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { page: "a4".into(), margin: "2.5cm".into(), text_size: "11pt".into(), font: None, numbering: true, style: "quiet".into(), accent: None }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Meta {
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub date: Option<String>,
    pub version: Option<u32>,
}

pub struct Doc {
    pub meta: Meta,
    pub settings: Settings,
    pub blocks: Vec<Block>,
    pub env: MathEnv,
    pub labels: HashMap<String, Target>,
}

type Line<'a> = (usize, &'a str);

pub fn parse(src: &str, diags: &mut Diagnostics) -> Doc {
    let lines: Vec<Line> = src.lines().enumerate().map(|(i, l)| (i + 1, l)).collect();
    let mut p = DocParser {
        diags,
        meta: Meta::default(),
        settings: Settings::default(),
        env: MathEnv::new(),
        vars: HashMap::new(),
        labels: HashMap::new(),
        counters: Counters::default(),
    };
    let body_start = p.front_matter(&lines);
    let blocks = p.blocks(&lines[body_start..], 0);
    if p.meta.version.is_none() {
        p.diags.warning(
            1,
            1,
            "This file does not state its language version.",
            Some("Add `functor: 1` to the front matter, between two `---` lines.".into()),
        );
    }
    Doc { meta: p.meta, settings: p.settings, blocks, env: p.env, labels: p.labels }
}

#[derive(Default)]
struct Counters {
    headings: Vec<usize>,
    theorem: usize,
    figure: usize,
    equation: usize,
}

struct DocParser<'d> {
    diags: &'d mut Diagnostics,
    meta: Meta,
    settings: Settings,
    env: MathEnv,
    vars: HashMap<String, String>,
    labels: HashMap<String, Target>,
    counters: Counters,
}

fn indent_col(line: &str) -> usize {
    line.chars().take_while(|c| c.is_whitespace()).count() + 1
}

impl DocParser<'_> {
    fn front_matter(&mut self, lines: &[Line]) -> usize {
        if lines.first().map(|(_, l)| l.trim()) != Some("---") {
            return 0;
        }
        let Some(end) = lines.iter().skip(1).position(|(_, l)| l.trim() == "---") else {
            self.diags.error(1, 1, "The front matter has no closing `---`.", Some("Add a line with `---` after the last setting.".into()));
            return lines.len();
        };
        for &(n, line) in &lines[1..=end] {
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            let Some((key, value)) = t.split_once(':') else {
                self.diags.error(n, 1, format!("`{t}` is not a setting."), Some("Write `key: value`, for example `title: My notes`.".into()));
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            let col = line.find(value).map_or(1, |i| line[..i].chars().count() + 1);
            match key {
                "functor" => match value.parse::<u32>() {
                    Ok(v) if v == LANGUAGE_VERSION => self.meta.version = Some(v),
                    Ok(v) => {
                        self.meta.version = Some(v);
                        self.diags.error(n, col, format!("This compiler knows Functor {LANGUAGE_VERSION}, but the file asks for Functor {v}."), Some(format!("Use `functor: {LANGUAGE_VERSION}`, or update the compiler.")));
                    }
                    Err(_) => self.diags.error(n, col, "The language version must be a number.", Some(format!("Write `functor: {LANGUAGE_VERSION}`."))),
                },
                "title" => self.meta.title = Some(unquote(value)),
                "author" | "authors" => self.meta.authors = parse_list(value),
                "date" => self.meta.date = Some(unquote(value)),
                _ => {
                    let hint = did_you_mean(key, ["functor", "title", "author", "authors", "date"]).map(|s| format!("Did you mean `{s}`?"));
                    self.diags.warning(n, 1, format!("Unknown setting `{key}`. The compiler ignores it."), hint);
                }
            }
        }
        end + 2
    }

    fn inline(&mut self, span: &Span) -> Vec<Inline> {
        let mut ctx = Ctx { vars: &self.vars, diags: self.diags };
        parse_inline(span, &mut ctx)
    }

    fn inline_line(&mut self, text: &str, line: usize, col: usize) -> Vec<Inline> {
        self.inline(&Span::single(text, line, col))
    }

    fn declaration(&mut self, n: usize, line: &str) -> bool {
        let t = line.trim_start();
        let col = indent_col(line);
        let word: String = t.chars().take_while(|c| c.is_alphabetic()).collect();
        let rest = t[word.len()..].trim_start();
        let looks = match word.as_str() {
            "object" | "sym" | "let" | "use" | "kind" | "style" | "default" => t[word.len()..].starts_with(' ') && !rest.is_empty(),
            "set" => rest.contains('(') && is_ident(rest.split('(').next().unwrap_or("").trim()),
            _ => false,
        };
        if !looks {
            return false;
        }
        let rest_col = col + (t.len() - rest.len());
        match word.as_str() {
            "object" => self.decl_object(n, rest, rest_col),
            "sym" => self.decl_sym(n, rest, rest_col),
            "let" => self.decl_let(n, rest, rest_col),
            "set" => self.decl_set(n, rest, rest_col),
            _ => self.diags.error(
                n,
                col,
                format!("`{word}` comes in phase 1."),
                Some("Phase 0 has `object`, `set`, `sym` and `let`.".into()),
            ),
        }
        true
    }

    fn decl_object(&mut self, n: usize, rest: &str, col: usize) {
        let Some((kind, names)) = rest.split_once(':') else {
            self.diags.error(n, col, "An object declaration needs a kind and names.", Some("Write `object vec: v, u, F`.".into()));
            return;
        };
        let kind = kind.trim();
        if kind != "vec" {
            let hint = did_you_mean(kind, ["vec"]).map(|s| format!("Did you mean `{s}`?")).unwrap_or_else(|| "Phase 0 has the kind `vec`. Own kinds come in phase 1.".into());
            self.diags.error(n, col, format!("Unknown kind `{kind}`."), Some(hint));
            return;
        }
        for name in names.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            if !is_ident(name) {
                self.diags.error(n, col, format!("`{name}` is not a valid name."), Some("Names use letters, digits and `_`.".into()));
                continue;
            }
            self.env.objects.insert(name.to_string(), kind.to_string());
        }
    }

    fn decl_sym(&mut self, n: usize, rest: &str, col: usize) {
        let Some((name, recipe)) = rest.split_once('=') else {
            self.diags.error(n, col, "A `sym` needs a name and a recipe.", Some("Write `sym csum = sum.circle`.".into()));
            return;
        };
        let (name, recipe) = (name.trim(), recipe.trim());
        if !is_ident(name) || name.contains('-') {
            self.diags.error(n, col, format!("`{name}` is not a valid symbol name."), Some("Use letters and digits, for example `csum`.".into()));
            return;
        }
        if crate::symbols::all_names().contains(&name) {
            self.diags.warning(n, col, format!("`{name}` hides the built-in symbol with the same name."), Some("Select another name.".into()));
        }
        let recipe_col = col + rest.find(recipe).map_or(0, |i| rest[..i].chars().count());
        let mut probe = Diagnostics::default();
        {
            let pos = |o: usize| (n, recipe_col + recipe[..o.min(recipe.len())].chars().count());
            let mut ctx = math::Ctx { env: &self.env, diags: &mut probe, pos: &pos, display: false };
            math::parse(recipe, &mut ctx);
        }
        let ok = probe.items.iter().all(|d| d.severity != crate::diag::Severity::Error);
        self.diags.items.extend(probe.items);
        let (_, ops) = math::split_recipe(recipe);
        if recipe.contains(char::is_whitespace) || ops.iter().any(|o| o.is_empty()) {
            self.diags.error(n, recipe_col, "A recipe is one base and its operations, with no spaces.", Some("For example `sum.circle` or `in.not`.".into()));
            return;
        }
        if ok {
            self.env.syms.insert(name.to_string(), recipe.to_string());
        }
    }

    fn decl_let(&mut self, n: usize, rest: &str, col: usize) {
        let Some((name, value)) = rest.split_once('=') else {
            self.diags.error(n, col, "A `let` needs a name and a value.", Some("Write `let n = 20`.".into()));
            return;
        };
        let (name, value) = (name.trim(), value.trim());
        if !is_ident(name) {
            self.diags.error(n, col, format!("`{name}` is not a valid name."), None);
            return;
        }
        let v = if value.starts_with('"') {
            if value.len() < 2 || !value.ends_with('"') {
                self.diags.error(n, col, "This text has no closing `\"`.", Some("Add `\"` at the end.".into()));
                return;
            }
            value[1..value.len() - 1].to_string()
        } else if value.parse::<f64>().is_ok() {
            value.to_string()
        } else {
            self.diags.error(n, col, format!("`{value}` is not a value."), Some("Use a number, or text in quotes: `let name = \"text\"`.".into()));
            return;
        };
        self.vars.insert(name.to_string(), v);
    }

    fn decl_set(&mut self, n: usize, rest: &str, col: usize) {
        let open = rest.find('(').unwrap_or(rest.len());
        let target = rest[..open].trim();
        let Some(close) = rest.rfind(')') else {
            self.diags.error(n, col, "This `set` has no closing `)`.", Some("Add `)` at the end of the line.".into()));
            return;
        };
        let args = &rest[open + 1..close];
        let allowed = crate::symbols::SET_OPTIONS;
        let Some((_, keys)) = allowed.iter().find(|(t, _)| *t == target) else {
            let hint = did_you_mean(target, allowed.iter().map(|(t, _)| *t)).map(|s| format!("Did you mean `{s}`?"));
            self.diags.error(n, col, format!("`set {target}` is not known."), hint.or(Some(format!("Targets: {}.", allowed.iter().map(|(t, _)| *t).collect::<Vec<_>>().join(", ")))));
            return;
        };
        for arg in args.split(',').map(str::trim).filter(|a| !a.is_empty()) {
            let arg_col = col + rest.find(arg).map_or(0, |i| rest[..i].chars().count());
            let Some((k, v)) = arg.split_once(':') else {
                self.diags.error(n, arg_col, format!("`{arg}` needs a name and a value."), Some("Write `name: value`.".into()));
                continue;
            };
            let (k, v) = (k.trim(), unquote(v.trim()));
            if !keys.contains(&k) {
                let hint = did_you_mean(k, keys.iter().copied()).map(|s| format!("Did you mean `{s}`?"));
                self.diags.error(n, arg_col, format!("`{target}` has no option `{k}`."), hint.or(Some(format!("Options: {}.", keys.join(", ")))));
                continue;
            }
            match (target, k) {
                ("vec", "style") => {
                    if v == "arrow" || v == "bold" {
                        self.env.vec_style = v;
                    } else if v == "column" {
                        self.diags.error(n, arg_col, "`column` is not a style of a named vector.", Some("For vectors with components, write `set vec(body: column)`.".into()));
                    } else {
                        self.diags.error(n, arg_col, format!("`{v}` is not a vector style."), Some("Use `arrow` or `bold`.".into()));
                    }
                }
                (_, "fence") => {
                    let (open, close) = math::fence_pair(&v, &self.env);
                    if v != "none" && open.is_none() && close.is_none() {
                        self.diags.error(n, arg_col, format!("`{v}` is not a fence."), Some("Use a fence such as `paren`, `brack`, `angle`, `vbar.x2`, `none`, or one symbol such as `L.script`.".into()));
                    } else {
                        self.env.kinds.insert(format!("{target}.fence"), v);
                    }
                }
                (_, "grow") => {
                    if v == "stretch" || v == "magnify" {
                        self.env.kinds.insert(format!("{target}.grow"), v);
                    } else {
                        self.diags.error(n, arg_col, format!("`{v}` is not a way to grow."), Some("Use `stretch` or `magnify`.".into()));
                    }
                }
                ("vec", "body") => {
                    if v == "column" || v == "row" {
                        self.env.kinds.insert("vec.body".into(), v);
                    } else {
                        self.diags.error(n, arg_col, format!("`{v}` is not a vector body."), Some("Use `column` or `row`.".into()));
                    }
                }
                (_, "inline") => {
                    if v == "small" || v == "full" {
                        self.env.kinds.insert(format!("{target}.inline"), v);
                    } else {
                        self.diags.error(n, arg_col, format!("`{v}` is not an inline size."), Some("Use `small` or `full`.".into()));
                    }
                }
                ("diagram", "gap") => {
                    if ["small", "normal", "large"].contains(&v.as_str()) {
                        self.env.kinds.insert("diagram.gap".into(), v);
                    } else {
                        self.diags.error(n, arg_col, format!("`{v}` is not a diagram gap."), Some("Use `small`, `normal` or `large`.".into()));
                    }
                }
                ("diagram", "cross") => {
                    if v == "gap" || v == "plain" {
                        self.env.kinds.insert("diagram.cross".into(), v);
                    } else {
                        self.diags.error(n, arg_col, format!("`{v}` is not a diagram crossing."), Some("Use `gap` or `plain`.".into()));
                    }
                }
                ("cases", "word") => {
                    self.env.kinds.insert("cases.word".into(), v);
                }
                ("document", "style") => {
                    if DOC_STYLES.contains(&v.as_str()) {
                        self.settings.style = v;
                    } else {
                        let hint = did_you_mean(&v, DOC_STYLES.iter().copied()).map(|s| format!("Did you mean `{s}`?"));
                        self.diags.error(n, arg_col, format!("`{v}` is not a document style."), hint.or(Some(format!("Use {}.", DOC_STYLES.iter().map(|s| format!("`{s}`")).collect::<Vec<_>>().join(" or ")))));
                    }
                }
                ("document", "accent") => {
                    let hex = v.strip_prefix('#').unwrap_or("");
                    if (hex.len() == 3 || hex.len() == 6) && hex.chars().all(|c| c.is_ascii_hexdigit()) {
                        self.settings.accent = Some(v);
                    } else {
                        self.diags.error(n, arg_col, format!("`{v}` is not a colour."), Some("Write a hex colour such as `#4a6a85`.".into()));
                    }
                }
                ("heading", "numbering") => self.settings.numbering = v != "none",
                ("page", "size") => {
                    if ["a4", "a5", "letter"].contains(&v.as_str()) {
                        self.settings.page = v;
                    } else {
                        self.diags.error(n, arg_col, format!("Unknown page size `{v}`."), Some("Use `a4`, `a5` or `letter`.".into()));
                    }
                }
                ("page", "margin") => self.settings.margin = self.length(n, arg_col, &v).unwrap_or(self.settings.margin.clone()),
                ("text", "size") => self.settings.text_size = self.length(n, arg_col, &v).unwrap_or(self.settings.text_size.clone()),
                ("text", "font") => self.settings.font = Some(v),
                _ => {}
            }
        }
    }

    fn length(&mut self, n: usize, col: usize, v: &str) -> Option<String> {
        let unit = v.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.');
        let num = &v[..v.len() - unit.len()];
        if num.parse::<f64>().is_ok() && ["pt", "mm", "cm", "in", "em"].contains(&unit) {
            Some(v.to_string())
        } else {
            self.diags.error(n, col, format!("`{v}` is not a length."), Some("Write a number and a unit: `pt`, `mm`, `cm`, `in` or `em`.".into()));
            None
        }
    }

    fn label_attr(&mut self, text: &str, line: usize, col: usize) -> (String, Option<String>, Vec<(String, String)>) {
        let t = text.trim_end();
        if !t.ends_with('}') {
            return (t.to_string(), None, Vec::new());
        }
        let Some(open) = t.rfind('{') else {
            return (t.to_string(), None, Vec::new());
        };
        let inner = &t[open + 1..t.len() - 1];
        if !inner.trim_start().starts_with('#') && !inner.contains('=') {
            return (t.to_string(), None, Vec::new());
        }
        let mut label = None;
        let mut attrs = Vec::new();
        for part in split_attrs(inner) {
            if let Some(l) = part.strip_prefix('#') {
                if take_label(l) != l || l.is_empty() {
                    self.diags.error(line, col + t[..open].chars().count(), format!("`#{l}` is not a valid label."), Some("A label starts with a letter, for example `{#thm:main}`.".into()));
                } else {
                    label = Some(l.to_string());
                }
            } else if let Some((k, v)) = part.split_once('=') {
                attrs.push((k.trim().to_string(), unquote(v.trim())));
            }
        }
        (t[..open].trim_end().to_string(), label, attrs)
    }

    fn add_label(&mut self, label: &Option<String>, target: Target, line: usize, col: usize) {
        if let Some(l) = label {
            if self.labels.contains_key(l) {
                self.diags.error(line, col, format!("The label `{l}` is used twice."), Some("Each label must be unique. Rename one of them.".into()));
            } else {
                self.labels.insert(l.clone(), target);
            }
        }
    }

    fn blocks(&mut self, lines: &[Line], depth: usize) -> Vec<Block> {
        let mut out = Vec::new();
        let mut para = Span::new();
        let mut i = 0;
        macro_rules! flush {
            () => {
                if !para.text.is_empty() {
                    let p = std::mem::replace(&mut para, Span::new());
                    let content = self.inline(&p);
                    out.push(Block::Para(content));
                }
            };
        }
        while i < lines.len() {
            let (n, line) = lines[i];
            let t = line.trim();
            let col = indent_col(line);
            if t.is_empty() {
                flush!();
                i += 1;
                continue;
            }
            if self.declaration(n, line) {
                flush!();
                i += 1;
                continue;
            }
            if let Some(level) = heading_level(t) {
                flush!();
                let text = t[level..].trim_start();
                let text_col = col + (t.len() - text.len());
                let (text, label, _) = self.label_attr(text, n, text_col);
                let content = self.inline_line(&text, n, text_col);
                let number = self.settings.numbering.then(|| self.next_heading(level));
                let plain = plain_text(&content);
                self.add_label(&label, Target::Heading { number: number.clone(), text: plain }, n, col);
                out.push(Block::Heading { level, content, label, number });
                i += 1;
                continue;
            }
            if t.starts_with("$$") {
                flush!();
                let (block, used) = self.display_math(&lines[i..]);
                out.push(block);
                i += used;
                continue;
            }
            if t.starts_with("```") {
                flush!();
                let lang = t[3..].trim().to_string();
                let mut text = Vec::new();
                let mut j = i + 1;
                while j < lines.len() && !lines[j].1.trim_start().starts_with("```") {
                    text.push(lines[j].1);
                    j += 1;
                }
                if j >= lines.len() {
                    self.diags.error(n, col, "This code block has no closing ```.", Some("Add a line with ``` after the code.".into()));
                }
                out.push(Block::Code { lang, text: text.join("\n") });
                i = (j + 1).min(lines.len());
                continue;
            }
            if t.starts_with(":::") {
                flush!();
                if t == ":::" {
                    self.diags.error(n, col, "This `:::` closes a block that is not open.", Some("Remove it, or open a block with `::: theorem`.".into()));
                    i += 1;
                    continue;
                }
                let (block, used) = self.env_block(&lines[i..], depth);
                if let Some(b) = block {
                    out.push(b);
                }
                i += used;
                continue;
            }
            if t == "---" || t == "***" {
                flush!();
                out.push(Block::Rule);
                i += 1;
                continue;
            }
            if list_marker(t).is_some() {
                flush!();
                let (block, used) = self.list(&lines[i..]);
                out.push(block);
                i += used;
                continue;
            }
            if t.starts_with('|') && lines.get(i + 1).is_some_and(|(_, l)| is_table_sep(l.trim())) {
                flush!();
                let (block, used) = self.table(&lines[i..]);
                out.push(block);
                i += used;
                continue;
            }
            para.push_line(t, n, col);
            i += 1;
        }
        flush!();
        out
    }

    fn next_heading(&mut self, level: usize) -> String {
        let c = &mut self.counters.headings;
        c.resize(level.max(c.len()), 0);
        c.truncate(level);
        if let Some(last) = c.last_mut() {
            *last += 1;
        }
        let first = c.iter().position(|&x| x > 0).unwrap_or(0);
        c[first..].iter().map(|x| x.to_string()).collect::<Vec<_>>().join(".")
    }

    fn display_math(&mut self, lines: &[Line]) -> (Block, usize) {
        let (n0, first) = lines[0];
        let col0 = indent_col(first);
        let after_open = &first.trim_start()[2..];
        let mut src = String::new();
        let mut used = 1;
        let mut tail = String::new();
        let mut closed = false;
        let mut current = after_open.to_string();
        let mut idx = 0;
        loop {
            if let Some(end) = current.find("$$") {
                src.push_str(&current[..end]);
                tail = current[end + 2..].to_string();
                closed = true;
                break;
            }
            src.push_str(&current);
            idx += 1;
            if idx >= lines.len() {
                break;
            }
            src.push('\n');
            current = lines[idx].1.to_string();
            used += 1;
        }
        if !closed {
            self.diags.error(n0, col0, "This display math has no closing `$$`.", Some("Add `$$` where the math ends.".into()));
        }
        let last_line = lines[used - 1].0;
        let (rest, label, _) = self.label_attr(&tail, last_line, 1);
        if !rest.trim().is_empty() {
            self.diags.warning(last_line, 1, "Text after `$$` is ignored.", Some("Put text on a new line.".into()));
        }
        let number = label.as_ref().map(|_| {
            self.counters.equation += 1;
            self.counters.equation
        });
        if let Some(num) = number {
            self.add_label(&label, Target::Equation(num), last_line, 1);
        }
        (Block::Math { src, line: n0, col: col0 + 2, label, number }, used)
    }

    fn env_block(&mut self, lines: &[Line], depth: usize) -> (Option<Block>, usize) {
        let (n, line) = lines[0];
        let col = indent_col(line);
        let head = line.trim()[3..].trim();
        let kind: String = head.chars().take_while(|c| c.is_alphanumeric() || *c == '-').collect();
        let (_, label, attrs) = self.label_attr(head, n, col);
        let mut level = 0;
        let mut end = None;
        for (j, (_, l)) in lines.iter().enumerate().skip(1) {
            let t = l.trim();
            if t == ":::" {
                if level == 0 {
                    end = Some(j);
                    break;
                }
                level -= 1;
            } else if t.starts_with(":::") {
                level += 1;
            }
        }
        let close = end.unwrap_or_else(|| {
            self.diags.error(n, col, format!("The block `{kind}` is not closed."), Some("Add a line with `:::` where the block ends.".into()));
            lines.len()
        });
        let used = (close + 1).min(lines.len());
        if !ENV_KINDS.contains(&kind.as_str()) {
            let hint = did_you_mean(&kind, ENV_KINDS.iter().copied()).map(|s| format!("Did you mean `{s}`?"));
            self.diags.error(n, col + 4, format!("Unknown block `{kind}`."), hint.or(Some(format!("Blocks: {}.", ENV_KINDS.join(", ")))));
            return (None, used);
        }
        let number = if NUMBERED.contains(&kind.as_str()) {
            self.counters.theorem += 1;
            Some(self.counters.theorem)
        } else if kind == "figure" {
            self.counters.figure += 1;
            Some(self.counters.figure)
        } else {
            None
        };
        if let Some(num) = number {
            self.add_label(&label, Target::Env { kind: kind.clone(), number: num }, n, col);
        } else if label.is_some() {
            self.diags.warning(n, col, format!("A `{kind}` block has no number, so a reference to it shows its name only."), None);
            self.add_label(&label, Target::Env { kind: kind.clone(), number: 0 }, n, col);
        }
        let mut caption = None;
        let mut body_lines: Vec<Line> = Vec::new();
        for &(bn, bl) in &lines[1..close.min(lines.len())] {
            if kind == "figure" {
                if let Some(c) = bl.trim_start().strip_prefix("Caption:") {
                    let ccol = bl.find("Caption:").map_or(1, |i| bl[..i].chars().count() + 9);
                    caption = Some(self.inline_line(c.trim(), bn, ccol + 1));
                    continue;
                }
            }
            body_lines.push((bn, bl));
        }
        let body = self.blocks(&body_lines, depth + 1);
        let get = |k: &str| attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
        for (k, _) in &attrs {
            if !["title", "width"].contains(&k.as_str()) {
                self.diags.warning(n, col, format!("Unknown option `{k}` on `{kind}`."), Some("Options: title, width.".into()));
            }
        }
        if kind == "figure" && caption.is_none() {
            self.diags.warning(n, col, "This figure has no caption.", Some("Add a line `Caption: ...` inside the figure.".into()));
        }
        (Some(Block::Env(Env { kind, label, title: get("title"), width: get("width"), number, caption, body })), used)
    }

    fn list(&mut self, lines: &[Line]) -> (Block, usize) {
        let ordered = list_marker(lines[0].1.trim()).is_some_and(|(o, _)| o);
        let mut items: Vec<Span> = Vec::new();
        let mut used = 0;
        for &(n, line) in lines {
            let t = line.trim();
            if t.is_empty() {
                break;
            }
            match list_marker(t) {
                Some((o, len)) if o == ordered => {
                    let text = t[len..].trim_start();
                    let col = indent_col(line) + (t.len() - text.len());
                    items.push(Span::single(text, n, col));
                }
                Some(_) => break,
                None if line.starts_with(' ') && !items.is_empty() => {
                    if let Some(last) = items.last_mut() {
                        last.push_line(t, n, indent_col(line));
                    }
                }
                None => break,
            }
            used += 1;
        }
        let items = items.iter().map(|s| self.inline(s)).collect();
        (Block::List { ordered, items }, used.max(1))
    }

    fn table(&mut self, lines: &[Line]) -> (Block, usize) {
        let (n0, h) = lines[0];
        let align = table_cells(lines[1].1)
            .iter()
            .map(|(c, _)| match (c.starts_with(':'), c.ends_with(':')) {
                (true, true) => Align::Center,
                (true, false) => Align::Left,
                (false, true) => Align::Right,
                _ => Align::None,
            })
            .collect();
        let header = table_cells(h).iter().map(|(c, col)| self.inline_line(c, n0, *col)).collect();
        let mut rows = Vec::new();
        let mut used = 2;
        for &(n, line) in &lines[2..] {
            if !line.trim().starts_with('|') {
                break;
            }
            let row: Vec<Vec<Inline>> = table_cells(line).iter().map(|(c, col)| self.inline_line(c, n, *col)).collect();
            rows.push(row);
            used += 1;
        }
        (Block::Table { align, header, rows }, used)
    }
}

fn table_cells(line: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' {
            i += 2;
            continue;
        }
        if c == '|' {
            if let Some(s) = start {
                let raw: String = chars[s..i].iter().collect();
                let lead = raw.chars().take_while(|c| c.is_whitespace()).count();
                out.push((raw.trim().to_string(), s + lead + 1));
            }
            start = Some(i + 1);
        }
        i += 1;
    }
    if let Some(s) = start {
        let raw: String = chars[s.min(chars.len())..].iter().collect();
        if !raw.trim().is_empty() {
            let lead = raw.chars().take_while(|c| c.is_whitespace()).count();
            out.push((raw.trim().to_string(), s + lead + 1));
        }
    }
    out
}

fn heading_level(t: &str) -> Option<usize> {
    let level = t.chars().take_while(|&c| c == '#').count();
    ((1..=6).contains(&level) && t[level..].starts_with(' ')).then_some(level)
}

fn list_marker(t: &str) -> Option<(bool, usize)> {
    if t.starts_with("- ") || t.starts_with("* ") {
        return Some((false, 2));
    }
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    (digits > 0 && t[digits..].starts_with(". ")).then_some((true, digits + 2))
}

fn is_table_sep(t: &str) -> bool {
    t.starts_with('|') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) && t.contains('-')
}

fn split_attrs(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in s.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                cur.push(c);
            }
            ' ' if !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn parse_list(s: &str) -> Vec<String> {
    let s = s.trim().trim_start_matches('[').trim_end_matches(']');
    s.split(',').map(unquote).filter(|x| !x.is_empty()).collect()
}

pub fn plain_text(inl: &[Inline]) -> String {
    inl.iter()
        .map(|i| match i {
            Inline::Text(t) | Inline::Code(t) => t.clone(),
            Inline::Strong(v) | Inline::Emph(v) => plain_text(v),
            Inline::Link { text, .. } => plain_text(text),
            Inline::Math { src, .. } => src.clone(),
            _ => String::new(),
        })
        .collect()
}
