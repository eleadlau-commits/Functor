#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Ord,
    Op,
    Bin,
    Rel,
    Open,
    Close,
    Punct,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limits {
    Never,
    Display,
}

pub struct Named {
    pub name: &'static str,
    pub cp: u32,
    pub class: Class,
    pub large: bool,
    pub limits: Limits,
}

const fn n(name: &'static str, cp: u32, class: Class) -> Named {
    Named { name, cp, class, large: false, limits: Limits::Never }
}

const fn big(name: &'static str, cp: u32, limits: Limits) -> Named {
    Named { name, cp, class: Class::Op, large: true, limits }
}

use Class::*;

pub const NAMES: &[Named] = &[
    big("sum", 0x2211, Limits::Display),
    big("prod", 0x220F, Limits::Display),
    big("coprod", 0x2210, Limits::Display),
    big("integral", 0x222B, Limits::Never),
    big("union", 0x22C3, Limits::Display),
    big("inter", 0x22C2, Limits::Display),
    n("plus", 0x2B, Bin),
    n("minus", 0x2212, Bin),
    n("times", 0xD7, Bin),
    n("cdot", 0x22C5, Bin),
    n("pm", 0xB1, Bin),
    n("mp", 0x2213, Bin),
    n("div", 0xF7, Bin),
    n("circ", 0x2218, Bin),
    n("ast", 0x2217, Bin),
    n("star", 0x22C6, Bin),
    n("wedge", 0x2227, Bin),
    n("vee", 0x2228, Bin),
    n("cup", 0x222A, Bin),
    n("cap", 0x2229, Bin),
    n("setminus", 0x2216, Bin),
    n("eq", 0x3D, Rel),
    n("lt", 0x3C, Rel),
    n("gt", 0x3E, Rel),
    n("le", 0x2264, Rel),
    n("ge", 0x2265, Rel),
    n("approx", 0x2248, Rel),
    n("sim", 0x223C, Rel),
    n("equiv", 0x2261, Rel),
    n("propto", 0x221D, Rel),
    n("in", 0x2208, Rel),
    n("ni", 0x220B, Rel),
    n("subset", 0x2282, Rel),
    n("supset", 0x2283, Rel),
    n("subseteq", 0x2286, Rel),
    n("supseteq", 0x2287, Rel),
    n("perp", 0x22A5, Rel),
    n("mid", 0x2223, Rel),
    n("parallel", 0x2225, Rel),
    n("to", 0x2192, Rel),
    n("gets", 0x2190, Rel),
    n("implies", 0x21D2, Rel),
    n("iff", 0x21D4, Rel),
    n("mapsto", 0x21A6, Rel),
    n("infty", 0x221E, Ord),
    n("partial", 0x2202, Ord),
    n("nabla", 0x2207, Ord),
    n("forall", 0x2200, Ord),
    n("exists", 0x2203, Ord),
    n("emptyset", 0x2205, Ord),
    n("ell", 0x2113, Ord),
    n("aleph", 0x2135, Ord),
    n("angle", 0x2220, Ord),
    n("triangle", 0x25B3, Ord),
    n("square", 0x25A1, Ord),
    n("dots", 0x2026, Ord),
    n("cdots", 0x22EF, Ord),
    n("prime", 0x2032, Ord),
    n("Gamma", 0x393, Ord),
    n("Delta", 0x394, Ord),
    n("Theta", 0x398, Ord),
    n("Lambda", 0x39B, Ord),
    n("Xi", 0x39E, Ord),
    n("Pi", 0x3A0, Ord),
    n("Sigma", 0x3A3, Ord),
    n("Upsilon", 0x3A5, Ord),
    n("Phi", 0x3A6, Ord),
    n("Psi", 0x3A8, Ord),
    n("Omega", 0x3A9, Ord),
];

pub const GREEK: &[&str] = &[
    "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta", "iota", "kappa", "lambda", "mu", "nu", "xi",
    "omicron", "pi", "rho", "varsigma", "sigma", "tau", "upsilon", "phi", "chi", "psi", "omega",
];

