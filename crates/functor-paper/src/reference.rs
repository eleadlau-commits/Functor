use crate::doc::Settings;
use crate::math::MathEnv;
use crate::symbols::{self, Class, Limits, OpKind};

pub struct Page {
    pub path: String,
    pub title: String,
    pub section: &'static str,
    pub src: String,
    pub generated: bool,
}

pub const SECTIONS: &[&str] = &["Start", "Document", "Math", "Symbols", "Structures", "Help"];

pub const HAND_PAGES: &[(&str, &str, &str)] = &[
    ("index", "Functor reference", "Start"),
    ("cheatsheet", "Cheat sheet", "Start"),
    ("latex", "From LaTeX", "Start"),
    ("document/file", "File and front matter", "Document"),
    ("document/text", "Text and punctuation", "Document"),
    ("document/parts", "Headings, lists, tables, code", "Document"),
    ("document/blocks", "Blocks", "Document"),
    ("document/references", "Labels and references", "Document"),
    ("document/declarations", "object, sym, let", "Document"),
    ("document/settings", "Document settings", "Document"),
    ("math/basics", "Math basics", "Math"),
    ("math/calls", "frac, sqrt, text, join", "Math"),
    ("math/display", "Display math and chains", "Math"),
    ("symbols/recipes", "Recipes", "Symbols"),
    ("symbols/kinds", "Meaning and style", "Symbols"),
    ("structures/grids", "Grids, matrices, vectors, cases", "Structures"),
    ("structures/fences", "Fences and wrap", "Structures"),
    ("structures/diagrams", "Diagrams and labelled arrows", "Structures"),
    ("help/errors", "Common errors", "Help"),
];

pub const GEN_DIR: &str = "gen";

const FAMILY: &[(OpKind, &str, &str)] = &[
    (OpKind::Decorate, "decorate", "A mark above or below the base. The mark follows the slant of an italic letter."),
    (OpKind::Enclose, "enclose", "A shape around the base: a circle, a box or a square."),
    (OpKind::Strike, "strike", "A line through the base."),
    (OpKind::Repeat, "repeat", "Copies of the base side by side. A font glyph is used when one exists, for example two integrals."),
    (OpKind::Reflect, "reflect", "A mirror image or a half turn of the base."),
    (OpKind::Style, "style", "Another alphabet for a letter: bold, double-struck, fraktur, script, upright."),
    (OpKind::Magnify, "magnify", "A smaller or larger copy of the base."),
    (OpKind::Deform, "deform", "A changed outline of the base. The straight parts grow and the ends and thin strokes keep their shape."),
    (OpKind::Fence, "fence", "Brackets around a group or a symbol. The brackets grow with the content."),
];

const OP_EXAMPLES: &[(&str, &[&str])] = &[
    ("hat", &["x.hat", "theta.hat", "A.hat"]),
    ("tilde", &["x.tilde", "theta.tilde"]),
    ("bar", &["x.bar", "z.bar"]),
    ("dot", &["x.dot", "theta.dot"]),
    ("ddot", &["x.ddot"]),
    ("check", &["f.check"]),
    ("breve", &["u.breve"]),
    ("arrow", &["v.arrow", "F.arrow"]),
    ("under", &["x.under"]),
    ("circle", &["sum.circle", "plus.circle", "integral.circle", "nabla.circle"]),
    ("box", &["plus.box", "theta.hat.box"]),
    ("square", &["x.square", "F.bb.square"]),
    ("not", &["in.not", "eq.not", "exists.not", "sim.not"]),
    ("cross", &["h.cross", "in.cross"]),
    ("x2", &["integral.x2", "partial.x2"]),
    ("x3", &["integral.x3"]),
    ("x4", &["integral.x4"]),
    ("flip", &["E.flip", "to.flip"]),
    ("flipv", &["Pi.flipv"]),
    ("turn", &["A.turn", "Sigma.turn"]),
    ("bold", &["v.bold", "A.bold", "sum.bold"]),
    ("bb", &["R.bb", "N.bb", "C.bb"]),
    ("frak", &["g.frak", "A.frak"]),
    ("script", &["L.script", "F.script"]),
    ("up", &["d.up", "e.up"]),
    ("small", &["T.up.small", "x.small"]),
    ("tall", &["integral.tall", "x.tall"]),
    ("wide", &["to.wide", "eq.wide"]),
    ("paren", &["(a + b).paren", "(frac(1, 2)).paren"]),
    ("brack", &["(a, b).brack", "(M).brack.x2"]),
    ("brace", &["(x).brace", "(x).brace.left"]),
    ("angle", &["(a + b).angle", "(u, v).angle"]),
    ("vbar", &["(x).vbar", "(v).vbar.x2"]),
    ("floor", &["(x).floor", "(frac(n, 2)).floor"]),
    ("ceil", &["(x).ceil"]),
];

