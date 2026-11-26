pub use functor_math::{deform, diag, glyph, lint, math, symbols, trace};
pub mod ast;
pub mod doc;
pub mod incr;
pub mod inline;
pub mod json;
pub mod linebreak;
pub mod page;
pub mod pdf;
pub mod reference;
pub mod render;
pub mod text;

use diag::{Diagnostic, Diagnostics, Severity};
use glyph::CacheStats;
use trace::Trace;

pub struct Output {
    pub html: String,
    pub diagnostics: Vec<Diagnostic>,
    pub stats: CacheStats,
    pub traces: Vec<Trace>,
}

impl Output {
    pub fn errors(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Error).count()
    }

    pub fn to_json(&self) -> String {
        let diags = diags_json(&self.diagnostics);
        let stats = json::Obj::new()
            .num("built", self.stats.built)
            .num("hits", self.stats.hits)
            .num("fromFont", self.stats.from_font)
            .end();
        let traces = json::array(self.traces.iter().map(trace_json));
        json::Obj::new().str("html", &self.html).raw("diagnostics", diags).raw("stats", stats).raw("traces", traces).end()
    }
}

pub fn diags_json(diagnostics: &[Diagnostic]) -> String {
    json::array(diagnostics.iter().map(|d| {
        json::Obj::new()
            .str("severity", d.severity.as_str())
            .num("line", d.line)
            .num("col", d.col)
            .str("message", &d.message)
            .raw("hint", json::opt(d.hint.as_deref()))
            .end()
    }))
}

pub fn trace_json(t: &Trace) -> String {
    let strs = |v: &[String]| json::array(v.iter().map(|s| json::string(s)));
    let ops = json::array(t.ops.iter().map(|o| json::Obj::new().str("name", o).str("meaning", trace::op_meaning(o)).end()));
    json::Obj::new()
        .num("line", t.line)
        .num("col", t.col)
        .num("endLine", t.end_line)
        .num("endCol", t.end_col)
        .str("source", &t.source)
        .str("form", t.form.as_str())
        .str("meaning", &t.meaning)
        .raw("style", strs(&t.style))
        .raw("hand", strs(&t.hand))
        .str("recipe", &t.recipe)
        .raw("ops", ops)
        .str("glyph", &t.glyph.text())
        .end()
}

fn traced<T>(env: &math::MathEnv, diags: &mut Diagnostics, f: impl FnOnce(&mut Diagnostics) -> T) -> (T, Vec<Trace>) {
    trace::start();
    let out = f(diags);
    let traces = trace::finish();
    lint::lint(&traces.iter().collect::<Vec<_>>(), env, diags);
    (out, traces)
}

pub fn compile(src: &str) -> Output {
    compile_width(src, None)
}

pub fn compile_width(src: &str, width_pt: Option<f64>) -> Output {
    glyph::take_stats();
    let mut diags = Diagnostics::default();
    let d = doc::parse(src, &mut diags);
    let (html, traces) = traced(&d.env, &mut diags, |diags| render::render_width(&d, diags, width_pt));
    Output { html, diagnostics: diags.finish(), stats: glyph::take_stats(), traces }
}

pub struct PdfOutput {
    pub pdf: Vec<u8>,
    pub diagnostics: Vec<Diagnostic>,
    pub pages: usize,
}

pub fn compile_pdf(src: &str) -> PdfOutput {
    let mut diags = Diagnostics::default();
    let d = doc::parse(src, &mut diags);
    let (layout, _) = traced(&d.env, &mut diags, |diags| page::layout(&d, diags));
    let pages = layout.pages.len();
    PdfOutput { pdf: pdf::write(&layout), diagnostics: diags.finish(), pages }
}

pub fn glyph_svg(recipe: &str) -> Option<String> {
    math::glyph_preview(recipe, &math::MathEnv::new())
}