pub const FUNCTIONS: &[(&str, Limits)] = &[
    ("sin", Limits::Never),
    ("cos", Limits::Never),
    ("tan", Limits::Never),
    ("log", Limits::Never),
    ("ln", Limits::Never),
    ("exp", Limits::Never),
    ("det", Limits::Display),
    ("dim", Limits::Never),
    ("ker", Limits::Never),
    ("deg", Limits::Never),
    ("gcd", Limits::Display),
    ("arg", Limits::Never),
    ("lim", Limits::Display),
    ("max", Limits::Display),
    ("min", Limits::Display),
    ("sup", Limits::Display),
    ("inf", Limits::Display),
];

pub const SPACES: &[(&str, f64)] = &[("thin", 167.0), ("med", 222.0), ("thick", 278.0), ("quad", 1000.0), ("qquad", 2000.0)];

pub fn space(name: &str) -> Option<f64> {
    SPACES.iter().find(|(n, _)| *n == name).map(|(_, w)| *w)
}

pub const CALLS: &[&str] = &["frac", "sqrt", "vec", "text"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpKind {
    Decorate,
    Enclose,
    Strike,
    Repeat,
    Reflect,
    Style,
}

pub struct OpDef {
    pub name: &'static str,
    pub kind: OpKind,
    pub mark: u32,
    pub meaning: &'static str,
}

const fn op(name: &'static str, kind: OpKind, mark: u32, meaning: &'static str) -> OpDef {
    OpDef { name, kind, mark, meaning }
}

pub const OPS: &[OpDef] = &[
    op("hat", OpKind::Decorate, 0x2C6, "decorate: a derived object, often an estimate"),
    op("tilde", OpKind::Decorate, 0x2DC, "decorate: a related object, often an approximation"),
    op("bar", OpKind::Decorate, 0xAF, "decorate: a derived object, often a mean or conjugate"),
    op("dot", OpKind::Decorate, 0x2D9, "decorate: a first derivative in time"),
    op("ddot", OpKind::Decorate, 0xA8, "decorate: a second derivative in time"),
    op("check", OpKind::Decorate, 0x2C7, "decorate: a dual or inverse object"),
    op("breve", OpKind::Decorate, 0x2D8, "decorate: a related object"),
    op("arrow", OpKind::Decorate, 0x20D7, "decorate: a vector"),
    op("under", OpKind::Decorate, 0xAF, "decorate below: a related object"),
    op("circle", OpKind::Enclose, 0, "enclose: a closed or restricted version"),
    op("box", OpKind::Enclose, 0, "enclose: a restricted or boxed version"),
    op("not", OpKind::Strike, 0, "strike: negation"),
    op("cross", OpKind::Strike, 0, "strike: a fixed variant"),
    op("x2", OpKind::Repeat, 2, "repeat: second order"),
    op("x3", OpKind::Repeat, 3, "repeat: third order"),
    op("x4", OpKind::Repeat, 4, "repeat: fourth order"),
    op("flip", OpKind::Reflect, 0, "reflect left to right: a dual or opposite"),
    op("flipv", OpKind::Reflect, 0, "reflect top to bottom: a dual or opposite"),
    op("turn", OpKind::Reflect, 0, "turn half a circle: a dual or opposite"),
    op("bold", OpKind::Style, 0, "style: a new kind of object, often a vector"),
    op("bb", OpKind::Style, 0, "style: a number set"),
];

pub const LATER_OPS: &[(&str, &str)] = &[
    ("deform", "phase 1"),
    ("fuse", "phase 1"),
    ("stretch", "phase 1"),
    ("column", "phase 1"),
];

pub const COMPOSITES: &[(&str, &str, u32)] = &[
    ("integral", "circle", 0x222E),
    ("integral", "x2", 0x222C),
    ("integral", "x3", 0x222D),
    ("integral", "x2.circle", 0x222F),
    ("integral", "x3.circle", 0x2230),
    ("plus", "circle", 0x2295),
    ("minus", "circle", 0x2296),
    ("times", "circle", 0x2297),
    ("cdot", "circle", 0x2299),
    ("plus", "box", 0x229E),
    ("minus", "box", 0x229F),
    ("times", "box", 0x22A0),
    ("cdot", "box", 0x22A1),
    ("in", "not", 0x2209),
    ("ni", "not", 0x220C),
    ("eq", "not", 0x2260),
    ("lt", "not", 0x226E),
    ("gt", "not", 0x226F),
    ("le", "not", 0x2270),
    ("ge", "not", 0x2271),
    ("sim", "not", 0x2241),
    ("approx", "not", 0x2249),
    ("equiv", "not", 0x2262),
    ("subset", "not", 0x2284),
    ("supset", "not", 0x2285),
    ("subseteq", "not", 0x2288),
    ("supseteq", "not", 0x2289),
    ("exists", "not", 0x2204),
    ("mid", "not", 0x2224),
    ("parallel", "not", 0x2226),
    ("E", "flip", 0x2203),
    ("A", "turn", 0x2200),
    ("Delta", "turn", 0x2207),
    ("h", "cross", 0x210F),
    ("to", "flip", 0x2190),
    ("implies", "flip", 0x21D0),
];

pub fn named(name: &str) -> Option<&'static Named> {
    NAMES.iter().find(|s| s.name == name)
}

