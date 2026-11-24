use std::collections::HashMap;

use crate::ast::{Align, Block, Env, Inline, Target};
use crate::diag::{did_you_mean, Diagnostics};
use crate::doc::Doc;
use crate::math::{self, MathEnv};

pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

pub fn escape_attr(s: &str) -> String {
    escape(s).replace('"', "&quot;")
}

fn safe_url(url: &str) -> String {
    let lower = url.trim().to_ascii_lowercase();
    if lower.starts_with("javascript:") || lower.starts_with("data:text") || lower.starts_with("vbscript:") {
        "#".into()
    } else {
        escape_attr(url)
    }
}

fn label_id(label: &str) -> String {
    format!("fn-{}", label.replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "-"))
}

pub fn math_pos(src: &str, line: usize, col: usize) -> impl Fn(usize) -> (usize, usize) + '_ {
    move |o: usize| {
        let before = &src[..o.min(src.len())];
        let nl = before.matches('\n').count();
        if nl == 0 {
            (line, col + before.chars().count())
        } else {
            let last = before.rsplit('\n').next().unwrap_or("");
            (line + nl, 1 + last.chars().count())
        }
    }
}

struct R<'a> {
    env: &'a MathEnv,
    labels: &'a HashMap<String, Target>,
    diags: &'a mut Diagnostics,
    out: String,
}

pub fn render(doc: &Doc, diags: &mut Diagnostics) -> String {
    let mut r = R { env: &doc.env, labels: &doc.labels, diags, out: String::new() };
    let s = &doc.settings;
    r.out.push_str(&format!(
        "<article class=\"fn-doc\" data-page=\"{}\" style=\"--fn-margin:{};--fn-text-size:{}{}\">",
        escape_attr(&s.page),
        escape_attr(&s.margin),
        escape_attr(&s.text_size),
        s.font.as_ref().map(|f| format!(";--fn-font:{}", escape_attr(f))).unwrap_or_default()
    ));
    let m = &doc.meta;
    if m.title.is_some() || !m.authors.is_empty() || m.date.is_some() {
        r.out.push_str("<header class=\"fn-title\">");
        if let Some(t) = &m.title {
            r.out.push_str(&format!("<h1>{}</h1>", escape(t)));
        }
        if !m.authors.is_empty() {
            r.out.push_str(&format!("<p class=\"fn-authors\">{}</p>", escape(&m.authors.join(", "))));
        }
        if let Some(d) = &m.date {
            r.out.push_str(&format!("<p class=\"fn-date\">{}</p>", escape(d)));
        }
        r.out.push_str("</header>");
    }
    for b in &doc.blocks {
        r.block(b);
    }
    r.out.push_str("</article>");
    r.out
}

