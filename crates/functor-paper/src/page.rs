use std::rc::Rc;

use crate::ast::{Align, Block, Env, Inline};
use crate::diag::Diagnostics;
use crate::doc::Doc;
use crate::linebreak::INF;
use crate::math::{self, LBox};
use crate::render::{capital, columns, has_notes, length_pt, math_pos, page_width_pt, Look, Notes, COLUMN_GAP_PT, NOTE_COLUMN_PT, PAR_INDENT, R};
use crate::text::{self, Builder, PItem, PLine, Style, Wrap};

pub const INK: [f32; 3] = [0.106, 0.106, 0.122];
pub const ACCENT: [f32; 3] = [0.290, 0.416, 0.522];
pub const MUTED: [f32; 3] = [0.357, 0.357, 0.4];
const MARK_RISE: f64 = 0.36;
const NOTE_SIZE: f64 = 0.8;
const NOTE_GAP: f64 = 18.0;
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
    Qed { x: f64, y: f64, a: f64, color: [f32; 3] },
}

impl Draw {
    fn shifted(&self, dx: f64, dy: f64) -> Draw {
        match self.clone() {
            Draw::Text { x, y, s, style, size, scale, color } => Draw::Text { x: x + dx, y: y + dy, s, style, size, scale, color },
            Draw::Math { x, y, size, lbox, color } => Draw::Math { x: x + dx, y: y + dy, size, lbox, color },
            Draw::Rect { x, y, w, h, color } => Draw::Rect { x: x + dx, y: y + dy, w, h, color },
            Draw::Qed { x, y, a, color } => Draw::Qed { x: x + dx, y: y + dy, a, color },
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
    Box { h: f64, d: f64, draws: Vec<Draw>, side: Vec<Side> },
    Glue { w: f64, stretch: f64, shrink: f64 },
    Penalty(f64),
    Open(Span),
    Close,
}

#[derive(Clone)]
struct Side {
    lines: Vec<Vec<Draw>>,
    fs: f64,
    num: usize,
    x: f64,
    left: bool,
    bar: Option<[f32; 3]>,
    color: [f32; 3],
}

impl Side {
    fn asc(&self) -> f64 {
        ASCENT * self.fs
    }

    fn pitch(&self) -> f64 {
        LEADING * self.fs
    }

    fn height(&self, k: usize) -> f64 {
        self.asc() + (k.max(1) - 1) as f64 * self.pitch() + DESCENT * self.fs
    }

    fn fit(&self, room: f64) -> usize {
        let free = room - self.asc() - DESCENT * self.fs;
        if free < 0.0 {
            0
        } else {
            ((free / self.pitch() + 1e-6).floor() as usize + 1).min(self.lines.len())
        }
    }

    fn label(&self) -> Vec<Draw> {
        let s = format!("{} continued", self.num);
        let style = Style { italic: true, ..Style::default() };
        let x = if self.left { -NOTE_GAP - text::width(&s, style) * self.fs } else { self.x };
        vec![Draw::Text { x, y: 0.0, s, style, size: self.fs, scale: 1.0, color: self.color }]
    }

    fn split(mut self, k: usize) -> (Side, Side) {
        let rest = self.lines.split_off(k);
        let mut tail = Side { lines: rest, ..self.clone() };
        tail.lines.insert(0, self.label());
        (self, tail)
    }

    fn draws(&self, top: f64, out: &mut Vec<Draw>) {
        let base = top - self.asc();
        for (j, l) in self.lines.iter().enumerate() {
            out.extend(l.iter().map(|d| d.shifted(0.0, base - j as f64 * self.pitch())));
        }
        if let Some(color) = self.bar {
            let h = self.height(self.lines.len());
            out.push(Draw::Rect { x: -NOTE_GAP + 6.0, y: top - h, w: 1.5, h, color });
        }
    }
}

struct Piece {
    want: f64,
    side: Side,
}

fn place_sides(pieces: Vec<Piece>, hi: f64, lo: f64, gap: f64, out: &mut Vec<Draw>) -> Vec<Piece> {
    let mut pieces = pieces;
    let mut carry = Vec::new();
    let span = |ps: &[Piece]| -> f64 { ps.iter().map(|p| p.side.height(p.side.lines.len()) + gap).sum::<f64>() - gap };
    while !pieces.is_empty() && span(&pieces) > hi - lo + 0.01 {
        let excess = span(&pieces) - (hi - lo);
        let i = (0..pieces.len()).max_by_key(|&i| pieces[i].side.lines.len()).unwrap_or(0);
        let side = &pieces[i].side;
        let total = side.lines.len();
        let mut k = side.fit(side.height(total) - excess);
        if total - k < 2 {
            k = total.saturating_sub(2);
        }
        if k < 2 {
            k = if pieces.len() == 1 && carry.is_empty() { side.fit(hi - lo).max(1) } else { 0 };
        }
        let p = pieces.remove(i);
        if k == 0 || k >= total {
            carry.push(Piece { want: f64::INFINITY, side: p.side });
        } else {
            let (head, tail) = p.side.split(k);
            pieces.insert(i, Piece { want: p.want, side: head });
            carry.push(Piece { want: f64::INFINITY, side: tail });
        }
    }
    let n = pieces.len();
    let h: Vec<f64> = pieces.iter().map(|p| p.side.height(p.side.lines.len())).collect();
    let mut pos = vec![0.0; n];
    let mut ceil = hi;
    for i in 0..n {
        pos[i] = pieces[i].want.min(ceil);
        ceil = pos[i] - h[i] - gap;
    }
    let mut floor = lo;
    for i in (0..n).rev() {
        if pos[i] - h[i] < floor {
            pos[i] = floor + h[i];
        }
        floor = pos[i] + gap;
    }
    let mut ceil = hi;
    for i in 0..n {
        pos[i] = pos[i].min(ceil);
        ceil = pos[i] - h[i] - gap;
        pieces[i].side.draws(pos[i], out);
    }
    carry
}

#[derive(Clone, Copy)]
struct Span {
    x: f64,
    w: f64,
    color: [f32; 3],
    pad: f64,
    behind: bool,
}

#[derive(Clone, Copy)]
struct Pal {
    ink: [f32; 3],
    muted: [f32; 3],
    accent: [f32; 3],
    label: [f32; 3],
    rule: [f32; 3],
    code_bg: [f32; 3],
    tint: [f32; 3],
    paper: Option<[f32; 3]>,
}

fn hex(v: &str) -> Option<[f32; 3]> {
    let h = v.strip_prefix('#')?;
    let full: String = if h.len() == 3 { h.chars().flat_map(|c| [c, c]).collect() } else { h.to_string() };
    if full.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(&full, 16).ok()?;
    Some([((n >> 16) & 255) as f32 / 255.0, ((n >> 8) & 255) as f32 / 255.0, (n & 255) as f32 / 255.0])
}

fn mix(a: [f32; 3], b: [f32; 3], k: f32) -> [f32; 3] {
    [a[0] * k + b[0] * (1.0 - k), a[1] * k + b[1] * (1.0 - k), a[2] * k + b[2] * (1.0 - k)]
}

fn palette(look: Look, accent: Option<&str>) -> Pal {
    let user = accent.and_then(hex);
    if look == Look::Board {
        let paper = [0.118, 0.165, 0.192];
        let accent = user.unwrap_or([0.663, 0.769, 0.855]);
        return Pal {
            ink: [0.910, 0.933, 0.945],
            muted: [0.624, 0.690, 0.729],
            accent,
            label: [0.941, 0.875, 0.627],
            rule: [0.227, 0.294, 0.333],
            code_bg: [0.165, 0.227, 0.267],
            tint: mix(accent, paper, 0.1),
            paper: Some(paper),
        };
    }
    let accent = user.unwrap_or(ACCENT);
    Pal { ink: INK, muted: MUTED, accent, label: accent, rule: RULE, code_bg: CODE_BG, tint: mix(accent, [1.0, 1.0, 1.0], 0.08), paper: None }
}

fn line_width(l: &PLine) -> f64 {
    l.items
        .iter()
        .map(|it| match it {
            PItem::Text { x, s, style, scale, .. } => x + text::width(s, *style) * scale * if style.mono { text::CODE_SCALE } else { 1.0 },
            PItem::Mark { x, s } => x + text::width(s, Style::default()) * text::MARK_SCALE,
            PItem::Math { x, lbox, .. } => x + lbox.w / 1000.0,
        })
        .fold(0.0, f64::max)
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
    side: Vec<(usize, Side)>,
    lead: Option<Draw>,
    tight: bool,
    look: Look,
    c: Pal,
    head: Vec<V>,
}

pub fn layout(doc: &Doc, diags: &mut Diagnostics) -> Layout {
    let s = &doc.settings;
    let width = page_width_pt(&s.page);
    let height = page_height_pt(&s.page);
    let margin = length_pt(&s.margin).unwrap_or(72.0);
    let size = length_pt(&s.text_size).unwrap_or(11.0);
    let look = Look::of(&s.style);
    let size = size * look.scale();
    let col = if look.notes() != Notes::After && has_notes(&doc.blocks) { NOTE_COLUMN_PT } else { 0.0 };
    let full = (width - 2.0 * margin - col).max(72.0);
    let cols = if columns(look, full) { 2 } else { 1 };
    let measure = if cols == 2 { (full - COLUMN_GAP_PT) / 2.0 } else { full };
    let r = R {
        env: &doc.env,
        labels: &doc.labels,
        diags,
        out: String::new(),
        measure: Some(measure / size),
        base: Style::default(),
        indent: 0.0,
        in_figure: false,
        look,
        pdf: true,
        notes: 0,
        note_src: Vec::new(),
        after_par: false,
        top_line: None,
    };
    let c = palette(look, s.accent.as_deref());
    let mut p = P { r, v: Vec::new(), size, measure, indent: 0.0, base: Style::default(), side: Vec::new(), lead: None, tight: false, look, c, head: Vec::new() };
    if cols == 2 {
        p.measure = full;
        p.title(doc);
        p.head = std::mem::take(&mut p.v);
        p.measure = measure;
    } else {
        p.title(doc);
    }
    for b in &doc.blocks {
        p.block(b);
    }
    let goal = height - 2.0 * margin;
    let x0 = margin + if look.notes() == Notes::Left { col } else { 0.0 };
    let pages = p.paginate(&Geo { goal, margin, x0, width, height, cols, col_w: measure });
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

    fn push_box(&mut self, h: f64, d: f64, draws: Vec<Draw>, leading: f64, side: Vec<Side>) {
        if let Some(pd) = self.last_depth() {
            let gap = leading - pd - h;
            let w = if gap < 0.0 { 0.1 * self.size } else { gap };
            self.v.push(V::Glue { w, stretch: 0.0, shrink: 0.0 });
        }
        self.v.push(V::Box { h, d, draws, side });
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
                        color: if *link { self.c.accent } else { color },
                    });
                }
                PItem::Mark { x, s } => {
                    draws.push(Draw::Text {
                        x: x0 + x * size,
                        y: MARK_RISE * size,
                        s: s.clone(),
                        style: Style::default(),
                        size: size * text::MARK_SCALE,
                        scale: 1.0,
                        color: if self.look != Look::Classic { self.c.label } else { color },
                    });
                }
                PItem::Math { x, lbox, link } => {
                    h = h.max(lbox.h * size / 1000.0);
                    d = d.max(lbox.d * size / 1000.0);
                    draws.push(Draw::Math { x: x0 + x * size, y: 0.0, size, lbox: lbox.clone(), color: if *link { self.c.accent } else { color } });
                }
            }
        }
        (h, d, draws)
    }

    fn lines(&mut self, lines: Vec<PLine>, size: f64, x0: f64, color: [f32; 3], extra: Option<Draw>) {
        let n = lines.len();
        let last = n.saturating_sub(1);
        let mut side = std::mem::take(&mut self.side);
        let mut lead = self.lead.take();
        for (i, line) in lines.iter().enumerate() {
            let (h, d, mut draws) = self.line_box(line, size, x0, color);
            let mut here = Vec::new();
            let mut k = 0;
            while k < side.len() {
                if side[k].0 == i || (i == last && side[k].0 > last) {
                    here.push(side.remove(k).1);
                } else {
                    k += 1;
                }
            }
            if let Some(l) = lead.take() {
                draws.push(l);
            }
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
            self.push_box(h, d, draws, LEADING * size, here);
        }
    }

    fn par_skip(&mut self) {
        let s = self.size;
        if self.tight {
            self.tight = false;
            self.glue(0.2 * s, 0.05 * s, 0.0);
            return;
        }
        self.glue(0.6 * s, 0.2 * s, 0.1 * s);
    }

    fn builder(&self) -> Builder {
        Builder::new(self.base)
    }

    fn para_with(&mut self, c: &[Inline], prefix: &[(String, Style, Wrap)], extra: Option<Draw>) {
        let mut b = self.builder();
        let indented = self.r.indent_next(prefix.is_empty() && !c.is_empty());
        if indented {
            b.indent(PAR_INDENT);
        }
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
        let after = self.place_notes(&lines);
        if indented {
            self.par_join();
        } else {
            self.par_skip();
        }
        let extra = extra.map(|e| e.shifted(self.measure, 0.0));
        self.lines(lines, self.size, x0, self.c.ink, extra);
        self.r.after_par = after.is_empty();
        self.emit_after(after, x0);
        self.par_skip();
    }

    fn par_join(&mut self) {
        let s = self.size;
        if let Some(V::Glue { w, stretch, shrink }) = self.v.last_mut() {
            (*w, *stretch, *shrink) = (0.0, 0.1 * s, 0.0);
            return;
        }
        self.v.push(V::Glue { w: 0.0, stretch: 0.1 * s, shrink: 0.0 });
    }

    fn place_notes(&mut self, lines: &[PLine]) -> Vec<Vec<PLine>> {
        let notes = std::mem::take(&mut self.r.note_src);
        let fs = NOTE_SIZE * self.size;
        let mut after = Vec::new();
        let side = self.look.notes();
        for (n, v) in &notes {
            if side == Notes::After {
                after.push(self.note_lines(*n, v, (self.measure - self.indent) / fs));
                continue;
            }
            let key = n.to_string();
            let at = lines.iter().position(|l| l.items.iter().any(|it| matches!(it, PItem::Mark { s, .. } if *s == key))).unwrap_or(0);
            let nl = self.note_lines(*n, v, (NOTE_COLUMN_PT - NOTE_GAP) / fs);
            let mut out = Vec::new();
            for l in &nl {
                let x = if side == Notes::Right { self.measure + NOTE_GAP } else { -NOTE_GAP - line_width(l) * fs };
                out.push(self.line_box(l, fs, x, self.c.muted).2);
            }
            let left = side == Notes::Left;
            let bar = left.then_some(self.c.accent);
            self.side.push((at, Side { lines: out, fs, num: *n, x: self.measure + NOTE_GAP, left, bar, color: self.c.muted }));
        }
        after
    }

    fn emit_after(&mut self, after: Vec<Vec<PLine>>, x0: f64) {
        let fs = NOTE_SIZE * self.size;
        for nl in after {
            self.glue(0.2 * self.size, 0.0, 0.0);
            self.lines(nl, fs, x0, self.c.muted, None);
        }
    }

    fn flush_notes(&mut self) {
        if self.r.note_src.is_empty() {
            return;
        }
        let saved = self.look;
        self.look = Look::Classic;
        let after = self.place_notes(&[]);
        self.look = saved;
        let x0 = self.indent;
        self.emit_after(after, x0);
    }

    fn note_lines(&mut self, n: usize, v: &[Inline], measure: f64) -> Vec<PLine> {
        let mut b = Builder::new(Style::default());
        b.text(&format!("{n}"));
        b.text(" ");
        let flat: Vec<Inline> = v.iter().map(|i| if let Inline::Note(x) = i { Inline::Emph(x.clone()) } else { i.clone() }).collect();
        self.r.build(&mut b, &flat);
        b.place(measure, false, false)
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
        let (ink, muted) = (self.c.ink, self.c.muted);
        if self.look == Look::Classic {
            if let Some(t) = &m.title {
                self.centered_text(t, Style::default(), 1.7 * s, ink);
                self.glue(0.5 * s, 0.0, 0.0);
            }
            if !m.authors.is_empty() {
                self.centered_text(&m.authors.join(", "), Style::default(), s, ink);
            }
            if let Some(d) = &m.date {
                self.centered_text(d, Style::default(), s, muted);
            }
            self.glue(2.0 * s, 0.5 * s, 0.3 * s);
            return;
        }
        let (k, bold) = match self.look {
            Look::Compact => (1.7, true),
            Look::Board => (2.1, false),
            _ => (2.0, false),
        };
        if let Some(t) = &m.title {
            let ts = k * s;
            let mut b = Builder::new(Style { bold, ..Style::default() });
            b.text(t);
            let lines = b.place(self.measure / ts, false, false);
            self.lines(lines, ts, 0.0, ink, None);
            self.glue(0.45 * s, 0.0, 0.0);
        }
        let mut meta: Vec<String> = Vec::new();
        if !m.authors.is_empty() {
            meta.push(m.authors.join(", "));
        }
        if let Some(d) = &m.date {
            meta.push(d.clone());
        }
        if !meta.is_empty() {
            let ms = if self.look == Look::Lecture { 0.9 * s } else { s };
            let mut b = Builder::new(Style::default());
            b.text(&meta.join(" \u{B7} "));
            let lines = b.place(self.measure / ms, false, false);
            self.lines(lines, ms, 0.0, muted, None);
        }
        let rule = match self.look {
            Look::Quiet => Some((0.5, self.c.rule)),
            Look::Compact => Some((1.2, ink)),
            _ => None,
        };
        if let Some((h, color)) = rule {
            self.glue(0.9 * s, 0.0, 0.0);
            let draws = vec![Draw::Rect { x: 0.0, y: 0.0, w: self.measure, h, color }];
            self.v.push(V::Box { h, d: 0.0, draws, side: Vec::new() });
        }
        self.glue(1.2 * s, 0.4 * s, 0.2 * s);
    }

    fn heading_lines(&mut self, style: Style, content: &[Inline], measure: f64) -> Vec<PLine> {
        let (n0, k0) = (self.r.notes, self.r.note_src.len());
        let mut bld = Builder::new(style);
        self.r.build(&mut bld, content);
        let (items, w) = bld.natural();
        if w <= measure {
            return vec![PLine { items }];
        }
        self.r.notes = n0;
        self.r.note_src.truncate(k0);
        let mut again = Builder::new(style);
        self.r.build(&mut again, content);
        again.place(measure, false, false)
    }

    fn block(&mut self, b: &Block) {
        let para = matches!(b, Block::Para(_));
        if !para {
            self.r.after_par = false;
        }
        self.block_inner(b);
        if !para {
            self.r.after_par = false;
        }
    }

    fn block_inner(&mut self, b: &Block) {
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
                let ink = self.c.ink;
                if self.look == Look::Classic {
                    let bold = Style { bold: true, ..Style::default() };
                    let mut pre = Vec::new();
                    if let Some(n) = number {
                        pre.push(Inline::Text(format!("{n}\u{2003}")));
                    }
                    pre.extend(content.iter().cloned());
                    let lines = self.heading_lines(bold, &pre, self.measure / hs);
                    let after = self.place_notes(&lines);
                    self.lines(lines, hs, 0.0, ink, None);
                    self.emit_after(after, 0.0);
                } else {
                    if self.look == Look::Lecture && *level == 1 {
                        let draws = vec![Draw::Rect { x: 0.0, y: 0.0, w: self.measure, h: 0.5, color: self.c.rule }];
                        self.v.push(V::Box { h: 0.5, d: 0.0, draws, side: Vec::new() });
                        self.penalty(INF);
                        self.glue(0.55 * hs, 0.0, 0.0);
                    }
                    let (k, bold, italic) = match self.look {
                        Look::Compact => (1.0, true, false),
                        Look::Board => (k * 1.06, false, *level == 2),
                        _ => (k * 1.04, false, *level == 2),
                    };
                    let hs = s * k;
                    let style = Style { bold, italic, ..Style::default() };
                    let label = number.as_ref().map(|n| if self.look == Look::Lecture && *level == 1 { format!("\u{A7}{n}") } else { n.to_string() });
                    let up = Style { bold, ..Style::default() };
                    let (nx, x0) = match &label {
                        Some(l) => {
                            let w = text::width(l, up) * hs;
                            if self.look == Look::Quiet {
                                (-(w + 0.75 * hs), 0.0)
                            } else {
                                (0.0, w + 0.5 * hs)
                            }
                        }
                        None => (0.0, 0.0),
                    };
                    let lines = self.heading_lines(style, content, (self.measure - x0) / hs);
                    let after = self.place_notes(&lines);
                    if let Some(l) = label {
                        self.lead = Some(Draw::Text { x: nx, y: 0.0, s: l, style: up, size: hs, scale: 1.0, color: self.c.label });
                    }
                    self.lines(lines, hs, x0, ink, None);
                    self.emit_after(after, 0.0);
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
                let ink = self.c.ink;
                for (i, it) in items.iter().enumerate() {
                    let label = if *ordered { format!("{}.", i + 1) } else { "\u{2022}".to_string() };
                    let lw = text::width(&label, self.base) * s;
                    let tag = Draw::Text { x: self.indent - 0.6 * s - lw, y: 0.0, s: label, style: self.base, size: s, scale: 1.0, color: ink };
                    let mut b = self.builder();
                    self.r.build(&mut b, it);
                    let m = (self.measure - self.indent) / s;
                    let mut lines = b.place(m, true, true);
                    if lines.is_empty() {
                        lines.push(PLine { items: Vec::new() });
                    }
                    let after = self.place_notes(&lines);
                    if i > 0 {
                        self.glue(0.2 * s, 0.1 * s, 0.0);
                    }
                    self.lead = Some(tag);
                    let x0 = self.indent;
                    self.lines(lines, s, x0, ink, None);
                    self.emit_after(after, x0);
                }
                self.indent = saved;
                self.par_skip();
            }
            Block::Table { align, header, rows } => self.table(align, header, rows),
            Block::Code { text, .. } => self.code(text),
            Block::Env(e) => self.env(e),
            Block::Rule => {
                self.glue(0.8 * s, 0.2 * s, 0.2 * s);
                let draws = vec![Draw::Rect { x: 0.0, y: 0.0, w: self.measure, h: 0.5, color: self.c.rule }];
                self.v.push(V::Box { h: 0.5, d: 0.0, draws, side: Vec::new() });
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
        let mut draws = vec![Draw::Math { x, y: 0.0, size: s, lbox: Rc::new(b), color: self.c.ink }];
        if let Some(n) = number {
            let label = format!("({n})");
            let up = Style::default();
            let lw = text::width(&label, up) * s;
            let color = if self.look == Look::Classic { self.c.ink } else { self.c.muted };
            draws.push(Draw::Text { x: self.measure - lw, y: 0.0, s: label, style: up, size: s, scale: 1.0, color });
        }
        self.glue(0.9 * s, 0.3 * s, 0.5 * s);
        self.v.push(V::Box { h, d, draws, side: Vec::new() });
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
                Draw::Rect { x: 0.0, y: -d, w: self.measure, h: h + d, color: self.c.code_bg },
                Draw::Text { x: 1.0 * fs, y: 0.0, s: l.to_string(), style: mono, size: fs / text::CODE_SCALE * text::CODE_SCALE, scale: 1.0, color: self.c.ink },
            ];
            if i > 0 {
                self.penalty(0.0);
            }
            self.v.push(V::Box { h, d, draws, side: Vec::new() });
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
        let x0 = if matches!(self.look, Look::Quiet | Look::Lecture) { 0.0 } else { ((self.measure - total) / 2.0).max(0.0) };
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
                let (_, _, d) = self.line_box(&PLine { items }, s, x + pad + off, self.c.ink);
                draws.extend(d);
                x += cw + 2.0 * pad;
            }
            if i == 0 {
                draws.push(Draw::Rect { x: x0, y: row_h, w: total, h: 0.9, color: self.c.ink });
                draws.push(Draw::Rect { x: x0, y: -row_d, w: total, h: 0.5, color: self.c.ink });
            }
            if i > 0 {
                self.penalty(if i == 1 { INF } else { 0.0 });
            }
            self.v.push(V::Box { h: row_h, d: row_d, draws, side: Vec::new() });
        }
        let ink = self.c.ink;
        if let Some(V::Box { draws, d, .. }) = self.v.last_mut() {
            draws.push(Draw::Rect { x: x0, y: -*d, w: total, h: 0.9, color: ink });
        }
        self.flush_notes();
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
                        let muted = self.c.muted;
                        self.centered_text(&format!("[image: {alt}]"), Style::default(), s, muted);
                    }
                    _ => self.block(b),
                }
            }
            let (n0, k0) = (self.r.notes, self.r.note_src.len());
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
                let ink = self.c.ink;
                self.lines(vec![PLine { items }], fs, x0, ink, None);
            } else {
                self.r.notes = n0;
                self.r.note_src.truncate(k0);
                let mut b = Builder::new(self.base);
                b.text(&format!("Figure\u{A0}{}.", e.number.unwrap_or(0)));
                if let Some(c) = &e.caption {
                    b.text(" ");
                    self.r.build(&mut b, c);
                }
                let lines = b.place(self.measure / fs, true, true);
                let ink = self.c.ink;
                self.lines(lines, fs, 0.0, ink, None);
            }
            self.flush_notes();
            self.glue(1.0 * s, 0.3 * s, 0.2 * s);
            return;
        }
        let mut name = capital(&e.kind);
        if let Some(n) = e.number {
            name.push_str(&format!("\u{A0}{n}"));
        }
        let kind = e.kind.as_str();
        let theorem_like = matches!(kind, "theorem" | "lemma" | "corollary" | "proposition");
        let plain = matches!(kind, "definition" | "example" | "remark");
        let headed = self.look.headed() && !matches!(kind, "proof" | "quote" | "note");
        let saved = (self.base, self.indent, self.measure);
        if self.look == Look::Classic && theorem_like {
            self.base.italic = true;
        }
        match kind {
            "quote" => {
                self.indent += 2.0 * s;
                self.measure -= 2.0 * s;
            }
            "note" => self.indent += 1.2 * s,
            _ => {}
        }
        if headed {
            self.glue(0.9 * s, 0.3 * s, 0.2 * s);
            let span = if self.look == Look::Lecture && theorem_like {
                Span { x: self.indent, w: self.measure - self.indent + 0.6 * s, color: self.c.tint, pad: 0.55 * s, behind: true }
            } else {
                Span { x: self.indent, w: 1.5, color: if plain { self.c.rule } else { self.c.label }, pad: 0.15 * s, behind: false }
            };
            self.v.push(V::Open(span));
            self.indent += 1.2 * s;
            let color = if plain { self.c.ink } else { self.c.label };
            let x = self.indent;
            let (mut draws, w) = if self.look == Look::Lecture {
                let st = Style { bold: true, ..Style::default() };
                let fs = 0.92 * s;
                (vec![Draw::Text { x, y: 0.0, s: name.clone(), style: st, size: fs, scale: 1.0, color }], text::width(&name, st) * fs)
            } else {
                self.caps(&name, x, s, color)
            };
            if let Some(t) = &e.title {
                let st = Style { italic: true, ..Style::default() };
                draws.push(Draw::Text { x: x + w + 0.3 * s, y: 0.0, s: format!("({t})"), style: st, size: s, scale: 1.0, color: self.c.muted });
            }
            self.push_box(ASCENT * s, DESCENT * s, draws, LEADING * s, Vec::new());
            self.penalty(INF);
            self.tight = true;
            for b in &e.body {
                match b {
                    Block::Para(c) => self.para_with(c, &[], None),
                    other => self.block(other),
                }
                self.tight = false;
            }
            self.tight = false;
            self.v.push(V::Close);
            self.glue(0.9 * s, 0.3 * s, 0.2 * s);
            (self.base, self.indent, self.measure) = saved;
            return;
        }
        let bar = self.look == Look::Compact && !matches!(kind, "proof" | "quote" | "note");
        self.glue(0.4 * s, 0.2 * s, 0.1 * s);
        if bar {
            self.v.push(V::Open(Span { x: self.indent, w: 1.1, color: if plain { self.c.rule } else { self.c.accent }, pad: 0.1 * s, behind: false }));
            self.indent += 0.7 * s;
        }
        let name_style = if e.kind == "proof" { Style { italic: true, ..Style::default() } } else { Style { bold: true, ..Style::default() } };
        let open = if self.look == Look::Compact && !plain && kind != "proof" { "<a>".to_string() } else { String::new() };
        let name_wrap = Wrap { open, close: "" };
        let mut prefix = vec![(name, name_style, name_wrap.clone())];
        if let Some(t) = &e.title {
            prefix.push((format!(" ({t})"), Style::default(), Wrap { open: String::new(), close: "" }));
        }
        prefix.push((".".into(), name_style, name_wrap));
        let qed = if e.kind == "proof" {
            let color = if self.look == Look::Classic { self.c.ink } else { self.c.label };
            Some(Draw::Qed { x: -0.6 * s, y: 0.0, a: 0.6 * s, color })
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
        if bar {
            self.v.push(V::Close);
        }
        self.glue(0.4 * s, 0.2 * s, 0.1 * s);
        (self.base, self.indent, self.measure) = saved;
    }

    fn caps(&self, name: &str, x0: f64, size: f64, color: [f32; 3]) -> (Vec<Draw>, f64) {
        let st = Style::default();
        let mut x = x0;
        let mut draws = Vec::new();
        for ch in name.chars() {
            let (t, fs) = if ch.is_lowercase() { (ch.to_uppercase().to_string(), size * 0.76) } else { (ch.to_string(), size) };
            let w = text::width(&t, st) * fs;
            if !ch.is_whitespace() {
                draws.push(Draw::Text { x, y: 0.0, s: t, style: st, size: fs, scale: 1.0, color });
            }
            x += w + 0.05 * size;
        }
        (draws, x - x0 - 0.05 * size)
    }

    fn paginate(&mut self, g: &Geo) -> Vec<Page> {
        let v = std::mem::take(&mut self.v);
        let head = std::mem::take(&mut self.head);
        let head_h: f64 = head
            .iter()
            .map(|x| match x {
                V::Box { h, d, .. } => h + d,
                V::Glue { w, .. } => *w,
                _ => 0.0,
            })
            .sum();
        let top = g.height - g.margin;
        let mut frames: Vec<Vec<Draw>> = Vec::new();
        let mut open: Vec<Open> = Vec::new();
        let gap = 0.35 * self.size;
        let mut carry: Vec<Piece> = Vec::new();
        let mut start = 0;
        let n = v.len();
        let prev_box = |i: usize, start: usize| -> bool {
            let mut j = i;
            while j > start {
                j -= 1;
                match v[j] {
                    V::Box { .. } => return true,
                    V::Open(_) | V::Close => continue,
                    _ => return false,
                }
            }
            false
        };
        while start < n {
            while start < n && !matches!(v[start], V::Box { .. }) {
                track(&v[start], &mut open, &mut Vec::new(), &mut Vec::new());
                start += 1;
            }
            if start >= n {
                break;
            }
            let lowered = if frames.len() < g.cols && head_h > 0.0 { head_h } else { 0.0 };
            let goal = g.goal - lowered;
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
                    V::Glue { .. } => i > start && prev_box(i, start),
                    _ => false,
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
                    _ => {}
                }
                i += 1;
            }
            if i >= n {
                end = n;
            }
            let mut draws = Vec::new();
            let mut behind = Vec::new();
            let mut y = top - lowered;
            let mut first = true;
            let hi = y;
            let mut pieces = std::mem::take(&mut carry);
            for o in open.iter_mut() {
                o.top = None;
            }
            for item in &v[start..end] {
                match item {
                    V::Box { h, d, draws: ds, side } => {
                        for o in open.iter_mut() {
                            if o.top.is_none() {
                                o.top = Some(y);
                            }
                        }
                        y -= h;
                        for dr in ds {
                            draws.push(dr.shifted(0.0, y));
                        }
                        for sd in side {
                            pieces.push(Piece { want: y + sd.asc(), side: sd.clone() });
                        }
                        y -= d;
                        for o in open.iter_mut() {
                            o.bottom = y;
                        }
                        first = false;
                    }
                    V::Glue { w, .. } => {
                        if !first {
                            y -= w;
                        }
                    }
                    other => track(other, &mut open, &mut draws, &mut behind),
                }
            }
            for o in &open {
                if let Some(t) = o.top {
                    emit(o, t, &mut draws, &mut behind);
                }
            }
            for o in open.iter_mut() {
                o.top = None;
            }
            carry = place_sides(pieces, hi, g.margin, gap, &mut draws);
            behind.extend(draws);
            frames.push(behind);
            start = end;
            if start < n && !matches!(v[start], V::Box { .. }) {
                track(&v[start], &mut open, &mut Vec::new(), &mut Vec::new());
                start += 1;
            }
        }
        while !carry.is_empty() {
            let mut draws = Vec::new();
            carry = place_sides(std::mem::take(&mut carry), top, g.margin, gap, &mut draws);
            frames.push(draws);
        }
        if frames.is_empty() {
            frames.push(Vec::new());
        }
        let mut head_draws = Vec::new();
        let mut y = top;
        let mut first = true;
        for item in &head {
            match item {
                V::Box { h, d, draws: ds, .. } => {
                    y -= h;
                    head_draws.extend(ds.iter().map(|dr| dr.shifted(0.0, y)));
                    y -= d;
                    first = false;
                }
                V::Glue { w, .. } if !first => y -= w,
                _ => {}
            }
        }
        let mut pages = Vec::new();
        for (k, chunk) in frames.chunks(g.cols).enumerate() {
            let mut draws = Vec::new();
            if let Some(p) = self.c.paper {
                draws.push(Draw::Rect { x: 0.0, y: 0.0, w: g.width, h: g.height, color: p });
            }
            if k == 0 {
                draws.extend(head_draws.iter().map(|dr| dr.shifted(g.x0, 0.0)));
            }
            for (c, fr) in chunk.iter().enumerate() {
                let dx = g.x0 + c as f64 * (g.col_w + COLUMN_GAP_PT);
                draws.extend(fr.iter().map(|dr| dr.shifted(dx, 0.0)));
            }
            if chunk.len() > 1 {
                let lowered = if k == 0 { head_h } else { 0.0 };
                let x = g.x0 + g.col_w + COLUMN_GAP_PT / 2.0 - 0.25;
                draws.push(Draw::Rect { x, y: g.margin, w: 0.5, h: g.goal - lowered, color: self.c.rule });
            }
            let num = (k + 1).to_string();
            let nw = text::width(&num, Style::default()) * self.size;
            draws.push(Draw::Text {
                x: (g.width - nw) / 2.0,
                y: g.margin / 2.0,
                s: num,
                style: Style::default(),
                size: self.size,
                scale: 1.0,
                color: if self.look == Look::Classic { self.c.ink } else { self.c.muted },
            });
            pages.push(Page { draws });
        }
        pages
    }
}