pub fn greek(name: &str) -> Option<u32> {
    let i = GREEK.iter().position(|g| *g == name)? as u32;
    Some(0x1D6FC + i)
}

pub fn function(name: &str) -> Option<Limits> {
    FUNCTIONS.iter().find(|(f, _)| *f == name).map(|(_, l)| *l)
}

pub fn op_def(name: &str) -> Option<&'static OpDef> {
    OPS.iter().find(|o| o.name == name)
}

pub fn composite(base: &str, ops: &str) -> Option<u32> {
    COMPOSITES.iter().find(|(b, o, _)| *b == base && *o == ops).map(|(_, _, cp)| *cp)
}

pub fn italic_letter(c: char) -> Option<u32> {
    match c {
        'h' => Some(0x210E),
        'a'..='z' => Some(0x1D44E + (c as u32 - 'a' as u32)),
        'A'..='Z' => Some(0x1D434 + (c as u32 - 'A' as u32)),
        _ => None,
    }
}

pub fn bold_letter(c: char) -> Option<u32> {
    match c {
        'a'..='z' => Some(0x1D482 + (c as u32 - 'a' as u32)),
        'A'..='Z' => Some(0x1D468 + (c as u32 - 'A' as u32)),
        _ => None,
    }
}

pub fn double_struck(c: char) -> Option<u32> {
    match c {
        'C' => Some(0x2102),
        'H' => Some(0x210D),
        'N' => Some(0x2115),
        'P' => Some(0x2119),
        'Q' => Some(0x211A),
        'R' => Some(0x211D),
        'Z' => Some(0x2124),
        'A'..='Z' => Some(0x1D538 + (c as u32 - 'A' as u32)),
        _ => None,
    }
}

pub fn char_class(c: char) -> Class {
    match c {
        '+' | '\u{2212}' | '*' | '\u{B1}' | '\u{D7}' | '\u{22C5}' => Bin,
        '=' | '<' | '>' | ':' | '\u{2264}' | '\u{2265}' | '\u{2208}' | '\u{2192}' => Rel,
        '(' | '[' | '{' => Open,
        ')' | ']' | '}' => Close,
        ',' | ';' => Punct,
        _ => Ord,
    }
}

pub fn all_names() -> Vec<&'static str> {
    let mut v: Vec<&str> = NAMES.iter().map(|s| s.name).collect();
    v.extend(GREEK.iter().copied());
    v.extend(FUNCTIONS.iter().map(|(f, _)| *f));
    v.extend(CALLS.iter().copied());
    v.extend(SPACES.iter().map(|(n, _)| *n));
    v
}
