use std::fs;

const INPUTS: [&str; 4] = ["data/glyphs.tsv", "src/glyph.rs", "src/math.rs", "src/symbols.rs"];

fn main() {
    let mut h: u64 = 0xcbf29ce484222325;
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    for b in version.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    for path in INPUTS {
        println!("cargo:rerun-if-changed={path}");
        for b in fs::read(path).unwrap_or_default() {
            h = (h ^ b as u64).wrapping_mul(0x100000001b3);
        }
    }
    println!("cargo:rustc-env=FUNCTOR_GLYPH_KEY={h:016x}");
}
