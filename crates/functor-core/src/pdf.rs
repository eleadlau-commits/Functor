use std::collections::BTreeMap;
use std::f64::consts::PI;

use pdf_writer::types::{CidFontType, FontFlags, LineCapStyle, LineJoinStyle, SystemInfo, UnicodeCmap};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref, Str, TextStr};

use crate::glyph::{Affine, Shape, STROKE};
use crate::math::Paint;
use crate::page::{Draw, Layout, INK};
use crate::text::{self, Style};

const FONT_NAMES: [&str; 5] = ["LMRoman10-Regular", "LMRoman10-Italic", "LMRoman10-Bold", "LMRoman10-BoldItalic", "LMMono10-Regular"];

struct Fonts {
    used: BTreeMap<usize, BTreeMap<u16, String>>,
}

impl Fonts {
    fn note(&mut self, s: &str, style: Style) {
        let font = text::font_index(style);
        let map = self.used.entry(font).or_default();
        let clusters = text::shape_clusters(s, style);
        let mut starts: Vec<u32> = clusters.iter().map(|c| c.1).collect();
        starts.sort_unstable();
        starts.dedup();
        for (gid, c) in clusters {
            let next = starts.iter().find(|&&x| x > c).map_or(s.len(), |&x| x as usize);
            let chars = s.get(c as usize..next).unwrap_or("");
            map.entry(gid).or_insert_with(|| chars.to_string());
        }
    }
}

pub fn write(layout: &Layout) -> Vec<u8> {
    let mut fonts = Fonts { used: BTreeMap::new() };
    for p in &layout.pages {
        for d in &p.draws {
            if let Draw::Text { s, style, .. } = d {
                fonts.note(s, *style);
            }
        }
    }
    let mut pdf = Pdf::new();
    pdf.set_version(1, 7);
    let mut next = 1;
    let mut alloc = || {
        let r = Ref::new(next);
        next += 1;
        r
    };
    let catalog = alloc();
    let tree = alloc();
    let info = alloc();
    let font_refs: BTreeMap<usize, Ref> = fonts.used.keys().map(|&k| (k, alloc())).collect();
    let page_refs: Vec<(Ref, Ref)> = layout.pages.iter().map(|_| (alloc(), alloc())).collect();
    pdf.catalog(catalog).pages(tree);
    pdf.pages(tree).kids(page_refs.iter().map(|p| p.0)).count(page_refs.len() as i32);
    {
        let mut di = pdf.document_info(info);
        di.producer(TextStr("Functor"));
        if let Some(t) = &layout.title {
            di.title(TextStr(t));
        }
        if let Some(a) = &layout.author {
            di.author(TextStr(a));
        }
    }
    for (pi, page) in layout.pages.iter().enumerate() {
        let (pref, cref) = page_refs[pi];
        let mut p = pdf.page(pref);
        p.parent(tree).media_box(Rect::new(0.0, 0.0, layout.width as f32, layout.height as f32)).contents(cref);
        let mut res = p.resources();
        let mut fd = res.fonts();
        for (k, r) in &font_refs {
            fd.pair(Name(format!("F{k}").as_bytes()), *r);
        }
        fd.finish();
        res.finish();
        p.finish();
        let data = content(&page.draws);
        let packed = miniz_deflate(&data);
        match packed {
            Some(z) => {
                pdf.stream(cref, &z).filter(Filter::FlateDecode);
            }
            None => {
                pdf.stream(cref, &data);
            }
        }
    }
    for (font, map) in &fonts.used {
        let t0 = font_refs[font];
        let cid = alloc();
        let desc = alloc();
        let file = alloc();
        let cmap_ref = alloc();
        let name = Name(FONT_NAMES[*font].as_bytes());
        let sys = SystemInfo { registry: Str(b"Adobe"), ordering: Str(b"Identity"), supplement: 0 };
        pdf.type0_font(t0).base_font(name).encoding_predefined(Name(b"Identity-H")).descendant_font(cid).to_unicode(cmap_ref);
        {
            let mut c = pdf.cid_font(cid);
            c.subtype(CidFontType::Type0).base_font(name).system_info(sys).font_descriptor(desc).default_width(500.0);
            let n = text::glyph_count(*font);
            let ws: Vec<f32> = (0..n).map(|g| (text::glyph_advance(*font, g) * 1000.0) as f32).collect();
            c.widths().consecutive(0, ws);
        }
        let mut flags = FontFlags::NON_SYMBOLIC;
        if *font == 1 || *font == 3 {
            flags |= FontFlags::ITALIC;
        }
        if *font == 4 {
            flags |= FontFlags::FIXED_PITCH;
        } else {
            flags |= FontFlags::SERIF;
        }
        pdf.font_descriptor(desc)
            .name(name)
            .flags(flags)
            .bbox(Rect::new(-500.0, -300.0, 1500.0, 1000.0))
            .italic_angle(if *font == 1 || *font == 3 { -14.0 } else { 0.0 })
            .ascent(800.0)
            .descent(-200.0)
            .cap_height(683.0)
            .stem_v(if *font >= 2 && *font <= 3 { 120.0 } else { 80.0 })
            .font_file3(file);
        let bytes = text::font_data(Style { italic: *font == 1 || *font == 3, bold: *font == 2 || *font == 3, mono: *font == 4 });
        let packed = miniz_deflate(bytes);
        let mut st = pdf.stream(file, packed.as_deref().unwrap_or(bytes));
        if packed.is_some() {
            st.filter(Filter::FlateDecode);
        }
        st.pair(Name(b"Subtype"), Name(b"OpenType"));
        st.finish();
        let mut cm = UnicodeCmap::new(Name(b"Functor-UCS"), sys);
        for (gid, chars) in map {
            if !chars.is_empty() {
                cm.pair_with_multiple(*gid, chars.chars());
            }
        }
        let buf = cm.finish();
        pdf.cmap(cmap_ref, &buf);
    }
    pdf.finish()
}

