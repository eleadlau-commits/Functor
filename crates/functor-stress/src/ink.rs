use std::collections::HashSet;

use functor_paper::glyph::{Affine, Glyph, Shape, STROKE};
use functor_paper::math::{self, LBox, Node, Paint};
use functor_paper::symbols::Limits;

pub const CELL: f64 = 10.0;
pub const MIN_OVERLAP: usize = 4;

#[derive(Clone)]
pub enum Mark {
    Glyph(Affine, Glyph),
    Rule(Affine, f64, f64),
}

pub fn marks(b: &LBox) -> Vec<Mark> {
    let mut v = Vec::new();
    b.paint(&mut |p| match p {
        Paint::Glyph(a, g) => v.push(Mark::Glyph(a, g.clone())),
        Paint::Rule(a, w, h) => v.push(Mark::Rule(a, w, h)),
    });
    v
}

fn count(n: &Node, display: bool) -> usize {
    marks(&lay(n, display)).len()
}

fn lay(n: &Node, display: bool) -> LBox {
    if display {
        math::display_box(n)
    } else {
        let p = math::inline_pieces(n, "");
        if p.len() == 1 {
            (*p[0].lbox).clone()
        } else {
            math::display_box(n)
        }
    }
}

type Pt = (f64, f64);

fn flatten(d: &str) -> Vec<(Vec<Pt>, bool)> {
    let mut out: Vec<(Vec<Pt>, bool)> = Vec::new();
    let mut cur: Vec<Pt> = Vec::new();
    let toks = tokens(d);
    let mut i = 0;
    let mut cmd = 'M';
    let (mut x, mut y) = (0.0, 0.0);
    let (mut sx, mut sy) = (0.0, 0.0);
    let (mut lcx, mut lcy) = (0.0, 0.0);
    let num = |i: &mut usize| -> Option<f64> {
        match toks.get(*i) {
            Some(Tok::N(v)) => {
                *i += 1;
                Some(*v)
            }
            _ => None,
        }
    };
    while i < toks.len() {
        if let Tok::C(c) = toks[i] {
            cmd = c;
            i += 1;
            if c == 'Z' || c == 'z' {
                if !cur.is_empty() {
                    out.push((std::mem::take(&mut cur), true));
                }
                x = sx;
                y = sy;
                continue;
            }
        }
        let rel = cmd.is_ascii_lowercase();
        let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
        let before = i;
        match cmd.to_ascii_uppercase() {
            'M' => {
                let (Some(a), Some(b)) = (num(&mut i), num(&mut i)) else {
                    break;
                };
                if cur.len() > 1 {
                    out.push((std::mem::take(&mut cur), false));
                }
                cur.clear();
                x = ox + a;
                y = oy + b;
                sx = x;
                sy = y;
                cur.push((x, y));
                cmd = if rel { 'l' } else { 'L' };
            }
            'L' => {
                let (Some(a), Some(b)) = (num(&mut i), num(&mut i)) else {
                    break;
                };
                x = ox + a;
                y = oy + b;
                cur.push((x, y));
            }
            'H' => {
                let Some(a) = num(&mut i) else { break };
                x = ox + a;
                cur.push((x, y));
            }
            'V' => {
                let Some(b) = num(&mut i) else { break };
                y = oy + b;
                cur.push((x, y));
            }
            'C' | 'S' => {
                let s = cmd.eq_ignore_ascii_case(&'S');
                let (c1x, c1y) = if s {
                    (2.0 * x - lcx, 2.0 * y - lcy)
                } else {
                    let (Some(a), Some(b)) = (num(&mut i), num(&mut i)) else {
                        break;
                    };
                    (ox + a, oy + b)
                };
                let (Some(a), Some(b), Some(c), Some(e)) =
                    (num(&mut i), num(&mut i), num(&mut i), num(&mut i))
                else {
                    break;
                };
                let (c2x, c2y, ex, ey) = (ox + a, oy + b, ox + c, oy + e);
                for k in 1..=8 {
                    let t = k as f64 / 8.0;
                    let u = 1.0 - t;
                    cur.push((
                        u * u * u * x
                            + 3.0 * u * u * t * c1x
                            + 3.0 * u * t * t * c2x
                            + t * t * t * ex,
                        u * u * u * y
                            + 3.0 * u * u * t * c1y
                            + 3.0 * u * t * t * c2y
                            + t * t * t * ey,
                    ));
                }
                lcx = c2x;
                lcy = c2y;
                x = ex;
                y = ey;
            }
            'Q' => {
                let (Some(a), Some(b), Some(c), Some(e)) =
                    (num(&mut i), num(&mut i), num(&mut i), num(&mut i))
                else {
                    break;
                };
                let (cx, cy, ex, ey) = (ox + a, oy + b, ox + c, oy + e);
                for k in 1..=6 {
                    let t = k as f64 / 6.0;
                    let u = 1.0 - t;
                    cur.push((
                        u * u * x + 2.0 * u * t * cx + t * t * ex,
                        u * u * y + 2.0 * u * t * cy + t * t * ey,
                    ));
                }
                x = ex;
                y = ey;
            }
            'A' => {
                let (Some(rx), Some(_ry), Some(_rot), Some(large), Some(sweep), Some(a), Some(b)) = (
                    num(&mut i),
                    num(&mut i),
                    num(&mut i),
                    num(&mut i),
                    num(&mut i),
                    num(&mut i),
                    num(&mut i),
                ) else {
                    break;
                };
                let (ex, ey) = (ox + a, oy + b);
                arc(
                    &mut cur,
                    (x, y),
                    (ex, ey),
                    rx.abs(),
                    large != 0.0,
                    sweep != 0.0,
                );
                x = ex;
                y = ey;
            }
            _ => {
                i += 1;
            }
        }
        if i == before {
            i += 1;
        }
    }
    if cur.len() > 1 {
        out.push((cur, false));
    }
    out
}

