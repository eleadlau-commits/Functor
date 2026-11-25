use std::collections::HashMap;
use std::rc::Rc;

use crate::diag::{did_you_mean, Diagnostics};
use crate::deform;
use crate::glyph::{self, Affine, Glyph};
use crate::symbols::{self, Class, Limits, OpKind};
use crate::trace::{self, Form, Source};

pub const MAX_OPS: usize = 3;
pub const HARD_OPS: usize = 8;
pub const MAX_DEPTH: usize = 48;

#[derive(Clone, Debug, Default)]
pub struct MathEnv {
    pub syms: HashMap<String, String>,
    pub objects: HashMap<String, String>,
    pub vec_style: String,
    pub kinds: HashMap<String, String>,
}

impl MathEnv {
    pub fn new() -> Self {
        MathEnv { vec_style: "arrow".into(), ..Default::default() }
    }

    pub fn opt(&self, kind: &str, key: &str) -> String {
        if let Some(v) = self.kinds.get(&format!("{kind}.{key}")) {
            return v.clone();
        }
        if let Some((_, _, v)) = symbols::KIND_DEFAULTS.iter().find(|(k, o, _)| *k == kind && *o == key) {
            return v.to_string();
        }
        if key == "grow" && kind != "wrap" {
            return self.opt("wrap", "grow");
        }
        String::new()
    }

