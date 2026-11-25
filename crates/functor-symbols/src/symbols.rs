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
    n("ltimes", 0x22C9, Bin),
    n("rtimes", 0x22CA, Bin),
    n("sqcup", 0x2294, Bin),
    n("sqcap", 0x2293, Bin),
    n("uplus", 0x228E, Bin),
    n("wr", 0x2240, Bin),
    n("bullet", 0x2219, Bin),
    n("smash", 0x2A33, Bin),
    n("semi", 0x2A3E, Bin),
    n("par", 0x214B, Bin),
    n("prec", 0x227A, Rel),
    n("succ", 0x227B, Rel),
    n("cong", 0x2245, Rel),
    n("simeq", 0x2243, Rel),
    n("asymp", 0x224D, Rel),
    n("bowtie", 0x22C8, Rel),
    n("pitchfork", 0x22D4, Rel),
    n("indep", 0x2AEB, Rel),
    n("between", 0x226C, Rel),
    n("normal", 0x22B2, Rel),
    n("proves", 0x22A2, Rel),
    n("models", 0x22A8, Rel),
    n("forces", 0x22A9, Rel),
    n("lollipop", 0x22B8, Rel),
    n("therefore", 0x2234, Rel),
    n("because", 0x2235, Rel),
    n("tofro", 0x2194, Rel),
    n("equil", 0x21CC, Rel),
    n("mono", 0x21A3, Rel),
    n("epi", 0x21A0, Rel),
    n("incl", 0x21AA, Rel),
    n("bot", 0x22A5, Ord),
    n("lnot", 0xAC, Ord),
    n("complement", 0x2201, Ord),
    n("diamond", 0x25C7, Ord),
    n("wp", 0x2118, Ord),
    n("beth", 0x2136, Ord),
    n("gimel", 0x2137, Ord),
    n("daleth", 0x2138, Ord),
    n("digamma", 0x3DD, Ord),
    n("imath", 0x1D6A4, Ord),
    n("jmath", 0x1D6A5, Ord),
    n("vdots", 0x22EE, Ord),
    n("ddots", 0x22F1, Ord),
    n("udots", 0x22F0, Ord),
    n("degree", 0xB0, Ord),
    n("permille", 0x2030, Ord),
    n("dagger", 0x2020, Ord),
    n("ddagger", 0x2021, Ord),
    n("qed", 0x220E, Ord),
    n("corner", 0x231F, Ord),
    n("lightning", 0x21AF, Ord),
    n("plimsoll", 0x29B5, Ord),
    n("diameter", 0x2300, Ord),
    n("blackstar", 0x2605, Ord),
    n("checkmark", 0x2713, Ord),
    n("flat", 0x266D, Ord),
    n("natural", 0x266E, Ord),
    n("sharp", 0x266F, Ord),
    n("clubs", 0x2663, Ord),
];

pub const SAME_GLYPH: &[(&str, &str)] = &[("perp", "bot")];

pub const CAL_BASE: u32 = 0xF0000;

pub const DUPLICATES: &[(u32, u32, &str)] = &[
    (0x2126, 0x3A9, "Omega.up"),
    (0x212B, 0xC5, "A.up.ring"),
    (0xB5, 0x3BC, "mu.up"),
    (0x2206, 0x394, "Delta"),
    (0x212A, 0x4B, "K.up"),
    (0x2A2F, 0xD7, "times"),
    (0x2A3F, 0x2210, "coprod"),
    (0x27C2, 0x22A5, "perp"),
];

