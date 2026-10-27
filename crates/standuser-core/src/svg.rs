//! Stand card as one self-contained SVG (port of swarm/round3/card/gen_card.py, text-as-outlines mode).
//!
//! All text is drawn as paths with class="f-{cls}" (fill colour from the palette CSS), so it renders identically in
//! browsers, GitHub READMEs and PNG renderers. Fixes gen_card.py line 121, where text lost its `f-` prefix.
//! The look is the manga's: a flat saturated cover colour for the ground, heavy lettering (Dela Gothic One for the stat labels and
//! ゴゴゴ, Archivo Black for the grades) with a black outline drawn as a second, thicker black path under the fill path, and the
//! Part 5 stat chart: a hexagon with a ring per grade and a grade (outlined lettering) at each vertex. The header follows the anime
//! eyecatch: 【STAND NAME】 and the name in Noto Serif JP Bold, level and plain, top left; 【STAND MASTER】 and the user bottom right.

use crate::font::{body_text, font, serif_text, BodyText, Draw, Font, FontKind, SerifText};
use crate::palette::{self, Pal};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

pub const W: f64 = 1080.0;
pub const H: f64 = 1350.0;

pub const JP: [&str; 6] = ["破壊力", "スピード", "射程距離", "持続力", "精密動作性", "成長性"];
pub const EN: [&str; 6] = ["Destructive Power", "Speed", "Range", "Stamina", "Precision", "Potential"];
/// The one footer line at the bottom of the card; the data credit and the TMDB disclaimer are on the page.
pub const FOOTER: &str = "standuser.pages.dev · Fan work, not affiliated · * provisional";
/// The footer as drawn: two rows at the bottom left, beside the Stand master (a test checks they say FOOTER).
pub const FOOTER_ROWS: [&str; 2] = ["standuser.pages.dev · Fan work,", "not affiliated · * provisional"];
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
    /// Noto Serif JP Bold: the Stand name, the master and their labels.
    serif: SerifText,
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
            FontKind::Serif => &self.serif,
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

    /// A 【label】 in the serif, set so the ink of its bracket (not the blank beside it) is at x: the left edge for "start", the right
    /// edge for "end". The run's box is the ink's.
    fn bracket_label(&mut self, s: &str, x: f64, y: f64, size: f64, anchor: &str) {
        let (lb, rb) = (self.serif.bearings('【', size).0, self.serif.bearings('】', size).1);
        let w = self.serif.width(s, size, 0.0);
        let x0 = if anchor == "end" { x + rb - w } else { x - lb };
        self.text(s, x0, y, size, FontKind::Serif, "ink", "start", 0.0);
        let r = self.runs.last_mut().unwrap();
        r.bbox[0] += lb;
        r.bbox[2] -= rb;
    }

    /// One ゴ of the cascade: outlined lettering centred on (cx, cy), tilted about its centre. Its run's box is that of the tilted ink.
    fn gogo(&mut self, size: f64, cx: f64, cy: f64) {
        let (up, down) = self.display.ink_extent("ゴ", size);
        let w = self.display.width("ゴ", size, 0.0);
        let (x0, y) = (cx - w / 2.0, cy + (up - down) / 2.0);
        let d = self.display.path("ゴ", x0, y, size, 0.0);
        let (hw, hh) = (w / 2.0 + OUTLINE / 2.0, (up + down) / 2.0 + OUTLINE / 2.0);
        let th = GOGOGO_TILT.to_radians();
        let (ex, ey) = (hw * th.cos().abs() + hh * th.sin().abs(), hw * th.sin().abs() + hh * th.cos().abs());
        self.runs.push(Run { text: "ゴ".into(), size, kind: FontKind::Display, outlined: true, rotated: true, bbox: [cx - ex, cy - ey, cx + ex, cy + ey] });
        self.els.push(format!("<g data-part=\"go\"><g transform=\"rotate({GOGOGO_TILT} {} {})\"><path d=\"{d}\" class=\"o\" stroke-width=\"{OUTLINE}\"/><path d=\"{d}\" class=\"f-fill\"/></g></g>", n1(cx), n1(cy)));
    }
}

/// The chart: centre height and the hexagon's circumradius (the ∞ ring; a grade E to A is the ring k of 6 for k = 1 to 5). The hexagon
/// is 55% of the card's width or more: its plate is √3 · CHART_R wide.
pub const CX: f64 = W / 2.0;
pub const CY: f64 = 664.0;
pub const CHART_R: f64 = 346.0;
/// The grade sits on each vertex as lettering: Archivo Black with the black outline, the largest lettering on the card after the Stand
/// name. Its size, the size of the 完成 grade (Dela Gothic One), and the size of the JP label beside it.
pub const GRADE_SIZE: f64 = 80.0;
pub const GRADE_JP_SIZE: f64 = 72.0;
const LABEL_SIZE: f64 = MIN_TEXT;
/// Gap between a grade and its label.
const LABEL_GAP: f64 = 12.0;

/// The thick black border: its width, drawn inward from the card's edge.
pub const FRAME: f64 = 16.0;

/// No text on the card is smaller than this, in card units (the card is 1080 wide): at a phone's 390 px width it is still about 12 px.
pub const MIN_TEXT: f64 = 34.0;