fn arc(cur: &mut Vec<Pt>, p0: Pt, p1: Pt, r: f64, large: bool, sweep: bool) {
    let (dx, dy) = ((p1.0 - p0.0) / 2.0, (p1.1 - p0.1) / 2.0);
    let d2 = dx * dx + dy * dy;
    if d2 == 0.0 || r == 0.0 {
        cur.push(p1);
        return;
    }
    let r = r.max(d2.sqrt());
    let h = ((r * r - d2) / d2).max(0.0).sqrt();
    let sign = if large == sweep { -1.0 } else { 1.0 };
    let mx = (p0.0 + p1.0) / 2.0;
    let my = (p0.1 + p1.1) / 2.0;
    let (cx, cy) = (mx + sign * h * -dy, my + sign * h * dx);
    let a0 = (p0.1 - cy).atan2(p0.0 - cx);
    let mut a1 = (p1.1 - cy).atan2(p1.0 - cx);
    let tau = std::f64::consts::TAU;
    if sweep && a1 < a0 {
        a1 += tau;
    }
    if !sweep && a1 > a0 {
        a1 -= tau;
    }
    for k in 1..=16 {
        let a = a0 + (a1 - a0) * k as f64 / 16.0;
        cur.push((cx + r * a.cos(), cy + r * a.sin()));
    }
}

enum Tok {
    C(char),
    N(f64),
}

fn tokens(d: &str) -> Vec<Tok> {
    let b = d.as_bytes();
    let mut v = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        if c.is_ascii_alphabetic() && c != 'e' && c != 'E' {
            v.push(Tok::C(c));
            i += 1;
        } else if c == '-' || c == '+' || c == '.' || c.is_ascii_digit() {
            let s = i;
            i += 1;
            let mut dot = c == '.';
            while i < b.len() {
                let e = b[i] as char;
                if e.is_ascii_digit() || (e == '.' && !dot) {
                    dot |= e == '.';
                    i += 1;
                } else if (e == 'e' || e == 'E') && i + 1 < b.len() {
                    i += 1;
                    if b[i] == b'-' || b[i] == b'+' {
                        i += 1;
                    }
                } else {
                    break;
                }
            }
            if let Ok(x) = d[s..i].parse() {
                v.push(Tok::N(x));
            }
        } else {
            i += 1;
        }
    }
    v
}

pub type Cells = HashSet<(i32, i32)>;

fn fill(polys: &[Vec<Pt>], cells: &mut Cells) {
    let (mut y0, mut y1) = (f64::MAX, f64::MIN);
    for p in polys {
        for q in p {
            y0 = y0.min(q.1);
            y1 = y1.max(q.1);
        }
    }
    if y0 > y1 || !y0.is_finite() || !y1.is_finite() || y1 - y0 > 1e6 {
        return;
    }
    let r0 = (y0 / CELL).floor() as i32;
    let r1 = (y1 / CELL).ceil() as i32;
    let mut xs: Vec<(f64, i32)> = Vec::new();
    for row in r0..=r1 {
        let yc = (row as f64 + 0.5) * CELL;
        xs.clear();
        for p in polys {
            let n = p.len();
            for k in 0..n {
                let (a, b) = (p[k], p[(k + 1) % n]);
                if (a.1 <= yc) != (b.1 <= yc) {
                    let t = (yc - a.1) / (b.1 - a.1);
                    xs.push((a.0 + t * (b.0 - a.0), if b.1 > a.1 { 1 } else { -1 }));
                }
            }
        }
        xs.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut wind = 0;
        for k in 0..xs.len() {
            let prev = wind;
            wind += xs[k].1;
            if prev != 0 && k > 0 {
                let c0 = (xs[k - 1].0 / CELL - 0.5).ceil() as i32;
                let c1 = (xs[k].0 / CELL - 0.5).floor() as i32;
                for c in c0..=c1 {
                    cells.insert((c, row));
                }
            }
        }
    }
}

