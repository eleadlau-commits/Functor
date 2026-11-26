use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use crate::ast::{Block, Inline};
use crate::diag::{Diagnostic, Diagnostics};
use crate::doc::{self, Doc};
use crate::glyph::{self, CacheStats};
use crate::json;
use crate::lint;
use crate::render::{self, has_notes, Flow, Frame};
use crate::trace::{self, Trace};

const KEEP: u64 = 4;

struct Piece {
    html: String,
    diags: Vec<Diagnostic>,
    traces: Vec<Trace>,
    at: usize,
    flow: Flow,
    used: u64,
}

pub struct Part {
    pub key: u64,
    pub line: usize,
    pub end_line: usize,
    pub fresh: bool,
}

pub struct Update {
    pub open: String,
    pub parts: Vec<Part>,
    pub diagnostics: Vec<Diagnostic>,
    pub stats: CacheStats,
    pub rendered: usize,
    pub reused: usize,
    json: String,
}

#[derive(Default)]
pub struct Session {
    pieces: HashMap<u64, Piece>,
    sent: HashSet<u64>,
    gen: u64,
}

impl Session {
    pub fn new() -> Self {
        Session::default()
    }

    pub fn reset(&mut self) {
        self.sent.clear();
    }

    pub fn html(&self, u: &Update) -> String {
        let mut out = u.open.clone();
        for p in &u.parts {
            out.push_str(self.pieces.get(&p.key).map_or("", |x| x.html.as_str()));
        }
        out.push_str("</article>");
        out
    }

    pub fn traces(&self, u: &Update) -> Vec<Trace> {
        u.parts.iter().flat_map(|p| self.pieces.get(&p.key).map(|x| moved(&x.traces, x.at, p.line)).unwrap_or_default()).collect()
    }

    pub fn compile(&mut self, src: &str, width_pt: Option<f64>) -> Update {
        self.gen += 1;
        let gen = self.gen;
        glyph::take_stats();
        let mut diags = Diagnostics::default();
        let d = doc::parse(src, &mut diags);
        let f = render::frame(&d, width_pt);
        let ctx = context(&d, &f);
        let mut parts = Vec::with_capacity(d.blocks.len() + 1);
        let (mut rendered, mut reused) = (0, 0);
        if !f.title.is_empty() {
            let key = hash_of(&("title", &f.title));
            self.pieces
                .entry(key)
                .or_insert_with(|| Piece { html: f.title.clone(), diags: Vec::new(), traces: Vec::new(), at: 1, flow: Flow::default(), used: 0 })
                .used = gen;
            parts.push(Part { key, line: 1, end_line: 1, fresh: false });
        }
        let mut flow = Flow::default();
        let mut twins: Vec<(usize, Vec<Trace>)> = Vec::new();
        for (i, b) in d.blocks.iter().enumerate() {
            let (line, end_line) = d.spans.get(i).copied().unwrap_or((1, 1));
            let rel = rebase(b, line);
            let key = block_key(&rel, ctx, flow, &d);
            let piece = match self.pieces.get_mut(&key) {
                Some(p) => {
                    reused += 1;
                    p
                }
                None => {
                    rendered += 1;
                    let mut bd = Diagnostics::default();
                    trace::start();
                    let mut st = flow;
                    let html = render::render_block(&d, &f, &mut bd, &rel, &mut st);
                    let traces = trace::finish();
                    let grew = Flow { after_par: st.after_par, notes: st.notes - flow.notes };
                    self.pieces.entry(key).or_insert(Piece { html, diags: bd.items, traces, at: 1, flow: grew, used: 0 })
                }
            };
            if piece.used == gen {
                twins.push((parts.len(), moved(&piece.traces, piece.at, line)));
            } else if piece.at != line {
                for t in &mut piece.traces {
                    t.line = t.line + line - piece.at;
                    t.end_line = t.end_line + line - piece.at;
                }
                piece.at = line;
            }
            piece.used = gen;
            flow = Flow { after_par: piece.flow.after_par, notes: flow.notes + piece.flow.notes };
            let off = line - 1;
            diags.items.extend(piece.diags.iter().map(|x| Diagnostic { line: x.line + off, ..x.clone() }));
            parts.push(Part { key, line, end_line, fresh: false });
        }
        {
            let mut all: Vec<&Trace> = Vec::new();
            let mut tw = twins.iter().peekable();
            for (i, p) in parts.iter().enumerate() {
                match tw.peek() {
                    Some((j, t)) if *j == i => {
                        all.extend(t.iter());
                        tw.next();
                    }
                    _ => all.extend(self.pieces.get(&p.key).map(|x| x.traces.iter()).into_iter().flatten()),
                }
            }
            lint::lint(&all, &d.env, &mut diags);
        }
        self.pieces.retain(|_, p| p.used + KEEP > gen);
        let mut sent = HashSet::with_capacity(parts.len());
        for p in &mut parts {
            p.fresh = !self.sent.contains(&p.key);
            sent.insert(p.key);
        }
        self.sent = sent;
        let diagnostics = diags.finish();
        let stats = glyph::take_stats();
        let json = self.to_json(&f.open, &parts, &diagnostics, &stats, rendered, reused);
        Update { open: f.open, parts, diagnostics, stats, rendered, reused, json }
    }

