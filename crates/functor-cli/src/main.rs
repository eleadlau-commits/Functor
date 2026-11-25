use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use functor_paper::diag::Diagnostic;
use functor_paper::diag::Severity;
use functor_paper::{compile, compile_pdf, glyph, glyph_svg};

const USAGE: &str = "functor compile <file.fn> [-o out.html]
functor pdf <file.fn> [-o out.pdf]
functor check <file.fn>
functor glyph <recipe>
functor bench [file.fn]
functor ref gen [docs/reference]
functor ref build <out-dir> [docs/reference]

Built glyphs are kept in a disk cache. Set FUNCTOR_CACHE_DIR to move it, or pass --no-cache.";

const BENCH_SYMBOLS: &[&str] = &[
    "sum.circle", "prod.circle", "union.circle", "inter.box", "integral.flip", "theta.hat", "theta.tilde", "theta.bar",
    "x.dot", "x.ddot", "f.check", "u.breve", "v.arrow", "F.bold", "R.bb", "x.under", "alpha.hat", "beta.circle",
    "gamma.box", "delta.not", "in.cross", "subset.circle", "le.box", "approx.circle", "sim.box", "to.not", "mapsto.not",
    "iff.not", "partial.x2", "nabla.circle", "nabla.flip", "infty.circle", "star.circle", "wedge.circle", "vee.box",
    "cup.circle", "cap.box", "ell.hat", "aleph.bar", "Omega.hat", "Sigma.turn", "Pi.flipv", "Psi.tilde", "Phi.circle",
    "x.hat.circle", "y.bar.not", "theta.hat.box", "sum.x2", "integral.x2.flip", "a.bold.hat",
];

fn print_diags(path: &str, diags: &[Diagnostic]) {
    for d in diags {
        eprintln!("{path}:{}:{}: {}: {}", d.line, d.col, d.severity.as_str(), d.message);
        if let Some(h) = &d.hint {
            eprintln!("  hint: {h}");
        }
    }
}

fn read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("Cannot read {path}: {e}"))
}

const FONTS: [(&str, &str, u32, &[u8]); 5] = [
    ("Functor LM", "normal", 400, include_bytes!("../../../web/src/fonts/functor-lm-regular.woff2")),
    ("Functor LM", "italic", 400, include_bytes!("../../../web/src/fonts/functor-lm-italic.woff2")),
    ("Functor LM", "normal", 700, include_bytes!("../../../web/src/fonts/functor-lm-bold.woff2")),
    ("Functor LM", "italic", 700, include_bytes!("../../../web/src/fonts/functor-lm-bolditalic.woff2")),
    ("Functor LM Mono", "normal", 400, include_bytes!("../../../web/src/fonts/functor-lm-mono.woff2")),
];

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn font_faces() -> String {
    FONTS
        .iter()
        .map(|(family, style, weight, data)| {
            format!(
                "@font-face{{font-family:\"{family}\";font-style:{style};font-weight:{weight};font-display:block;src:url(data:font/woff2;base64,{}) format(\"woff2\")}}",
                base64(data)
            )
        })
        .collect()
}

fn page(body: &str, title: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>{}</title><style>{}{}</style></head><body>{body}<script>{}functorRefs(document.body);</script></body></html>",
        functor_paper::render::escape(title),
        font_faces(),
        include_str!("../../../web/src/doc.css"),
        include_str!("../../../web/src/refs.js")
    )
}

fn cache_file() -> Option<PathBuf> {
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let dir = env("FUNCTOR_CACHE_DIR")
        .or_else(|| env("XDG_CACHE_HOME").map(|d| d.join("functor")))
        .or_else(|| env("LOCALAPPDATA").map(|d| d.join("functor")))
        .or_else(|| env("HOME").map(|d| d.join(".cache").join("functor")))?;
    Some(dir.join("glyphs.txt"))
}

fn load_cache(file: &Path) -> usize {
    std::fs::read_to_string(file).map(|t| glyph::import_cache(&t)).unwrap_or(0)
}

