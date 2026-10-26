//! Stand card as one self-contained SVG (port of swarm/round3/card/gen_card.py, text-as-outlines mode).
//!
//! All text is drawn as paths with class="f-{cls}" (fill colour from the palette CSS), so it renders identically in
//! browsers, GitHub READMEs and PNG renderers. Fixes gen_card.py line 121, where text lost its `f-` prefix.
//! The look is the manga's: a flat saturated cover colour for the ground, heavy lettering (Dela Gothic One for the katakana, labels and
//! ゴゴゴ, Archivo Black for the Stand name) with a black outline drawn as a second, thicker black path under the fill path, and the
//! Part 5 stat chart: a hexagon with a ring per grade, circled A to E up the top axis, and a circled grade at each vertex.

use crate::font::{body_text, font, BodyText, Draw, Font, FontKind};
use crate::palette::{self, Pal};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

pub const W: f64 = 1080.0;
pub const H: f64 = 1350.0;

pub const JP: [&str; 6] = ["破壊力", "スピード", "射程距離", "持続力", "精密動作性", "成長性"];
pub const EN: [&str; 6] = ["Destructive Power", "Speed", "Range", "Stamina", "Precision", "Potential"];
/// The one footer line at the bottom of the card; the data credit and the TMDB disclaimer are on the page.
pub const FOOTER: &str = "standuser.pages.dev · Fan work, not affiliated · * provisional";
/// The data credit the card used to carry; the page gets it in the analysis JSON (`card.credit`).
pub const CREDIT: &str = "Data: Letterboxd, TMDB, MusicBrainz";
/// The archetype caveat: it is a reading of the stats, not a canon fact. Not drawn on the card; the page gets it in `card.type_note`.
pub const TYPE_NOTE: &str = "archetype by stats · about 60% agreement with canon";
/// Added for the weakest type rule (Far-Distance Autopilot, AUC 0.59).
pub const TYPE_NOTE_LOW: &str = "low confidence";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    /// Light palette plus a prefers-color-scheme dark override (browsers, GitHub). PNG renderers show the light one.
    Auto,
}

/// Everything the analysis says about the card. The card draws only part of it (docs/DESIGN.md section 4): `name`, `user`, `type_jp`,
/// `type_en`, `grades` with `prov`, `plot` and `unknown`, `ability_name`, `ability_line` and `nearest`. The other fields are no longer drawn
/// on the card; they travel in the analysis JSON (`analysis.card`), so the page has every one of them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardData {
    /// English Stand name, upper case, without 「」.
    pub name: String,
    /// (page only) One line saying where the name came from, built from computed slots only; empty when the name has no such line.
    #[serde(default)]
    pub why: String,
    pub user: String,
    pub type_jp: String,
    pub type_en: String,
    /// (page only) The archetype caveat: "archetype by stats · about 60% agreement with canon", plus "low confidence" for Automatic.
    #[serde(default)]
    pub type_note: String,
    /// Letters clockwise from the top: Power, Speed, Range, Stamina, Precision, Potential.
    pub grades: Vec<String>,
    /// Polygon radius step per axis (E=1 .. A=5, ∞=6, ∅=0, "?"=3).
    pub plot: Vec<f64>,
    /// A grade with an asterisk on the card: provisional.
    pub prov: Vec<bool>,
    /// "?" axes get a hollow marker and dashed edges.
    pub unknown: Vec<bool>,
    /// (page only) (metric line, example film) per axis: the stat value line and the film title.
    pub evidence: Vec<(String, String)>,
    /// (page only) "graded on N of M films" per axis (what the stat was computed from).
    #[serde(default)]
    pub coverage: Vec<String>,
    /// (page only) The full ability text: "Name. sentence: figures".
    pub ability: String,
    /// The ability's name, drawn on the card.
    #[serde(default)]
    pub ability_name: String,
    /// The one sentence of at most 20 words drawn under the ability name.
    #[serde(default)]
    pub ability_line: String,
    /// (page only)
    pub weakness: String,
    pub nearest: Vec<String>,
    /// (page only) The signature film line.
    pub signature: String,
    /// (page only) Some grade is provisional (the card had a PROVISIONAL DATA pill for it).
    pub badge: bool,
    /// (page only) The data credit line the card used to carry.
    #[serde(default)]
    pub credit: String,
    /// "PWR".."DEV": the signature stat's accent (palette.rs); none draws the neutral brick and ink.
    #[serde(default)]
    pub accent: Option<String>,
}