    fn to_json(&self, open: &str, parts: &[Part], diagnostics: &[Diagnostic], stats: &CacheStats, rendered: usize, reused: usize) -> String {
        let parts = json::array(parts.iter().map(|p| {
            let o = json::Obj::new().str("key", &format!("{:016x}", p.key)).num("line", p.line).num("endLine", p.end_line);
            match self.pieces.get(&p.key).filter(|_| p.fresh) {
                Some(x) => o.str("html", &x.html).raw("traces", json::array(moved(&x.traces, x.at, 1).iter().map(crate::trace_json))).end(),
                None => o.end(),
            }
        }));
        let stats = json::Obj::new()
            .num("built", stats.built)
            .num("hits", stats.hits)
            .num("fromFont", stats.from_font)
            .num("rendered", rendered)
            .num("reused", reused)
            .end();
        json::Obj::new().str("open", open).raw("parts", parts).raw("diagnostics", crate::diags_json(diagnostics)).raw("stats", stats).end()
    }
}

impl Update {
    pub fn to_json(&self) -> &str {
        &self.json
    }
}

fn hash_of<T: Hash>(t: &T) -> u64 {
    let mut h = DefaultHasher::new();
    t.hash(&mut h);
    h.finish()
}

fn context(d: &Doc, f: &Frame) -> u64 {
    let mut h = DefaultHasher::new();
    f.open.hash(&mut h);
    f.measure.map(f64::to_bits).hash(&mut h);
    format!("{:?}", d.settings).hash(&mut h);
    let e = &d.env;
    e.vec_style.hash(&mut h);
    for m in [&e.syms, &e.objects, &e.kinds] {
        let mut v: Vec<_> = m.iter().collect();
        v.sort();
        v.hash(&mut h);
    }
    h.finish()
}

fn block_key(rel: &Block, ctx: u64, flow: Flow, d: &Doc) -> u64 {
    let mut h = DefaultHasher::new();
    ctx.hash(&mut h);
    rel.hash(&mut h);
    flow.after_par.hash(&mut h);
    if has_notes(std::slice::from_ref(rel)) {
        flow.notes.hash(&mut h);
    }
    let mut refs = Vec::new();
    block_refs(rel, &mut refs);
    let mut missing = false;
    for r in refs {
        r.hash(&mut h);
        match d.labels.get(r) {
            Some(t) => t.hash(&mut h),
            None => missing = true,
        }
    }
    if missing {
        let mut keys: Vec<_> = d.labels.keys().collect();
        keys.sort();
        keys.hash(&mut h);
    }
    h.finish()
}

fn block_refs<'a>(b: &'a Block, out: &mut Vec<&'a str>) {
    match b {
        Block::Heading { content, .. } | Block::Para(content) => inline_refs(content, out),
        Block::List { items, .. } => items.iter().for_each(|i| inline_refs(i, out)),
        Block::Table { header, rows, .. } => header.iter().chain(rows.iter().flatten()).for_each(|c| inline_refs(c, out)),
        Block::Env(e) => {
            e.body.iter().for_each(|b| block_refs(b, out));
            if let Some(c) = &e.caption {
                inline_refs(c, out);
            }
        }
        Block::Math { .. } | Block::Code { .. } | Block::Rule => {}
    }
}

fn inline_refs<'a>(v: &'a [Inline], out: &mut Vec<&'a str>) {
    for i in v {
        match i {
            Inline::Ref { label, .. } => out.push(label),
            Inline::Strong(v) | Inline::Emph(v) | Inline::Note(v) | Inline::Link { text: v, .. } => inline_refs(v, out),
            _ => {}
        }
    }
}

fn rebase(b: &Block, line: usize) -> Block {
    let mut b = b.clone();
    shift_block(&mut b, line.saturating_sub(1));
    b
}

