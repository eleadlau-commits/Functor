pub const INF: f64 = 10000.0;
pub const FILL: f64 = 1.0e6;

const LINE_PENALTY: f64 = 10.0;
const ADJ_DEMERITS: f64 = 10000.0;
const DOUBLE_HYPHEN_DEMERITS: f64 = 10000.0;
const FINAL_HYPHEN_DEMERITS: f64 = 5000.0;
const PRETOLERANCE: f64 = 100.0;
const TOLERANCE: f64 = 200.0;
const EMERGENCY_STRETCH: f64 = 1.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Box,
    Glue,
    Penalty,
}

#[derive(Clone, Copy, Debug)]
pub struct BItem {
    pub kind: Kind,
    pub w: f64,
    pub stretch: f64,
    pub shrink: f64,
    pub p: f64,
    pub flagged: bool,
    pub disc: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub end: usize,
    pub ratio: f64,
    pub protrude: (f64, f64),
    pub expand: (f64, f64),
    pub glue: (f64, f64),
}

#[derive(Clone, Copy)]
struct Node {
    pos: usize,
    from: usize,
    fitness: usize,
    total: f64,
    w: f64,
    st: f64,
    sh: f64,
    flagged: bool,
    prev: Option<usize>,
}

fn badness(r: f64) -> f64 {
    if r.is_infinite() {
        return INF;
    }
    (100.0 * r.abs().powi(3)).min(INF)
}

fn fitness(r: f64) -> usize {
    if r < -0.5 {
        0
    } else if r <= 0.5 {
        1
    } else if r <= 1.0 {
        2
    } else {
        3
    }
}

struct Pass {
    tolerance: f64,
    hyphens: bool,
    emergency: f64,
    last_resort: bool,
}

pub fn break_lines(items: &[BItem], width: f64, margins: &[(f64, f64)]) -> Vec<Line> {
    let passes = [
        Pass { tolerance: PRETOLERANCE, hyphens: false, emergency: 0.0, last_resort: false },
        Pass { tolerance: TOLERANCE, hyphens: true, emergency: 0.0, last_resort: false },
        Pass { tolerance: TOLERANCE, hyphens: true, emergency: EMERGENCY_STRETCH, last_resort: true },
    ];
    for pass in &passes {
        if let Some(lines) = run(items, width, margins, pass) {
            return lines;
        }
    }
    vec![Line { end: items.len() - 1, ratio: 0.0, protrude: (0.0, 0.0), expand: (0.0, 0.0), glue: (0.0, 0.0) }]
}

fn first_box(items: &[BItem], from: usize, to: usize) -> Option<usize> {
    (from..to).find(|&i| items[i].kind == Kind::Box)
}

fn last_box(items: &[BItem], from: usize, to: usize) -> Option<usize> {
    (from..to).rev().find(|&i| items[i].kind == Kind::Box)
}

