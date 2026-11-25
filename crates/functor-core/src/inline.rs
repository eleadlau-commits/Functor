use std::collections::HashMap;

use crate::ast::Inline;
use crate::diag::{did_you_mean, Diagnostics};

pub struct Span {
    pub text: String,
    segments: Vec<(usize, usize, usize)>,
}

impl Span {
    pub fn new() -> Self {
        Span { text: String::new(), segments: Vec::new() }
    }

    pub fn single(text: &str, line: usize, col: usize) -> Self {
        let mut s = Span::new();
        s.push_line(text, line, col);
        s
    }

    pub fn push_line(&mut self, text: &str, line: usize, col: usize) {
        if !self.text.is_empty() {
            self.text.push('\n');
        }
        self.segments.push((self.text.len(), line, col));
        self.text.push_str(text);
    }

    pub fn pos(&self, offset: usize) -> (usize, usize) {
        let seg = self
            .segments
            .iter()
            .rev()
            .find(|(start, _, _)| *start <= offset)
            .copied()
            .unwrap_or((0, 1, 1));
        let (start, line, col) = seg;
        let chars = self.text[start..offset.min(self.text.len())].chars().count();
        (line, col + chars)
    }
}

pub struct Ctx<'a> {
    pub vars: &'a HashMap<String, String>,
    pub diags: &'a mut Diagnostics,
}

pub fn parse_inline(span: &Span, ctx: &mut Ctx) -> Vec<Inline> {
    let mut p = Parser { s: &span.text, span, i: 0, ctx };
    p.run(None)
}

struct Parser<'a, 'b, 'c> {
    s: &'a str,
    span: &'a Span,
    i: usize,
    ctx: &'b mut Ctx<'c>,
}

const ESCAPABLE: &str = "\\$*_`{}[]@#!|~\"'-^";