fn stroke(lines: &[Vec<Pt>], r: f64, cells: &mut Cells) {
    for l in lines {
        for w in l.windows(2) {
            let (a, b) = (w[0], w[1]);
            let c0 = ((a.0.min(b.0) - r) / CELL).floor() as i32;
            let c1 = ((a.0.max(b.0) + r) / CELL).ceil() as i32;
            let r0 = ((a.1.min(b.1) - r) / CELL).floor() as i32;
            let r1 = ((a.1.max(b.1) + r) / CELL).ceil() as i32;
            if (c1 - c0) as i64 * (r1 - r0) as i64 > 4_000_000 {
                continue;
            }
            for cx in c0..=c1 {
                for cy in r0..=r1 {
                    let p = ((cx as f64 + 0.5) * CELL, (cy as f64 + 0.5) * CELL);
                    if seg_dist(p, a, b) <= r {
                        cells.insert((cx, cy));
                    }
                }
            }
        }
    }
}

fn seg_dist(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 == 0.0 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0)
    };
    let (qx, qy) = (a.0 + t * dx, a.1 + t * dy);
    ((p.0 - qx).powi(2) + (p.1 - qy).powi(2)).sqrt()
}

pub fn rasterize(ms: &[Mark]) -> Cells {
    let mut cells = Cells::new();
    for m in ms {
        match m {
            Mark::Rule(a, w, h) => {
                let poly = vec![
                    a.apply(0.0, 0.0),
                    a.apply(*w, 0.0),
                    a.apply(*w, *h),
                    a.apply(0.0, *h),
                ];
                fill(&[poly], &mut cells);
            }
            Mark::Glyph(a, g) => {
                for it in &g.items {
                    let t = it.tf.then(*a);
                    let (path, is_stroke) = match &it.shape {
                        Shape::Font(p) => (*p, false),
                        Shape::Outline(p) => (&**p, false),
                        Shape::Stroke(p) => (p.as_str(), true),
                    };
                    let subs: Vec<(Vec<Pt>, bool)> = flatten(path)
                        .into_iter()
                        .map(|(pts, closed)| {
                            (
                                pts.into_iter().map(|(x, y)| t.apply(x, y)).collect(),
                                closed,
                            )
                        })
                        .collect();
                    if is_stroke {
                        let m = t.0;
                        let sc = (m[0] * m[3] - m[1] * m[2]).abs().sqrt();
                        let lines: Vec<Vec<Pt>> = subs
                            .into_iter()
                            .map(|(mut p, closed)| {
                                if closed && !p.is_empty() {
                                    p.push(p[0]);
                                }
                                p
                            })
                            .collect();
                        stroke(&lines, STROKE * sc / 2.0, &mut cells);
                    } else {
                        let polys: Vec<Vec<Pt>> = subs.into_iter().map(|(p, _)| p).collect();
                        fill(&polys, &mut cells);
                    }
                }
            }
        }
    }
    cells
}

