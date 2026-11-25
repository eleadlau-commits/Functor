use std::collections::HashMap;
use std::rc::Rc;

use crate::diag::{did_you_mean, Diagnostics};
use crate::glyph::{self, Affine, Glyph};
use crate::symbols::{self, Class, Limits, OpKind};

pub const MAX_OPS: usize = 3;

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
}

impl Node {
    fn class(&self) -> Class {
        match self {
            Node::Glyph { class, .. } => *class,
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
    let mut p = Parser { s: src, i: 0, base, ctx };
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
        let (l, c) = (self.ctx.pos)(self.base + at);
        self.ctx.diags.error(l, c, msg, hint);
    }

    fn warn(&mut self, at: usize, msg: impl Into<String>, hint: Option<String>) {
        let (l, c) = (self.ctx.pos)(self.base + at);
        self.ctx.diags.warning(l, c, msg, hint);
    }

    fn row(&mut self, stops: &[char]) -> Vec<Node> {
        let mut out = Vec::new();
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
                    let g = font_atom(0x2032 + (primes.min(3) as u32 - 1), Class::Ord);
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
                            let inner = self.row(&[')']);
                            if self.peek() == Some(')') {
                                self.bump();
                            } else {
                                self.err(at, "This script group has no closing `)`.", Some("Add `)` where the script ends.".into()));
                            }
                            Some(Node::Row(inner))
                        }
                        Some(_) => self.atom(),
                        None => None,
                    };
                    let Some(arg) = arg else {
                        self.err(at, format!("`{c}` needs a script after it."), Some(format!("For example `x{c}2` or `x{c}(n + 1)`.")));
                        break;
                    };
                    let slot = if c == '_' { &mut sub } else { &mut sup };
                    if slot.is_some() {
                        self.err(at, format!("This term has two `{c}` scripts."), Some("Put both parts in one group, for example `x_(i j)`.".into()));
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
        } else {
            Node::Scripts { base: Box::new(base), sub: sub.map(Box::new), sup: sup.map(Box::new) }
        }
    }

    fn atom(&mut self) -> Option<Node> {
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
                let body = self.row(&[close]);
                let close_glyph = if self.peek() == Some(close) {
                    self.bump();
                    Some(font_glyph_rc(close as u32))
                } else {
                    self.err(start, format!("This `{c}` is not closed."), Some(format!("Add `{close}` where the group ends.")));
                    None
                };
                if c == '(' && close_glyph.is_some() {
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
                Some(Node::Glyph { g: word_glyph(&cps, "text"), class: Class::Ord, limits: Limits::Never, italic: false, large: false })
            }
            _ => {
                self.bump();
                let (cp, class) = match c {
                    '-' => (0x2212, Class::Bin),
                    '*' => (0x2217, Class::Bin),
                    _ => (c as u32, symbols::char_class(c)),
                };
                if glyph::font_glyph(cp, false).is_none() {
                    self.err(start, format!("The font has no glyph for `{c}`."), Some("Use a named symbol instead, or build it from a base and operations.".into()));
                    return None;
                }
                let ops = self.ops();
                if ops.is_empty() {
                    return Some(font_atom(cp, class));
                }
                let base = Rc::new(Glyph::from_font(cp, false)?);
                let recipe = format!("{c}{}", join_ops(&ops));
                let g = self.apply_ops(base, &ops, &recipe, false, None)?;
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
        loop {
            let row = self.row(&[',', ')']);
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
            "frac" | "join" | "binom" | "inner" => 2,
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
                self.fenced("mat", g)
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
            Node::Grid { rows, aligns: vec![CellAlign::Center], col_gap: 0.0, row_gap: 200.0, display: false }
        };
        Some(self.fenced("vec", body))
    }

    fn fenced(&self, kind: &str, body: Node) -> Node {
        let (open, close) = self.fence_pair(&self.ctx.env.opt(kind, "fence"));
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
        Node::Glyph { g, class: Class::Ord, limits: Limits::Never, italic: false, large: false }
    }

    fn group_ops(&mut self, body: Node, ops: &[(String, usize)], start: usize) -> Option<Node> {
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
        let g = self.apply_ops(base, ops, &recipe, false, None)?;
        Some(Node::Glyph { g, class: Class::Ord, limits: Limits::Never, italic: false, large: false })
    }

    fn resolve(&mut self, word: &str, ops: &[(String, usize)], start: usize, depth: usize) -> Node {
        if depth > 8 {
            self.err(start, format!("`{word}` refers to itself."), Some("A `sym` cannot use its own name.".into()));
            return Node::Row(vec![]);
        }
        if let Some(recipe) = self.ctx.env.syms.get(word).cloned() {
            let (base_word, mut base_ops) = split_recipe(&recipe);
            base_ops.extend(ops.iter().map(|(o, _)| o.clone()));
            let with_pos: Vec<(String, usize)> = base_ops.into_iter().map(|o| (o, start)).collect();
            return self.resolve(&base_word, &with_pos, start, depth + 1);
        }
        if let Some(kind) = self.ctx.env.objects.get(word).cloned() {
            if kind == "vec" && !ops.iter().any(|(o, _)| o == "arrow" || o == "bold") {
                let mut all = vec![(self.ctx.env.vec_style.clone(), start)];
                all.extend(ops.iter().cloned());
                return self.resolve_base(word, &all, start);
            }
        }
        self.resolve_base(word, ops, start)
    }

    fn resolve_base(&mut self, word: &str, ops: &[(String, usize)], start: usize) -> Node {
        if let Some(k) = ops.iter().position(|(o, _)| is_fence(o)) {
            let n = self.resolve_base(word, &ops[..k], start);
            return self.node_ops(n, &ops[k..], start).unwrap_or(Node::Row(vec![]));
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
        let (base_cp, class, limits, italic, large) = if let Some(n) = symbols::named(word) {
            (n.cp, n.class, n.limits, false, n.large && display)
        } else if let Some(cp) = symbols::greek(word) {
            (cp, Class::Ord, Limits::Never, true, false)
        } else if let Some(lim) = symbols::function(word) {
            if let Some((o, at)) = ops.first() {
                self.err(*at, format!("`{word}` is a function name; it takes no operation `.{o}`."), None);
            }
            let cps: Vec<u32> = word.chars().map(|c| c as u32).collect();
            return Node::Glyph { g: word_glyph(&cps, "fn"), class: Class::Op, limits: lim, italic: false, large: false };
        } else if word.chars().count() == 1 {
            let c = word.chars().next().unwrap_or('x');
            let cp = symbols::italic_letter(c).unwrap_or(c as u32);
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
                    self.err(*at, format!("Operation `{o}` comes in {when}."), Some("Functor 1 has decorate, enclose, strike, repeat, reflect, style, magnify and fence.".into()));
                } else {
                    let hint = did_you_mean(o, symbols::OPS.iter().map(|d| d.name)).map(|s| format!("Did you mean `{s}`?"));
                    self.err(*at, format!("Unknown operation `{o}`."), hint.or(Some("See the list of operations in the reference.".into())));
                }
                return glyph_node(base_cp, class, limits, italic, large);
            }
        }
        if ops.is_empty() {
            return glyph_node(base_cp, class, limits, italic, large);
        }
        if let Some(cp) = symbols::composite(word, &ops_text[1..]) {
            if glyph::font_glyph(cp, large).is_some() {
                glyph::note_font_glyph();
                return glyph_node(cp, class, limits, false, large);
            }
        }
        let Some(base) = Glyph::from_font(base_cp, large) else {
            return glyph_node(base_cp, class, limits, italic, large);
        };
        let mut base = base;
        base.italic = italic;
        let recipe = format!("{word}{ops_text}{}", if large { ":L" } else { "" });
        let letter = (word.chars().count() == 1).then(|| word.chars().next()).flatten();
        match self.apply_ops(Rc::new(base), ops, &recipe, large, letter) {
            Some(g) => Node::Glyph { g, class, limits, italic, large },
            None => glyph_node(base_cp, class, limits, italic, large),
        }
    }

    fn apply_ops(&mut self, base: Rc<Glyph>, ops: &[(String, usize)], recipe: &str, large: bool, letter: Option<char>) -> Option<Rc<Glyph>> {
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
                    OpKind::Fence => g,
                    OpKind::Style => {
                        let swap = if i == 0 { letter.and_then(|c| symbols::styled_letter(o, c)) } else { None };
                        match (swap.and_then(|cp| Glyph::from_font(cp, large)), o.as_str()) {
                            (Some(mut x), _) => {
                                x.italic = o == "bold";
                                x
                            }
                            (None, "bold") => g.emboldened(),
                            (None, "bb") => {
                                errors.push((i, "`bb` works only on a single capital letter.".into()));
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
    let node = parse(spec, &mut ctx);
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

fn single_glyph(n: &Node) -> Option<Rc<Glyph>> {
    match n {
        Node::Glyph { g, .. } => Some(g.clone()),
        Node::Row(v) if v.len() == 1 => single_glyph(&v[0]),
        _ => None,
    }
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

fn layout_row(nodes: &[Node], st: St) -> LBox {
    let mut out = LBox::empty();
    let mut prev: Option<Class> = None;
    for n in nodes {
        let mut class = n.class();
        if class == Class::Bin && matches!(prev, None | Some(Class::Bin | Class::Rel | Class::Open | Class::Op | Class::Punct)) {
            class = Class::Ord;
        }
        if let Some(p) = prev {
            out.w += space(p, class, st);
        }
        let b = layout(n, st);
        let x = out.w;
        out.h = out.h.max(b.h);
        out.d = out.d.max(b.d);
        out.w += b.w;
        out.place(b, x, 0.0);
        prev = Some(class);
    }
    out
}

fn layout(node: &Node, st: St) -> LBox {
    let s = st.scale;
    match node {
        Node::Glyph { g, .. } => LBox {
            w: g.w * s,
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
            let sub_b = sub.as_ref().map(|n| layout(n, ss));
            let sup_b = sup.as_ref().map(|n| layout(n, ss));
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
            let sup_dx = if italic { 60.0 * s } else { 20.0 * s };
            let sub_dx = if large { -0.28 * bw } else { 10.0 * s };
            out.place(b, 0.0, 0.0);
            out.w = bw;
            if let Some(x) = sup_b {
                out.h = out.h.max(u + x.h);
                out.w = out.w.max(bw + sup_dx + x.w);
                out.place(x, bw + sup_dx, u);
            }
            if let Some(x) = sub_b {
                out.d = out.d.max(v + x.d);
                out.w = out.w.max(bw + sub_dx + x.w);
                out.place(x, bw + sub_dx, -v);
            }
            out.w += 30.0 * s;
            out
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
            let top = b.h + 110.0 * s;
            let bottom = b.d + 60.0 * s;
            let need = top + bottom;
            let have = (rad.h + rad.d) * s;
            let sy = (need / have).max(1.0);
            let ty = top - rad.h * s * sy;
            let mut out = LBox::empty();
            let rw = rad.w * s;
            out.items.push((Affine::scale(s, s * sy).then(Affine::translate(0.0, ty)), Draw::G(rad.clone())));
            out.items.push((Affine::translate(rw - 10.0 * s, top - t), Draw::Rule(b.w + 60.0 * s, t)));
            let bw = b.w;
            out.place(b, rw + 30.0 * s, 0.0);
            out.w = rw + bw + 70.0 * s;
            out.h = top;
            out.d = bottom.max(-(ty - rad.d * s * sy));
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
                let sx = if magnify { sy } else { 1.0 };
                let ty = if sy > 1.0 { axis - (g.h - g.d) / 2.0 * s * sy } else { 0.0 };
                let x = out.w;
                out.items.push((Affine::scale(s * sx, s * sy).then(Affine::translate(x, ty)), Draw::G(g.clone())));
                out.w += g.w * s * sx;
                out.h = out.h.max(ty + g.h * s * sy);
                out.d = out.d.max(-(ty - g.d * s * sy));
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
            Some(l) => format!("role=\"img\" aria-label=\"{}\"", crate::render::escape_attr(l)),
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
        let (_, d) = run("x.bb + x.bb", false);
        let cols: Vec<usize> = d.iter().map(|d| d.col).collect();
        assert_eq!(cols, vec![3, 10]);
        let (_, d) = run("x.bb", false);
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("bb"));
    }

    #[test]
    fn preview_refuses_bad_recipe_twice() {
        glyph::clear_cache();
        let env = MathEnv::new();
        assert!(glyph_preview("x.bb", &env).is_none());
        assert!(glyph_preview("x.bb", &env).is_none());
    }

    #[test]
    fn disk_cache_round_trip() {
        glyph::clear_cache();
        let (a, _) = run("sum.circle + x.hat.box + x.bb", false);
        let before = render(&a, false, "x");
        let text = glyph::export_cache();
        assert!(glyph::take_dirty());
        glyph::clear_cache();
        assert_eq!(glyph::import_cache(&text), 3);
        assert!(!glyph::take_dirty());
        glyph::take_stats();
        let (b, d) = run("sum.circle + x.hat.box + x.bb", false);
        let s = glyph::take_stats();
        assert_eq!((s.built, s.hits), (0, 3));
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
            Node::Fenced { body, .. } => grid_of(body),
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

    #[test]
    fn style_rule_picks_the_fence() {
        let mut env = MathEnv::new();
        env.kinds.insert("mat.fence".into(), "brack".into());
        let (open, _) = fence_pair(&env.opt("mat", "fence"), &env);
        assert_eq!(open.unwrap().items, Glyph::from_font(0x5B, false).unwrap().items);
        assert_eq!(env.opt("norm", "fence"), "vbar.x2");
        assert_eq!(env.grow("vec"), Grow::Stretch);
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
}