    pub fn grow(&self, kind: &str) -> Grow {
        if self.opt(kind, "grow") == "magnify" {
            Grow::Magnify
        } else {
            Grow::Stretch
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grow {
    Stretch,
    Magnify,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug)]
pub enum Node {
    Glyph { g: Rc<Glyph>, class: Class, limits: Limits, italic: bool, large: bool },
    Row(Vec<Node>),
    Scripts { base: Box<Node>, sub: Option<Box<Node>>, sup: Option<Box<Node>> },
    Frac(Box<Node>, Box<Node>),
    Sqrt(Box<Node>),
    Fenced { open: Option<Rc<Glyph>>, close: Option<Rc<Glyph>>, body: Box<Node>, grow: Grow },
    Grid { rows: Vec<Vec<Node>>, aligns: Vec<CellAlign>, col_gap: f64, row_gap: f64, display: bool },
    Text(Box<Node>),
    Conds(Vec<Node>),
    Prime(Box<Node>),
    Diff(Box<Node>),
    Over { g: Rc<Glyph>, class: Class, above: Option<Box<Node>>, below: Option<Box<Node>> },
    Small(Box<Node>),
    Diagram { rows: Vec<Vec<Node>>, edges: Vec<Edge>, gap: f64, cross: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Cell(usize, usize),
    Arrow(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Auto,
    Left,
    Right,
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub from: End,
    pub to: End,
    pub g: Rc<Glyph>,
    pub labels: Vec<(Node, Side)>,
    pub name: Option<String>,
    pub bend: f64,
}

impl Node {
    fn class(&self) -> Class {
        match self {
            Node::Glyph { class, .. } | Node::Over { class, .. } => *class,
            Node::Scripts { base, .. } => base.class(),
            Node::Row(v) if v.len() == 1 => v[0].class(),
            _ => Class::Ord,
        }
    }
}

pub struct Ctx<'a> {
    pub env: &'a MathEnv,
    pub diags: &'a mut Diagnostics,
    pub pos: &'a dyn Fn(usize) -> (usize, usize),
    pub display: bool,
}

pub fn parse(src: &str, ctx: &mut Ctx) -> Node {
    if ctx.display {
        let lines = top_lines(src);
        if lines.len() > 1 {
            return chain(src, &lines, ctx);
        }
    }
    parse_at(src, 0, ctx)
}

fn parse_at(src: &str, base: usize, ctx: &mut Ctx) -> Node {
    let mut p = Parser { s: src, i: 0, base, ctx, depth: 0, too_deep: false, edge_mode: false, edge_bend: 0.0 };
    let row = p.row(&[]);
    if p.i < p.s.len() {
        let at = p.i;
        p.err(at, format!("Unexpected `{}`.", p.peek().unwrap_or(' ')), Some("Remove it, or check the brackets before it.".into()));
    }
    Node::Row(row)
}

fn top_lines(src: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let (mut depth, mut quote, mut start) = (0i32, false, 0usize);
    for (i, c) in src.char_indices() {
        match c {
            '"' => quote = !quote,
            '(' | '[' | '{' if !quote => depth += 1,
            ')' | ']' | '}' if !quote => depth -= 1,
            '\n' if !quote && depth <= 0 => {
                out.push((start, i));
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push((start, src.len()));
    out.retain(|(a, b)| !src[*a..*b].trim().is_empty());
    out
}

fn chain(src: &str, lines: &[(usize, usize)], ctx: &mut Ctx) -> Node {
    let mut rows = Vec::new();
    for &(a, b) in lines {
        let Node::Row(nodes) = parse_at(&src[a..b], a, ctx) else { continue };
        let split = nodes.iter().position(|n| n.class() == Class::Rel);
        let lead = |w: f64| Node::Glyph { g: Rc::new(Glyph::empty(w)), class: Class::Ord, limits: Limits::Never, italic: false, large: false };
        let (left, right) = match split {
            Some(k) => {
                let mut nodes = nodes;
                let rest = nodes.split_off(k);
                let mut r = vec![lead(0.0)];
                r.extend(rest);
                (nodes, r)
            }
            None => {
                let mut r = vec![lead(0.0), lead(1000.0)];
                r.extend(nodes);
                (Vec::new(), r)
            }
        };
        rows.push(vec![Node::Row(left), Node::Row(right)]);
    }
    Node::Grid { rows, aligns: vec![CellAlign::Right, CellAlign::Left], col_gap: 0.0, row_gap: 300.0, display: true }
}

struct Parser<'a, 'b, 'c> {
    s: &'a str,
    i: usize,
    base: usize,
    ctx: &'b mut Ctx<'c>,
    depth: usize,
    too_deep: bool,
    edge_mode: bool,
    edge_bend: f64,
}

impl Parser<'_, '_, '_> {
    fn peek(&self) -> Option<char> {
        self.s[self.i..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.i += c.len_utf8();
        Some(c)
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }

    fn err(&mut self, at: usize, msg: impl Into<String>, hint: Option<String>) {
        if self.too_deep {
            return;
        }
        let (l, c) = (self.ctx.pos)(self.base + at);
        self.ctx.diags.error(l, c, msg, hint);
    }

    fn warn(&mut self, at: usize, msg: impl Into<String>, hint: Option<String>) {
        let (l, c) = (self.ctx.pos)(self.base + at);
        self.ctx.diags.warning(l, c, msg, hint);
    }

    fn row(&mut self, stops: &[char]) -> Vec<Node> {
        let out = Vec::new();
        if self.depth >= MAX_DEPTH {
            let at = self.i;
            if !self.too_deep {
                self.err(at, format!("This formula has more than {MAX_DEPTH} groups inside each other."), Some("Use `sym` or split the formula.".into()));
            }
            self.too_deep = true;
            self.i = self.s.len();
            return out;
        }
        self.depth += 1;
        let out = self.row_inner(stops, out);
        self.depth -= 1;
        out
    }

    fn row_inner(&mut self, stops: &[char], mut out: Vec<Node>) -> Vec<Node> {
        loop {
            self.skip_ws();
            match self.peek() {
                None => break,
                Some(c) if stops.contains(&c) => break,
                Some(_) => {
                    let before = self.i;
                    if let Some(n) = self.term() {
                        out.push(n);
                    }
                    if self.i == before {
                        self.bump();
                    }
                }
            }
        }
        out
    }

    fn term(&mut self) -> Option<Node> {
        let start = self.i;
        let base = match self.peek()? {
            '_' | '^' => {
                self.err(start, "A script needs something before it.", Some("Write the base first, for example `x_1`.".into()));
                self.bump();
                return None;
            }
            ')' | ']' | '}' => {
                let c = self.bump().unwrap_or(')');
                self.err(start, format!("This `{c}` has no opening bracket."), Some("Remove it, or add the opening bracket.".into()));
                return None;
            }
            _ => self.atom()?,
        };
        Some(self.scripts(base))
    }

    fn scripts(&mut self, base: Node) -> Node {
        let mut sub = None;
        let mut sup: Option<Node> = None;
        loop {
            let save = self.i;
            self.skip_ws();
            match self.peek() {
                Some('\'') => {
                    let mut primes = 0;
                    while self.peek() == Some('\'') {
                        self.bump();
                        primes += 1;
                    }
                    let g = Node::Prime(Box::new(font_atom(0x2032 + (primes.min(3) as u32 - 1), Class::Ord)));
                    sup = Some(match sup {
                        None => g,
                        Some(Node::Row(mut v)) => {
                            v.insert(0, g);
                            Node::Row(v)
                        }
                        Some(other) => Node::Row(vec![g, other]),
                    });
                }
                Some(c @ ('_' | '^')) => {
                    let at = self.i;
                    self.bump();
                    self.skip_ws();
                    let arg = match self.peek() {
                        Some('(') => {
                            self.bump();
                            let mut rows = vec![Node::Row(self.row(&[')', ';']))];
                            while self.peek() == Some(';') {
                                self.bump();
                                rows.push(Node::Row(self.row(&[')', ';'])));
                            }
                            if self.peek() == Some(')') {
                                self.bump();
                            } else {
                                self.err(at, "This script group has no closing `)`.", Some("Add `)` where the script ends.".into()));
                            }
                            Some(if rows.len() == 1 { rows.pop().unwrap() } else { Node::Conds(rows) })
                        }
                        Some(_) => self.atom(),
                        None => None,
                    };
                    let Some(arg) = arg else {
                        self.err(at, format!("`{c}` needs a script after it."), Some(format!("For example `x{c}2` or `x{c}(n + 1)`.")));
                        break;
                    };
                    if c == '^' && matches!(sup, Some(Node::Prime(_))) {
                        sup = Some(Node::Row(vec![sup.take().unwrap(), arg]));
                        continue;
                    }
                    let slot = if c == '_' { &mut sub } else { &mut sup };
                    if slot.is_some() {
                        self.err(at, format!("This term has two `{c}` scripts."), Some(format!("Put both parts in one group, for example `x{c}(i j)`.")));
                    }
                    *slot = Some(arg);
                }
                _ => {
                    self.i = save;
                    break;
                }
            }
        }
        if sub.is_none() && sup.is_none() {
            base
        } else if let Node::Glyph { g, class: Class::Rel, .. } = &base {
            Node::Over { g: g.clone(), class: Class::Rel, above: sup.map(Box::new), below: sub.map(Box::new) }
        } else {
            let limits = matches!(&base, Node::Glyph { limits: Limits::Display, .. });
            let n = Node::Scripts { base: Box::new(base), sub: sub.map(Box::new), sup: sup.map(Box::new) };
            if limits && self.ctx.env.opt("limits", "overhang") == "off" {
                Node::Row(vec![n])
            } else {
                n
            }
        }
    }

    fn atom(&mut self) -> Option<Node> {
        let start = self.i;
        let frame = trace::Frame::new();
        let node = self.atom_inner()?;
        let (a, b) = ((self.ctx.pos)(self.base + start), (self.ctx.pos)(self.base + self.i));
        frame.finish(a, b, &self.s[start..self.i], &node);
        Some(node)
    }

    fn atom_inner(&mut self) -> Option<Node> {
        let start = self.i;
        let c = self.peek()?;
        if c.is_ascii_digit() {
            let mut glyphs = Vec::new();
            while let Some(d) = self.peek() {
                let is_decimal = d == '.' && self.s[self.i + 1..].starts_with(|x: char| x.is_ascii_digit()) && !glyphs.is_empty();
                if d.is_ascii_digit() || is_decimal {
                    self.bump();
                    glyphs.push(d as u32);
                } else {
                    break;
                }
            }
            let g = word_glyph(&glyphs, "num");
            return Some(Node::Glyph { g, class: Class::Ord, limits: Limits::Never, italic: false, large: false });
        }
        if c.is_alphabetic() {
            let word = self.ident();
            if self.peek() == Some('(') && symbols::CALLS.contains(&word.as_str()) {
                let node = self.structure(&word, start)?;
                let ops = self.ops();
                if ops.is_empty() {
                    return Some(node);
                }
                return self.node_ops(node, &ops, start);
            }
            let ops = self.ops();
            if ops.is_empty() {
                if symbols::DIFFERENTIALS.contains(&word.as_str()) {
                    let d = if self.ctx.env.opt("dd", "style") == "up" { font_atom('d' as u32, Class::Ord) } else { italic_atom(symbols::italic_letter('d').unwrap_or('d' as u32)) };
                    return Some(Node::Diff(Box::new(d)));
                }
                if let Some(w) = symbols::space(&word) {
                    return Some(Node::Glyph { g: Rc::new(Glyph::empty(w)), class: Class::Ord, limits: Limits::Never, italic: false, large: false });
                }
            }
            if let Some(k) = ops.iter().position(|(o, _)| is_fence(o)) {
                let node = self.resolve(&word, &ops[..k], start, 0);
                return self.node_ops(node, &ops[k..], start);
            }
            return Some(self.resolve(&word, &ops, start, 0));
        }
        match c {
            '(' | '[' | '{' => {
                self.bump();
                let close = match c {
                    '(' => ')',
                    '[' => ']',
                    _ => '}',
                };
                let half = match c {
                    '(' => ']',
                    '[' => ')',
                    _ => '}',
                };
                let body = self.row(&[close, half]);
                let closed_by = self.peek().filter(|k| *k == close || *k == half);
                let close_glyph = if let Some(k) = closed_by {
                    self.bump();
                    Some(font_glyph_rc(k as u32))
                } else {
                    self.err(start, format!("This `{c}` is not closed."), Some(format!("Add `{close}` where the group ends.")));
                    None
                };
                if c == '(' && closed_by == Some(')') {
                    let ops = self.ops();
                    if !ops.is_empty() {
                        return self.node_ops(Node::Row(body), &ops, start);
                    }
                }
                Some(Node::Fenced { open: Some(font_glyph_rc(c as u32)), close: close_glyph, body: Box::new(Node::Row(body)), grow: Grow::Stretch })
            }
            '"' => {
                self.bump();
                let text_start = self.i;
                let end = self.s[self.i..].find('"').map(|e| self.i + e);
                let Some(end) = end else {
                    self.err(start, "This text has no closing `\"`.", Some("Add `\"` where the text ends.".into()));
                    self.i = self.s.len();
                    return None;
                };
                let text = &self.s[text_start..end];
                self.i = end + 1;
                let cps: Vec<u32> = text.chars().map(|ch| ch as u32).collect();
                Some(Node::Text(Box::new(Node::Glyph { g: word_glyph(&cps, "text"), class: Class::Ord, limits: Limits::Never, italic: false, large: false })))
            }
            _ => {
                self.bump();
                let (cp, class) = match c {
                    '-' => (0x2212, Class::Bin),
                    '*' => (0x2217, Class::Bin),
                    _ => (c as u32, symbols::char_class(c)),
                };
                let (cp, class) = match symbols::duplicate_of(cp) {
                    Some((canon, name)) => {
                        self.warn(start, format!("`{c}` is a second code for a symbol that Functor already has."), Some(format!("Write `{name}`. Functor keeps one form for each symbol.")));
                        (canon, char::from_u32(canon).map_or(class, symbols::char_class))
                    }
                    None => (cp, class),
                };
                if glyph::font_glyph(cp, false).is_none() {
                    self.err(start, format!("The font has no glyph for `{c}`."), Some("Use a named symbol instead, or build it from a base and operations.".into()));
                    return None;
                }
                let ops = self.ops();
                let ops = self.take_bend(&ops);
                trace::meaning(Form::Name, false, || (format!("char {cp}"), format!("The typed character `{c}`, a {}.", trace::class_text(class))));
                if ops.is_empty() {
                    trace_build(&c.to_string(), &ops, Source::Font(cp));
                    return Some(font_atom(cp, class));
                }
                let base = Rc::new(Glyph::from_font(cp, false)?);
                let recipe = format!("{c}{}", join_ops(&ops));
                trace_build(&recipe, &ops, Source::Built);
                let g = self.apply_ops(base, &ops, &recipe, false, None, None)?;
                Some(Node::Glyph { g, class, limits: Limits::Never, italic: false, large: false })
            }
        }
    }

    fn ident(&mut self) -> String {
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_alphanumeric()) {
            self.bump();
        }
        self.s[start..self.i].to_string()
    }

    fn ops(&mut self) -> Vec<(String, usize)> {
        let mut ops = Vec::new();
        while self.peek() == Some('.') && self.s[self.i + 1..].starts_with(|c: char| c.is_alphabetic()) {
            self.bump();
            let at = self.i;
            let name = self.ident();
            if self.peek() == Some('(') {
                let close = self.s[self.i..].find(')').map(|e| self.i + e);
                self.err(at, format!("`{name}` takes no arguments in Functor 1."), Some(format!("Write `.{name}` without brackets.")));
                self.i = close.map_or(self.s.len(), |e| e + 1);
            }
            ops.push((name, at));
        }
        ops
    }

    fn call(&mut self, name: &str, start: usize) -> Option<Node> {
        self.bump();
        let mut args = Vec::new();
        let mut texts: Vec<String> = Vec::new();
        loop {
            let a0 = self.i;
            let row = self.row(&[',', ')']);
            texts.push(self.s[a0..self.i].trim().to_string());
            args.push(Node::Row(row));
            match self.peek() {
                Some(',') => {
                    self.bump();
                }
                Some(')') => {
                    self.bump();
                    break;
                }
                _ => {
                    self.err(start, format!("`{name}(` is not closed."), Some("Add `)` after the last argument.".into()));
                    break;
                }
            }
        }
        let want = match name {
            "frac" | "join" | "binom" | "inner" | "stack" => 2,
            "wrap" if args.len() == 3 => 3,
            "wrap" => 2,
            _ => 1,
        };
        if args.len() != want {
            self.err(
                start,
                format!("`{name}` takes {want} argument{}, but it has {}.", if want == 1 { "" } else { "s" }, args.len()),
                Some(match name {
                    "frac" => "Write `frac(top, bottom)`.".into(),
                    "join" => "Write `join(a, b)`.".into(),
                    "stack" => "Write `stack(top, bottom)`.".into(),
                    "binom" => "Write `binom(n, k)`.".into(),
                    "inner" => "Write `inner(u, v)`.".into(),
                    "wrap" => "Write `wrap(glyph, body)` or `wrap(left, right, body)`.".into(),
                    _ => format!("Write `{name}(x)`."),
                }),
            );
        }
        if name == "join" {
            return Some(self.join(args, start));
        }
        if name == "stack" {
            return Some(self.stack(args, &texts, start));
        }
        if name == "wrap" {
            return Some(self.wrap(args, start));
        }
        let mut it = args.into_iter();
        let a = it.next().unwrap_or(Node::Row(vec![]));
        let mut b = || it.next().unwrap_or(Node::Row(vec![]));
        Some(match name {
            "frac" => Node::Frac(Box::new(a), Box::new(b())),
            "sqrt" => Node::Sqrt(Box::new(a)),
            "binom" => {
                let g = Node::Grid { rows: vec![vec![a], vec![b()]], aligns: vec![CellAlign::Center], col_gap: 0.0, row_gap: 120.0, display: false };
                self.fenced("binom", g)
            }
            "inner" => {
                let body = Node::Row(vec![a, font_atom(0x2C, Class::Punct), b()]);
                self.fenced("inner", body)
            }
            "abs" | "norm" | "floor" | "ceil" => self.fenced(name, a),
            _ => a,
        })
    }

    fn structure(&mut self, name: &str, start: usize) -> Option<Node> {
        match name {
            "vec" => self.vec_call(start),
            "diagram" => self.diagram(start),
            "grid" | "mat" | "cases" => {
                let rows = self.cells(name, start);
                Some(self.build_grid(name, rows, start))
            }
            _ => self.call(name, start),
        }
    }

    fn cells(&mut self, name: &str, start: usize) -> Vec<Vec<Node>> {
        self.bump();
        let mut rows: Vec<Vec<Node>> = vec![Vec::new()];
        loop {
            let row = self.row(&[',', ';', ')']);
            if let Some(last) = rows.last_mut() {
                last.push(Node::Row(row));
            }
            match self.peek() {
                Some(',') => {
                    self.bump();
                }
                Some(';') => {
                    self.bump();
                    rows.push(Vec::new());
                }
                Some(')') => {
                    self.bump();
                    break;
                }
                _ => {
                    self.err(start, format!("`{name}(` is not closed."), Some("Add `)` after the last row.".into()));
                    break;
                }
            }
        }
        if rows.len() > 1 && rows.last().is_some_and(|r| r.len() == 1 && matches!(&r[0], Node::Row(v) if v.is_empty())) {
            rows.pop();
        }
        rows
    }

    fn build_grid(&mut self, name: &str, mut rows: Vec<Vec<Node>>, start: usize) -> Node {
        let ncols = rows.iter().map(Vec::len).max().unwrap_or(0);
        if rows.iter().any(|r| r.len() != ncols) {
            self.warn(
                start,
                format!("The rows of this `{name}` have different numbers of cells."),
                Some(format!("Give each row {ncols} cells. A cell can be empty: `a, , c`.")),
            );
        }
        match name {
            "cases" => {
                let word = self.ctx.env.opt("cases", "word");
                if !word.is_empty() {
                    for r in rows.iter_mut().filter(|r| r.len() == 2) {
                        let cond = r.pop().unwrap_or(Node::Row(vec![]));
                        let cps: Vec<u32> = word.chars().map(|c| c as u32).collect();
                        let w = Node::Glyph { g: word_glyph(&cps, "text"), class: Class::Ord, limits: Limits::Never, italic: false, large: false };
                        let gap = Node::Glyph { g: Rc::new(Glyph::empty(333.0)), class: Class::Ord, limits: Limits::Never, italic: false, large: false };
                        r.push(Node::Row(vec![w, gap, cond]));
                    }
                }
                let g = Node::Grid { rows, aligns: vec![CellAlign::Left; ncols.max(1)], col_gap: 1000.0, row_gap: 200.0, display: false };
                self.fenced("cases", g)
            }
            "mat" => {
                let g = Node::Grid { rows, aligns: vec![CellAlign::Center; ncols.max(1)], col_gap: 800.0, row_gap: 200.0, display: false };
                let m = self.fenced("mat", g);
                self.small("mat", m)
            }
            _ => Node::Grid { rows, aligns: vec![CellAlign::Center; ncols.max(1)], col_gap: 800.0, row_gap: 200.0, display: false },
        }
    }

    fn vec_call(&mut self, start: usize) -> Option<Node> {
        let save = self.i;
        self.bump();
        self.skip_ws();
        if self.peek().is_some_and(char::is_alphabetic) {
            let at = self.i;
            let word = self.ident();
            if self.peek() != Some('(') {
                let ops = self.ops();
                self.skip_ws();
                if self.peek() == Some(')') {
                    self.bump();
                    trace::meaning(Form::Kind, false, || (format!("vec {word}"), format!("`{word}` is a vector, from `vec({word})`.")));
                    let rule = rule_text(self.ctx.env, "vec", "style");
                    trace::edit(|t| {
                        t.style.push(rule);
                        t.subject = word.clone();
                        t.hand = ops.iter().filter(|(o, _)| style_like(o)).map(|(o, _)| o.clone()).collect();
                    });
                    let mut all = vec![(self.ctx.env.vec_style.clone(), at)];
                    all.extend(ops);
                    return Some(self.resolve_base(&word, &all, at));
                }
            }
        }
        self.i = save;
        let cells: Vec<Node> = self.cells("vec", start).into_iter().flatten().collect();
        let body = if self.ctx.env.opt("vec", "body") == "row" {
            let mut row = Vec::new();
            for (k, c) in cells.into_iter().enumerate() {
                if k > 0 {
                    row.push(font_atom(0x2C, Class::Punct));
                }
                row.push(c);
            }
            Node::Row(row)
        } else {
            let rows = cells.into_iter().map(|c| vec![c]).collect();
            let g = Node::Grid { rows, aligns: vec![CellAlign::Center], col_gap: 0.0, row_gap: 200.0, display: false };
            let v = self.fenced("vec", g);
            return Some(self.small("vec", v));
        };
        Some(self.fenced("vec", body))
    }

    fn small(&self, kind: &str, node: Node) -> Node {
        if !self.ctx.display && self.ctx.env.opt(kind, "inline") == "small" {
            Node::Small(Box::new(node))
        } else {
            node
        }
    }

    fn diagram(&mut self, start: usize) -> Option<Node> {
        self.bump();
        let mut rows: Vec<Vec<Node>> = vec![Vec::new()];
        let mut texts: Vec<Vec<String>> = vec![Vec::new()];
        let mut has_edges = false;
        loop {
            self.skip_ws();
            let a = self.i;
            let row = self.row(&[',', ';', ')', '|']);
            let t = norm(&self.s[a..self.i]);
            if let (Some(r), Some(tr)) = (rows.last_mut(), texts.last_mut()) {
                r.push(Node::Row(row));
                tr.push(t);
            }
            match self.peek() {
                Some(',') => {
                    self.bump();
                }
                Some(';') => {
                    self.bump();
                    rows.push(Vec::new());
                    texts.push(Vec::new());
                }
                Some('|') => {
                    self.bump();
                    has_edges = true;
                    break;
                }
                Some(')') => {
                    self.bump();
                    break;
                }
                _ => {
                    self.err(start, "`diagram(` is not closed.", Some("Add `)` after the last arrow.".into()));
                    return None;
                }
            }
        }
        if rows.len() > 1 && texts.last().is_some_and(|r| r.len() == 1 && r[0].is_empty()) {
            rows.pop();
            texts.pop();
        }
        let ncols = rows.iter().map(Vec::len).max().unwrap_or(0);
        if rows.iter().any(|r| r.len() != ncols) {
            self.warn(
                start,
                "The rows of this `diagram` have different numbers of cells.",
                Some(format!("Give each row {ncols} cells. A cell can be empty: `A, , B`.")),
            );
        }
        let mut edges = Vec::new();
        if has_edges {
            loop {
                self.skip_ws();
                match self.peek() {
                    Some(')') => {
                        self.bump();
                        break;
                    }
                    None => {
                        self.err(start, "`diagram(` is not closed.", Some("Add `)` after the last arrow.".into()));
                        break;
                    }
                    _ => {}
                }
                if let Some(e) = self.edge(&texts, &edges) {
                    edges.push(e);
                }
                match self.peek() {
                    Some(',') => {
                        self.bump();
                    }
                    Some(')') => {
                        self.bump();
                        break;
                    }
                    _ => {
                        self.err(start, "`diagram(` is not closed.", Some("Add `)` after the last arrow.".into()));
                        break;
                    }
                }
            }
        }
        let gap = match self.ctx.env.opt("diagram", "gap").as_str() {
            "small" => 0.65,
            "large" => 1.4,
            _ => 1.0,
        };
        let cross = self.ctx.env.opt("diagram", "cross") != "plain";
        let node = Node::Diagram { rows, edges, gap, cross };
        Some(self.small("diagram", node))
    }

    fn top_colon(&self) -> bool {
        let (mut depth, mut quote) = (0i32, false);
        for c in self.s[self.i..].chars() {
            match c {
                '"' => quote = !quote,
                _ if quote => {}
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' if depth == 0 => return false,
                ')' | ']' | '}' => depth -= 1,
                ',' if depth == 0 => return false,
                ':' if depth == 0 => return true,
                _ => {}
            }
        }
        false
    }

    fn edge(&mut self, texts: &[Vec<String>], done: &[Edge]) -> Option<Edge> {
        let at = self.i;
        let mut labels = Vec::new();
        let mut name = None;
        if self.top_colon() {
            let a = self.i;
            let l = self.row(&[':']);
            name = Some(norm(&self.s[a..self.i]));
            self.bump();
            labels.push((Node::Row(l), Side::Auto));
        }
        let mut parts: Vec<(usize, usize, Node)> = Vec::new();
        self.edge_mode = true;
        self.edge_bend = 0.0;
        loop {
            self.skip_ws();
            match self.peek() {
                None | Some(',') | Some(')') => break,
                _ => {}
            }
            let a = self.i;
            if let Some(n) = self.term() {
                parts.push((a, self.i, n));
            }
            if self.i == a {
                self.bump();
            }
        }
        self.edge_mode = false;
        let bend = self.edge_bend;
        let Some(k) = parts.iter().position(|(_, _, n)| n.class() == Class::Rel) else {
            self.err(at, "This arrow has no arrow symbol.", Some("Write `f: A to B`. Any relation works, for example `A = B`.".into()));
            return None;
        };
        if k == 0 || k + 1 == parts.len() {
            self.err(at, "An arrow needs a cell before and after the arrow symbol.", Some("Write `f: A to B`.".into()));
            return None;
        }
        let from_t = norm(&self.s[parts[0].0..parts[k - 1].1]);
        let to_t = norm(&self.s[parts[k + 1].0..parts[parts.len() - 1].1]);
        let rel_t = &self.s[parts[k].0..parts[k].1];
        if name.is_none() {
            if let Some(i) = rel_t.find(['^', '_']) {
                let n = norm(rel_t[i + 1..].trim().trim_start_matches('(').trim_end_matches(')'));
                if !n.is_empty() {
                    name = Some(n);
                }
            }
        }
        let arrow = |t: &str| done.iter().rposition(|e| e.name.as_deref() == Some(t));
        let fa = arrow(&from_t).filter(|_| !self.is_cell(texts, &from_t));
        let ta = arrow(&to_t).filter(|_| !self.is_cell(texts, &to_t));
        let (from, to) = match (fa, ta) {
            (Some(a), Some(b)) => {
                if a == b {
                    self.err(at, "A 2-cell needs two different arrows.", Some("Write `alpha: f implies g`, where f and g name two arrows.".into()));
                    return None;
                }
                if bend != 0.0 {
                    self.err(at, "A 2-cell cannot bend.", Some("Remove `bend` from this arrow.".into()));
                }
                (End::Arrow(a), End::Arrow(b))
            }
            (Some(_), None) | (None, Some(_)) => {
                self.err(at, "An arrow goes from a cell to a cell, or from an arrow to an arrow.", Some("For a 2-cell, name both arrows: `alpha: f implies g`.".into()));
                return None;
            }
            _ => {
                let from = self.find_cell(texts, &from_t, parts[0].0)?;
                let to = self.find_cell(texts, &to_t, parts[k + 1].0)?;
                if from == to {
                    self.err(at, "An arrow needs two different cells.", Some("Loops come in a later phase.".into()));
                    return None;
                }
                (End::Cell(from.0, from.1), End::Cell(to.0, to.1))
            }
        };
        let rel = parts.swap_remove(k).2;
        let g = match rel {
            Node::Glyph { g, .. } => g,
            Node::Over { g, above, below, .. } => {
                if let Some(a) = above {
                    labels.push((*a, Side::Left));
                }
                if let Some(b) = below {
                    labels.push((*b, Side::Right));
                }
                g
            }
            _ => {
                self.err(at, "The arrow must be one symbol.", Some("Build it first, for example `sym hook = ...`, then write `A hook B`.".into()));
                return None;
            }
        };
        Some(Edge { from, to, g, labels, name, bend })
    }

    fn is_cell(&self, texts: &[Vec<String>], t: &str) -> bool {
        texts.iter().any(|r| r.iter().any(|c| c == t))
    }

    fn take_bend(&mut self, ops: &[(String, usize)]) -> Vec<(String, usize)> {
        if !self.edge_mode {
            return ops.to_vec();
        }
        let mut out = Vec::with_capacity(ops.len());
        let mut bent = false;
        for (o, at) in ops {
            if o == "bend" {
                self.edge_bend = (self.edge_bend.abs() + 30.0).min(60.0) * if self.edge_bend < 0.0 { -1.0 } else { 1.0 };
                bent = true;
            } else if o == "flipv" && bent {
                self.edge_bend = -self.edge_bend;
            } else {
                out.push((o.clone(), *at));
            }
        }
        out
    }

    fn find_cell(&mut self, texts: &[Vec<String>], t: &str, at: usize) -> Option<(usize, usize)> {
        let hits: Vec<(usize, usize)> = texts
            .iter()
            .enumerate()
            .flat_map(|(r, row)| row.iter().enumerate().filter(|(_, c)| c.as_str() == t).map(move |(c, _)| (r, c)))
            .collect();
        match hits.len() {
            1 => return Some(hits[0]),
            0 => {}
            _ => {
                self.err(at, format!("`{t}` is in more than one cell."), Some("Write the position of the cell instead, such as `2.1` (row 2, column 1).".into()));
                return None;
            }
        }
        if let Some((r, c)) = t.split_once('.') {
            if let (Ok(r), Ok(c)) = (r.trim().parse::<usize>(), c.trim().parse::<usize>()) {
                if r >= 1 && c >= 1 && texts.get(r - 1).is_some_and(|row| c <= row.len()) {
                    return Some((r - 1, c - 1));
                }
                self.err(at, format!("This diagram has no cell at row {r}, column {c}."), None);
                return None;
            }
        }
        self.err(at, format!("`{t}` is not a cell of this diagram."), Some("Write the content of a cell, or its position such as `2.1` (row 2, column 1).".into()));
        None
    }

    fn fenced(&self, kind: &str, body: Node) -> Node {
        let spec = self.ctx.env.opt(kind, "fence");
        trace::meaning(Form::Kind, false, || (format!("kind {kind}"), format!("The kind `{kind}`: {}.", trace::kind_text(kind))));
        trace::edit(|t| {
            if t.form == Form::Kind {
                t.structure = !matches!(kind, "abs" | "norm" | "floor" | "ceil");
                t.subject = kind.to_string();
                t.recipe = format!("fence: {spec}");
                t.glyph = Source::Fence;
            }
            t.style.push(rule_text(self.ctx.env, kind, "fence"));
        });
        let (open, close) = trace::paused(|| self.fence_pair(&spec));
        Node::Fenced { open, close, body: Box::new(body), grow: self.ctx.env.grow(kind) }
    }

    fn fence_pair(&self, spec: &str) -> (Option<Rc<Glyph>>, Option<Rc<Glyph>>) {
        fence_pair(spec, self.ctx.env)
    }

    fn wrap(&mut self, mut args: Vec<Node>, start: usize) -> Node {
        let body = args.pop().unwrap_or(Node::Row(vec![]));
        let glyphs: Vec<Rc<Glyph>> = args.iter().filter_map(single_glyph).collect();
        if glyphs.len() != args.len() || glyphs.is_empty() {
            self.err(start, "`wrap` needs single symbols as its fences.", Some("Write `wrap(L.script, x + y)`. Build the fence first, then wrap.".into()));
            return body;
        }
        let open = glyphs[0].clone();
        let close = glyphs.get(1).cloned().unwrap_or_else(|| Rc::new(open.reflect("flip")));
        Node::Fenced { open: Some(open), close: Some(close), body: Box::new(body), grow: self.ctx.env.grow("wrap") }
    }

    fn node_ops(&mut self, mut node: Node, ops: &[(String, usize)], start: usize) -> Option<Node> {
        let mut pending: Vec<(String, usize)> = Vec::new();
        let mut i = 0;
        while i < ops.len() {
            let (o, at) = &ops[i];
            i += 1;
            if let Some(f) = symbols::fence_def(o) {
                if !pending.is_empty() {
                    node = self.group_ops(node, &pending, start)?;
                    pending.clear();
                }
                let mut mods = Vec::new();
                while i < ops.len() && symbols::FENCE_MODIFIERS.contains(&ops[i].0.as_str()) {
                    mods.push(ops[i].0.clone());
                    i += 1;
                }
                let (open, close) = builtin_fence(f, &mods);
                let spec: String = std::iter::once(o.as_str()).chain(mods.iter().map(String::as_str)).collect::<Vec<_>>().join(".");
                trace::meaning(Form::Hand, true, || ("fence".into(), format!("No meaning in the source: fences `.{spec}` written by hand.")));
                trace::edit(|t| {
                    t.recipe = format!("fences .{spec} around the group");
                    t.ops = std::iter::once(o).chain(mods.iter()).cloned().collect();
                    t.glyph = Source::Fence;
                    t.style.clear();
                });
                node = Node::Fenced { open, close, body: Box::new(node), grow: Grow::Stretch };
                continue;
            }
            if o == "left" || o == "right" {
                self.err(*at, format!("`{o}` works only after a fence."), Some(format!("For example `(x).brace.{o}`.")));
                continue;
            }
            if symbols::op_def(o).is_none() {
                let hint = did_you_mean(o, symbols::OPS.iter().map(|d| d.name)).map(|s| format!("Did you mean `{s}`?"));
                self.err(*at, format!("Unknown operation `{o}`."), hint.or(Some("See the list of operations in the reference.".into())));
                continue;
            }
            pending.push((o.clone(), *at));
        }
        if !pending.is_empty() {
            node = self.group_ops(node, &pending, start)?;
        }
        Some(node)
    }

    fn join(&mut self, args: Vec<Node>, start: usize) -> Node {
        let mut glyphs = Vec::new();
        for a in &args {
            match single_glyph(a) {
                Some(g) => glyphs.push(g),
                None => {
                    self.err(start, "`join` takes two single symbols.", Some("Write `join(o, r)`. Build each side first, for example `join(circ, r.up)`.".into()));
                    return Node::Row(args);
                }
            }
        }
        if glyphs.len() != 2 {
            return Node::Row(args);
        }
        let g = Rc::new(glyphs[0].join(&glyphs[1]));
        trace::meaning(Form::Hand, false, || ("join".into(), "No meaning in the source: two symbols joined by hand with `join`.".into()));
        trace::edit(|t| {
            t.recipe = "join(a, b)".into();
            t.glyph = Source::Joined;
        });
        Node::Glyph { g, class: Class::Ord, limits: Limits::Never, italic: false, large: false }
    }

    fn stack(&mut self, args: Vec<Node>, texts: &[String], start: usize) -> Node {
        let classes: Vec<Class> = args.iter().filter_map(node_class).collect();
        let class = if classes.contains(&Class::Rel) {
            Class::Rel
        } else if classes.contains(&Class::Bin) {
            Class::Bin
        } else {
            Class::Ord
        };
        if let (Some(a), Some(b)) = (texts.first(), texts.get(1)) {
            if let Some(cp) = symbols::stack_glyph(a, b) {
                if glyph::font_glyph(cp, false).is_some() {
                    glyph::note_font_glyph();
                    trace::meaning(Form::Hand, false, || (format!("stack {a} {b}"), "No meaning in the source: two symbols stacked by hand with `stack`.".into()));
                    trace::edit(|t| {
                        t.recipe = format!("stack({a}, {b})");
                        t.glyph = Source::Composite(cp);
                    });
                    return glyph_node(cp, class, Limits::Never, false, false);
                }
            }
        }
        let mut glyphs = Vec::new();
        for (a, t) in args.iter().zip(texts) {
            match single_glyph(a) {
                Some(g) if t.starts_with('"') => glyphs.push(Rc::new(g.magnified(0.7))),
                Some(g) => glyphs.push(g),
                None => {
                    self.err(start, "`stack` takes two single symbols.", Some("Write `stack(lt, sim)` or `stack(eq, \"def\")`. Build each part first.".into()));
                    return Node::Row(args);
                }
            }
        }
        if glyphs.len() != 2 {
            return Node::Row(args);
        }
        let g = Rc::new(glyphs[0].stack(&glyphs[1]));
        trace::meaning(Form::Hand, false, || (format!("stack {}", texts.join(" ")), "No meaning in the source: two symbols stacked by hand with `stack`.".into()));
        trace::edit(|t| {
            t.recipe = format!("stack({})", texts.join(", "));
            t.glyph = Source::Joined;
        });
        Node::Glyph { g, class, limits: Limits::Never, italic: false, large: false }
    }

    fn group_ops(&mut self, body: Node, ops: &[(String, usize)], start: usize) -> Option<Node> {
        if ops.len() > HARD_OPS {
            self.err(start, format!("This group has {} operations. The most is {HARD_OPS}.", ops.len()), Some("Define a `sym` with a clear name.".into()));
            return Some(body);
        }
        if ops.len() > MAX_OPS {
            self.warn(start, format!("This group has {} operations. Symbols with more than {MAX_OPS} are hard to read.", ops.len()), Some("Use fewer operations, or define a `sym` with a clear name.".into()));
        }
        let b = layout(&body, St { scale: 1.0, display: self.ctx.display, level: 0 });
        let mut items = Vec::new();
        for (a, draw) in &b.items {
            match draw {
                Draw::G(g) => items.extend(g.transformed(*a)),
                Draw::Rule(rw, rh) => {
                    let (x, y) = a.apply(0.0, rh / 2.0);
                    items.push(glyph::Item { shape: glyph::Shape::Stroke(format!("M{x} {y}L{} {y}", x + rw)), tf: Affine::ID, bold: false });
                }
            }
        }
        let base = Rc::new(Glyph { w: b.w, h: b.h, d: b.d, items, italic: false });
        let mut h = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(&format!("{:?}", base.items), &mut h);
        let recipe = format!("group:{:x}{}", std::hash::Hasher::finish(&h), join_ops(ops));
        trace::meaning(Form::Hand, true, || (format!("group{}", join_ops(ops)), "No meaning in the source: operations on a group, written by hand.".into()));
        trace_build(&format!("(group){}", join_ops(ops)), ops, Source::Built);
        let g = self.apply_ops(base, ops, &recipe, false, None, None)?;
        Some(Node::Glyph { g, class: Class::Ord, limits: Limits::Never, italic: false, large: false })
    }

    fn resolve(&mut self, word: &str, ops: &[(String, usize)], start: usize, depth: usize) -> Node {
        if depth > 8 {
            self.err(start, format!("`{word}` refers to itself."), Some("A `sym` cannot use its own name.".into()));
            return Node::Row(vec![]);
        }
        if depth == 0 {
            trace::edit(|t| {
                t.subject = word.to_string();
                t.hand = ops.iter().filter(|(o, _)| style_like(o)).map(|(o, _)| o.clone()).collect();
            });
        }
        if let Some(recipe) = self.ctx.env.syms.get(word).cloned() {
            trace::meaning(Form::Sym, false, || (format!("sym {word}"), format!("Your symbol `{word}`, from `sym {word} = {recipe}`.")));
            let (base_word, mut base_ops) = split_recipe(&recipe);
            base_ops.extend(ops.iter().map(|(o, _)| o.clone()));
            let with_pos: Vec<(String, usize)> = base_ops.into_iter().map(|o| (o, start)).collect();
            return self.resolve(&base_word, &with_pos, start, depth + 1);
        }
        if symbols::ARROW_KINDS.contains(&word) {
            if let Some(name) = symbols::arrow_style(&self.ctx.env.opt(word, "style")) {
                return self.resolve_base(name, ops, start);
            }
        }
        if symbols::named(word).is_some() && !symbols::ARROW_KINDS.contains(&word) {
            if let Some(rule) = self.ctx.env.kinds.get(&format!("{word}.style")).cloned() {
                let rule_kinds: Vec<OpKind> = rule.split('.').filter_map(|o| symbols::op_def(o).map(|d| d.kind)).collect();
                let hand: Vec<&str> = ops.iter().map(|(o, _)| o.as_str()).filter(|o| symbols::op_def(o).is_some_and(|d| rule_kinds.contains(&d.kind))).collect();
                trace::edit(|t| {
                    t.subject = word.to_string();
                    t.style.push(if hand.is_empty() { format!("`set {word}(style: {rule})`") } else { format!("`set {word}(style: {rule})`: not used, because `.{}` is written by hand", hand.join("`, `.")) });
                });
                if hand.is_empty() {
                    let mut all: Vec<(String, usize)> = rule.split('.').map(|o| (o.to_string(), start)).collect();
                    all.extend(ops.iter().cloned());
                    return self.resolve_base(word, &all, start);
                }
            }
        }
        if symbols::greek_index(word).is_some() {
            if let Some(style) = self.ctx.env.kinds.get(&format!("greek.{word}")).cloned() {
                let styled = ops.first().and_then(|(o, _)| symbols::op_def(o)).is_some_and(|d| d.kind == OpKind::Style);
                if !styled {
                    let mut all = vec![(style, start)];
                    all.extend(ops.iter().cloned());
                    return self.resolve_base(word, &all, start);
                }
            }
        }
        if let Some(kind) = self.ctx.env.objects.get(word).cloned() {
            trace::meaning(Form::Object, false, || (format!("{kind} {word}"), format!("`{word}` is {}, from `object {kind}: {word}`.", trace::kind_text(&kind))));
            if kind == "vec" {
                let s = rule_text(self.ctx.env, "vec", "style");
                let hand: Vec<&str> = ops.iter().map(|(o, _)| o.as_str()).filter(|o| *o == "arrow" || *o == "bold").collect();
                trace::edit(|t| {
                    t.subject = word.to_string();
                    t.style.push(if hand.is_empty() { s } else { format!("{s}: not used, because `.{}` is written by hand", hand.join("`, `.")) });
                });
            }
            if kind == "vec" && !ops.iter().any(|(o, _)| o == "arrow" || o == "bold") {
                let mut all = vec![(self.ctx.env.vec_style.clone(), start)];
                all.extend(ops.iter().cloned());
                return self.resolve_base(word, &all, start);
            }
        }
        self.resolve_base(word, ops, start)
    }

    fn resolve_base(&mut self, word: &str, ops: &[(String, usize)], start: usize) -> Node {
        let owned = self.take_bend(ops);
        let ops = &owned[..];
        if let Some(k) = ops.iter().position(|(o, _)| is_fence(o)) {
            let n = self.resolve_base(word, &ops[..k], start);
            return self.node_ops(n, &ops[k..], start).unwrap_or(Node::Row(vec![]));
        }
        if ops.len() > HARD_OPS {
            self.err(start, format!("`{word}` has {} operations. The most is {HARD_OPS}.", ops.len()), Some("Define a `sym` with a clear name.".into()));
            return self.resolve_base(word, &[], start);
        }
        if ops.len() > MAX_OPS {
            self.warn(
                start,
                format!("`{word}` has {} operations. Symbols with more than {MAX_OPS} are hard to read.", ops.len()),
                Some("Use fewer operations, or define a `sym` with a clear name.".into()),
            );
        }
        let ops_text = join_ops(ops);
        let display = self.ctx.display;
        let (base_cp, mut class, mut limits, italic, mut large) = if let Some(n) = symbols::named(word) {
            trace::meaning(Form::Name, false, || (format!("name {word}"), format!("The built-in symbol `{word}`, a {}.", trace::class_text(n.class))));
            (n.cp, n.class, self.limits_rule(word, n.limits), false, n.large && display)
        } else if let Some(cp) = symbols::greek(word) {
            trace::meaning(Form::Name, false, || (format!("name {word}"), format!("The Greek letter `{word}`.")));
            (cp, Class::Ord, Limits::Never, true, false)
        } else if let Some(lim) = symbols::function(word) {
            trace::meaning(Form::Name, false, || (format!("name {word}"), format!("The function name `{word}`.")));
            trace_build(word, &[], Source::Word);
            if let Some((o, at)) = ops.first() {
                self.err(*at, format!("`{word}` is a function name; it takes no operation `.{o}`."), None);
            }
            let cps: Vec<u32> = symbols::function_text(word).chars().map(|c| c as u32).collect();
            return Node::Glyph { g: word_glyph(&cps, "fn"), class: Class::Op, limits: self.limits_rule(word, lim), italic: false, large: false };
        } else if word.chars().count() == 1 {
            let mut c = word.chars().next().unwrap_or('x');
            if let Some((canon, name)) = symbols::duplicate_of(c as u32) {
                self.warn(start, format!("`{c}` is a second code for a symbol that Functor already has."), Some(format!("Write `{name}`. Functor keeps one form for each symbol.")));
                c = char::from_u32(canon).unwrap_or(c);
            }
            let cp = symbols::italic_letter(c).unwrap_or(c as u32);
            trace::meaning(Form::Name, false, || (format!("letter {c}"), format!("The letter `{c}`.")));
            (cp, Class::Ord, Limits::Never, cp != c as u32, false)
        } else {
            let names = symbols::all_names();
            let mut user: Vec<&str> = self.ctx.env.syms.keys().map(String::as_str).collect();
            user.extend(names.iter().copied());
            if let Some(s) = did_you_mean(word, user) {
                self.err(start, format!("Unknown symbol `{word}`."), Some(format!("Did you mean `{s}`?")));
            } else if !ops.is_empty() {
                self.err(start, format!("Unknown symbol `{word}`."), Some(format!("Define it with `sym {word} = ...`, or write single letters with spaces.")));
            } else {
                let spaced = word.chars().map(String::from).collect::<Vec<_>>().join(" ");
                self.warn(start, format!("`{word}` is not a symbol name, so it shows as single letters."), Some(format!("For a product, write `{spaced}`. For a new name, write `sym {word} = ...`.")));
            }
            let nodes: Vec<Node> = word
                .chars()
                .map(|c| {
                    let cp = symbols::italic_letter(c).unwrap_or(c as u32);
                    italic_atom(cp)
                })
                .collect();
            return Node::Row(nodes);
        };
        for (o, at) in ops {
            if symbols::op_def(o).is_none() {
                if let Some((_, when)) = symbols::LATER_OPS.iter().find(|(n, _)| n == o) {
                    self.err(*at, format!("Operation `{o}` is not in Functor 1."), Some(when.to_string()));
                } else {
                    let hint = did_you_mean(o, symbols::OPS.iter().map(|d| d.name)).map(|s| format!("Did you mean `{s}`?"));
                    self.err(*at, format!("Unknown operation `{o}`."), hint.or(Some("See the list of operations in the reference.".into())));
                }
                return glyph_node(base_cp, class, limits, italic, large);
            }
        }
        if ops.is_empty() {
            trace_build(word, ops, Source::Font(base_cp));
            return glyph_node(base_cp, class, limits, italic, large);
        }
        if ops.iter().any(|(o, _)| o == "big") {
            class = Class::Op;
            limits = self.limits_rule(word, Limits::Display);
            large = display;
        }
        if let Some(cp) = symbols::composite(word, &ops_text[1..]) {
            if glyph::font_glyph(cp, large).is_some() {
                glyph::note_font_glyph();
                trace_build(&format!("{word}{ops_text}"), ops, Source::Composite(cp));
                return glyph_node(cp, class, limits, false, large);
            }
        }
        let prefix = (1..ops.len()).rev().find_map(|k| {
            let tail = join_ops(&ops[..k]);
            symbols::composite(word, &tail[1..]).filter(|cp| glyph::font_glyph(*cp, large).is_some()).map(|cp| (k, cp))
        });
        let (from_cp, rest, from_italic) = match prefix {
            Some((k, cp)) => (cp, &ops[k..], false),
            None => (base_cp, ops, italic),
        };
        let Some(base) = Glyph::from_font(from_cp, large) else {
            return glyph_node(base_cp, class, limits, italic, large);
        };
        let mut base = base;
        base.italic = from_italic;
        let recipe = format!("{word}{ops_text}{}", if large { ":L" } else { "" });
        trace_build(&format!("{word}{ops_text}"), ops, Source::Built);
        let letter = (prefix.is_none() && word.chars().count() == 1).then(|| word.chars().next()).flatten();
        let greek = if prefix.is_none() { symbols::greek_index(word) } else { None };
        match self.apply_ops(Rc::new(base), rest, &recipe, large, letter, greek) {
            Some(g) => Node::Glyph { g, class, limits, italic, large },
            None => glyph_node(base_cp, class, limits, italic, large),
        }
    }

    fn limits_rule(&self, word: &str, default: Limits) -> Limits {
        match self.ctx.env.kinds.get(&format!("{word}.limits")).map(String::as_str) {
            Some("over") => Limits::Display,
            Some("side") => Limits::Never,
            _ => default,
        }
    }

    fn apply_ops(&mut self, base: Rc<Glyph>, ops: &[(String, usize)], recipe: &str, large: bool, letter: Option<char>, greek: Option<usize>) -> Option<Rc<Glyph>> {
        let built = glyph::cached_with_notes(recipe, || {
            let mut errors: glyph::Notes = Vec::new();
            let mut g = (*base).clone();
            for (i, (o, _)) in ops.iter().enumerate() {
                let def = symbols::op_def(o)?;
                g = match def.kind {
                    OpKind::Decorate => match g.decorate(def.mark, o == "under") {
                        Some(x) => x,
                        None => {
                            errors.push((i, format!("The font has no mark for `{o}`.")));
                            g
                        }
                    },
                    OpKind::Enclose => g.enclose(o),
                    OpKind::Strike => g.strike(o),
                    OpKind::Repeat => g.repeat(def.mark as usize),
                    OpKind::Reflect => g.reflect(o),
                    OpKind::Magnify => g.magnified(def.mark as f64 / 100.0),
                    OpKind::Deform if o == "bend" => deform::bend(&g, def.mark as f64),
                    OpKind::Deform if o == "slant" => deform::slant(&g, def.mark as f64),
                    OpKind::Deform if o == "taper" => deform::taper(&g, def.mark as f64),
                    OpKind::Deform => {
                        let axis = if o == "wide" { deform::Axis::X } else { deform::Axis::Y };
                        let size = if axis == deform::Axis::Y { g.h + g.d } else { g.w };
                        deform::deform(&g, axis, size * (def.mark as f64 / 100.0 - 1.0))
                    }
                    OpKind::Fence => g,
                    OpKind::Style => {
                        let swap = if i == 0 { letter.and_then(|c| symbols::styled_letter(o, c)).or_else(|| greek.and_then(|g| symbols::styled_greek(o, g))) } else { None };
                        match (swap.and_then(|cp| Glyph::from_font(cp, large)), o.as_str()) {
                            (Some(mut x), _) => {
                                x.italic = o == "bold" || (o == "alt" && greek.is_some());
                                x
                            }
                            (None, "bold") => g.emboldened(),
                            (None, "bb") => {
                                errors.push((i, "`bb` works only on a single letter or digit.".into()));
                                g
                            }
                            (None, "cal") => {
                                errors.push((i, "`cal` works only on a single capital letter.".into()));
                                g
                            }
                            (None, "alt") => {
                                errors.push((i, "This symbol has no other common form.".into()));
                                g
                            }
                            (None, _) => {
                                errors.push((i, format!("`{o}` works only on a single letter, as the first operation.")));
                                g
                            }
                        }
                    }
                };
            }
            Some((g, errors))
        });
        let (g, notes) = built?;
        for (i, e) in notes.iter() {
            self.err(ops[*i].1, e.clone(), None);
        }
        Some(g)
    }
}

fn trace_build(recipe: &str, ops: &[(String, usize)], glyph: Source) {
    trace::edit(|t| {
        t.recipe = recipe.to_string();
        t.ops = ops.iter().map(|(o, _)| o.clone()).collect();
        t.glyph = glyph;
    });
}

fn style_like(op: &str) -> bool {
    op == "arrow" || symbols::op_def(op).is_some_and(|d| d.kind == OpKind::Style)
}

fn rule_text(env: &MathEnv, kind: &str, key: &str) -> String {
    let value = if (kind, key) == ("vec", "style") { env.vec_style.clone() } else { env.opt(kind, key) };
    let set = if (kind, key) == ("vec", "style") { value != "arrow" } else { env.kinds.contains_key(&format!("{kind}.{key}")) };
    format!("`set {kind}({key}: {value})`{}", if set { "" } else { ", the default" })
}

fn norm(t: &str) -> String {
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_fence(op: &str) -> bool {
    symbols::fence_def(op).is_some()
}

fn builtin_fence(f: &symbols::FenceDef, mods: &[String]) -> (Option<Rc<Glyph>>, Option<Rc<Glyph>>) {
    let double = mods.iter().any(|m| m == "x2");
    let pick = |single: u32, dbl: Option<u32>| -> Rc<Glyph> {
        if double {
            if let Some(g) = dbl.and_then(|cp| Glyph::from_font(cp, false)) {
                return Rc::new(g);
            }
            return Rc::new(Glyph::from_font(single, false).unwrap_or_else(|| Glyph::empty(300.0)).repeat(2));
        }
        font_glyph_rc(single)
    };
    let open = pick(f.open, f.double.map(|d| d.0));
    let close = pick(f.close, f.double.map(|d| d.1));
    let left = mods.iter().any(|m| m == "left");
    let right = mods.iter().any(|m| m == "right");
    ((!right).then_some(open), (!left).then_some(close))
}

pub fn fence_pair(spec: &str, env: &MathEnv) -> (Option<Rc<Glyph>>, Option<Rc<Glyph>>) {
    let spec = spec.trim();
    if spec.is_empty() || spec == "none" {
        return (None, None);
    }
    let (base, mods) = split_recipe(spec);
    if let Some(f) = symbols::fence_def(&base) {
        if mods.iter().all(|m| symbols::FENCE_MODIFIERS.contains(&m.as_str())) {
            return builtin_fence(f, &mods);
        }
    }
    let mut diags = Diagnostics::default();
    let pos = |_: usize| (0, 0);
    let mut ctx = Ctx { env, diags: &mut diags, pos: &pos, display: false };
    let node = trace::paused(|| parse(spec, &mut ctx));
    match single_glyph(&node) {
        Some(g) => {
            let close = Rc::new(g.reflect("flip"));
            (Some(g), Some(close))
        }
        None => (None, None),
    }
}

fn join_ops(ops: &[(String, usize)]) -> String {
    ops.iter().map(|(o, _)| format!(".{o}")).collect()
}

pub fn split_recipe(recipe: &str) -> (String, Vec<String>) {
    let mut parts = recipe.trim().split('.');
    let base = parts.next().unwrap_or("").to_string();
    (base, parts.map(str::to_string).collect())
}

fn font_glyph_rc(cp: u32) -> Rc<Glyph> {
    Rc::new(Glyph::from_font(cp, false).unwrap_or_else(|| Glyph::empty(300.0)))
}

fn font_atom(cp: u32, class: Class) -> Node {
    Node::Glyph { g: font_glyph_rc(cp), class, limits: Limits::Never, italic: false, large: false }
}

fn node_class(n: &Node) -> Option<Class> {
    match n {
        Node::Glyph { class, .. } => Some(*class),
        Node::Row(v) if v.len() == 1 => node_class(&v[0]),
        Node::Text(t) => node_class(t),
        _ => None,
    }
}

fn single_glyph(n: &Node) -> Option<Rc<Glyph>> {
    match n {
        Node::Glyph { g, .. } => Some(g.clone()),
        Node::Row(v) if v.len() == 1 => single_glyph(&v[0]),
        Node::Text(t) => single_glyph(t),
        _ => None,
    }
}

thread_local! {
    static FONT_RIGHT: std::cell::RefCell<HashMap<usize, (f64, f64, f64, f64)>> = std::cell::RefCell::new(HashMap::new());
}

fn italic_correction(g: &Glyph) -> f64 {
    if g.items.is_empty() {
        return 0.0;
    }
    let mut right = f64::NEG_INFINITY;
    for it in &g.items {
        let b = match &it.shape {
            glyph::Shape::Font(p) => FONT_RIGHT.with(|c| {
                *c.borrow_mut().entry(p.as_ptr() as usize).or_insert_with(|| {
                    let one = Glyph { w: 0.0, h: 0.0, d: 0.0, items: vec![glyph::Item { shape: glyph::Shape::Font(p), tf: Affine::ID, bold: false }], italic: false };
                    one.ink_box()
                })
            }),
            _ => {
                let one = Glyph { w: 0.0, h: 0.0, d: 0.0, items: vec![glyph::Item { shape: it.shape.clone(), tf: Affine::ID, bold: false }], italic: false };
                one.ink_box()
            }
        };
        for (x, y) in [(b.0, b.1), (b.2, b.1), (b.0, b.3), (b.2, b.3)] {
            right = right.max(it.tf.apply(x, y).0);
        }
    }
    if right.is_finite() {
        (right - g.w).max(0.0)
    } else {
        0.0
    }
}

fn ink_bottom(b: &LBox) -> f64 {
    ink_extent(b).1
}

fn ink_extent(b: &LBox) -> (f64, f64, f64, f64) {
    let mut e = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for (a, d) in &b.items {
        let corners = match d {
            Draw::G(g) => {
                if g.items.is_empty() {
                    continue;
                }
                let (x0, y0, x1, y1) = g.ink_box();
                [(x0, y0), (x1, y0), (x0, y1), (x1, y1)]
            }
            Draw::Rule(w, h) => [(0.0, 0.0), (*w, 0.0), (0.0, *h), (*w, *h)],
        };
        for (x, y) in corners {
            let (px, py) = a.apply(x, y);
            e = (e.0.min(px), e.1.min(py), e.2.max(px), e.3.max(py));
        }
    }
    e
}

fn italic_atom(cp: u32) -> Node {
    let mut g = Glyph::from_font(cp, false).unwrap_or_else(|| Glyph::empty(400.0));
    g.italic = true;
    Node::Glyph { g: Rc::new(g), class: Class::Ord, limits: Limits::Never, italic: true, large: false }
}

fn glyph_node(cp: u32, class: Class, limits: Limits, italic: bool, large: bool) -> Node {
    let mut g = Glyph::from_font(cp, large).unwrap_or_else(|| Glyph::empty(400.0));
    g.italic = italic;
    Node::Glyph { g: Rc::new(g), class, limits, italic, large }
}

fn word_glyph(cps: &[u32], kind: &str) -> Rc<Glyph> {
    let key: String = format!("{kind}:{}", cps.iter().filter_map(|c| char::from_u32(*c)).collect::<String>());
    glyph::cached(&key, || {
        let mut out = Glyph::empty(0.0);
        for &cp in cps {
            match Glyph::from_font(cp, false) {
                Some(g) => {
                    out.items.extend(g.transformed(Affine::translate(out.w, 0.0)));
                    out.w += g.w;
                    out.h = out.h.max(g.h);
                    out.d = out.d.max(g.d);
                }
                None => out.w += 333.0,
            }
        }
        Some(out)
    })
    .unwrap_or_else(|| Rc::new(Glyph::empty(0.0)))
}

#[derive(Clone)]
enum Draw {
    G(Rc<Glyph>),
    Rule(f64, f64),
}

#[derive(Clone)]
pub struct LBox {
    pub w: f64,
    pub h: f64,
    pub d: f64,
    items: Vec<(Affine, Draw)>,
}

impl LBox {
    fn empty() -> LBox {
        LBox { w: 0.0, h: 0.0, d: 0.0, items: Vec::new() }
    }

    fn place(&mut self, other: LBox, x: f64, y: f64) {
        let t = Affine::translate(x, y);
        for (a, d) in other.items {
            self.items.push((a.then(t), d));
        }
    }
}

#[derive(Clone, Copy)]
struct St {
    scale: f64,
    display: bool,
    level: u8,
}

impl St {
    fn script(self) -> St {
        St { scale: (self.scale * 0.7).max(0.5), display: false, level: self.level + 1 }
    }
}

const MU: f64 = 1000.0 / 18.0;

fn space(a: Class, b: Class, st: St) -> f64 {
    use Class::*;
    let mu = match (a, b) {
        (Ord, Op) | (Op, Ord) | (Op, Op) | (Close, Op) => 3.0,
        (Ord, Bin) | (Bin, Ord) | (Bin, Op) | (Bin, Open) | (Close, Bin) | (Op, Bin) => 4.0,
        (Ord, Rel) | (Rel, Ord) | (Rel, Op) | (Rel, Open) | (Close, Rel) | (Op, Rel) | (Bin, Rel) => 5.0,
        (Punct, _) => 3.0,
        _ => 0.0,
    };
    if st.level > 0 && mu > 3.0 {
        return 0.0;
    }
    mu * MU * st.scale
}

fn layout_stack(rows: &[Node], st: St) -> LBox {
    let boxes: Vec<LBox> = rows.iter().map(|r| layout(r, st)).collect();
    let gap = 90.0 * st.scale;
    let w = boxes.iter().map(|b| b.w).fold(0.0, f64::max);
    let mut out = LBox::empty();
    out.w = w;
    let mut y = 0.0;
    for (i, b) in boxes.into_iter().enumerate() {
        if i == 0 {
            out.h = b.h;
        } else {
            y -= b.h + gap;
        }
        let (bw, bd) = (b.w, b.d);
        out.place(b, (w - bw) / 2.0, y);
        out.d = -y + bd;
        y -= bd;
    }
    out
}

fn is_text(n: &Node) -> bool {
    matches!(n, Node::Text(_))
}

fn is_diff(n: &Node) -> bool {
    match n {
        Node::Diff(_) => true,
        Node::Scripts { base, .. } => is_diff(base),
        _ => false,
    }
}

struct Bands {
    lo: Option<f64>,
    hi: Option<f64>,
    over: f64,
}

fn bands(n: &Node, st: St) -> Option<Bands> {
    let Node::Scripts { base, sub, sup } = n else { return None };
    let Node::Glyph { g, limits: Limits::Display, .. } = base.as_ref() else { return None };
    if !st.display {
        return None;
    }
    let s = st.scale;
    let gap = 120.0 * s;
    Some(Bands {
        lo: sub.as_ref().map(|_| g.d * s + gap),
        hi: sup.as_ref().map(|_| g.h * s + gap),
        over: 0.0,
    })
}

fn layout_row(nodes: &[Node], st: St) -> LBox {
    let mut items: Vec<(LBox, f64, Option<Bands>)> = Vec::new();
    let mut prev: Option<Class> = None;
    let mut prev_text = false;
    for n in nodes {
        let mut class = n.class();
        if class == Class::Bin && matches!(prev, None | Some(Class::Bin | Class::Rel | Class::Open | Class::Op | Class::Punct)) {
            class = Class::Ord;
        }
        let mut sp = prev.map_or(0.0, |p| space(p, class, st));
        let text = is_text(n);
        if let Some(p) = prev {
            if (text || prev_text) && !matches!(p, Class::Punct | Class::Open) && !matches!(class, Class::Punct | Class::Close) {
                sp = sp.max(278.0 * st.scale);
            }
        }
        if prev.is_some_and(|p| !matches!(p, Class::Open | Class::Punct)) && is_diff(n) {
            sp = sp.max(167.0 * st.scale);
        }
        let b = layout(n, st);
        let mut bd = bands(n, st);
        if let Some(x) = bd.as_mut() {
            let base_w = match n {
                Node::Scripts { base, .. } => layout(base, st).w,
                _ => b.w,
            };
            x.over = ((b.w - base_w) / 2.0).max(0.0);
        }
        items.push((b, sp, bd));
        prev = Some(class);
        prev_text = text;
    }
    let pad = 60.0 * st.scale;
    let clear = |b: &LBox, bd: &Bands| bd.lo.map_or(true, |lo| b.d + pad <= lo) && bd.hi.map_or(true, |hi| b.h + pad <= hi);
    let n = items.len();
    let mut cut = vec![(0.0f64, 0.0f64); n];
    for k in 0..n {
        let Some(bd) = &items[k].2 else { continue };
        if bd.over <= 0.0 {
            continue;
        }
        let mut room = 0.0;
        let mut j = k + 1;
        while j < n && room < bd.over && clear(&items[j].0, bd) && items[j].2.is_none() {
            room += items[j].1 + items[j].0.w;
            j += 1;
        }
        let right = room.min(bd.over);
        let mut room = 0.0;
        let mut j = k;
        while j > 0 && room < bd.over && clear(&items[j - 1].0, bd) && items[j - 1].2.is_none() {
            room += items[j].1 + items[j - 1].0.w;
            j -= 1;
        }
        let left = room.min(bd.over);
        let keep = 2.0 * pad;
        cut[k] = ((left - keep).max(0.0).min(bd.over), (right - keep).max(0.0).min(bd.over));
    }
    let mut out = LBox::empty();
    for (k, (b, sp, _)) in items.into_iter().enumerate() {
        let (l, r) = cut[k];
        out.w += sp;
        let x = out.w - l;
        out.h = out.h.max(b.h);
        out.d = out.d.max(b.d);
        out.w += b.w - l - r;
        out.place(b, x, 0.0);
    }
    out
}

fn layout(node: &Node, st: St) -> LBox {
    let s = st.scale;
    match node {
        Node::Glyph { g, .. } => LBox {
            w: (g.w + italic_correction(g)) * s,
            h: g.h * s,
            d: g.d * s,
            items: vec![(Affine::scale(s, s), Draw::G(g.clone()))],
        },
        Node::Row(nodes) => layout_row(nodes, st),
        Node::Scripts { base, sub, sup } => {
            let b = layout(base, st);
            let (limits, large, italic) = match base.as_ref() {
                Node::Glyph { limits, large, italic, .. } => (*limits == Limits::Display && st.display, *large, *italic),
                _ => (false, false, false),
            };
            let ss = st.script();
            let lay = |n: &Node| match n {
                Node::Conds(rows) if limits => layout_stack(rows, ss),
                _ => layout(n, ss),
            };
            let (prime, rest) = match sup.as_deref() {
                Some(Node::Prime(_)) if !limits => (sup.as_deref(), None),
                Some(Node::Row(v)) if !limits && matches!(v.first(), Some(Node::Prime(_))) => {
                    (v.first(), if v.len() > 1 { Some(Node::Row(v[1..].to_vec())) } else { None })
                }
                _ => (None, sup.as_deref().cloned()),
            };
            let sub_b = sub.as_deref().map(lay);
            let sup_b = rest.as_ref().map(lay);
            let prime_b = prime.map(|n| layout(n, st));
            let mut out = LBox::empty();
            if limits {
                let w = b.w.max(sub_b.as_ref().map_or(0.0, |x| x.w)).max(sup_b.as_ref().map_or(0.0, |x| x.w));
                out.w = w;
                out.h = b.h;
                out.d = b.d;
                let gap = 120.0 * s;
                if let Some(x) = sup_b {
                    let y = b.h + gap + x.d;
                    out.h = y + x.h;
                    let xw = x.w;
                    out.place(x, (w - xw) / 2.0, y);
                }
                if let Some(x) = sub_b {
                    let y = -(b.d + gap + x.h);
                    out.d = -y + x.d;
                    let xw = x.w;
                    out.place(x, (w - xw) / 2.0, y);
                }
                let bw = b.w;
                out.place(b, (w - bw) / 2.0, 0.0);
                return out;
            }
            let mut u = (if st.display { 410.0 } else { 360.0 } * s).max(b.h - 250.0 * s);
            let mut v = (160.0 * s).max(b.d + 60.0 * s);
            if large {
                u = u.max(b.h - 300.0 * s);
                v = v.max(b.d - 150.0 * s);
            }
            if let (Some(p), Some(q)) = (&sup_b, &sub_b) {
                let gap = (u - p.d) - (q.h - v);
                if gap < 160.0 * s {
                    v += 160.0 * s - gap;
                }
            }
            out.h = b.h;
            out.d = b.d;
            let bw = b.w;
            let ic = match base.as_ref() {
                Node::Glyph { g, .. } => italic_correction(g) * s,
                _ => 0.0,
            };
            let ink_right = match base.as_ref() {
                Node::Glyph { g, .. } if large => g.ink_box().2 * s,
                _ => bw,
            };
            let sup_dx = if large { (ink_right - bw).max(0.0) + 90.0 * s } else if italic { 40.0 * s } else { 20.0 * s };
            let sub_dx = if large && ink_right > bw + 1.0 { -0.28 * (bw - ic) - ic } else if large { 30.0 * s } else { 10.0 * s - ic };
            out.place(b, 0.0, 0.0);
            out.w = bw;
            let mut sup_x = bw + sup_dx;
            if let (Some(p), Some(q)) = (&prime_b, &sub_b) {
                let bottom = ink_bottom(p);
                if bottom.is_finite() && q.h - v > bottom - 60.0 * s {
                    v = q.h - bottom + 60.0 * s;
                }
            }
            if let Some(x) = prime_b {
                let px = bw + if italic { 20.0 * s } else { 10.0 * s };
                out.h = out.h.max(x.h);
                out.w = out.w.max(px + x.w);
                sup_x = px + x.w + 20.0 * s;
                out.place(x, px, 0.0);
            }
            if let Some(x) = sup_b {
                out.h = out.h.max(u + x.h);
                out.w = out.w.max(sup_x + x.w);
                out.place(x, sup_x, u);
            }
            if let Some(x) = sub_b {
                out.d = out.d.max(v + x.d);
                out.w = out.w.max(bw + sub_dx + x.w);
                out.place(x, bw + sub_dx, -v);
            }
            out.w += 30.0 * s;
            out
        }
        Node::Text(inner) | Node::Prime(inner) | Node::Diff(inner) => layout(inner, st),
        Node::Conds(rows) => {
            let mut v = Vec::new();
            for (i, r) in rows.iter().enumerate() {
                if i > 0 {
                    v.push(font_atom(',' as u32, Class::Punct));
                }
                v.push(r.clone());
            }
            layout_row(&v, st)
        }
        Node::Frac(num, den) => {
            let cs = if st.display { St { display: false, ..st } } else { st.script() };
            let n = layout(num, cs);
            let d = layout(den, cs);
            let axis = 250.0 * s;
            let t = 40.0 * s;
            let gap = if st.display { 110.0 } else { 70.0 } * s;
            let pad = 80.0 * s;
            let w = n.w.max(d.w) + 2.0 * pad;
            let mut out = LBox::empty();
            out.w = w;
            let ny = axis + t / 2.0 + gap + n.d;
            let dy = axis - t / 2.0 - gap - d.h;
            out.h = ny + n.h;
            out.d = -(dy - d.d);
            out.items.push((Affine::translate(pad / 2.0, axis - t / 2.0), Draw::Rule(w - pad, t)));
            let (nw, dw) = (n.w, d.w);
            out.place(n, (w - nw) / 2.0, ny);
            out.place(d, (w - dw) / 2.0, dy);
            out
        }
        Node::Sqrt(inner) => {
            let b = layout(inner, st);
            let rad = font_glyph_rc(0x221A);
            let t = 40.0 * s;
            let (il, ib, ir, it) = ink_extent(&b);
            let shift = if il.is_finite() { (-il).max(0.0) } else { 0.0 };
            let inner_w = if ir.is_finite() { b.w.max(ir) + shift } else { b.w };
            let top = b.h.max(if it.is_finite() { it } else { 0.0 }) + 110.0 * s;
            let bottom = b.d.max(if ib.is_finite() { -ib } else { 0.0 }) + 60.0 * s;
            let need = top + bottom;
            let have = (rad.h + rad.d) * s;
            let sy = (need / have).max(1.0);
            let rad = deform::grown(&rad, deform::Axis::Y, sy);
            let ty = top - rad.h * s;
            let mut out = LBox::empty();
            let rw = rad.w * s;
            out.items.push((Affine::scale(s, s).then(Affine::translate(0.0, ty)), Draw::G(rad.clone())));
            out.items.push((Affine::translate(rw - 10.0 * s, top - t), Draw::Rule(inner_w + 60.0 * s, t)));
            let bw = inner_w;
            out.place(b, rw + 30.0 * s + shift, 0.0);
            out.w = rw + bw + 70.0 * s;
            out.h = top;
            out.d = bottom.max(-(ty - rad.d * s));
            out
        }
        Node::Grid { rows, aligns, col_gap, row_gap, display } => {
            let cs = St { display: *display && st.display, ..st };
            let boxes: Vec<Vec<LBox>> = rows.iter().map(|r| r.iter().map(|c| layout(c, cs)).collect()).collect();
            let ncols = boxes.iter().map(Vec::len).max().unwrap_or(0);
            let mut widths = vec![0.0f64; ncols];
            for r in &boxes {
                for (j, b) in r.iter().enumerate() {
                    widths[j] = widths[j].max(b.w);
                }
            }
            let (strut_h, strut_d) = (720.0 * s, 260.0 * s);
            let heights: Vec<(f64, f64)> = boxes
                .iter()
                .map(|r| r.iter().fold((strut_h, strut_d), |(h, d), b| (h.max(b.h), d.max(b.d))))
                .collect();
            let gap = row_gap * s;
            let total: f64 = heights.iter().map(|(h, d)| h + d).sum::<f64>() + gap * (heights.len().saturating_sub(1)) as f64;
            let axis = 250.0 * s;
            let top_y = axis + total / 2.0;
            let mut out = LBox::empty();
            let mut y = top_y;
            for (r, (h, d)) in boxes.into_iter().zip(heights) {
                let base = y - h;
                let mut x = 0.0;
                for (j, b) in r.into_iter().enumerate() {
                    let w = widths[j];
                    let dx = match aligns.get(j).or(aligns.last()).copied().unwrap_or(CellAlign::Center) {
                        CellAlign::Left => 0.0,
                        CellAlign::Center => (w - b.w) / 2.0,
                        CellAlign::Right => w - b.w,
                    };
                    out.place(b, x + dx, base);
                    x += w + col_gap * s;
                }
                y = base - d - gap;
            }
            out.w = widths.iter().sum::<f64>() + col_gap * s * (ncols.saturating_sub(1)) as f64;
            out.h = top_y;
            out.d = total - top_y;
            out
        }
        Node::Over { g, above, below, .. } => {
            let ls = st.script();
            let a = above.as_ref().map(|n| layout(n, ls));
            let b = below.as_ref().map(|n| layout(n, ls));
            let need = (a.as_ref().map_or(0.0, |x| x.w).max(b.as_ref().map_or(0.0, |x| x.w)) + 600.0 * s).max(g.w * s * 1.4);
            let gl = if need > g.w * s && stretchy(g) { deform::grown(g, deform::Axis::X, need / (g.w * s)) } else { g.clone() };
            let w = (gl.w * s).max(need);
            let (_, iy0, _, iy1) = gl.ink_box();
            let mut out = LBox::empty();
            out.w = w;
            out.h = gl.h * s;
            out.d = gl.d * s;
            out.items.push((Affine::scale(s, s).then(Affine::translate((w - gl.w * s) / 2.0, 0.0)), Draw::G(gl.clone())));
            if let Some(x) = a {
                let y = iy1 * s + 80.0 * s + x.d;
                out.h = out.h.max(y + x.h);
                let xw = x.w;
                out.place(x, (w - xw) / 2.0, y);
            }
            if let Some(x) = b {
                let y = iy0 * s - 80.0 * s - x.h;
                out.d = out.d.max(-(y - x.d));
                let xw = x.w;
                out.place(x, (w - xw) / 2.0, y);
            }
            out
        }
        Node::Small(inner) => {
            let k = 0.7;
            let b = layout(inner, St { scale: s * k, display: false, level: st.level });
            let dy = 250.0 * s * (1.0 - k);
            let mut out = LBox::empty();
            out.w = b.w;
            out.h = b.h + dy;
            out.d = (b.d - dy).max(0.0);
            out.place(b, 0.0, dy);
            out
        }
        Node::Diagram { rows, edges, gap, cross } => layout_diagram(rows, edges, *gap, *cross, st),
        Node::Fenced { open, close, body, grow } => {
            let b = layout(body, st);
            let axis = 250.0 * s;
            let mut out = LBox::empty();
            let (bh, bd) = (b.h, b.d);
            let magnify = *grow == Grow::Magnify;
            let delim = |g: &Rc<Glyph>, out: &mut LBox| {
                let gh = (g.h + g.d) * s;
                let content = bh.max(axis) + bd.max(0.0);
                let sy = if content > gh * 1.05 { (2.0 * (bh - axis).max(bd + axis) + 120.0 * s) / gh } else { 1.0 };
                let sy = sy.max(1.0);
                let (m, g) = if sy <= 1.0 {
                    (1.0, g.clone())
                } else if magnify {
                    (sy, g.clone())
                } else {
                    let (m, rest) = deform::fence_split(sy);
                    (m, deform::grown(g, deform::Axis::Y, rest))
                };
                let ty = if sy > 1.0 { axis - (g.h - g.d) / 2.0 * s * m } else { 0.0 };
                let x = out.w;
                out.items.push((Affine::scale(s * m, s * m).then(Affine::translate(x, ty)), Draw::G(g.clone())));
                out.w += (g.w + italic_correction(&g)) * s * m;
                out.h = out.h.max(ty + g.h * s * m);
                out.d = out.d.max(-(ty - g.d * s * m));
            };
            if let Some(g) = open {
                delim(g, &mut out);
            }
            let x = out.w;
            out.h = out.h.max(b.h);
            out.d = out.d.max(b.d);
            out.w += b.w;
            out.place(b, x, 0.0);
            if let Some(g) = close {
                delim(g, &mut out);
            }
            out
        }
    }
}

fn stretchy(g: &Glyph) -> bool {
    deform::plain_share(g, deform::Axis::X) >= 0.3
}

fn rotate(ux: f64, uy: f64) -> Affine {
    Affine([ux, uy, -uy, ux, 0.0, 0.0])
}

struct Bounds(f64, f64, f64, f64);

impl Bounds {
    fn add(&mut self, x: f64, y: f64) {
        self.0 = self.0.min(x);
        self.1 = self.1.min(y);
        self.2 = self.2.max(x);
        self.3 = self.3.max(y);
    }

    fn add_glyph(&mut self, g: &Glyph, a: Affine) {
        let (x0, y0, x1, y1) = g.ink_box();
        for (x, y) in [(x0, y0), (x0, y1), (x1, y0), (x1, y1)] {
            let (px, py) = a.apply(x, y);
            self.add(px, py);
        }
    }
}

fn edge_glyph(g: &Rc<Glyph>, len: f64) -> Glyph {
    let (gx0, _, gx1, _) = g.ink_box();
    let iw = (gx1 - gx0).max(1.0);
    if stretchy(g) {
        let gl = if len > iw { deform::grown(g, deform::Axis::X, (len - iw + g.w) / g.w) } else { g.clone() };
        let (x0, _, x1, _) = gl.ink_box();
        let k = (len / (x1 - x0)).min(1.0);
        let tf = Affine::translate(-x0, 0.0).then(Affine::scale(k, 1.0));
        return Glyph { w: len, h: gl.h, d: gl.d, items: gl.transformed(tf), italic: false };
    }
    let mut items = g.transformed(Affine::translate(len / 2.0 - (gx0 + gx1) / 2.0, 0.0));
    let side = (len - iw) / 2.0 - 120.0;
    if side > 150.0 {
        for a in [0.0, len - side] {
            items.push(glyph::Item { shape: glyph::Shape::Stroke(format!("M{} 250L{} 250", f(a), f(a + side))), tf: Affine::ID, bold: false });
        }
    }
    Glyph { w: len, h: g.h, d: g.d, items, italic: false }
}

fn turn((x, y): (f64, f64), a: f64) -> (f64, f64) {
    let (c, s) = (a.cos(), a.sin());
    (x * c - y * s, x * s + y * c)
}

struct Path {
    place: Affine,
    len: f64,
    half: f64,
    thick: f64,
    pts: Vec<(f64, f64, f64)>,
    mid: (f64, f64),
    normal: (f64, f64),
    two: bool,
}

fn seg_hit(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> Option<(f64, f64)> {
    let r = (b.0 - a.0, b.1 - a.1);
    let q = (d.0 - c.0, d.1 - c.1);
    let den = r.0 * q.1 - r.1 * q.0;
    if den.abs() < 1e-9 {
        return None;
    }
    let t = ((c.0 - a.0) * q.1 - (c.1 - a.1) * q.0) / den;
    let u = ((c.0 - a.0) * r.1 - (c.1 - a.1) * r.0) / den;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some((t, u))
}

fn layout_diagram(rows: &[Vec<Node>], edges: &[Edge], gap: f64, cross: bool, st: St) -> LBox {
    let s = st.scale;
    let cs = St { display: false, ..st };
    let ls = st.script();
    let axis = 250.0 * s;
    let boxes: Vec<Vec<LBox>> = rows.iter().map(|r| r.iter().map(|c| layout(c, cs)).collect()).collect();
    let nrows = boxes.len();
    let ncols = boxes.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0.0f64; ncols];
    for r in &boxes {
        for (j, b) in r.iter().enumerate() {
            widths[j] = widths[j].max(b.w);
        }
    }
    let heights: Vec<(f64, f64)> = boxes.iter().map(|r| r.iter().fold((680.0 * s, 200.0 * s), |(h, d), b| (h.max(b.h), d.max(b.d)))).collect();
    let labels: Vec<Vec<(LBox, Side)>> = edges.iter().map(|e| e.labels.iter().map(|(n, side)| (layout(n, ls), *side)).collect()).collect();
    let mut col_gaps = vec![2600.0 * s * gap; ncols.saturating_sub(1)];
    let mut row_gaps = vec![1900.0 * s * gap; nrows.saturating_sub(1)];
    for (e, ls) in edges.iter().zip(&labels) {
        let (End::Cell(fr, fc), End::Cell(tr, tc)) = (e.from, e.to) else { continue };
        let lw = ls.iter().map(|(b, _)| b.w).fold(0.0, f64::max);
        let gw = if stretchy(&e.g) { 0.0 } else { e.g.w * s + 600.0 * s };
        let gw = if e.bend != 0.0 { gw.max(4200.0 * s) } else { gw };
        if fr == tr && fc.abs_diff(tc) == 1 {
            let c = fc.min(tc);
            col_gaps[c] = col_gaps[c].max(lw + 700.0 * s).max(gw);
        }
        if fc == tc && fr.abs_diff(tr) == 1 {
            let r = fr.min(tr);
            let lh = ls.iter().filter(|(_, side)| *side != Side::Auto).map(|(b, _)| b.h + b.d).fold(0.0, f64::max);
            row_gaps[r] = row_gaps[r].max(gw).max(lh + 700.0 * s);
        }
    }
    let mut xs = Vec::with_capacity(ncols);
    let mut x = 0.0;
    for (j, w) in widths.iter().enumerate() {
        xs.push(x + w / 2.0);
        x += w + col_gaps.get(j).copied().unwrap_or(0.0);
    }
    let total: f64 = heights.iter().map(|(h, d)| h + d).sum::<f64>() + row_gaps.iter().sum::<f64>();
    let mut bases = Vec::with_capacity(nrows);
    let mut y = axis + total / 2.0;
    for (i, (h, d)) in heights.iter().enumerate() {
        bases.push(y - h);
        y -= h + d + row_gaps.get(i).copied().unwrap_or(0.0);
    }
    let mut out = LBox::empty();
    let mut bd = Bounds(f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    let mut extent = vec![vec![(0.0f64, 0.0f64, 0.0f64); ncols]; nrows];
    for (i, r) in boxes.into_iter().enumerate() {
        for (j, b) in r.into_iter().enumerate() {
            let (cx, base) = (xs[j], bases[i]);
            extent[i][j] = (b.w / 2.0, b.h.max(450.0 * s) - axis, b.d.max(0.0) + axis);
            bd.add(cx - b.w / 2.0, base - b.d);
            bd.add(cx + b.w / 2.0, base + b.h);
            let bw = b.w;
            out.place(b, cx - bw / 2.0, base);
        }
    }
    let centre = |(r, c): (usize, usize)| (xs[c], bases[r] + axis);
    let mid = (xs.iter().sum::<f64>() / ncols.max(1) as f64, bases.iter().map(|b| b + axis).sum::<f64>() / nrows.max(1) as f64);
    let pad = 160.0 * s;
    let exit = |(r, c): (usize, usize), (ux, uy): (f64, f64)| {
        let (hw, up, down) = extent[r][c];
        let tx = if ux.abs() > 1e-9 { (hw + pad) / ux.abs() } else { f64::MAX };
        let ty = if uy > 1e-9 { (up + pad) / uy } else if uy < -1e-9 { (down + pad) / -uy } else { f64::MAX };
        tx.min(ty)
    };
    let mut paths: Vec<Path> = Vec::with_capacity(edges.len());
    for e in edges {
        let thick = {
            let (_, y0, _, y1) = e.g.ink_box();
            (y1 - y0) * s
        };
        let (p0, p1, half, two) = match (e.from, e.to) {
            (End::Arrow(a), End::Arrow(b)) if a < paths.len() && b < paths.len() => {
                let (ma, mb) = (paths[a].mid, paths[b].mid);
                let d = ((mb.0 - ma.0).powi(2) + (mb.1 - ma.1).powi(2)).sqrt().max(1e-6);
                let u = ((mb.0 - ma.0) / d, (mb.1 - ma.1) / d);
                let ta = paths[a].thick / 2.0 + 200.0 * s;
                let tb = paths[b].thick / 2.0 + 200.0 * s;
                ((ma.0 + u.0 * ta, ma.1 + u.1 * ta), (mb.0 - u.0 * tb, mb.1 - u.1 * tb), 0.0, true)
            }
            (End::Cell(fr, fc), End::Cell(tr, tc)) => {
                let (ax, ay) = centre((fr, fc));
                let (bx, by) = centre((tr, tc));
                let dist = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt().max(1e-6);
                let u = ((bx - ax) / dist, (by - ay) / dist);
                let half = e.bend.to_radians();
                let u0 = turn(u, half);
                let u1 = turn(u, -half);
                let t0 = exit((fr, fc), u0);
                let t1 = exit((tr, tc), (-u1.0, -u1.1));
                ((ax + u0.0 * t0, ay + u0.1 * t0), (bx - u1.0 * t1, by - u1.1 * t1), half, false)
            }
            _ => ((0.0, 0.0), (0.0, 0.0), 0.0, true),
        };
        let chord = ((p1.0 - p0.0).powi(2) + (p1.1 - p0.1).powi(2)).sqrt().max(200.0 * s);
        let uc = ((p1.0 - p0.0) / chord, (p1.1 - p0.1) / chord);
        let arc_len = if half.abs() > 1e-6 { chord * half / half.sin() } else { chord };
        let len = arc_len / s;
        let c = (len / 2.0, 250.0);
        let start = deform::arc_point((0.0, 250.0), half, len, c);
        let place = Affine::translate(-start.0, -start.1).then(Affine::scale(s, s)).then(rotate(uc.0, uc.1)).then(Affine::translate(p0.0, p0.1));
        let n = if half.abs() > 1e-6 { 16 } else { 1 };
        let pts = (0..=n)
            .map(|k| {
                let lx = len * k as f64 / n as f64;
                let q = deform::arc_point((lx, 250.0), half, len, c);
                let (px, py) = place.apply(q.0, q.1);
                (px, py, lx)
            })
            .collect();
        let m = deform::arc_point((len / 2.0, 250.0), half, len, c);
        let mid = place.apply(m.0, m.1);
        paths.push(Path { place, len, half, thick, pts, mid, normal: (-uc.1, uc.0), two });
    }
    let mut gaps: Vec<Vec<(f64, f64)>> = vec![Vec::new(); paths.len()];
    if cross {
        for i in 0..paths.len() {
            for j in i + 1..paths.len() {
                if paths[i].two || paths[j].two {
                    continue;
                }
                let (pi, pj) = (&paths[i].pts, &paths[j].pts);
                for a in pi.windows(2) {
                    for b in pj.windows(2) {
                        let Some((t, u)) = seg_hit((a[0].0, a[0].1), (a[1].0, a[1].1), (b[0].0, b[0].1), (b[1].0, b[1].1)) else { continue };
                        let li = a[0].2 + (a[1].2 - a[0].2) * t;
                        let lj = b[0].2 + (b[1].2 - b[0].2) * u;
                        let end = 500.0;
                        if li < end || li > paths[i].len - end || lj < end || lj > paths[j].len - end {
                            continue;
                        }
                        let w = paths[j].thick / s / 2.0 + 200.0;
                        gaps[i].push((li - w, li + w));
                    }
                }
            }
        }
    }
    for ((e, ls), (p, cuts)) in edges.iter().zip(labels).zip(paths.iter().zip(&gaps)) {
        let mut gl = edge_glyph(&e.g, p.len);
        if !cuts.is_empty() {
            gl = deform::cut(&gl, cuts);
        }
        if p.half.abs() > 1e-6 {
            gl = deform::arc(&gl, p.half, p.len, (p.len / 2.0, 250.0));
        }
        let gl = Rc::new(gl);
        bd.add_glyph(&gl, p.place);
        out.items.push((p.place, Draw::G(gl)));
        let (lx, ly) = p.normal;
        let auto_left = if p.half.abs() > 1e-6 {
            p.half > 0.0
        } else {
            let dot = lx * (p.mid.0 - mid.0) + ly * (p.mid.1 - mid.1);
            if dot.abs() < 1.0 { true } else { dot > 0.0 }
        };
        for (b, side) in ls {
            let left = match side {
                Side::Left => true,
                Side::Right => false,
                Side::Auto => auto_left,
            };
            let (nx, ny) = if left { (lx, ly) } else { (-lx, -ly) };
            let reach = b.w / 2.0 * nx.abs() + (b.h + b.d) / 2.0 * ny.abs() + p.thick / 2.0 + 90.0 * s;
            let (cx, cy) = (p.mid.0 + nx * reach, p.mid.1 + ny * reach);
            let (bx0, by0) = (cx - b.w / 2.0, cy - (b.h - b.d) / 2.0);
            bd.add(bx0, by0 - b.d);
            bd.add(bx0 + b.w, by0 + b.h);
            out.place(b, bx0, by0);
        }
    }
    if bd.0 > bd.2 {
        return out;
    }
    let mut fin = LBox::empty();
    fin.w = bd.2 - bd.0;
    fin.h = bd.3.max(0.0);
    fin.d = (-bd.1).max(0.0);
    fin.place(out, -bd.0, 0.0);
    fin
}

pub fn render(node: &Node, display: bool, label: &str) -> String {
    let b = layout(node, St { scale: 1.0, display, level: 0 });
    render_box(&b, display, Some(label))
}

pub struct MathPiece {
    pub svg: String,
    pub lbox: Rc<LBox>,
    pub w: f64,
    pub gap: Option<(f64, f64, f64, f64)>,
}

fn row_classes(nodes: &[Node]) -> Vec<Class> {
    let mut prev: Option<Class> = None;
    nodes
        .iter()
        .map(|n| {
            let mut class = n.class();
            if class == Class::Bin && matches!(prev, None | Some(Class::Bin | Class::Rel | Class::Open | Class::Op | Class::Punct)) {
                class = Class::Ord;
            }
            prev = Some(class);
            class
        })
        .collect()
}

pub fn inline_pieces(node: &Node, label: &str) -> Vec<MathPiece> {
    let st = St { scale: 1.0, display: false, level: 0 };
    let nodes: &[Node] = match node {
        Node::Row(v) if v.len() > 1 => v,
        _ => {
            let b = layout(node, st);
            return vec![MathPiece { svg: render_box(&b, false, Some(label)), w: b.w.max(1.0) / 1000.0, lbox: Rc::new(b), gap: None }];
        }
    };
    let classes = row_classes(nodes);
    let mut pieces = Vec::new();
    let mut start = 0;
    for i in 0..nodes.len() {
        let last = i + 1 == nodes.len();
        let breakable = !last
            && matches!(classes[i], Class::Rel | Class::Bin)
            && !matches!(classes[i + 1], Class::Rel | Class::Punct | Class::Close);
        if !breakable && !last {
            continue;
        }
        let b = layout_row(&nodes[start..=i], st);
        let gap = if last {
            None
        } else {
            let w = space(classes[i], classes[i + 1], st);
            let mu = w / MU;
            let (stretch, shrink) = if mu >= 5.0 { (w, 0.0) } else if mu >= 4.0 { (w / 2.0, w) } else { (0.0, 0.0) };
            let penalty = if classes[i] == Class::Rel { 500.0 } else { 700.0 };
            Some((w / 1000.0, stretch / 1000.0, shrink / 1000.0, penalty))
        };
        let lab = if start == 0 { Some(label) } else { None };
        pieces.push(MathPiece { svg: render_box(&b, false, lab), w: b.w.max(1.0) / 1000.0, lbox: Rc::new(b), gap });
        start = i + 1;
    }
    pieces
}

pub fn display_box(node: &Node) -> LBox {
    layout(node, St { scale: 1.0, display: true, level: 0 })
}

pub enum Paint<'a> {
    Glyph(Affine, &'a Glyph),
    Rule(Affine, f64, f64),
}

impl LBox {
    pub fn paint(&self, f: &mut dyn FnMut(Paint)) {
        for (a, d) in &self.items {
            match d {
                Draw::G(g) => f(Paint::Glyph(*a, g)),
                Draw::Rule(w, h) => f(Paint::Rule(*a, *w, *h)),
            }
        }
    }
}

fn render_box(b: &LBox, display: bool, label: Option<&str>) -> String {
    let pad = 40.0;
    let (w, h, d) = (b.w.max(1.0), b.h.max(0.0), b.d.max(0.0));
    let mut body = String::new();
    let flip = Affine([1.0, 0.0, 0.0, -1.0, 0.0, 0.0]);
    for (a, draw) in &b.items {
        match draw {
            Draw::G(g) => g.to_svg_group(&mut body, a.then(flip)),
            Draw::Rule(rw, rh) => {
                let m = a.then(flip).0;
                body.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
                    f(m[4]),
                    f(m[5] - rh),
                    f(*rw),
                    f(*rh)
                ));
            }
        }
    }
    let total = h + d + 2.0 * pad;
    format!(
        "<svg class=\"fn-math{}\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 {} {} {}\" style=\"width:{}em;height:{}em;vertical-align:{}em\" fill=\"currentColor\" {}>{}</svg>",
        if display { " fn-display-math" } else { "" },
        f(-h - pad),
        f(w),
        f(total),
        f(w / 1000.0),
        f(total / 1000.0),
        f(-(d + pad) / 1000.0),
        match label {
            Some(l) => format!("role=\"img\" aria-label=\"{}\"", crate::escape::escape_attr(l)),
            None => "aria-hidden=\"true\"".into(),
        },
        body
    )
}

fn f(x: f64) -> String {
    let v = (x * 10.0).round() / 10.0;
    if v == v.trunc() {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

pub fn glyph_preview(recipe: &str, env: &MathEnv) -> Option<String> {
    let mut diags = Diagnostics::default();
    let pos = |_: usize| (1, 1);
    let node = {
        let mut ctx = Ctx { env, diags: &mut diags, pos: &pos, display: false };
        parse(recipe, &mut ctx)
    };
    if diags.items.iter().any(|d| d.severity == crate::diag::Severity::Error) {
        return None;
    }
    Some(render(&node, false, recipe))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(src: &str, display: bool) -> (Node, Vec<crate::diag::Diagnostic>) {
        let mut env = MathEnv::new();
        env.syms.insert("csum".into(), "sum.circle".into());
        env.objects.insert("F".into(), "vec".into());
        let mut diags = Diagnostics::default();
        let pos = |o: usize| (1, o + 1);
        let n = {
            let mut ctx = Ctx { env: &env, diags: &mut diags, pos: &pos, display };
            parse(src, &mut ctx)
        };
        (n, diags.finish())
    }

    #[test]
    fn parses_common_formulas_without_errors() {
        for src in [
            "F = m a",
            "csum_(k = 1)^n a_k",
            "integral.x2_D f d x d y",
            "x in.not A",
            "theta.hat",
            "frac(a + b, 2) = sqrt(x^2 + 1)",
            "E.flip x, A.turn y",
            "lim_(n to infty) (1 + frac(1, n))^n",
            "vec(v) + F'",
        ] {
            let (_, d) = run(src, true);
            assert!(d.is_empty(), "{src}: {d:?}");
        }
    }

    #[test]
    fn unknown_names_suggest() {
        let (_, d) = run("alpah + sum.circel", false);
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].hint.as_deref(), Some("Did you mean `alpha`?"));
        assert_eq!(d[1].hint.as_deref(), Some("Did you mean `circle`?"));
        assert_eq!(d[1].col, 13);
    }

    #[test]
    fn composite_uses_font_glyph() {
        glyph::take_stats();
        let _ = run("integral.circle", false);
        assert_eq!(glyph::take_stats().from_font, 1);
    }

    #[test]
    fn big_makes_a_large_operator_from_the_font() {
        glyph::take_stats();
        let (n, d) = run("plus.circle.big_(i = 1)^n V_i", true);
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(glyph::take_stats().from_font, 1);
        let Node::Row(v) = n else { panic!() };
        assert!(v.iter().any(|x| matches!(x, Node::Scripts { .. }) || matches!(x, Node::Glyph { class: Class::Op, .. })));
    }

    #[test]
    fn new_operations_build_without_errors() {
        for src in ["to.rot", "to.flip.rot", "x.ring", "v.harpoon", "L.cal", "phi.alt", "mu.up", "alpha.bold", "k.bb", "le.alt", "lt.x2", "bot.flipv", "stack(lt, sim)", "stack(\"def\", eq)", "stack(sum, plus)", "x.rot", "limsup_(n to infty) a_n", "Hom(A, B)"] {
            let (_, d) = run(src, false);
            assert!(d.is_empty(), "{src}: {d:?}");
        }
    }

    #[test]
    fn stack_and_rot_use_the_font_when_they_can() {
        glyph::take_stats();
        let _ = run("stack(lt, sim) + to.rot", false);
        assert_eq!(glyph::take_stats().from_font, 2);
    }

    #[test]
    fn style_errors_explain_themselves() {
        let (_, d) = run("alpha.alt", false);
        assert!(d[0].message.contains("no other common form"), "{d:?}");
        let (_, d) = run("x.cal", false);
        assert!(d[0].message.contains("capital"), "{d:?}");
    }

    #[test]
    fn duplicate_codes_give_a_warning() {
        let (_, d) = run("\u{2126}", false);
        assert_eq!(d.len(), 1);
        assert!(d[0].hint.as_deref().unwrap_or("").contains("Omega.up"));
    }

    #[test]
    fn built_glyph_is_cached() {
        glyph::clear_cache();
        glyph::take_stats();
        let _ = run("sum.circle + sum.circle", false);
        let s = glyph::take_stats();
        assert_eq!((s.built, s.hits), (1, 1));
    }

    #[test]
    fn cached_recipe_keeps_its_errors() {
        glyph::clear_cache();
        let (_, d) = run("x.cal + x.cal", false);
        let cols: Vec<usize> = d.iter().map(|d| d.col).collect();
        assert_eq!(cols, vec![3, 11]);
        let (_, d) = run("x.cal", false);
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("cal"));
    }

    #[test]
    fn preview_refuses_bad_recipe_twice() {
        glyph::clear_cache();
        let env = MathEnv::new();
        assert!(glyph_preview("x.cal", &env).is_none());
        assert!(glyph_preview("x.cal", &env).is_none());
    }

    #[test]
    fn disk_cache_round_trip() {
        glyph::clear_cache();
        let (a, _) = run("sum.circle + x.hat.box + x.cal + x.tall", false);
        let before = render(&a, false, "x");
        let text = glyph::export_cache();
        assert!(text.contains("\nO\t"));
        assert!(glyph::take_dirty());
        glyph::clear_cache();
        assert_eq!(glyph::import_cache(&text), 4);
        assert!(!glyph::take_dirty());
        glyph::take_stats();
        let (b, d) = run("sum.circle + x.hat.box + x.cal + x.tall", false);
        let s = glyph::take_stats();
        assert_eq!((s.built, s.hits), (0, 4));
        assert_eq!(d.len(), 1);
        assert_eq!(render(&b, false, "x"), before);
        assert_eq!(glyph::export_cache(), text);
    }

    #[test]
    fn disk_cache_refuses_other_versions() {
        glyph::clear_cache();
        let _ = run("sum.circle", false);
        let text = glyph::export_cache().replacen(glyph::cache_key(), "0000000000000000", 1);
        glyph::clear_cache();
        assert_eq!(glyph::import_cache(&text), 0);
        assert_eq!(glyph::import_cache("garbage"), 0);
        assert_eq!(glyph::cache_len(), 0);
    }

    #[test]
    fn density_warning() {
        let (_, d) = run("x.hat.circle.not.flip", false);
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("4 operations"));
    }

    fn diagram_of(n: &Node) -> (&Vec<Vec<Node>>, &Vec<Edge>) {
        match n {
            Node::Row(v) if v.len() == 1 => diagram_of(&v[0]),
            Node::Small(b) => diagram_of(b),
            Node::Diagram { rows, edges, .. } => (rows, edges),
            _ => panic!("not a diagram"),
        }
    }

    #[test]
    fn labelled_arrow_grows_to_fit() {
        let (n, d) = run("A to^(f circ g) B", true);
        assert!(d.is_empty(), "{d:?}");
        let Node::Row(v) = &n else { panic!() };
        assert!(matches!(&v[1], Node::Over { above: Some(_), below: None, .. }));
        let w = layout(&v[1], St { scale: 1.0, display: true, level: 0 }).w;
        let plain = Glyph::from_font(0x2192, false).unwrap().w;
        assert!(w > plain * 1.5, "{w} vs {plain}");
        let (n, _) = run("x =_p y", false);
        let Node::Row(v) = &n else { panic!() };
        assert!(matches!(&v[1], Node::Over { above: None, below: Some(_), .. }));
        let (n, _) = run("x^2", false);
        assert!(matches!(&n, Node::Row(v) if matches!(v[0], Node::Scripts { .. })));
    }

    #[test]
    fn diagram_is_a_grid_with_edges() {
        let (n, d) = run("diagram(A, B; C, D | f: A to B, g: A to C, B to_h D, C mapsto D, A = D)", true);
        assert!(d.is_empty(), "{d:?}");
        let (rows, edges) = diagram_of(&n);
        assert_eq!((rows.len(), rows[0].len()), (2, 2));
        let ends: Vec<_> = edges.iter().map(|e| (e.from, e.to)).collect();
        let c = |r, k| End::Cell(r, k);
        assert_eq!(ends, vec![(c(0, 0), c(0, 1)), (c(0, 0), c(1, 0)), (c(0, 1), c(1, 1)), (c(1, 0), c(1, 1)), (c(0, 0), c(1, 1))]);
        assert_eq!(edges[0].labels[0].1, Side::Auto);
        assert_eq!(edges[2].labels[0].1, Side::Right);
        assert!(edges[4].labels.is_empty());
        let svg = render(&n, true, "d");
        assert!(svg.contains("<path"));
    }

    #[test]
    fn curved_arrows_and_two_cells() {
        let (n, d) = run("diagram(A, B | f: A to.bend B, g: A to.bend.flipv B, alpha: f implies g)", true);
        assert!(d.is_empty(), "{d:?}");
        let (_, edges) = diagram_of(&n);
        assert_eq!(edges[0].bend, 30.0);
        assert_eq!(edges[1].bend, -30.0);
        assert_eq!((edges[2].from, edges[2].to), (End::Arrow(0), End::Arrow(1)));
        let (_, d) = run("diagram(A, B | A to^f B, A to_g B, f implies g)", true);
        assert!(d.is_empty(), "{d:?}");
        let (_, d) = run("diagram(A, B | f: A to B, f implies B)", true);
        assert!(d.iter().any(|x| x.message.contains("arrow to an arrow")));
        let (_, d) = run("diagram(A, B | f: A to B, f implies f)", true);
        assert!(d.iter().any(|x| x.message.contains("two different arrows")));
        let svg = render(&n, true, "x");
        assert!(svg.contains("<path"));
    }

    #[test]
    fn bend_outside_a_diagram_bends_the_glyph() {
        let (n, d) = run("A to.bend B", true);
        assert!(d.is_empty(), "{d:?}");
        let Node::Row(v) = &n else { panic!() };
        let g = single(&v[1]);
        let glyph::Shape::Outline(_) = &g.items[0].shape else { panic!("not deformed") };
    }

    #[test]
    fn crossing_arrows_get_a_gap() {
        let count = |src: &str| {
            let (n, _) = run(src, true);
            render(&n, true, "x").matches('M').count()
        };
        let gap = count("diagram(A, B; C, D | A to B, A to D, B to C, C to D)");
        let plain = count("diagram(A, B; C, D | A to B, A to D, C to D, B to C)");
        assert!(gap > count("diagram(A, B; C, D | A to B, C to D)"));
        assert!(gap >= plain);
    }

    #[test]
    fn diagram_names_cells_by_content_or_position() {
        let (n, d) = run("diagram(A, A; B, C | 1.1 to 1.2, f: A_1 to B)", true);
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].message.contains("not a cell"));
        assert_eq!(diagram_of(&n).1[0].to, End::Cell(0, 1));
        let (_, d) = run("diagram(A, A | A to A)", true);
        assert!(d[0].message.contains("more than one cell"));
        assert!(d[0].hint.as_deref().unwrap_or("").contains("2.1"));
        let (_, d) = run("diagram(A, B | A B)", true);
        assert!(d[0].message.contains("no arrow symbol"));
        let (_, d) = run("diagram(A, B | 3.1 to B)", true);
        assert!(d[0].message.contains("row 3"));
        let (_, d) = run("diagram(x + y, z | x + y to z)", true);
        assert!(d.is_empty(), "{d:?}");
    }