impl Parser<'_, '_, '_> {
    fn rest(&self) -> &str {
        &self.s[self.i..]
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn prev_char(&self) -> Option<char> {
        self.s[..self.i].chars().next_back()
    }

    fn error(&mut self, at: usize, msg: impl Into<String>, hint: Option<String>) {
        let (l, c) = self.span.pos(at);
        self.ctx.diags.error(l, c, msg, hint);
    }

    fn warning(&mut self, at: usize, msg: impl Into<String>, hint: Option<String>) {
        let (l, c) = self.span.pos(at);
        self.ctx.diags.warning(l, c, msg, hint);
    }

    fn run(&mut self, close: Option<&str>) -> Vec<Inline> {
        let mut out: Vec<Inline> = Vec::new();
        let mut text = String::new();
        while let Some(c) = self.peek() {
            if let Some(cl) = close {
                if self.rest().starts_with(cl) {
                    break;
                }
            }
            let start = self.i;
            let node = match c {
                '\\' => {
                    let next = self.rest()[1..].chars().next();
                    match next {
                        Some(n) if ESCAPABLE.contains(n) => {
                            self.i += 1 + n.len_utf8();
                            text.push(n);
                            continue;
                        }
                        _ => None,
                    }
                }
                '$' => self.math(),
                '`' => self.code(),
                '*' | '_' => self.emphasis(c),
                '^' if self.rest().starts_with("^[") => self.note(),
                '[' => self.bracket(),
                '!' if self.rest().starts_with("![") => self.image(),
                '@' => self.reference(),
                '{' => self.variable(),
                '"' | '\'' | '-' | '~' => {
                    if let Some((ch, len)) = self.punctuation(c) {
                        self.i += len;
                        text.push(ch);
                        continue;
                    }
                    None
                }
                _ => None,
            };
            match node {
                Some(n) => {
                    if !text.is_empty() {
                        out.push(Inline::Text(std::mem::take(&mut text)));
                    }
                    out.push(n);
                }
                None => {
                    self.i = start + c.len_utf8();
                    text.push(c);
                }
            }
        }
        if !text.is_empty() {
            out.push(Inline::Text(text));
        }
        out
    }

    fn punctuation(&self, c: char) -> Option<(char, usize)> {
        let opens = self.prev_char().is_none_or(|p| p.is_whitespace() || "([{\u{2014}\u{2013}/\u{A0}".contains(p));
        match c {
            '"' => Some((if opens { '\u{201C}' } else { '\u{201D}' }, 1)),
            '\'' => Some((if opens { '\u{2018}' } else { '\u{2019}' }, 1)),
            '~' => Some(('\u{A0}', 1)),
            '-' if self.rest().starts_with("---") => Some(('\u{2014}', 3)),
            '-' if self.rest().starts_with("--") => Some(('\u{2013}', 2)),
            _ => None,
        }
    }

    fn find_unescaped(&self, from: usize, pat: char) -> Option<usize> {
        let bytes = self.s.as_bytes();
        let mut j = from;
        while j < self.s.len() {
            let ch = self.s[j..].chars().next()?;
            if ch == '\\' {
                j += 1 + self.s[j + 1..].chars().next().map_or(0, |c| c.len_utf8());
                continue;
            }
            if ch == pat {
                return Some(j);
            }
            j += ch.len_utf8();
            let _ = bytes;
        }
        None
    }

    fn math(&mut self) -> Option<Inline> {
        let start = self.i;
        let (line, col) = self.span.pos(start + 1);
        match self.find_unescaped(start + 1, '$') {
            Some(end) => {
                let tex = self.s[start + 1..end].to_string();
                self.i = end + 1;
                if tex.trim().is_empty() {
                    self.warning(start, "Empty math. Remove the two `$` signs or add math between them.", None);
                }
                Some(Inline::Math { src: tex, line, col })
            }
            None => {
                self.error(
                    start,
                    "This math has no closing `$`.",
                    Some("Add `$` where the math ends. To write a dollar sign, use `\\$`.".into()),
                );
                let tex = self.s[start + 1..].to_string();
                self.i = self.s.len();
                Some(Inline::Math { src: tex, line, col })
            }
        }
    }

    fn code(&mut self) -> Option<Inline> {
        let start = self.i;
        match self.s[start + 1..].find('`') {
            Some(rel) => {
                let end = start + 1 + rel;
                self.i = end + 1;
                Some(Inline::Code(self.s[start + 1..end].to_string()))
            }
            None => {
                self.warning(start, "This code has no closing backtick.", Some("Add ` where the code ends.".into()));
                None
            }
        }
    }

    fn emphasis(&mut self, c: char) -> Option<Inline> {
        let start = self.i;
        let strong = self.rest().starts_with(if c == '*' { "**" } else { "__" });
        let marker = if strong { if c == '*' { "**" } else { "__" } } else if c == '*' { "*" } else { "_" };
        if c == '_' && self.prev_char().is_some_and(|p| p.is_alphanumeric()) {
            return None;
        }
        let after = &self.s[start + marker.len()..];
        if after.starts_with(char::is_whitespace) || after.is_empty() {
            return None;
        }
        self.i = start + marker.len();
        let inner = self.run(Some(marker));
        if self.rest().starts_with(marker) {
            self.i += marker.len();
            Some(if strong { Inline::Strong(inner) } else { Inline::Emph(inner) })
        } else {
            self.warning(
                start,
                format!("`{marker}` starts {} text, but it does not end.", if strong { "bold" } else { "italic" }),
                Some(format!("Add `{marker}` where it ends, or write `\\{c}` for a plain `{c}`.")),
            );
            self.i = start;
            None
        }
    }

    fn bracket(&mut self) -> Option<Inline> {
        let start = self.i;
        if self.rest().starts_with("[@") {
            let end = self.s[start..].find(']')? + start;
            let keys: Vec<String> = self.s[start + 1..end]
                .split(';')
                .map(|k| k.trim().trim_start_matches('@').to_string())
                .filter(|k| !k.is_empty())
                .collect();
            if keys.is_empty() {
                return None;
            }
            self.i = end + 1;
            return Some(Inline::Cite(keys));
        }
        let close = self.find_unescaped(start + 1, ']')?;
        if !self.s[close + 1..].starts_with('(') {
            return None;
        }
        let url_end = self.s[close + 2..].find(')')? + close + 2;
        let url = self.s[close + 2..url_end].trim().to_string();
        let label_span = Span::single(&self.s[start + 1..close], 0, 0);
        let (l, c) = self.span.pos(start + 1);
        let mut inner_span = Span::new();
        inner_span.push_line(&label_span.text, l, c);
        let text = parse_inline(&inner_span, self.ctx);
        self.i = url_end + 1;
        Some(Inline::Link { text, url })
    }

    fn note(&mut self) -> Option<Inline> {
        let start = self.i;
        let bytes = self.s.as_bytes();
        let mut depth = 0usize;
        let mut math = false;
        let mut k = start + 2;
        let mut close = None;
        while k < bytes.len() {
            match bytes[k] {
                b'\\' => k += 1,
                b'$' => math = !math,
                b'[' if !math => depth += 1,
                b']' if !math => {
                    if depth == 0 {
                        close = Some(k);
                        break;
                    }
                    depth -= 1;
                }
                _ => {}
            }
            k += 1;
        }
        let Some(close) = close else {
            self.error(start, "This note has no closing `]`.", Some("Write a note as `^[text]`.".into()));
            self.i = self.s.len();
            return Some(Inline::Text(String::new()));
        };
        let (l, c) = self.span.pos(start + 2);
        let mut inner_span = Span::new();
        inner_span.push_line(&self.s[start + 2..close], l, c);
        let body = parse_inline(&inner_span, self.ctx);
        if body.iter().any(|i| matches!(i, Inline::Note(_))) {
            self.warning(start, "A note inside a note shows as plain text.", Some("Put the second note after the first one.".into()));
        }
        self.i = close + 1;
        Some(Inline::Note(body))
    }

    fn image(&mut self) -> Option<Inline> {
        let start = self.i;
        let close = self.s[start + 2..].find(']')? + start + 2;
        if !self.s[close + 1..].starts_with('(') {
            return None;
        }
        let end = self.s[close + 2..].find(')')? + close + 2;
        let alt = self.s[start + 2..close].to_string();
        let src = self.s[close + 2..end].trim().to_string();
        if src.is_empty() {
            self.error(start, "This image has no file.", Some("Write the file name in the brackets: `![](plot.svg)`.".into()));
        }
        self.i = end + 1;
        Some(Inline::Image { alt, src })
    }

    fn reference(&mut self) -> Option<Inline> {
        let start = self.i;
        if self.prev_char().is_some_and(|p| p.is_alphanumeric()) {
            return None;
        }
        let label = take_label(&self.s[start + 1..]);
        if label.is_empty() {
            return None;
        }
        self.i = start + 1 + label.len();
        let (line, col) = self.span.pos(start);
        Some(Inline::Ref { label: label.to_string(), line, col })
    }

    fn variable(&mut self) -> Option<Inline> {
        let start = self.i;
        let end = self.s[start..].find('}')? + start;
        let name = self.s[start + 1..end].trim();
        if name.is_empty() || !is_ident(name) {
            return None;
        }
        self.i = end + 1;
        match self.ctx.vars.get(name) {
            Some(v) => Some(Inline::Text(v.clone())),
            None => {
                let hint = match did_you_mean(name, self.ctx.vars.keys().map(String::as_str)) {
                    Some(s) => format!("Did you mean `{s}`?"),
                    None => format!("Define it first: `let {name} = ...`. For a plain brace, write `\\{{`."),
                };
                self.error(start, format!("`{name}` is not defined."), Some(hint));
                Some(Inline::Text(format!("{{{name}}}")))
            }
        }
    }
}

