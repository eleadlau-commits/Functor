use std::collections::HashMap;

use crate::ast::{Align, Block, Env, Inline, Target};
use crate::diag::{did_you_mean, Diagnostics};
use crate::doc::Doc;
use crate::math::{self, MathEnv};
use crate::text::{self, Builder, Style, Wrap};

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
    measure: Option<f64>,
    base: Style,
    indent: f64,
    in_figure: bool,
    quiet: bool,
}

pub fn length_pt(s: &str) -> Option<f64> {
    let s = s.trim();
    let split = s.find(|c: char| c.is_ascii_alphabetic())?;
    let (n, unit) = s.split_at(split);
    let n: f64 = n.trim().parse().ok()?;
    let k = match unit {
        "pt" => 1.0,
        "mm" => 72.0 / 25.4,
        "cm" => 72.0 / 2.54,
        "in" => 72.0,
        _ => return None,
    };
    Some(n * k)
}

pub fn page_width_pt(page: &str) -> f64 {
    match page {
        "a5" => 419.53,
        "letter" => 612.0,
        _ => 595.28,
    }
}

const LIST_INDENT: f64 = 2.5;
const QUOTE_INDENT: f64 = 2.0;
const NOTE_INDENT: f64 = 1.2;

pub fn render(doc: &Doc, diags: &mut Diagnostics) -> String {
    render_width(doc, diags, None)
}