pub fn duplicate_of(cp: u32) -> Option<(u32, &'static str)> {
    DUPLICATES.iter().find(|(d, _, _)| *d == cp).map(|(_, c, n)| (*c, *n))
}

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
    ("sinh", Limits::Never),
    ("cosh", Limits::Never),
    ("tanh", Limits::Never),
    ("coth", Limits::Never),
    ("sec", Limits::Never),
    ("csc", Limits::Never),
    ("cot", Limits::Never),
    ("arcsin", Limits::Never),
    ("arccos", Limits::Never),
    ("arctan", Limits::Never),
    ("lg", Limits::Never),
    ("limsup", Limits::Display),
    ("liminf", Limits::Display),
    ("argmax", Limits::Display),
    ("argmin", Limits::Display),
    ("colim", Limits::Display),
    ("rank", Limits::Never),
    ("tr", Limits::Never),
    ("Tr", Limits::Never),
    ("sgn", Limits::Never),
    ("lcm", Limits::Display),
    ("span", Limits::Never),
    ("diag", Limits::Never),
    ("id", Limits::Never),
    ("im", Limits::Never),
    ("coker", Limits::Never),
    ("Aut", Limits::Never),
    ("End", Limits::Never),
    ("Hom", Limits::Never),
    ("Ext", Limits::Never),
    ("Tor", Limits::Never),
    ("Spec", Limits::Never),
    ("Gal", Limits::Never),
    ("Re", Limits::Never),
    ("Im", Limits::Never),
    ("supp", Limits::Never),
    ("vol", Limits::Never),
    ("erf", Limits::Never),
    ("res", Limits::Never),
    ("Pr", Limits::Display),
    ("Var", Limits::Never),
    ("Cov", Limits::Never),
    ("Corr", Limits::Never),
    ("grad", Limits::Never),
    ("curl", Limits::Never),
    ("mod", Limits::Never),
];

pub const SPACES: &[(&str, f64)] = &[("neg", -83.0), ("thin", 167.0), ("med", 222.0), ("thick", 278.0), ("quad", 1000.0), ("qquad", 2000.0)];

pub fn space(name: &str) -> Option<f64> {
    SPACES.iter().find(|(n, _)| *n == name).map(|(_, w)| *w)
}

pub const CALLS: &[&str] = &["frac", "sqrt", "vec", "text", "join", "grid", "mat", "cases", "binom", "abs", "norm", "inner", "floor", "ceil", "wrap", "stack", "diagram"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpKind {
    Decorate,
    Enclose,
    Strike,
    Repeat,
    Reflect,
    Style,
    Magnify,
    Fence,
    Deform,
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
    op("ring", OpKind::Decorate, 0x2DA, "decorate: a ring above, such as the ring in Å or ≗"),
    op("harpoon", OpKind::Decorate, 0x20D1, "decorate: a half arrow head; on an arrow it makes a harpoon"),
    op("circle", OpKind::Enclose, 0, "enclose: a closed or restricted version"),
    op("box", OpKind::Enclose, 0, "enclose: a restricted or boxed version"),
    op("square", OpKind::Enclose, 0, "enclose: a boxed version with equal sides"),
    op("not", OpKind::Strike, 0, "strike: negation"),
    op("cross", OpKind::Strike, 0, "strike: a fixed variant"),
    op("x2", OpKind::Repeat, 2, "repeat: second order"),
    op("x3", OpKind::Repeat, 3, "repeat: third order"),
    op("x4", OpKind::Repeat, 4, "repeat: fourth order"),
    op("flip", OpKind::Reflect, 0, "reflect left to right: a dual or opposite"),
    op("flipv", OpKind::Reflect, 0, "reflect top to bottom: a dual or opposite"),
    op("turn", OpKind::Reflect, 0, "turn half a circle: a dual or opposite"),
    op("rot", OpKind::Reflect, 0, "turn a quarter circle to the left: the same thing in a new direction, such as an arrow up"),
    op("bold", OpKind::Style, 0, "style: a new kind of object, often a vector"),
    op("bb", OpKind::Style, 0, "style: a number set"),
    op("frak", OpKind::Style, 0, "style: an ideal, an algebra or a structure"),
    op("script", OpKind::Style, 0, "style: a family, a category or a transform"),
    op("up", OpKind::Style, 0, "style: a constant, a unit or a name"),
    op("cal", OpKind::Style, 0, "style: calligraphic capitals, a second family next to script"),
    op("alt", OpKind::Style, 0, "style: the other common form of the same symbol, such as ϕ for φ"),
    op("small", OpKind::Magnify, 80, "magnify: a smaller version, about four fifths"),
    op("big", OpKind::Magnify, 140, "magnify: the n-ary version, a large operator with limits"),
    op("tall", OpKind::Deform, 150, "deform: a taller version; the straight parts grow and the ends keep their shape"),
    op("wide", OpKind::Deform, 160, "deform: a wider version, such as a long arrow; the ends keep their shape"),
    op("paren", OpKind::Fence, 0, "fence: round brackets, a group or a tuple"),
    op("brack", OpKind::Fence, 0, "fence: square brackets, a group or a matrix"),
    op("brace", OpKind::Fence, 0, "fence: curly brackets, a set"),
    op("angle", OpKind::Fence, 0, "fence: angle brackets, a pairing or an average"),
    op("vbar", OpKind::Fence, 0, "fence: bars, a size or a determinant"),
    op("floor", OpKind::Fence, 0, "fence: round down"),
    op("ceil", OpKind::Fence, 0, "fence: round up"),
];

pub struct FenceDef {
    pub name: &'static str,
    pub open: u32,
    pub close: u32,
    pub double: Option<(u32, u32)>,
}

const fn fence(name: &'static str, open: u32, close: u32, double: Option<(u32, u32)>) -> FenceDef {
    FenceDef { name, open, close, double }
}

pub const FENCES: &[FenceDef] = &[
    fence("paren", 0x28, 0x29, None),
    fence("brack", 0x5B, 0x5D, Some((0x27E6, 0x27E7))),
    fence("brace", 0x7B, 0x7D, None),
    fence("angle", 0x27E8, 0x27E9, None),
    fence("vbar", 0x7C, 0x7C, Some((0x2225, 0x2225))),
    fence("floor", 0x230A, 0x230B, None),
    fence("ceil", 0x2308, 0x2309, None),
];

pub const FENCE_MODIFIERS: &[&str] = &["x2", "left", "right"];

pub fn fence_def(name: &str) -> Option<&'static FenceDef> {
    FENCES.iter().find(|f| f.name == name)
}

