use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use crate::glyph::{self, Affine, Glyph, Item, Shape};

const BINS: usize = 48;
const FLAT_STEPS: usize = 8;
pub const MAGNIFY_SHARE: f64 = 0.2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
}

type P = (f64, f64);

#[derive(Clone, Copy, Debug)]
enum Seg {
    Move(P),
    Line(P),
    Cubic(P, P, P),
    Close,
}

fn parse(path: &str, tf: Affine, fill: bool) -> Option<Vec<Seg>> {
    let mut out = Vec::new();
    let mut nums: Vec<f64> = Vec::new();
    let mut cmd = 'M';
    let mut cur = (0.0, 0.0);
    let mut start = (0.0, 0.0);
    let mut ok = true;
    let mut flush = |cmd: char, nums: &mut Vec<f64>, out: &mut Vec<Seg>| {
        let a = |x: f64, y: f64| tf.apply(x, y);
        match cmd {
            'M' => {
                for (k, c) in nums.chunks(2).enumerate() {
                    if c.len() == 2 {
                        cur = (c[0], c[1]);
                        if k == 0 {
                            start = cur;
                            out.push(Seg::Move(a(cur.0, cur.1)));
                        } else {
                            out.push(Seg::Line(a(cur.0, cur.1)));
                        }
                    }
                }
            }
            'L' => {
                for c in nums.chunks(2) {
                    if c.len() == 2 {
                        cur = (c[0], c[1]);
                        out.push(Seg::Line(a(cur.0, cur.1)));
                    }
                }
            }
            'H' => {
                for &x in nums.iter() {
                    cur = (x, cur.1);
                    out.push(Seg::Line(a(cur.0, cur.1)));
                }
            }
            'V' => {
                for &y in nums.iter() {
                    cur = (cur.0, y);
                    out.push(Seg::Line(a(cur.0, cur.1)));
                }
            }
            'C' => {
                for c in nums.chunks(6) {
                    if c.len() == 6 {
                        cur = (c[4], c[5]);
                        out.push(Seg::Cubic(a(c[0], c[1]), a(c[2], c[3]), a(c[4], c[5])));
                    }
                }
            }
            'Z' | 'z' => {
                cur = start;
                out.push(Seg::Close);
            }
            _ => ok = false,
        }
        nums.clear();
    };
    let bytes = path.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_ascii_alphabetic() {
            flush(cmd, &mut nums, &mut out);
            cmd = c;
            i += 1;
            if c == 'Z' || c == 'z' {
                flush(cmd, &mut nums, &mut out);
                cmd = ' ';
            }
        } else if c == '-' || c == '.' || c.is_ascii_digit() {
            let s = i;
            i += 1;
            let mut dot = c == '.';
            while i < bytes.len() {
                let d = bytes[i] as char;
                if d.is_ascii_digit() || (d == '.' && !dot) {
                    dot |= d == '.';
                    i += 1;
                } else if (d == 'e' || d == 'E') && i + 1 < bytes.len() {
                    i += 1;
                    if bytes[i] == b'-' || bytes[i] == b'+' {
                        i += 1;
                    }
                } else {
                    break;
                }
            }
            nums.push(path[s..i].parse().ok()?);
        } else {
            i += 1;
        }
    }
    if cmd != ' ' {
        flush(cmd, &mut nums, &mut out);
    }
    ok.then(|| if fill { closed(out) } else { out })
}

fn closed(path: Vec<Seg>) -> Vec<Seg> {
    let mut out = Vec::with_capacity(path.len() + 4);
    let mut open = false;
    for s in path {
        match s {
            Seg::Move(_) => {
                if open {
                    out.push(Seg::Close);
                }
                open = true;
                out.push(s);
            }
            Seg::Close => {
                open = false;
                out.push(s);
            }
            _ => out.push(s),
        }
    }
    if open {
        out.push(Seg::Close);
    }
    out
}

fn along(p: P, axis: Axis) -> f64 {
    if axis == Axis::Y {
        p.1
    } else {
        p.0
    }
}

fn across(p: P, axis: Axis) -> f64 {
    if axis == Axis::Y {
        p.0
    } else {
        p.1
    }
}

fn bez(p0: P, c1: P, c2: P, p1: P, t: f64) -> P {
    let m = 1.0 - t;
    let (a, b, c, d) = (m * m * m, 3.0 * m * m * t, 3.0 * m * t * t, t * t * t);
    (
        a * p0.0 + b * c1.0 + c * c2.0 + d * p1.0,
        a * p0.1 + b * c1.1 + c * c2.1 + d * p1.1,
    )
}

