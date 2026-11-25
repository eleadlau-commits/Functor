use std::cell::RefCell;
use std::collections::HashMap;

use functor_paper::diag::{Diagnostics, Severity};
use functor_paper::doc;
use functor_paper::math::{self, Ctx, MathEnv};
use functor_paper::symbols::{self, Class, Limits, OpKind};

use crate::rng::Rng;

const WORDS: &[&str] = &[
    "the",
    "map",
    "is",
    "a",
    "functor",
    "so",
    "every",
    "object",
    "has",
    "an",
    "image",
    "and",
    "we",
    "see",
    "that",
    "this",
    "holds",
    "for",
    "all",
    "points",
    "in",
    "space",
    "let",
    "be",
    "given",
    "then",
    "it",
    "follows",
    "by",
    "induction",
    "on",
    "length",
    "of",
    "chain",
    "hence",
    "result",
    "limit",
    "exists",
    "category",
    "small",
    "group",
    "acts",
    "freely",
    "ring",
    "field",
    "module",
    "finite",
    "rank",
    "sheaf",
    "cover",
    "open",
];

const TEXTS: &[&str] = &[
    "if",
    "otherwise",
    "for all",
    "else",
    "tr",
    "and",
    "where",
    "def",
];

pub struct Tables {
    pub letters: Vec<String>,
    pub greek: Vec<&'static str>,
    pub ord: Vec<&'static str>,
    pub bin: Vec<&'static str>,
    pub rel: Vec<&'static str>,
    pub big_limits: Vec<&'static str>,
    pub big_side: Vec<&'static str>,
    pub functions: Vec<&'static str>,
    pub limit_functions: Vec<&'static str>,
    pub spaces: Vec<&'static str>,
    pub symbol_ops: Vec<&'static str>,
    pub style_ops: Vec<&'static str>,
    pub fences: Vec<&'static str>,
    pub fence_mods: Vec<&'static str>,
    pub composites: Vec<String>,
    pub stacks: Vec<(&'static str, &'static str)>,
    pub doc_styles: Vec<&'static str>,
    pub env_kinds: Vec<&'static str>,
    valid: RefCell<HashMap<String, bool>>,
}

impl Tables {
    pub fn new() -> Tables {
        let mut letters: Vec<String> = ('a'..='z')
            .chain('A'..='Z')
            .map(|c| c.to_string())
            .collect();
        letters.retain(|l| symbols::named(l).is_none());
        let by_class = |c: Class| {
            symbols::NAMES
                .iter()
                .filter(|n| n.class == c && !n.large)
                .map(|n| n.name)
                .collect::<Vec<_>>()
        };
        let big = |l: Limits| {
            symbols::NAMES
                .iter()
                .filter(|n| n.large && n.limits == l)
                .map(|n| n.name)
                .collect::<Vec<_>>()
        };
        Tables {
            letters,
            greek: symbols::GREEK.to_vec(),
            ord: by_class(Class::Ord),
            bin: by_class(Class::Bin),
            rel: by_class(Class::Rel),
            big_limits: big(Limits::Display),
            big_side: big(Limits::Never),
            functions: symbols::FUNCTIONS
                .iter()
                .filter(|(_, l)| *l == Limits::Never)
                .map(|(f, _)| *f)
                .collect(),
            limit_functions: symbols::FUNCTIONS
                .iter()
                .filter(|(_, l)| *l == Limits::Display)
                .map(|(f, _)| *f)
                .collect(),
            spaces: symbols::SPACES.iter().map(|(n, _)| *n).collect(),
            symbol_ops: symbols::OPS
                .iter()
                .filter(|o| !matches!(o.kind, OpKind::Fence | OpKind::Style))
                .map(|o| o.name)
                .collect(),
            style_ops: symbols::OPS
                .iter()
                .filter(|o| o.kind == OpKind::Style)
                .map(|o| o.name)
                .collect(),
            fences: symbols::FENCES.iter().map(|f| f.name).collect(),
            fence_mods: symbols::FENCE_MODIFIERS.to_vec(),
            composites: symbols::COMPOSITES
                .iter()
                .map(|(b, o, _)| format!("{b}.{o}"))
                .collect(),
            stacks: symbols::STACKS.iter().map(|(a, b, _)| (*a, *b)).collect(),
            doc_styles: doc::DOC_STYLES.to_vec(),
            env_kinds: doc::ENV_KINDS.to_vec(),
            valid: RefCell::new(HashMap::new()),
        }
    }

