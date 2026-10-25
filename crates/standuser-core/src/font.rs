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

    /// Fit a film line "Title: Subtitle (Country)" to `maxw` without cutting a word in half. Order: the line as it is; the part after
    /// the title's ":" dropped; the title cut at a word boundary with "…" (the trailing "(Country)" kept); the whole line cut at a
    /// word boundary with "…"; and only for a single word wider than the box, `fit`.
    pub fn fit_title(&self, s: &str, size: f64, maxw: f64) -> String {
        let fits = |t: &str| self.width(t, size, 0.0) <= maxw;
        if fits(s) {
            return s.to_string();
        }
        let (title, suffix) = match s.rfind(" (").filter(|_| s.ends_with(')')) {
            Some(i) => (&s[..i], &s[i..]),
            None => (s, ""),
        };
        let title = title.split(':').next().unwrap_or(title).trim_end();
        let line = format!("{title}{suffix}");
        if !title.is_empty() && fits(&line) {
            return line;
        }
        let cut = |text: &str, tail: &str| -> Option<String> {
            let words: Vec<&str> = text.split(' ').filter(|w| !w.is_empty()).collect();
            (1..words.len()).rev().map(|n| format!("{}…{tail}", words[..n].join(" "))).find(|c| fits(c))
        };
        cut(title, suffix).or_else(|| cut(&line, "")).unwrap_or_else(|| self.fit(&line, size, maxw))
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

#[cfg(test)]
mod title_tests {
    use super::*;

    fn serif() -> Font {
        font(FontKind::Serif)
    }

    #[test]
    fn a_title_that_fits_is_left_alone() {
        let f = serif();
        assert_eq!(f.fit_title("Ghost Rider: Spirit of Vengeance (Denmark)", 18.0, 2000.0), "Ghost Rider: Spirit of Vengeance (Denmark)");
    }

    #[test]
    fn the_subtitle_after_the_colon_goes_first_and_the_country_stays() {
        let f = serif();
        let long = "Ghost Rider: Spirit of Vengeance (United Arab Emirates)";
        let short = "Ghost Rider (United Arab Emirates)";
        let w = f.width(short, 18.0, 0.0) + 1.0;
        assert!(f.width(long, 18.0, 0.0) > w);
        assert_eq!(f.fit_title(long, 18.0, w), short);
    }

    #[test]
    fn if_still_too_long_the_title_is_cut_at_a_word_boundary_with_an_ellipsis() {
        let f = serif();
        let long = "Ghost Rider: Spirit of Vengeance (United Arab Emirates)";
        let w = f.width("Ghost… (United Arab Emirates)", 18.0, 0.0) + 1.0;
        assert_eq!(f.fit_title(long, 18.0, w), "Ghost… (United Arab Emirates)");
        let t = "The Lord of the Rings the Fellowship of the Ring (Denmark)";
        let w = f.width("The Lord of the Rings… (Denmark)", 18.0, 0.0) + 1.0;
        assert_eq!(f.fit_title(t, 18.0, w), "The Lord of the Rings… (Denmark)");
    }

    #[test]
    fn never_cuts_inside_a_word_at_any_width() {
        let f = serif();
        let src = "Ghost Rider: Spirit of Vengeance (United Arab Emirates)";
        let words: Vec<&str> = src.split([' ', ':']).map(|w| w.trim_matches(|c| c == '(' || c == ')')).filter(|w| !w.is_empty()).collect();
        for w in (120..460).step_by(7) {
            let r = f.fit_title(src, 18.0, w as f64);
            assert!(f.width(&r, 18.0, 0.0) <= w as f64 + 0.01, "{r} at {w}");
            // every complete word of the result is a word of the source; only the ellipsis may end a piece early
            for piece in r.replace('…', " … ").split_whitespace().filter(|p| *p != "…") {
                let piece = piece.trim_matches(|c| c == '(' || c == ')' || c == ':');
                assert!(words.contains(&piece), "{piece:?} is not a whole word of the source in {r:?} at width {w}");
            }
        }
    }

    #[test]
    fn a_single_word_wider_than_the_box_falls_back_to_a_character_cut() {
        let f = serif();
        let r = f.fit_title("Supercalifragilisticexpialidocious", 18.0, 100.0);
        assert!(r.ends_with('…') && f.width(&r, 18.0, 0.0) <= 100.0);
    }
}

