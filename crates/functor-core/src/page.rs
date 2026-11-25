use std::rc::Rc;

use crate::ast::{Align, Block, Env, Inline};
use crate::diag::Diagnostics;
use crate::doc::Doc;
use crate::linebreak::INF;
use crate::math::{self, LBox};
use crate::render::{capital, length_pt, math_pos, page_width_pt, R};
use crate::text::{self, Builder, PItem, PLine, Style, Wrap};

pub const INK: [f32; 3] = [0.106, 0.106, 0.122];
pub const ACCENT: [f32; 3] = [0.290, 0.416, 0.522];
pub const MUTED: [f32; 3] = [0.357, 0.357, 0.4];
pub const RULE: [f32; 3] = [0.851, 0.851, 0.878];
pub const CODE_BG: [f32; 3] = [0.961, 0.961, 0.973];

const ASCENT: f64 = 0.72;
const DESCENT: f64 = 0.22;
const LEADING: f64 = 1.24;
const CLUB_PENALTY: f64 = 150.0;
const WIDOW_PENALTY: f64 = 150.0;
const SECTION_PENALTY: f64 = -300.0;
const DEPLORABLE: f64 = 100000.0;

#[derive(Clone)]
pub enum Draw {
    Text { x: f64, y: f64, s: String, style: Style, size: f64, scale: f64, color: [f32; 3] },
    Math { x: f64, y: f64, size: f64, lbox: Rc<LBox>, color: [f32; 3] },
    Rect { x: f64, y: f64, w: f64, h: f64, color: [f32; 3] },
    Qed { x: f64, y: f64, a: f64 },
}

impl Draw {
    fn shifted(&self, dx: f64, dy: f64) -> Draw {
        match self.clone() {
            Draw::Text { x, y, s, style, size, scale, color } => Draw::Text { x: x + dx, y: y + dy, s, style, size, scale, color },
            Draw::Math { x, y, size, lbox, color } => Draw::Math { x: x + dx, y: y + dy, size, lbox, color },
            Draw::Rect { x, y, w, h, color } => Draw::Rect { x: x + dx, y: y + dy, w, h, color },
            Draw::Qed { x, y, a } => Draw::Qed { x: x + dx, y: y + dy, a },
        }
    }
}

pub struct Page {
    pub draws: Vec<Draw>,
}

pub struct Layout {
    pub width: f64,
    pub height: f64,
    pub pages: Vec<Page>,
    pub title: Option<String>,
    pub author: Option<String>,
}

enum V {
    Box { h: f64, d: f64, draws: Vec<Draw> },
    Glue { w: f64, stretch: f64, shrink: f64 },
    Penalty(f64),
}

pub fn page_height_pt(page: &str) -> f64 {
    match page {
        "a5" => 595.28,
        "letter" => 792.0,
        _ => 841.89,
    }
}

struct P<'a> {
    r: R<'a>,
    v: Vec<V>,
    size: f64,
    measure: f64,
    indent: f64,
    base: Style,
}

pub fn layout(doc: &Doc, diags: &mut Diagnostics) -> Layout {
    let s = &doc.settings;
    let width = page_width_pt(&s.page);
    let height = page_height_pt(&s.page);
    let margin = length_pt(&s.margin).unwrap_or(72.0);
    let size = length_pt(&s.text_size).unwrap_or(11.0);
    let measure = (width - 2.0 * margin).max(72.0);
    let r = R {
        env: &doc.env,
        labels: &doc.labels,
        diags,
        out: String::new(),
        measure: Some(measure / size),
        base: Style::default(),
        indent: 0.0,
        in_figure: false,
    };
    let mut p = P { r, v: Vec::new(), size, measure, indent: 0.0, base: Style::default() };
    p.title(doc);
    for b in &doc.blocks {
        p.block(b);
    }
    let goal = height - 2.0 * margin;
    let pages = p.paginate(goal, margin, width, height);
    Layout { width, height, pages, title: doc.meta.title.clone(), author: if doc.meta.authors.is_empty() { None } else { Some(doc.meta.authors.join(", ")) } }
}