pub const KIND_DEFAULTS: &[(&str, &str, &str)] = &[
    ("vec", "fence", "paren"),
    ("vec", "body", "column"),
    ("mat", "fence", "paren"),
    ("mat", "inline", "small"),
    ("vec", "inline", "small"),
    ("diagram", "inline", "small"),
    ("diagram", "gap", "normal"),
    ("cases", "fence", "brace.left"),
    ("cases", "word", ""),
    ("binom", "fence", "paren"),
    ("abs", "fence", "vbar"),
    ("norm", "fence", "vbar.x2"),
    ("inner", "fence", "angle"),
    ("floor", "fence", "floor"),
    ("ceil", "fence", "ceil"),
    ("wrap", "grow", "stretch"),
    ("lint", "notation", "on"),
];

pub const SET_OPTIONS: &[(&str, &[&str])] = &[
    ("vec", &["style", "fence", "body", "grow", "inline"]),
    ("mat", &["fence", "grow", "inline"]),
    ("cases", &["fence", "word", "grow"]),
    ("binom", &["fence", "grow"]),
    ("abs", &["fence", "grow"]),
    ("norm", &["fence", "grow"]),
    ("inner", &["fence", "grow"]),
    ("floor", &["fence", "grow"]),
    ("ceil", &["fence", "grow"]),
    ("wrap", &["grow"]),
    ("diagram", &["gap", "inline"]),
    ("document", &["style", "accent"]),
    ("lint", &["notation"]),
    ("heading", &["numbering"]),
    ("page", &["size", "margin"]),
    ("text", &["size", "font"]),
];