struct Geo {
    goal: f64,
    margin: f64,
    x0: f64,
    width: f64,
    height: f64,
    cols: usize,
    col_w: f64,
}

struct Open {
    span: Span,
    top: Option<f64>,
    bottom: f64,
}

fn track(item: &V, open: &mut Vec<Open>, draws: &mut Vec<Draw>, behind: &mut Vec<Draw>) {
    match item {
        V::Open(span) => open.push(Open { span: *span, top: None, bottom: 0.0 }),
        V::Close => {
            if let Some(o) = open.pop() {
                if let Some(t) = o.top {
                    emit(&o, t, draws, behind);
                }
            }
        }
        _ => {}
    }
}

fn emit(o: &Open, top: f64, draws: &mut Vec<Draw>, behind: &mut Vec<Draw>) {
    let s = o.span;
    let r = Draw::Rect { x: s.x, y: o.bottom - s.pad, w: s.w, h: top - o.bottom + 2.0 * s.pad, color: s.color };
    if s.behind {
        behind.push(r);
    } else {
        draws.push(r);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_x(l: &Layout, word: &str) -> f64 {
        l.pages.iter().flat_map(|p| &p.draws).find_map(|d| if let Draw::Text { x, s, .. } = d { s.starts_with(word).then_some(*x) } else { None }).unwrap()
    }

    #[test]
    fn compact_indents_in_pdf() {
        let src = "---\nfunctor: 1\n---\n\nset document(style: STYLE)\n\n# One\n\nAnn text.\n\nBob text.\n\n$$ x $$\n\nCal text.\n";
        for (st, want) in [("compact", true), ("quiet", false)] {
            let mut diags = Diagnostics::default();
            let d = crate::doc::parse(&src.replace("STYLE", st), &mut diags);
            let l = layout(&d, &mut diags);
            let (a, b, c) = (first_x(&l, "Ann"), first_x(&l, "Bob"), first_x(&l, "Cal"));
            assert_eq!(b - a > 5.0, want, "{st}: {a} {b}");
            assert!((c - a).abs() < 0.5, "{st}: {a} {c}");
        }
    }

    #[test]
    fn margin_notes_do_not_overlap() {
        let src = "---\nfunctor: 1\n---\n\nA.^[One long note that takes a few lines in the narrow margin column of the page.] B.^[Two long notes that take a few lines in the narrow margin column of the page.] C.^[Three.]\n";
        let mut diags = Diagnostics::default();
        let d = crate::doc::parse(src, &mut diags);
        let l = layout(&d, &mut diags);
        let margin = length_pt(&d.settings.margin).unwrap();
        let edge = l.width - margin - NOTE_COLUMN_PT;
        let mut ys: Vec<f64> = l.pages[0]
            .draws
            .iter()
            .filter_map(|x| if let Draw::Text { x, y, .. } = x { (*x > edge).then_some(*y) } else { None })
            .collect();
        ys.sort_by(|a, b| b.partial_cmp(a).unwrap());
        ys.dedup_by(|a, b| (*a - *b).abs() < 0.01);
        assert!(ys.len() >= 5, "{ys:?}");
        let fs = NOTE_SIZE * 11.0;
        for w in ys.windows(2) {
            assert!(w[0] - w[1] >= LEADING * fs - 0.01, "{ys:?}");
        }
    }

    fn long_doc(style: &str) -> String {
        let fill = "A smooth map sends tangent vectors to tangent vectors. The chain rule composes the differentials. ".repeat(4);
        let mut src = format!("---\nfunctor: 1\n---\n\nset document(style: {style})\n\n");
        for _ in 0..5 {
            src += &format!("{fill}\n\n");
        }
        src += &format!("{fill}Foot.^[{}] {fill}\n\n", "A long note that cannot fit in the space left on the page. ".repeat(9));
        src += &format!("{fill}Huge.^[{}] {fill}\n\n", "A note taller than a page must break across pages. ".repeat(70));
        for k in 0..6 {
            src += &format!("{fill}{}\n\n", if k % 2 == 1 { "Small.^[A short note.]" } else { "" });
        }
        src
    }

    #[test]
    fn long_notes_stay_on_the_page() {
        for style in ["quiet", "lecture"] {
            let mut diags = Diagnostics::default();
            let d = crate::doc::parse(&long_doc(style), &mut diags);
            let l = layout(&d, &mut diags);
            let margin = length_pt(&d.settings.margin).unwrap();
            let top = l.height - margin;
            let mut cont = 0;
            for (k, page) in l.pages.iter().enumerate() {
                let num = (k + 1).to_string();
                for dr in &page.draws {
                    if let Draw::Text { y, s, .. } = dr {
                        if *s == num && *y < margin {
                            continue;
                        }
                        assert!(*y >= margin - 3.0 && *y <= top, "{style}: page {} text {s:?} at {y}", k + 1);
                        if s.ends_with("continued") {
                            cont += 1;
                        }
                    }
                }
            }
            assert!(cont >= 2, "{style}: {cont}");
        }
    }
}
