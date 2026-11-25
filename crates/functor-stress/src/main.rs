use std::path::PathBuf;

use functor_stress::check::{self, Mode, Opts};
use functor_stress::gen::Tables;
use functor_stress::run::{self, RunOpts};
use functor_stress::{census, page_source, sheet};

const USAGE: &str = "functor-stress run [--seed N] [--from N] [--pages N] [--mode valid|garbage|both] [--jobs N] [--depth N] [--pdf-every N] [--shrink N] [--hang-ms N] [--out DIR]
functor-stress page --seed N --index N [--mode valid|garbage] [--depth N]
functor-stress check <file.fn> [--mode valid|garbage]
functor-stress sheet [--seed N] [--count N] [--out DIR]
functor-stress census";

fn flag<T: std::str::FromStr>(args: &[String], name: &str, default: T) -> T {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn mode_flag(args: &[String]) -> Option<Mode> {
    let m: String = flag(args, "--mode", "both".to_string());
    Mode::parse(&m)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("");
    match cmd {
        "run" => {
            let d = RunOpts::default();
            let o = RunOpts {
                seed: flag(&args, "--seed", d.seed),
                from: flag(&args, "--from", d.from),
                pages: flag(&args, "--pages", d.pages),
                mode: mode_flag(&args),
                jobs: flag(&args, "--jobs", d.jobs),
                out: flag(&args, "--out", d.out.clone()),
                hang_ms: flag(&args, "--hang-ms", d.hang_ms),
                shrink: flag(&args, "--shrink", d.shrink),
                depth: flag(&args, "--depth", d.depth),
                pdf_every: flag(&args, "--pdf-every", d.pdf_every),
            };
            let s = run::run(&o);
            let md = run::report(&o, &s);
            let _ = std::fs::create_dir_all(&o.out);
            let _ = std::fs::write(o.out.join("report.md"), &md);
            println!("{md}");
            if !s.new_sigs.is_empty() {
                eprintln!(
                    "{} new kinds of failure. See {}",
                    s.new_sigs.len(),
                    o.out.join("report.md").display()
                );
                std::process::exit(1);
            }
        }
        "child" => run::child(
            flag(&args, "--seed", 1),
            flag(&args, "--from", 0),
            flag(&args, "--to", 0),
            mode_flag(&args),
            flag(&args, "--depth", 3),
            flag(&args, "--pdf-every", 4),
        ),
        "probe" => run::probe_child(mode_flag(&args).unwrap_or(Mode::Valid)),
        "page" => {
            let i = flag(&args, "--index", 0u64);
            let m = run::page_mode(mode_flag(&args), i);
            print!(
                "{}",
                page_source(
                    &Tables::new(),
                    flag(&args, "--seed", 1),
                    i,
                    m,
                    flag(&args, "--depth", 3)
                )
            );
        }
        "check" => {
            let Some(path) = args.get(1) else {
                eprintln!("{USAGE}");
                std::process::exit(2);
            };
            let src = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("{path}: {e}");
                std::process::exit(2)
            });
            let fs = check::check(
                &src,
                mode_flag(&args).unwrap_or(Mode::Valid),
                Opts::default(),
            );
            for f in &fs {
                println!("{}\n    {}", f.sig, f.detail);
            }
            if !fs.is_empty() {
                std::process::exit(1);
            }
            println!("ok");
        }
        "sheet" => {
            let seed = flag(&args, "--seed", 1u64);
            let out: PathBuf = flag(&args, "--out", PathBuf::from("target/stress/sheet"));
            let _ = std::fs::create_dir_all(&out);
            let src = sheet::sheet(seed, flag(&args, "--count", 50));
            let html = functor_paper::compile(&src);
            let pdf = functor_paper::compile_pdf(&src);
            let _ = std::fs::write(out.join("sheet.fn"), &src);
            let _ = std::fs::write(out.join("sheet.pdf"), &pdf.pdf);
            println!(
                "{} formulas, {} errors, written to {}",
                flag(&args, "--count", 50),
                html.errors(),
                out.display()
            );
        }
        "census" => {
            let bad = census(&Tables::new());
            for b in &bad {
                println!("{b}");
            }
            println!("{} recipes with a problem", bad.len());
            if !bad.is_empty() {
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}