pub fn symbols_json() -> String {
    let names = json::array(symbols::all_names().into_iter().map(|n| {
        let kind = if symbols::CALLS.contains(&n) {
            "call"
        } else if symbols::function(n).is_some() {
            "function"
        } else if symbols::greek(n).is_some() {
            "greek"
        } else if symbols::space(n).is_some() {
            "space"
        } else {
            "symbol"
        };
        json::Obj::new().str("name", n).str("kind", kind).end()
    }));
    let ops = json::array(symbols::OPS.iter().map(|o| json::Obj::new().str("name", o.name).str("meaning", o.meaning).end()));
    let later = json::array(symbols::LATER_OPS.iter().map(|(n, _)| json::string(n)));
    let envs = json::array(doc::ENV_KINDS.iter().map(|e| json::string(e)));
    json::Obj::new().raw("names", names).raw("ops", ops).raw("later", later).raw("blocks", envs).end()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "---\nfunctor: 1\ntitle: Test\n---\n\nobject vec: F\nsym csum = sum.circle\nlet n = 3\n\n# Intro {#sec:intro}\n\nWe have $F = m a$ and {n} items, see @eq:main and @sec:intro.\n\n$$\ncsum_(k = 1)^n a_k = 0\n$$ {#eq:main}\n\n::: theorem {#thm:a title=\"Main\"}\nIf $x in.not A$ then @thm:a holds.\n:::\n";

    #[test]
    fn blocks_carry_source_lines() {
        let src = "---\nfunctor: 1\n---\n\n# Intro\n\nOne line\nand two.\n\n::: note\nFirst.\n\n$$\nx = 1\n$$\n:::\n";
        let html = compile(src).html;
        assert!(html.contains("<h2 data-fn-line=\"5\" data-fn-end-line=\"5\""), "{html}");
        assert!(html.contains("<p data-fn-line=\"7\" data-fn-end-line=\"8\""), "{html}");
        assert!(html.contains("data-fn-line=\"10\" data-fn-end-line=\"16\""), "{html}");
        assert!(html.contains("<div data-fn-rel=\"3-5\" class=\"fn-equation\""), "{html}");
    }

    #[test]
    fn compiles_sample_without_diagnostics() {
        let out = compile(SAMPLE);
        assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
        assert!(out.html.contains("Theorem\u{A0}1"));
        assert!(out.html.contains("(1)"));
        assert!(out.html.contains("Section\u{A0}1"));
        assert!(out.to_json().starts_with("{\"html\""));
    }

    #[test]
    fn reports_bad_ref_and_missing_version() {
        let out = compile("See @eq:mian.\n\n$$ x $$ {#eq:main}\n");
        assert_eq!(out.diagnostics.len(), 2, "{:?}", out.diagnostics);
        assert!(out.diagnostics.iter().any(|d| d.hint.as_deref() == Some("Did you mean `@eq:main`?")));
    }

    #[test]
    fn phase_one_features_are_refused_clearly() {
        let out = compile("functor: 1\n\nuse physics\n\n$x.deform$\n");
        assert!(out.diagnostics.iter().any(|d| d.message.contains("phase 1")));
    }

    #[test]
    fn set_rules_for_structures() {
        let ok = compile("---\nfunctor: 1\n---\n\nsym lsf = L.script\nset vec(fence: angle, body: row, grow: magnify)\nset mat(fence: brack)\nset cases(word: \"if\")\nset norm(fence: lsf)\n\n$vec(1, 2) + mat(1; 2) + norm(x) + cases(1, x gt 0)$\n");
        assert!(ok.diagnostics.is_empty(), "{:?}", ok.diagnostics);
        let bad = compile("---\nfunctor: 1\n---\n\nset abs(fence: nope.x)\nset vec(body: diagonal)\nset wrap(grow: big)\nset vec(style: column)\n");
        assert_eq!(bad.errors(), 4, "{:?}", bad.diagnostics);
        assert!(bad.diagnostics.iter().any(|d| d.hint.as_deref() == Some("For vectors with components, write `set vec(body: column)`.")));
    }

    #[test]
    fn set_rules_for_arrow_kinds_and_greek() {
        let ok = compile("---\nfunctor: 1\n---\n\nset mono(style: hook)\nset epi(style: plain)\nset greek(phi: alt, pi: up)\n\n$f: A mono B$, $phi + pi + yo(A)$\n");
        assert!(ok.diagnostics.is_empty(), "{:?}", ok.diagnostics);
        let bad = compile("---\nfunctor: 1\n---\n\nset mono(style: hooks)\nset greek(alpha: alt)\nset greek(phi: script)\nset greek(phy: alt)\n");
        assert_eq!(bad.errors(), 4, "{:?}", bad.diagnostics);
        assert!(bad.diagnostics.iter().any(|d| d.hint.as_deref() == Some("Did you mean `hook`?")));
        assert!(bad.diagnostics.iter().any(|d| d.message == "`alpha` has no other common form."));
        assert!(bad.diagnostics.iter().any(|d| d.hint.as_deref() == Some("Did you mean `phi`?")));
    }

    #[test]
    fn set_rules_for_symbol_names() {
        let paths = |src: &str| -> Vec<String> {
            let html = compile(&format!("---\nfunctor: 1\n---\n\n{src}\n")).html;
            html.split(" d=\"").skip(1).map(|p| p.split('"').next().unwrap_or("").to_string()).collect()
        };
        assert_eq!(paths("set le(style: alt)\n\n$a le b$"), paths("$a le.alt b$"));
        assert_eq!(paths("set partial(style: slant)\n\n$partial f$"), paths("$partial.slant f$"));
        assert_eq!(paths("set implies(style: wide)\n\n$A implies B$"), paths("$A implies.wide B$"));
        assert_eq!(paths("set implies(style: wide)\n\n$A implies.wide B$"), paths("$A implies.wide B$"));
        assert_ne!(paths("$a le.alt.not b$"), paths("$a le.not b$"));
        let ok = compile("---\nfunctor: 1\n---\n\nset le(style: alt)\nset integral(limits: over)\nset lim(limits: side)\nset limits(overhang: off)\n\n$a le.not b + integral_0^1 f + lim_(n to infty) a_n$\n");
        assert!(ok.diagnostics.is_empty(), "{:?}", ok.diagnostics);
        let bad = compile("---\nfunctor: 1\n---\n\nset le(style: hat)\nset emptyset(style: alt)\nset sum(limits: under)\nset sin(limits: over)\nset lee(style: alt)\nset limits(overhang: maybe)\nset eq(limits: side)\n");
        assert_eq!(bad.errors(), 7, "{:?}", bad.diagnostics);
        assert!(bad.diagnostics.iter().any(|d| d.message == "`hat` changes what `le` means, so a style rule cannot give it."));
        assert!(bad.diagnostics.iter().any(|d| d.message == "`emptyset` has no other common form."));
        assert!(bad.diagnostics.iter().any(|d| d.hint.as_deref() == Some("Did you mean `le`?")));
    }

    #[test]
    fn overhang_rule_keeps_limits_inside_the_operator_box() {
        let src = "$$\nx + sum_(i = 1, j lt i, k lt j)^n y\n$$\n";
        let width = |rule: &str| -> f64 {
            let html = compile(&format!("---\nfunctor: 1\n---\n\n{rule}{src}")).html;
            let vb = html.rsplit("viewBox=\"").next().unwrap_or("").split('"').next().unwrap_or("").to_string();
            vb.split(' ').nth(2).and_then(|w| w.parse().ok()).unwrap_or(0.0)
        };
        assert!(width("set limits(overhang: off)\n\n") > width("") + 500.0);
    }

    #[test]
    fn page_break_rules() {
        let out = compile("---\nfunctor: 1\n---\n\nset heading(new_page: 1)\n\n# One\n\nText.\n\n# Two\n\nText.\n\n===\n\n## Three\n");
        assert!(out.diagnostics.iter().all(|d| d.severity != crate::diag::Severity::Error));
        assert_eq!(out.html.matches("fn-new-page").count(), 1);
        assert_eq!(out.html.matches("fn-page-break").count(), 1);
        let bad = compile("---\nfunctor: 1\n---\n\nset heading(new_page: 7)\n\nTitle\n===\n\n::: note\n===\n:::\n");
        let msgs: Vec<&str> = bad.diagnostics.iter().map(|d| d.message.as_str()).collect();
        assert!(msgs.iter().any(|m| m.contains("not a heading level")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("does not make the line above a heading")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("cannot go in a block")), "{msgs:?}");
    }

    #[test]
    fn document_style_rules() {
        let quiet = compile("---\nfunctor: 1\n---\n\nText.\n");
        assert!(quiet.html.contains("fn-doc fn-style-quiet"));
        let classic = compile("---\nfunctor: 1\n---\n\nset document(style: classic, accent: #7091AC)\n\nText.\n");
        assert!(classic.diagnostics.is_empty(), "{:?}", classic.diagnostics);
        assert!(classic.html.contains("fn-doc fn-style-classic"));
        assert!(classic.html.contains("--fn-accent:#7091AC"));
        let bad = compile("---\nfunctor: 1\n---\n\nset document(style: quite, accent: blue)\n");
        assert_eq!(bad.errors(), 2, "{:?}", bad.diagnostics);
        assert!(bad.diagnostics.iter().any(|d| d.hint.as_deref() == Some("Did you mean `quiet`?")));
    }

    #[test]
    fn margin_notes() {
        let src = "---\nfunctor: 1\n---\n\nText with a note.^[A note with $x^2$ and [a link](https://example.org).] More text.^[Second.]\n";
        let quiet = compile(src);
        assert!(quiet.diagnostics.is_empty(), "{:?}", quiet.diagnostics);
        assert!(quiet.html.contains("fn-has-notes"));
        assert_eq!(quiet.html.matches("class=\"fn-mark\"").count(), 2);
        assert_eq!(quiet.html.matches("class=\"fn-side\"").count(), 2);
        assert!(quiet.html.contains("<span class=\"fn-side-n\">2</span>"));
        let classic = compile(&src.replace("---\n\nText", "---\n\nset document(style: classic)\n\nText"));
        assert!(classic.html.contains("fn-style-classic fn-has-notes"));
        let open = compile("---\nfunctor: 1\n---\n\nA note ^[never closed.\n");
        assert_eq!(open.errors(), 1, "{:?}", open.diagnostics);
        let plain = compile("---\nfunctor: 1\n---\n\nNo notes, and \\^[ is plain.\n");
        assert!(!plain.html.contains("fn-has-notes") && plain.html.contains("^["));
        assert!(compile_pdf(src).diagnostics.is_empty());
    }

    #[test]
    fn trace_shows_four_layers() {
        let out = compile("---\nfunctor: 1\n---\n\nobject vec: v\nsym csum = sum.circle\n\n$v + norm(v) + csum + theta.hat$\n");
        assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
        let at = |src: &str| out.traces.iter().find(|t| t.source == src).unwrap_or_else(|| panic!("no trace for {src}: {:?}", out.traces));
        let v = at("v");
        assert_eq!((v.line, v.col, v.form), (8, 2, trace::Form::Object));
        assert_eq!(v.recipe, "v.arrow");
        assert!(v.style[0].contains("set vec(style: arrow)"));
        let n = at("norm(v)");
        assert_eq!((n.form, n.recipe.as_str()), (trace::Form::Kind, "fence: vbar.x2"));
        let c = at("csum");
        assert_eq!((c.form, c.recipe.as_str()), (trace::Form::Sym, "sum.circle"));
        let h = at("theta.hat");
        assert_eq!(h.ops[0], "hat");
        assert_eq!(h.glyph, trace::Source::Built);
        assert!(out.to_json().contains("\"traces\":[{"));
    }

    #[test]
    fn notation_check() {
        let src = "---\nfunctor: 1\n---\n\nobject vec: v\nsym csum = sum.circle\n\n$norm(v) + (w).vbar.x2 + abs(x) + (y).vbar + v.bold + v.arrow + u.arrow + u.bold + csum + sum.circle$\n";
        let out = compile(src);
        let hints: Vec<&str> = out.diagnostics.iter().filter_map(|d| d.hint.as_deref()).collect();
        for want in ["Write `norm(w)`.", "Write `abs(y)`.", "Remove `.bold`, or change the rule for all vectors: `set vec(style: bold)`.", "Remove `.arrow`.", "Write `csum`."] {
            assert!(hints.contains(&want), "missing {want}: {:?}", out.diagnostics);
        }
        assert!(out.diagnostics.iter().any(|d| d.message.starts_with("`u` has two vector looks")));
        assert!(out.diagnostics.iter().all(|d| d.severity == Severity::Warning));
        assert_eq!(out.diagnostics.len(), 6, "{:?}", out.diagnostics);
        let off = compile(&src.replace("sym csum", "set lint(notation: off)\nsym csum"));
        assert!(off.diagnostics.is_empty(), "{:?}", off.diagnostics);
        let two = compile("---\nfunctor: 1\n---\n\nset abs(fence: vbar.x2)\n\n$abs(x) + norm(v)$\n");
        assert!(two.diagnostics.iter().any(|d| d.message.contains("different meanings")), "{:?}", two.diagnostics);
        let bad = compile("---\nfunctor: 1\n---\n\nset lint(notation: maybe)\n");
        assert_eq!(bad.errors(), 1);
    }

    #[test]
    fn style_presets() {
        let src = "---\nfunctor: 1\ntitle: T\nauthor: A\n---\n\nset document(style: STYLE)\n\n# One\n\nText.^[A note.] More.^[Close.]\n\n::: theorem {title=\"X\"}\nBody.\n:::\n\n- An item.^[In a list.]\n";
        for st in ["quiet", "classic", "lecture", "compact", "board"] {
            let doc = src.replace("STYLE", st);
            let out = compile(&doc);
            assert!(out.diagnostics.is_empty(), "{st}: {:?}", out.diagnostics);
            assert!(out.html.contains(&format!("fn-style-{st}")));
            let pdf = compile_pdf(&doc);
            assert!(pdf.diagnostics.is_empty() && pdf.pages >= 1, "{st}");
        }
        let compact = compile(&src.replace("STYLE", "compact"));
        assert!(compact.html.contains("fn-cols-2"));
        let narrow = compile_width(&src.replace("STYLE", "compact"), Some(300.0));
        assert!(!narrow.html.contains("fn-cols-2"));
        let board = compile(&src.replace("STYLE", "board"));
        assert!(board.html.contains("--fn-text-size:13.2pt"));
        let lecture = compile(&src.replace("STYLE", "lecture"));
        assert!(lecture.html.contains("fn-headed"));
    }

    #[test]
    fn compact_indents_paragraphs() {
        let src = "---\nfunctor: 1\n---\n\nset document(style: STYLE)\n\n# One\n\nAlpha text.\n\nBravo text.\n\n::: theorem\nCharlie text.\n\nDelta text.\n:::\n\nEcho text.\n\n- Foxtrot.\n- Golf.\n\nHotel text.\n";
        let compact = compile(&src.replace("STYLE", "compact"));
        assert!(compact.diagnostics.is_empty(), "{:?}", compact.diagnostics);
        assert_eq!(compact.html.matches("fn-par fn-indented").count(), 2);
        assert_eq!(compact.html.matches("class=\"fn-indent\"").count(), 2);
        for st in ["quiet", "classic", "lecture", "board"] {
            let out = compile(&src.replace("STYLE", st));
            assert!(!out.html.contains("fn-indented"), "{st}");
        }
    }

    #[test]
    fn symbols_list_is_json() {
        let s = symbols_json();
        assert!(s.contains("\"circle\"") && s.contains("\"theorem\""));
    }
}