/// The left edge of the text, and the right edge.
pub const MARGIN: f64 = 48.0;
pub const RIGHT: f64 = W - MARGIN;
/// The eyecatch's two labels, in capitals, in the serif.
pub const NAME_LABEL: &str = "【STAND NAME】";
pub const MASTER_LABEL: &str = "【STAND MASTER】";
/// The Stand master is the bottom right corner: its label above the user name, both right-aligned to the margin, in a box this wide.
pub const MASTER_W: f64 = 420.0;
const MASTER_LABEL_Y: f64 = 1262.0;
const MASTER_USER_Y: f64 = 1312.0;
const MASTER_USER_SIZE: f64 = 44.0;
const TYPE_SIZE: f64 = MIN_TEXT;
const TYPE_JP_Y: f64 = 220.0;
const TYPE_EN_Y: f64 = 266.0;

/// The bottom of the card is laid out from the foot up: the bottom band (the footer at the left in two rows, the Stand master at the
/// right), the nearest canon line, the ability's sentence (at most 3 rows) and above it the ability's name.
const LINE_SIZE: f64 = MIN_TEXT;
const LINE_PITCH: f64 = 42.0;
const LINE_WIDTH: f64 = RIGHT - MARGIN;
const LINE_MAX_ROWS: usize = 3;
const ABILITY_SIZE: f64 = 44.0;
pub const FOOT_Y: f64 = 1312.0;
pub const FOOT_PITCH: f64 = 50.0;
/// The footer is wrapped to the left of the master's box.
pub const FOOT_W: f64 = RIGHT - MARGIN - MASTER_W - 24.0;
pub const NEAR_Y: f64 = 1212.0;
const NEAR_SIZE: f64 = MIN_TEXT;
/// Baseline of the last row of the sentence, and of the ability's name above the first row.
const LINE_LAST_Y: f64 = NEAR_Y - 48.0;
const ABILITY_RISE: f64 = 50.0;

/// Where the ability's name and its rows of sentence stand: (name baseline, first row baseline).
pub fn ability_rows(rows: usize) -> (f64, f64) {
    let first = LINE_LAST_Y - (rows.clamp(1, LINE_MAX_ROWS) as f64 - 1.0) * LINE_PITCH;
    (first - ABILITY_RISE, first)
}

/// The ゴゴゴ: four ゴ in one diagonal cascade at 36 degrees from horizontal (local/ref/004385f76965c1727c35e5134bc5fe89.jpg), each smaller than
/// the one before (a Dela Gothic One ゴ is one em wide): (size, centre x, centre y). They stand in the gap between the Stand name's block
/// and the hexagon's upper left, clear of every text and of the plate by 10 px. Four is what the gap holds at that angle: a line down to the
/// right from the card's top left meets the hexagon's upper-left edge after about 300 px, the 成長性 label stands under it, and the glyphs
/// may not shrink below 40 px. A test (`the_gogogo_are_one_shallow_cascade_in_the_gap_clear_of_all_text`) checks all this on 14 grounds.
pub const GOGOGO: [(f64, f64, f64); 4] = [(60.0, 92.0, 243.0), (54.0, 168.9, 298.8), (48.0, 239.2, 350.0), (42.0, 302.3, 395.8)];
/// The cascade's one tilt, in degrees as SVG's rotate() takes them (negative climbs to the right): the slant of the manga's sound effects.
/// It is the only rotation on the card; the slant belongs to series logos and sound effects, not to a Stand name.
pub const GOGOGO_TILT: f64 = -8.0;
/// Width of the black outline of the display lettering, in px (half of it shows outside the glyph).
pub const OUTLINE: f64 = 6.0;

/// The Stand name follows the anime eyecatch (local/ref/iuhipmy89x781.png): level, Noto Serif JP Bold, plain (no outline, no shadow, no
/// tilt), at the top left under 【STAND NAME】. Its baseline is one fixed line; its size is the largest from NAME_MAX_SIZE down to
/// NAME_MIN_SIZE that keeps it inside the margins, and the label sits just over its tallest letter.
pub const NAME_BASELINE: f64 = 170.0;
pub const NAME_MAX_SIZE: f64 = 104.0;
pub const NAME_MIN_SIZE: f64 = 40.0;
pub const NAME_WIDTH: f64 = RIGHT - MARGIN;
const LABEL_SIZE_HEAD: f64 = MIN_TEXT;
/// Between the label's lowest ink and the name's highest.
const HEAD_GAP: f64 = 12.0;

/// Where the name goes: its text, its size, its baseline start (x, y), and the baseline of the label above it.
pub struct NamePlace {
    pub text: String,
    pub size: f64,
    pub x: f64,
    pub y: f64,
    pub label_y: f64,
}

fn name_at(f: &SerifText, text: &str, size: f64) -> NamePlace {
    let (up, _) = f.ink_extent(text, size);
    let (_, label_down) = f.ink_extent(NAME_LABEL, LABEL_SIZE_HEAD);
    NamePlace { text: text.to_string(), size, x: MARGIN, y: NAME_BASELINE, label_y: NAME_BASELINE - up - HEAD_GAP - label_down }
}

pub fn name_fits(f: &SerifText, text: &str, size: f64) -> bool {
    f.width(text, size, 0.0) <= NAME_WIDTH
}

/// The largest size from NAME_MAX_SIZE down to NAME_MIN_SIZE at which the name fits the line. Only a name that does not fit even at the
/// smallest size is cut, with an ellipsis; none in names.json is (a test checks every one).
pub fn place_name(f: &SerifText, name: &str) -> NamePlace {
    let mut size = NAME_MAX_SIZE;
    while size >= NAME_MIN_SIZE {
        if name_fits(f, name, size) {
            return name_at(f, name, size);
        }
        size -= 1.0;
    }
    let cut = f.fit(name, NAME_MIN_SIZE, NAME_WIDTH);
    name_at(f, &cut, NAME_MIN_SIZE)
}

