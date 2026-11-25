use functor_stress::check::{self, Mode, Opts};
use functor_stress::gen::{Gen, Tables};
use functor_stress::rng::Rng;
use functor_stress::{census, page_source, sheet};

fn regressions() -> Vec<(String, String)> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/regressions");
    let mut v: Vec<(String, String)> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "fn"))
        .map(|p| {
            (
                p.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read_to_string(&p).unwrap(),
            )
        })
        .collect();
    v.sort();
    v
}

#[test]
fn fixed_regressions_stay_fixed() {
    let rs = regressions();
    assert!(!rs.is_empty());
    for (name, src) in rs {
        let mode = if name.starts_with("garbage") {
            Mode::Garbage
        } else {
            Mode::Valid
        };
        let fs = check::check(
            &src,
            mode,
            Opts {
                collisions: false,
                ..Opts::default()
            },
        );
        assert!(fs.is_empty(), "{name}: {fs:?}");
    }
}

#[test]
fn generated_pages_compile_without_errors_or_crashes() {
    let t = Tables::new();
    for i in 0..40 {
        let src = page_source(&t, 7, i, Mode::Valid, 2);
        let fs = check::check(
            &src,
            Mode::Valid,
            Opts {
                collisions: false,
                pdf: i % 8 == 0,
                ..Opts::default()
            },
        );
        let bad: Vec<_> = fs.iter().filter(|f| f.kind != "slow").collect();
        assert!(bad.is_empty(), "page {i}: {bad:?}\n{src}");
    }
}

#[test]
fn garbage_pages_give_errors_with_lines_not_crashes() {
    let run = || {
        let t = Tables::new();
        for i in 0..40 {
            let src = page_source(&t, 7, i, Mode::Garbage, 2);
            let fs = check::check(
                &src,
                Mode::Garbage,
                Opts {
                    collisions: false,
                    pdf: false,
                    ..Opts::default()
                },
            );
            let bad: Vec<_> = fs.iter().filter(|f| f.kind != "slow").collect();
            assert!(bad.is_empty(), "page {i}: {bad:?}");
        }
    };
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn pages_are_the_same_for_the_same_seed() {
    let t = Tables::new();
    assert_eq!(
        page_source(&t, 3, 11, Mode::Valid, 3),
        page_source(&t, 3, 11, Mode::Valid, 3)
    );
    assert_ne!(
        page_source(&t, 3, 11, Mode::Valid, 3),
        page_source(&t, 3, 12, Mode::Valid, 3)
    );
}

#[test]
fn every_symbol_and_recipe_has_ink() {
    let bad = census(&Tables::new());
    assert!(bad.is_empty(), "{bad:?}");
}

#[test]
fn collision_check_sees_touching_parts() {
    use functor_paper::glyph::Affine;
    use functor_stress::ink::{rasterize, Mark};
    let a = rasterize(&[Mark::Rule(Affine::ID, 200.0, 40.0)]);
    let b = rasterize(&[Mark::Rule(Affine::translate(100.0, 0.0), 200.0, 40.0)]);
    let c = rasterize(&[Mark::Rule(Affine::translate(400.0, 0.0), 200.0, 40.0)]);
    assert!(a.intersection(&b).count() > 10);
    assert_eq!(a.intersection(&c).count(), 0);
}

#[test]
fn italic_letters_and_roots_do_not_touch() {
    for m in [
        "frac(a + b, c) = sum_(i = 1)^n x_i",
        "(V) (Y) (f) (N aleph) V 3",
        "T^d Y^e f(x) P_n^2 integral^d f",
        "W'_(x.tall) f'^2",
        "sqrt(w.circle.cross) sqrt(x)",
        "(p) wrap(P, t)",
    ] {
        for (open, close) in [("$$\n", "\n$$"), ("$", "$")] {
            let src = format!("---\nfunctor: 1\n---\n\n{open}{m}{close}\n");
            let hit = check::collisions(&src);
            assert!(hit.is_empty(), "{m}: {:?}", hit.iter().map(|c| (&c.what, c.overlap)).collect::<Vec<_>>());
        }
    }
}

#[test]
fn pdf_check_reads_a_real_pdf() {
    let p = functor_paper::compile_pdf("---\nfunctor: 1\n---\n\nText and $x^2$.\n");
    check::pdf_ok(&p.pdf, p.pages).unwrap();
    let mut broken = p.pdf.clone();
    broken.truncate(broken.len() / 2);
    assert!(check::pdf_ok(&broken, p.pages).is_err());
}

#[test]
fn contact_sheet_compiles() {
    let src = sheet::sheet(5, 12);
    let out = functor_paper::compile(&src);
    assert_eq!(out.errors(), 0, "{:?}", out.diagnostics);
}

#[test]
fn generator_reaches_every_call() {
    let t = Tables::new();
    let mut g = Gen::new(&t, Rng::new(9));
    let text: String = (0..400).map(|_| g.formula(3)).collect::<Vec<_>>().join(" ");
    for call in functor_paper::symbols::CALLS
        .iter()
        .filter(|c| **c != "text")
    {
        assert!(
            text.contains(&format!("{call}(")),
            "generator never writes `{call}(`"
        );
    }
}
