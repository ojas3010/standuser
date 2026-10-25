//! Stand card as one self-contained SVG (port of swarm/round3/card/gen_card.py, text-as-outlines mode).
//!
//! All text is drawn as paths with class="f-{cls}" (fill colour from the palette CSS), so it renders identically in
//! browsers, GitHub READMEs and PNG renderers. Fixes gen_card.py line 121, where text lost its `f-` prefix.

use crate::font::{font, Font, FontKind};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

pub const W: f64 = 1080.0;
pub const H: f64 = 1350.0;

pub const JP: [&str; 6] = ["破壊力", "スピード", "射程距離", "持続力", "精密動作性", "成長性"];
pub const EN: [&str; 6] = ["Destructive Power", "Speed", "Range", "Stamina", "Precision", "Potential"];
/// The small print at the bottom of the card.
pub const FOOTER: &str = "* provisional  ·  Fan work, not affiliated. Ratings: Letterboxd. Film data: TMDB (not endorsed or certified by TMDB). Soundtrack data from MusicBrainz.";
/// Under the type name: it is a reading of the stats, not a canon fact.
pub const TYPE_NOTE: &str = "archetype by stats · about 60% agreement with canon";
/// Added for the weakest type rule (Far-Distance Autopilot, AUC 0.59).
pub const TYPE_NOTE_LOW: &str = "low confidence";
pub const LEGEND: [(&str, &str); 5] = [("A", "超スゴイ"), ("B", "スゴイ"), ("C", "人間並"), ("D", "ニガテ"), ("E", "超ニガテ")];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    /// Light palette plus a prefers-color-scheme dark override (browsers, GitHub). PNG renderers show the light one.
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardData {
    /// English Stand name, upper case, without 「」.
    pub name: String,
    /// One line saying where the name came from, built from computed slots only; empty draws nothing.
    #[serde(default)]
    pub why: String,
    pub user: String,
    pub type_jp: String,
    pub type_en: String,
    /// Small line under the type: "archetype by stats · about 60% agreement with canon", plus "low confidence" for Automatic.
    #[serde(default)]
    pub type_note: String,
    /// Letters clockwise from the top: Power, Speed, Range, Stamina, Precision, Potential.
    pub grades: Vec<String>,
    /// Polygon radius step per axis (E=1 .. A=5, ∞=6, ∅=0, "?"=3).
    pub plot: Vec<f64>,
    pub prov: Vec<bool>,
    /// "?" axes get a hollow marker and dashed edges.
    pub unknown: Vec<bool>,
    /// (metric line, example film) per axis.
    pub evidence: Vec<(String, String)>,
    /// "graded on N of M films" per axis (what the stat was computed from).
    #[serde(default)]
    pub coverage: Vec<String>,
    pub ability: String,
    pub weakness: String,
    pub nearest: Vec<String>,
    pub signature: String,
    pub badge: bool,
}

struct Pal {
    bg: &'static str,
    ink: &'static str,
    mute: &'static str,
    gold: &'static str,
    red: &'static str,
    fill: &'static str,
    ring: &'static str,
    card: &'static str,
    faint: &'static str,
}

const LIGHT: Pal = Pal { bg: "#FBF8F0", ink: "#1C0B01", mute: "#5C5048", gold: "#9A7B00", red: "#C8200E", fill: "#89C3DE", ring: "#6B5D5A", card: "#FFFFFF", faint: "#E6DFCF" };
const DARK: Pal = Pal { bg: "#101014", ink: "#F4F1EA", mute: "#B9B3A8", gold: "#D8B93A", red: "#FF6A55", fill: "#5FA8D3", ring: "#C9C6C0", card: "#1B1B21", faint: "#2C2C34" };