impl P<'_> {
    fn glue(&mut self, w: f64, stretch: f64, shrink: f64) {
        if let Some(V::Glue { w: pw, stretch: ps, shrink: ph }) = self.v.last_mut() {
            if w > *pw {
                (*pw, *ps, *ph) = (w, stretch, shrink);
            }
            return;
        }
        self.v.push(V::Glue { w, stretch, shrink });
    }

    fn penalty(&mut self, p: f64) {
        self.v.push(V::Penalty(p));
    }

    fn last_depth(&self) -> Option<f64> {
        self.v.iter().rev().find_map(|x| if let V::Box { d, .. } = x { Some(*d) } else { None })
    }

    fn push_box(&mut self, h: f64, d: f64, draws: Vec<Draw>, leading: f64) {
        if let Some(pd) = self.last_depth() {
            let gap = leading - pd - h;
            let w = if gap < 0.0 { 0.1 * self.size } else { gap };
            self.v.push(V::Glue { w, stretch: 0.0, shrink: 0.0 });
        }
        self.v.push(V::Box { h, d, draws });
    }

    fn line_box(&self, line: &PLine, size: f64, x0: f64, color: [f32; 3]) -> (f64, f64, Vec<Draw>) {
        let mut h = ASCENT * size;
        let mut d = DESCENT * size;
        let mut draws = Vec::new();
        for it in &line.items {
            match it {
                PItem::Text { x, s, style, scale, link } => {
                    let fs = if style.mono { size * text::CODE_SCALE } else { size };
                    draws.push(Draw::Text {
                        x: x0 + x * size,
                        y: 0.0,
                        s: s.clone(),
                        style: *style,
                        size: fs,
                        scale: *scale,
                        color: if *link { ACCENT } else { color },
                    });
                }
                PItem::Math { x, lbox, link } => {
                    h = h.max(lbox.h * size / 1000.0);
                    d = d.max(lbox.d * size / 1000.0);
                    draws.push(Draw::Math { x: x0 + x * size, y: 0.0, size, lbox: lbox.clone(), color: if *link { ACCENT } else { color } });
                }
            }
        }
        (h, d, draws)
    }

    fn lines(&mut self, lines: Vec<PLine>, size: f64, x0: f64, color: [f32; 3], extra: Option<Draw>) {
        let n = lines.len();
        let last = n.saturating_sub(1);
        for (i, line) in lines.iter().enumerate() {
            let (h, d, mut draws) = self.line_box(line, size, x0, color);
            if i == last {
                if let Some(e) = &extra {
                    draws.push(e.clone());
                }
            }
            if i > 0 {
                let mut p = 0.0;
                if i == 1 {
                    p += CLUB_PENALTY;
                }
                if i == last {
                    p += WIDOW_PENALTY;
                }
                if n == 2 {
                    p = INF;
                }
                if p != 0.0 {
                    self.penalty(p);
                }
            }
            self.push_box(h, d, draws, LEADING * size);
        }
    }

    fn par_skip(&mut self) {
        let s = self.size;
        self.glue(0.6 * s, 0.2 * s, 0.1 * s);
    }

    fn builder(&self) -> Builder {
        Builder::new(self.base)
    }

    fn para_with(&mut self, c: &[Inline], prefix: &[(String, Style, Wrap)], extra: Option<Draw>) {
        let mut b = self.builder();
        for (t, st, w) in prefix {
            b.push(*st, Some(w.clone()));
            b.text(t);
            b.pop();
        }
        if !prefix.is_empty() {
            b.text(" ");
        }
        self.r.build(&mut b, c);
        if b.is_empty() {
            return;
        }
        let m = (self.measure - self.indent) / self.size;
        let lines = b.place(m, true, true);
        let x0 = self.indent;
        self.par_skip();
        let extra = extra.map(|e| e.shifted(self.measure, 0.0));
        self.lines(lines, self.size, x0, INK, extra);
        self.par_skip();
    }

    fn centered_text(&mut self, t: &str, style: Style, size: f64, color: [f32; 3]) {
        let mut b = Builder::new(style);
        b.text(t);
        self.centered_builder(b, size, color);
    }

    fn centered_builder(&mut self, b: Builder, size: f64, color: [f32; 3]) {
        let measure = self.measure / size;
        let (items, w) = b.natural();
        let x0 = if w <= measure { (self.measure - w * size) / 2.0 } else { 0.0 };
        self.lines(vec![PLine { items }], size, x0, color, None);
    }

    fn title(&mut self, doc: &Doc) {
        let m = &doc.meta;
        if m.title.is_none() && m.authors.is_empty() && m.date.is_none() {
            return;
        }
        let s = self.size;
        if let Some(t) = &m.title {
            self.centered_text(t, Style::default(), 1.7 * s, INK);
            self.glue(0.5 * s, 0.0, 0.0);
        }
        if !m.authors.is_empty() {
            self.centered_text(&m.authors.join(", "), Style::default(), s, INK);
        }
        if let Some(d) = &m.date {
            self.centered_text(d, Style::default(), s, MUTED);
        }
        self.glue(2.0 * s, 0.5 * s, 0.3 * s);
    }

    fn block(&mut self, b: &Block) {
        let s = self.size;
        match b {
            Block::Heading { level, content, number, .. } => {
                let k = match level {
                    1 => 1.3,
                    2 => 1.12,
                    _ => 1.0,
                };
                let hs = s * k;
                self.penalty(SECTION_PENALTY);
                self.glue(1.4 * hs, 0.3 * hs, 0.2 * hs);
                let bold = Style { bold: true, ..Style::default() };
                let mut bld = Builder::new(bold);
                if let Some(n) = number {
                    bld.text(&format!("{n}\u{2003}"));
                }
                self.r.build(&mut bld, content);
                let m = self.measure / hs;
                let (items, w) = bld.natural();
                if w <= m {
                    self.lines(vec![PLine { items }], hs, 0.0, INK, None);
                } else {
                    let mut again = Builder::new(bold);
                    if let Some(n) = number {
                        again.text(&format!("{n}\u{2003}"));
                    }
                    self.r.build(&mut again, content);
                    let lines = again.place(m, false, false);
                    self.lines(lines, hs, 0.0, INK, None);
                }
                self.penalty(INF);
                self.glue(0.5 * hs, 0.1 * hs, 0.0);
            }
            Block::Para(c) => self.para_with(c, &[], None),
            Block::Math { src, line, col, number, .. } => self.display(src, *line, *col, *number),
            Block::List { ordered, items } => {
                self.par_skip();
                let saved = self.indent;
                self.indent += 2.5 * s;
                for (i, it) in items.iter().enumerate() {
                    let label = if *ordered { format!("{}.", i + 1) } else { "\u{2022}".to_string() };
                    let lw = text::width(&label, self.base) * s;
                    let tag = Draw::Text { x: self.indent - 0.6 * s - lw, y: 0.0, s: label, style: self.base, size: s, scale: 1.0, color: INK };
                    let mut b = self.builder();
                    self.r.build(&mut b, it);
                    let m = (self.measure - self.indent) / s;
                    let mut lines = b.place(m, true, true);
                    if lines.is_empty() {
                        lines.push(PLine { items: Vec::new() });
                    }
                    let first = lines.remove(0);
                    let (h, d, mut draws) = self.line_box(&first, s, self.indent, INK);
                    draws.push(tag);
                    if i > 0 {
                        self.glue(0.2 * s, 0.1 * s, 0.0);
                    }
                    self.push_box(h, d, draws, LEADING * s);
                    let x0 = self.indent;
                    self.lines(lines, s, x0, INK, None);
                }
                self.indent = saved;
                self.par_skip();
            }
            Block::Table { align, header, rows } => self.table(align, header, rows),
            Block::Code { text, .. } => self.code(text),
            Block::Env(e) => self.env(e),
            Block::Rule => {
                self.glue(0.8 * s, 0.2 * s, 0.2 * s);
                let draws = vec![Draw::Rect { x: 0.0, y: 0.0, w: self.measure, h: 0.5, color: RULE }];
                self.v.push(V::Box { h: 0.5, d: 0.0, draws });
                self.glue(0.8 * s, 0.2 * s, 0.2 * s);
            }
        }
    }

    fn display(&mut self, src: &str, line: usize, col: usize, number: Option<usize>) {
        let s = self.size;
        let pos = math_pos(src, line, col);
        let node = {
            let mut ctx = math::Ctx { env: self.r.env, diags: self.r.diags, pos: &pos, display: true };
            math::parse(src, &mut ctx)
        };
        let b = math::display_box(&node);
        let w = b.w * s / 1000.0;
        let h = (b.h * s / 1000.0).max(ASCENT * s);
        let d = (b.d * s / 1000.0).max(DESCENT * s);
        let x = self.indent + (self.measure - self.indent - w) / 2.0;
        let mut draws = vec![Draw::Math { x, y: 0.0, size: s, lbox: Rc::new(b), color: INK }];
        if let Some(n) = number {
            let label = format!("({n})");
            let up = Style::default();
            let lw = text::width(&label, up) * s;
            draws.push(Draw::Text { x: self.measure - lw, y: 0.0, s: label, style: up, size: s, scale: 1.0, color: INK });
        }
        self.glue(0.9 * s, 0.3 * s, 0.5 * s);
        self.v.push(V::Box { h, d, draws });
        self.glue(0.9 * s, 0.3 * s, 0.5 * s);
    }

    fn code(&mut self, text: &str) {
        let s = self.size;
        let fs = 0.9 * s;
        let pad = 0.7 * fs;
        let lead = LEADING * fs;
        let lines: Vec<&str> = text.lines().collect();
        self.par_skip();
        let mono = Style { mono: true, ..Style::default() };
        let n = lines.len().max(1);
        for (i, l) in lines.iter().enumerate() {
            let top = if i == 0 { pad } else { 0.0 };
            let bottom = if i + 1 == n { pad } else { 0.0 };
            let h = ASCENT * fs + top;
            let d = (lead - ASCENT * fs) + bottom;
            let draws = vec![
                Draw::Rect { x: 0.0, y: -d, w: self.measure, h: h + d, color: CODE_BG },
                Draw::Text { x: 1.0 * fs, y: 0.0, s: l.to_string(), style: mono, size: fs / text::CODE_SCALE * text::CODE_SCALE, scale: 1.0, color: INK },
            ];
            if i > 0 {
                self.penalty(0.0);
            }
            self.v.push(V::Box { h, d, draws });
        }
        self.par_skip();
    }

    fn table(&mut self, align: &[Align], header: &[Vec<Inline>], rows: &[Vec<Vec<Inline>>]) {
        let s = self.size;
        let pad = 0.8 * s;
        let bold = Style { bold: true, ..self.base };
        let mut cells: Vec<Vec<(Vec<PItem>, f64)>> = Vec::new();
        let mut hdr = Vec::new();
        for c in header {
            let mut b = Builder::new(bold);
            self.r.build(&mut b, c);
            let (it, w) = b.natural();
            hdr.push((it, w * s));
        }
        cells.push(hdr);
        for row in rows {
            let mut out = Vec::new();
            for c in row {
                let mut b = Builder::new(self.base);
                self.r.build(&mut b, c);
                let (it, w) = b.natural();
                out.push((it, w * s));
            }
            cells.push(out);
        }
        let ncol = cells.iter().map(Vec::len).max().unwrap_or(0);
        let mut widths = vec![0.0f64; ncol];
        for row in &cells {
            for (j, (_, w)) in row.iter().enumerate() {
                widths[j] = widths[j].max(*w);
            }
        }
        let total: f64 = widths.iter().map(|w| w + 2.0 * pad).sum();
        let x0 = ((self.measure - total) / 2.0).max(0.0);
        let row_h = 0.25 * s + ASCENT * s;
        let row_d = 0.25 * s + DESCENT * s;
        self.par_skip();
        self.glue(1.0 * s, 0.2 * s, 0.1 * s);
        for (i, row) in cells.into_iter().enumerate() {
            let mut draws = Vec::new();
            let mut x = x0;
            for (j, (items, w)) in row.into_iter().enumerate() {
                let cw = widths[j];
                let off = match align.get(j) {
                    Some(Align::Center) => (cw - w) / 2.0,
                    Some(Align::Right) => cw - w,
                    _ => 0.0,
                };
                let (_, _, d) = self.line_box(&PLine { items }, s, x + pad + off, INK);
                draws.extend(d);
                x += cw + 2.0 * pad;
            }
            if i == 0 {
                draws.push(Draw::Rect { x: x0, y: row_h, w: total, h: 0.9, color: INK });
                draws.push(Draw::Rect { x: x0, y: -row_d, w: total, h: 0.5, color: INK });
            }
            if i > 0 {
                self.penalty(if i == 1 { INF } else { 0.0 });
            }
            self.v.push(V::Box { h: row_h, d: row_d, draws });
        }
        if let Some(V::Box { draws, d, .. }) = self.v.last_mut() {
            draws.push(Draw::Rect { x: x0, y: -*d, w: total, h: 0.9, color: INK });
        }
        self.glue(1.0 * s, 0.2 * s, 0.1 * s);
        self.par_skip();
    }

    fn env(&mut self, e: &Env) {
        let s = self.size;
        if e.kind == "figure" {
            self.glue(1.0 * s, 0.3 * s, 0.2 * s);
            for b in &e.body {
                match b {
                    Block::Para(c) if c.iter().any(|i| matches!(i, Inline::Image { .. })) => {
                        let alt = c
                            .iter()
                            .find_map(|i| if let Inline::Image { alt, .. } = i { Some(alt.clone()) } else { None })
                            .unwrap_or_default();
                        self.centered_text(&format!("[image: {alt}]"), Style::default(), s, MUTED);
                    }
                    _ => self.block(b),
                }
            }
            let mut b = Builder::new(self.base);
            b.push(Style { bold: true, ..self.base }, None);
            b.text(&format!("Figure\u{A0}{}.", e.number.unwrap_or(0)));
            b.pop();
            if let Some(c) = &e.caption {
                b.text(" ");
                self.r.build(&mut b, c);
            }
            let fs = 0.92 * s;
            let (items, w) = b.natural();
            if w * fs <= self.measure {
                let x0 = (self.measure - w * fs) / 2.0;
                self.lines(vec![PLine { items }], fs, x0, INK, None);
            } else {
                let mut b = Builder::new(self.base);
                b.text(&format!("Figure\u{A0}{}.", e.number.unwrap_or(0)));
                if let Some(c) = &e.caption {
                    b.text(" ");
                    self.r.build(&mut b, c);
                }
                let lines = b.place(self.measure / fs, true, true);
                self.lines(lines, fs, 0.0, INK, None);
            }
            self.glue(1.0 * s, 0.3 * s, 0.2 * s);
            return;
        }
        let mut name = capital(&e.kind);
        if let Some(n) = e.number {
            name.push_str(&format!("\u{A0}{n}"));
        }
        let saved = (self.base, self.indent, self.measure);
        if matches!(e.kind.as_str(), "theorem" | "lemma" | "corollary" | "proposition") {
            self.base.italic = true;
        }
        match e.kind.as_str() {
            "quote" => {
                self.indent += 2.0 * s;
                self.measure -= 2.0 * s;
            }
            "note" => self.indent += 1.2 * s,
            _ => {}
        }
        let name_style = if e.kind == "proof" { Style { italic: true, ..Style::default() } } else { Style { bold: true, ..Style::default() } };
        let name_wrap = Wrap { open: String::new(), close: "" };
        let mut prefix = vec![(name, name_style, name_wrap.clone())];
        if let Some(t) = &e.title {
            prefix.push((format!(" ({t})"), Style::default(), name_wrap.clone()));
        }
        prefix.push((".".into(), name_style, name_wrap));
        self.glue(0.4 * s, 0.2 * s, 0.1 * s);
        let qed = if e.kind == "proof" {
            Some(Draw::Qed { x: -0.6 * s, y: 0.0, a: 0.6 * s })
        } else {
            None
        };
        let n = e.body.len();
        let mut started = false;
        for (i, b) in e.body.iter().enumerate() {
            let extra = if i + 1 == n { qed.clone() } else { None };
            match b {
                Block::Para(c) => {
                    if started {
                        self.para_with(c, &[], extra);
                    } else {
                        self.para_with(c, &prefix, extra);
                    }
                }
                other => {
                    if !started {
                        self.para_with(&[], &prefix, None);
                    }
                    self.block(other);
                }
            }
            started = true;
        }
        if !started {
            self.para_with(&[], &prefix, qed);
        }
        self.glue(0.4 * s, 0.2 * s, 0.1 * s);
        (self.base, self.indent, self.measure) = saved;
    }

    fn paginate(&mut self, goal: f64, margin: f64, width: f64, height: f64) -> Vec<Page> {
        let v = std::mem::take(&mut self.v);
        let mut pages = Vec::new();
        let mut start = 0;
        let n = v.len();
        while start < n {
            while start < n && !matches!(v[start], V::Box { .. }) {
                start += 1;
            }
            if start >= n {
                break;
            }
            let mut t = 0.0;
            let mut stretch = 0.0;
            let mut shrink = 0.0;
            let mut prev_d = 0.0;
            let mut best: Option<(usize, f64)> = None;
            let mut end = n;
            let mut i = start;
            while i < n {
                let legal = match &v[i] {
                    V::Penalty(p) => *p < INF,
                    V::Glue { .. } => i > start && matches!(v[i - 1], V::Box { .. }),
                    V::Box { .. } => false,
                };
                if legal {
                    let pi = if let V::Penalty(p) = v[i] { p } else { 0.0 };
                    let total = t + prev_d;
                    let b = if total <= goal {
                        if stretch > 0.0 {
                            (100.0 * ((goal - total) / stretch).powi(3)).min(INF)
                        } else if goal - total < 0.5 {
                            0.0
                        } else {
                            INF
                        }
                    } else if total - shrink > goal {
                        f64::INFINITY
                    } else {
                        (100.0 * ((total - goal) / shrink).powi(3)).min(INF)
                    };
                    let c = if b.is_infinite() {
                        f64::INFINITY
                    } else if pi <= -INF {
                        pi
                    } else if b < INF {
                        b + pi
                    } else {
                        DEPLORABLE
                    };
                    if c <= best.map_or(f64::INFINITY, |x| x.1) {
                        best = Some((i, c));
                    }
                    if c.is_infinite() || pi <= -INF {
                        end = best.map_or(i, |x| x.0);
                        if end == start {
                            end = i.max(start + 1);
                        }
                        break;
                    }
                }
                match &v[i] {
                    V::Box { h, d, .. } => {
                        t += prev_d + h;
                        prev_d = *d;
                    }
                    V::Glue { w, stretch: st, shrink: sh } => {
                        t += w;
                        stretch += st;
                        shrink += sh;
                    }
                    V::Penalty(_) => {}
                }
                i += 1;
            }
            if i >= n {
                end = n;
            }
            let mut draws = Vec::new();
            let mut y = height - margin;
            let mut first = true;
            for item in &v[start..end] {
                match item {
                    V::Box { h, d, draws: ds } => {
                        y -= h;
                        for dr in ds {
                            draws.push(dr.shifted(margin, y));
                        }
                        y -= d;
                        first = false;
                    }
                    V::Glue { w, .. } => {
                        if !first {
                            y -= w;
                        }
                    }
                    V::Penalty(_) => {}
                }
            }
            let num = (pages.len() + 1).to_string();
            let nw = text::width(&num, Style::default()) * self.size;
            draws.push(Draw::Text {
                x: (width - nw) / 2.0,
                y: margin / 2.0,
                s: num,
                style: Style::default(),
                size: self.size,
                scale: 1.0,
                color: INK,
            });
            pages.push(Page { draws });
            start = end;
            if start < n && !matches!(v[start], V::Box { .. }) {
                start += 1;
            }
        }
        if pages.is_empty() {
            pages.push(Page { draws: Vec::new() });
        }
        pages
    }
}
