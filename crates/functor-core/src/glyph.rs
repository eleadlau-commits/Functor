use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::OnceLock;

const DATA: &str = include_str!("../data/glyphs.tsv");

pub const STROKE: f64 = 40.0;
pub const JOIN_OVERLAP: f64 = 45.0;
pub const AXIS: f64 = 250.0;

#[derive(Clone, Copy)]
pub struct FontGlyph {
    pub h: f64,
    pub d: f64,
    pub w: f64,
    pub path: &'static str,
}

fn table() -> &'static HashMap<(bool, u32), FontGlyph> {
    static TABLE: OnceLock<HashMap<(bool, u32), FontGlyph>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut map = HashMap::new();
        for line in DATA.lines() {
            let mut f = line.splitn(6, '\t');
            let (Some(v), Some(cp), Some(h), Some(d), Some(w)) = (f.next(), f.next(), f.next(), f.next(), f.next()) else {
                continue;
            };
            let path = f.next().unwrap_or("");
            let Ok(cp) = u32::from_str_radix(cp, 16) else { continue };
            let num = |s: &str| s.parse::<f64>().unwrap_or(0.0);
            map.insert((v == "L", cp), FontGlyph { h: num(h), d: num(d), w: num(w), path });
        }
        map
    })
}

pub fn font_glyph(cp: u32, large: bool) -> Option<FontGlyph> {
    let t = table();
    if large {
        if let Some(g) = t.get(&(true, cp)) {
            return Some(*g);
        }
    }
    t.get(&(false, cp)).copied()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine(pub [f64; 6]);

impl Affine {
    pub const ID: Affine = Affine([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    pub fn translate(x: f64, y: f64) -> Affine {
        Affine([1.0, 0.0, 0.0, 1.0, x, y])
    }

    pub fn scale(sx: f64, sy: f64) -> Affine {
        Affine([sx, 0.0, 0.0, sy, 0.0, 0.0])
    }

    pub fn then(self, next: Affine) -> Affine {
        let [a, b, c, d, e, f] = self.0;
        let [a2, b2, c2, d2, e2, f2] = next.0;
        Affine([
            a2 * a + c2 * b,
            b2 * a + d2 * b,
            a2 * c + c2 * d,
            b2 * c + d2 * d,
            a2 * e + c2 * f + e2,
            b2 * e + d2 * f + f2,
        ])
    }

    pub fn apply(self, x: f64, y: f64) -> (f64, f64) {
        let [a, b, c, d, e, f] = self.0;
        (a * x + c * y + e, b * x + d * y + f)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Font(&'static str),
    Stroke(String),
    Outline(Rc<str>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub shape: Shape,
    pub tf: Affine,
    pub bold: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Glyph {
    pub w: f64,
    pub h: f64,
    pub d: f64,
    pub items: Vec<Item>,
    pub italic: bool,
}

impl Glyph {
    pub fn from_font(cp: u32, large: bool) -> Option<Glyph> {
        let Some(g) = font_glyph(cp, large) else {
            return Glyph::built_fallback(cp);
        };
        Some(Glyph {
            w: g.w,
            h: g.h,
            d: g.d,
            items: vec![Item { shape: Shape::Font(g.path), tf: Affine::ID, bold: false }],
            italic: false,
        })
    }

    fn built_fallback(cp: u32) -> Option<Glyph> {
        match cp {
            0x2026 => {
                let dot = Glyph::from_font(0x2E, false)?;
                let gap = 1000.0 * 3.0 / 18.0;
                let step = dot.w + gap;
                let mut items = Vec::new();
                for i in 0..3 {
                    items.extend(dot.transformed(Affine::translate(step * i as f64, 0.0)));
                }
                Some(Glyph { w: dot.w * 3.0 + gap * 2.0, h: dot.h, d: dot.d, items, italic: false })
            }
            _ => None,
        }
    }

    pub fn empty(w: f64) -> Glyph {
        Glyph { w, h: 0.0, d: 0.0, items: Vec::new(), italic: false }
    }

    pub fn transformed(&self, tf: Affine) -> Vec<Item> {
        self.items.iter().map(|it| Item { shape: it.shape.clone(), tf: it.tf.then(tf), bold: it.bold }).collect()
    }

    pub fn ink_box(&self) -> (f64, f64, f64, f64) {
        let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for it in &self.items {
            let pts = match &it.shape {
                Shape::Font(p) => path_points(p),
                Shape::Stroke(p) => path_points(p),
                Shape::Outline(p) => path_points(p),
            };
            for (x, y) in pts {
                let (x, y) = it.tf.apply(x, y);
                b = (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y));
            }
        }
        if b.0 > b.2 {
            (0.0, -self.d, self.w, self.h)
        } else {
            b
        }
    }

    pub fn decorate(&self, mark_cp: u32, below: bool) -> Option<Glyph> {
        let mark = Glyph::from_font(mark_cp, false)?;
        let (mx0, my0, mx1, my1) = mark.ink_box();
        let (bx0, by0, bx1, by1) = self.ink_box();
        let skew = if self.italic { 0.12 * (by1 - by0.max(0.0)) } else { 0.0 };
        let center = (bx0 + bx1) / 2.0 + if below { 0.0 } else { skew };
        let mw = mx1 - mx0;
        let base_w = bx1 - bx0;
        let sx = if mw > base_w * 1.6 && base_w > 0.0 { (base_w * 1.4 / mw).max(0.6) } else { 1.0 };
        let dx = center - (mx0 + mx1) / 2.0 * sx;
        let gap = 60.0;
        let dy = if below { by0.min(0.0) - gap - my1 } else { by1.max(430.0) + gap - my0 };
        let tf = Affine::scale(sx, 1.0).then(Affine::translate(dx, dy));
        let mut items = self.items.clone();
        items.extend(mark.transformed(tf));
        let (x0, _, x1, _) = (mx0 * sx + dx, 0.0, mx1 * sx + dx, 0.0);
        let left = (-x0).max(0.0);
        let w = self.w.max(x1) + left;
        let shifted: Vec<Item> = items.into_iter().map(|it| Item { tf: it.tf.then(Affine::translate(left, 0.0)), ..it }).collect();
        let (h, d) = if below { (self.h, self.d.max(-(my0 + dy))) } else { (self.h.max(my1 + dy), self.d) };
        Some(Glyph { w, h, d, items: shifted, italic: self.italic })
    }

    pub fn enclose(&self, shape: &str) -> Glyph {
        let (x0, y0, x1, y1) = self.ink_box();
        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let pad = 90.0;
        let side = 50.0;
        let (path, half_w, half_h) = match shape {
            "circle" => {
                let r = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt() / 2.0 * 0.86 + pad * 0.5;
                let r = r.max((x1 - x0) / 2.0 + pad * 0.4).max((y1 - y0) / 2.0 + pad * 0.4);
                (circle_path(0.0, 0.0, r), r, r)
            }
            _ => {
                let mut hw = (x1 - x0) / 2.0 + pad;
                let mut hh = (y1 - y0) / 2.0 + pad;
                if shape == "square" {
                    hw = hw.max(hh);
                    hh = hw;
                }
                (format!("M{} {}L{} {}L{} {}L{} {}Z", -hw, -hh, hw, -hh, hw, hh, -hw, hh), hw, hh)
            }
        };
        let new_cx = half_w + side + STROKE / 2.0;
        let shift = Affine::translate(new_cx - cx, 0.0);
        let mut items = self.transformed(shift);
        items.push(Item { shape: Shape::Stroke(path), tf: Affine::translate(new_cx, cy), bold: false });
        Glyph {
            w: 2.0 * (half_w + side + STROKE / 2.0),
            h: self.h.max(cy + half_h + STROKE / 2.0),
            d: self.d.max(-(cy - half_h - STROKE / 2.0)),
            items,
            italic: false,
        }
    }

    pub fn strike(&self, kind: &str) -> Glyph {
        let (x0, y0, x1, y1) = self.ink_box();
        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let path = if kind == "cross" {
            let y = y0 + (y1 - y0) * 0.62;
            let half = ((x1 - x0) * 0.42).max(120.0);
            format!("M{} {}L{} {}", cx - half - 40.0, y, cx + half - 40.0, y)
        } else {
            let hh = ((y1 - y0) / 2.0 + 120.0).max(330.0);
            let hw = hh * 0.36;
            format!("M{} {}L{} {}", cx - hw, cy - hh, cx + hw, cy + hh)
        };
        let mut items = self.items.clone();
        items.push(Item { shape: Shape::Stroke(path), tf: Affine::ID, bold: false });
        let hh = ((y1 - y0) / 2.0 + 120.0).max(330.0);
        let (h, d) = if kind == "cross" { (self.h, self.d) } else { (self.h.max(cy + hh), self.d.max(-(cy - hh))) };
        Glyph { w: self.w, h, d, items, italic: self.italic }
    }

    pub fn repeat(&self, n: usize) -> Glyph {
        let (x0, _, x1, _) = self.ink_box();
        let step = ((x1 - x0) * 0.62).max(self.w * 0.45);
        let mut items = Vec::new();
        for i in 0..n {
            items.extend(self.transformed(Affine::translate(step * i as f64, 0.0)));
        }
        Glyph { w: self.w + step * (n as f64 - 1.0), h: self.h, d: self.d, items, italic: self.italic }
    }

    pub fn reflect(&self, kind: &str) -> Glyph {
        let (x0, y0, x1, y1) = self.ink_box();
        if kind == "rot" {
            let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
            let half_w = (y1 - y0) / 2.0;
            let side = x0.max(0.0);
            let nx = side + half_w;
            let tf = Affine([0.0, 1.0, -1.0, 0.0, nx + cy, cy - cx]);
            let ny0 = cy - (x1 - x0) / 2.0;
            let ny1 = cy + (x1 - x0) / 2.0;
            return Glyph { w: nx + half_w + side, h: ny1.max(0.0), d: (-ny0).max(0.0), items: self.transformed(tf), italic: false };
        }
        let tf = match kind {
            "flip" => Affine([-1.0, 0.0, 0.0, 1.0, x0 + x1, 0.0]),
            "flipv" => Affine([1.0, 0.0, 0.0, -1.0, 0.0, y0 + y1]),
            _ => Affine([-1.0, 0.0, 0.0, -1.0, x0 + x1, y0 + y1]),
        };
        Glyph { w: self.w, h: self.h, d: self.d, items: self.transformed(tf), italic: false }
    }

    pub fn join(&self, other: &Glyph) -> Glyph {
        let (_, _, ax1, _) = self.ink_box();
        let (bx0, _, bx1, _) = other.ink_box();
        let dx = ax1 - JOIN_OVERLAP - bx0;
        let mut items = self.items.clone();
        items.extend(other.transformed(Affine::translate(dx, 0.0)));
        Glyph {
            w: self.w.max(dx + other.w).max(dx + bx1),
            h: self.h.max(other.h),
            d: self.d.max(other.d),
            items,
            italic: other.italic,
        }
    }

    pub fn stack(&self, below: &Glyph) -> Glyph {
        let (ax0, ay0, ax1, ay1) = self.ink_box();
        let (bx0, by0, bx1, by1) = below.ink_box();
        let width = (ax1 - ax0).max(bx1 - bx0);
        let side = 40.0;
        let mid = side + width / 2.0;
        let gap = 60.0;
        let total = (ay1 - ay0) + gap + (by1 - by0);
        let bottom = AXIS - total / 2.0;
        let b_dy = bottom - by0;
        let a_dy = bottom + (by1 - by0) + gap - ay0;
        let mut items = self.transformed(Affine::translate(mid - (ax0 + ax1) / 2.0, a_dy));
        items.extend(below.transformed(Affine::translate(mid - (bx0 + bx1) / 2.0, b_dy)));
        let top = bottom + total;
        Glyph { w: width + 2.0 * side, h: top.max(0.0), d: (-bottom).max(0.0), items, italic: false }
    }

    pub fn magnified(&self, s: f64) -> Glyph {
        Glyph { w: self.w * s, h: self.h * s, d: self.d * s, items: self.transformed(Affine::scale(s, s)), italic: self.italic }
    }

    pub fn emboldened(&self) -> Glyph {
        let items = self.items.iter().map(|it| Item { bold: true, ..it.clone() }).collect();
        Glyph { w: self.w + 30.0, items, ..self.clone() }
    }

    pub fn to_svg_group(&self, out: &mut String, tf: Affine) {
        for it in &self.items {
            let m = it.tf.then(tf).0;
            let matrix = format!("matrix({} {} {} {} {} {})", r(m[0]), r(m[1]), r(m[2]), r(m[3]), r(m[4]), r(m[5]));
            let fill = match &it.shape {
                Shape::Font(p) => Some(*p),
                Shape::Outline(p) => Some(&**p),
                Shape::Stroke(_) => None,
            };
            match (&it.shape, fill) {
                (_, Some(p)) => {
                    if it.bold {
                        out.push_str(&format!(
                            "<path transform=\"{matrix}\" d=\"{p}\" stroke=\"currentColor\" stroke-width=\"30\" stroke-linejoin=\"round\"/>"
                        ));
                    } else {
                        out.push_str(&format!("<path transform=\"{matrix}\" d=\"{p}\"/>"));
                    }
                }
                (Shape::Stroke(p), _) => out.push_str(&format!(
                    "<path transform=\"{matrix}\" d=\"{p}\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"{STROKE}\" stroke-linecap=\"round\"/>"
                )),
                _ => {}
            }
        }
    }

    pub fn to_svg(&self, height_em: f64) -> String {
        let pad = 20.0;
        let total = self.h + self.d + 2.0 * pad;
        let mut body = String::new();
        self.to_svg_group(&mut body, Affine([1.0, 0.0, 0.0, -1.0, pad, self.h + pad]));
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" height=\"{}em\" fill=\"currentColor\">{}</svg>",
            r(self.w + 2.0 * pad),
            r(total),
            r(height_em * total / 1000.0),
            body
        )
    }
}

fn r(x: f64) -> String {
    let v = (x * 100.0).round() / 100.0;
    if v == v.trunc() {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

fn circle_path(cx: f64, cy: f64, r: f64) -> String {
    format!("M{} {}A{r} {r} 0 1 0 {} {}A{r} {r} 0 1 0 {} {}Z", cx - r, cy, cx + r, cy, cx - r, cy)
}

pub fn path_points(p: &str) -> Vec<(f64, f64)> {
    let mut pts = Vec::new();
    let mut nums: Vec<f64> = Vec::new();
    let mut cmd = 'M';
    let mut cur = (0.0, 0.0);
    let flush = |cmd: char, nums: &mut Vec<f64>, pts: &mut Vec<(f64, f64)>, cur: &mut (f64, f64)| {
        match cmd {
            'H' => {
                for &x in nums.iter() {
                    *cur = (x, cur.1);
                    pts.push(*cur);
                }
            }
            'V' => {
                for &y in nums.iter() {
                    *cur = (cur.0, y);
                    pts.push(*cur);
                }
            }
            'A' => {
                for c in nums.chunks(7) {
                    if c.len() == 7 {
                        let r = c[0];
                        pts.push((c[5] - r, c[6] - r));
                        pts.push((c[5] + r, c[6] + r));
                        *cur = (c[5], c[6]);
                        pts.push(*cur);
                    }
                }
            }
            _ => {
                for c in nums.chunks(2) {
                    if c.len() == 2 {
                        *cur = (c[0], c[1]);
                        pts.push(*cur);
                    }
                }
            }
        }
        nums.clear();
    };
    let bytes = p.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_ascii_alphabetic() {
            flush(cmd, &mut nums, &mut pts, &mut cur);
            cmd = c;
            i += 1;
        } else if c == '-' || c == '.' || c.is_ascii_digit() {
            let start = i;
            i += 1;
            let mut seen_dot = c == '.';
            while i < bytes.len() {
                let d = bytes[i] as char;
                if d.is_ascii_digit() || (d == '.' && !seen_dot) {
                    seen_dot |= d == '.';
                    i += 1;
                } else {
                    break;
                }
            }
            if let Ok(v) = p[start..i].parse() {
                nums.push(v);
            }
        } else {
            i += 1;
        }
    }
    flush(cmd, &mut nums, &mut pts, &mut cur);
    pts
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheStats {
    pub built: usize,
    pub hits: usize,
    pub from_font: usize,
}

pub type Notes = Vec<(usize, String)>;

struct Entry {
    glyph: Rc<Glyph>,
    notes: Rc<Notes>,
}

thread_local! {
    static CACHE: RefCell<HashMap<String, Entry>> = RefCell::new(HashMap::new());
    static STATS: RefCell<CacheStats> = RefCell::new(CacheStats::default());
    static DIRTY: RefCell<bool> = const { RefCell::new(false) };
}

pub fn cached(recipe: &str, build: impl FnOnce() -> Option<Glyph>) -> Option<Rc<Glyph>> {
    cached_with_notes(recipe, || build().map(|g| (g, Vec::new()))).map(|(g, _)| g)
}

pub fn cached_with_notes(recipe: &str, build: impl FnOnce() -> Option<(Glyph, Notes)>) -> Option<(Rc<Glyph>, Rc<Notes>)> {
    if let Some(hit) = CACHE.with(|c| c.borrow().get(recipe).map(|e| (e.glyph.clone(), e.notes.clone()))) {
        STATS.with(|s| s.borrow_mut().hits += 1);
        return Some(hit);
    }
    let (g, notes) = build()?;
    let entry = Entry { glyph: Rc::new(g), notes: Rc::new(notes) };
    let out = (entry.glyph.clone(), entry.notes.clone());
    STATS.with(|s| s.borrow_mut().built += 1);
    CACHE.with(|c| c.borrow_mut().insert(recipe.to_string(), entry));
    DIRTY.with(|d| *d.borrow_mut() = true);
    Some(out)
}

pub fn note_font_glyph() {
    STATS.with(|s| s.borrow_mut().from_font += 1);
}

pub fn take_stats() -> CacheStats {
    STATS.with(|s| std::mem::take(&mut *s.borrow_mut()))
}

pub fn clear_cache() {
    CACHE.with(|c| c.borrow_mut().clear());
    DIRTY.with(|d| *d.borrow_mut() = false);
}

pub fn cache_len() -> usize {
    CACHE.with(|c| c.borrow().len())
}

pub fn take_dirty() -> bool {
    DIRTY.with(|d| std::mem::take(&mut *d.borrow_mut()))
}

pub fn cache_key() -> &'static str {
    env!("FUNCTOR_GLYPH_KEY")
}

const MAGIC: &str = "functor-glyphs 1";

fn font_ids() -> &'static HashMap<usize, (bool, u32)> {
    static IDS: OnceLock<HashMap<usize, (bool, u32)>> = OnceLock::new();
    IDS.get_or_init(|| table().iter().map(|(k, g)| (g.path.as_ptr() as usize, *k)).collect())
}

fn tf_text(tf: Affine) -> String {
    tf.0.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" ")
}

fn clean(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

fn write_entry(out: &mut String, recipe: &str, e: &Entry) -> Option<()> {
    let g = &e.glyph;
    let mut body = format!("R\t{}\t{}\t{}\t{}\t{}\n", clean(recipe), g.w, g.h, g.d, g.italic as u8);
    for (i, msg) in e.notes.iter() {
        body.push_str(&format!("N\t{i}\t{}\n", clean(msg)));
    }
    for it in &g.items {
        let bold = it.bold as u8;
        match &it.shape {
            Shape::Font(p) => {
                let (large, cp) = font_ids().get(&(p.as_ptr() as usize))?;
                body.push_str(&format!("F\t{}\t{cp:x}\t{}\t{bold}\n", if *large { 'L' } else { 'S' }, tf_text(it.tf)));
            }
            Shape::Stroke(p) => body.push_str(&format!("S\t{}\t{bold}\t{}\n", tf_text(it.tf), clean(p))),
            Shape::Outline(p) => body.push_str(&format!("O\t{}\t{bold}\t{}\n", tf_text(it.tf), clean(p))),
        }
    }
    out.push_str(&body);
    Some(())
}

pub fn export_cache() -> String {
    let mut out = format!("{MAGIC} {}\n", cache_key());
    CACHE.with(|c| {
        let c = c.borrow();
        let mut keys: Vec<&String> = c.keys().collect();
        keys.sort();
        for k in keys {
            write_entry(&mut out, k, &c[k]);
        }
    });
    out
}

fn parse_tf(s: &str) -> Option<Affine> {
    let v: Vec<f64> = s.split(' ').map(|x| x.parse().ok()).collect::<Option<_>>()?;
    Some(Affine(v.try_into().ok()?))
}

fn parse_cache(text: &str) -> Option<Vec<(String, Entry)>> {
    let mut lines = text.lines();
    if lines.next()? != format!("{MAGIC} {}", cache_key()) {
        return None;
    }
    let mut out: Vec<(String, Glyph, Notes)> = Vec::new();
    for line in lines {
        let f: Vec<&str> = line.split('\t').collect();
        match (f[0], out.last_mut()) {
            ("R", _) if f.len() == 6 => {
                let n = |s: &str| s.parse::<f64>().ok();
                let mut g = Glyph::empty(n(f[2])?);
                g.h = n(f[3])?;
                g.d = n(f[4])?;
                g.italic = f[5] == "1";
                out.push((f[1].to_string(), g, Vec::new()));
            }
            ("N", Some(last)) if f.len() == 3 => last.2.push((f[1].parse().ok()?, f[2].to_string())),
            ("F", Some(last)) if f.len() == 5 => {
                let cp = u32::from_str_radix(f[2], 16).ok()?;
                let path = table().get(&(f[1] == "L", cp))?.path;
                last.1.items.push(Item { shape: Shape::Font(path), tf: parse_tf(f[3])?, bold: f[4] == "1" });
            }
            ("S", Some(last)) if f.len() == 4 => {
                last.1.items.push(Item { shape: Shape::Stroke(f[3].to_string()), tf: parse_tf(f[1])?, bold: f[2] == "1" });
            }
            ("O", Some(last)) if f.len() == 4 => {
                last.1.items.push(Item { shape: Shape::Outline(Rc::from(f[3])), tf: parse_tf(f[1])?, bold: f[2] == "1" });
            }
            _ => return None,
        }
    }
    Some(out.into_iter().map(|(k, g, n)| (k, Entry { glyph: Rc::new(g), notes: Rc::new(n) })).collect())
}

pub fn import_cache(text: &str) -> usize {
    let Some(entries) = parse_cache(text) else { return 0 };
    let n = entries.len();
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        for (k, e) in entries {
            c.entry(k).or_insert(e);
        }
    });
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_loads_and_has_sum() {
        let g = font_glyph(0x2211, true).expect("large sum");
        assert!(g.w > 1000.0);
        assert!(font_glyph(0x1D44E, false).is_some());
    }

    #[test]
    fn enclose_grows_the_box() {
        let sum = Glyph::from_font(0x2211, false).unwrap();
        let c = sum.enclose("circle");
        assert!(c.w > sum.w && c.h > sum.h);
        assert_eq!(c.items.len(), 2);
    }

    #[test]
    fn path_points_reads_implicit_negatives() {
        let pts = path_points("M10-20L30 40H5V-7");
        assert_eq!(pts, vec![(10.0, -20.0), (30.0, 40.0), (5.0, 40.0), (5.0, -7.0)]);
    }

    #[test]
    fn join_overlaps_by_the_join_amount() {
        let o = Glyph::from_font(0x6F, false).unwrap();
        let r = Glyph::from_font(0x72, false).unwrap();
        let j = o.join(&r);
        let (ox0, _, ox1, _) = o.ink_box();
        let (rx0, _, rx1, _) = r.ink_box();
        let (jx0, _, jx1, _) = j.ink_box();
        assert!(((jx1 - jx0) - ((ox1 - ox0) + (rx1 - rx0) - JOIN_OVERLAP)).abs() < 1e-6);
        assert!(j.w >= jx1);
    }
}