pub const LATER_OPS: &[(&str, &str)] = &[
    ("deform", "Deform has two operations: `tall` and `wide`. For example `integral.tall` or `to.wide`."),
    ("stretch", "Write `tall` or `wide`. They stretch the straight parts and keep the ends."),
    ("fuse", "`fuse` comes in a later phase. Use `join(a, b)` for now."),
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
    ("integral", "x4", 0x2A0C),
    ("integral", "cross", 0x2A0D),
    ("div", "circle", 0x2298),
    ("ast", "circle", 0x229B),
    ("circ", "circle", 0x229A),
    ("eq", "circle", 0x229C),
    ("plus", "dot", 0x2214),
    ("minus", "dot", 0x2238),
    ("vee", "under", 0x22BB),
    ("wedge", "bar", 0x22BC),
    ("vee", "bar", 0x22BD),
    ("lt", "x2", 0x226A),
    ("gt", "x2", 0x226B),
    ("lt", "x3", 0x22D8),
    ("gt", "x3", 0x22D9),
    ("subset", "x2", 0x22D0),
    ("supset", "x2", 0x22D1),
    ("cup", "x2", 0x22D3),
    ("cap", "x2", 0x22D2),
    ("subset", "square", 0x228F),
    ("supset", "square", 0x2290),
    ("subseteq", "square", 0x2291),
    ("supseteq", "square", 0x2292),
    ("eq", "dot", 0x2250),
    ("eq", "ring", 0x2257),
    ("prec", "not", 0x2280),
    ("succ", "not", 0x2281),
    ("cong", "not", 0x2247),
    ("simeq", "not", 0x2244),
    ("proves", "not", 0x22AC),
    ("models", "not", 0x22AD),
    ("bot", "flipv", 0x22A4),
    ("proves", "flip", 0x22A3),
    ("models", "flip", 0x2AE4),
    ("corner", "flip", 0x231E),
    ("to", "not", 0x219B),
    ("to", "flip.not", 0x219A),
    ("implies", "not", 0x21CF),
    ("iff", "not", 0x21CE),
    ("tofro", "not", 0x21AE),
    ("to", "rot", 0x2191),
    ("to", "flip.rot", 0x2193),
    ("implies", "rot", 0x21D1),
    ("implies", "flip.rot", 0x21D3),
    ("tofro", "rot", 0x2195),
    ("iff", "rot", 0x21D5),
    ("to", "harpoon", 0x21C0),
    ("to", "flip.harpoon", 0x21BC),
    ("implies", "x3", 0x21DB),
    ("mapsto", "flip", 0x21A4),
    ("to", "wide", 0x27F6),
    ("to", "flip.wide", 0x27F5),
    ("tofro", "wide", 0x27F7),
    ("implies", "wide", 0x27F9),
    ("implies", "flip.wide", 0x27F8),
    ("iff", "wide", 0x27FA),
    ("mapsto", "wide", 0x27FC),
    ("prime", "x2", 0x2033),
    ("prime", "x3", 0x2034),
    ("prime", "x4", 0x2057),
    ("F", "turn", 0x2132),
    ("G", "turn", 0x2141),
    ("L", "turn", 0x2142),
    ("Y", "turn", 0x2144),
    ("Omega", "turn", 0x2127),
    ("lambda", "cross", 0x19B),
    ("in", "small", 0x220A),
    ("ni", "small", 0x220D),
    ("le", "alt", 0x2A7D),
    ("ge", "alt", 0x2A7E),
    ("wedge", "big", 0x22C0),
    ("vee", "big", 0x22C1),
    ("cup", "big", 0x22C3),
    ("cap", "big", 0x22C2),
    ("plus", "circle.big", 0x2A01),
    ("times", "circle.big", 0x2A02),
    ("cdot", "circle.big", 0x2A00),
    ("uplus", "big", 0x2A04),
    ("sqcup", "big", 0x2A06),
    ("sqcap", "big", 0x2A05),
    ("times", "big", 0x2A09),
];

pub const STACKS: &[(&str, &str, u32)] = &[
    ("lt", "eq", 0x2266),
    ("gt", "eq", 0x2267),
    ("lt", "sim", 0x2272),
    ("gt", "sim", 0x2273),
    ("lt", "gt", 0x2276),
    ("gt", "lt", 0x2277),
    ("prec", "eq", 0x2AAF),
    ("succ", "eq", 0x2AB0),
    ("prec", "sim", 0x227E),
    ("succ", "sim", 0x227F),
    ("approx", "eq", 0x224A),
    ("triangle", "eq", 0x225C),
    ("subset", "eq.not", 0x228A),
    ("supset", "eq.not", 0x228B),
    ("to", "to", 0x21C9),
    ("to.flip", "to.flip", 0x21C7),
    ("to", "to.flip", 0x21C4),
    ("to.flip", "to", 0x21C6),
];

pub fn stack_glyph(top: &str, bottom: &str) -> Option<u32> {
    STACKS.iter().find(|(a, b, _)| *a == top && *b == bottom).map(|(_, _, cp)| *cp)
}

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
        'a'..='z' => Some(0x1D552 + (c as u32 - 'a' as u32)),
        '0'..='9' => Some(0x1D7D8 + (c as u32 - '0' as u32)),
        _ => None,
    }
}