    #[test]
    fn inline_grids_are_small_by_rule() {
        let (n, _) = run("mat(a, b; c, d)", false);
        assert!(matches!(&n, Node::Row(v) if matches!(v[0], Node::Small(_))));
        let (n, _) = run("mat(a, b; c, d)", true);
        assert!(matches!(&n, Node::Row(v) if !matches!(v[0], Node::Small(_))));
        let mut env = MathEnv::new();
        env.kinds.insert("mat.inline".into(), "full".into());
        let mut diags = Diagnostics::default();
        let pos = |o: usize| (1, o + 1);
        let mut ctx = Ctx { env: &env, diags: &mut diags, pos: &pos, display: false };
        let n = parse("mat(a, b; c, d)", &mut ctx);
        assert!(matches!(&n, Node::Row(v) if !matches!(v[0], Node::Small(_))));
        let (n, _) = run("vec(1, 2)", false);
        assert!(matches!(&n, Node::Row(v) if matches!(v[0], Node::Small(_))));
        let (n, _) = run("diagram(A, B | A to B)", false);
        assert!(matches!(&n, Node::Row(v) if matches!(v[0], Node::Small(_))));
    }

    #[test]
    fn renders_svg() {
        let (n, _) = run("sum.circle_k a_k", true);
        let svg = render(&n, true, "x");
        assert!(svg.starts_with("<svg") && svg.contains("<path"));
    }

