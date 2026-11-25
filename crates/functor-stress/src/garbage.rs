use crate::rng::Rng;

const PIECES: &[&str] = &[
    "(",
    ")",
    "[",
    "]",
    "{",
    "}",
    "$",
    "$$",
    "_",
    "^",
    ".",
    ",",
    ";",
    "\"",
    "\\",
    "@",
    "#",
    ":::",
    "^[",
    "{#",
    "---",
    "|",
    "`",
    "```",
    "*",
    "**",
    "\n",
    "\n\n",
    " ",
    "\t",
    "\r",
    "\0",
    "frac(",
    "sqrt(",
    "mat(",
    "cases(",
    "vec(",
    "wrap(",
    "join(",
    "stack(",
    ".not",
    ".x2",
    ".bb",
    ".wide",
    ".tall",
    ".paren",
    ".left",
    "set ",
    "sym ",
    "let ",
    "object vec: ",
    "__",
    "^^",
    "''''",
    "é",
    "∑",
    "𝔸",
    "\u{200B}",
    "\u{FEFF}",
    "\u{301}",
    "🙂",
    "\u{7F}",
    "x.y.z.w.v",
    "sum_",
    "integral^",
    "lim_(",
    "\"if",
    "{#eq:",
    "@eq:",
    "::: theorem",
    ":::",
    "1e999",
    "-0",
    "NaN",
];

pub fn mutate(src: &str, r: &mut Rng) -> String {
    let mut s: Vec<char> = src.chars().collect();
    let n = r.range(1, 12);
    for _ in 0..n {
        let len = s.len().max(1);
        let at = r.below(len + 1).min(s.len());
        match r.below(11) {
            0 => {
                let end = (at + r.range(1, 20)).min(s.len());
                s.drain(at..end);
            }
            1..=3 => {
                let p = r.pick(PIECES);
                for (k, c) in p.chars().enumerate() {
                    s.insert(at + k, c);
                }
            }
            4 => {
                let c = char::from_u32(r.range(1, 0x2FFFF) as u32).unwrap_or('?');
                s.insert(at, c);
            }
            5 => {
                let end = (at + r.range(1, 40)).min(s.len());
                let chunk: Vec<char> = s[at..end].to_vec();
                let to = r.below(s.len() + 1);
                for (k, c) in chunk.into_iter().enumerate() {
                    s.insert(to + k, c);
                }
            }
            6 => {
                s.truncate(at);
            }
            7 => {
                let depth = if r.chance(0.1) {
                    r.range(800, 2000)
                } else {
                    r.range(5, 300)
                };
                let open = *r.pick(&["(", "[", "{", "frac(", "sqrt(", "x_(", "x^("]);
                let mut ins = String::new();
                for _ in 0..depth {
                    ins.push_str(open);
                }
                ins.push('x');
                if r.chance(0.5) {
                    for _ in 0..depth {
                        ins.push_str(match open {
                            "frac(" => ", y)",
                            "[" => "]",
                            "{" => "}",
                            _ => ")",
                        });
                    }
                }
                for (k, c) in ins.chars().enumerate() {
                    s.insert(at + k, c);
                }
            }
            8 => {
                if at < s.len() && s[at].is_ascii_alphabetic() {
                    s[at] = (b'a' + r.below(26) as u8) as char;
                }
            }
            9 => {
                let reps = r.range(2, 60);
                let end = (at + r.range(1, 6)).min(s.len());
                let chunk: String = s[at..end].iter().collect();
                let ins = chunk.repeat(reps);
                for (k, c) in ins.chars().enumerate() {
                    s.insert(at + k, c);
                }
            }
            _ => {
                let a = r.below(len).min(s.len().saturating_sub(1));
                let b = r.below(len).min(s.len().saturating_sub(1));
                if a < s.len() && b < s.len() {
                    s.swap(a, b);
                }
            }
        }
    }
    s.into_iter().collect()
}

pub fn bytes(r: &mut Rng) -> String {
    let n = r.range(0, 400);
    let raw: Vec<u8> = (0..n).map(|_| r.below(256) as u8).collect();
    String::from_utf8_lossy(&raw).into_owned()
}