pub fn fraktur_letter(c: char) -> Option<u32> {
    match c {
        'C' => Some(0x212D),
        'H' => Some(0x210C),
        'I' => Some(0x2111),
        'R' => Some(0x211C),
        'Z' => Some(0x2128),
        'A'..='Z' => Some(0x1D504 + (c as u32 - 'A' as u32)),
        'a'..='z' => Some(0x1D51E + (c as u32 - 'a' as u32)),
        _ => None,
    }
}

pub fn script_letter(c: char) -> Option<u32> {
    match c {
        'B' => Some(0x212C),
        'E' => Some(0x2130),
        'F' => Some(0x2131),
        'H' => Some(0x210B),
        'I' => Some(0x2110),
        'L' => Some(0x2112),
        'M' => Some(0x2133),
        'R' => Some(0x211B),
        'e' => Some(0x212F),
        'g' => Some(0x210A),
        'o' => Some(0x2134),
        'A'..='Z' => Some(0x1D49C + (c as u32 - 'A' as u32)),
        'a'..='z' => Some(0x1D4B6 + (c as u32 - 'a' as u32)),
        _ => None,
    }
}

pub fn styled_letter(style: &str, c: char) -> Option<u32> {
    match style {
        "bold" => bold_letter(c),
        "bb" => double_struck(c),
        "frak" => fraktur_letter(c),
        "script" => script_letter(c),
        "up" => c.is_ascii_alphabetic().then_some(c as u32),
        "cal" => c.is_ascii_uppercase().then_some(CAL_BASE + c as u32),
        _ => None,
    }
}

pub fn char_class(c: char) -> Class {
    match c {
        '+' | '\u{2212}' | '*' | '\u{B1}' | '\u{D7}' | '\u{22C5}' => return Bin,
        '=' | '<' | '>' | ':' | '\u{2264}' | '\u{2265}' | '\u{2208}' | '\u{2192}' => return Rel,
        '(' | '[' | '{' => return Open,
        ')' | ']' | '}' => return Close,
        ',' | ';' => return Punct,
        _ => {}
    }
    let cp = c as u32;
    if let Some(n) = NAMES.iter().find(|n| n.cp == cp) {
        return if n.large { Op } else { n.class };
    }
    unicode_class(cp).unwrap_or(Ord)
}

const CLASS_DATA: &str = include_str!("../data/classes.tsv");

pub fn unicode_class(cp: u32) -> Option<Class> {
    static TABLE: std::sync::OnceLock<std::collections::HashMap<u32, Class>> = std::sync::OnceLock::new();
    let t = TABLE.get_or_init(|| {
        CLASS_DATA
            .lines()
            .filter_map(|l| {
                let (cp, c) = l.split_once('\t')?;
                let class = match c {
                    "B" => Bin,
                    "R" => Rel,
                    "O" => Op,
                    "L" => Open,
                    "C" => Close,
                    _ => return None,
                };
                Some((u32::from_str_radix(cp, 16).ok()?, class))
            })
            .collect()
    });
    t.get(&cp).copied()
}