    fn single(n: &Node) -> Rc<Glyph> {
        single_glyph(n).expect("one glyph")
    }

    #[test]
    fn style_swaps_to_font_letters() {
        for (src, cp) in [("C.frak", 0x212D), ("c.frak", 0x1D520), ("o.script", 0x2134), ("L.script", 0x2112), ("u.up", 0x75)] {
            let (n, d) = run(src, false);
            assert!(d.is_empty(), "{src}: {d:?}");
            let want = Glyph::from_font(cp, false).unwrap();
            assert_eq!(single(&n).items, want.items, "{src}");
        }
    }

    #[test]
    fn style_on_a_word_is_an_error() {
        let (_, d) = run("sum.frak", false);
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("`frak` works only on a single letter"));
    }

    #[test]
    fn join_makes_one_touching_glyph() {
        let (n, d) = run("join(o.up, r.up)", false);
        assert!(d.is_empty(), "{d:?}");
        let g = single(&n);
        let (o, r) = (Glyph::from_font(0x6F, false).unwrap(), Glyph::from_font(0x72, false).unwrap());
        assert_eq!(g.items.len(), o.items.len() + r.items.len());
        let (_, _, ox1, _) = o.ink_box();
        let (gx0, _, gx1, _) = g.ink_box();
        let (rx0, _, rx1, _) = r.ink_box();
        assert!(gx1 - gx0 < (ox1 - gx0) + (rx1 - rx0), "the two inks overlap");
    }

    #[test]
    fn join_needs_two_single_symbols() {
        let (_, d) = run("join(x)", false);
        assert!(d[0].message.contains("takes 2 arguments"));
        let (_, d) = run("join(x + y, z)", false);
        assert!(d[0].message.contains("two single symbols"));
    }

    #[test]
    fn neg_pulls_symbols_closer() {
        let width = |src: &str| layout(&run(src, false).0, St { scale: 1.0, display: false, level: 0 }).w;
        assert!((width("x neg y") - (width("x y") - 83.0)).abs() < 1e-6);
    }

    #[test]
    fn logo_wordmark_builds() {
        let env = MathEnv::new();
        assert!(glyph_preview("(F.bb_(neg neg neg T.up.small)).arrow.square", &env).is_some());
        assert!(glyph_preview("F.bb.square neg u.up.tilde_(neg u.up.tilde.turn) neg c.frak neg neg T.up neg join(o.up, r.up.arrow)", &env).is_some());
    }

    #[test]
    fn group_takes_operations() {
        let (n, d) = run("(x_T).box", false);
        assert!(d.is_empty(), "{d:?}");
        let g = single(&n);
        assert_eq!(g.items.len(), 3);
        let (n, _) = run("(x_T)", false);
        assert!(matches!(n, Node::Row(ref v) if matches!(v[0], Node::Fenced { .. })));
    }

    #[test]
    fn square_has_equal_sides() {
        let (n, d) = run("(x_T).square", false);
        assert!(d.is_empty(), "{d:?}");
        let g = single(&n);
        let glyph::Shape::Stroke(p) = &g.items.last().unwrap().shape else { panic!("no stroke") };
        let pts = glyph::path_points(p);
        let (w, h) = ((pts[1].0 - pts[0].0).abs(), (pts[2].1 - pts[1].1).abs());
        assert!((w - h).abs() < 1e-6, "{w} {h}");
    }

    #[test]
    fn small_scales_by_four_fifths() {
        let (n, d) = run("T.up.small", false);
        assert!(d.is_empty(), "{d:?}");
        let t = Glyph::from_font(0x54, false).unwrap();
        assert!((single(&n).w - t.w * 0.8).abs() < 1e-6);
    }

    fn top(n: Node) -> Node {
        match n {
            Node::Row(mut v) if v.len() == 1 => v.remove(0),
            other => other,
        }
    }

    fn grid_of(n: &Node) -> (usize, usize) {
        match n {
            Node::Grid { rows, .. } => (rows.len(), rows.iter().map(Vec::len).max().unwrap_or(0)),
            Node::Fenced { body, .. } | Node::Small(body) => grid_of(body),
            Node::Row(v) if v.len() == 1 => grid_of(&v[0]),
            _ => (0, 0),
        }
    }

    #[test]
    fn structures_parse_without_errors() {
        for src in [
            "mat(1, 0; 0, 1)",
            "mat(\n  a, b;\n  c, d\n)",
            "vec(1, 2, 3)",
            "cases(x, x ge 0; -x, \"otherwise\")",
            "binom(n, k) + abs(x) + norm(v) + inner(u, v) + floor(x) + ceil(x)",
            "(a + b).angle + (x).vbar.x2 + (M).brack.x2 + x.angle + theta.hat.vbar",
            "wrap(L.script, x + y) + wrap(T.frak, S.frak, a) + grid(1, 2; 3, 4).brack",
            "(x).brace.left",
        ] {
            let (_, d) = run(src, false);
            assert!(d.is_empty(), "{src}: {d:?}");
        }
    }

    #[test]
    fn matrix_is_a_fenced_grid() {
        let (n, _) = run("mat(1, 2, 3; 4, 5, 6)", false);
        assert_eq!(grid_of(&n), (2, 3));
        let (n, _) = run("mat(\n  a, b;\n  c, d;\n)", false);
        assert_eq!(grid_of(&n), (2, 2));
    }

    #[test]
    fn vector_with_components_is_a_column() {
        let (n, _) = run("vec(1, 2, 3)", false);
        assert_eq!(grid_of(&n), (3, 1));
        let (n, d) = run("vec(F)", false);
        assert!(d.is_empty());
        assert!(single_glyph(&n).is_some(), "a named vector stays one glyph");
    }

    #[test]
    fn fence_x2_doubles() {
        let n = top(run("(x).vbar.x2", false).0);
        let Node::Fenced { open: Some(o), .. } = &n else { panic!("not fenced: {n:?}") };
        assert_eq!(o.items, Glyph::from_font(0x2225, false).unwrap().items);
        let n = top(run("(x).angle.x2", false).0);
        let Node::Fenced { open: Some(o), .. } = &n else { panic!("not fenced") };
        assert_eq!(o.items.len(), 2, "built by repeat");
    }

    #[test]
    fn one_sided_fence() {
        let n = top(run("(x).brace.left", false).0);
        assert!(matches!(n, Node::Fenced { open: Some(_), close: None, .. }));
    }

    #[test]
    fn wrap_mirrors_one_glyph() {
        let (n, d) = run("wrap(L.script, x)", false);
        assert!(d.is_empty(), "{d:?}");
        let n = top(n);
        let Node::Fenced { open: Some(o), close: Some(c), .. } = &n else { panic!("not fenced") };
        assert_eq!(c.items, o.reflect("flip").items);
        let (_, d) = run("wrap(x + y, z)", false);
        assert!(d[0].message.contains("single symbols"));
    }

    #[test]
    fn magnify_grows_the_width() {
        let width = |grow: &str| {
            let mut env = MathEnv::new();
            env.kinds.insert("wrap.grow".into(), grow.into());
            let mut diags = Diagnostics::default();
            let pos = |o: usize| (1, o + 1);
            let n = {
                let mut ctx = Ctx { env: &env, diags: &mut diags, pos: &pos, display: true };
                parse("wrap(L.script, mat(1; 2; 3; 4))", &mut ctx)
            };
            layout(&n, St { scale: 1.0, display: true, level: 0 }).w
        };
        assert!(width("magnify") > width("stretch") + 100.0);
    }

    fn parse_display(src: &str, diags: &mut Diagnostics) -> Node {
        let env = MathEnv::new();
        let pos = |o: usize| (1, o + 1);
        let mut ctx = Ctx { env: &env, diags, pos: &pos, display: true };
        parse(src, &mut ctx)
    }

    #[test]
    fn integral_upper_limit_clears_the_sign() {
        let mut diags = Diagnostics::default();
        let n = parse_display("integral^1", &mut diags);
        let b = layout(&n, St { scale: 1.0, display: true, level: 0 });
        let sign = Glyph::from_font(0x222B, true).unwrap();
        let right = sign.ink_box().2;
        let (a, _) = b.items.last().unwrap();
        assert!(a.0[4] >= right + 20.0, "limit at {} sign ends at {right}", a.0[4]);
    }

    #[test]
    fn tall_adds_half_the_height() {
        let mut diags = Diagnostics::default();
        let n = parse_display("x.tall", &mut diags);
        let Some(g) = single_glyph(&n) else { panic!("no glyph") };
        let x = Glyph::from_font(0x1D465, false).unwrap();
        assert!(((g.h + g.d) - 1.5 * (x.h + x.d)).abs() < 2.0);
        assert_eq!(diags.items.len(), 0);
    }

    #[test]
    fn deform_word_points_to_tall_and_wide() {
        let mut diags = Diagnostics::default();
        parse_display("x.deform", &mut diags);
        assert!(diags.items.iter().any(|d| d.hint.as_deref().is_some_and(|h| h.contains("tall"))));
    }

    #[test]
    fn style_rule_picks_the_fence() {
        let mut env = MathEnv::new();
        env.kinds.insert("mat.fence".into(), "brack".into());
        let (open, _) = fence_pair(&env.opt("mat", "fence"), &env);
        assert_eq!(open.unwrap().items, Glyph::from_font(0x5B, false).unwrap().items);
        assert_eq!(env.opt("norm", "fence"), "vbar.x2");
        assert_eq!(env.grow("vec"), Grow::Stretch);
    }

    fn run_env(src: &str, env: &MathEnv) -> (Node, Vec<crate::diag::Diagnostic>) {
        let mut diags = Diagnostics::default();
        let pos = |o: usize| (1, o + 1);
        let n = {
            let mut ctx = Ctx { env, diags: &mut diags, pos: &pos, display: false };
            parse(src, &mut ctx)
        };
        (n, diags.finish())
    }

    #[test]
    fn arrow_kinds_take_their_look_from_a_rule() {
        let mut env = MathEnv::new();
        for (word, cp) in [("mono", 0x21A3), ("epi", 0x21A0), ("incl", 0x21AA)] {
            let (n, d) = run_env(word, &env);
            assert!(d.is_empty(), "{word}: {d:?}");
            assert_eq!(single(&n).items, Glyph::from_font(cp, false).unwrap().items, "{word}");
        }
        env.kinds.insert("mono.style".into(), "hook".into());
        env.kinds.insert("epi.style".into(), "plain".into());
        for (word, cp) in [("mono", 0x21AA), ("epi", 0x2192), ("incl", 0x21AA)] {
            let (n, _) = run_env(word, &env);
            assert_eq!(single(&n).items, Glyph::from_font(cp, false).unwrap().items, "{word}");
            assert_eq!(node_class(&n), Some(Class::Rel));
        }
        let (n, d) = run_env("diagram(A, B | f: A mono B)", &env);
        assert!(d.is_empty(), "{d:?}");
        let t = top(n);
        let (_, edges) = diagram_of(&t);
        assert_eq!(edges[0].g.items, Glyph::from_font(0x21AA, false).unwrap().items);
    }

    #[test]
    fn greek_rule_changes_every_letter_but_an_explicit_style_wins() {
        let mut env = MathEnv::new();
        env.kinds.insert("greek.phi".into(), "alt".into());
        let (n, d) = run_env("phi", &env);
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(single(&n).items, Glyph::from_font(0x1D719, false).unwrap().items);
        let (with_rule, _) = run_env("phi.hat", &env);
        let (without, _) = run_env("phi.hat", &MathEnv::new());
        assert_ne!(single(&with_rule).items, single(&without).items);
        let (n, _) = run_env("phi.up", &env);
        assert_eq!(single(&n).items, Glyph::from_font(0x3C6, false).unwrap().items);
        let (n, _) = run_env("theta", &env);
        assert_eq!(single(&n).items, Glyph::from_font(0x1D703, false).unwrap().items);
    }

    #[test]
    fn yoneda_is_a_built_glyph() {
        let (_, d) = run("yo(A)", false);
        assert!(d.is_empty(), "{d:?}");
        let g = Glyph::from_font(0x3088, false).expect("yo");
        assert!(matches!(g.items[0].shape, glyph::Shape::Outline(_)));
        assert!(g.h > 600.0 && g.w > 500.0);
        let (_, d) = run("yo.box", false);
        assert!(d.is_empty(), "{d:?}");
    }

    #[test]
    fn display_lines_make_an_aligned_chain() {
        let (n, d) = run("(a + b)^2 = (a + b)(a + b)\n  = a^2 + 2 a b + b^2\n  le 2 (a^2 + b^2)", true);
        assert!(d.is_empty(), "{d:?}");
        let Node::Grid { rows, aligns, .. } = &n else { panic!("not a chain") };
        assert_eq!(rows.len(), 3);
        assert_eq!(aligns, &vec![CellAlign::Right, CellAlign::Left]);
        assert!(matches!(&rows[1][0], Node::Row(v) if v.is_empty()), "a line that starts with a relation continues the left side");
        let (n, _) = run("mat(\n a, b;\n c, d\n)", true);
        assert!(!matches!(n, Node::Grid { display: true, .. }), "new lines inside brackets do not split");
    }

    #[test]
    fn chain_errors_point_at_their_line() {
        let mut env = MathEnv::new();
        env.syms.clear();
        let mut diags = Diagnostics::default();
        let src = "a = b\n  = alpah";
        let pos = |o: usize| {
            let before = &src[..o.min(src.len())];
            (1 + before.matches('\n').count(), 1 + before.rsplit('\n').next().unwrap_or("").chars().count())
        };
        {
            let mut ctx = Ctx { env: &env, diags: &mut diags, pos: &pos, display: true };
            parse(src, &mut ctx);
        }
        let d = diags.finish();
        assert_eq!((d[0].line, d[0].col), (2, 5));
    }

    #[test]
    fn ragged_rows_warn() {
        let (_, d) = run("mat(1, 2; 3)", false);
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("different numbers of cells"));
    }

    fn wide(src: &str) -> f64 {
        let (n, d) = run(src, true);
        assert!(d.is_empty(), "{src}: {d:?}");
        display_box(&n).w
    }

    #[test]
    fn text_in_math_gets_a_word_space() {
        assert!(wide("p \"prime\"") > wide("p") + wide("\"prime\"") + 200.0);
        assert!((wide("(\"prime\")") - (wide("(x)") - wide("x") + wide("\"prime\""))).abs() < 1.0);
    }

    #[test]
    fn semicolons_in_a_limit_stack_in_display_and_join_in_text() {
        let (n, d) = run("sum_(p \"prime\"; p le n) 1/p", true);
        assert!(d.is_empty(), "{d:?}");
        let one = display_box(&run("sum_(p le n) 1/p", true).0);
        assert!(display_box(&n).d > one.d + 300.0);
        let text = St { scale: 1.0, display: false, level: 0 };
        let (t, d) = run("sum_(p \"prime\"; p le n) 1/p", false);
        assert!(d.is_empty(), "{d:?}");
        let (u, _) = run("sum_(p le n) 1/p", false);
        assert!(layout(&t, text).d < layout(&u, text).d + 50.0);
    }

    #[test]
    fn a_wide_limit_hangs_under_clear_neighbours_only() {
        let full = wide("x = sum_(1 le i lt j le n) a_i b_j") - wide("x =") - wide("a_i b_j");
        let lone = wide("sum_(1 le i lt j le n)");
        assert!(full < lone - 200.0, "{full} {lone}");
        let deep = wide("sum_(1 le i lt j le n) frac(a_i, b_j)") - wide("frac(a_i, b_j)");
        assert!(deep > lone - 50.0, "{deep} {lone}");
    }

    #[test]
    fn a_prime_is_full_size_at_font_height() {
        let a = display_box(&run("f'", true).0);
        let b = display_box(&run("f^2", true).0);
        assert!(a.h < b.h, "{} {}", a.h, b.h);
        assert!(wide("f'_n") < wide("f'") + wide("n"));
    }

    #[test]
    fn dd_has_a_thin_space_before_it_but_not_at_the_start() {
        assert!(wide("f dd x") > wide("f d x") + 100.0);
        assert!((wide("dd x") - wide("d x")).abs() < 1.0);
    }

}