pub fn has_ink(g: &Glyph) -> bool {
    !rasterize(&[Mark::Glyph(Affine::ID, g.clone())]).is_empty()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Collision {
    pub what: String,
    pub overlap: usize,
    pub display: bool,
}

struct Part {
    name: String,
    n: usize,
}

fn parts(node: &Node, display: bool) -> Option<(&'static str, Vec<Part>, Vec<(usize, usize)>)> {
    let p = |name: &str, n: usize| Part {
        name: name.to_string(),
        n,
    };
    match node {
        Node::Scripts { base, sub, sup } => {
            let limits = display
                && matches!(
                    base.as_ref(),
                    Node::Glyph {
                        limits: Limits::Display,
                        ..
                    }
                );
            let b = p("base", count(base, display));
            let sp = sup.as_ref().map(|s| p("sup", count(s, display)));
            let sb = sub.as_ref().map(|s| p("sub", count(s, display)));
            let v: Vec<Part> = if limits {
                [sp, sb, Some(b)].into_iter().flatten().collect()
            } else {
                [Some(b), sp, sb].into_iter().flatten().collect()
            };
            let pairs = all_pairs(v.len());
            Some((if limits { "limits" } else { "scripts" }, v, pairs))
        }
        Node::Over { above, below, .. } => {
            let mut v = vec![p("arrow", 1)];
            if let Some(a) = above {
                v.push(p("above", count(a, false)));
            }
            if let Some(b) = below {
                v.push(p("below", count(b, false)));
            }
            let pairs = all_pairs(v.len());
            Some(("label", v, pairs))
        }
        Node::Frac(a, b) => Some((
            "frac",
            vec![
                p("rule", 1),
                p("num", count(a, false)),
                p("den", count(b, false)),
            ],
            vec![(0, 1), (0, 2), (1, 2)],
        )),
        Node::Sqrt(b) => Some((
            "sqrt",
            vec![p("radical", 1), p("bar", 1), p("body", count(b, display))],
            vec![(0, 2), (1, 2)],
        )),
        Node::Fenced {
            open, close, body, ..
        } => {
            let mut v = Vec::new();
            if open.is_some() {
                v.push(p("open", 1));
            }
            let bi = v.len();
            v.push(p("body", count(body, display)));
            if close.is_some() {
                v.push(p("close", 1));
            }
            let pairs = (0..v.len())
                .filter(|&k| k != bi)
                .map(|k| (k.min(bi), k.max(bi)))
                .collect();
            Some(("fence", v, pairs))
        }
        Node::Row(ns) if ns.len() > 1 => {
            let v: Vec<Part> = ns
                .iter()
                .enumerate()
                .map(|(k, n)| p(&format!("item{k}"), count(n, display)))
                .collect();
            let pairs = (1..v.len()).map(|k| (k - 1, k)).collect();
            Some(("row", v, pairs))
        }
        Node::Grid { rows, .. } => {
            let mut v = Vec::new();
            let mut at = Vec::new();
            for (i, r) in rows.iter().enumerate() {
                for (j, c) in r.iter().enumerate() {
                    at.push((i, j));
                    v.push(p(&format!("cell{i}.{j}"), count(c, false)));
                }
            }
            let mut pairs = Vec::new();
            for a in 0..at.len() {
                for b in a + 1..at.len() {
                    let (p0, p1) = (at[a], at[b]);
                    if (p0.0 == p1.0 && p1.1 == p0.1 + 1) || (p0.1 == p1.1 && p1.0 == p0.0 + 1) {
                        pairs.push((a, b));
                    }
                }
            }
            Some(("grid", v, pairs))
        }
        _ => None,
    }
}

fn all_pairs(n: usize) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    for a in 0..n {
        for b in a + 1..n {
            v.push((a, b));
        }
    }
    v
}

#[allow(unreachable_patterns)]
fn children(node: &Node) -> Vec<&Node> {
    match node {
        Node::Glyph { .. } => vec![],
        Node::Row(v) => v.iter().collect(),
        Node::Scripts { base, sub, sup } => [Some(base.as_ref()), sub.as_deref(), sup.as_deref()]
            .into_iter()
            .flatten()
            .collect(),
        Node::Frac(a, b) => vec![a, b],
        Node::Sqrt(b) => vec![b],
        Node::Fenced { body, .. } => vec![body],
        Node::Grid { rows, .. } => rows.iter().flatten().collect(),
        Node::Over { above, below, .. } => [above.as_deref(), below.as_deref()]
            .into_iter()
            .flatten()
            .collect(),
        Node::Small(b) => vec![b],
        Node::Diagram { rows, edges, .. } => rows
            .iter()
            .flatten()
            .chain(edges.iter().flat_map(|e| e.labels.iter().map(|(n, _)| n)))
            .collect(),
        _ => vec![],
    }
}

pub fn check_node(node: &Node, display: bool, budget: &mut usize, out: &mut Vec<Collision>) {
    if *budget == 0 {
        return;
    }
    *budget -= 1;
    if let Some((kind, ps, pairs)) = parts(node, display) {
        let all = marks(&lay(node, display));
        let total: usize = ps.iter().map(|p| p.n).sum();
        if total == all.len() && !pairs.is_empty() {
            let mut at = 0;
            let mut cells = Vec::new();
            for p in &ps {
                cells.push(rasterize(&all[at..at + p.n]));
                at += p.n;
            }
            for (a, b) in pairs {
                let ov = cells[a].intersection(&cells[b]).count();
                if ov >= MIN_OVERLAP {
                    let (na, nb) = (strip(&ps[a].name), strip(&ps[b].name));
                    out.push(Collision {
                        what: format!("{kind} {na}/{nb}"),
                        overlap: ov,
                        display,
                    });
                }
            }
        }
    }
    for c in children(node) {
        check_node(c, display, budget, out);
    }
}

fn strip(s: &str) -> String {
    s.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.')
        .to_string()
}
