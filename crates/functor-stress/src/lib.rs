pub mod check;
pub mod garbage;
pub mod gen;
pub mod ink;
pub mod rng;
pub mod run;
pub mod sheet;
pub mod shrink;

use check::Mode;
use gen::{Gen, Tables};
use rng::Rng;

pub fn page_source(t: &Tables, seed: u64, index: u64, mode: Mode, depth: usize) -> String {
    let mut r = Rng::for_page(seed, index);
    if mode == Mode::Garbage && r.chance(0.04) {
        return garbage::bytes(&mut r);
    }
    let mut g = Gen::new(t, r);
    let page = g.page(index, depth);
    match mode {
        Mode::Valid => page,
        Mode::Garbage => {
            let mut r = g.r.clone();
            garbage::mutate(&page, &mut r)
        }
    }
}

pub fn census(t: &Tables) -> Vec<String> {
    use functor_paper::math::MathEnv;
    let env = MathEnv::new();
    let mut bad = Vec::new();
    let mut recipes: Vec<String> = Vec::new();
    let bases: Vec<String> = functor_paper::symbols::NAMES
        .iter()
        .map(|n| n.name.to_string())
        .chain(t.greek.iter().map(|g| g.to_string()))
        .chain(t.letters.iter().cloned())
        .chain(t.composites.iter().cloned())
        .collect();
    for b in &bases {
        recipes.push(b.clone());
        for op in t.symbol_ops.iter().chain(t.style_ops.iter()) {
            recipes.push(format!("{b}.{op}"));
        }
    }
    for (a, b) in &t.stacks {
        recipes.push(format!("stack({a}, {b})"));
    }
    for r in recipes {
        if !gen::parses_clean(&r, &env) {
            continue;
        }
        for display in [false, true] {
            let node = check::parse_math(&r, &env, display);
            let b = functor_paper::math::display_box(&node);
            if !b.w.is_finite() || !b.h.is_finite() || !b.d.is_finite() {
                bad.push(format!("{r}: size is not a number"));
                break;
            }
            let marks = ink::marks(&b);
            if ink::rasterize(&marks).is_empty() {
                bad.push(format!("{r}: no ink"));
                break;
            }
        }
    }
    bad
}
