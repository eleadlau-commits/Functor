pub mod ast;
pub mod diag;
pub mod doc;
pub mod glyph;
pub mod inline;
pub mod json;
pub mod linebreak;
pub mod math;
pub mod page;
pub mod pdf;
pub mod render;
pub mod symbols;
pub mod text;

use diag::{Diagnostic, Diagnostics, Severity};
use glyph::CacheStats;

pub struct Output {
    pub html: String,
    pub diagnostics: Vec<Diagnostic>,
    pub stats: CacheStats,
}

impl Output {
    pub fn errors(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Error).count()
    }

    pub fn to_json(&self) -> String {
        let diags = json::array(self.diagnostics.iter().map(|d| {
            json::Obj::new()
                .str("severity", d.severity.as_str())
                .num("line", d.line)
                .num("col", d.col)
                .str("message", &d.message)
                .raw("hint", json::opt(d.hint.as_deref()))
                .end()
        }));
        let stats = json::Obj::new()
            .num("built", self.stats.built)
            .num("hits", self.stats.hits)
            .num("fromFont", self.stats.from_font)
            .end();
        json::Obj::new().str("html", &self.html).raw("diagnostics", diags).raw("stats", stats).end()
    }
}

pub fn compile(src: &str) -> Output {
    compile_width(src, None)
}

pub fn compile_width(src: &str, width_pt: Option<f64>) -> Output {
    glyph::take_stats();
    let mut diags = Diagnostics::default();
    let d = doc::parse(src, &mut diags);
    let html = render::render_width(&d, &mut diags, width_pt);
    Output { html, diagnostics: diags.finish(), stats: glyph::take_stats() }
}

pub struct PdfOutput {
    pub pdf: Vec<u8>,
    pub diagnostics: Vec<Diagnostic>,
    pub pages: usize,
}

pub fn compile_pdf(src: &str) -> PdfOutput {
    let mut diags = Diagnostics::default();
    let d = doc::parse(src, &mut diags);
    let layout = page::layout(&d, &mut diags);
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
    fn symbols_list_is_json() {
        let s = symbols_json();
        assert!(s.contains("\"circle\"") && s.contains("\"theorem\""));
    }
}