pub fn all_names() -> Vec<&'static str> {
    let mut v: Vec<&str> = NAMES.iter().map(|s| s.name).collect();
    v.extend(GREEK.iter().copied());
    v.extend(FUNCTIONS.iter().map(|(f, _)| *f));
    v.extend(CALLS.iter().copied());
    v.extend(SPACES.iter().map(|(n, _)| *n));
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glyph::Glyph;

    #[test]
    fn every_named_symbol_has_a_glyph() {
        let missing: Vec<&str> = NAMES.iter().filter(|n| Glyph::from_font(n.cp, false).is_none()).map(|n| n.name).collect();
        assert!(missing.is_empty(), "no glyph for {missing:?}");
    }

    #[test]
    fn every_recipe_glyph_is_in_the_font() {
        let missing: Vec<String> = COMPOSITES
            .iter()
            .map(|(b, o, cp)| (format!("{b}.{o}"), *cp))
            .chain(STACKS.iter().map(|(a, b, cp)| (format!("stack({a}, {b})"), *cp)))
            .filter(|(_, cp)| Glyph::from_font(*cp, false).is_none())
            .map(|(r, _)| r)
            .collect();
        assert!(missing.is_empty(), "no glyph for {missing:?}");
    }

    #[test]
    fn no_name_is_in_two_tables() {
        let mut seen: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
        let tables: [(&str, Vec<&str>); 5] = [
            ("names", NAMES.iter().map(|n| n.name).collect()),
            ("greek", GREEK.to_vec()),
            ("functions", FUNCTIONS.iter().map(|(f, _)| *f).collect()),
            ("spaces", SPACES.iter().map(|(n, _)| *n).collect()),
            ("calls", CALLS.to_vec()),
        ];
        for (table, names) in tables.iter() {
            for n in names {
                if let Some(other) = seen.insert(n, table) {
                    panic!("`{n}` is in {other} and in {table}");
                }
            }
        }
    }

    #[test]
    fn no_two_names_share_a_glyph() {
        let mut seen: std::collections::HashMap<u32, &str> = std::collections::HashMap::new();
        for n in NAMES {
            if let Some(other) = seen.insert(n.cp, n.name) {
                let allowed = SAME_GLYPH.iter().any(|(a, b)| (*a == other && *b == n.name) || (*b == other && *a == n.name));
                assert!(allowed, "`{other}` and `{}` are the same glyph", n.name);
            }
        }
    }

    #[test]
    fn no_two_recipes_share_a_glyph() {
        let mut seen: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
        let recipes = COMPOSITES.iter().map(|(b, o, cp)| (format!("{b}.{o}"), *cp)).chain(STACKS.iter().map(|(a, b, cp)| (format!("stack({a}, {b})"), *cp)));
        for (r, cp) in recipes {
            if let Some(other) = seen.insert(cp, r.clone()) {
                panic!("`{other}` and `{r}` make the same glyph");
            }
        }
    }

    #[test]
    fn duplicates_point_to_a_glyph_functor_has() {
        for (dup, canon, _) in DUPLICATES {
            assert!(Glyph::from_font(*canon, false).is_some(), "{dup:x}");
        }
    }

    #[test]
    fn typed_unicode_gets_a_math_class() {
        assert_eq!(char_class('\u{2297}'), Bin);
        assert_eq!(char_class('\u{226A}'), Rel);
        assert_eq!(char_class('\u{27E8}'), Open);
        assert_eq!(char_class('\u{221E}'), Ord);
    }

    #[test]
    fn greek_styles() {
        assert_eq!(styled_greek("alt", greek_index("phi").unwrap()), Some(0x1D719));
        assert_eq!(styled_greek("up", greek_index("mu").unwrap()), Some(0x3BC));
        assert_eq!(styled_greek("alt", greek_index("alpha").unwrap()), None);
        assert_eq!(function_text("limsup"), "lim sup");
    }

    #[test]
    fn dots_is_built_from_three_periods() {
        let dots = Glyph::from_font(0x2026, false).unwrap();
        let period = Glyph::from_font(0x2E, false).unwrap();
        assert!((dots.w - (3.0 * period.w + 2.0 * 1000.0 * 3.0 / 18.0)).abs() < 1e-6);
    }
}

pub fn styled_greek(style: &str, index: usize) -> Option<u32> {
    let i = index as u32;
    match style {
        "up" => Some(0x3B1 + i),
        "bold" => Some(0x1D736 + i),
        "alt" => match GREEK.get(index).copied()? {
            "epsilon" => Some(0x1D716),
            "theta" => Some(0x1D717),
            "kappa" => Some(0x1D718),
            "phi" => Some(0x1D719),
            "rho" => Some(0x1D71A),
            "pi" => Some(0x1D71B),
            _ => None,
        },
        _ => None,
    }
}

pub fn greek_index(name: &str) -> Option<usize> {
    GREEK.iter().position(|g| *g == name)
}

pub fn function_text(name: &str) -> &str {
    match name {
        "limsup" => "lim sup",
        "liminf" => "lim inf",
        "argmax" => "arg max",
        "argmin" => "arg min",
        _ => name,
    }
}
