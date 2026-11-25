use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::OnceLock;

use crate::linebreak::{self, BItem, Kind};
use crate::render::escape;

const FONT_DATA: [&[u8]; 5] = [
    include_bytes!("../data/fonts/functor-lm-regular.otf"),
    include_bytes!("../data/fonts/functor-lm-italic.otf"),
    include_bytes!("../data/fonts/functor-lm-bold.otf"),
    include_bytes!("../data/fonts/functor-lm-bolditalic.otf"),
    include_bytes!("../data/fonts/functor-lm-mono.otf"),
];

pub const CODE_SCALE: f64 = 0.92;
const HYPHEN_PENALTY: f64 = 50.0;
const EX_HYPHEN_PENALTY: f64 = 50.0;
const EXTRA_SPACE: f64 = 0.111;
pub const EXPANSION: f64 = 0.02;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Style {
    pub italic: bool,
    pub bold: bool,
    pub mono: bool,
}

impl Style {
    fn font(self) -> usize {
        if self.mono {
            4
        } else {
            (self.italic as usize) + 2 * (self.bold as usize)
        }
    }

    fn scale(self) -> f64 {
        if self.mono {
            CODE_SCALE
        } else {
            1.0
        }
    }
}

struct Faces(Vec<rustybuzz::Face<'static>>);

unsafe impl Sync for Faces {}
unsafe impl Send for Faces {}

fn faces() -> &'static [rustybuzz::Face<'static>] {
    static F: OnceLock<Faces> = OnceLock::new();
    &F.get_or_init(|| Faces(FONT_DATA.iter().map(|d| rustybuzz::Face::from_slice(d, 0).expect("bundled font")).collect())).0
}

thread_local! {
    static CACHE: RefCell<HashMap<(usize, String), f64>> = RefCell::new(HashMap::new());
}

pub fn width(s: &str, style: Style) -> f64 {
    let font = style.font();
    let key = (font, s.to_string());
    if let Some(w) = CACHE.with(|c| c.borrow().get(&key).copied()) {
        return w * style.scale();
    }
    let face = &faces()[font];
    let mut buf = rustybuzz::UnicodeBuffer::new();
    buf.push_str(s);
    let out = rustybuzz::shape(face, &[], buf);
    let upem = face.units_per_em() as f64;
    let mut w = 0.0;
    for (info, pos) in out.glyph_infos().iter().zip(out.glyph_positions()) {
        w += if info.glyph_id == 0 { 0.5 * upem } else { pos.x_advance as f64 };
    }
    let w = w / upem;
    CACHE.with(|c| c.borrow_mut().insert(key, w));
    w * style.scale()
}

pub fn covered(s: &str, style: Style) -> bool {
    let face = &faces()[style.font()];
    s.chars().all(|c| c.is_whitespace() || face.glyph_index(c).is_some())
}

#[derive(Clone, Debug, PartialEq)]
pub struct Wrap {
    pub open: String,
    pub close: &'static str,
}

#[derive(Clone, Debug)]
enum Content {
    Text(String),
    Raw(String),
}

#[derive(Clone, Debug)]
enum Item {
    Box { w: f64, content: Content, style: Style, wraps: Rc<Vec<Wrap>> },
    Glue { w: f64, stretch: f64, shrink: f64, style: Style, wraps: Rc<Vec<Wrap>>, nbsp: bool },
    Penalty { w: f64, p: f64, flagged: bool, discretionary: bool, style: Style, wraps: Rc<Vec<Wrap>> },
}

pub struct Builder {
    items: Vec<Item>,
    styles: Vec<Style>,
    wraps: Vec<Rc<Vec<Wrap>>>,
    last: Option<char>,
    prev_last: Option<char>,
    pub hyphenate: bool,
}

impl Builder {
    pub fn new(base: Style) -> Self {
        Builder { items: Vec::new(), styles: vec![base], wraps: vec![Rc::new(Vec::new())], last: None, prev_last: None, hyphenate: true }
    }

    pub fn style(&self) -> Style {
        *self.styles.last().unwrap()
    }

    fn cur_wraps(&self) -> Rc<Vec<Wrap>> {
        self.wraps.last().unwrap().clone()
    }

    pub fn push(&mut self, style: Style, wrap: Option<Wrap>) {
        self.styles.push(style);
        let mut w = (**self.wraps.last().unwrap()).clone();
        if let Some(x) = wrap {
            w.push(x);
        }
        self.wraps.push(Rc::new(w));
    }

    pub fn pop(&mut self) {
        if self.styles.len() > 1 {
            self.styles.pop();
            self.wraps.pop();
        }
    }