impl R<'_> {
    fn block(&mut self, b: &Block) {
        match b {
            Block::Heading { level, content, label, number } => {
                let lv = (*level + 1).min(6);
                let id = label.as_deref().map(|l| format!(" id=\"{}\"", label_id(l))).unwrap_or_default();
                self.out.push_str(&format!("<h{lv}{id}>"));
                if let Some(n) = number {
                    self.out.push_str(&format!("<span class=\"fn-num\">{n}</span> "));
                }
                self.inlines(content);
                self.out.push_str(&format!("</h{lv}>"));
            }
            Block::Para(c) => {
                self.out.push_str("<p>");
                self.inlines(c);
                self.out.push_str("</p>");
            }
            Block::Math { src, line, col, label, number } => {
                let id = label.as_deref().map(|l| format!(" id=\"{}\"", label_id(l))).unwrap_or_default();
                self.out.push_str(&format!("<div class=\"fn-equation\"{id}>"));
                self.math(src, *line, *col, true);
                if let Some(n) = number {
                    self.out.push_str(&format!("<span class=\"fn-eqnum\">({n})</span>"));
                }
                self.out.push_str("</div>");
            }
            Block::List { ordered, items } => {
                let tag = if *ordered { "ol" } else { "ul" };
                self.out.push_str(&format!("<{tag}>"));
                for it in items {
                    self.out.push_str("<li>");
                    self.inlines(it);
                    self.out.push_str("</li>");
                }
                self.out.push_str(&format!("</{tag}>"));
            }
            Block::Table { align, header, rows } => {
                let style = |i: usize| match align.get(i) {
                    Some(Align::Left) => " style=\"text-align:left\"",
                    Some(Align::Center) => " style=\"text-align:center\"",
                    Some(Align::Right) => " style=\"text-align:right\"",
                    _ => "",
                };
                self.out.push_str("<table class=\"fn-table\"><thead><tr>");
                for (i, h) in header.iter().enumerate() {
                    self.out.push_str(&format!("<th{}>", style(i)));
                    self.inlines(h);
                    self.out.push_str("</th>");
                }
                self.out.push_str("</tr></thead><tbody>");
                for row in rows {
                    self.out.push_str("<tr>");
                    for (i, c) in row.iter().enumerate() {
                        self.out.push_str(&format!("<td{}>", style(i)));
                        self.inlines(c);
                        self.out.push_str("</td>");
                    }
                    self.out.push_str("</tr>");
                }
                self.out.push_str("</tbody></table>");
            }
            Block::Code { lang, text } => {
                self.out.push_str(&format!(
                    "<pre class=\"fn-code\" data-lang=\"{}\"><code>{}</code></pre>",
                    escape_attr(lang),
                    escape(text)
                ));
            }
            Block::Env(e) => self.env_block(e),
            Block::Rule => self.out.push_str("<hr>"),
        }
    }

    fn env_block(&mut self, e: &Env) {
        let id = e.label.as_deref().map(|l| format!(" id=\"{}\"", label_id(l))).unwrap_or_default();
        if e.kind == "figure" {
            let width = e.width.as_deref().map(|w| format!(" style=\"max-width:{}\"", escape_attr(w))).unwrap_or_default();
            self.out.push_str(&format!("<figure class=\"fn-figure\"{id}{width}>"));
            for b in &e.body {
                self.block(b);
            }
            self.out.push_str("<figcaption>");
            self.out.push_str(&format!("<span class=\"fn-env-name\">Figure {}.</span> ", e.number.unwrap_or(0)));
            if let Some(c) = &e.caption {
                self.inlines(c);
            }
            self.out.push_str("</figcaption></figure>");
            return;
        }
        self.out.push_str(&format!("<section class=\"fn-env fn-{}\"{id}>", escape_attr(&e.kind)));
        let mut name = capital(&e.kind);
        if let Some(n) = e.number {
            name.push_str(&format!(" {n}"));
        }
        self.out.push_str(&format!("<span class=\"fn-env-name\">{name}</span>"));
        if let Some(t) = &e.title {
            self.out.push_str(&format!(" <span class=\"fn-env-title\">({})</span>", escape(t)));
        }
        self.out.push_str("<span class=\"fn-env-sep\">.</span> ");
        for b in &e.body {
            self.block(b);
        }
        if e.kind == "proof" {
            self.out.push_str("<span class=\"fn-qed\">\u{220E}</span>");
        }
        self.out.push_str("</section>");
    }

    fn math(&mut self, src: &str, line: usize, col: usize, display: bool) {
        let pos = math_pos(src, line, col);
        let node = {
            let mut ctx = math::Ctx { env: self.env, diags: self.diags, pos: &pos, display };
            math::parse(src, &mut ctx)
        };
        let label = src.split_whitespace().collect::<Vec<_>>().join(" ");
        self.out.push_str(&math::render(&node, display, &label));
    }

    fn inlines(&mut self, v: &[Inline]) {
        for i in v {
            self.inline(i);
        }
    }

    fn inline(&mut self, i: &Inline) {
        match i {
            Inline::Text(t) => self.out.push_str(&escape(t)),
            Inline::Strong(v) => {
                self.out.push_str("<strong>");
                self.inlines(v);
                self.out.push_str("</strong>");
            }
            Inline::Emph(v) => {
                self.out.push_str("<em>");
                self.inlines(v);
                self.out.push_str("</em>");
            }
            Inline::Code(t) => self.out.push_str(&format!("<code>{}</code>", escape(t))),
            Inline::Math { src, line, col } => self.math(src, *line, *col, false),
            Inline::Link { text, url } => {
                self.out.push_str(&format!("<a href=\"{}\">", safe_url(url)));
                self.inlines(text);
                self.out.push_str("</a>");
            }
            Inline::Image { alt, src } => {
                self.out.push_str(&format!("<img src=\"{}\" alt=\"{}\">", safe_url(src), escape_attr(alt)));
            }
            Inline::Ref { label, line, col } => self.reference(label, *line, *col),
            Inline::Cite(keys) => {
                let keys: Vec<String> = keys.iter().map(|k| escape(k)).collect();
                self.out.push_str(&format!("<cite class=\"fn-cite\">[{}]</cite>", keys.join("; ")));
            }
        }
    }

    fn reference(&mut self, label: &str, line: usize, col: usize) {
        match self.labels.get(label) {
            Some(t) => {
                let text = match t {
                    Target::Heading { number: Some(n), .. } => format!("Section {n}"),
                    Target::Heading { number: None, text } => text.clone(),
                    Target::Env { kind, number: 0 } => capital(kind),
                    Target::Env { kind, number } => format!("{} {number}", capital(kind)),
                    Target::Equation(n) => format!("({n})"),
                };
                self.out.push_str(&format!("<a class=\"fn-ref\" href=\"#{}\">{}</a>", label_id(label), escape(&text)));
            }
            None => {
                let hint = match did_you_mean(label, self.labels.keys().map(String::as_str)) {
                    Some(s) => format!("Did you mean `@{s}`?"),
                    None => format!("Add `{{#{label}}}` to a heading, an equation or a block. To write a plain `@`, use `\\@`."),
                };
                self.diags.error(line, col, format!("The label `{label}` does not exist."), Some(hint));
                self.out.push_str(&format!("<span class=\"fn-ref fn-missing\">@{}</span>", escape(label)));
            }
        }
    }
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}