const OP_LATEX: &[(&str, &str)] = &[
    ("hat", "`\\hat{x}` becomes `x.hat`"),
    ("tilde", "`\\tilde{x}` becomes `x.tilde`"),
    ("bar", "`\\bar{x}` and `\\overline{x}` become `x.bar`"),
    ("dot", "`\\dot{x}` becomes `x.dot`"),
    ("ddot", "`\\ddot{x}` becomes `x.ddot`"),
    ("check", "`\\check{f}` becomes `f.check`"),
    ("breve", "`\\breve{u}` becomes `u.breve`"),
    ("arrow", "`\\vec{v}` becomes `v.arrow`, but prefer `object vec: v`"),
    ("under", "`\\underline{x}` becomes `x.under`"),
    ("circle", "`\\oint` becomes `integral.circle`, `\\oplus` becomes `plus.circle`"),
    ("box", "`\\boxplus` becomes `plus.box`, `\\boxed{x}` becomes `(x).box`"),
    ("square", "no LaTeX command"),
    ("not", "`\\notin` becomes `in.not`, `\\neq` becomes `eq.not`, `\\nexists` becomes `exists.not`"),
    ("cross", "`\\hbar` becomes `h.cross`"),
    ("x2", "`\\iint` becomes `integral.x2`"),
    ("x3", "`\\iiint` becomes `integral.x3`"),
    ("x4", "`\\iiiint` becomes `integral.x4`"),
    ("flip", "`\\exists` is `exists`, and also `E.flip`"),
    ("flipv", "no LaTeX command"),
    ("turn", "`\\forall` is `forall`, and also `A.turn`"),
    ("bold", "`\\mathbf{A}` and `\\boldsymbol{v}` become `A.bold` and `v.bold`"),
    ("bb", "`\\mathbb{R}` becomes `R.bb`"),
    ("frak", "`\\mathfrak{g}` becomes `g.frak`"),
    ("script", "`\\mathcal{L}` and `\\mathscr{L}` become `L.script`"),
    ("up", "`\\mathrm{d}` becomes `d.up`"),
    ("small", "no direct LaTeX command"),
    ("tall", "`\\bigl(` and the like, for a bigger glyph"),
    ("wide", "`\\longrightarrow`, `\\xrightarrow{}` without the label"),
    ("paren", "`\\left( x \\right)` becomes `(x).paren`, or just `(x)`"),
    ("brack", "`\\left[ x \\right]` becomes `(x).brack`, `\\llbracket x \\rrbracket` becomes `(x).brack.x2`"),
    ("brace", "`\\left\\{ x \\right\\}` becomes `(x).brace`, `\\left\\{ x \\right.` becomes `(x).brace.left`"),
    ("angle", "`\\langle u, v \\rangle` becomes `(u, v).angle` or `inner(u, v)`"),
    ("vbar", "`\\lvert x \\rvert` becomes `(x).vbar` or `abs(x)`, `\\lVert v \\rVert` becomes `(v).vbar.x2` or `norm(v)`"),
    ("floor", "`\\lfloor x \\rfloor` becomes `(x).floor` or `floor(x)`"),
    ("ceil", "`\\lceil x \\rceil` becomes `(x).ceil` or `ceil(x)`"),
];