pub fn take_label(s: &str) -> &str {
    let mut end = 0;
    for (i, ch) in s.char_indices() {
        if ch.is_alphanumeric() || matches!(ch, '_' | '-' | ':' | '.') {
            end = i + ch.len_utf8();
        } else {
            break;
        }
    }
    let label = &s[..end];
    let trimmed = label.trim_end_matches(['.', ':', '-']);
    if trimmed.chars().next().is_some_and(|c| c.is_alphabetic()) {
        trimmed
    } else {
        ""
    }
}

pub fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_alphabetic() || c == '_')
        && chars.all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> (Vec<Inline>, Diagnostics) {
        let vars = HashMap::from([("n".to_string(), "20".to_string())]);
        let mut diags = Diagnostics::default();
        let out = {
            let mut ctx = Ctx { vars: &vars, diags: &mut diags };
            parse_inline(&Span::single(s, 3, 1), &mut ctx)
        };
        (out, diags)
    }

    #[test]
    fn parses_marks() {
        let (out, d) = parse("a **b** *c* `d` $x^2$ [@k1; @k2] @fig:plot. {n}");
        assert!(d.items.is_empty(), "{:?}", d.items);
        assert!(out.contains(&Inline::Strong(vec![Inline::Text("b".into())])));
        assert!(out.iter().any(|i| matches!(i, Inline::Math { src, .. } if src == "x^2")));
        assert!(out.contains(&Inline::Cite(vec!["k1".into(), "k2".into()])));
        assert!(out.iter().any(|i| matches!(i, Inline::Ref { label, .. } if label == "fig:plot")));
        assert!(out.contains(&Inline::Text("20".into())));
    }

    #[test]
    fn smart_punctuation() {
        let (out, d) = parse("\"Quote\" and 'single', don't, pages 3--5 --- see Theorem~2. Not `a--b`, \\\"raw\\\" a\\-\\-b");
        assert!(d.items.is_empty(), "{:?}", d.items);
        let text: String = out
            .iter()
            .map(|i| match i {
                Inline::Text(t) => t.clone(),
                Inline::Code(c) => format!("[{c}]"),
                _ => String::new(),
            })
            .collect();
        assert_eq!(
            text,
            "\u{201C}Quote\u{201D} and \u{2018}single\u{2019}, don\u{2019}t, pages 3\u{2013}5 \u{2014} see Theorem\u{A0}2. Not [a--b], \"raw\" a--b"
        );
    }

    #[test]
    fn unclosed_math_reports_position() {
        let (_, d) = parse("one $x + y");
        assert_eq!(d.items.len(), 1);
        assert_eq!((d.items[0].line, d.items[0].col), (3, 5));
    }

    #[test]
    fn email_is_not_a_ref_and_snake_case_is_not_italic() {
        let (out, d) = parse("mail me@host.org about snake_case_name");
        assert!(d.items.is_empty());
        assert!(out.iter().all(|i| matches!(i, Inline::Text(_))));
    }

    #[test]
    fn unknown_variable_suggests() {
        let (_, d) = parse("{m}");
        assert_eq!(d.items[0].hint.as_deref(), Some("Did you mean `n`?"));
    }
}