fn css(p: &Pal) -> String {
    let mut s = String::new();
    for (k, v) in [("bg", p.bg), ("ink", p.ink), ("mute", p.mute), ("paper", p.paper), ("bk", p.bk), ("pmute", p.pmute), ("fill", p.fill)] {
        let _ = write!(s, ".f-{k}{{fill:{v}}}.s-{k}{{stroke:{v}}}");
    }
    // the black outline of the lettering and the radar: a filled and stroked path, drawn under the fill path of the same text
    let _ = write!(s, ".o{{fill:{0};stroke:{0};stroke-linejoin:round}}.r{{stroke-linejoin:round}}", p.bk);
    s
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn n1(v: f64) -> String {
    format!("{v:.1}")
}

/// One piece of text as drawn: what it says, in which face and size, and the box of its ink (before any rotation).
#[derive(Debug, Clone)]
pub struct Run {
    pub text: String,
    pub size: f64,
    pub kind: FontKind,
    pub outlined: bool,
    pub rotated: bool,
    /// Left, top, right and bottom of the ink.
    pub bbox: [f64; 4],
}

struct Ctx {
    /// Noto Sans JP Bold, Japanese below 48 px.
    jp: Font,
    /// Archivo 700, the Latin body text.
    body: BodyText,
    /// Archivo Black, the Stand name and the Latin display text.
    name: Font,
    /// Dela Gothic One, Japanese at 48 px or larger.
    display: Font,
    els: Vec<String>,
    runs: Vec<Run>,
}

/// Japanese at this size or larger is lettering: Dela Gothic One with the black outline. Below it, Noto Sans JP Bold, plain.
pub const JP_LETTERING_MIN: f64 = 48.0;

impl Ctx {
    fn face(&self, kind: FontKind) -> &dyn Draw {
        match kind {
            FontKind::Jp => &self.jp,
            FontKind::Body => &self.body,
            FontKind::Name => &self.name,
            FontKind::Display => &self.display,
        }
    }

    /// Japanese text: lettering (Dela Gothic One, outlined) from JP_LETTERING_MIN px up, Noto Sans JP Bold below.
    #[allow(clippy::too_many_arguments)]
    fn jp_text(&mut self, s: &str, x: f64, y: f64, size: f64, cls: &str, anchor: &str) {
        if size >= JP_LETTERING_MIN {
            self.lettering(s, x, y, size, FontKind::Display, cls, anchor, 0.0, OUTLINE);
        } else {
            self.text(s, x, y, size, FontKind::Jp, cls, anchor, 0.0);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn text(&mut self, s: &str, x: f64, y: f64, size: f64, kind: FontKind, cls: &str, anchor: &str, ls: f64) {
        let face = self.face(kind);
        let w = face.width(s, size, ls);
        let x0 = match anchor {
            "middle" => x - w / 2.0,
            "end" => x - w,
            _ => x,
        };
        let d = face.path(s, x0, y, size, ls);
        let (up, down) = face.ink_extent(s, size);
        self.runs.push(Run { text: s.into(), size, kind, outlined: false, rotated: false, bbox: [x0, y - up, x0 + w, y + down] });
        self.els.push(format!("<path d=\"{d}\" class=\"f-{cls}\"/>"));
    }

    /// Lettering with the manga's black outline: a black path stroked `ow` wide, and the fill path over it.
    #[allow(clippy::too_many_arguments)]
    fn lettering(&mut self, s: &str, x: f64, y: f64, size: f64, kind: FontKind, cls: &str, anchor: &str, ls: f64, ow: f64) {
        let face = self.face(kind);
        let w = face.width(s, size, ls);
        let x0 = match anchor {
            "middle" => x - w / 2.0,
            "end" => x - w,
            _ => x,
        };
        let d = face.path(s, x0, y, size, ls);
        let (up, down) = face.ink_extent(s, size);
        self.runs.push(Run { text: s.into(), size, kind, outlined: true, rotated: false, bbox: [x0 - ow / 2.0, y - up - ow / 2.0, x0 + w + ow / 2.0, y + down + ow / 2.0] });
        self.els.push(format!("<path d=\"{d}\" class=\"o\" stroke-width=\"{ow}\"/><path d=\"{d}\" class=\"f-{cls}\"/>"));
    }

    /// `s` centred on (cx, cy) both ways, by its real ink (a letter in a circle).
    fn centred(&mut self, s: &str, cx: f64, cy: f64, size: f64, kind: FontKind, cls: &str) {
        let (up, down) = self.face(kind).ink_extent(s, size);
        self.text(s, cx, cy + (up - down) / 2.0, size, kind, cls, "middle", 0.0);
    }

    /// Japanese `s` centred on (cx, cy) like `centred`.
    fn centred_jp(&mut self, s: &str, cx: f64, cy: f64, size: f64, cls: &str) {
        let kind = if size >= JP_LETTERING_MIN { FontKind::Display } else { FontKind::Jp };
        let (up, down) = self.face(kind).ink_extent(s, size);
        self.jp_text(s, cx, cy + (up - down) / 2.0, size, cls, "middle");
    }
}

/// The chart: centre height and the hexagon's circumradius (the ∞ ring; a grade E to A is the ring k of 6 for k = 1 to 5). The hexagon
/// is 55% of the card's width or more: its plate is √3 · CHART_R wide.
pub const CX: f64 = W / 2.0;
pub const CY: f64 = 664.0;
pub const CHART_R: f64 = 346.0;
/// The circled grade sits on each vertex: its radius, its letter's size, and the size of the JP label beside it.
const BADGE_R: f64 = 32.0;
const BADGE_TEXT: f64 = 38.0;
const LABEL_SIZE: f64 = MIN_TEXT;
/// Gap between a badge and its label.
const LABEL_GAP: f64 = 12.0;

/// The thick black border: its width, drawn inward from the card's edge.
pub const FRAME: f64 = 16.0;

/// No text on the card is smaller than this, in card units (the card is 1080 wide): at a phone's 390 px width it is still about 12 px.
pub const MIN_TEXT: f64 = 34.0;

/// The left edge of the text, and the right edge.
pub const MARGIN: f64 = 48.0;
pub const RIGHT: f64 = W - MARGIN;
/// The Stand master sits in the free corner above the name's low left end; the type sits in the free corner under its high right end.
const MASTER_Y: f64 = 90.0;
const MASTER_SIZE: f64 = MIN_TEXT;
const TYPE_SIZE: f64 = MIN_TEXT;
const TYPE_JP_Y: f64 = 214.0;
const TYPE_EN_Y: f64 = 262.0;

/// The bottom of the card is laid out from the foot up: the footer, the nearest canon line, the ability's sentence (at most 3 rows) and
/// above it the ability's name.
const LINE_SIZE: f64 = MIN_TEXT;
const LINE_PITCH: f64 = 42.0;
const LINE_WIDTH: f64 = RIGHT - MARGIN;
const LINE_MAX_ROWS: usize = 3;
const ABILITY_SIZE: f64 = 44.0;
pub const FOOT_Y: f64 = 1312.0;
pub const NEAR_Y: f64 = 1262.0;
const NEAR_SIZE: f64 = MIN_TEXT;
/// Baseline of the last row of the sentence, and of the ability's name above the first row.
const LINE_LAST_Y: f64 = NEAR_Y - 48.0;
const ABILITY_RISE: f64 = 50.0;

/// Where the ability's name and its rows of sentence stand: (name baseline, first row baseline).
pub fn ability_rows(rows: usize) -> (f64, f64) {
    let first = LINE_LAST_Y - (rows.clamp(1, LINE_MAX_ROWS) as f64 - 1.0) * LINE_PITCH;
    (first - ABILITY_RISE, first)
}

/// The ゴゴゴ: three ゴ on a diagonal down the left side, staggered, each smaller than the last (a Dela Gothic One ゴ is one em wide):
/// (size, centre x, centre y). They stand in the free strip between the card's edge and the hexagon, clear of every text.
pub const GOGOGO: [(f64, f64, f64); 3] = [(110.0, 98.0, 546.0), (84.0, 136.0, 668.0), (62.0, 152.0, 776.0)];
/// Width of the black outline of the display lettering, in px (half of it shows outside the glyph).
pub const OUTLINE: f64 = 6.0;
/// The Stand name's outline, which is heavier: the name is the card's logo.
pub const NAME_OUTLINE: f64 = 12.0;
/// The Stand name's one tilt, in degrees as SVG's rotate() takes them: negative is counter-clockwise, so the name climbs to the right,
/// like the slant of a Part logo. Nothing else on the card is rotated.
pub const NAME_TILT: f64 = -5.0;
/// The header box the tilted name stays inside. Its low left corner is the box's low left corner, so the name's bottom edge is one fixed
/// line whatever the name, and the corners above and below that line stay free for the master and the type.
pub const NAME_X0: f64 = MARGIN;
pub const NAME_X1: f64 = RIGHT;
pub const NAME_TOP: f64 = 30.0;
pub const NAME_BOTTOM: f64 = 270.0;
pub const NAME_MAX_SIZE: f64 = 120.0;
pub const NAME_MIN_SIZE: f64 = 34.0;
/// Letter spacing of the Stand name.
const NAME_LS: f64 = 1.0;

/// Where the name goes: the text in 「」, its size, and the baseline start (x, y) it is rotated about, plus the corners of the tilted box
/// (the ink and its outline).
pub struct NamePlace {
    pub text: String,
    pub size: f64,
    pub x: f64,
    pub y: f64,
    pub corners: [(f64, f64); 4],
}

fn name_box(f: &Font, text: &str, size: f64) -> NamePlace {
    let w = f.width(text, size, NAME_LS);
    let (up, down) = f.ink_extent(text, size);
    let pad = NAME_OUTLINE / 2.0;
    let th = NAME_TILT.to_radians();
    let rot = |(x, y): (f64, f64)| (x * th.cos() - y * th.sin(), x * th.sin() + y * th.cos());
    let raw = [rot((-pad, -up - pad)), rot((w + pad, -up - pad)), rot((w + pad, down + pad)), rot((-pad, down + pad))];
    let (min_x, max_y) = (raw.iter().map(|p| p.0).fold(f64::MAX, f64::min), raw.iter().map(|p| p.1).fold(f64::MIN, f64::max));
    // as far left and as low as the box allows: that leaves the most room for the right end to climb
    let (x, y) = (NAME_X0 - min_x, NAME_BOTTOM - max_y);
    NamePlace { text: text.to_string(), size, x, y, corners: raw.map(|(px, py)| (px + x, py + y)) }
}

fn name_fits(p: &NamePlace) -> bool {
    let c = &p.corners;
    c.iter().all(|q| q.0 >= NAME_X0 - 0.01 && q.0 <= NAME_X1 && q.1 >= NAME_TOP && q.1 <= NAME_BOTTOM + 0.01)
}

/// The largest size from NAME_MAX_SIZE down to NAME_MIN_SIZE at which the tilted name fits the header. Only a name that does not fit
/// even at the smallest size is cut, with an ellipsis; none in names.json is (a test checks every one).
pub fn place_name(f: &Font, name: &str) -> NamePlace {
    let text = format!("「{name}」");
    let mut size = NAME_MAX_SIZE;
    while size >= NAME_MIN_SIZE {
        let p = name_box(f, &text, size);
        if name_fits(&p) {
            return p;
        }
        size -= 1.0;
    }
    let mut inner: Vec<char> = name.chars().collect();
    loop {
        let cut = format!("「{}…」", inner.iter().collect::<String>().trim_end());
        let p = name_box(f, &cut, NAME_MIN_SIZE);
        if inner.is_empty() || name_fits(&p) {
            return p;
        }
        inner.pop();
    }
}

/// The hexagon's six vertices at radius `r` about (cx, cy), clockwise from the top.
fn hex(cx: f64, cy: f64, r: f64) -> Vec<(f64, f64)> {
    (0..6).map(|i| {
        let a = (i as f64 * 60.0).to_radians();
        (cx + r * a.sin(), cy - r * a.cos())
    }).collect()
}

fn pts(v: &[(f64, f64)]) -> String {
    v.iter().map(|(x, y)| format!("{x:.1},{y:.1}")).collect::<Vec<_>>().join(" ")
}

/// "Nearest canon: A · B · C": all the names that fit the line, never a name cut in half; the first name is cut only if it alone is too long.
fn nearest_line(f: &BodyText, names: &[String]) -> String {
    for n in (1..=names.len()).rev() {
        let line = format!("Nearest canon: {}", names[..n].join(" · "));
        if f.width(&line, NEAR_SIZE, 0.0) <= RIGHT - MARGIN {
            return line;
        }
    }
    f.fit(&format!("Nearest canon: {}", names.join(" · ")), NEAR_SIZE, RIGHT - MARGIN)
}

pub fn card_svg(d: &CardData, theme: Theme) -> String {
    card_build(d, theme).0
}

/// The card's SVG and every piece of text on it, as drawn (for the tests that measure the card).
pub fn card_build(d: &CardData, theme: Theme) -> (String, Vec<Run>) {
    use FontKind::{Body, Name};
    let mut c = Ctx { jp: font(FontKind::Jp), body: body_text(), name: font(Name), display: font(FontKind::Display), els: vec![], runs: vec![] };
    let (cx, cy, r_chart) = (CX, CY, CHART_R);

    // the ground, and the one frame: the thick black border of a manga panel, flush with the card's edge (no margin outside it, no
    // line inside it)
    c.els.push(format!("<rect width=\"{W}\" height=\"{H}\" class=\"f-bg\"/>"));
    c.els.push(format!("<rect x=\"{0}\" y=\"{0}\" width=\"{1}\" height=\"{2}\" class=\"s-bk\" fill=\"none\" stroke-width=\"{FRAME}\"/>", FRAME / 2.0, W - FRAME, H - FRAME));
    // the ゴゴゴ of the scene, on a diagonal down the left side
    for (sz, gx, gy) in GOGOGO {
        let (up, down) = c.display.ink_extent("ゴ", sz);
        c.jp_text("ゴ", gx, gy + (up - down) / 2.0, sz, "fill", "middle");
    }
    // the Stand name: the card's one diagonal, largest size that keeps its tilted box in the header; the outline and the fill share the tilt
    let np = place_name(&c.name, &d.name);
    {
        let dpath = c.name.path(&np.text, np.x, np.y, np.size, NAME_LS);
        let xs = np.corners.map(|p| p.0);
        let ys = np.corners.map(|p| p.1);
        c.runs.push(Run {
            text: np.text.clone(),
            size: np.size,
            kind: Name,
            outlined: true,
            rotated: true,
            bbox: [xs.iter().cloned().fold(f64::MAX, f64::min), ys.iter().cloned().fold(f64::MAX, f64::min), xs.iter().cloned().fold(f64::MIN, f64::max), ys.iter().cloned().fold(f64::MIN, f64::max)],
        });
        c.els.push(format!(
            "<g transform=\"rotate({} {} {})\"><path d=\"{dpath}\" class=\"o\" stroke-width=\"{NAME_OUTLINE}\"/><path d=\"{dpath}\" class=\"f-paper\"/></g>",
            NAME_TILT, n1(np.x), n1(np.y)
        ));
    }
    // the Stand master (top left, above the name's low end) and the type in Japanese and English (right, under the name's high end)
    let label = "本体―";
    c.jp_text(label, MARGIN, MASTER_Y, MASTER_SIZE, "ink", "start");
    let lw = c.jp.width(label, MASTER_SIZE, 0.0);
    let user = c.body.fit(&d.user, MASTER_SIZE, 470.0 - lw - 10.0);
    c.text(&user, MARGIN + lw + 10.0, MASTER_Y, MASTER_SIZE, Body, "ink", "start", 0.0);
    c.jp_text(&d.type_jp, RIGHT, TYPE_JP_Y, TYPE_SIZE, "ink", "end");
    c.text(&d.type_en, RIGHT, TYPE_EN_Y, TYPE_SIZE, Body, "ink", "end", 0.0);

    // the chart, after the Part 5 tailpiece: a paper hexagon, a ring per grade, three spokes, a dot on each spoke at each ring
    let outer = hex(cx, cy, r_chart);
    c.els.push(format!("<polygon points=\"{}\" class=\"f-paper s-bk r\" stroke-width=\"4\"/>", pts(&outer)));
    for k in 1..=5 {
        c.els.push(format!("<polygon points=\"{}\" fill=\"none\" class=\"s-bk\" stroke-width=\"1.5\"/>", pts(&hex(cx, cy, r_chart * k as f64 / 6.0))));
    }
    for (x, y) in outer.iter().take(3) {
        c.els.push(format!("<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" class=\"s-bk\" stroke-width=\"1.5\"/>", x, y, 2.0 * cx - x, 2.0 * cy - y));
    }
    // value polygon
    let vals: Vec<(f64, f64)> = (0..6)
        .map(|i| {
            let a = (i as f64 * 60.0).to_radians();
            let r = r_chart * d.plot[i] / 6.0;
            (cx + r * a.sin(), cy - r * a.cos())
        })
        .collect();
    c.els.push(format!("<polygon points=\"{}\" class=\"f-fill s-bk r\" stroke-width=\"3.5\"/>", pts(&vals)));
    for i in 0..6 {
        if d.unknown[i] {
            for j in [(i + 5) % 6, (i + 1) % 6] {
                c.els.push(format!("<path d=\"M{:.1} {:.1}L{:.1} {:.1}\" class=\"s-bk\" stroke-width=\"2\" stroke-dasharray=\"6 5\" fill=\"none\"/>", vals[i].0, vals[i].1, vals[j].0, vals[j].1));
            }
        }
    }
    for i in 0..6 {
        let a = (i as f64 * 60.0).to_radians();
        for kk in 1..=5 {
            let r = r_chart * kk as f64 / 6.0;
            c.els.push(format!("<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"2.6\" class=\"f-bk\"/>", cx + r * a.sin(), cy - r * a.cos()));
        }
    }
    for i in 0..6 {
        if d.unknown[i] {
            c.els.push(format!("<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"7\" class=\"f-paper s-bk\" stroke-width=\"2.5\"/>", vals[i].0, vals[i].1));
        }
    }
    // the circled grade on each vertex, and the label beside it: to the right at the top, the bottom and the two right vertices; at the
    // two left vertices below and above, so the label stays off the card's edge (破壊力 top, スピード upper right, 射程距離 lower right,
    // 持続力 bottom, 精密動作性 lower left, 成長性 upper left)
    for i in 0..6 {
        let a = (i as f64 * 60.0).to_radians();
        let (bx, by) = (cx + r_chart * a.sin(), cy - r_chart * a.cos());
        let g = format!("{}{}", d.grades[i], if d.prov[i] { "*" } else { "" });
        let pill = d.grades[i] == "完成";
        let half = if pill { 46.0 } else { BADGE_R };
        if pill {
            c.els.push(format!("<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{}\" height=\"{}\" rx=\"{BADGE_R}\" class=\"f-paper s-bk\" stroke-width=\"3\"/>", bx - half, by - BADGE_R, 2.0 * half, 2.0 * BADGE_R));
            c.centred_jp(&g, bx, by, BADGE_TEXT - 4.0, "bk");
        } else {
            c.els.push(format!("<circle cx=\"{bx:.1}\" cy=\"{by:.1}\" r=\"{BADGE_R}\" class=\"f-paper s-bk\" stroke-width=\"3\"/>"));
            c.centred(&g, bx, by, BADGE_TEXT, Name, "bk");
        }
        let (up, down) = c.jp.ink_extent(JP[i], LABEL_SIZE);
        let mid = (up - down) / 2.0;
        match i {
            0..=3 => c.jp_text(JP[i], bx + half + LABEL_GAP, by + mid, LABEL_SIZE, "ink", "start"),
            4 => c.jp_text(JP[i], bx + BADGE_R * 0.8, by + BADGE_R + LABEL_GAP + up, LABEL_SIZE, "ink", "end"),
            _ => c.jp_text(JP[i], bx + BADGE_R * 0.8, by - BADGE_R - LABEL_GAP, LABEL_SIZE, "ink", "end"),
        }
    }

    // the ability: its name, then its one sentence
    let aname = if d.ability_name.is_empty() { "Ability" } else { d.ability_name.as_str() };
    let rows: Vec<String> = c.body.wrap(&d.ability_line, LINE_SIZE, LINE_WIDTH).into_iter().take(LINE_MAX_ROWS).collect();
    let (name_y, first_y) = ability_rows(rows.len());
    c.text(aname, MARGIN, name_y, ABILITY_SIZE, Name, "ink", "start", 0.0);
    for (i, ln) in rows.iter().enumerate() {
        c.text(ln, MARGIN, first_y + i as f64 * LINE_PITCH, LINE_SIZE, Body, "ink", "start", 0.0);
    }
    // the nearest canon line, and the one footer line
    let near = nearest_line(&c.body, &d.nearest);
    c.text(&near, MARGIN, NEAR_Y, NEAR_SIZE, Body, "ink", "start", 0.0);
    c.text(FOOTER, MARGIN, FOOT_Y, NEAR_SIZE, Body, "mute", "start", 0.0);

    // assemble
    let key = d.accent.as_deref();
    let (base, extra) = match theme {
        Theme::Light => (css(&palette::light(key)), String::new()),
        Theme::Dark => (css(&palette::dark(key)), String::new()),
        Theme::Auto => (css(&palette::light(key)), format!("@media (prefers-color-scheme:dark){{{}}}", css(&palette::dark(key)))),
    };
    let title = format!("Stand card for {}: {}, {} type", d.user, d.name, d.type_en);
    // the same asterisks the card shows, so a screen reader hears which grades are provisional
    let any_prov = d.prov.iter().any(|p| *p);
    let desc = format!(
        "JoJo-style Stand stat chart. {}.{}",
        (0..6).map(|i| format!("{} {}{}", EN[i], d.grades[i], if d.prov[i] { "*" } else { "" })).collect::<Vec<_>>().join(", "),
        if any_prov { " Grades marked with an asterisk are provisional." } else { "" }
    );
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {W} {H}\" width=\"{W}\" height=\"{H}\" role=\"img\" aria-labelledby=\"t d\"><title id=\"t\">{}</title><desc id=\"d\">{}</desc><style>{base}{extra}</style>{}</svg>",
        esc(&title),
        esc(&desc),
        c.els.join("")
    );
    (svg, c.runs)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn sample() -> CardData {
        CardData {
            name: "DIRTY WEATHER".into(),
            why: "Why 「DIRTY WEATHER」: verbatim: \"DIRTY WEATHER\", track 3 of Some Band's Lighthouse score (2019); Lighthouse is one of your Favourite Films (4.5★). \"weather\" fits your Wind Reading ability.".into(),
            user: "jas3344".into(),
            type_jp: "近距離パワー型".into(),
            type_en: "Close-Range Power".into(),
            type_note: format!("{TYPE_NOTE_LOW} · {TYPE_NOTE}"),
            grades: ["C", "B", "C", "A", "A", "?"].iter().map(|s| s.to_string()).collect(),
            plot: vec![3.0, 4.0, 3.0, 5.0, 5.0, 3.0],
            prov: vec![false, true, true, true, false, false],
            unknown: vec![false, false, false, false, false, true],
            evidence: (0..6).map(|i| (format!("metric {i}"), format!("Film {i}"))).collect(),
            coverage: (0..6).map(|i| format!("graded on {} of 300 films", 200 + i)).collect(),
            ability: "Wind Reading. It reads the air and answers it: it agrees with the crowd on 89% of the 74,164 film pairs it can compare (difficulty-adjusted score 0.88).".into(),
            ability_name: "Wind Reading".into(),
            ability_line: "It reads the air and answers it: it agrees with the crowd on 89% of film pairs.".into(),
            credit: CREDIT.into(),
            weakness: "Soft spot: ordinary hands (Power C, “same as a human”). It rates 0.05★ below the crowd on average, about what a human would.".into(),
            nearest: vec!["D4C Love Train".into(), "Dolly Dagger".into(), "The World".into()],
            signature: "Signature film: Synthetic Film One (2008), 1.0★ against the crowd's 3.54★".into(),
            badge: true,
            accent: Some("PRC".into()),
        }
    }

    #[test]
    fn text_uses_f_prefixed_classes() {
        // Regression: gen_card.py line 121 emitted class="{cls}" so text lost its fill colour.
        let s = card_svg(&sample(), Theme::Auto);
        assert!(s.contains("class=\"f-ink\""));
        assert!(s.contains("class=\"f-bk\"") && s.contains("class=\"f-fill\""));
        for bad in ["class=\"ink\"", "class=\"bk\"", "class=\"mute\"", "class=\"gold\""] {
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
        let key = sample().accent;
        assert!(dk.contains(palette::dark(key.as_deref()).bg) && !dk.contains(palette::light(key.as_deref()).bg));
        assert!(l.starts_with("<svg") && l.ends_with("</svg>"));
        assert!(l.len() < 900_000, "{}", l.len());
    }

    #[test]
    fn very_long_names_are_cut_inside_the_frame() {
        let mut d = sample();
        d.name = "THE QUICK BROWN COMPOSER OF MANY LONG NAMES AND HOMAGES TO EVERYONE".into();
        let f = font(FontKind::Name);
        // the whole name does not fit the header even at the smallest size, so it is cut with an ellipsis and then does fit
        assert!(!name_fits(&name_box(&f, &format!("「{}」", d.name), NAME_MIN_SIZE)));
        let p = place_name(&f, &d.name);
        assert!(p.text.ends_with("…」") && p.size == NAME_MIN_SIZE && name_fits(&p), "{} at {}", p.text, p.size);
        assert!(card_svg(&d, Theme::Light).contains("class=\"f-ink\""));
    }

    #[test]
    fn every_name_in_names_json_fits_the_tilted_header_uncut() {
        let names: serde_json::Value = serde_json::from_str(include_str!("../data/names.json")).unwrap();
        let f = font(FontKind::Name);
        let mut all: Vec<(f64, String)> = vec![];
        for v in names["names"].as_object().unwrap().values() {
            for cand in v[1].as_array().unwrap() {
                let n = cand[0].as_str().unwrap().to_string();
                // the card draws the name in upper case, as names.json stores it; a fit at the smallest size is what "never clips" needs
                let b = name_box(&f, &format!("「{n}」"), NAME_MIN_SIZE);
                assert!(name_fits(&b), "{n:?} does not fit the header even at {NAME_MIN_SIZE} px");
                all.push((f.width(&n, NAME_MIN_SIZE, 2.0), n));
            }
        }
        assert!(all.len() > 20000);
        // the widest ones get their real size: never cut, never below the floor, and every corner of the tilted box inside the header
        all.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, n) in all.iter().take(100) {
            let p = place_name(&f, n);
            assert!(p.text == format!("「{n}」") && p.size >= NAME_MIN_SIZE && name_fits(&p), "{n}: {} at {}", p.text, p.size);
        }
        println!("the widest name in names.json, {:?}, is drawn at {} px", all[0].1, place_name(&f, &all[0].1).size);
    }

    #[test]
    fn the_name_is_the_one_tilt_and_the_fills_are_flat() {
        for theme in [Theme::Light, Theme::Dark, Theme::Auto] {
            let mut d = sample();
            for key in [None, Some("PWR"), Some("SPD"), Some("RNG"), Some("STA"), Some("PRC"), Some("DEV")] {
                d.accent = key.map(String::from);
                let s = card_svg(&d, theme);
                assert_eq!(s.matches("rotate(").count(), 1, "exactly one rotated element");
                assert_eq!(s.matches("transform=").count(), 1);
                // the tilted group is the name: its black outline path and its white fill path, nothing else
                let at = s.find("<g transform=").unwrap();
                let g = &s[at..at + s[at..].find("</g>").unwrap()];
                assert!(g.contains(&format!("rotate({NAME_TILT} ")) && g.matches("<path").count() == 2, "{}", &g[..60]);
                assert!(g.contains("class=\"o\"") && g.contains("class=\"f-paper\""));
                for banned in ["Gradient", "<pattern", "<filter", "filter:", "shadow", "<image", "<mask", "blur", "opacity"] {
                    assert!(!s.contains(banned), "{banned}");
                }
            }
        }
    }

    #[test]
    fn the_lettering_has_the_manga_outline() {
        // every outlined text is a black path (class "o", with a stroke width) followed by the fill path of the same glyphs: the name
        // and each ゴ (3 of them)
        let s = card_svg(&sample(), Theme::Light);
        assert_eq!(s.matches("class=\"o\"").count(), 1 + 3);
        let mut pairs = 0;
        let parts: Vec<&str> = s.split("<path d=\"").skip(1).collect();
        for (k, part) in parts.iter().enumerate() {
            let (d, rest) = part.split_once('"').unwrap();
            if rest.starts_with(" class=\"o\" stroke-width=\"") {
                assert!(parts[k + 1].starts_with(&format!("{d}\" class=\"f-")), "the fill path of an outline must follow it, with the same glyphs");
                pairs += 1;
            }
        }
        assert_eq!(pairs, 4);
        assert!(s.contains(".o{fill:#0b0b0b;stroke:#0b0b0b;stroke-linejoin:round}"));
    }

    #[test]
    fn the_card_takes_the_users_ground_and_second_colour() {
        let mut d = sample();
        d.accent = Some("PRC".into());
        let s = card_svg(&d, Theme::Light);
        let p = palette::light(Some("PRC"));
        assert!(s.contains(&format!(".f-bg{{fill:{}}}", p.bg)) && s.contains(&format!(".f-fill{{fill:{}}}", p.fill)));
        // the radar is a flat fill of the second colour with a black outline: no pattern, no gradient, no opacity
        assert!(s.contains("<polygon") && s.contains("class=\"f-fill s-bk r\""));
        for gone in ["gold", "f-red", "f-acc", "f-pair", "radar", "#9A7B00", "#C8200E", "tone", "<circle cx=\"540\" cy=\"510.0\" r=\"138"] {
            assert!(!s.contains(gone), "{gone}");
        }
        // a different signature gives a different card, in the dark card a different ground again
        d.accent = Some("DEV".into());
        assert_ne!(card_svg(&d, Theme::Light), s);
        assert!(card_svg(&d, Theme::Dark).contains(&format!(".f-bg{{fill:{}}}", palette::dark(Some("DEV")).bg)));
    }

    #[test]
    fn the_chart_is_the_part_5_hexagon() {
        // hexagonal plate and rings (7 polygons with the value polygon), one circled grade per vertex
        let s = card_svg(&sample(), Theme::Light);
        assert_eq!(s.matches("<polygon").count(), 1 + 5 + 1);
        assert!(!s.contains("stroke-dasharray=\"6 5\"") || sample().unknown.iter().any(|u| *u));
        assert_eq!(s.matches(&format!("r=\"{BADGE_R}\" class=\"f-paper s-bk\"")).count(), 6, "six circled grades");
        // the vertices are where the Part 5 chart puts them: Power on top, Speed upper right, Range lower right, Stamina below,
        // Precision lower left, Potential upper left (clockwise from the top)
        let v = hex(540.0, 100.0, 100.0);
        assert!((v[0].0 - 540.0).abs() < 1e-9 && v[0].1 < 100.0 && v[3].1 > 100.0);
        assert!(v[1].0 > 540.0 && v[1].1 < 100.0 && v[2].0 > 540.0 && v[2].1 > 100.0);
        assert!(v[4].0 < 540.0 && v[4].1 > 100.0 && v[5].0 < 540.0 && v[5].1 < 100.0);
    }

    #[test]
    fn text_fits_the_frame() {
        // wrapping keeps lines inside the box
        let f = body_text();
        for ln in f.wrap(&sample().ability_line, LINE_SIZE, LINE_WIDTH) {
            assert!(f.width(&ln, LINE_SIZE, 0.0) <= LINE_WIDTH + 0.01);
        }
        assert!(f.fit("a very long film title ".repeat(10).as_str(), 19.0, 440.0).ends_with('…'));
    }

    /// The text the card draws, in the order it is drawn (the Stand name in its 「」).
    fn texts(d: &CardData) -> Vec<String> {
        card_build(d, Theme::Light).1.into_iter().map(|r| r.text).collect()
    }

    #[test]
    fn the_card_keeps_only_the_listed_content() {
        let d = sample();
        let t = texts(&d);
        let all = t.join("\n");
        // kept: the Stand name, the Stand master, the type in Japanese and English, six grades with their asterisks, the ability name and
        // sentence, the nearest canon line and the one footer line
        assert!(t.iter().any(|s| s == "「DIRTY WEATHER」") && all.contains("本体―") && all.contains("jas3344"));
        assert!(all.contains("近距離パワー型") && all.contains("Close-Range Power"));
        assert!(t.iter().any(|s| s == "B*") && t.iter().any(|s| s == "C") && t.iter().any(|s| s == "?"), "{t:?}");
        assert!(t.iter().any(|s| s == "Wind Reading") && all.replace('\n', " ").contains(&d.ability_line));
        assert!(t.iter().any(|s| s == "Nearest canon: D4C Love Train · Dolly Dagger · The World"));
        assert_eq!(t.last().unwrap(), "standuser.pages.dev · Fan work, not affiliated · * provisional");
        // removed: stat value lines, film titles, graded-on lines, weakness, signature film, why, archetype caveat, PROVISIONAL DATA, credit
        for gone in ["metric 0", "Film 0", "graded on", "Soft spot", "Signature film", "Why", "archetype", "PROVISIONAL", "Data:", "Letterboxd", "TMDB", "MusicBrainz", "A―", "超スゴイ"] {
            assert!(!all.contains(gone), "{gone} is still on the card");
        }
        // no hidden text: every grade and label is one of the listed runs
        assert_eq!(t.iter().filter(|s| JP.contains(&s.as_str())).count(), 6);
    }

    #[test]
    fn the_footer_is_the_one_line() {
        assert_eq!(FOOTER, "standuser.pages.dev · Fan work, not affiliated · * provisional");
        assert!(!FOOTER.contains('\n') && !FOOTER.contains("endorsed") && !FOOTER.contains("Data:"));
    }

    #[test]
    fn the_hexagon_is_at_least_55_percent_of_the_card_width() {
        let s = card_svg(&sample(), Theme::Light);
        // the plate is the first polygon: read its points back
        let at = s.find("<polygon points=\"").unwrap() + "<polygon points=\"".len();
        let pts: Vec<(f64, f64)> = s[at..at + s[at..].find('"').unwrap()]
            .split(' ')
            .map(|p| {
                let (x, y) = p.split_once(',').unwrap();
                (x.parse().unwrap(), y.parse().unwrap())
            })
            .collect();
        let (min, max) = (pts.iter().map(|p| p.0).fold(f64::MAX, f64::min), pts.iter().map(|p| p.0).fold(f64::MIN, f64::max));
        assert!(max - min >= 0.55 * W, "the hexagon is {:.1}% of the card's width", 100.0 * (max - min) / W);
        assert!((3.0f64).sqrt() * CHART_R >= 0.55 * W);
    }

    #[test]
    fn the_stand_name_is_larger_than_it_was() {
        // the old card capped the name at 80 px and drew the median name in names.json at that cap; every name that was at the cap is
        // now larger, and the median name is well above it
        let f = font(FontKind::Name);
        let names: serde_json::Value = serde_json::from_str(include_str!("../data/names.json")).unwrap();
        let mut sizes: Vec<f64> = vec![];
        for v in names["names"].as_object().unwrap().values() {
            for cand in v[1].as_array().unwrap().iter().take(1) {
                sizes.push(place_name(&f, cand[0].as_str().unwrap()).size);
            }
        }
        sizes.sort_by(|a, b| a.total_cmp(b));
        let median = sizes[sizes.len() / 2];
        println!("{} names, drawn at {} px at the smallest, {median} px at the median, {} px at the largest", sizes.len(), sizes[0], sizes[sizes.len() - 1]);
        assert!(median >= 85.0 && sizes[0] >= NAME_MIN_SIZE, "median {median}");
    }

    #[test]
    fn every_text_is_in_the_face_its_role_names() {
        // Archivo Black for the Stand name and the Latin display text; Archivo 700 for all other Latin; Dela Gothic One with the black
        // outline for Japanese at 48 px or larger; Noto Sans JP Bold, no outline, for Japanese below 48 px. No other face.
        let mut d = sample();
        d.grades[5] = "完成".into();
        for theme in [Theme::Light, Theme::Dark] {
            for run in card_build(&d, theme).1 {
                let japanese = |c: char| c as u32 >= 0x2E80 && c != '「' && c != '」';
                let all_jp = run.text.chars().all(|c| japanese(c) || c == '―');
                match run.kind {
                    FontKind::Display => assert!(run.size >= JP_LETTERING_MIN && run.outlined && all_jp, "{run:?}"),
                    FontKind::Jp => assert!(run.size < JP_LETTERING_MIN && !run.outlined && all_jp, "{run:?}"),
                    FontKind::Name => assert!(!run.text.chars().any(japanese) || run.text.starts_with('「'), "{run:?}"),
                    FontKind::Body => assert!(!run.outlined && !run.text.chars().any(|c| c as u32 >= 0x2E80), "{run:?}"),
                }
            }
        }
        // the roles of the pieces the rule names
        let runs = card_build(&d, Theme::Light).1;
        let kind = |t: &str| runs.iter().find(|r| r.text.starts_with(t)).unwrap_or_else(|| panic!("{t}")).kind;
        assert_eq!(kind("「DIRTY"), FontKind::Name);
        assert_eq!(kind("Wind Reading"), FontKind::Name);
        assert_eq!(kind("B*"), FontKind::Name);
        assert_eq!(kind("It reads"), FontKind::Body);
        assert_eq!(kind("standuser"), FontKind::Body);
        assert_eq!(kind("Close-Range"), FontKind::Body);
        assert_eq!(kind("ゴ"), FontKind::Display);
        assert_eq!(kind("近距離"), FontKind::Jp);
        assert_eq!(kind("本体"), FontKind::Jp);
        assert_eq!(kind("完成"), FontKind::Jp);
    }

    #[test]
    fn no_text_on_the_card_is_below_34_px() {
        // 34 px in card units reads at 390 px phone width: every run of every card, whatever it says
        let mut d = sample();
        for (name, user, line) in [
            ("A", "x", "Short."),
            ("YORKTOWN AND THE RETURN HOME", "a_user_name_15ch", "It unravels into tendrils that cross borders: 3.4 effective production countries per 50 films, and then some more."),
            ("THE QUICK BROWN COMPOSER OF MANY LONG NAMES AND HOMAGES TO EVERYONE", "u", ""),
        ] {
            d.name = name.into();
            d.user = user.into();
            d.ability_line = line.into();
            d.grades[2] = "完成".into();
            for theme in [Theme::Light, Theme::Dark] {
                let runs = card_build(&d, theme).1;
                assert!(runs.len() > 20);
                for r in &runs {
                    assert!(r.size >= MIN_TEXT, "{r:?} is below {MIN_TEXT} px");
                }
            }
        }
        assert_eq!(MIN_TEXT, 34.0);
    }

    #[test]
    fn the_nearest_canon_line_drops_whole_names_never_cuts_one() {
        let f = body_text();
        let names: Vec<String> = ["Dirty Deeds Done Dirt Cheap", "Highway to Hell", "Echoes ACT3 (Part 5)"].map(String::from).into();
        let line = nearest_line(&f, &names);
        assert!(f.width(&line, NEAR_SIZE, 0.0) <= RIGHT - MARGIN && line.starts_with("Nearest canon: Dirty Deeds Done Dirt Cheap"), "{line}");
        assert!(!line.contains('…') && (line.ends_with("Cheap") || line.ends_with("Hell") || line.ends_with("(Part 5)")), "{line}");
        // all three fit when they are short
        let short: Vec<String> = ["Kiss", "Cream", "Oasis"].map(String::from).into();
        assert_eq!(nearest_line(&f, &short), "Nearest canon: Kiss · Cream · Oasis");
    }

    #[test]
    fn every_ability_sentence_fits_its_rows_at_34_px() {
        let f = body_text();
        for line in crate::ability::all_card_lines() {
            let rows = f.wrap(&line, LINE_SIZE, LINE_WIDTH);
            assert!(rows.len() <= 2, "{} rows: {line}", rows.len());
            assert!(rows.len() <= LINE_MAX_ROWS);
        }
    }

    #[test]
    fn the_gogogo_are_three_staggered_on_a_diagonal_clear_of_all_text() {
        for theme in [Theme::Light, Theme::Dark] {
            let mut d = sample();
            d.name = "YORKTOWN AND THE RETURN HOME".into();
            let runs = card_build(&d, theme).1;
            let go: Vec<&Run> = runs.iter().filter(|r| r.text == "ゴ").collect();
            assert_eq!(go.len(), 3, "three ゴ");
            // three sizes, each smaller than the one before, all at least the lettering size
            assert!(go[0].size > go[1].size && go[1].size > go[2].size && go[2].size >= JP_LETTERING_MIN, "{:?}", go.iter().map(|r| r.size).collect::<Vec<_>>());
            // on a diagonal (each lower and further right) and staggered (no two share a column or a row)
            let centre = |r: &Run| ((r.bbox[0] + r.bbox[2]) / 2.0, (r.bbox[1] + r.bbox[3]) / 2.0);
            for w in go.windows(2) {
                let (a, b) = (centre(w[0]), centre(w[1]));
                assert!(b.0 - a.0 >= 15.0 && b.1 - a.1 >= w[0].size / 2.0 + w[1].size / 2.0 - 40.0 && b.1 > a.1, "{a:?} {b:?}");
            }
            // clear of every other piece of text by 10 px, and of the hexagon (its plate and its circled grades)
            let apart = |a: &[f64; 4], b: &[f64; 4], pad: f64| a[2] + pad <= b[0] || b[2] + pad <= a[0] || a[3] + pad <= b[1] || b[3] + pad <= a[1];
            for g in &go {
                for r in runs.iter().filter(|r| r.text != "ゴ") {
                    assert!(apart(&g.bbox, &r.bbox, 10.0), "ゴ {:?} touches {:?}", g.bbox, r);
                }
                assert!(g.bbox[2] + 10.0 <= CX - 3.0f64.sqrt() / 2.0 * CHART_R, "ゴ {:?} reaches the plate", g.bbox);
                for i in [4usize, 5] {
                    let a = (i as f64 * 60.0).to_radians();
                    let (bx, by) = (CX + CHART_R * a.sin(), CY - CHART_R * a.cos());
                    assert!(apart(&g.bbox, &[bx - BADGE_R, by - BADGE_R, bx + BADGE_R, by + BADGE_R], 10.0), "ゴ {:?} touches a grade", g.bbox);
                }
            }
        }
    }

    #[test]
    fn the_card_has_one_frame_and_everything_is_inside_it() {
        let d = sample();
        for theme in [Theme::Light, Theme::Dark] {
            let (s, runs) = card_build(&d, theme);
            // one frame: a single unfilled stroked rectangle, the thick black border, flush with the card's edge
            let frames: Vec<&str> = s.split("<rect ").skip(1).filter(|r| r.contains("fill=\"none\"")).collect();
            assert_eq!(frames.len(), 1, "{frames:?}");
            let f = frames[0];
            assert!(f.contains(&format!("x=\"{}\" y=\"{}\"", FRAME / 2.0, FRAME / 2.0)) && f.contains(&format!("width=\"{}\" height=\"{}\"", W - FRAME, H - FRAME)), "{f}");
            assert!(f.contains("class=\"s-bk\"") && f.contains(&format!("stroke-width=\"{FRAME}\"")) && !f.contains("rx="), "{f}");
            // no outer margin (the ground fills the card to its edge) and no thin inner line
            assert!(s.contains(&format!("<rect width=\"{W}\" height=\"{H}\" class=\"f-bg\"/>")));
            assert!(!s.contains("class=\"s-ink\""), "no thin line of the ink colour");
            // the border is thick, and all text is inside it
            assert!(FRAME >= 16.0);
            for r in &runs {
                let b = r.bbox;
                assert!(b[0] >= FRAME && b[1] >= FRAME && b[2] <= W - FRAME && b[3] <= H - FRAME, "{r:?} reaches the frame");
            }
        }
    }
}