const OP_RULES: &[(&str, &str)] = &[
    ("under", "The mark goes below the base."),
    ("square", "`square` is also a named symbol, the empty square. After a dot it is the operation."),
    ("bold", "On a single letter as the first operation, it uses the bold letter of the font. On any other base, it makes the strokes thicker."),
    ("bb", "Works on a single capital letter, as the first operation."),
    ("frak", "Works on a single letter, as the first operation."),
    ("script", "Works on a single letter, as the first operation."),
    ("up", "Works on a single letter, as the first operation."),
    ("small", "Scales the base to four fifths of its size."),
    ("tall", "Makes the base half again as tall. Stems and straight parts grow, the ends keep their shape."),
    ("wide", "Makes the base wider. The shaft of an arrow grows, the head keeps its shape."),
    ("x2", "Uses the font glyph when the font has one, for example the double integral."),
];

const FENCE_RULES: &str = "Works on a group, `(a + b).OP`, or on a symbol, `x.OP`. Write `x2` after it to double the fence. Write `left` or `right` after it to keep one side only. A style rule can give a kind its fence: `set mat(fence: OP)`.";

fn header(title: &str) -> String {
    format!("---\nfunctor: 1\ntitle: {title}\n---\n\nset heading(numbering: none)\n\n")
}

fn family_of(kind: OpKind) -> (&'static str, &'static str) {
    FAMILY.iter().find(|(k, _, _)| *k == kind).map(|(_, n, d)| (*n, *d)).unwrap_or(("", ""))
}