fn css(p: &Pal) -> String {
    let mut s = String::new();
    for (k, v) in [("bg", p.bg), ("ink", p.ink), ("mute", p.mute), ("gold", p.gold), ("red", p.red), ("fill", p.fill), ("ring", p.ring), ("card", p.card), ("faint", p.faint)] {
        let _ = write!(s, ".f-{k}{{fill:{v}}}.s-{k}{{stroke:{v}}}");
    }
    s
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn n1(v: f64) -> String {
    format!("{v:.1}")
}

struct Ctx {
    sans: Font,
    serif: Font,
    els: Vec<String>,
}

impl Ctx {
    #[allow(clippy::too_many_arguments)]
    fn text(&mut self, s: &str, x: f64, y: f64, size: f64, kind: FontKind, cls: &str, anchor: &str, rot: f64, ls: f64, op: Option<f64>) {
        let face = if kind == FontKind::Sans { &self.sans } else { &self.serif };
        let w = face.width(s, size, ls);
        let x0 = match anchor {
            "middle" => x - w / 2.0,
            "end" => x - w,
            _ => x,
        };
        let tr = if rot != 0.0 { format!(" transform=\"rotate({} {} {})\"", rot, n1(x), n1(y)) } else { String::new() };
        let o = op.map(|o| format!(" opacity=\"{o}\"")).unwrap_or_default();
        let d = face.path(s, x0, y, size, ls);
        self.els.push(format!("<path d=\"{d}\" class=\"f-{cls}\"{tr}{o}/>"));
    }
}

const WHY_SIZE: f64 = 14.0;
const WHY_WIDTH: f64 = 940.0;

/// The type note is 14 px and at most this wide: at 15 px with a 400 px limit its end touched the medallion's rim.
const NOTE_SIZE: f64 = 14.0;
const NOTE_WIDTH: f64 = 380.0;

/// Pack the note's " · " parts into lines no wider than NOTE_WIDTH, so no part is ever cut.
fn note_lines(f: &Font, note: &str) -> Vec<String> {
    let mut lines: Vec<String> = vec![];
    for part in note.split(" · ").filter(|p| !p.is_empty()) {
        match lines.last_mut() {
            Some(last) if f.width(&format!("{last} · {part}"), NOTE_SIZE, 0.0) <= NOTE_WIDTH => *last = format!("{last} · {part}"),
            _ => lines.push(part.to_string()),
        }
    }
    lines
}

pub fn card_svg(d: &CardData, theme: Theme) -> String {
    use FontKind::{Sans, Serif};
    let mut c = Ctx { sans: font(Sans), serif: font(Serif), els: vec![] };
    let (cx, cy, r_chart) = (540.0, 546.0, 138.0);
    let k = r_chart / 150.0;

    // background and paper frame
    c.els.push(format!("<rect width=\"{W}\" height=\"{H}\" class=\"f-bg\"/>"));
    c.els.push(format!("<rect x=\"24\" y=\"24\" width=\"{}\" height=\"{}\" rx=\"6\" class=\"s-ink\" fill=\"none\" stroke-width=\"3\"/>", W - 48.0, H - 48.0));
    c.els.push(format!("<rect x=\"34\" y=\"34\" width=\"{}\" height=\"{}\" rx=\"3\" class=\"s-gold\" fill=\"none\" stroke-width=\"1.5\"/>", W - 68.0, H - 68.0));
    for i in 0..4 {
        let i = i as f64;
        c.text("ゴ", 66.0, 372.0 + i * 118.0, 92.0, Sans, "gold", "middle", -6.0 + i * 3.0, 0.0, Some(0.22));
        c.text("ゴ", W - 66.0, 372.0 + i * 118.0, 92.0, Sans, "gold", "middle", 6.0 - i * 3.0, 0.0, Some(0.22));
    }
    // header
    c.text("【STAND NAME】", 70.0, 84.0, 22.0, Serif, "mute", "start", 0.0, 2.0, None);
    if d.badge {
        c.els.push("<rect x=\"770\" y=\"64\" width=\"240\" height=\"40\" rx=\"20\" fill=\"none\" class=\"s-red\" stroke-width=\"2.5\"/>".into());
        c.text("PROVISIONAL DATA", 890.0, 91.0, 19.0, Sans, "red", "middle", 0.0, 1.5, None);
    }
    // shrink to 44 px, then cut with an ellipsis so the name never leaves the frame
    let mut name = format!("「{}」", d.name);
    let mut nsz = 78.0;
    while c.serif.width(&name, nsz, 2.0) > 940.0 && nsz > 44.0 {
        nsz -= 2.0;
    }
    if c.serif.width(&name, nsz, 2.0) > 940.0 {
        let mut inner: Vec<char> = d.name.chars().collect();
        while !inner.is_empty() && c.serif.width(&format!("「{}…」", inner.iter().collect::<String>().trim_end()), nsz, 2.0) > 940.0 {
            inner.pop();
        }
        name = format!("「{}…」", inner.iter().collect::<String>().trim_end());
    }
    c.text(&name, 70.0, 168.0, nsz, Serif, "ink", "start", 0.0, 2.0, None);
    // the why line: two lines of 14 px under the name, cut with an ellipsis rather than overflowing
    let why = c.sans.wrap(&d.why, WHY_SIZE, WHY_WIDTH);
    for (i, ln) in why.iter().take(2).enumerate() {
        let ln = if i == 1 && why.len() > 2 { c.sans.fit(&format!("{ln} …"), WHY_SIZE, WHY_WIDTH) } else { ln.clone() };
        c.text(&ln, 70.0, 190.0 + i as f64 * 16.0, WHY_SIZE, Sans, "mute", "start", 0.0, 0.0, None);
    }
    c.text("【STAND MASTER】", 70.0, 238.0, 22.0, Serif, "mute", "start", 0.0, 2.0, None);
    let user = c.sans.fit(&d.user, 34.0, 470.0);
    c.text(&format!("本体 ― {user}"), 350.0, 240.0, 34.0, Sans, "ink", "start", 0.0, 0.0, None);
    c.text(&format!("{} · {}", d.type_jp, d.type_en), 70.0, 288.0, 28.0, Sans, "mute", "start", 0.0, 0.0, None);
    for (i, ln) in note_lines(&c.sans, &d.type_note).iter().enumerate() {
        c.text(ln, 70.0, 309.0 + i as f64 * 16.0, NOTE_SIZE, Sans, "mute", "start", 0.0, 0.0, None);
    }

    // medallion
    let (ro, ri) = (268.0 * k, 250.0 * k);
    c.els.push(format!("<circle cx=\"{cx}\" cy=\"{cy}\" r=\"{ro:.1}\" fill=\"none\" class=\"s-gold\" stroke-width=\"3\"/>"));
    c.els.push(format!("<circle cx=\"{cx}\" cy=\"{cy}\" r=\"{:.1}\" fill=\"none\" class=\"s-gold\" stroke-width=\"2\"/>", ri - 1.0));
    let mut ticks = String::new();
    for i in 0..144 {
        let a = (i as f64 * 2.5).to_radians();
        let _ = write!(ticks, "M{:.1} {:.1}L{:.1} {:.1}", cx + ri * a.sin(), cy - ri * a.cos(), cx + (ro - 3.0) * a.sin(), cy - (ro - 3.0) * a.cos());
    }
    c.els.push(format!("<path d=\"{ticks}\" class=\"s-gold\" stroke-width=\"4.5\" fill=\"none\"/>"));
    c.els.push("<defs><pattern id=\"tone\" width=\"5\" height=\"5\" patternUnits=\"userSpaceOnUse\" patternTransform=\"rotate(45)\"><rect width=\"5\" height=\"5\" class=\"f-fill\" opacity=\".55\"/><circle cx=\"2.5\" cy=\"2.5\" r=\"1.45\" class=\"f-fill\"/></pattern></defs>".into());
    c.els.push(format!("<circle cx=\"{cx}\" cy=\"{cy}\" r=\"{r_chart}\" fill=\"none\" class=\"s-ring\" stroke-width=\"2.2\"/>"));
    // value polygon
    let pts: Vec<(f64, f64)> = (0..6)
        .map(|i| {
            let a = (i as f64 * 60.0).to_radians();
            let r = r_chart * d.plot[i] / 6.0;
            (cx + r * a.sin(), cy - r * a.cos())
        })
        .collect();
    let poly: Vec<String> = pts.iter().map(|(x, y)| format!("{x:.1} {y:.1}")).collect();
    c.els.push(format!("<path d=\"M{}Z\" fill=\"url(#tone)\"/>", poly.join("L")));
    for i in 0..6 {
        if d.unknown[i] {
            for j in [(i + 5) % 6, (i + 1) % 6] {
                c.els.push(format!("<path d=\"M{:.1} {:.1}L{:.1} {:.1}\" class=\"s-ring\" stroke-width=\"2\" stroke-dasharray=\"6 5\" fill=\"none\"/>", pts[i].0, pts[i].1, pts[j].0, pts[j].1));
            }
        }
    }
    for i in 0..3 {
        let a = (i as f64 * 60.0).to_radians();
        c.els.push(format!("<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" class=\"s-ink\" stroke-width=\"2\"/>", cx - r_chart * a.sin(), cy + r_chart * a.cos(), cx + r_chart * a.sin(), cy - r_chart * a.cos()));
    }
    for i in 0..6 {
        let a = (i as f64 * 60.0).to_radians();
        for kk in 1..=5 {
            let r = r_chart * kk as f64 / 6.0;
            c.els.push(format!("<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"3.2\" class=\"f-ink\"/>", cx + r * a.sin(), cy - r * a.cos()));
        }
    }
    for (kk, l) in "EDCBA".chars().enumerate() {
        c.text(&l.to_string(), cx + 11.0, cy - r_chart * (kk + 1) as f64 / 6.0 + 6.0, 16.0, Sans, "mute", "start", 0.0, 0.0, None);
    }
    for i in 0..6 {
        if d.unknown[i] {
            c.els.push(format!("<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"7\" class=\"f-bg s-ring\" stroke-width=\"2.5\"/>", pts[i].0, pts[i].1));
        }
    }
    // labels on the medallion annulus and red grade letters
    let rot: [f64; 6] = [0.0, 60.0, -60.0, 0.0, 60.0, -60.0];
    let (r_lab, r_let) = (224.0 * k, 183.0 * k);
    for i in 0..6 {
        let a = (i as f64 * 60.0).to_radians();
        let (lx, ly) = (cx + r_lab * a.sin(), cy - r_lab * a.cos());
        let th = rot[i].to_radians();
        let (bx, by) = (lx + 0.36 * 24.0 * (-th.sin()), ly + 0.36 * 24.0 * th.cos());
        c.text(JP[i], bx, by, 24.0, Sans, "ink", "middle", rot[i], 0.0, None);
        let g = d.grades[i].as_str();
        let (gx, gy) = (cx + r_let * a.sin(), cy - r_let * a.cos());
        let sz = if g == "完成" { 24.0 } else { 40.0 };
        c.text(g, gx, gy + if sz == 40.0 { 14.0 } else { 8.0 }, sz, Sans, "red", "middle", 0.0, 0.0, None);
    }

    // legend, rule, six stat cells (2 columns x 3 rows)
    let (y0, pitch) = (852.0, 72.0);
    let legend = LEGEND.iter().map(|(a, b)| format!("{a}―{b}")).collect::<Vec<_>>().join("  ");
    c.text(&legend, W / 2.0, 818.0, 20.0, Sans, "mute", "middle", 0.0, 0.0, None);
    c.els.push(format!("<line x1=\"70\" y1=\"{}\" x2=\"{}\" y2=\"{}\" class=\"s-ink\" stroke-width=\"2\"/>", y0 - 24.0, W - 70.0, y0 - 24.0));
    for i in 0..6 {
        let (col, row) = (i % 2, i / 2);
        let (x, y) = (70.0 + col as f64 * 470.0, y0 + row as f64 * pitch);
        let head = format!("{}―{}{}", JP[i], d.grades[i], if d.prov[i] { "*" } else { "" });
        c.text(&head, x, y + 20.0, 27.0, Sans, "ink", "start", 0.0, 0.0, None);
        if let Some(cv) = d.coverage.get(i) {
            let cv = c.sans.fit(cv, 15.0, 270.0);
            c.text(&cv, x + 440.0, y + 19.0, 15.0, Sans, "mute", "end", 0.0, 0.0, None);
        }
        let m = c.sans.fit(&d.evidence[i].0, 18.0, 440.0);
        c.text(&m, x, y + 42.0, 18.0, Sans, "mute", "start", 0.0, 0.0, None);
        let f = c.serif.fit_title(&d.evidence[i].1, 18.0, 440.0);
        c.text(&f, x, y + 61.0, 18.0, Serif, "ink", "start", 0.0, 0.0, None);
    }

    // ability and weakness box
    let ab = c.serif.wrap(&d.ability, 21.0, 800.0);
    let wk = c.serif.wrap(&d.weakness, 19.0, 800.0);
    let (al, wl) = (ab.len().min(3), wk.len().min(2));
    let top = 1078.0;
    let hbox = 22.0 + al as f64 * 26.0 + 8.0 + wl as f64 * 23.0 + 12.0;
    c.els.push(format!("<rect x=\"70\" y=\"{top}\" width=\"{}\" height=\"{hbox}\" rx=\"2\" class=\"f-card s-ink\" stroke-width=\"2\"/>", W - 140.0));
    c.text("能力―", 88.0, top + 30.0, 22.0, Sans, "red", "start", 0.0, 0.0, None);
    for (i, ln) in ab.iter().take(3).enumerate() {
        let ln = if i == 2 && ab.len() > 3 { c.serif.fit(&format!("{ln} …"), 21.0, 800.0) } else { ln.clone() };
        c.text(&ln, 170.0, top + 30.0 + i as f64 * 26.0, 21.0, Serif, "ink", "start", 0.0, 0.0, None);
    }
    let wy = top + 30.0 + (al as f64 - 1.0) * 26.0 + 34.0;
    c.text("弱点―", 88.0, wy, 20.0, Sans, "mute", "start", 0.0, 0.0, None);
    for (i, ln) in wk.iter().take(2).enumerate() {
        let ln = if i == 1 && wk.len() > 2 { c.serif.fit(&format!("{ln} …"), 19.0, 800.0) } else { ln.clone() };
        c.text(&ln, 170.0, wy + i as f64 * 23.0, 19.0, Serif, "mute", "start", 0.0, 0.0, None);
    }

    // footer rows
    let near = c.sans.fit(&format!("Nearest canon: {}", d.nearest.join(" · ")), 22.0, 940.0);
    c.text(&near, 70.0, 1272.0, 22.0, Sans, "ink", "start", 0.0, 0.0, None);
    let sig = c.serif.fit(&d.signature, 20.0, 940.0);
    c.text(&sig, 70.0, 1294.0, 20.0, Serif, "mute", "start", 0.0, 0.0, None);
    c.text(FOOTER, 70.0, 1312.0, 12.5, Sans, "mute", "start", 0.0, 0.0, None);

    // assemble
    let (base, extra) = match theme {
        Theme::Light => (css(&LIGHT), String::new()),
        Theme::Dark => (css(&DARK), String::new()),
        Theme::Auto => (css(&LIGHT), format!("@media (prefers-color-scheme:dark){{{}}}", css(&DARK))),
    };
    let title = format!("Stand card for {}: {}, {} type", d.user, d.name, d.type_en);
    // the same asterisks the card shows, so a screen reader hears which grades are provisional
    let any_prov = d.prov.iter().any(|p| *p);
    let desc = format!(
        "JoJo-style Stand stat chart. {}.{}",
        (0..6).map(|i| format!("{} {}{}", EN[i], d.grades[i], if d.prov[i] { "*" } else { "" })).collect::<Vec<_>>().join(", "),
        if any_prov { " Grades marked with an asterisk are provisional." } else { "" }
    );
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {W} {H}\" width=\"{W}\" height=\"{H}\" role=\"img\" aria-labelledby=\"t d\"><title id=\"t\">{}</title><desc id=\"d\">{}</desc><style>{base}{extra}</style>{}</svg>",
        esc(&title),
        esc(&desc),
        c.els.join("")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn sample() -> CardData {
        CardData {
            name: "DIRTY WEATHER".into(),
            why: "Why 「DIRTY WEATHER」: verbatim: \"DIRTY WEATHER\", track 3 of Some Band's Lighthouse score (2019); Lighthouse is one of your Favourite Films (4.5★). \"weather\" fits your Wind Reading ability; fit rank 1 of 40.".into(),
            user: "jas3344".into(),
            type_jp: "近距離パワー型".into(),
            type_en: "Close-Range Power".into(),
            type_note: format!("{TYPE_NOTE_LOW} · {TYPE_NOTE}"),
            grades: ["C", "B", "C", "A", "A", "?"].iter().map(|s| s.to_string()).collect(),
            plot: vec![3.0, 4.0, 3.0, 5.0, 5.0, 3.0],
            prov: vec![false, true, true, true, false, true],
            unknown: vec![false, false, false, false, false, true],
            evidence: (0..6).map(|i| (format!("metric {i}"), format!("Film {i}"))).collect(),
            coverage: (0..6).map(|i| format!("graded on {} of 300 films", 200 + i)).collect(),
            ability: "Wind Reading. It reads the air and answers it: it agrees with the crowd on 89% of the 74,164 film pairs it can compare (difficulty-adjusted score 0.88).".into(),
            weakness: "Soft spot: ordinary hands (Power C, “same as a human”). It rates 0.05★ below the crowd on average, about what a human would.".into(),
            nearest: vec!["D4C Love Train".into(), "Dolly Dagger".into(), "The World".into()],
            signature: "Signature film: Synthetic Film One (2008), 1.0★ against the crowd's 3.54★".into(),
            badge: true,
        }
    }

    #[test]
    fn text_uses_f_prefixed_classes() {
        // Regression: gen_card.py line 121 emitted class="{cls}" so text lost its fill colour.
        let s = card_svg(&sample(), Theme::Auto);
        assert!(s.contains("class=\"f-ink\""));
        assert!(s.contains("class=\"f-red\""));
        for bad in ["class=\"ink\"", "class=\"red\"", "class=\"mute\"", "class=\"gold\""] {
            assert!(!s.contains(bad), "{bad}");
        }
        assert!(s.contains("prefers-color-scheme:dark"));
        assert!(!s.contains("<text"), "text is outlined");
    }

    #[test]
    fn desc_marks_provisional_grades_with_asterisks() {
        let d = sample();
        let s = card_svg(&d, Theme::Light);
        let desc = &s[s.find("<desc").unwrap()..s.find("</desc>").unwrap()];
        assert!(desc.contains("Speed B*") && desc.contains("Range C*") && desc.contains("Power C,"), "{desc}");
        assert!(desc.contains("provisional"));
        let mut none = sample();
        none.prov = vec![false; 6];
        let s = card_svg(&none, Theme::Light);
        let desc = &s[s.find("<desc").unwrap()..s.find("</desc>").unwrap()];
        assert!(!desc.contains('*') && !desc.contains("provisional"), "{desc}");
    }

    #[test]
    fn themes_and_size() {
        let l = card_svg(&sample(), Theme::Light);
        assert!(!l.contains("prefers-color-scheme"));
        let dk = card_svg(&sample(), Theme::Dark);
        assert!(dk.contains("#101014"));
        assert!(l.starts_with("<svg") && l.ends_with("</svg>"));
        assert!(l.len() < 900_000, "{}", l.len());
    }

    #[test]
    fn very_long_names_are_cut_inside_the_frame() {
        let mut d = sample();
        d.name = "THE QUICK BROWN COMPOSER OF MANY LONG NAMES AND HOMAGES TO EVERYONE".into();
        let s = card_svg(&d, Theme::Light);
        assert!(s.contains("class=\"f-ink\""));
        // the same rule the renderer uses: at 44 px the cut name is at most 940 px wide
        let f = font(FontKind::Serif);
        assert!(f.width(&format!("「{}」", d.name), 44.0, 2.0) > 940.0, "the test name must need cutting");
    }

    #[test]
    fn text_fits_the_frame() {
        // wrapping keeps lines inside the box
        let f = font(FontKind::Serif);
        for ln in f.wrap(&sample().ability, 21.0, 800.0) {
            assert!(f.width(&ln, 21.0, 0.0) <= 800.0 + 0.01);
        }
        assert!(f.fit("a very long film title ".repeat(10).as_str(), 19.0, 440.0).ends_with('…'));
    }

    #[test]
    fn no_card_label_is_cut() {
        let c = Ctx { sans: font(FontKind::Sans), serif: font(FontKind::Serif), els: vec![] };
        let uncut = |f: &Font, s: &str, size: f64, w: f64| assert_eq!(f.fit(s, size, w), s, "cut: {s}");
        let d = sample();
        // both notes, with and without the low-confidence tag
        for note in [d.type_note.clone(), TYPE_NOTE.to_string()] {
            let lines = note_lines(&c.sans, &note);
            assert!(lines.len() <= 2 && lines.join(" · ") == note, "{lines:?}");
            for ln in &lines {
                assert!(c.sans.width(ln, NOTE_SIZE, 0.0) <= NOTE_WIDTH, "{ln}");
            }
        }
        // the note stays clear of the medallion rim (centre 540,546, outer radius 268*138/150) on every line
        let (cx, cy, ro) = (540.0f64, 546.0f64, 268.0 * 138.0 / 150.0);
        for (i, ln) in note_lines(&c.sans, &d.type_note).iter().enumerate() {
            let y = 309.0 + i as f64 * 16.0 + 3.0; // lowest point of the glyphs
            let rim_x = cx - (ro * ro - (cy - y).powi(2)).max(0.0).sqrt();
            assert!(70.0 + c.sans.width(ln, NOTE_SIZE, 0.0) < rim_x, "line {i} reaches the rim at x={rim_x}");
        }
        for i in 0..6 {
            uncut(&c.sans, &d.coverage[i], 15.0, 270.0);
            uncut(&c.sans, &d.evidence[i].0, 18.0, 440.0);
            uncut(&c.serif, &d.evidence[i].1, 18.0, 440.0);
        }
        uncut(&c.sans, &format!("Nearest canon: {}", d.nearest.join(" · ")), 22.0, 940.0);
        uncut(&c.serif, &d.signature, 20.0, 940.0);
        uncut(&c.sans, &d.user, 34.0, 470.0);
        assert!(c.serif.wrap(&d.ability, 21.0, 800.0).len() <= 3 && c.serif.wrap(&d.weakness, 19.0, 800.0).len() <= 2);
    }
}
