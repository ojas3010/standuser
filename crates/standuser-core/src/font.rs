//! Text as outlines: glyph paths from the embedded font subsets, so the SVG renders the same everywhere with no font files (all
//! SIL OFL; the licence texts sit beside the fonts in data/). Sans is Noto Sans JP Bold, for body text; Display is Dela Gothic One,
//! the manga lettering (katakana, the stat labels, ゴゴゴ); Name is Archivo Black, the Stand name. Characters missing from the
//! subset are drawn as □.

use ttf_parser::{Face, OutlineBuilder};

static SANS: &[u8] = include_bytes!("../data/sans.ttf");
static NAME: &[u8] = include_bytes!("../data/name.ttf");
static DISPLAY: &[u8] = include_bytes!("../data/display.ttf");

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FontKind {
    Sans,
    /// The Stand name (Archivo Black): Basic Latin, Latin-1, punctuation and 「」.
    Name,
    /// The manga lettering (Dela Gothic One): kana, the kanji of the labels and types, Basic Latin, 【】.
    Display,
}

pub struct Font {
    face: Face<'static>,
    upm: f64,
}

pub fn font(kind: FontKind) -> Font {
    let data = match kind {
        FontKind::Sans => SANS,
        FontKind::Name => NAME,
        FontKind::Display => DISPLAY,
    };
    let face = Face::parse(data, 0).expect("embedded font");
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

    /// How far the ink of `s` reaches above and below the baseline at `size`: (highest, deepest), both positive.
    pub fn ink_extent(&self, s: &str, size: f64) -> (f64, f64) {
        let (mut up, mut down) = (0.0f64, 0.0f64);
        for c in s.chars() {
            if let Some(b) = self.glyph(c).and_then(|g| self.face.glyph_bounding_box(g)) {
                up = up.max(b.y_max as f64);
                down = down.max(-(b.y_min as f64));
            }
        }
        (up * size / self.upm, down * size / self.upm)
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
        // the width of the line is kept as it grows: re-measuring the whole line for every token made wrapping quadratic
        let mut lines = Vec::new();
        let mut line = String::new();
        let (mut w, mut trail) = (0.0, 0.0); // advance of the line, and of the spaces at its end
        for t in toks {
            let tw: f64 = t.chars().map(|c| self.adv(c, size)).sum();
            let blank = t.chars().all(|c| c == ' ');
            if (if blank { w - trail } else { w + tw }) <= maxw {
                line.push_str(&t);
                w += tw;
                trail = if blank { trail + tw } else { 0.0 };
            } else {
                if !line.trim().is_empty() {
                    lines.push(line.trim_end().to_string());
                }
                line = t.trim_start().to_string();
                w = line.chars().map(|c| self.adv(c, size)).sum();
                trail = 0.0;
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
        let (title, suffix) = match s.rfind(" (").filter(|_| s.ends_with(')')) {
            Some(i) => (&s[..i], &s[i..]),
            None => (s, ""),
        };
        self.fit_parts("", title, suffix, size, maxw)
    }

    /// The signature-film line "Signature film: Title (1999), 4.5★ against the crowd's 3.46★ (+1.04), 192,136 ratings." under the same
    /// rule: only the title gives way (subtitle first, then words from its end), so the numbers stay; a line of another shape is
    /// treated as a plain film line.
    pub fn fit_signature(&self, s: &str, size: f64, maxw: f64) -> String {
        const PREFIX: &str = "Signature film: ";
        let year_at = |rest: &str| {
            rest.match_indices(" (").map(|(i, _)| i).find(|&i| rest.get(i + 2..i + 7).is_some_and(|y| y.as_bytes()[..4].iter().all(u8::is_ascii_digit) && y.ends_with(')')))
        };
        match s.strip_prefix(PREFIX).and_then(|rest| year_at(rest).map(|i| (&rest[..i], &rest[i..]))) {
            Some((title, tail)) => self.fit_parts(PREFIX, title, tail, size, maxw),
            None => self.fit_title(s, size, maxw),
        }
    }

    fn fit_parts(&self, prefix: &str, title: &str, suffix: &str, size: f64, maxw: f64) -> String {
        let fits = |t: &str| self.width(t, size, 0.0) <= maxw;
        let whole = format!("{prefix}{title}{suffix}");
        if fits(&whole) {
            return whole;
        }
        let title = title.split(':').next().unwrap_or(title).trim_end();
        let line = format!("{prefix}{title}{suffix}");
        if !title.is_empty() && fits(&line) {
            return line;
        }
        let cut = |text: &str, head: &str, tail: &str| -> Option<String> {
            let words: Vec<&str> = text.split(' ').filter(|w| !w.is_empty()).collect();
            (1..words.len()).rev().map(|n| format!("{head}{}…{tail}", words[..n].join(" "))).find(|c| fits(c))
        };
        // a one-word title has no word boundary to cut at: shorten the word itself so that what follows it (the numbers) stays
        let single_word = |text: &str, head: &str, tail: &str| -> Option<String> {
            if text.contains(' ') {
                return None;
            }
            let chars: Vec<char> = text.chars().collect();
            (1..chars.len()).rev().map(|n| format!("{head}{}…{tail}", chars[..n].iter().collect::<String>())).find(|c| fits(c))
        };
        cut(title, prefix, suffix)
            .or_else(|| single_word(title, prefix, suffix))
            .or_else(|| cut(&line, "", ""))
            .unwrap_or_else(|| self.fit(&line, size, maxw))
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

    /// The wrap the card used before it kept the line width as it grows: every line of the new one is the same.
    fn wrap_naive(f: &Font, text: &str, size: f64, maxw: f64) -> Vec<String> {
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
        let (mut lines, mut line) = (Vec::new(), String::new());
        for t in toks {
            let cand = format!("{line}{t}");
            if f.width(cand.trim_end(), size, 0.0) <= maxw {
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

    #[test]
    fn wrap_is_unchanged_by_the_incremental_width() {
        let f = font(FontKind::Sans);
        let mut texts: Vec<String> = crate::ability::all_sample_sentences();
        texts.push("Why 「DIRTY WEATHER」: verbatim: \"DIRTY WEATHER\", track 3 of Some Band's Lighthouse score (2019); Lighthouse is one of your Favourite Films (4.5★). \"weather\" fits your Wind Reading ability.".into());
        texts.push("日本語の文章と English words mixed together 久石譲 달파란  with  double  spaces and a verylongunbrokenwordthatcannotbebrokenanywhereatallbecauseithasnospaces ".repeat(3));
        texts.push("  leading spaces and a trailing one ".into());
        texts.push(String::new());
        for t in &texts {
            for (size, w) in [(20.0, 820.0), (19.0, 820.0), (20.0, 940.0), (18.0, 300.0), (20.0, 60.0)] {
                assert_eq!(f.wrap(t, size, w), wrap_naive(&f, t, size, w), "{t} at {size}/{w}");
            }
        }
    }

    /// The fitting rules run on the card's body text, which is the sans.
    fn sans() -> Font {
        font(FontKind::Sans)
    }

    #[test]
    fn a_title_that_fits_is_left_alone() {
        let f = sans();
        assert_eq!(f.fit_title("Ghost Rider: Spirit of Vengeance (Denmark)", 18.0, 2000.0), "Ghost Rider: Spirit of Vengeance (Denmark)");
    }

    #[test]
    fn the_subtitle_after_the_colon_goes_first_and_the_country_stays() {
        let f = sans();
        let long = "Ghost Rider: Spirit of Vengeance (United Arab Emirates)";
        let short = "Ghost Rider (United Arab Emirates)";
        let w = f.width(short, 18.0, 0.0) + 1.0;
        assert!(f.width(long, 18.0, 0.0) > w);
        assert_eq!(f.fit_title(long, 18.0, w), short);
    }

    #[test]
    fn if_still_too_long_the_title_is_cut_at_a_word_boundary_with_an_ellipsis() {
        let f = sans();
        let long = "Ghost Rider: Spirit of Vengeance (United Arab Emirates)";
        let w = f.width("Ghost… (United Arab Emirates)", 18.0, 0.0) + 1.0;
        assert_eq!(f.fit_title(long, 18.0, w), "Ghost… (United Arab Emirates)");
        let t = "The Lord of the Rings the Fellowship of the Ring (Denmark)";
        let w = f.width("The Lord of the Rings… (Denmark)", 18.0, 0.0) + 1.0;
        assert_eq!(f.fit_title(t, 18.0, w), "The Lord of the Rings… (Denmark)");
    }

    #[test]
    fn never_cuts_inside_a_word_at_any_width() {
        let f = sans();
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

    const LONG_SIG: &str = "Signature film: Borat: Cultural Learnings of America for Make Benefit Glorious Nation of Kazakhstan (2006), 4.5★ against the crowd's 3.46★ (+1.04), 192,136 ratings.";

    #[test]
    fn the_signature_line_is_cut_mid_word_by_plain_fit_and_not_by_fit_signature() {
        let f = sans();
        assert!(f.width(LONG_SIG, 20.0, 0.0) > 940.0);
        let old = f.fit(LONG_SIG, 20.0, 940.0);
        assert!(old.ends_with('…') && !old.ends_with(" …") && !old.contains("ratings"), "the old cut ends inside the line: {old}");
        // the subtitle after ':' goes first, and the numbers stay
        let r = f.fit_signature(LONG_SIG, 20.0, 940.0);
        assert_eq!(r, "Signature film: Borat (2006), 4.5★ against the crowd's 3.46★ (+1.04), 192,136 ratings.");
    }

    #[test]
    fn a_signature_title_without_a_colon_is_cut_at_a_word_boundary_and_keeps_the_numbers() {
        let f = sans();
        let s = "Signature film: The Lord of the Rings the Fellowship of the Ring Extended Edition Director's Cut (2001), 4.5★ against the crowd's 3.46★ (+1.04), 192,136 ratings.";
        let r = f.fit_signature(s, 20.0, 940.0);
        assert!(f.width(&r, 20.0, 0.0) <= 940.0, "{r}");
        assert!(r.starts_with("Signature film: The Lord of") && r.contains("… (2001), 4.5★ against the crowd's 3.46★ (+1.04), 192,136 ratings."), "{r}");
        let title = r.strip_prefix("Signature film: ").unwrap().split('…').next().unwrap();
        assert!(s.contains(&format!("{title} ")), "the title ends on a whole word: {title:?}");
        // short lines and other shapes pass through
        assert_eq!(f.fit_signature("Signature film: not enough crowd data", 20.0, 940.0), "Signature film: not enough crowd data");
        let odd = f.fit_signature("Something else entirely: with a colon and a very long tail that goes on and on and on and on and on and on and on and on and on and on", 20.0, 500.0);
        assert_eq!(odd, "Something else entirely", "another shape is a plain film line: the part after the colon goes");
    }

    #[test]
    fn a_one_word_signature_title_is_shortened_so_the_numbers_after_it_stay() {
        let f = sans();
        // this line was cut after "(-3.55)," by the word-boundary rule, which dropped the rating count
        let s = "Signature film: themuppetchristmascarol (1992), 0.5★ against the crowd's 4.05★ (-3.55), 388,056 ratings.";
        assert!(f.width(s, 20.0, 0.0) > 940.0);
        let r = f.fit_signature(s, 20.0, 940.0);
        assert!(f.width(&r, 20.0, 0.0) <= 940.0, "{r}");
        assert!(r.starts_with("Signature film: themuppet") && r.ends_with("… (1992), 0.5★ against the crowd's 4.05★ (-3.55), 388,056 ratings."), "{r}");
        // a film line with a one-word title keeps its country the same way
        let r = f.fit_title("Supercalifragilisticexpialidocious (United Arab Emirates)", 18.0, 300.0);
        assert!(r.ends_with("… (United Arab Emirates)") && f.width(&r, 18.0, 0.0) <= 300.0, "{r}");
    }

    #[test]
    fn a_single_word_wider_than_the_box_falls_back_to_a_character_cut() {
        let f = sans();
        let r = f.fit_title("Supercalifragilisticexpialidocious", 18.0, 100.0);
        assert!(r.ends_with('…') && f.width(&r, 18.0, 0.0) <= 100.0);
    }
}

