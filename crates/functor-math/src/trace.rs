use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use crate::deform::fingerprint;
use crate::glyph::Glyph;
use crate::math::Node;
use crate::symbols::{self, Class};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Form {
    #[default]
    None,
    Name,
    Object,
    Sym,
    Kind,
    Hand,
}

impl Form {
    pub fn as_str(self) -> &'static str {
        match self {
            Form::None => "none",
            Form::Name => "name",
            Form::Object => "object",
            Form::Sym => "sym",
            Form::Kind => "kind",
            Form::Hand => "hand",
        }
    }

    pub fn has_meaning(self) -> bool {
        matches!(self, Form::Object | Form::Sym | Form::Kind)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Trace {
    pub line: usize,
    pub col: usize,
    pub end_line: usize,
    pub end_col: usize,
    pub source: String,
    pub form: Form,
    pub key: String,
    pub meaning: String,
    pub subject: String,
    pub style: Vec<String>,
    pub hand: Vec<String>,
    pub recipe: String,
    pub ops: Vec<String>,
    pub glyph: Source,
    pub shape: Vec<Rc<Glyph>>,
    pub structure: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Source {
    #[default]
    None,
    Font(u32),
    Composite(u32),
    Built,
    Word,
    Fence,
    Joined,
}

impl Source {
    pub fn text(self) -> String {
        match self {
            Source::None => String::new(),
            Source::Font(cp) => format!("Font glyph {}.", cp_text(cp)),
            Source::Composite(cp) => format!("Font glyph {}: the font has this recipe as one glyph.", cp_text(cp)),
            Source::Built => "Built from the recipe and kept in the glyph cache.".into(),
            Source::Word => "Letters from the text font.".into(),
            Source::Fence => "Fence glyphs from the font. They grow to fit the body.".into(),
            Source::Joined => "Built from two glyphs and kept in the glyph cache.".into(),
        }
    }
}

pub fn op_meaning(op: &str) -> &'static str {
    symbols::op_def(op).map_or("repeat or one side", |d| d.meaning)
}

#[derive(Default)]
struct State {
    active: bool,
    paused: usize,
    stack: Vec<Trace>,
    out: Vec<Trace>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn on() -> bool {
    STATE.with(|s| {
        let s = s.borrow();
        s.active && s.paused == 0
    })
}

pub fn start() {
    STATE.with(|s| *s.borrow_mut() = State { active: true, ..State::default() });
}

pub fn finish() -> Vec<Trace> {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let mut out = std::mem::take(&mut s.out);
        *s = State::default();
        if !out.is_sorted_by_key(|t| (t.line, t.col)) {
            out.sort_by_key(|t| (t.line, t.col));
        }
        out.dedup_by(|a, b| a.line == b.line && a.col == b.col && a.source == b.source);
        out
    })
}

pub fn paused<T>(f: impl FnOnce() -> T) -> T {
    STATE.with(|s| s.borrow_mut().paused += 1);
    let r = f();
    STATE.with(|s| s.borrow_mut().paused -= 1);
    r
}

pub fn edit(f: impl FnOnce(&mut Trace)) {
    if !on() {
        return;
    }
    STATE.with(|s| {
        if let Some(t) = s.borrow_mut().stack.last_mut() {
            f(t);
        }
    });
}

pub fn meaning(form: Form, force: bool, f: impl FnOnce() -> (String, String)) {
    edit(|t| {
        if force || t.form == Form::None {
            t.form = form;
            (t.key, t.meaning) = f();
        }
    });
}

pub struct Frame {
    live: bool,
}

impl Frame {
    pub fn new() -> Frame {
        let live = on();
        if live {
            STATE.with(|s| s.borrow_mut().stack.push(Trace::default()));
        }
        Frame { live }
    }

    pub fn finish(mut self, start: (usize, usize), end: (usize, usize), source: &str, node: &Node) {
        if !self.live {
            return;
        }
        self.live = false;
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let Some(mut t) = s.stack.pop() else { return };
            if t.form == Form::None {
                return;
            }
            (t.line, t.col, t.end_line, t.end_col) = (start.0, start.1, end.0, end.1);
            t.source = source.to_string();
            if !t.structure {
                shape(node, &mut t.shape);
            }
            s.out.push(t);
        });
    }
}

impl Default for Frame {
    fn default() -> Self {
        Frame::new()
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        if self.live {
            STATE.with(|s| s.borrow_mut().stack.pop());
        }
    }
}

fn shape(node: &Node, out: &mut Vec<Rc<Glyph>>) {
    match node {
        Node::Glyph { g, .. } => out.push(g.clone()),
        Node::Fenced { open, close, .. } => {
            let none = || Rc::new(Glyph::empty(0.0));
            out.push(open.clone().unwrap_or_else(none));
            out.push(close.clone().unwrap_or_else(none));
        }
        Node::Row(v) if v.len() == 1 => shape(&v[0], out),
        _ => {}
    }
}

pub fn rough_look(t: &Trace) -> Option<u64> {
    if t.shape.is_empty() {
        return None;
    }
    let mut h = DefaultHasher::new();
    for g in &t.shape {
        (g.w.to_bits(), g.h.to_bits(), g.d.to_bits(), g.items.len()).hash(&mut h);
    }
    Some(h.finish())
}

pub fn look(t: &Trace) -> Option<u64> {
    if t.shape.is_empty() {
        return None;
    }
    let mut h = DefaultHasher::new();
    for g in &t.shape {
        fingerprint(g).hash(&mut h);
    }
    Some(h.finish())
}

pub fn kind_text(kind: &str) -> &'static str {
    match kind {
        "abs" => "an absolute value or a size",
        "norm" => "a norm",
        "floor" => "rounding down",
        "ceil" => "rounding up",
        "inner" => "an inner product",
        "binom" => "a binomial coefficient",
        "vec" => "a vector",
        "mat" => "a matrix",
        "cases" => "a choice between cases",
        _ => "a structure",
    }
}

pub fn class_text(c: Class) -> &'static str {
    match c {
        Class::Ord => "plain symbol",
        Class::Op => "large operator",
        Class::Bin => "binary operation",
        Class::Rel => "relation",
        Class::Open => "opening fence",
        Class::Close => "closing fence",
        Class::Punct => "punctuation mark",
    }
}

pub fn cp_text(cp: u32) -> String {
    let c = char::from_u32(cp).map(String::from).unwrap_or_default();
    format!("{c} U+{cp:04X}")
}