    fn space_factor(&self) -> f64 {
        let mut last = self.last;
        let mut before = self.prev_last;
        if matches!(last, Some(')' | '\u{2019}' | '\u{201D}' | ']')) {
            last = before;
            before = None;
        }
        match last {
            Some('.' | '?' | '!') if !before.is_some_and(|c| c.is_uppercase()) => 3000.0,
            Some(':') => 2000.0,
            Some(';') => 1500.0,
            Some(',') => 1250.0,
            _ => 1000.0,
        }
    }

    fn glue(&mut self, nbsp: bool) {
        let style = self.style();
        let base = width(" ", style);
        let sf = if style.mono { 1000.0 } else { self.space_factor() };
        let w = if sf >= 2000.0 { base + EXTRA_SPACE } else { base };
        let (stretch, shrink) = if style.mono { (0.0, 0.0) } else { (base / 2.0 * sf / 1000.0, base / 3.0 * 1000.0 / sf) };
        if nbsp {
            self.penalty(0.0, linebreak::INF, false, false);
        }
        self.items.push(Item::Glue { w, stretch, shrink, style, wraps: self.cur_wraps(), nbsp });
    }

    pub fn space(&mut self) {
        if matches!(self.items.last(), None | Some(Item::Glue { .. })) {
            return;
        }
        self.glue(false);
    }

    fn penalty(&mut self, w: f64, p: f64, flagged: bool, discretionary: bool) {
        let style = self.style();
        self.items.push(Item::Penalty { w, p, flagged, discretionary, style, wraps: self.cur_wraps() });
    }

    fn boxed(&mut self, w: f64, s: &str) {
        let style = self.style();
        self.items.push(Item::Box { w, content: Content::Text(s.to_string()), style, wraps: self.cur_wraps() });
        let mut cs = s.chars().rev();
        let a = cs.next();
        let b = cs.next();
        if a.is_some() {
            self.prev_last = b.or(self.last);
            self.last = a;
        }
    }

    pub fn raw_box(&mut self, w: f64, html: String) {
        let style = self.style();
        self.items.push(Item::Box { w, content: Content::Raw(html), style, wraps: self.cur_wraps() });
        self.last = None;
        self.prev_last = None;
    }

    pub fn math_gap(&mut self, w: f64, stretch: f64, shrink: f64, penalty: f64) {
        self.penalty(0.0, penalty, false, false);
        let style = self.style();
        self.items.push(Item::Glue { w, stretch, shrink, style, wraps: self.cur_wraps(), nbsp: false });
    }

    pub fn unbreakable(&mut self, s: &str) {
        let w = width(s, self.style());
        self.boxed(w, s);
    }

    pub fn text(&mut self, s: &str) {
        let mut word = String::new();
        for c in s.chars() {
            match c {
                ' ' | '\n' | '\t' | '\r' => {
                    self.word(&std::mem::take(&mut word));
                    self.space();
                }
                '\u{A0}' => {
                    self.word(&std::mem::take(&mut word));
                    self.glue(true);
                }
                _ => word.push(c),
            }
        }
        self.word(&word);
    }

    fn word(&mut self, word: &str) {
        if word.is_empty() {
            return;
        }
        let style = self.style();
        let mut parts: Vec<&str> = Vec::new();
        let mut start = 0;
        for (i, c) in word.char_indices() {
            let end = i + c.len_utf8();
            if matches!(c, '-' | '\u{2013}' | '\u{2014}') && end < word.len() && i > 0 {
                parts.push(&word[start..end]);
                start = end;
            }
        }
        parts.push(&word[start..]);
        let n = parts.len();
        for (k, part) in parts.into_iter().enumerate() {
            if n == 1 && self.hyphenate && !style.mono {
                self.hyphenated(part);
            } else {
                self.unbreakable(part);
            }
            if k + 1 < n {
                self.penalty(0.0, EX_HYPHEN_PENALTY, true, false);
            }
        }
    }