fn save_cache(file: &Path) {
    if !glyph::take_dirty() {
        return;
    }
    let Some(dir) = file.parent() else { return };
    let tmp = file.with_extension("tmp");
    let _ = std::fs::create_dir_all(dir)
        .and_then(|_| std::fs::write(&tmp, glyph::export_cache()))
        .and_then(|_| std::fs::rename(&tmp, file));
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn bench(file: Option<&str>) -> Result<bool, String> {
    let mut ok = true;
    let mut worst = (0.0, "");
    let mut total = 0.0;
    for s in BENCH_SYMBOLS {
        glyph::clear_cache();
        let t = Instant::now();
        let svg = glyph_svg(s).ok_or_else(|| format!("The recipe `{s}` does not build."))?;
        let dt = ms(t);
        assert!(!svg.is_empty());
        total += dt;
        if dt > worst.0 {
            worst = (dt, s);
        }
    }
    let gate_sym = worst.0 < 5.0;
    ok &= gate_sym;
    println!("symbols   {} recipes, cold build", BENCH_SYMBOLS.len());
    println!("          mean {:.3} ms, worst {:.3} ms ({})  gate < 5 ms: {}", total / BENCH_SYMBOLS.len() as f64, worst.0, worst.1, pass(gate_sym));
    glyph::clear_cache();
    let t = Instant::now();
    for s in BENCH_SYMBOLS {
        glyph_svg(s);
    }
    let cold = ms(t);
    let t = Instant::now();
    for s in BENCH_SYMBOLS {
        glyph_svg(s);
    }
    let warm = ms(t);
    println!("cache     all {} cold {:.3} ms, warm {:.3} ms", BENCH_SYMBOLS.len(), cold, warm);
    if let Some(path) = file {
        let src = read(path)?;
        glyph::clear_cache();
        let t = Instant::now();
        let out = compile(&src);
        let first = ms(t);
        let mut runs = Vec::new();
        for _ in 0..20 {
            let t = Instant::now();
            compile(&src);
            runs.push(ms(t));
        }
        runs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let gate_doc = first < 100.0;
        ok &= gate_doc;
        println!("document  {path}: {} lines, {} errors", src.lines().count(), out.errors());
        println!("          first {:.2} ms, median warm {:.2} ms  gate < 100 ms: {}", first, runs[runs.len() / 2], pass(gate_doc));
        println!("          glyphs built {}, cache hits {}, from font {}", out.stats.built, out.stats.hits, out.stats.from_font);
        let text = glyph::export_cache();
        let t = Instant::now();
        glyph::clear_cache();
        let n = glyph::import_cache(&text);
        let load = ms(t);
        let t = Instant::now();
        compile(&src);
        let after = ms(t);
        println!("disk      {} glyphs, {} bytes, load {:.3} ms, then compile {:.2} ms", n, text.len(), load, after);
    }
    Ok(ok)
}

fn pass(b: bool) -> &'static str {
    if b {
        "PASS"
    } else {
        "FAIL"
    }
}

fn run(args: &[String]) -> Result<bool, String> {
    let disk = !args.iter().any(|a| a == "--no-cache");
    let args: Vec<String> = args.iter().filter(|a| *a != "--no-cache").cloned().collect();
    let file = cache_file().filter(|_| disk && args.first().is_some_and(|c| c != "bench"));
    if let Some(f) = &file {
        load_cache(f);
    }
    let result = run_cmd(&args);
    if let Some(f) = &file {
        save_cache(f);
    }
    result
}