fn miniz_deflate(data: &[u8]) -> Option<Vec<u8>> {
    let z = miniz_oxide::deflate::compress_to_vec_zlib(data, 6);
    (z.len() < data.len()).then_some(z)
}

fn content(draws: &[Draw]) -> Vec<u8> {
    let mut c = Content::new();
    for d in draws {
        match d {
            Draw::Rect { x, y, w, h, color } => {
                c.set_fill_rgb(color[0], color[1], color[2]);
                c.rect(*x as f32, *y as f32, *w as f32, *h as f32);
                c.fill_nonzero();
            }
            Draw::Qed { x, y, a } => {
                let t = 0.4f32;
                c.set_stroke_rgb(INK[0], INK[1], INK[2]);
                c.set_line_width(t);
                c.rect(*x as f32 + t / 2.0, *y as f32 + t / 2.0, *a as f32 - t, *a as f32 - t);
                c.stroke();
            }
            Draw::Text { x, y, s, style, size, scale, color } => {
                let glyphs = text::shape(s, *style);
                if glyphs.is_empty() {
                    continue;
                }
                let font = text::font_index(*style);
                c.set_fill_rgb(color[0], color[1], color[2]);
                c.begin_text();
                c.set_font(Name(format!("F{font}").as_bytes()), *size as f32);
                c.set_horizontal_scaling((*scale * 100.0) as f32);
                c.set_text_matrix([1.0, 0.0, 0.0, 1.0, *x as f32, *y as f32]);
                let mut pos = c.show_positioned();
                let mut items = pos.items();
                let mut run: Vec<u8> = Vec::new();
                for (gid, adv, _) in glyphs {
                    run.extend_from_slice(&gid.to_be_bytes());
                    let diff = (text::glyph_advance(font, gid) - adv) * 1000.0;
                    if diff.abs() > 0.01 {
                        items.show(Str(&run));
                        items.adjust(diff as f32);
                        run.clear();
                    }
                }
                if !run.is_empty() {
                    items.show(Str(&run));
                }
                items.finish();
                pos.finish();
                c.end_text();
            }
            Draw::Math { x, y, size, lbox, color } => {
                let k = *size / 1000.0;
                let base = Affine([k, 0.0, 0.0, k, *x, *y]);
                c.set_fill_rgb(color[0], color[1], color[2]);
                c.set_stroke_rgb(color[0], color[1], color[2]);
                lbox.paint(&mut |p| match p {
                    Paint::Rule(a, w, h) => {
                        c.save_state();
                        c.transform(mat(a.then(base)));
                        c.rect(0.0, 0.0, w as f32, h as f32);
                        c.fill_nonzero();
                        c.restore_state();
                    }
                    Paint::Glyph(a, g) => {
                        for it in &g.items {
                            c.save_state();
                            c.transform(mat(it.tf.then(a).then(base)));
                            match &it.shape {
                                Shape::Font(_) | Shape::Outline(_) => {
                                    let path: &str = match &it.shape {
                                        Shape::Font(p) => p,
                                        Shape::Outline(p) => p,
                                        _ => "",
                                    };
                                    svg_path(&mut c, path);
                                    if it.bold {
                                        c.set_line_width(30.0);
                                        c.set_line_join(LineJoinStyle::RoundJoin);
                                        c.fill_nonzero_and_stroke();
                                    } else {
                                        c.fill_nonzero();
                                    }
                                }
                                Shape::Stroke(path) => {
                                    svg_path(&mut c, path);
                                    c.set_line_width(STROKE as f32);
                                    c.set_line_cap(LineCapStyle::RoundCap);
                                    c.set_line_join(LineJoinStyle::RoundJoin);
                                    c.stroke();
                                }
                            }
                            c.restore_state();
                        }
                    }
                });
            }
        }
    }
    c.finish().into_vec()
}