    fn hyphenated(&mut self, word: &str) {
        let style = self.style();
        let lead = word.find(|c: char| c.is_alphabetic()).unwrap_or(word.len());
        let tail = word.rfind(|c: char| c.is_alphabetic()).map_or(lead, |i| i + word[i..].chars().next().unwrap().len_utf8());
        let core = &word[lead..tail.max(lead)];
        if core.chars().count() < 5 || !core.chars().all(char::is_alphabetic) {
            self.unbreakable(word);
            return;
        }
        let mut cuts: Vec<usize> = Vec::new();
        let mut pos = lead;
        let syl: Vec<&str> = hypher::hyphenate(core, hypher::Lang::English).collect();
        for s in &syl[..syl.len().saturating_sub(1)] {
            pos += s.len();
            cuts.push(pos);
        }
        if cuts.is_empty() {
            self.unbreakable(word);
            return;
        }
        let hyphen = width("-", style);
        let mut prev_end = 0;
        let mut prev_w = 0.0;
        for &cut in &cuts {
            let w = width(&word[..cut], style);
            self.boxed(w - prev_w, &word[prev_end..cut]);
            self.penalty(hyphen, HYPHEN_PENALTY, true, true);
            prev_end = cut;
            prev_w = w;
        }
        let w = width(word, style);
        self.boxed(w - prev_w, &word[prev_end..]);
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn finish(mut self, measure: f64, base: Style, protrude: bool, expand: bool) -> String {
        while matches!(self.items.last(), Some(Item::Glue { .. })) {
            self.items.pop();
        }
        let items = self.items;
        let mut b: Vec<BItem> = items
            .iter()
            .map(|it| match it {
                Item::Box { w, content, style, .. } => {
                    let x = if expand && expands(content, style) { w * EXPANSION } else { 0.0 };
                    BItem { kind: Kind::Box, w: *w, stretch: x, shrink: x, p: 0.0, flagged: false, disc: false }
                }
                Item::Glue { w, stretch, shrink, .. } => {
                    BItem { kind: Kind::Glue, w: *w, stretch: *stretch, shrink: *shrink, p: 0.0, flagged: false, disc: false }
                }
                Item::Penalty { w, p, flagged, discretionary, .. } => {
                    BItem { kind: Kind::Penalty, w: *w, stretch: 0.0, shrink: 0.0, p: *p, flagged: *flagged, disc: *discretionary }
                }
            })
            .collect();
        b.push(BItem { kind: Kind::Glue, w: 0.0, stretch: linebreak::FILL, shrink: 0.0, p: 0.0, flagged: false, disc: false });
        b.push(BItem { kind: Kind::Penalty, w: 0.0, stretch: 0.0, shrink: 0.0, p: -linebreak::INF, flagged: false, disc: false });
        let mut margins = vec![(0.0, 0.0); b.len()];
        if protrude {
            for (i, it) in items.iter().enumerate() {
                if let Item::Box { content: Content::Text(s), style, .. } = it {
                    let l = s.chars().next().map_or(0.0, |c| protrusion(c, style, true));
                    let r = s.chars().next_back().map_or(0.0, |c| protrusion(c, style, false));
                    margins[i] = (l, r);
                }
                if let Item::Penalty { discretionary: true, style, .. } = it {
                    margins[i] = (0.0, protrusion('-', style, false));
                }
            }
        }
        let lines = linebreak::break_lines(&b, measure, &margins);
        let base_space = width(" ", base);
        let mut out = String::from("<p class=\"fn-par\">");
        let mut start = 0;
        for line in &lines {
            let end = line.end;
            let mut s = start;
            while s < end && !matches!(b[s].kind, Kind::Box) {
                s += 1;
            }
            let (r, e) = split_ratio(line);
            let base_delta = adjust(base_space, base_space / 2.0, base_space / 3.0, r) / e - base_space;
            let (pl, _) = line.protrude;
            let mut style_attr = format!("word-spacing:{}em", num(base_delta));
            if pl.abs() > 1e-6 {
                style_attr.push_str(&format!(";margin-left:{}em", num(-pl)));
            }
            if (e - 1.0).abs() > 1e-5 {
                style_attr.push_str(&format!(";transform:scaleX({})", num(e)));
            }
            out.push_str(&format!("<span class=\"fn-line\" style=\"{style_attr}\">"));
            let mut open: Vec<Wrap> = Vec::new();
            for it in items.iter().take(end.min(items.len())).skip(s) {
                match it {
                    Item::Box { w, content, wraps, style } => {
                        sync(&mut out, &mut open, wraps);
                        let inner = match content {
                            Content::Text(t) => escape(t),
                            Content::Raw(h) => h.clone(),
                        };
                        if (e - 1.0).abs() > 1e-5 && !expands(content, style) {
                            out.push_str(&format!(
                                "<span class=\"fn-keep\" style=\"width:{}em;transform:scaleX({})\">{inner}</span>",
                                num(w / e),
                                num(1.0 / e)
                            ));
                        } else {
                            out.push_str(&inner);
                        }
                    }
                    Item::Glue { w, stretch, shrink, style, wraps, nbsp } => {
                        sync(&mut out, &mut open, wraps);
                        let ch = if *nbsp { "\u{A0}" } else { " " };
                        let target = adjust(*w, *stretch, *shrink, r) / e;
                        let delta = target - width(" ", *style);
                        if (delta - base_delta).abs() < 1e-4 && *style == base {
                            out.push_str(ch);
                        } else {
                            out.push_str(&format!("<span style=\"word-spacing:{}em\">{ch}</span>", num(delta)));
                        }
                    }
                    Item::Penalty { .. } => {}
                }
            }
            if let Some(Item::Penalty { discretionary: true, wraps, .. }) = items.get(end) {
                sync(&mut out, &mut open, wraps);
                out.push('-');
            }
            sync(&mut out, &mut open, &Rc::new(Vec::new()));
            out.push_str("</span>");
            start = end;
        }
        out.push_str("</p>");
        out
    }
}

fn expands(content: &Content, style: &Style) -> bool {
    matches!(content, Content::Text(_)) && !style.mono
}

fn split_ratio(line: &linebreak::Line) -> (f64, f64) {
    let r = line.ratio;
    let (xs, xh) = line.expand;
    let (gs, gh) = line.glue;
    if r >= 0.0 {
        if xs <= 0.0 {
            return (r, 1.0);
        }
        let re = r.min(1.0);
        let rest = r * (gs + xs) - re * xs;
        let rg = if gs > 0.0 { rest / gs } else { 0.0 };
        (rg, 1.0 + re * EXPANSION)
    } else if xh <= 0.0 || gh < 0.0 {
        (r, 1.0)
    } else {
        (r, 1.0 + r.max(-1.0) * EXPANSION)
    }
}

fn adjust(w: f64, stretch: f64, shrink: f64, r: f64) -> f64 {
    if r >= 0.0 {
        w + r * stretch
    } else {
        w + r * shrink
    }
}

fn sync(out: &mut String, open: &mut Vec<Wrap>, want: &[Wrap]) {
    let common = open.iter().zip(want.iter()).take_while(|(a, b)| a == b).count();
    while open.len() > common {
        let w = open.pop().unwrap();
        out.push_str(w.close);
    }
    for w in &want[common..] {
        out.push_str(&w.open);
        open.push(w.clone());
    }
}

pub fn protrusion(c: char, style: &Style, left: bool) -> f64 {
    if style.mono {
        return 0.0;
    }
    let f = if left {
        match c {
            '\u{201C}' | '\u{2018}' | '"' | '\'' => 0.5,
            '(' | '[' => 0.05,
            _ => 0.0,
        }
    } else {
        match c {
            '.' | ',' => 0.7,
            '-' => 0.7,
            '\u{2013}' => 0.3,
            '\u{2014}' => 0.2,
            ':' | ';' => 0.5,
            '!' | '?' => 0.2,
            '\u{201D}' | '\u{2019}' | '"' | '\'' => 0.5,
            ')' | ']' => 0.05,
            _ => 0.0,
        }
    };
    if f == 0.0 {
        return 0.0;
    }
    f * width(&c.to_string(), *style)
}

fn num(x: f64) -> String {
    let v = (x * 10000.0).round() / 10000.0;
    if v == 0.0 {
        "0".into()
    } else {
        format!("{v}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kerning_and_ligatures_change_width() {
        let s = Style::default();
        assert!(width("AV", s) < width("A", s) + width("V", s));
        assert!(width("fi", s) < width("f", s) + width("i", s) + 0.001);
        assert!((width(" ", s) - 0.333).abs() < 0.001);
    }

    #[test]
    fn breaks_a_paragraph_into_full_lines() {
        let mut b = Builder::new(Style::default());
        b.text("The total-fit line breaker looks at the whole paragraph at once. It prefers lines whose spaces stretch by the same amount, and it avoids hyphens on consecutive lines.");
        let html = b.finish(20.0, Style::default(), true, true);
        assert!(html.matches("fn-line").count() >= 3, "{html}");
        assert!(html.starts_with("<p class=\"fn-par\">"));
    }

    #[test]
    fn sentence_space_is_wider() {
        let mut b = Builder::new(Style::default());
        b.text("Done. Next");
        let Item::Glue { w, stretch, .. } = &b.items[1] else { panic!() };
        assert!((*w - (0.333 + EXTRA_SPACE)).abs() < 0.001);
        assert!(*stretch > 0.4);
        let mut c = Builder::new(Style::default());
        c.text("U.S. Next");
        let Item::Glue { w, .. } = &c.items[1] else { panic!() };
        assert!((*w - 0.333).abs() < 0.001);
    }

    #[test]
    fn long_words_get_hyphenation_points() {
        let mut b = Builder::new(Style::default());
        b.text("incomprehensibilities");
        let n = b.items.iter().filter(|i| matches!(i, Item::Penalty { discretionary: true, .. })).count();
        assert!(n >= 4, "{n}");
        let total: f64 = b.items.iter().map(|i| if let Item::Box { w, .. } = i { *w } else { 0.0 }).sum();
        assert!((total - width("incomprehensibilities", Style::default())).abs() < 1e-9);
    }
}