fn lookup<'a>(table: &'a [(&str, &'a str)], key: &str) -> Option<&'a str> {
    table.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

fn glyph_source(recipe: &str) -> String {
    let (base, ops) = crate::math::split_recipe(recipe);
    if base.starts_with('(') {
        return "fence around the group".into();
    }
    match symbols::composite(&base, &ops.join(".")) {
        Some(cp) => format!("font glyph U+{cp:04X}"),
        None if ops.len() == 1 && symbols::op_def(&ops[0]).is_some_and(|o| o.kind == OpKind::Style) => "font letter".into(),
        None => "built and cached".into(),
    }
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

fn meaning_text(m: &str) -> &str {
    m.split_once(": ").map_or(m, |(_, b)| b)
}

pub fn op_page(def: &symbols::OpDef) -> String {
    let (family, family_desc) = family_of(def.kind);
    let name = def.name;
    let mut s = header(name);
    s.push_str(&format!("`{name}` · {family} · {}.\n\n", meaning_text(def.meaning)));
    s.push_str("# Form\n\n");
    if def.kind == OpKind::Fence {
        s.push_str(&format!("`(a + b).{name}` on a group, `x.{name}` on a symbol.\n\n"));
    } else {
        s.push_str(&format!("`x.{name}` on a symbol, `(a + b).{name}` on a group. A recipe has one base and up to three operations.\n\n"));
    }
    s.push_str("# Examples\n\n| Recipe | Result | Glyph |\n|:--|:-:|:--|\n");
    let examples: &[&str] = OP_EXAMPLES.iter().find(|(k, _)| *k == name).map(|(_, v)| *v).unwrap_or(&[]);
    for r in examples {
        s.push_str(&format!("| `{r}` | ${r}$ | {} |\n", glyph_source(r)));
    }
    s.push_str("\n# Four layers\n\n| Layer | For this operation |\n|:--|:--|\n");
    s.push_str(&format!("| Meaning | {}. A guideline, not a rule. |\n", capital(meaning_text(def.meaning))));
    let style = match def.kind {
        OpKind::Style => "For a kind of object, prefer `object` and a `set` rule. Use the operation for fixed notation, such as `R.bb`.".to_string(),
        OpKind::Fence => format!("A kind can take this fence from a rule: `set abs(fence: {name})`."),
        _ => "Use a `sym` to give a recipe a short name.".to_string(),
    };
    s.push_str(&format!("| Style | {style} |\n"));
    s.push_str(&format!("| Construction | {family_desc} |\n"));
    s.push_str("| Glyph | The font glyph when the font has one, otherwise a built glyph kept in the recipe cache. |\n\n");
    s.push_str("# Rules\n\n");
    if def.kind == OpKind::Fence {
        s.push_str(&format!("- {}\n", FENCE_RULES.replace("OP", name)));
    } else {
        s.push_str("- More than three operations on one base gives a warning.\n");
    }
    if let Some(r) = lookup(OP_RULES, name) {
        s.push_str(&format!("- {r}\n"));
    }
    s.push_str("\n# From LaTeX\n\n");
    s.push_str(&format!("{}.\n\n", lookup(OP_LATEX, name).unwrap_or("no LaTeX command")));
    s.push_str("# See also\n\n[All operations](symbols/operations.html) · [Recipes](symbols/recipes.html)\n");
    s
}

fn operations_page() -> String {
    let mut s = header("Operations");
    s.push_str(&format!(
        "Functor has {} operations in {} families. Write an operation after a dot: `theta.hat`. Each operation has a meaning. The meaning is a guideline, not a rule.\n\n",
        symbols::OPS.len(),
        FAMILY.len()
    ));
    for (kind, family, desc) in FAMILY {
        s.push_str(&format!("# {family}\n\n{desc}\n\n| Operation | Example | Meaning |\n|:--|:-:|:--|\n"));
        for d in symbols::OPS.iter().filter(|o| o.kind == *kind) {
            let ex = OP_EXAMPLES.iter().find(|(k, _)| *k == d.name).and_then(|(_, v)| v.first()).copied().unwrap_or("x");
            s.push_str(&format!("| [`{}`](symbols/ops/{}.html) | ${ex}$ | {} |\n", d.name, d.name, meaning_text(d.meaning)));
        }
        s.push('\n');
    }
    let later: Vec<String> = symbols::LATER_OPS.iter().map(|(n, w)| format!("`{n}` ({w})")).collect();
    s.push_str(&format!("# Not yet\n\nThe compiler refuses these with a clear message: {}.\n", later.join(", ")));
    s
}

fn names_page() -> String {
    let mut s = header("Named symbols");
    s.push_str("Write a symbol by its name, with spaces between names: `x in A`. Each name is short and canonical. There are no aliases in files.\n\n");
    for (title, filter) in [
        ("Large operators", Class::Op),
        ("Binary operators", Class::Bin),
        ("Relations", Class::Rel),
        ("Ordinary symbols", Class::Ord),
    ] {
        let rows: Vec<&symbols::Named> = symbols::NAMES.iter().filter(|n| n.class == filter && !(filter == Class::Ord && n.name.starts_with(char::is_uppercase))).collect();
        if rows.is_empty() {
            continue;
        }
        s.push_str(&format!("# {title}\n\n| Name | Result | Unicode |\n|:--|:-:|:--|\n"));
        for n in rows {
            let lim = if n.large && n.limits == Limits::Display { ", limits below and above in display" } else { "" };
            s.push_str(&format!("| `{}` | ${}$ | U+{:04X}{lim} |\n", n.name, n.name, n.cp));
        }
        s.push('\n');
    }
    s.push_str("Capital Greek letters are on the [Greek letters](symbols/greek.html) page.\n");
    s
}

fn greek_page() -> String {
    let mut s = header("Greek letters");
    s.push_str("Lowercase Greek letters are italic. Capital Greek letters are upright.\n\n# Lowercase\n\n| Name | Result |\n|:--|:-:|\n");
    for g in symbols::GREEK {
        s.push_str(&format!("| `{g}` | ${g}$ |\n"));
    }
    s.push_str("\n# Capitals\n\n| Name | Result |\n|:--|:-:|\n");
    for n in symbols::NAMES.iter().filter(|n| n.class == Class::Ord && n.name.starts_with(char::is_uppercase)) {
        s.push_str(&format!("| `{}` | ${}$ |\n", n.name, n.name));
    }
    s.push_str("\nA capital Greek letter that looks like a Latin letter has no name. Write the Latin letter with `.up`: `A.up` for capital alpha.\n");
    s
}

fn functions_page() -> String {
    let mut s = header("Functions and spaces");
    s.push_str("A function name is set upright. It takes no operations.\n\n# Functions\n\n| Name | Result | Limits in display |\n|:--|:-:|:--|\n");
    for (f, l) in symbols::FUNCTIONS {
        let lim = if *l == Limits::Display { "below" } else { "at the side" };
        s.push_str(&format!("| `{f}` | ${f} x$ | {lim} |\n"));
    }
    s.push_str("\nFor another upright name, write text in quotes: `\"tr\" A`.\n\n# Spaces\n\n| Name | Width | Example |\n|:--|:--|:-:|\n");
    for (n, w) in symbols::SPACES {
        s.push_str(&format!("| `{n}` | {:.3} em | $a {n} b$ |\n", w / 1000.0));
    }
    s
}

fn composites_page() -> String {
    let mut s = header("Font composites");
    s.push_str("These recipes have a glyph in the font, so the compiler uses it. All other recipes are built from the base and the operations, and kept in a cache.\n\n| Recipe | Result | Unicode |\n|:--|:-:|:--|\n");
    for (b, o, cp) in symbols::COMPOSITES {
        s.push_str(&format!("| `{b}.{o}` | ${b}.{o}$ | U+{cp:04X} |\n"));
    }
    s
}

fn kind_default(kind: &str, key: &str) -> String {
    let env = MathEnv::new();
    let st = Settings::default();
    let v = match (kind, key) {
        ("vec", "style") => env.vec_style.clone(),
        ("document", "style") => st.style.clone(),
        ("document", "accent") => "none".into(),
        ("heading", "numbering") => if st.numbering { "on".into() } else { "none".into() },
        ("page", "size") => st.page.clone(),
        ("page", "margin") => st.margin.clone(),
        ("text", "size") => st.text_size.clone(),
        ("text", "font") => st.font.clone().unwrap_or_else(|| "Functor LM".into()),
        _ => env.opt(kind, key),
    };
    if v.is_empty() {
        "none".into()
    } else {
        v
    }
}

fn set_rules_page() -> String {
    let mut s = header("All set rules");
    s.push_str("A `set` line changes the look of the whole document. Write it on its own line, before the text it changes.\n\n| Rule | Option | Default |\n|:--|:--|:--|\n");
    for (target, keys) in symbols::SET_OPTIONS {
        if *target == "greek" {
            s.push_str("| `set greek(...)` | a lowercase Greek name, such as `phi` | `italic` |\n");
            continue;
        }
        for k in keys.iter() {
            s.push_str(&format!("| `set {target}(...)` | `{k}` | `{}` |\n", kind_default(target, k)));
        }
    }
    s.push_str(&format!("\nA `fence` value is a fence operation such as `brack` or `vbar.x2`, the word `none`, or any one symbol, for example a `sym`. A `grow` value is `stretch` or `magnify`. An arrow `style` is {}. A Greek value is {}.\n", symbols::ARROW_STYLES.iter().map(|(st, name, _)| format!("`{st}` (${name}$)")).collect::<Vec<_>>().join(", "), symbols::GREEK_STYLES.iter().map(|g| format!("`{g}`")).collect::<Vec<_>>().join(", ")));
    s
}

pub fn generated_pages() -> Vec<Page> {
    let mut v = vec![
        Page { path: "symbols/names".into(), title: "Named symbols".into(), section: "Symbols", src: names_page(), generated: true },
        Page { path: "symbols/greek".into(), title: "Greek letters".into(), section: "Symbols", src: greek_page(), generated: true },
        Page { path: "symbols/functions".into(), title: "Functions and spaces".into(), section: "Symbols", src: functions_page(), generated: true },
        Page { path: "symbols/operations".into(), title: "Operations".into(), section: "Symbols", src: operations_page(), generated: true },
        Page { path: "symbols/composites".into(), title: "Font composites".into(), section: "Symbols", src: composites_page(), generated: true },
        Page { path: "document/set-rules".into(), title: "All set rules".into(), section: "Document", src: set_rules_page(), generated: true },
    ];
    for d in symbols::OPS {
        v.push(Page { path: format!("symbols/ops/{}", d.name), title: d.name.into(), section: "Operations", src: op_page(d), generated: true });
    }
    v
}

pub fn file_of(p: &Page) -> String {
    if p.generated {
        format!("{GEN_DIR}/{}.fn", p.path)
    } else {
        format!("{}.fn", p.path)
    }
}

pub fn load(dir: &std::path::Path) -> Result<Vec<Page>, String> {
    let mut pages = Vec::new();
    for (path, title, section) in HAND_PAGES {
        let file = dir.join(format!("{path}.fn"));
        let src = std::fs::read_to_string(&file).map_err(|e| format!("Cannot read {}: {e}", file.display()))?;
        pages.push(Page { path: path.to_string(), title: title.to_string(), section, src, generated: false });
    }
    pages.extend(generated_pages());
    Ok(pages)
}

pub fn fn_examples(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Option<Vec<&str>> = None;
    for line in src.lines() {
        let t = line.trim_start();
        match &mut cur {
            None if t.starts_with("```fn") => cur = Some(Vec::new()),
            Some(lines) if t.starts_with("```") => {
                out.push(lines.join("\n"));
                cur = None;
            }
            Some(lines) => lines.push(line),
            None => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::ENV_KINDS;
    use crate::diag::Severity;

    fn dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/reference")
    }

    fn pages() -> Vec<Page> {
        load(&dir()).expect("reference pages")
    }

    fn errors(src: &str) -> Vec<String> {
        crate::compile(src).diagnostics.iter().filter(|d| d.severity == Severity::Error).map(|d| format!("{}:{} {}", d.line, d.col, d.message)).collect()
    }

    #[test]
    fn every_page_compiles_without_errors_or_warnings() {
        for p in pages() {
            let out = crate::compile(&p.src);
            assert!(out.diagnostics.is_empty(), "{}: {:?}", p.path, out.diagnostics);
        }
    }

    #[test]
    fn every_fn_example_compiles() {
        let mut count = 0;
        for p in pages() {
            for ex in fn_examples(&p.src) {
                count += 1;
                let e = errors(&ex);
                assert!(e.is_empty(), "{}: example\n{ex}\n{e:?}", p.path);
            }
        }
        assert!(count > 20, "only {count} examples");
    }

    #[test]
    fn generated_files_are_fresh() {
        for p in generated_pages() {
            let file = dir().join(file_of(&p));
            let on_disk = std::fs::read_to_string(&file).unwrap_or_default();
            assert!(on_disk == p.src, "{} is out of date. Run `functor ref gen`.", file.display());
        }
    }

    #[test]
    fn every_construct_has_a_place() {
        let all: String = pages().iter().map(|p| p.src.as_str()).collect::<Vec<_>>().join("\n");
        for c in symbols::CALLS {
            assert!(all.contains(&format!("{c}(")), "call `{c}` is not in the reference");
        }
        for k in ENV_KINDS {
            assert!(all.contains(&format!("::: {k}")), "block `{k}` has no example");
        }
        for (t, keys) in symbols::SET_OPTIONS {
            let keys: &[&str] = if *t == "greek" { &["phi"] } else { keys };
            for k in keys.iter() {
                assert!(all.contains(&format!("set {t}(")) && all.contains(&format!("{k}:")), "set {t}({k}) is not in the reference");
            }
        }
        for op in symbols::OPS {
            assert!(generated_pages().iter().any(|p| p.path == format!("symbols/ops/{}", op.name)));
        }
    }

    #[test]
    fn cheat_sheet_names_every_operation_and_call() {
        let cs = std::fs::read_to_string(dir().join("cheatsheet.fn")).expect("cheat sheet");
        for op in symbols::OPS {
            assert!(cs.contains(&format!("`{}`", op.name)) || cs.contains(&format!(".{}", op.name)), "cheat sheet misses `{}`", op.name);
        }
        for c in symbols::CALLS {
            assert!(cs.contains(&format!("{c}(")), "cheat sheet misses `{c}(`");
        }
        for k in ENV_KINDS {
            assert!(cs.contains(k), "cheat sheet misses block `{k}`");
        }
        let names = symbols::NAMES.iter().map(|n| n.name).chain(symbols::GREEK.iter().copied()).chain(symbols::FUNCTIONS.iter().map(|(f, _)| *f)).chain(symbols::SPACES.iter().map(|(n, _)| *n));
        for n in names {
            assert!(cs.contains(&format!("`{n}`")), "cheat sheet misses the name `{n}`");
        }
    }

    #[test]
    fn examples_are_found() {
        assert_eq!(fn_examples("a\n```fn\n$x$\n```\n```text\ny\n```\n"), vec!["$x$".to_string()]);
    }
}