fn mat(a: Affine) -> [f32; 6] {
    a.0.map(|v| v as f32)
}

fn numbers(s: &str) -> Vec<(char, Vec<f64>)> {
    let mut out: Vec<(char, Vec<f64>)> = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let ch = b[i] as char;
        if ch.is_ascii_alphabetic() && ch != 'e' && ch != 'E' {
            out.push((ch, Vec::new()));
            i += 1;
        } else if ch == '-' || ch == '+' || ch == '.' || ch.is_ascii_digit() {
            let start = i;
            i += 1;
            let mut dot = ch == '.';
            while i < b.len() {
                let d = b[i] as char;
                if d.is_ascii_digit() || (d == '.' && !dot) {
                    dot |= d == '.';
                    i += 1;
                } else if (d == 'e' || d == 'E') && i + 1 < b.len() {
                    i += 1;
                    if b[i] == b'-' || b[i] == b'+' {
                        i += 1;
                    }
                } else {
                    break;
                }
            }
            if let (Ok(v), Some(last)) = (s[start..i].parse::<f64>(), out.last_mut()) {
                last.1.push(v);
            }
        } else {
            i += 1;
        }
    }
    out
}

fn svg_path(c: &mut Content, p: &str) {
    let mut cur = (0.0f64, 0.0f64);
    let mut start = cur;
    let f = |v: f64| v as f32;
    for (cmd, n) in numbers(p) {
        let rel = cmd.is_ascii_lowercase();
        let off = |x: f64, y: f64, cur: (f64, f64)| if rel { (cur.0 + x, cur.1 + y) } else { (x, y) };
        match cmd.to_ascii_uppercase() {
            'M' => {
                for (k, ch) in n.chunks(2).enumerate() {
                    if ch.len() < 2 {
                        break;
                    }
                    cur = off(ch[0], ch[1], cur);
                    if k == 0 {
                        c.move_to(f(cur.0), f(cur.1));
                        start = cur;
                    } else {
                        c.line_to(f(cur.0), f(cur.1));
                    }
                }
            }
            'L' => {
                for ch in n.chunks(2) {
                    if ch.len() < 2 {
                        break;
                    }
                    cur = off(ch[0], ch[1], cur);
                    c.line_to(f(cur.0), f(cur.1));
                }
            }
            'H' => {
                for &x in &n {
                    cur = (if rel { cur.0 + x } else { x }, cur.1);
                    c.line_to(f(cur.0), f(cur.1));
                }
            }
            'V' => {
                for &y in &n {
                    cur = (cur.0, if rel { cur.1 + y } else { y });
                    c.line_to(f(cur.0), f(cur.1));
                }
            }
            'C' => {
                for ch in n.chunks(6) {
                    if ch.len() < 6 {
                        break;
                    }
                    let p1 = off(ch[0], ch[1], cur);
                    let p2 = off(ch[2], ch[3], cur);
                    let p3 = off(ch[4], ch[5], cur);
                    c.cubic_to(f(p1.0), f(p1.1), f(p2.0), f(p2.1), f(p3.0), f(p3.1));
                    cur = p3;
                }
            }
            'Q' => {
                for ch in n.chunks(4) {
                    if ch.len() < 4 {
                        break;
                    }
                    let q = off(ch[0], ch[1], cur);
                    let e = off(ch[2], ch[3], cur);
                    let p1 = (cur.0 + 2.0 / 3.0 * (q.0 - cur.0), cur.1 + 2.0 / 3.0 * (q.1 - cur.1));
                    let p2 = (e.0 + 2.0 / 3.0 * (q.0 - e.0), e.1 + 2.0 / 3.0 * (q.1 - e.1));
                    c.cubic_to(f(p1.0), f(p1.1), f(p2.0), f(p2.1), f(e.0), f(e.1));
                    cur = e;
                }
            }
            'A' => {
                for ch in n.chunks(7) {
                    if ch.len() < 7 {
                        break;
                    }
                    let e = off(ch[5], ch[6], cur);
                    for seg in arc(cur, ch[0], ch[1], ch[2], ch[3] != 0.0, ch[4] != 0.0, e) {
                        c.cubic_to(f(seg[0]), f(seg[1]), f(seg[2]), f(seg[3]), f(seg[4]), f(seg[5]));
                    }
                    cur = e;
                }
            }
            'Z' => {
                c.close_path();
                cur = start;
            }
            _ => {}
        }
    }
}

