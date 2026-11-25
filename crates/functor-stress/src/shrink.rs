pub fn shrink(src: &str, keep: &mut dyn FnMut(&str) -> bool, budget: usize) -> String {
    let mut probes = 0;
    let lines: Vec<String> = src.split_inclusive('\n').map(str::to_string).collect();
    let lines = ddmin(lines, keep, &mut probes, budget / 3);
    let words = split_words(&lines.concat());
    let words = ddmin(words, keep, &mut probes, budget * 2 / 3);
    let chars: Vec<String> = words.concat().chars().map(|c| c.to_string()).collect();
    ddmin(chars, keep, &mut probes, budget).concat()
}

fn split_words(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() {
            cur.push(c);
        } else {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            out.push(c.to_string());
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn ddmin(
    mut units: Vec<String>,
    keep: &mut dyn FnMut(&str) -> bool,
    probes: &mut usize,
    budget: usize,
) -> Vec<String> {
    let mut n = 2usize;
    while units.len() >= 2 && *probes < budget {
        let len = units.len();
        let chunk = len.div_ceil(n);
        let mut reduced = false;
        let mut start = 0;
        while start < len && *probes < budget {
            let end = (start + chunk).min(len);
            let cand: String = units[..start].concat() + &units[end..].concat();
            *probes += 1;
            if keep(&cand) {
                units.drain(start..end);
                n = (n - 1).max(2);
                reduced = true;
                break;
            }
            start = end;
        }
        if !reduced {
            if n >= len {
                break;
            }
            n = (n * 2).min(len);
        }
    }
    units
}

#[cfg(test)]
mod tests {
    #[test]
    fn finds_the_bad_part() {
        let src = "a b c\nd BAD e\nf g h\n";
        let out = super::shrink(src, &mut |s: &str| s.contains("BAD"), 300);
        assert_eq!(out, "BAD");
    }
}