fn run_cmd(args: &[String]) -> Result<bool, String> {
    let cmd = args.first().map(String::as_str).unwrap_or("");
    match cmd {
        "compile" | "check" => {
            let path = args.get(1).ok_or(USAGE)?;
            let src = read(path)?;
            let out = compile(&src);
            print_diags(path, &out.diagnostics);
            if cmd == "compile" {
                let dest = match args.iter().position(|a| a == "-o") {
                    Some(i) => args.get(i + 1).cloned().ok_or(USAGE)?,
                    None => path.trim_end_matches(".fn").to_string() + ".html",
                };
                let title = std::path::Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or("Functor");
                std::fs::write(&dest, page(&out.html, title)).map_err(|e| format!("Cannot write {dest}: {e}"))?;
                println!("wrote {dest}");
            }
            Ok(out.errors() == 0)
        }
        "pdf" => {
            let path = args.get(1).ok_or(USAGE)?;
            let src = read(path)?;
            let out = compile_pdf(&src);
            print_diags(path, &out.diagnostics);
            let dest = match args.iter().position(|a| a == "-o") {
                Some(i) => args.get(i + 1).cloned().ok_or(USAGE)?,
                None => path.trim_end_matches(".fn").to_string() + ".pdf",
            };
            std::fs::write(&dest, &out.pdf).map_err(|e| format!("Cannot write {dest}: {e}"))?;
            println!("wrote {dest} ({} pages)", out.pages);
            Ok(!out.diagnostics.iter().any(|d| d.severity == Severity::Error))
        }
        "glyph" => {
            let recipe = args.get(1).ok_or(USAGE)?;
            match glyph_svg(recipe) {
                Some(svg) => {
                    println!("{svg}");
                    Ok(true)
                }
                None => Err(format!("The recipe `{recipe}` does not build. Run `functor check` on a file that uses it to see why.")),
            }
        }
        "bench" => bench(args.get(1).map(String::as_str)),
        "ref" => reference(&args[1..]),
        _ => Err(USAGE.into()),
    }
}

const REF_CSS: &str = "body.ref{margin:0;display:flex;align-items:flex-start;background:#f4f4f6;font-family:\"Functor LM\",serif}\
.ref-nav{position:sticky;top:0;flex:0 0 230px;height:100vh;overflow-y:auto;box-sizing:border-box;padding:20px 16px;font-size:14px;line-height:1.5;background:#fff;border-right:1px solid #d9d9e0}\
.ref-nav a{display:block;color:#1b1b1f;text-decoration:none;padding:1px 6px;border-radius:4px}\
.ref-nav a:hover{background:#eef1f5}.ref-nav a.here{background:#e3e9f0;color:#2f4a62;font-weight:700}\
.ref-nav h2{font-size:12px;letter-spacing:.06em;text-transform:uppercase;color:#5b5b66;margin:16px 0 4px}\
.ref-nav .ref-home{font-size:17px;font-weight:700;padding:0 6px 4px}\
.ref-nav .ref-ops{display:flex;flex-wrap:wrap;gap:0 2px}.ref-nav .ref-ops a{display:inline-block;font-family:\"Functor LM Mono\",monospace;font-size:12px}\
.ref-nav .ref-tools{margin-top:16px;border-top:1px solid #d9d9e0;padding-top:8px}\
.ref-main{flex:1 1 auto;overflow-x:auto;padding:24px 0}.ref-main .fn-doc{box-shadow:0 1px 4px rgba(0,0,0,.08)}\
@media (max-width:900px){body.ref{display:block}.ref-nav{position:static;height:auto;border-right:0;border-bottom:1px solid #d9d9e0}}";

fn ref_page(p: &functor_paper::reference::Page, pages: &[functor_paper::reference::Page], html: &str) -> String {
    use functor_paper::reference::SECTIONS;
    use functor_paper::render::escape;
    let depth = p.path.matches('/').count();
    let root = "../".repeat(depth);
    let mut nav = String::from("<a class=\"ref-home\" href=\"index.html\">Functor reference</a>");
    let mut sections: Vec<&str> = SECTIONS.to_vec();
    sections.push("Operations");
    for sec in sections {
        let items: Vec<&functor_paper::reference::Page> = pages.iter().filter(|q| q.section == sec && q.path != "index").collect();
        if items.is_empty() {
            continue;
        }
        nav.push_str(&format!("<h2>{}</h2>", escape(sec)));
        if sec == "Operations" {
            nav.push_str("<div class=\"ref-ops\">");
        }
        for q in items {
            let here = if q.path == p.path { " class=\"here\"" } else { "" };
            nav.push_str(&format!("<a href=\"{}.html\"{here}>{}</a>", q.path, escape(&q.title)));
        }
        if sec == "Operations" {
            nav.push_str("</div>");
        }
    }
    nav.push_str(&format!(
        "<div class=\"ref-tools\"><a href=\"{}.fn\">Source of this page</a><a href=\"llms.txt\">For AI: llms.txt</a><a href=\"../\">Open the editor</a></div>",
        p.path
    ));
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{} · Functor reference</title><base href=\"{root}\"><link rel=\"stylesheet\" href=\"style.css\"></head><body class=\"ref\"><nav class=\"ref-nav\">{nav}</nav><main class=\"ref-main\">{html}</main></body></html>",
        escape(&p.title)
    )
}