fn arc(p0: (f64, f64), rx: f64, ry: f64, rot: f64, large: bool, sweep: bool, p1: (f64, f64)) -> Vec<[f64; 6]> {
    if (p0.0 - p1.0).abs() < 1e-9 && (p0.1 - p1.1).abs() < 1e-9 {
        return Vec::new();
    }
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx < 1e-9 || ry < 1e-9 {
        return vec![[p0.0, p0.1, p1.0, p1.1, p1.0, p1.1]];
    }
    let phi = rot * PI / 180.0;
    let (sin, cos) = phi.sin_cos();
    let dx = (p0.0 - p1.0) / 2.0;
    let dy = (p0.1 - p1.1) / 2.0;
    let x1 = cos * dx + sin * dy;
    let y1 = -sin * dx + cos * dy;
    let l = x1 * x1 / (rx * rx) + y1 * y1 / (ry * ry);
    if l > 1.0 {
        rx *= l.sqrt();
        ry *= l.sqrt();
    }
    let num = (rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1).max(0.0);
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut k = (num / den).sqrt();
    if large == sweep {
        k = -k;
    }
    let cx1 = k * rx * y1 / ry;
    let cy1 = -k * ry * x1 / rx;
    let cx = cos * cx1 - sin * cy1 + (p0.0 + p1.0) / 2.0;
    let cy = sin * cx1 + cos * cy1 + (p0.1 + p1.1) / 2.0;
    let ang = |ux: f64, uy: f64, vx: f64, vy: f64| {
        let a = (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
        a
    };
    let t1 = ang(1.0, 0.0, (x1 - cx1) / rx, (y1 - cy1) / ry);
    let mut dt = ang((x1 - cx1) / rx, (y1 - cy1) / ry, (-x1 - cx1) / rx, (-y1 - cy1) / ry);
    if !sweep && dt > 0.0 {
        dt -= 2.0 * PI;
    } else if sweep && dt < 0.0 {
        dt += 2.0 * PI;
    }
    let n = (dt.abs() / (PI / 2.0)).ceil().max(1.0) as usize;
    let step = dt / n as f64;
    let alpha = 4.0 / 3.0 * (step / 4.0).tan();
    let pt = |t: f64| {
        let (s, c) = t.sin_cos();
        (cx + rx * c * cos - ry * s * sin, cy + rx * c * sin + ry * s * cos)
    };
    let der = |t: f64| {
        let (s, c) = t.sin_cos();
        (-rx * s * cos - ry * c * sin, -rx * s * sin + ry * c * cos)
    };
    let mut out = Vec::new();
    let mut t = t1;
    for _ in 0..n {
        let a = pt(t);
        let da = der(t);
        let b = pt(t + step);
        let db = der(t + step);
        out.push([a.0 + alpha * da.0, a.1 + alpha * da.1, b.0 - alpha * db.0, b.1 - alpha * db.1, b.0, b.1]);
        t += step;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arc_ends_at_target() {
        let segs = arc((0.0, 0.0), 10.0, 10.0, 0.0, true, false, (20.0, 0.0));
        let last = segs.last().unwrap();
        assert!((last[4] - 20.0).abs() < 1e-6 && last[5].abs() < 1e-6);
        assert_eq!(segs.len(), 2);
    }

    #[test]
    fn writes_a_pdf_with_fonts() {
        let out = crate::compile_pdf("---\nfunctor: 1\ntitle: T\n---\n\n# One\n\nSome text with $x^2 + 1 = y$ inside.\n");
        let s = String::from_utf8_lossy(&out.pdf);
        assert!(s.starts_with("%PDF-1.7"));
        assert!(s.contains("/FontFile3") && s.contains("/Identity-H") && s.contains("/ToUnicode"));
        assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
    }
}