fn shift_block(b: &mut Block, off: usize) {
    match b {
        Block::Heading { content, .. } | Block::Para(content) => shift_inlines(content, off),
        Block::Math { line, .. } => *line = line.saturating_sub(off),
        Block::List { items, .. } => items.iter_mut().for_each(|i| shift_inlines(i, off)),
        Block::Table { header, rows, .. } => header.iter_mut().chain(rows.iter_mut().flatten()).for_each(|c| shift_inlines(c, off)),
        Block::Env(e) => {
            e.body.iter_mut().for_each(|b| shift_block(b, off));
            if let Some(c) = &mut e.caption {
                shift_inlines(c, off);
            }
        }
        Block::Code { .. } | Block::Rule => {}
    }
}

fn shift_inlines(v: &mut [Inline], off: usize) {
    for i in v {
        match i {
            Inline::Math { line, .. } | Inline::Ref { line, .. } => *line = line.saturating_sub(off),
            Inline::Strong(v) | Inline::Emph(v) | Inline::Note(v) | Inline::Link { text: v, .. } => shift_inlines(v, off),
            _ => {}
        }
    }
}

fn moved(t: &[Trace], from: usize, to: usize) -> Vec<Trace> {
    t.iter().map(|x| Trace { line: x.line + to - from, end_line: x.end_line + to - from, ..x.clone() }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn same(s: &mut Session, src: &str, w: Option<f64>) -> Update {
        let full = crate::compile_width(src, w);
        let u = s.compile(src, w);
        assert_eq!(s.html(&u), full.html);
        assert_eq!(u.diagnostics, full.diagnostics);
        assert_eq!(s.traces(&u), full.traces);
        u
    }

    #[test]
    fn matches_a_full_compile_through_edits() {
        let sample = include_str!("../../../examples/sample.fn");
        let mut s = Session::new();
        let first = same(&mut s, sample, None);
        assert_eq!(first.reused, 0);
        assert!(first.parts.iter().all(|p| p.fresh));
        let again = same(&mut s, sample, None);
        assert_eq!(again.rendered, 0);
        assert!(again.parts.iter().all(|p| !p.fresh));
        let moved = sample.replacen("# Symbols", "Intro words.\n\n# Symbols", 1);
        let u = same(&mut s, &moved, None);
        assert_eq!(u.rendered, 2, "a new paragraph and the heading after it");
        let eq = moved.replacen("# Results {#sec:results}", "# Results {#sec:results}\n\n$$\nx = 1\n$$ {#eq:new}", 1);
        let u = same(&mut s, &eq, None);
        assert!(u.rendered >= 3 && u.rendered < u.parts.len() / 2, "{} of {}", u.rendered, u.parts.len());
        let typo = eq.replacen("@eq:newton", "@eq:newtn", 1);
        same(&mut s, &typo, None);
        let vec = typo.replacen("object vec: F, v, u", "object vec: F, v", 1);
        let u = same(&mut s, &vec, None);
        assert_eq!(u.reused, 0);
        same(&mut s, &vec, Some(300.0));
        let compact = vec.replacen("let n = 3", "let n = 3\nset document(style: compact)", 1);
        same(&mut s, &compact, None);
        same(&mut s, &compact.replacen("# Vectors", "More text.\n\n# Vectors", 1), None);
        let notes = "---\nfunctor: 1\n---\n\nA.^[One.]\n\n# Gap\n\nB.^[Two.]\n";
        same(&mut s, notes, None);
        same(&mut s, &notes.replace("A.^[One.]", "A."), None);
        let twins = "---\nfunctor: 1\n---\n\nobject vec: v\n\n$v.bold$\n\n# A\n\n$v.bold$\n\n# A\n\n$v.bold$\n";
        same(&mut s, twins, None);
        same(&mut s, &twins.replacen("# A", "Hi.\n\n# A", 1), None);
    }

    #[test]
    fn sends_html_once() {
        let mut s = Session::new();
        let a = s.compile("---\nfunctor: 1\n---\n\nOne.\n\nTwo.\n", None);
        assert!(a.to_json().matches("\"html\"").count() == 2);
        let b = s.compile("---\nfunctor: 1\n---\n\nOne.\n\nTwo!\n", None);
        assert_eq!(b.to_json().matches("\"html\"").count(), 1);
        s.reset();
        let c = s.compile("---\nfunctor: 1\n---\n\nOne.\n\nTwo!\n", None);
        assert_eq!(c.to_json().matches("\"html\"").count(), 2);
    }
}