    pub fn is_valid(&self, src: &str) -> bool {
        if let Some(v) = self.valid.borrow().get(src) {
            return *v;
        }
        let v = parses_clean(src, &MathEnv::new());
        self.valid.borrow_mut().insert(src.to_string(), v);
        v
    }
}

impl Default for Tables {
    fn default() -> Self {
        Tables::new()
    }
}

pub fn parses_clean(src: &str, env: &MathEnv) -> bool {
    let mut diags = Diagnostics::default();
    let pos = |_: usize| (1, 1);
    let mut ctx = Ctx {
        env,
        diags: &mut diags,
        pos: &pos,
        display: false,
    };
    let _ = math::parse(src, &mut ctx);
    !diags.items.iter().any(|d| d.severity == Severity::Error)
}

pub struct Gen<'a> {
    pub t: &'a Tables,
    pub r: Rng,
    pub syms: Vec<String>,
    pub vectors: Vec<String>,
    pub labels: Vec<String>,
    pub in_table: bool,
}

impl<'a> Gen<'a> {
    pub fn new(t: &'a Tables, r: Rng) -> Gen<'a> {
        Gen {
            t,
            r,
            syms: Vec::new(),
            vectors: Vec::new(),
            labels: Vec::new(),
            in_table: false,
        }
    }

    fn letter(&mut self) -> String {
        self.r.pick(&self.t.letters).clone()
    }

    fn number(&mut self) -> String {
        match self.r.below(4) {
            0 => self.r.range(0, 9).to_string(),
            1 => self.r.range(10, 999).to_string(),
            2 => format!("{}.{}", self.r.range(0, 99), self.r.range(0, 99)),
            _ => self.r.range(1, 4).to_string(),
        }
    }

    pub fn base_symbol(&mut self) -> String {
        match self.r.below(10) {
            0..=3 => self.letter(),
            4..=5 => self.r.pick(&self.t.greek).to_string(),
            6 => self.r.pick(&self.t.ord).to_string(),
            7 => self.r.pick(&self.t.rel).to_string(),
            8 => self.r.pick(&self.t.bin).to_string(),
            _ => self.r.pick(&self.t.big_limits).to_string(),
        }
    }

    pub fn recipe(&mut self) -> String {
        if self.r.chance(0.08) {
            return self.r.pick(&self.t.composites).clone();
        }
        let base = self.base_symbol();
        if self.vectors.contains(&base) || self.syms.contains(&base) {
            return base;
        }
        let mut cur = base.clone();
        let mut ops = 0;
        let want = match self.r.below(20) {
            0..=9 => 1,
            10..=16 => 2,
            _ => 3,
        };
        if self.r.chance(0.3) {
            let s = self.r.pick(&self.t.style_ops);
            let cand = format!("{cur}.{s}");
            if self.t.is_valid(&cand) {
                cur = cand;
                ops += 1;
            }
        }
        let mut tries = 0;
        while ops < want && tries < 8 {
            tries += 1;
            let op = self.r.pick(&self.t.symbol_ops);
            let cand = format!("{cur}.{op}");
            if self.t.is_valid(&cand) {
                cur = cand;
                ops += 1;
            }
        }
        cur
    }

    fn leaf(&mut self) -> String {
        match self.r.below(20) {
            0..=6 => self.letter(),
            7..=8 => self.number(),
            9..=11 => self.r.pick(&self.t.greek).to_string(),
            12 => self.r.pick(&self.t.ord).to_string(),
            13..=15 => self.recipe(),
            16 if !self.syms.is_empty() => self.r.pick(&self.syms).clone(),
            17 if !self.vectors.is_empty() => self.r.pick(&self.vectors).clone(),
            18 => format!("\"{}\"", self.r.pick(TEXTS)),
            _ => format!("{}'", self.letter()),
        }
    }

    fn binop(&mut self) -> String {
        match self.r.below(10) {
            0..=2 => self.r.pick(&["+", "-", "=", "<", ">"]).to_string(),
            3..=5 => self.r.pick(&self.t.bin).to_string(),
            6..=7 => self.r.pick(&self.t.rel).to_string(),
            8 => {
                let a = *self
                    .r
                    .pick(&["to", "implies", "mapsto", "=", "epi", "mono", "sim", "cong"]);
                let k = *self.r.pick(&["^", "_"]);
                format!("{a}{k}{}", self.arg(1))
            }
            _ => self.recipe_of(Class::Rel),
        }
    }

    fn recipe_of(&mut self, class: Class) -> String {
        let pool = if class == Class::Rel {
            &self.t.rel
        } else {
            &self.t.bin
        };
        let base = self.r.pick(pool).to_string();
        let op = self.r.pick(&self.t.symbol_ops);
        let cand = format!("{base}.{op}");
        if self.t.is_valid(&cand) {
            cand
        } else {
            base
        }
    }

    fn arg(&mut self, d: usize) -> String {
        if d == 0 || self.r.chance(0.45) {
            let l = self.leaf();
            if l.contains(' ') || l.contains('"') || l.contains('\'') {
                format!("({l})")
            } else {
                l
            }
        } else {
            format!("({})", self.expr(d - 1))
        }
    }

    pub fn expr(&mut self, d: usize) -> String {
        let n = if d == 0 {
            self.r.range(1, 2)
        } else {
            self.r.range(1, 4)
        };
        let mut out = self.term(d);
        for _ in 1..n {
            if self.r.chance(0.25) {
                out.push(' ');
                out.push_str(&self.term(d));
            } else {
                let op = self.binop();
                let t = self.term(d);
                out = format!("{out} {op} {t}");
            }
        }
        out
    }

    fn cells(&mut self, d: usize, rows: usize, cols: usize) -> String {
        let mut rs = Vec::new();
        for _ in 0..rows {
            let cs: Vec<String> = (0..cols).map(|_| self.expr(d.saturating_sub(1))).collect();
            rs.push(cs.join(", "));
        }
        rs.join("; ")
    }

    fn fence_spec(&mut self) -> String {
        let f = self.r.pick(&self.t.fences).to_string();
        if self.r.chance(0.25) {
            let m = self.r.pick(&self.t.fence_mods);
            format!("{f}.{m}")
        } else {
            f
        }
    }

    pub fn term(&mut self, d: usize) -> String {
        if d == 0 {
            return self.leaf();
        }
        let e = d - 1;
        match self.r.below(30) {
            0..=7 => self.leaf(),
            8..=10 => {
                let b = self.leaf();
                let b = if b.contains(' ') { format!("({b})") } else { b };
                if b.ends_with('\'') {
                    return match self.r.below(3) {
                        0 => format!("{b}_{}", self.arg(e)),
                        1 => format!("{b}^{}", self.arg(e)),
                        _ => format!("{b}_{}^{}", self.arg(e), self.arg(e)),
                    };
                }
                match self.r.below(3) {
                    0 => format!("{b}_{}", self.arg(e)),
                    1 => format!("{b}^{}", self.arg(e)),
                    _ => format!("{b}_{}^{}", self.arg(e), self.arg(e)),
                }
            }
            11 => format!("frac({}, {})", self.expr(e), self.expr(e)),
            12 => format!("sqrt({})", self.expr(e)),
            13 => {
                let (o, c) = *self.r.pick(&[("(", ")"), ("[", "]"), ("{", "}"), ("[", ")"), ("(", "]")]);
                format!("{o}{}{c}", self.expr(e))
            }
            14 => format!("({}).{}", self.expr(e), self.fence_spec()),
            15 => {
                let op = self.r.pick(&self.t.big_limits).to_string();
                let i = self.letter();
                format!(
                    "{op}_({i} = {})^{} {}",
                    self.number(),
                    self.arg(e),
                    self.term(e)
                )
            }
            16 => {
                let op = self.r.pick(&self.t.big_side).to_string();
                let x = self.letter();
                format!(
                    "{op}_{}^{} {} d {x}",
                    self.arg(e),
                    self.arg(e),
                    self.term(e)
                )
            }
            17 => {
                let f = self.r.pick(&self.t.limit_functions).to_string();
                format!(
                    "{f}_({} to {}) {}",
                    self.letter(),
                    self.r.pick(&["infty", "0", "a"]),
                    self.term(e)
                )
            }
            18 => {
                let f = self.r.pick(&self.t.functions).to_string();
                format!("{f} {}", self.arg(e))
            }
            19 => {
                let k = *self.r.pick(&["abs", "norm", "floor", "ceil"]);
                format!("{k}({})", self.expr(e))
            }
            20 => format!("inner({}, {})", self.expr(e), self.expr(e)),
            21 => format!("binom({}, {})", self.expr(e), self.expr(e)),
            22 => {
                let n = self.r.range(2, 4);
                let cs: Vec<String> = (0..n).map(|_| self.expr(e.saturating_sub(1))).collect();
                format!("vec({})", cs.join(", "))
            }
            23 => {
                let (r, c) = (self.r.range(1, 4), self.r.range(1, 4));
                let k = *self.r.pick(&["mat", "grid"]);
                format!("{k}({})", self.cells(e, r, c))
            }
            24 => {
                let n = self.r.range(1, 3);
                let rs: Vec<String> = (0..n)
                    .map(|_| {
                        format!(
                            "{}, {}",
                            self.expr(e.saturating_sub(1)),
                            self.expr(e.saturating_sub(1))
                        )
                    })
                    .collect();
                format!("cases({})", rs.join("; "))
            }
            25 => {
                let (a, b) = (self.base_symbol(), self.base_symbol());
                let call = *self.r.pick(&["join", "stack"]);
                let cand = format!("{call}({a}, {b})");
                if self.t.is_valid(&cand) {
                    cand
                } else if !self.t.stacks.is_empty() {
                    let (a, b) = *self.r.pick(&self.t.stacks);
                    format!("stack({a}, {b})")
                } else {
                    self.leaf()
                }
            }
            26 => {
                let g = self.recipe();
                let cand = if self.r.chance(0.5) {
                    format!("wrap({g}, {})", self.expr(e))
                } else {
                    format!("wrap({g}, {}, {})", self.recipe(), self.expr(e))
                };
                if self.t.is_valid(&cand) {
                    cand
                } else {
                    format!(
                        "wrap({}, {})",
                        self.r.pick(&self.t.letters).clone() + ".script",
                        self.expr(e)
                    )
                }
            }
            27 if !self.in_table && self.r.chance(0.5) => self.diagram(e),
            27 => format!("{} {}", self.term(e), self.r.pick(&self.t.spaces)),
            28 => {
                let b = format!("({})", self.expr(e));
                let ops = ["hat", "bar", "tilde", "box", "circle", "arrow"];
                let op = self.r.pick(&ops);
                let cand = format!("{b}.{op}");
                if self.t.is_valid(&cand) {
                    cand
                } else {
                    b
                }
            }
            _ => format!("{} {}", self.term(e), self.term(e)),
        }
    }

    pub fn diagram(&mut self, d: usize) -> String {
        let (rows, cols) = (self.r.range(1, 3), self.r.range(2, 3));
        let mut cells = Vec::new();
        for _ in 0..rows {
            let row: Vec<String> = (0..cols)
                .map(|_| {
                    if self.r.chance(0.1) {
                        String::new()
                    } else {
                        self.expr(d.min(1))
                    }
                })
                .collect();
            cells.push(row);
        }
        if cells[0][0].is_empty() {
            cells[0][0] = self.letter();
        }
        let mut arrows = Vec::new();
        for _ in 0..self.r.range(1, rows * cols + 1) {
            let (a, b) = (
                (self.r.below(rows), self.r.below(cols)),
                (self.r.below(rows), self.r.below(cols)),
            );
            if (a == b && self.r.chance(0.6)) || cells[a.0][a.1].is_empty() || cells[b.0][b.1].is_empty() {
                continue;
            }
            let pos = |p: (usize, usize)| format!("{}.{}", p.0 + 1, p.1 + 1);
            let arrow = *self.r.pick(&[
                "to", "mapsto", "epi", "mono", "implies", "=", "cong", "sim", "incl",
            ]);
            let lab = match self.r.below(4) {
                0 => format!("{}: ", self.leaf_plain()),
                1 => String::new(),
                _ => format!("{}: ", self.letter()),
            };
            let tail = match self.r.below(4) {
                0 => format!("^{}", self.letter()),
                1 => format!("_{}", self.letter()),
                _ => String::new(),
            };
            let ops = match (a == b, self.r.below(6)) {
                (true, 0) => ".rot",
                (true, 1) => ".turn",
                (false, 0) => ".bend",
                (false, 1) => ".bend.flipv",
                (false, 2) if self.r.chance(0.3) => ".bend.bend",
                _ => "",
            };
            arrows.push(format!("{lab}{} {arrow}{ops}{tail} {}", pos(a), pos(b)));
        }
        let body: Vec<String> = cells.iter().map(|r| r.join(", ")).collect();
        if arrows.is_empty() {
            format!("diagram({})", body.join("; "))
        } else {
            format!("diagram({} | {})", body.join("; "), arrows.join(", "))
        }
    }

    fn leaf_plain(&mut self) -> String {
        match self.r.below(3) {
            0 => self.r.pick(&self.t.greek).to_string(),
            1 => format!("{}_{}", self.letter(), self.r.range(1, 3)),
            _ => self.letter(),
        }
    }

    pub fn display(&mut self, depth: usize) -> String {
        let rows = if self.r.chance(0.6) {
            1
        } else {
            self.r.range(2, 5)
        };
        let mut lines = Vec::new();
        for i in 0..rows {
            let rel = self.r.pick(&self.t.rel).to_string();
            let rel = if self.r.chance(0.5) {
                "=".to_string()
            } else {
                rel
            };
            if i > 0 && self.r.chance(0.5) {
                lines.push(format!("    {rel} {}", self.expr(depth)));
            } else {
                lines.push(format!("{} {rel} {}", self.expr(depth), self.expr(depth)));
            }
        }
        lines.join("\n")
    }

    pub fn formula(&mut self, depth: usize) -> String {
        self.expr(depth)
    }

    fn words(&mut self, n: usize) -> String {
        (0..n)
            .map(|_| *self.r.pick(WORDS))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn sentence(&mut self, depth: usize) -> String {
        let mut s = String::new();
        let n = self.r.range(1, 3);
        for i in 0..n {
            if i > 0 {
                s.push(' ');
            }
            let k = self.r.range(2, 8);
            s.push_str(&self.words(k));
            match self.r.below(9) {
                0..=3 => {
                    let k = self.r.range(0, depth);
                    s.push_str(&format!(" ${}$", self.expr(k)))
                }
                4 => s.push_str(&format!(" *{}*", self.words(2))),
                5 => s.push_str(&format!(" **{}**", self.words(1))),
                6 if !self.labels.is_empty() => {
                    s.push_str(&format!(" @{}", self.r.pick(&self.labels).clone()))
                }
                7 => s.push_str(&format!(".^[{} ${}$.]", self.words(4), self.expr(1))),
                _ => {}
            }
        }
        let mut c = s.chars();
        let first = c
            .next()
            .map(|f| f.to_uppercase().collect::<String>())
            .unwrap_or_default();
        format!("{first}{}.", c.as_str())
    }

    fn set_rule(&mut self) -> String {
        let fence = self.fence_spec();
        let grow = *self.r.pick(&["stretch", "magnify"]);
        match self.r.below(10) {
            9 => format!(
                "set diagram(gap: {}, inline: {}, bend: {})",
                self.r.pick(&["small", "normal", "large"]),
                self.r.pick(&["small", "full"]),
                self.r.pick(&["small", "normal", "large", "20", "60"])
            ),
            0 => format!(
                "set vec(style: {}, fence: {fence}, body: {})",
                self.r.pick(&["arrow", "bold"]),
                self.r.pick(&["column", "row"])
            ),
            1 => format!("set mat(fence: {fence}, grow: {grow})"),
            2 => format!("set cases(word: \"{}\")", self.r.pick(&["if", "when"])),
            3 => format!(
                "set {}(fence: {fence})",
                self.r
                    .pick(&["abs", "norm", "inner", "binom", "floor", "ceil"])
            ),
            4 => format!("set wrap(grow: {grow})"),
            5 => format!("set document(style: {})", self.r.pick(&self.t.doc_styles)),
            6 => "set heading(numbering: none)".into(),
            7 => format!(
                "set page(size: {}, margin: {}cm)",
                self.r.pick(&["a4", "a5", "letter"]),
                self.r.range(1, 3)
            ),
            _ => format!("set text(size: {}pt)", self.r.range(9, 12)),
        }
    }

    pub fn page(&mut self, index: u64, depth: usize) -> String {
        let mut out = format!("---\nfunctor: 1\ntitle: Random page {index}\n---\n\n");
        for _ in 0..self.r.range(0, 3) {
            out.push_str(&self.set_rule());
            out.push('\n');
        }
        if self.r.chance(0.5) {
            let v = self.letter();
            out.push_str(&format!("object vec: {v}\n"));
            self.vectors.push(v);
        }
        for k in 0..self.r.range(0, 2) {
            let name = format!("sy{}", ["a", "b", "c"][k]);
            let rec = self.recipe();
            out.push_str(&format!("sym {name} = {rec}\n"));
            self.syms.push(name);
        }
        out.push('\n');
        let blocks = self.r.range(4, 10);
        let mut eq = 0;
        let mut sec = 0;
        let mut thm = 0;
        for _ in 0..blocks {
            match self.r.below(10) {
                0 => {
                    sec += 1;
                    let l = format!("sec:s{sec}");
                    out.push_str(&format!("# {} {{#{l}}}\n\n", self.words(3)));
                    self.labels.push(l);
                }
                1..=3 => {
                    let p: Vec<String> = (0..self.r.range(1, 4))
                        .map(|_| self.sentence(depth))
                        .collect();
                    out.push_str(&p.join(" "));
                    out.push_str("\n\n");
                }
                4..=6 => {
                    let k = self.r.range(1, depth.max(1));
                    let m = self.display(k);
                    if self.r.chance(0.5) {
                        eq += 1;
                        let l = format!("eq:e{eq}");
                        out.push_str(&format!("$$\n{m}\n$$ {{#{l}}}\n\n"));
                        self.labels.push(l);
                    } else {
                        out.push_str(&format!("$$\n{m}\n$$\n\n"));
                    }
                }
                7 => {
                    thm += 1;
                    let k =
                        *self
                            .r
                            .pick(&["theorem", "lemma", "definition", "remark", "proposition"]);
                    let l = format!("thm:t{thm}");
                    out.push_str(&format!(
                        "::: {k} {{#{l} title=\"{}\"}}\n{}\n:::\n\n",
                        self.words(2),
                        self.sentence(depth)
                    ));
                    if self.r.chance(0.5) {
                        out.push_str(&format!("::: proof\n{}\n:::\n\n", self.sentence(depth)));
                    }
                    self.labels.push(l);
                }
                8 => {
                    let ordered = self.r.chance(0.5);
                    for i in 0..self.r.range(2, 4) {
                        let mark = if ordered {
                            format!("{}.", i + 1)
                        } else {
                            "-".into()
                        };
                        out.push_str(&format!("{mark} {}\n", self.sentence(depth.min(2))));
                    }
                    out.push('\n');
                }
                _ => {
                    let cols = self.r.range(2, 3);
                    let head: Vec<String> = (0..cols).map(|_| self.words(1)).collect();
                    out.push_str(&format!(
                        "| {} |\n|{}\n",
                        head.join(" | "),
                        ":--|".repeat(cols)
                    ));
                    self.in_table = true;
                    for _ in 0..self.r.range(1, 3) {
                        let cells: Vec<String> =
                            (0..cols).map(|_| format!("${}$", self.expr(1))).collect();
                        out.push_str(&format!("| {} |\n", cells.join(" | ")));
                    }
                    self.in_table = false;
                    out.push('\n');
                }
            }
        }
        out
    }
}