/// Where the master stands, as a box (left, top, right, bottom), and the type lines: the widest master the card draws and the widest type
/// lines (Automatic, in Japanese and in English). A test checks the real text is inside them.
pub const MASTER_BOX: [f64; 4] = [RIGHT - MASTER_W, MASTER_LABEL_Y - 34.0, RIGHT, MASTER_USER_Y + 14.0];
pub const TYPE_JP_BOX: [f64; 4] = [RIGHT - 250.0, TYPE_JP_Y - 32.0, RIGHT, TYPE_JP_Y + 8.0];
pub const TYPE_EN_BOX: [f64; 4] = [RIGHT - 600.0, TYPE_EN_Y - 28.0, RIGHT, TYPE_EN_Y + 10.0];

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
    use FontKind::{Body, Name, Serif};
    let mut c = Ctx { jp: font(FontKind::Jp), body: body_text(), name: font(Name), display: font(FontKind::Display), serif: serif_text(), els: vec![], runs: vec![] };
    let (cx, cy, r_chart) = (CX, CY, CHART_R);

    // the ground, and the one frame: the thick black border of a manga panel, flush with the card's edge (no margin outside it, no
    // line inside it)
    c.els.push(format!("<rect width=\"{W}\" height=\"{H}\" class=\"f-bg\"/>"));
    c.els.push(format!("<rect x=\"{0}\" y=\"{0}\" width=\"{1}\" height=\"{2}\" class=\"s-bk\" fill=\"none\" stroke-width=\"{FRAME}\"/>", FRAME / 2.0, W - FRAME, H - FRAME));
    // the ゴゴゴ of the scene: one diagonal cascade down the left side
    for (sz, gx, gy) in GOGOGO {
        c.gogo(sz, gx, gy);
    }
    // the Stand name as in the eyecatch: 【STAND NAME】 over it, both level, plain, in the serif
    let np = place_name(&c.serif, &d.name);
    // the brackets carry blank at their sides: the labels are set so the ink of the bracket lines up with the margin
    c.els.push("<g data-part=\"name\">".into());
    c.bracket_label(NAME_LABEL, MARGIN, np.label_y, LABEL_SIZE_HEAD, "start");
    c.text(&np.text, np.x, np.y, np.size, Serif, "ink", "start", 0.0);
    c.els.push("</g>".into());
    // the type in Japanese and English, right, under the name
    c.jp_text(&d.type_jp, RIGHT, TYPE_JP_Y, TYPE_SIZE, "ink", "end");
    c.text(&d.type_en, RIGHT, TYPE_EN_Y, TYPE_SIZE, Body, "ink", "end", 0.0);

    // the chart, after the Part 5 tailpiece: a paper hexagon, a ring per grade, three spokes
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
    // the value polygon and its "?" marks are one part (data-part="fill"): the page's reveal grows it from the centre
    c.els.push("<g data-part=\"fill\">".into());
    c.els.push(format!("<polygon points=\"{}\" class=\"f-fill s-bk r\" stroke-width=\"3.5\"/>", pts(&vals)));
    for i in 0..6 {
        if d.unknown[i] {
            for j in [(i + 5) % 6, (i + 1) % 6] {
                c.els.push(format!("<path d=\"M{:.1} {:.1}L{:.1} {:.1}\" class=\"s-bk\" stroke-width=\"2\" stroke-dasharray=\"6 5\" fill=\"none\"/>", vals[i].0, vals[i].1, vals[j].0, vals[j].1));
            }
        }
    }
    for i in 0..6 {
        if d.unknown[i] {
            c.els.push(format!("<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"7\" class=\"f-paper s-bk\" stroke-width=\"2.5\"/>", vals[i].0, vals[i].1));
        }
    }
    c.els.push("</g>".into());
    // the grade on each vertex (outlined lettering, centred on the vertex by its letter; an asterisk hangs to the right of the letter), and
    // the label beside it: to the right at the top and the bottom, above or below the grade at the four side vertices, so the label stays
    // off the card's edge (破壊力 top, スピード upper right, 射程距離 lower right, 持続力 bottom, 精密動作性 lower left, 成長性 upper left)
    for i in 0..6 {
        let a = (i as f64 * 60.0).to_radians();
        let (bx, by) = (cx + r_chart * a.sin(), cy - r_chart * a.cos());
        let letter = d.grades[i].as_str();
        let g = format!("{letter}{}", if d.prov[i] { "*" } else { "" });
        let (kind, size) = if letter == "完成" { (FontKind::Display, GRADE_JP_SIZE) } else { (Name, GRADE_SIZE) };
        let (lw, w) = (c.face(kind).width(letter, size, 0.0), c.face(kind).width(&g, size, 0.0));
        let (up, down) = c.face(kind).ink_extent(&g, size);
        let y = by + (up - down) / 2.0;
        let x0 = bx - lw / 2.0;
        // each grade is a part (data-part="grade", data-i clockwise from the top): the page's reveal pops them in that order
        c.els.push(format!("<g data-part=\"grade\" data-i=\"{i}\">"));
        c.lettering(&g, x0, y, size, kind, "fill", "start", 0.0, OUTLINE);
        c.els.push("</g>".into());
        let (top, bottom, right) = (y - up - OUTLINE / 2.0, y + down + OUTLINE / 2.0, x0 + w + OUTLINE / 2.0);
        let (lup, ldown) = c.jp.ink_extent(JP[i], LABEL_SIZE);
        let mid = (lup - ldown) / 2.0;
        match i {
            0 | 3 => c.jp_text(JP[i], right + LABEL_GAP, by + mid, LABEL_SIZE, "ink", "start"),
            1 => c.jp_text(JP[i], x0, top - LABEL_GAP, LABEL_SIZE, "ink", "start"),
            2 => c.jp_text(JP[i], x0, bottom + LABEL_GAP + lup, LABEL_SIZE, "ink", "start"),
            4 => c.jp_text(JP[i], x0 + lw, bottom + LABEL_GAP + lup, LABEL_SIZE, "ink", "end"),
            _ => c.jp_text(JP[i], x0 + lw, top - LABEL_GAP, LABEL_SIZE, "ink", "end"),
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
    // the nearest canon line, then the bottom band: the footer at the left in two rows, the Stand master at the right
    let near = nearest_line(&c.body, &d.nearest);
    c.text(&near, MARGIN, NEAR_Y, NEAR_SIZE, Body, "ink", "start", 0.0);
    for (i, ln) in FOOTER_ROWS.iter().enumerate() {
        c.text(ln, MARGIN, FOOT_Y - (FOOTER_ROWS.len() - 1 - i) as f64 * FOOT_PITCH, NEAR_SIZE, Body, "mute", "start", 0.0);
    }
    c.bracket_label(MASTER_LABEL, RIGHT, MASTER_LABEL_Y, LABEL_SIZE_HEAD, "end");
    let user = c.serif.fit(&d.user, MASTER_USER_SIZE, MASTER_W);
    c.text(&user, RIGHT, MASTER_USER_Y, MASTER_USER_SIZE, Serif, "ink", "end", 0.0);

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
        assert!(s.contains("class=\"f-fill\""));
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
        let f = serif_text();
        // the whole name does not fit the line even at the smallest size, so it is cut with an ellipsis and then does fit
        assert!(!name_fits(&f, &d.name, NAME_MIN_SIZE));
        let p = place_name(&f, &d.name);
        assert!(p.text.ends_with('…') && p.size == NAME_MIN_SIZE && name_fits(&f, &p.text, p.size), "{} at {}", p.text, p.size);
        assert!(card_svg(&d, Theme::Light).contains("class=\"f-ink\""));
    }

    #[test]
    fn every_name_in_names_json_fits_the_header_uncut() {
        let names: serde_json::Value = serde_json::from_str(include_str!("../data/names.json")).unwrap();
        let f = serif_text();
        let mut all: Vec<(f64, String)> = vec![];
        for v in names["names"].as_object().unwrap().values() {
            for cand in v[1].as_array().unwrap() {
                let n = cand[0].as_str().unwrap().to_string();
                // the card draws the name in upper case, as names.json stores it; a fit at the smallest size is what "never clipped" needs
                assert!(name_fits(&f, &n, NAME_MIN_SIZE), "{n:?} does not fit the line even at {NAME_MIN_SIZE} px");
                all.push((f.width(&n, NAME_MIN_SIZE, 0.0), n));
            }
        }
        assert!(all.len() > 20000);
        // the widest ones get their real size: never cut, never below the floor
        all.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, n) in all.iter().take(100) {
            let p = place_name(&f, n);
            assert!(p.text == *n && p.size >= NAME_MIN_SIZE && name_fits(&f, &p.text, p.size), "{n}: {} at {}", p.text, p.size);
        }
        println!("the widest name in names.json, {:?}, is drawn at {} px", all[0].1, place_name(&f, &all[0].1).size);
    }

    #[test]
    fn only_the_gogogo_tilt_and_the_fills_are_flat() {
        for theme in [Theme::Light, Theme::Dark, Theme::Auto] {
            let mut d = sample();
            for key in [None, Some("PWR"), Some("SPD"), Some("RNG"), Some("STA"), Some("PRC"), Some("DEV")] {
                d.accent = key.map(String::from);
                let s = card_svg(&d, theme);
                // the six ゴ are the only rotated elements: each a group of its black outline path and its fill path
                assert_eq!(s.matches("rotate(").count(), GOGOGO.len(), "one rotated group per ゴ");
                assert_eq!(s.matches("transform=").count(), GOGOGO.len());
                for g in s.split("<g transform=").skip(1) {
                    let g = &g[..g.find("</g>").unwrap()];
                    assert!(g.starts_with(&format!("\"rotate({GOGOGO_TILT} ")) && g.matches("<path").count() == 2, "{}", &g[..40]);
                    assert!(g.contains("class=\"o\"") && g.contains("class=\"f-fill\""));
                }
                for banned in ["Gradient", "<pattern", "<filter", "filter:", "shadow", "<image", "<mask", "blur", "opacity"] {
                    assert!(!s.contains(banned), "{banned}");
                }
            }
        }
    }

    #[test]
    fn the_lettering_has_the_manga_outline() {
        // every outlined text is a black path (class "o", with a stroke width) followed by the fill path of the same glyphs: each ゴ (6 of
        // them) and each of the six grades. The Stand name is plain serif, with no outline.
        let s = card_svg(&sample(), Theme::Light);
        assert_eq!(s.matches("class=\"o\"").count(), GOGOGO.len() + 6);
        let mut pairs = 0;
        let parts: Vec<&str> = s.split("<path d=\"").skip(1).collect();
        for (k, part) in parts.iter().enumerate() {
            let (d, rest) = part.split_once('"').unwrap();
            if rest.starts_with(" class=\"o\" stroke-width=\"") {
                assert!(parts[k + 1].starts_with(&format!("{d}\" class=\"f-")), "the fill path of an outline must follow it, with the same glyphs");
                pairs += 1;
            }
        }
        assert_eq!(pairs, GOGOGO.len() + 6);
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
        // hexagonal plate and rings (7 polygons with the value polygon), one grade per vertex
        let s = card_svg(&sample(), Theme::Light);
        assert_eq!(s.matches("<polygon").count(), 1 + 5 + 1);
        assert!(!s.contains("stroke-dasharray=\"6 5\"") || sample().unknown.iter().any(|u| *u));
        assert_eq!(s.matches("<circle").count(), sample().unknown.iter().filter(|u| **u).count(), "no circles but the ? markers");
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

    /// The text the card draws, in the order it is drawn.
    fn texts(d: &CardData) -> Vec<String> {
        card_build(d, Theme::Light).1.into_iter().map(|r| r.text).collect()
    }

    #[test]
    fn the_card_keeps_only_the_listed_content() {
        let d = sample();
        let t = texts(&d);
        let all = t.join("\n");
        // kept: the Stand name under its label, the type in Japanese and English, six grades with their asterisks, the ability name and
        // sentence, the nearest canon line, the footer, and the Stand master (label and user) in the corner
        assert!(t.iter().any(|s| s == "DIRTY WEATHER") && t.iter().any(|s| s == NAME_LABEL) && t.iter().any(|s| s == MASTER_LABEL) && t.last().unwrap() == "jas3344");
        assert!(all.contains("近距離パワー型") && all.contains("Close-Range Power"));
        assert!(t.iter().any(|s| s == "B*") && t.iter().any(|s| s == "C") && t.iter().any(|s| s == "?"), "{t:?}");
        assert!(t.iter().any(|s| s == "Wind Reading") && all.replace('\n', " ").contains(&d.ability_line));
        assert!(t.iter().any(|s| s == "Nearest canon: D4C Love Train · Dolly Dagger · The World"));
        assert_eq!(FOOTER_ROWS.map(String::from).to_vec(), t[t.len() - 4..t.len() - 2].to_vec());
        // removed: stat value lines, film titles, graded-on lines, weakness, signature film, why, archetype caveat, PROVISIONAL DATA, credit,
        // and the old 本体― and the Stand name's corner brackets
        for gone in ["metric 0", "Film 0", "graded on", "Soft spot", "Signature film", "Why", "archetype", "PROVISIONAL", "Data:", "Letterboxd", "TMDB", "MusicBrainz", "A―", "超スゴイ", "本体", "「"] {
            assert!(!all.contains(gone), "{gone} is still on the card");
        }
        // no hidden text: every grade and label is one of the listed runs
        assert_eq!(t.iter().filter(|s| JP.contains(&s.as_str())).count(), 6);
    }

    #[test]
    fn the_footer_is_the_one_line() {
        assert_eq!(FOOTER, "standuser.pages.dev · Fan work, not affiliated · * provisional");
        assert!(!FOOTER.contains('\n') && !FOOTER.contains("endorsed") && !FOOTER.contains("Data:"));
        // drawn as two rows beside the Stand master, saying the same words, each row in the room the master leaves
        assert_eq!(FOOTER_ROWS.join(" "), FOOTER);
        let f = body_text();
        for r in FOOTER_ROWS {
            assert!(f.width(r, NEAR_SIZE, 0.0) <= FOOT_W, "{r}: {} of {FOOT_W}", f.width(r, NEAR_SIZE, 0.0));
        }
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
    fn the_stand_name_is_large_and_level() {
        // the name is drawn as large as the line allows, up to NAME_MAX_SIZE: the median name in names.json is well above the grades' 80 px
        let f = serif_text();
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
        assert!(median >= 80.0 && sizes[0] >= NAME_MIN_SIZE, "median {median}");
        // level and plain: its path is one fill path in the ink colour, with no outline, no group and no transform
        let s = card_svg(&sample(), Theme::Light);
        assert!(!s.contains("rotate(0") && !s.contains("skew") && !s.contains("matrix"));
        let (_, runs) = card_build(&sample(), Theme::Light);
        let name = runs.iter().find(|r| r.text == "DIRTY WEATHER").unwrap();
        assert!(!name.outlined && !name.rotated && name.kind == FontKind::Serif);
    }

    #[test]
    fn every_text_is_in_the_face_its_role_names() {
        // Noto Serif JP Bold, plain, for the Stand name, the user and the two eyecatch labels; Archivo Black for the grades and the ability's
        // name; Archivo 700 for all other Latin; Dela Gothic One with the black outline for Japanese at 48 px or larger; Noto Sans JP Bold,
        // no outline, for Japanese below 48 px. No other face.
        let mut d = sample();
        d.grades[5] = "完成".into();
        for theme in [Theme::Light, Theme::Dark] {
            for run in card_build(&d, theme).1 {
                let japanese = |c: char| c as u32 >= 0x2E80 && c != '【' && c != '】';
                let all_jp = run.text.chars().all(|c| japanese(c) || c == '―');
                match run.kind {
                    FontKind::Display => assert!((run.size >= JP_LETTERING_MIN || run.text == "ゴ") && run.outlined && all_jp, "{run:?}"),
                    FontKind::Jp => assert!(run.size < JP_LETTERING_MIN && !run.outlined && all_jp, "{run:?}"),
                    FontKind::Name => assert!(!run.text.chars().any(japanese), "{run:?}"),
                    FontKind::Body => assert!(!run.outlined && !run.text.chars().any(|c| c as u32 >= 0x2E80), "{run:?}"),
                    FontKind::Serif => assert!(!run.outlined && !run.rotated, "{run:?}"),
                }
            }
        }
        // the roles of the pieces the rule names
        let runs = card_build(&d, Theme::Light).1;
        let kind = |t: &str| runs.iter().find(|r| r.text.starts_with(t)).unwrap_or_else(|| panic!("{t}")).kind;
        assert_eq!(kind("DIRTY"), FontKind::Serif);
        assert_eq!(kind("【STAND NAME】"), FontKind::Serif);
        assert_eq!(kind("【STAND MASTER】"), FontKind::Serif);
        assert_eq!(kind("jas3344"), FontKind::Serif);
        assert_eq!(kind("B*"), FontKind::Name);
        assert_eq!(kind("It reads"), FontKind::Body);
        assert_eq!(kind("standuser"), FontKind::Body);
        assert_eq!(kind("Close-Range"), FontKind::Body);
        assert_eq!(kind("ゴ"), FontKind::Display);
        assert_eq!(kind("近距離"), FontKind::Jp);
        assert_eq!(kind("完成"), FontKind::Display, "the 完成 grade is lettering, like the other grades");
    }

    #[test]
    fn no_text_on_the_card_is_below_34_px() {
        // 34 px in card units reads at 390 px phone width: every run of every card, whatever it says
        let mut d = sample();
        for (name, user, line) in [
            ("A", "x", "Short."),
            ("YORKTOWN AND THE RETURN HOME", "abcdefghijklmno", "It unravels into tendrils that cross borders: 3.4 effective production countries per 50 films, and then some more."),
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

    /// Is the box `r` (left, top, right, bottom), grown by `pad`, clear of the convex polygon `poly`? Separating axis test.
    fn box_clear_of_polygon(r: &[f64; 4], poly: &[(f64, f64)], pad: f64) -> bool {
        let rc = [(r[0] - pad, r[1] - pad), (r[2] + pad, r[1] - pad), (r[2] + pad, r[3] + pad), (r[0] - pad, r[3] + pad)];
        let mut axes = vec![(1.0, 0.0), (0.0, 1.0)];
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            axes.push((-(b.1 - a.1), b.0 - a.0));
        }
        axes.iter().any(|&(ax, ay)| {
            let proj = |pts: &[(f64, f64)]| pts.iter().map(|p| p.0 * ax + p.1 * ay).fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
            let (pl, ph) = proj(poly);
            let (rl, rh) = proj(&rc);
            ph < rl || rh < pl
        })
    }

    #[test]
    fn the_gogogo_are_one_shallow_cascade_in_the_gap_clear_of_all_text() {
        // the longest Stand name in names.json (the widest at the smallest size), names with deep descenders at the largest size, a one-letter
        // name, and the widest type lines
        let names: serde_json::Value = serde_json::from_str(include_str!("../data/names.json")).unwrap();
        let f = serif_text();
        let mut longest = (0.0, String::new());
        for v in names["names"].as_object().unwrap().values() {
            for cand in v[1].as_array().unwrap() {
                let n = cand[0].as_str().unwrap();
                let w = f.width(n, NAME_MIN_SIZE, 0.0);
                if w > longest.0 {
                    longest = (w, n.to_string());
                }
            }
        }
        let cases = [longest.1.as_str(), "YORKTOWN AND THE RETURN HOME", "QUEEN, JAZZ (JQ)", "A", "DIRTY WEATHER"];
        let plate = hex(CX, CY, CHART_R);
        let mut checked = 0;
        // 7 keys (six accents and none) in the light and the dark card: 14 grounds
        for key in [None, Some("PWR"), Some("SPD"), Some("RNG"), Some("STA"), Some("PRC"), Some("DEV")] {
            for theme in [Theme::Light, Theme::Dark] {
                for name in cases {
                    let mut d = sample();
                    d.name = name.into();
                    d.accent = key.map(String::from);
                    d.type_jp = "遠隔自動操縦型".into();
                    d.type_en = "Automatic, Far-Distance Autopilot".into();
                    d.user = "abcdefghijklmno".into();
                    d.grades[1] = "完成".into();
                    let runs = card_build(&d, theme).1;
                    let go: Vec<&Run> = runs.iter().filter(|r| r.text == "ゴ").collect();
                    assert_eq!(go.len(), GOGOGO.len());
                    // each smaller than the one before; none below the card's smallest text
                    let sizes: Vec<f64> = go.iter().map(|r| r.size).collect();
                    assert!(sizes.windows(2).all(|w| w[0] > w[1]) && sizes[sizes.len() - 1] >= MIN_TEXT, "{sizes:?}");
                    // one diagonal from the top left down to the right, at 35 to 45 degrees from horizontal: from the first glyph to the last and
                    // from each to the next
                    let centre = |r: &Run| ((r.bbox[0] + r.bbox[2]) / 2.0, (r.bbox[1] + r.bbox[3]) / 2.0);
                    let angle = |a: (f64, f64), b: (f64, f64)| (b.1 - a.1).atan2(b.0 - a.0).to_degrees();
                    let cs: Vec<(f64, f64)> = go.iter().map(|r| centre(r)).collect();
                    let whole = angle(cs[0], cs[cs.len() - 1]);
                    assert!((35.0..=45.0).contains(&whole), "the cascade runs at {whole:.1} degrees");
                    for w in cs.windows(2) {
                        let a = angle(w[0], w[1]);
                        assert!((35.0..=45.0).contains(&a), "a step runs at {a:.1} degrees");
                    }
                    assert!(cs[0].0 < W / 4.0 && cs[0].1 < H / 4.0, "it starts at the top left: {:?}", cs[0]);
                    // no two touch
                    let apart = |a: &[f64; 4], b: &[f64; 4], pad: f64| a[2] + pad <= b[0] || b[2] + pad <= a[0] || a[3] + pad <= b[1] || b[3] + pad <= a[1];
                    for (i, a) in go.iter().enumerate() {
                        for b in go.iter().skip(i + 1) {
                            assert!(apart(&a.bbox, &b.bbox, 2.0), "ゴ {:?} touches ゴ {:?}", a.bbox, b.bbox);
                        }
                    }
                    // they lean (the cascade is the one rotation on the card), and stand in the gap: under the name block (the name, its label
                    // and the type lines), above the hexagon's upper-left vertex, each clear of every other text by 10 px, of the plate by
                    // 10 px, and of the frame
                    assert!(go.iter().all(|r| r.rotated) && runs.iter().filter(|r| r.rotated).count() == go.len());
                    let name_block = runs.iter().filter(|r| r.kind == FontKind::Serif && r.bbox[3] < 300.0).map(|r| r.bbox[3]).fold(0.0, f64::max);
                    for g in &go {
                        for r in runs.iter().filter(|r| r.text != "ゴ") {
                            assert!(apart(&g.bbox, &r.bbox, 10.0), "{name:?}: ゴ {:?} touches {:?}", g.bbox, r);
                        }
                        assert!(box_clear_of_polygon(&g.bbox, &plate, 10.0), "{name:?}: ゴ {:?} reaches the hexagon", g.bbox);
                        assert!(g.bbox[0] >= FRAME + 10.0, "ゴ {:?} is at the frame", g.bbox);
                        assert!(g.bbox[1] >= name_block + 10.0 && g.bbox[3] <= plate[5].1, "{name:?}: ゴ {:?} is outside the gap between the name block ({name_block}) and the hexagon ({})", g.bbox, plate[5].1);
                    }
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 14 * cases.len());
        println!("the longest name in names.json is {:?}; {checked} cards checked", longest.1);
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

    #[test]
    fn the_name_is_level_and_the_boxes_hold_the_real_text() {
        // the Stand name has no tilt, the cascade's is the card's one
        assert_eq!(GOGOGO_TILT, -8.0);
        // the master and type boxes contain the widest real master and the type lines of every type
        let mut d = sample();
        d.user = "abcdefghijklmno".into();
        for (jp, en) in [("遠隔自動操縦型", "Automatic, Far-Distance Autopilot"), ("遠隔操作型", "Long-Distance Operation"), ("近距離パワー型", "Close-Range Power"), ("未分類", "Unclassified")] {
            d.type_jp = jp.into();
            d.type_en = en.into();
            let runs = card_build(&d, Theme::Light).1;
            let inside = |t: &str, b: &[f64; 4]| {
                let r = runs.iter().find(|r| r.text.starts_with(t)).unwrap();
                assert!(r.bbox[0] >= b[0] && r.bbox[1] >= b[1] && r.bbox[2] <= b[2] && r.bbox[3] <= b[3], "{t}: {:?} is outside {b:?}", r.bbox);
            };
            inside("【STAND MASTER】", &MASTER_BOX);
            inside("abcdefgh", &MASTER_BOX);
            inside(jp, &TYPE_JP_BOX);
            inside(en, &TYPE_EN_BOX);
        }
        // the master is in the bottom right corner: right-aligned to the margin, below everything else on the right
        let runs = card_build(&d, Theme::Light).1;
        let user = runs.iter().find(|r| r.text == "abcdefghijklmno").unwrap();
        assert!((user.bbox[2] - RIGHT).abs() < 6.0 && user.bbox[1] > H / 2.0 && user.bbox[3] > H - 60.0, "{user:?}");
        let label = runs.iter().find(|r| r.text == MASTER_LABEL).unwrap();
        assert!(label.bbox[2] <= RIGHT + 2.0 && label.bbox[3] < user.bbox[1] + 20.0 && label.bbox[1] > H * 0.9, "{label:?}");
    }

    #[test]
    fn no_two_texts_overlap_and_the_label_sits_over_the_name() {
        let apart = |a: &[f64; 4], b: &[f64; 4], pad: f64| a[2] + pad <= b[0] || b[2] + pad <= a[0] || a[3] + pad <= b[1] || b[3] + pad <= a[1];
        let mut d = sample();
        for (name, jp, en) in [
            ("YORKTOWN AND THE RETURN HOME", "遠隔自動操縦型", "Automatic, Far-Distance Autopilot"),
            ("A", "近距離パワー型", "Close-Range Power"),
            ("DIRTY WEATHER, QUEEN OF JAZZ", "遠隔操作型", "Long-Distance Operation"),
            ("THE QUICK BROWN COMPOSER OF MANY LONG NAMES AND HOMAGES TO EVERYONE", "未分類", "Unclassified"),
        ] {
            d.name = name.into();
            d.type_jp = jp.into();
            d.type_en = en.into();
            d.user = "abcdefghijklmno".into();
            d.grades[1] = "完成".into();
            let runs = card_build(&d, Theme::Light).1;
            // the badge letter sits beside its label and no text box touches another (the cascade has its own test)
            for (i, a) in runs.iter().enumerate().filter(|(_, a)| !a.rotated) {
                for b in runs.iter().skip(i + 1).filter(|b| !b.rotated) {
                    assert!(apart(&a.bbox, &b.bbox, 2.0), "{name:?}: {a:?} touches {b:?}");
                }
            }
            // the label is directly over the name, both at the left margin
            let label = runs.iter().find(|r| r.text == NAME_LABEL).unwrap();
            let nm = runs.iter().find(|r| r.text.starts_with(&name[..1]) && r.kind == FontKind::Serif && r.size >= NAME_MIN_SIZE).unwrap();
            assert!(label.bbox[3] <= nm.bbox[1] && nm.bbox[1] - label.bbox[3] < 30.0 && (label.bbox[0] - nm.bbox[0]).abs() < 4.0, "{label:?} over {nm:?}");
            assert!(label.size < nm.size || label.size == NAME_MIN_SIZE.min(label.size));
        }
    }

    #[test]
    fn the_grades_are_the_largest_lettering_after_the_stand_name() {
        assert!(GRADE_SIZE >= 72.0 && GRADE_JP_SIZE >= 72.0);
        for theme in [Theme::Light, Theme::Dark] {
            let mut d = sample();
            d.grades[1] = "完成".into();
            let (svg, runs) = card_build(&d, theme);
            // six grades, each with its asterisk when provisional, in Archivo Black (Dela Gothic One for 完成) with the black outline
            let want: Vec<String> = (0..6).map(|i| format!("{}{}", d.grades[i], if d.prov[i] { "*" } else { "" })).collect();
            let grades: Vec<&Run> = want.iter().map(|w| runs.iter().find(|r| &r.text == w).unwrap_or_else(|| panic!("grade {w} is not drawn"))).collect();
            assert!(want.iter().any(|w| w.ends_with('*')), "the sample has a provisional grade: {want:?}");
            for g in &grades {
                assert!(g.outlined && g.size >= 72.0, "{g:?}");
                assert_eq!(g.kind, if g.text.starts_with("完成") { FontKind::Display } else { FontKind::Name }, "{g:?}");
            }
            // every other run is smaller than the smallest grade, bar the Stand name (the serif, as large as its line allows) and the
            // cascade's ゴ (the rotated runs)
            let smallest = grades.iter().map(|g| g.size).fold(f64::MAX, f64::min);
            for r in runs.iter().filter(|r| !r.rotated && r.text != d.name && !grades.iter().any(|g| std::ptr::eq(*g, *r))) {
                assert!(r.size < smallest, "{r:?} is as large as a grade ({smallest} px)");
            }
            assert!(svg.matches("class=\"o\" stroke-width=\"6\"").count() >= GOGOGO.len() + 6, "an outline under each grade and each ゴ");
        }
    }

    #[test]
    fn the_parts_the_pages_reveal_animates_are_marked() {
        // the page inlines the card while it plays the reveal and animates these groups by CSS: the ゴ in cascade order, the name with
        // its label, the radar fill, and the six grades clockwise from the top. No animation is in the SVG itself.
        let s = card_svg(&sample(), Theme::Light);
        assert_eq!(s.matches("data-part=\"go\"").count(), GOGOGO.len());
        assert_eq!(s.matches("data-part=\"name\"").count(), 1);
        assert_eq!(s.matches("data-part=\"fill\"").count(), 1);
        let grades: Vec<usize> = s.split("data-part=\"grade\" data-i=\"").skip(1).map(|p| p[..1].parse().unwrap()).collect();
        assert_eq!(grades, vec![0, 1, 2, 3, 4, 5]);
        // the groups are balanced: every one closes
        assert_eq!(s.matches("<g").count(), s.matches("</g>").count());
        // the name group holds the label and the name; the fill group the polygon and the "?" markers
        let at = s.find("data-part=\"name\"").unwrap();
        let name = &s[at..at + s[at..].find("</g>").unwrap()];
        assert_eq!(name.matches("<path").count(), 2);
        let at = s.find("data-part=\"fill\"").unwrap();
        let fill = &s[at..at + s[at..].find("</g>").unwrap()];
        assert!(fill.contains("<polygon") && fill.matches("<circle").count() == sample().unknown.iter().filter(|u| **u).count());
        assert!(!s.contains("<animate") && !s.contains("@keyframes"));
    }

    #[test]
    fn the_radar_has_no_scale_dots() {
        // the small black dots on each spoke at each ring are gone: the only circles are the "?" markers
        let mut d = sample();
        let s = card_svg(&d, Theme::Light);
        assert!(!s.contains("r=\"2.6\""), "a dot is still drawn");
        assert_eq!(s.matches("<circle").count(), d.unknown.iter().filter(|u| **u).count());
        d.unknown = vec![false; 6];
        assert_eq!(card_svg(&d, Theme::Light).matches("<circle").count(), 0);
        // the rings and the three spokes stay
        assert_eq!(s.matches("<polygon").count(), 7);
        assert_eq!(s.matches("<line").count(), 3);
    }
}
