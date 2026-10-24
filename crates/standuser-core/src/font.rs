//! Text as outlines: glyph paths from the embedded Noto Sans JP / Noto Serif JP Bold subsets, so the SVG renders the
//! same everywhere with no font files. Characters missing from the subset are drawn as □.

use ttf_parser::{Face, OutlineBuilder};

static SANS: &[u8] = include_bytes!("../data/sans.ttf");
static SERIF: &[u8] = include_bytes!("../data/serif.ttf");

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FontKind {
    Sans,
    Serif,
}

pub struct Font {
    face: Face<'static>,
    upm: f64,
}

pub fn font(kind: FontKind) -> Font {
    let face = Face::parse(if kind == FontKind::Sans { SANS } else { SERIF }, 0).expect("embedded font");
    let upm = face.units_per_em() as f64;
    Font { face, upm }
}

fn num(v: f64) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() { "0".into() } else { s.to_string() }
}

struct Pen<'a> {
    out: &'a mut String,
    sc: f64,
    ox: f64,
    oy: f64,
}

impl Pen<'_> {
    fn p(&self, x: f32, y: f32) -> String {
        format!("{} {}", num(self.ox + x as f64 * self.sc), num(self.oy - y as f64 * self.sc))
    }
}

impl OutlineBuilder for Pen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let s = format!("M{}", self.p(x, y));
        self.out.push_str(&s);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let s = format!("L{}", self.p(x, y));
        self.out.push_str(&s);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let s = format!("Q{} {}", self.p(x1, y1), self.p(x, y));
        self.out.push_str(&s);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let s = format!("C{} {} {}", self.p(x1, y1), self.p(x2, y2), self.p(x, y));
        self.out.push_str(&s);
    }
    fn close(&mut self) {
        self.out.push('Z');
    }
}

impl Font {
    fn glyph(&self, c: char) -> Option<ttf_parser::GlyphId> {
        self.face.glyph_index(c).or_else(|| self.face.glyph_index('□'))
    }

    pub fn has(&self, c: char) -> bool {
        self.face.glyph_index(c).is_some()
    }

    pub fn adv(&self, c: char, size: f64) -> f64 {
        match self.glyph(c).and_then(|g| self.face.glyph_hor_advance(g)) {
            Some(a) => a as f64 * size / self.upm,
            None => size * 0.5,
        }
    }

    pub fn width(&self, s: &str, size: f64, ls: f64) -> f64 {
        let n = s.chars().count();
        if n == 0 {
            return 0.0;
        }
        s.chars().map(|c| self.adv(c, size) + ls).sum::<f64>() - ls
    }

    /// SVG path data for `s` with its baseline starting at (x, y).
    pub fn path(&self, s: &str, x: f64, y: f64, size: f64, ls: f64) -> String {
        let sc = size / self.upm;
        let mut out = String::new();
        let mut cx = x;
        for c in s.chars() {
            if let Some(g) = self.glyph(c) {
                let mut pen = Pen { out: &mut out, sc, ox: cx, oy: y };
                let _ = self.face.outline_glyph(g, &mut pen);
            }
            cx += self.adv(c, size) + ls;
        }
        out
    }

    /// Greedy wrap: break on spaces for Latin, anywhere for CJK.
    pub fn wrap(&self, text: &str, size: f64, maxw: f64) -> Vec<String> {
        let mut toks: Vec<String> = Vec::new();
        let mut cur = String::new();
        for ch in text.chars() {
            if (ch as u32) > 0x2E80 {
                if !cur.is_empty() {
                    toks.push(std::mem::take(&mut cur));
                }
                toks.push(ch.to_string());
            } else if ch == ' ' {
                if !cur.is_empty() {
                    toks.push(std::mem::take(&mut cur));
                }
                toks.push(" ".into());
            } else {
                cur.push(ch);
            }
        }
        if !cur.is_empty() {
            toks.push(cur);
        }
        let mut lines = Vec::new();
        let mut line = String::new();
        for t in toks {
            let cand = format!("{line}{t}");
            if self.width(cand.trim_end(), size, 0.0) <= maxw {
                line = cand;
            } else {
                if !line.trim().is_empty() {
                    lines.push(line.trim_end().to_string());
                }
                line = t.trim_start().to_string();
            }
        }
        if !line.trim().is_empty() {
            lines.push(line.trim_end().to_string());
        }
        lines
    }

    /// Cut `s` to fit `maxw`, ending with an ellipsis.
    pub fn fit(&self, s: &str, size: f64, maxw: f64) -> String {
        if self.width(s, size, 0.0) <= maxw {
            return s.to_string();
        }
        let mut t: Vec<char> = s.chars().collect();
        while !t.is_empty() {
            t.pop();
            let cand: String = t.iter().collect::<String>().trim_end().to_string() + "…";
            if self.width(&cand, size, 0.0) <= maxw {
                return cand;
            }
        }
        "…".into()
    }
}