fn llms_txt(pages: &[functor_paper::reference::Page]) -> String {
    let mut s = String::from("# Functor\n\n> Functor is a typesetting language for mathematics. It is not LaTeX: no backslashes, round brackets for groups, symbols built from a base and short operations (theta.hat, in.not, sum.circle).\n\nRead cheatsheet.fn first. It holds the whole grammar on one page. latex.fn maps LaTeX to Functor. Check your output with `functor check file.fn`.\n\n## Pages (Functor source)\n\n");
    for p in pages {
        s.push_str(&format!("- [{}]({}.fn)\n", p.title, p.path));
    }
    s
}

fn reference(args: &[String]) -> Result<bool, String> {
    use functor_paper::reference::{file_of, generated_pages, load};
    let sub = args.first().map(String::as_str).unwrap_or("");
    match sub {
        "gen" => {
            let dir = std::path::PathBuf::from(args.get(1).map(String::as_str).unwrap_or("docs/reference"));
            for p in generated_pages() {
                let file = dir.join(file_of(&p));
                if let Some(parent) = file.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| format!("Cannot make {}: {e}", parent.display()))?;
                }
                std::fs::write(&file, &p.src).map_err(|e| format!("Cannot write {}: {e}", file.display()))?;
            }
            println!("wrote {} generated pages to {}", generated_pages().len(), dir.join("gen").display());
            Ok(true)
        }
        "build" => {
            let out = std::path::PathBuf::from(args.get(1).ok_or(USAGE)?);
            let dir = std::path::PathBuf::from(args.get(2).map(String::as_str).unwrap_or("docs/reference"));
            let pages = load(&dir)?;
            let mut ok = true;
            for p in &pages {
                let res = compile(&p.src);
                print_diags(&dir.join(file_of(p)).display().to_string(), &res.diagnostics);
                ok &= res.errors() == 0;
                for (ext, body) in [("html", ref_page(p, &pages, &res.html)), ("fn", p.src.clone())] {
                    let file = out.join(format!("{}.{ext}", p.path));
                    if let Some(parent) = file.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| format!("Cannot make {}: {e}", parent.display()))?;
                    }
                    std::fs::write(&file, body).map_err(|e| format!("Cannot write {}: {e}", file.display()))?;
                }
            }
            let css = format!("{}{}{}", font_faces(), include_str!("../../../web/src/doc.css"), REF_CSS);
            std::fs::write(out.join("style.css"), css).map_err(|e| format!("Cannot write style.css: {e}"))?;
            std::fs::write(out.join("llms.txt"), llms_txt(&pages)).map_err(|e| format!("Cannot write llms.txt: {e}"))?;
            println!("wrote {} pages to {}", pages.len(), out.display());
            Ok(ok)
        }
        _ => Err(USAGE.into()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_values() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn page_embeds_fonts_and_language() {
        let html = page("<p>x</p>", "t");
        assert!(html.starts_with("<!doctype html><html lang=\"en\">"));
        assert_eq!(html.matches("font/woff2;base64,").count(), 5);
    }

    #[test]
    fn cache_file_survives_a_round_trip() {
        let dir = std::env::temp_dir().join(format!("functor-cache-test-{}", std::process::id()));
        let file = dir.join("glyphs.txt");
        glyph::clear_cache();
        glyph_svg("sum.circle").unwrap();
        save_cache(&file);
        glyph::clear_cache();
        assert_eq!(load_cache(&file), 1);
        assert!(!glyph::take_dirty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