fn lerp(a: P, b: P, t: f64) -> P {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

fn split(p0: P, c1: P, c2: P, p1: P, t: f64) -> ([P; 4], [P; 4]) {
    let (a, b, c) = (lerp(p0, c1, t), lerp(c1, c2, t), lerp(c2, p1, t));
    let (d, e) = (lerp(a, b, t), lerp(b, c, t));
    let f = lerp(d, e, t);
    ([p0, a, d, f], [f, e, c, p1])
}

struct Profile {
    lo: f64,
    bw: f64,
    across: [f64; BINS],
    along: [f64; BINS],
    cross: [u32; BINS],
}

impl Profile {
    fn add(&mut self, a: P, b: P, axis: Axis) {
        let (ua, ub) = (along(a, axis), along(b, axis));
        let dv = (across(b, axis) - across(a, axis)).abs();
        let du = (ub - ua).abs();
        let (u0, u1) = (ua.min(ub), ua.max(ub));
        for (i, c) in self.cross.iter_mut().enumerate() {
            let mid = self.lo + (i as f64 + 0.5) * self.bw;
            if u0 <= mid && mid < u1 {
                *c += 1;
            }
        }
        let i0 = self.bin(u0);
        let i1 = self.bin(u1);
        if i0 == i1 || du < 1e-9 {
            self.across[i0] += dv;
            self.along[i0] += du;
            return;
        }
        for i in i0..=i1 {
            let b0 = (self.lo + i as f64 * self.bw).max(u0);
            let b1 = (self.lo + (i + 1) as f64 * self.bw).min(u1);
            let part = ((b1 - b0) / du).max(0.0);
            self.across[i] += dv * part;
            self.along[i] += du * part;
        }
    }

    fn bin(&self, u: f64) -> usize {
        (((u - self.lo) / self.bw).floor().max(0.0) as usize).min(BINS - 1)
    }
}

pub struct Map {
    knots: Vec<f64>,
    moved: Vec<f64>,
}

impl Map {
    pub fn apply(&self, u: f64) -> f64 {
        let n = self.knots.len();
        if u <= self.knots[0] {
            return u;
        }
        if u >= self.knots[n - 1] {
            return u + (self.moved[n - 1] - self.knots[n - 1]);
        }
        let i = self.seg(u);
        let (k0, k1) = (self.knots[i], self.knots[i + 1]);
        let (m0, m1) = (self.moved[i], self.moved[i + 1]);
        m0 + (u - k0) * (m1 - m0) / (k1 - k0)
    }

    fn seg(&self, u: f64) -> usize {
        match self.knots.binary_search_by(|k| k.partial_cmp(&u).unwrap()) {
            Ok(i) => i.min(self.knots.len() - 2),
            Err(i) => i.saturating_sub(1).min(self.knots.len() - 2),
        }
    }

    pub fn scales(&self) -> Vec<f64> {
        self.knots
            .windows(2)
            .zip(self.moved.windows(2))
            .map(|(k, m)| (m[1] - m[0]) / (k[1] - k[0]))
            .collect()
    }
}

fn plan(segs: &[Vec<Seg>], axis: Axis, lo: f64, hi: f64, extra: f64) -> Map {
    let bw = ((hi - lo) / BINS as f64).max(1e-6);
    let mut pr = Profile {
        lo,
        bw,
        across: [0.0; BINS],
        along: [0.0; BINS],
        cross: [0; BINS],
    };
    for path in segs {
        let mut cur = (0.0, 0.0);
        let mut start = (0.0, 0.0);
        for s in path {
            match *s {
                Seg::Move(p) => {
                    cur = p;
                    start = p;
                }
                Seg::Line(p) => {
                    pr.add(cur, p, axis);
                    cur = p;
                }
                Seg::Cubic(c1, c2, p) => {
                    let mut prev = cur;
                    for k in 1..=FLAT_STEPS {
                        let q = bez(cur, c1, c2, p, k as f64 / FLAT_STEPS as f64);
                        pr.add(prev, q, axis);
                        prev = q;
                    }
                    cur = p;
                }
                Seg::Close => {
                    pr.add(cur, start, axis);
                    cur = start;
                }
            }
        }
    }
    let flat: Vec<f64> = (0..BINS)
        .map(|i| pr.across[i] / (pr.along[i] + 0.15 * bw))
        .collect();
    let mut guard = flat.clone();
    for i in 0..BINS {
        let a = if i > 0 { flat[i - 1] } else { flat[i] };
        let b = if i + 1 < BINS { flat[i + 1] } else { flat[i] };
        guard[i] = flat[i].max(a.max(b) * 0.85);
    }
    let fewest = pr
        .cross
        .iter()
        .copied()
        .filter(|c| *c > 0)
        .min()
        .unwrap_or(2) as f64;
    let weight: Vec<f64> = (0..BINS)
        .map(|i| {
            let busy = (fewest / (pr.cross[i].max(1) as f64)).min(1.0).powi(4);
            let edge = ((i.min(BINS - 1 - i) as f64 + 0.5) / (BINS as f64 * 0.22)).min(1.0);
            busy * edge.powi(3) / (1.0 + 3.0 * guard[i]).powi(3)
        })
        .collect();
    let total: f64 = weight.iter().sum::<f64>().max(1e-9);
    let mut knots = Vec::with_capacity(BINS + 1);
    let mut moved = Vec::with_capacity(BINS + 1);
    let mut shift = 0.0;
    knots.push(lo);
    moved.push(lo);
    for (i, wt) in weight.iter().enumerate() {
        shift += extra * wt / total;
        knots.push(lo + (i + 1) as f64 * bw);
        moved.push(lo + (i + 1) as f64 * bw + shift);
    }
    Map { knots, moved }
}

fn map_point(p: P, axis: Axis, m: &Map) -> P {
    if axis == Axis::Y {
        (p.0, m.apply(p.1))
    } else {
        (m.apply(p.0), p.1)
    }
}

fn map_piece(p: [P; 4], axis: Axis, m: &Map) -> [P; 4] {
    let mid = along(bez(p[0], p[1], p[2], p[3], 0.5), axis);
    let i = m.seg(mid);
    let (k0, k1) = (m.knots[i], m.knots[i + 1]);
    let (m0, m1) = (m.moved[i], m.moved[i + 1]);
    let f = |u: f64| m0 + (u - k0) * (m1 - m0) / (k1 - k0);
    let f = |q: P| {
        if axis == Axis::Y {
            (q.0, f(q.1))
        } else {
            (f(q.0), q.1)
        }
    };
    [
        map_point(p[0], axis, m),
        f(p[1]),
        f(p[2]),
        map_point(p[3], axis, m),
    ]
}

fn crossings(p: [P; 4], axis: Axis, m: &Map) -> Vec<f64> {
    let us: Vec<f64> = p.iter().map(|q| along(*q, axis)).collect();
    let (lo, hi) = us
        .iter()
        .fold((f64::MAX, f64::MIN), |a, &u| (a.0.min(u), a.1.max(u)));
    let mut ts = Vec::new();
    for &k in &m.knots {
        if k <= lo + 1e-9 || k >= hi - 1e-9 {
            continue;
        }
        let g = |t: f64| along(bez(p[0], p[1], p[2], p[3], t), axis) - k;
        const N: usize = 24;
        let mut prev = g(0.0);
        for j in 1..=N {
            let t1 = j as f64 / N as f64;
            let cur = g(t1);
            if prev == 0.0 && j > 1 {
                ts.push((j - 1) as f64 / N as f64);
            } else if prev * cur < 0.0 {
                let (mut a, mut b) = ((j - 1) as f64 / N as f64, t1);
                let mut ga = prev;
                for _ in 0..40 {
                    let c = 0.5 * (a + b);
                    let gc = g(c);
                    if ga * gc <= 0.0 {
                        b = c;
                    } else {
                        a = c;
                        ga = gc;
                    }
                }
                ts.push(0.5 * (a + b));
            }
            prev = cur;
        }
    }
    ts.retain(|t| *t > 1e-6 && *t < 1.0 - 1e-6);
    ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ts.dedup_by(|a, b| (*a - *b).abs() < 1e-7);
    ts
}

struct Writer(String);

impl Writer {
    fn num(&mut self, v: f64) {
        let v = (v * 100.0).round() / 100.0;
        let s = if v == v.trunc() {
            format!("{}", v as i64)
        } else {
            format!("{v}")
        };
        if !s.starts_with('-')
            && self
                .0
                .chars()
                .last()
                .is_some_and(|c| c.is_ascii_digit() || c == '.')
        {
            self.0.push(' ');
        }
        self.0.push_str(&s);
    }

    fn pt(&mut self, c: char, p: P) {
        self.0.push(c);
        self.num(p.0);
        self.num(p.1);
    }
}

fn rebuild(path: &[Seg], axis: Axis, m: &Map) -> String {
    let mut w = Writer(String::new());
    let mut cur = (0.0, 0.0);
    let mut start = (0.0, 0.0);
    for s in path {
        match *s {
            Seg::Move(p) => {
                w.pt('M', map_point(p, axis, m));
                cur = p;
                start = p;
            }
            Seg::Line(p) => {
                line(&mut w, cur, p, axis, m);
                cur = p;
            }
            Seg::Cubic(c1, c2, p) => {
                let mut piece = [cur, c1, c2, p];
                let mut done = 0.0;
                for t in crossings(piece, axis, m) {
                    let local = (t - done) / (1.0 - done);
                    let (a, b) = split(piece[0], piece[1], piece[2], piece[3], local);
                    let q = map_piece(a, axis, m);
                    w.pt('C', q[1]);
                    w.0.push(' ');
                    w.num(q[2].0);
                    w.num(q[2].1);
                    w.0.push(' ');
                    w.num(q[3].0);
                    w.num(q[3].1);
                    piece = b;
                    done = t;
                }
                let q = map_piece(piece, axis, m);
                w.pt('C', q[1]);
                w.0.push(' ');
                w.num(q[2].0);
                w.num(q[2].1);
                w.0.push(' ');
                w.num(q[3].0);
                w.num(q[3].1);
                cur = p;
            }
            Seg::Close => {
                if (cur.0 - start.0).abs() > 1e-9 || (cur.1 - start.1).abs() > 1e-9 {
                    line(&mut w, cur, start, axis, m);
                }
                w.0.push('Z');
                cur = start;
            }
        }
    }
    w.0
}

fn line(w: &mut Writer, a: P, b: P, axis: Axis, m: &Map) {
    let (ua, ub) = (along(a, axis), along(b, axis));
    if (ub - ua).abs() > 1e-9 {
        let mut ks: Vec<f64> = m
            .knots
            .iter()
            .copied()
            .filter(|k| *k > ua.min(ub) && *k < ua.max(ub))
            .collect();
        if ub < ua {
            ks.reverse();
        }
        for k in ks {
            let t = (k - ua) / (ub - ua);
            w.pt('L', map_point(lerp(a, b, t), axis, m));
        }
    }
    w.pt('L', map_point(b, axis, m));
}

pub fn fingerprint(g: &Glyph) -> u64 {
    let mut h = DefaultHasher::new();
    for v in [g.w, g.h, g.d] {
        v.to_bits().hash(&mut h);
    }
    for it in &g.items {
        match &it.shape {
            Shape::Font(p) => {
                0u8.hash(&mut h);
                (p.as_ptr() as usize).hash(&mut h);
                p.len().hash(&mut h);
            }
            Shape::Stroke(p) => {
                1u8.hash(&mut h);
                p.hash(&mut h);
            }
            Shape::Outline(p) => {
                2u8.hash(&mut h);
                p.hash(&mut h);
            }
        }
        for v in it.tf.0 {
            v.to_bits().hash(&mut h);
        }
        it.bold.hash(&mut h);
    }
    h.finish()
}

pub fn deform(g: &Glyph, axis: Axis, extra: f64) -> Glyph {
    if extra <= 0.0 {
        return g.clone();
    }
    let (x0, y0, x1, y1) = g.ink_box();
    let (lo, hi) = if axis == Axis::Y { (y0, y1) } else { (x0, x1) };
    if hi - lo < 1.0 {
        return g.clone();
    }
    let parsed: Vec<Option<Vec<Seg>>> = g
        .items
        .iter()
        .map(|it| match &it.shape {
            Shape::Font(p) => parse(p, it.tf, true),
            Shape::Outline(p) => parse(p, it.tf, true),
            Shape::Stroke(p) => parse(p, it.tf, false),
        })
        .collect();
    let usable: Vec<Vec<Seg>> = parsed.iter().flatten().cloned().collect();
    let m = plan(&usable, axis, lo, hi, extra);
    let mut items = Vec::with_capacity(g.items.len());
    for (it, segs) in g.items.iter().zip(parsed) {
        match segs {
            Some(segs) => {
                let path = rebuild(&segs, axis, &m);
                let shape = match it.shape {
                    Shape::Stroke(_) => Shape::Stroke(path),
                    _ => Shape::Outline(Rc::from(path)),
                };
                items.push(Item {
                    shape,
                    tf: Affine::ID,
                    bold: it.bold,
                });
            }
            None => {
                let k = (hi - lo + extra) / (hi - lo);
                let tf = if axis == Axis::Y {
                    Affine([1.0, 0.0, 0.0, k, 0.0, lo * (1.0 - k)])
                } else {
                    Affine([k, 0.0, 0.0, 1.0, lo * (1.0 - k), 0.0])
                };
                items.push(Item {
                    shape: it.shape.clone(),
                    tf: it.tf.then(tf),
                    bold: it.bold,
                });
            }
        }
    }
    let (w, h, d) = if axis == Axis::Y {
        (g.w, m.apply(g.h), -m.apply(-g.d))
    } else {
        (m.apply(g.w), g.h, g.d)
    };
    Glyph {
        w,
        h,
        d,
        items,
        italic: g.italic,
    }
}

pub fn grown(g: &Rc<Glyph>, axis: Axis, factor: f64) -> Rc<Glyph> {
    let factor = (factor * 50.0).ceil() / 50.0;
    if factor <= 1.0 {
        return g.clone();
    }
    let key = format!(
        "deform:{:?}:{:016x}:{}",
        axis,
        fingerprint(g),
        (factor * 100.0).round() as i64
    );
    let size = if axis == Axis::Y { g.h + g.d } else { g.w };
    glyph::cached(&key, || Some(deform(g, axis, size * (factor - 1.0))))
        .unwrap_or_else(|| g.clone())
}

pub fn fence_split(sy: f64) -> (f64, f64) {
    if sy <= 1.0 {
        return (1.0, 1.0);
    }
    let m = sy.powf(MAGNIFY_SHARE);
    (m, sy / m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ink_h(g: &Glyph) -> f64 {
        let (_, y0, _, y1) = g.ink_box();
        y1 - y0
    }

    #[test]
    fn deform_adds_the_extra_height() {
        let p = Glyph::from_font(0x28, false).unwrap();
        let t = deform(&p, Axis::Y, 600.0);
        assert!((ink_h(&t) - ink_h(&p) - 600.0).abs() < 2.0);
        assert!((t.h + t.d - (p.h + p.d) - 600.0).abs() < 2.0);
        assert_eq!(t.w, p.w);
    }

    #[test]
    fn deform_keeps_the_ends_of_a_paren() {
        let p = Glyph::from_font(0x28, false).unwrap();
        let (_, lo, _, hi) = p.ink_box();
        let mut usable = Vec::new();
        for it in &p.items {
            if let Shape::Font(s) = &it.shape {
                usable.push(parse(s, it.tf, true).unwrap());
            }
        }
        let m = plan(&usable, Axis::Y, lo, hi, 1000.0);
        let sc = m.scales();
        let ends = (sc[0] + sc[BINS - 1]) / 2.0;
        let middle = sc[BINS / 2];
        assert!(middle > ends * 2.0, "middle {middle} ends {ends}");
    }

    #[test]
    fn deform_wide_makes_a_long_arrow() {
        let a = Glyph::from_font(0x2192, false).unwrap();
        let t = deform(&a, Axis::X, 800.0);
        assert!((t.w - a.w - 800.0).abs() < 2.0);
        assert!((ink_h(&t) - ink_h(&a)).abs() < 1.0);
    }

    #[test]
    fn grown_is_cached() {
        glyph::clear_cache();
        let p = Rc::new(Glyph::from_font(0x5B, false).unwrap());
        let a = grown(&p, Axis::Y, 2.5);
        let b = grown(&p, Axis::Y, 2.49);
        assert!(Rc::ptr_eq(&a, &b));
    }

    #[test]
    fn deform_closes_open_outlines_before_mapping() {
        let a = Glyph::from_font(0x27E9, false).unwrap();
        let t = deform(&a, Axis::Y, 1000.0);
        let Shape::Outline(p) = &t.items[0].shape else {
            panic!("no outline")
        };
        assert!(p.ends_with('Z'));
        let (x0, _, x1, _) = t.ink_box();
        let (a0, _, a1, _) = a.ink_box();
        assert!((x0 - a0).abs() < 1.0 && (x1 - a1).abs() < 1.0);
    }

    #[test]
    fn deform_leaves_open_strokes_open() {
        let x = Glyph::from_font(0x78, false).unwrap().strike("not");
        let t = deform(&x, Axis::Y, 400.0);
        let Shape::Stroke(p) = &t.items.last().unwrap().shape else {
            panic!("no stroke")
        };
        assert!(!p.contains('Z'));
    }

    #[test]
    fn parse_reads_font_paths() {
        let g = font_glyph_path(0x1D4B0);
        let segs = parse(g, Affine::ID, true).unwrap();
        assert!(segs.len() > 4);
    }

    fn font_glyph_path(cp: u32) -> &'static str {
        glyph::font_glyph(cp, false)
            .map(|g| g.path)
            .unwrap_or("M0 0L10 10")
    }
}