pub fn render_width(doc: &Doc, diags: &mut Diagnostics, width_pt: Option<f64>) -> String {
    let s = &doc.settings;
    let page = page_width_pt(&s.page);
    let measure = match (s.font.as_ref(), length_pt(&s.margin), length_pt(&s.text_size)) {
        (None, Some(m), Some(t)) if t > 0.0 => match width_pt {
            Some(w) if w > 0.0 => Some(w / t),
            _ if page > 2.0 * m => Some((page - 2.0 * m) / t),
            _ => None,
        },
        _ => None,
    };
    let mut r = R {
        env: &doc.env,
        labels: &doc.labels,
        diags,
        out: String::new(),
        measure,
        base: Style::default(),
        indent: 0.0,
        in_figure: false,
        quiet: s.style != "classic",
    };
    r.out.push_str(&format!(
        "<article class=\"fn-doc fn-style-{}\" data-page=\"{}\" style=\"--fn-page-width:{}pt;--fn-margin:{};--fn-text-size:{}{}{}\">",
        escape_attr(&s.style),
        escape_attr(&s.page),
        page,
        escape_attr(&s.margin),
        escape_attr(&s.text_size),
        s.font.as_ref().map(|f| format!(";--fn-font:{}", escape_attr(f))).unwrap_or_default(),
        s.accent.as_ref().map(|a| format!(";--fn-accent:{}", escape_attr(a))).unwrap_or_default()
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
                if !self.para(c, &[]) {
                    self.out.push_str("<p>");
                    self.inlines(c);
                    self.out.push_str("</p>");
                }
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
                self.indent += LIST_INDENT;
                for it in items {
                    self.out.push_str("<li>");
                    if !self.para(it, &[]) {
                        self.inlines(it);
                    }
                    self.out.push_str("</li>");
                }
                self.indent -= LIST_INDENT;
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
            let was = std::mem::replace(&mut self.in_figure, true);
            for b in &e.body {
                self.block(b);
            }
            self.in_figure = was;
            self.out.push_str("<figcaption>");
            self.out.push_str(&format!("<span class=\"fn-env-name\">Figure {}.</span> ", e.number.unwrap_or(0)));
            if let Some(c) = &e.caption {
                self.inlines(c);
            }
            self.out.push_str("</figcaption></figure>");
            return;
        }
        let headed = self.quiet && !matches!(e.kind.as_str(), "proof" | "quote" | "note");
        self.out.push_str(&format!("<section class=\"fn-env fn-{}{}\"{id}>", escape_attr(&e.kind), if headed { " fn-headed" } else { "" }));
        let mut name = capital(&e.kind);
        if let Some(n) = e.number {
            name.push_str(&format!(" {n}"));
        }
        let saved = (self.base, self.indent);
        let italic_body = !self.quiet && matches!(e.kind.as_str(), "theorem" | "lemma" | "corollary" | "proposition");
        if italic_body {
            self.base.italic = true;
        }
        match e.kind.as_str() {
            "quote" => self.indent += 2.0 * QUOTE_INDENT,
            "note" => self.indent += NOTE_INDENT,
            _ if headed => self.indent += NOTE_INDENT,
            _ => {}
        }
        if headed {
            self.out.push_str(&format!("<div class=\"fn-env-head\"><span class=\"fn-env-name\">{}</span>", escape(&name)));
            if let Some(t) = &e.title {
                self.out.push_str(&format!(" <span class=\"fn-env-title\">({})</span>", escape(t)));
            }
            self.out.push_str("</div>");
            for b in &e.body {
                self.block(b);
            }
            (self.base, self.indent) = saved;
            self.out.push_str("</section>");
            return;
        }
        let name_style = if e.kind == "proof" { Style { italic: true, ..Style::default() } } else { Style { bold: true, ..Style::default() } };
        let name_wrap = Wrap { open: "<span class=\"fn-env-name\">".into(), close: "</span>" };
        let mut prefix = vec![(name.replace(' ', "\u{A0}"), name_style, name_wrap.clone())];
        if let Some(t) = &e.title {
            prefix.push((format!(" ({t})"), Style { bold: false, ..self.base }, Wrap { open: "<span class=\"fn-env-title\">".into(), close: "</span>" }));
        }
        prefix.push((".".into(), name_style, name_wrap));
        let mut body = e.body.iter();
        let first_done = match e.body.first() {
            Some(Block::Para(c)) => self.para(c, &prefix),
            _ => false,
        };
        if first_done {
            body.next();
        } else {
            self.out.push_str(&format!("<span class=\"fn-env-name\">{name}</span>"));
            if let Some(t) = &e.title {
                self.out.push_str(&format!(" <span class=\"fn-env-title\">({})</span>", escape(t)));
            }
            self.out.push_str("<span class=\"fn-env-sep\">.</span> ");
        }
        for b in body {
            self.block(b);
        }
        (self.base, self.indent) = saved;
        if e.kind == "proof" {
            let qed = "<span class=\"fn-qed\">\u{220E}</span>";
            if self.out.ends_with("</span></p>") {
                let at = self.out.len() - "</span></p>".len();
                self.out.insert_str(at, qed);
            } else {
                self.out.push_str(qed);
            }
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

    fn para(&mut self, c: &[Inline], prefix: &[(String, Style, Wrap)]) -> bool {
        let Some(measure) = self.measure else { return false };
        if self.in_figure || !engine_ok(c, self.base) {
            return false;
        }
        let base = self.base;
        let mut b = Builder::new(base);
        for (t, st, w) in prefix {
            b.push(*st, Some(w.clone()));
            b.text(t);
            b.pop();
        }
        if !prefix.is_empty() {
            b.text(" ");
        }
        self.build(&mut b, c);
        if b.is_empty() {
            return false;
        }
        self.out.push_str(&b.finish(measure - self.indent, base, true, true));
        true
    }

    fn build(&mut self, b: &mut Builder, v: &[Inline]) {
        for i in v {
            match i {
                Inline::Text(t) => b.text(t),
                Inline::Strong(v) => {
                    let st = Style { bold: true, ..b.style() };
                    b.push(st, Some(Wrap { open: "<strong>".into(), close: "</strong>" }));
                    self.build(b, v);
                    b.pop();
                }
                Inline::Emph(v) => {
                    let cur = b.style();
                    let st = Style { italic: !cur.italic, ..cur };
                    let open = if st.italic { "<em>" } else { "<em class=\"fn-up\">" };
                    b.push(st, Some(Wrap { open: open.into(), close: "</em>" }));
                    self.build(b, v);
                    b.pop();
                }
                Inline::Code(t) => {
                    let st = Style { mono: true, italic: false, bold: false };
                    b.push(st, Some(Wrap { open: "<code>".into(), close: "</code>" }));
                    b.unbreakable(t);
                    b.pop();
                }
                Inline::Math { src, line, col } => {
                    let pos = math_pos(src, *line, *col);
                    let node = {
                        let mut ctx = math::Ctx { env: self.env, diags: self.diags, pos: &pos, display: false };
                        math::parse(src, &mut ctx)
                    };
                    let label = src.split_whitespace().collect::<Vec<_>>().join(" ");
                    for p in math::inline_pieces(&node, &label) {
                        b.raw_box(p.w, p.svg);
                        if let Some((w, st, sh, pen)) = p.gap {
                            b.math_gap(w, st, sh, pen);
                        }
                    }
                }
                Inline::Link { text, url } => {
                    b.push(b.style(), Some(Wrap { open: format!("<a href=\"{}\">", safe_url(url)), close: "</a>" }));
                    self.build(b, text);
                    b.pop();
                }
                Inline::Image { .. } => {}
                Inline::Ref { label, line, col } => {
                    let (text, open, close) = self.ref_parts(label, *line, *col);
                    b.push(b.style(), Some(Wrap { open, close }));
                    b.text(&text.replace(' ', "\u{A0}"));
                    b.pop();
                }
                Inline::Cite(keys) => {
                    b.push(b.style(), Some(Wrap { open: "<cite class=\"fn-cite\">".into(), close: "</cite>" }));
                    b.text(&format!("[{}]", keys.join("; ")));
                    b.pop();
                }
            }
        }
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

    fn ref_parts(&mut self, label: &str, line: usize, col: usize) -> (String, String, &'static str) {
        match self.labels.get(label) {
            Some(t) => {
                let text = match t {
                    Target::Heading { number: Some(n), .. } => format!("Section {n}"),
                    Target::Heading { number: None, text } => text.clone(),
                    Target::Env { kind, number: 0 } => capital(kind),
                    Target::Env { kind, number } => format!("{} {number}", capital(kind)),
                    Target::Equation(n) => format!("({n})"),
                };
                (text, format!("<a class=\"fn-ref\" href=\"#{}\">", label_id(label)), "</a>")
            }
            None => {
                let hint = match did_you_mean(label, self.labels.keys().map(String::as_str)) {
                    Some(s) => format!("Did you mean `@{s}`?"),
                    None => format!("Add `{{#{label}}}` to a heading, an equation or a block. To write a plain `@`, use `\\@`."),
                };
                self.diags.error(line, col, format!("The label `{label}` does not exist."), Some(hint));
                (format!("@{label}"), "<span class=\"fn-ref fn-missing\">".into(), "</span>")
            }
        }
    }

    fn reference(&mut self, label: &str, line: usize, col: usize) {
        let (text, open, close) = self.ref_parts(label, line, col);
        self.out.push_str(&open);
        self.out.push_str(&escape(&text));
        self.out.push_str(close);
    }
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

fn engine_ok(v: &[Inline], base: Style) -> bool {
    v.iter().all(|i| match i {
        Inline::Text(t) => text::covered(t, base),
        Inline::Strong(v) | Inline::Emph(v) => engine_ok(v, base),
        Inline::Link { text, .. } => engine_ok(text, base),
        Inline::Code(t) => text::covered(t, Style { mono: true, ..Style::default() }),
        Inline::Image { .. } => false,
        _ => true,
    })
}
