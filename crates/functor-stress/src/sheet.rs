use crate::gen::{Gen, Tables};
use crate::rng::Rng;

pub fn sheet(seed: u64, count: usize) -> String {
    let t = Tables::new();
    let mut g = Gen::new(&t, Rng::new(seed));
    let mut out = format!(
        "---\nfunctor: 1\ntitle: Random formulas, seed {seed}\n---\n\nset heading(numbering: none)\n\nEach formula is random. Look for parts that touch, gaps that are too big or too small, and symbols that look wrong. Report a bad one by its number and seed {seed}.\n\n"
    );
    for i in 1..=count {
        let depth = 1 + (i % 3);
        let f = if i % 3 == 0 {
            g.display(depth)
        } else {
            g.formula(depth)
        };
        let code = f.replace('\n', " / ").replace('`', "'");
        out.push_str(&format!("**{i}.** `{code}`\n\n$$\n{f}\n$$\n\n"));
    }
    out
}
