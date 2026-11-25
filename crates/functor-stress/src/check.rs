use std::time::Instant;

use functor_paper::ast::{Block, Inline};
use functor_paper::diag::{Diagnostic, Diagnostics, Severity};
use functor_paper::math::{self, Ctx, MathEnv};
use functor_paper::{compile, compile_pdf, doc, glyph};

use crate::ink;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Valid,
    Garbage,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Valid => "valid",
            Mode::Garbage => "garbage",
        }
    }

    pub fn parse(s: &str) -> Option<Mode> {
        match s {
            "valid" => Some(Mode::Valid),
            "garbage" => Some(Mode::Garbage),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Opts {
    pub pdf: bool,
    pub collisions: bool,
    pub slow_ms: u128,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            pdf: true,
            collisions: true,
            slow_ms: 1500,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    pub kind: &'static str,
    pub sig: String,
    pub detail: String,
}

impl Failure {
    fn new(kind: &'static str, key: &str, detail: impl Into<String>) -> Failure {
        let sig = if key.is_empty() {
            kind.to_string()
        } else {
            format!("{kind}: {key}")
        };
        Failure {
            kind,
            sig,
            detail: detail.into(),
        }
    }
}

pub fn normalize(msg: &str) -> String {
    let mut out = String::new();
    let mut in_code = false;
    let mut last_digit = false;
    for c in msg.chars() {
        if c == '`' {
            if !in_code {
                out.push_str("`_`");
            }
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        if c.is_ascii_digit() {
            if !last_digit {
                out.push('N');
            }
            last_digit = true;
            continue;
        }
        last_digit = false;
        out.push(c);
    }
    out.trim().to_string()
}

fn same(a: &functor_paper::Output, b: &functor_paper::Output) -> bool {
    a.html == b.html && a.diagnostics == b.diagnostics
}

pub fn check(src: &str, mode: Mode, o: Opts) -> Vec<Failure> {
    let mut f = Vec::new();
    glyph::clear_cache();
    let t = Instant::now();
    let out = compile(src);
    let ms = t.elapsed().as_millis();
    if ms > o.slow_ms {
        f.push(Failure::new("slow", "", format!("{ms} ms")));
    }
    let lines = src.lines().count().max(1);
    let errors: Vec<&Diagnostic> = out
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    if mode == Mode::Valid {
        if let Some(e) = errors.first() {
            f.push(Failure::new(
                "refused-valid",
                &normalize(&e.message),
                format!("line {}: {}", e.line, e.message),
            ));
        }
    }
    for d in &out.diagnostics {
        if d.line == 0 || d.line > lines + 1 || d.col == 0 {
            f.push(Failure::new(
                "bad-position",
                &normalize(&d.message),
                format!("line {} col {} of {lines}: {}", d.line, d.col, d.message),
            ));
            break;
        }
        if d.message.trim().is_empty() {
            f.push(Failure::new(
                "empty-message",
                "",
                format!("line {}", d.line),
            ));
            break;
        }
    }
    svg_checks(&out.html, mode, &mut f);
    let warm = compile(src);
    if !same(&out, &warm) {
        f.push(Failure::new(
            "nondeterministic",
            "warm cache",
            diff_note(&out, &warm),
        ));
    }
    let saved = glyph::export_cache();
    glyph::clear_cache();
    glyph::import_cache(&saved);
    let disk = compile(src);
    if !same(&out, &disk) {
        f.push(Failure::new(
            "nondeterministic",
            "disk cache",
            diff_note(&out, &disk),
        ));
    }
    if o.pdf {
        let p = compile_pdf(src);
        if let Err(e) = pdf_ok(&p.pdf, p.pages) {
            f.push(Failure::new("pdf", &normalize(&e), e));
        }
    }
    if o.collisions && mode == Mode::Valid && errors.is_empty() {
        for c in collisions(src) {
            let key = format!(
                "{} ({})",
                c.what,
                if c.display { "display" } else { "inline" }
            );
            if !f.iter().any(|x: &Failure| x.sig.ends_with(&key)) {
                f.push(Failure::new(
                    "collision",
                    &key,
                    format!("{} cells overlap in `{}`", c.overlap, c.src),
                ));
            }
        }
    }
    f
}

fn diff_note(a: &functor_paper::Output, b: &functor_paper::Output) -> String {
    if a.diagnostics != b.diagnostics {
        return format!(
            "diagnostics differ: {} then {}",
            a.diagnostics.len(),
            b.diagnostics.len()
        );
    }
    let at = a
        .html
        .bytes()
        .zip(b.html.bytes())
        .position(|(x, y)| x != y)
        .unwrap_or(a.html.len().min(b.html.len()));
    let s = at.saturating_sub(40);
    let cut = |h: &str| h.get(s..(at + 40).min(h.len())).unwrap_or("").to_string();
    format!(
        "html differs at byte {at}: `{}` vs `{}`",
        cut(&a.html),
        cut(&b.html)
    )
}

fn svg_checks(html: &str, mode: Mode, f: &mut Vec<Failure>) {
    let mut rest = html;
    while let Some(i) = rest.find("<svg class=\"fn-math") {
        let tail = &rest[i..];
        let Some(end) = tail.find("</svg>") else {
            break;
        };
        let svg = &tail[..end];
        let clean = strip_attr(svg, "aria-label=\"");
        if clean.contains("NaN") || clean.contains("inf") {
            f.push(Failure::new(
                "bad-number",
                "",
                snippet(&clean, if clean.contains("NaN") { "NaN" } else { "inf" }),
            ));
            return;
        }
        if mode == Mode::Valid
            && svg.contains("role=\"img\"")
            && !svg.contains("<path")
            && !svg.contains("<rect")
        {
            f.push(Failure::new("empty-math", "", label_of(svg)));
            return;
        }
        rest = &tail[end..];
    }
}

fn label_of(svg: &str) -> String {
    svg.find("aria-label=\"")
        .map(|i| svg[i + 12..].split('"').next().unwrap_or("").to_string())
        .unwrap_or_default()
}

fn strip_attr(s: &str, attr: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find(attr) {
        out.push_str(&rest[..i]);
        let after = &rest[i + attr.len()..];
        rest = after.find('"').map_or("", |j| &after[j + 1..]);
    }
    out.push_str(rest);
    out
}

fn snippet(s: &str, needle: &str) -> String {
    let i = s.find(needle).unwrap_or(0);
    let a = s.floor_char_boundary_compat(i.saturating_sub(60));
    let b = s.floor_char_boundary_compat((i + 60).min(s.len()));
    s[a..b].to_string()
}

trait Boundary {
    fn floor_char_boundary_compat(&self, i: usize) -> usize;
}

impl Boundary for str {
    fn floor_char_boundary_compat(&self, mut i: usize) -> usize {
        while i > 0 && !self.is_char_boundary(i) {
            i -= 1;
        }
        i
    }
}

pub fn pdf_ok(pdf: &[u8], pages: usize) -> Result<(), String> {
    if !pdf.starts_with(b"%PDF-") {
        return Err("no %PDF header".into());
    }
    let tail = &pdf[pdf.len().saturating_sub(64)..];
    if !find(tail, b"%%EOF").is_some() {
        return Err("no %%EOF".into());
    }
    let sx = rfind(pdf, b"startxref").ok_or("no startxref")?;
    let num: String = pdf[sx + 9..]
        .iter()
        .skip_while(|c| c.is_ascii_whitespace())
        .take_while(|c| c.is_ascii_digit())
        .map(|&c| c as char)
        .collect();
    let xref: usize = num.parse().map_err(|_| "bad startxref".to_string())?;
    if !pdf.get(xref..).is_some_and(|s| s.starts_with(b"xref")) {
        return Err("startxref does not point at xref".into());
    }
    let table = &pdf[xref..];
    let text = String::from_utf8_lossy(&table[..table.len().min(200_000)]);
    let mut lines = text.lines().skip(1);
    let head = lines.next().ok_or("empty xref")?;
    let mut hp = head.split_whitespace();
    let first: usize = hp
        .next()
        .and_then(|x| x.parse().ok())
        .ok_or("bad xref head")?;
    let n: usize = hp
        .next()
        .and_then(|x| x.parse().ok())
        .ok_or("bad xref head")?;
    for k in 0..n {
        let l = lines.next().ok_or("short xref")?;
        let parts: Vec<&str> = l.split_whitespace().collect();
        if parts.len() < 3 || parts[2] != "n" {
            continue;
        }
        let off: usize = parts[0].parse().map_err(|_| "bad xref entry".to_string())?;
        let want = format!("{} 0 obj", first + k);
        if !pdf
            .get(off..)
            .is_some_and(|s| s.starts_with(want.as_bytes()))
        {
            return Err(format!(
                "xref entry {} points at the wrong place",
                first + k
            ));
        }
    }
    let count = count_pages(pdf);
    if count != pages || pages == 0 {
        return Err(format!("{count} page objects, layout says {pages}"));
    }
    let mut at = 0;
    while let Some(i) = find(&pdf[at..], b"stream") {
        let s = at + i + 6;
        if pdf[..at + i].ends_with(b"end") {
            at = s;
            continue;
        }
        let s = if pdf.get(s) == Some(&b'\r') {
            s + 2
        } else {
            s + 1
        };
        let Some(e) = find(&pdf[s..], b"endstream") else {
            return Err("stream without endstream".into());
        };
        let data = &pdf[s..s + e];
        let dict_start = rfind(&pdf[..at + i], b"<<").unwrap_or(0);
        let dict = String::from_utf8_lossy(&pdf[dict_start..at + i]);
        if dict.contains("/FlateDecode")
            && !dict.contains("/FontFile")
            && !dict.contains("/Subtype /OpenType")
        {
            match miniz_oxide::inflate::decompress_to_vec_zlib(data) {
                Ok(raw) => {
                    let t = String::from_utf8_lossy(&raw);
                    if t.split_whitespace()
                        .any(|w| matches!(w, "NaN" | "inf" | "-inf"))
                    {
                        return Err("content stream has NaN or inf".into());
                    }
                }
                Err(_) => return Err("stream does not inflate".into()),
            }
        }
        at = s + e + 9;
    }
    Ok(())
}

fn count_pages(pdf: &[u8]) -> usize {
    let mut n = 0;
    let mut at = 0;
    while let Some(i) = find(&pdf[at..], b"/Type /Page") {
        let j = at + i + 11;
        if pdf.get(j) != Some(&b's') {
            n += 1;
        }
        at = j;
    }
    n
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

fn rfind(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).rposition(|w| w == n)
}

pub struct Found {
    pub what: String,
    pub overlap: usize,
    pub display: bool,
    pub src: String,
}

pub fn collisions(src: &str) -> Vec<Found> {
    let mut diags = Diagnostics::default();
    let d = doc::parse(src, &mut diags);
    let mut maths: Vec<(String, bool)> = Vec::new();
    collect_blocks(&d.blocks, &mut maths);
    let mut out = Vec::new();
    for (m, display) in maths {
        let node = parse_math(&m, &d.env, display);
        let mut found = Vec::new();
        let mut budget = 80;
        ink::check_node(&node, display, &mut budget, &mut found);
        for c in found {
            out.push(Found {
                what: c.what,
                overlap: c.overlap,
                display: c.display,
                src: m.clone(),
            });
        }
    }
    out
}

pub fn parse_math(src: &str, env: &MathEnv, display: bool) -> math::Node {
    let mut diags = Diagnostics::default();
    let pos = |_: usize| (1, 1);
    let mut ctx = Ctx {
        env,
        diags: &mut diags,
        pos: &pos,
        display,
    };
    math::parse(src, &mut ctx)
}

fn collect_blocks(bs: &[Block], out: &mut Vec<(String, bool)>) {
    for b in bs {
        match b {
            Block::Math { src, .. } => out.push((src.clone(), true)),
            Block::Heading { content, .. } | Block::Para(content) => collect_inl(content, out),
            Block::List { items, .. } => items.iter().for_each(|i| collect_inl(i, out)),
            Block::Table { header, rows, .. } => {
                header.iter().for_each(|c| collect_inl(c, out));
                rows.iter().flatten().for_each(|c| collect_inl(c, out));
            }
            Block::Env(e) => {
                collect_blocks(&e.body, out);
                if let Some(c) = &e.caption {
                    collect_inl(c, out);
                }
            }
            _ => {}
        }
    }
}

fn collect_inl(v: &[Inline], out: &mut Vec<(String, bool)>) {
    for i in v {
        match i {
            Inline::Math { src, .. } => out.push((src.clone(), false)),
            Inline::Strong(x) | Inline::Emph(x) | Inline::Note(x) => collect_inl(x, out),
            Inline::Link { text, .. } => collect_inl(text, out),
            _ => {}
        }
    }
}