fn run(items: &[BItem], width: f64, margins: &[(f64, f64)], pass: &Pass) -> Option<Vec<Line>> {
    let n = items.len();
    let mut sw = vec![0.0; n + 1];
    let mut sst = vec![0.0; n + 1];
    let mut ssh = vec![0.0; n + 1];
    let mut xst = vec![0.0; n + 1];
    let mut xsh = vec![0.0; n + 1];
    for (i, it) in items.iter().enumerate() {
        let (w, st, sh) = match it.kind {
            Kind::Box => (it.w, it.stretch, it.shrink),
            Kind::Glue => (it.w, it.stretch, it.shrink),
            Kind::Penalty => (0.0, 0.0, 0.0),
        };
        let (bx, bs) = if it.kind == Kind::Box { (it.stretch, it.shrink) } else { (0.0, 0.0) };
        sw[i + 1] = sw[i] + w;
        sst[i + 1] = sst[i] + st;
        ssh[i + 1] = ssh[i] + sh;
        xst[i + 1] = xst[i] + bx;
        xsh[i + 1] = xsh[i] + bs;
    }
    let mut nodes = vec![Node { pos: 0, from: 0, fitness: 1, total: 0.0, w: 0.0, st: 0.0, sh: 0.0, flagged: false, prev: None }];
    let mut active: Vec<usize> = vec![0];
    for b in 0..n {
        let it = items[b];
        let feasible = match it.kind {
            Kind::Penalty => it.p < INF && (pass.hyphens || !it.disc),
            Kind::Glue => b > 0 && items[b - 1].kind == Kind::Box,
            Kind::Box => false,
        };
        if !feasible {
            continue;
        }
        let forced = it.kind == Kind::Penalty && it.p <= -INF;
        let pw = if it.kind == Kind::Penalty { it.w } else { 0.0 };
        let mut best: [Option<(f64, usize, f64)>; 4] = [None; 4];
        let mut fallback: Option<(f64, usize, f64)> = None;
        let mut i = 0;
        while i < active.len() {
            let ai = active[i];
            let a = nodes[ai];
            let start = if a.pos == 0 { 0 } else { a.pos + 1 };
            let fb = first_box(items, a.pos, b);
            let lb = last_box(items, start.min(b), b);
            let pl = fb.map_or(0.0, |k| margins[k].0);
            let pr = if it.kind == Kind::Penalty && it.disc { margins[b].1 } else { lb.map_or(0.0, |k| margins[k].1) };
            let target = width + pl + pr;
            let l = sw[b] - a.w + pw;
            let y = sst[b] - a.st + pass.emergency;
            let z = ssh[b] - a.sh;
            let r = if l < target {
                if y > 0.0 {
                    (target - l) / y
                } else {
                    f64::INFINITY
                }
            } else if l > target {
                if z > 0.0 {
                    (target - l) / z
                } else {
                    f64::NEG_INFINITY
                }
            } else {
                0.0
            };
            if r < -1.0 || forced {
                active.remove(i);
            } else {
                i += 1;
            }
            let bad = badness(r);
            if r >= -1.0 && bad <= pass.tolerance {
                let mut d = (LINE_PENALTY + bad).powi(2);
                if it.kind == Kind::Penalty {
                    if it.p >= 0.0 {
                        d += it.p * it.p;
                    } else if it.p > -INF {
                        d -= it.p * it.p;
                    }
                }
                if it.flagged && a.flagged {
                    d += DOUBLE_HYPHEN_DEMERITS;
                }
                if forced && a.flagged {
                    d += FINAL_HYPHEN_DEMERITS;
                }
                let f = fitness(r);
                if (f as i64 - a.fitness as i64).abs() > 1 {
                    d += ADJ_DEMERITS;
                }
                let total = a.total + d;
                let slot = &mut best[f];
                if slot.is_none_or(|(t, _, _)| total < t) {
                    *slot = Some((total, ai, r));
                }
            } else if pass.last_resort && (r < -1.0 || bad > pass.tolerance) {
                let total = a.total + (LINE_PENALTY + INF).powi(2);
                if fallback.is_none_or(|(t, _, _)| total < t) {
                    fallback = Some((total, ai, r.max(-1.0)));
                }
            }
        }
        if best.iter().all(Option::is_none) && active.is_empty() {
            if let Some(fb) = fallback {
                best[fitness(fb.2)] = Some(fb);
            }
        }
        let min_total = best.iter().flatten().map(|x| x.0).fold(f64::INFINITY, f64::min);
        let mut j = b;
        while j < n {
            match items[j].kind {
                Kind::Box => break,
                Kind::Penalty if items[j].p <= -INF && j > b => break,
                _ => j += 1,
            }
        }
        for (f, slot) in best.iter().enumerate() {
            if let Some((total, prev, _)) = *slot {
                if total <= min_total + ADJ_DEMERITS {
                    nodes.push(Node {
                        pos: b,
                        from: j,
                        fitness: f,
                        total,
                        w: sw[j],
                        st: sst[j],
                        sh: ssh[j],
                        flagged: it.flagged,
                        prev: Some(prev),
                    });
                    active.push(nodes.len() - 1);
                }
            }
        }
        if active.is_empty() {
            return None;
        }
    }
    let end = (0..nodes.len()).filter(|&k| nodes[k].pos == n - 1).min_by(|&x, &y| nodes[x].total.total_cmp(&nodes[y].total))?;
    let mut chain = Vec::new();
    let mut k = Some(end);
    while let Some(i) = k {
        if nodes[i].prev.is_none() {
            break;
        }
        chain.push(i);
        k = nodes[i].prev;
    }
    chain.reverse();
    let mut lines = Vec::new();
    let mut prev = nodes[0];
    for &i in &chain {
        let node = nodes[i];
        let b = node.pos;
        let it = items[b];
        let start = if prev.pos == 0 && prev.prev.is_none() { 0 } else { prev.pos + 1 };
        let pw = if it.kind == Kind::Penalty { it.w } else { 0.0 };
        let fb = first_box(items, start.min(b), b);
        let lb = last_box(items, start.min(b), b);
        let pl = fb.map_or(0.0, |k| margins[k].0);
        let pr = if it.kind == Kind::Penalty && it.disc { margins[b].1 } else { lb.map_or(0.0, |k| margins[k].1) };
        let target = width + pl + pr;
        let l = sw[b] - prev.w + pw;
        let y = sst[b] - prev.st;
        let z = ssh[b] - prev.sh;
        let r = if l < target {
            if y > 0.0 {
                (target - l) / y
            } else {
                0.0
            }
        } else if l > target && z > 0.0 {
            ((target - l) / z).max(-1.0)
        } else {
            0.0
        };
        let ex = (xst[b] - xst[prev.from], xsh[b] - xsh[prev.from]);
        let gl = (y - ex.0, z - ex.1);
        lines.push(Line { end: b, ratio: r, protrude: (pl, pr), expand: ex, glue: gl });
        prev = node;
    }
    Some(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(w: f64) -> BItem {
        BItem { kind: Kind::Box, w, stretch: 0.0, shrink: 0.0, p: 0.0, flagged: false, disc: false }
    }

    fn space() -> BItem {
        BItem { kind: Kind::Glue, w: 1.0, stretch: 0.5, shrink: 0.33, p: 0.0, flagged: false, disc: false }
    }

    fn para(words: &[f64]) -> Vec<BItem> {
        let mut v = Vec::new();
        for (i, &w) in words.iter().enumerate() {
            if i > 0 {
                v.push(space());
            }
            v.push(word(w));
        }
        v.push(BItem { kind: Kind::Glue, w: 0.0, stretch: FILL, shrink: 0.0, p: 0.0, flagged: false, disc: false });
        v.push(BItem { kind: Kind::Penalty, w: 0.0, stretch: 0.0, shrink: 0.0, p: -INF, flagged: false, disc: false });
        v
    }

    #[test]
    fn lines_stay_inside_their_limits() {
        let words = [3.0, 5.0, 2.0, 7.0, 4.0, 3.0, 6.0, 2.0, 5.0, 4.0, 3.0, 8.0, 2.0, 4.0, 5.0, 3.0];
        let items = para(&words);
        let margins = vec![(0.0, 0.0); items.len()];
        let lines = break_lines(&items, 20.0, &margins);
        assert!(lines.len() >= 4);
        for l in &lines[..lines.len() - 1] {
            assert!(l.ratio >= -1.0 && l.ratio <= 2.0, "{l:?}");
        }
        assert_eq!(lines.last().unwrap().end, items.len() - 1);
    }

    #[test]
    fn total_fit_beats_first_fit() {
        let words = [4.0, 4.0, 4.0, 4.0, 10.0, 4.0, 4.0, 4.0, 4.0];
        let items = para(&words);
        let margins = vec![(0.0, 0.0); items.len()];
        let lines = break_lines(&items, 18.0, &margins);
        let worst = lines[..lines.len() - 1].iter().map(|l| l.ratio.abs()).fold(0.0, f64::max);
        assert!(worst < 1.5, "{lines:?}");
    }
}
