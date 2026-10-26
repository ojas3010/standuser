//! Stand card as one self-contained SVG (port of swarm/round3/card/gen_card.py, text-as-outlines mode).
//!
//! All text is drawn as paths with class="f-{cls}" (fill colour from the palette CSS), so it renders identically in
//! browsers, GitHub READMEs and PNG renderers. Fixes gen_card.py line 121, where text lost its `f-` prefix.
//! The look is the manga's: a flat saturated cover colour for the ground, heavy lettering (Dela Gothic One for the katakana, labels and
//! ゴゴゴ, Archivo Black for the Stand name) with a black outline drawn as a second, thicker black path under the fill path, and the
//! Part 5 stat chart: a hexagon with a ring per grade, circled A to E up the top axis, and a circled grade at each vertex.

use crate::font::{font, Font, FontKind};
use crate::palette::{self, Pal};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

pub const W: f64 = 1080.0;
pub const H: f64 = 1350.0;

pub const JP: [&str; 6] = ["破壊力", "スピード", "射程距離", "持続力", "精密動作性", "成長性"];
pub const EN: [&str; 6] = ["Destructive Power", "Speed", "Range", "Stamina", "Precision", "Potential"];
/// The one credit line at the bottom of the card. The TMDB disclaimer is on the site.
pub const FOOTER: &str = "* provisional  ·  Fan work, not affiliated  ·  Data: Letterboxd, TMDB, MusicBrainz";
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

struct Ctx {
    sans: Font,
    name: Font,
    display: Font,
    els: Vec<String>,
}

impl Ctx {
    fn face(&self, kind: FontKind) -> &Font {
        match kind {
            FontKind::Sans => &self.sans,
            FontKind::Name => &self.name,
            FontKind::Display => &self.display,
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
        self.els.push(format!("<path d=\"{d}\" class=\"o\" stroke-width=\"{ow}\"/><path d=\"{d}\" class=\"f-{cls}\"/>"));
    }

    /// `s` centred on (cx, cy) both ways, by its real ink (a letter in a circle).
    fn centred(&mut self, s: &str, cx: f64, cy: f64, size: f64, kind: FontKind, cls: &str) {
        let (up, down) = self.face(kind).ink_extent(s, size);
        self.text(s, cx, cy + (up - down) / 2.0, size, kind, cls, "middle", 0.0);
    }
}

/// The why line sits in its own rows under the signature film, in full (up to 3 lines) and at the signature line's size.
pub const WHY_SIZE: f64 = 20.0;
pub const WHY_WIDTH: f64 = 940.0;
pub const WHY_MAX_ROWS: usize = 3;
const WHY_PITCH: f64 = 22.0;
/// Baseline of the last why row (the rows above it follow at WHY_PITCH) and of the credit line.
const WHY_LAST_Y: f64 = 1284.0;
pub const FOOT_Y: f64 = 1304.0;
/// The block from 【STAND MASTER】 down sits this much higher than the old layout, which freed the header's why rows for them.
const UP: f64 = 36.0;
/// Top of the first stat-cell row, and the baseline of the grade legend above it.
pub const CELLS_Y: f64 = 852.0 - UP - 8.0;
pub const LEGEND_Y: f64 = 818.0 - UP - 4.0;
/// Smallest row pitch of the stat cells: a cell is 61 px tall.
pub const MIN_PITCH: f64 = 66.0;

/// The card's lower half, which flows from the bottom: the credit line is fixed, the why rows sit above it, then the signature film
/// and the nearest canon, then the ability box. The stat-cell rows take whatever pitch (72 px down to MIN_PITCH) lets the box end above
/// the nearest-canon row, so the full why line never costs a cut anywhere.
pub struct Layout {
    pub pitch: f64,
    pub box_top: f64,
    pub box_h: f64,
    pub near: f64,
    pub sig: f64,
    pub why: f64,
}

pub fn layout(ability_lines: usize, weakness_lines: usize, why_rows: usize) -> Layout {
    let (al, wl) = (ability_lines.max(1) as f64, weakness_lines.max(1) as f64);
    let box_h = 30.0 + (al - 1.0) * ABILITY_PITCH + 28.0 + (wl - 1.0) * WEAK_PITCH + 14.0;
    let why = WHY_LAST_Y - (why_rows.max(1) as f64 - 1.0) * WHY_PITCH;
    let sig = why - 24.0;
    let near = sig - 23.0;
    let mut pitch = 72.0;
    while pitch > MIN_PITCH && CELLS_Y + 3.0 * pitch + 14.0 + box_h + 4.0 > near - 17.0 {
        pitch -= 1.0;
    }
    Layout { pitch, box_top: CELLS_Y + 3.0 * pitch + 14.0, box_h, near, sig, why }
}

/// The why line as drawn: wrapped in full to WHY_WIDTH, at most WHY_MAX_ROWS rows. Only a line longer than that is cut with "…".
pub fn why_rows(f: &Font, why: &str) -> Vec<String> {
    let all = f.wrap(why, WHY_SIZE, WHY_WIDTH);
    all.iter()
        .take(WHY_MAX_ROWS)
        .enumerate()
        .map(|(i, ln)| if i + 1 == WHY_MAX_ROWS && all.len() > WHY_MAX_ROWS { f.fit(&format!("{ln} …"), WHY_SIZE, WHY_WIDTH) } else { ln.clone() })
        .collect()
}
const ABILITY_SIZE: f64 = 20.0;
const ABILITY_PITCH: f64 = 24.0;
const WEAK_SIZE: f64 = 19.0;
const WEAK_PITCH: f64 = 22.0;
const TEXT_WIDTH: f64 = 820.0;

/// The type note is 14 px and at most this wide.
const NOTE_SIZE: f64 = 14.0;
const NOTE_WIDTH: f64 = 380.0;
pub const NOTE_Y: f64 = 309.0 - UP;
/// The chart centre and the hexagon's circumradius (the ∞ ring; a grade E to A is the ring k of 6 for k = 1 to 5).
pub const CY: f64 = 546.0 - UP;
pub const CHART_R: f64 = 148.0;
/// The circled grade at each vertex: its radius, how far beyond the vertex its centre is, and the size of the JP label past it.
const BADGE_R: f64 = 22.0;
const BADGE_OFF: f64 = 34.0;
const LABEL_SIZE: f64 = 22.0;
/// Lowest ink of the chart (the bottom label's baseline and a little below it): the legend must clear this.
pub const CHART_BOTTOM: f64 = CY + CHART_R + BADGE_OFF + BADGE_R + 8.0 + LABEL_SIZE + 5.0;

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

/// The ゴゴゴ along both sides are the SFX of the scene: the card's second colour with the black outline, level, each one smaller than
/// the last as on the covers; their size (a Dela Gothic One ゴ is one em wide) and the centre of the column.
pub const GOGOGO_SIZES: [f64; 4] = [76.0, 70.0, 64.0, 58.0];
pub const GOGOGO_X: f64 = 82.0;
const GOGOGO_Y0: f64 = 380.0;
/// Width of the black outline of the display lettering, in px (half of it shows outside the glyph).
pub const OUTLINE: f64 = 6.0;
/// The Stand name's outline, which is heavier: the name is the card's logo.
pub const NAME_OUTLINE: f64 = 10.0;
/// The Stand name's one tilt, in degrees as SVG's rotate() takes them: negative is counter-clockwise, so the name climbs to the right,
/// like the slant of a Part logo. Nothing else on the card is rotated.
pub const NAME_TILT: f64 = -5.0;
/// The header box the tilted name stays inside: from the left margin to the right margin; its left end stays below the 【STAND NAME】
/// label, anywhere else it stays this far from the top frame, and it ends above the 【STAND MASTER】 row.
pub const NAME_X0: f64 = 70.0;
pub const NAME_X1: f64 = W - 70.0;
pub const NAME_TOP_AT_LEFT: f64 = 100.0;
pub const NAME_TOP: f64 = 54.0;
pub const NAME_BOTTOM: f64 = 190.0;
pub const NAME_MAX_SIZE: f64 = 80.0;
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
    c.iter().all(|q| q.0 >= NAME_X0 - 0.01 && q.0 <= NAME_X1 && q.1 >= NAME_TOP && q.1 <= NAME_BOTTOM + 0.01) && c[0].1 >= NAME_TOP_AT_LEFT
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

pub fn card_svg(d: &CardData, theme: Theme) -> String {
    use FontKind::{Display, Name, Sans};
    let mut c = Ctx { sans: font(Sans), name: font(Name), display: font(Display), els: vec![] };
    let (cx, cy, r_chart) = (540.0, CY, CHART_R);

    // ground, and the frame: the panel border in black, a fine line of the ink inside it
    c.els.push(format!("<rect width=\"{W}\" height=\"{H}\" class=\"f-bg\"/>"));
    c.els.push(format!("<rect x=\"24\" y=\"24\" width=\"{}\" height=\"{}\" rx=\"4\" class=\"s-bk\" fill=\"none\" stroke-width=\"8\"/>", W - 48.0, H - 48.0));
    c.els.push(format!("<rect x=\"39\" y=\"39\" width=\"{}\" height=\"{}\" rx=\"2\" class=\"s-ink\" fill=\"none\" stroke-width=\"2\"/>", W - 78.0, H - 78.0));
    // the ゴゴゴ of the scene, down both sides
    let mut gy = GOGOGO_Y0;
    for sz in GOGOGO_SIZES {
        for gx in [GOGOGO_X, W - GOGOGO_X] {
            c.lettering("ゴ", gx, gy, sz, Display, "fill", "middle", 0.0, OUTLINE);
        }
        gy += sz * 1.45;
    }
    // header
    c.text("【STAND NAME】", 70.0, 84.0, 20.0, Display, "ink", "start", 2.0);
    if d.badge {
        // neutral, not the accent: the badge is a note, not a mark
        // on the right of the note rows, clear of the type name and of the top right, where the name climbs to
        c.els.push("<rect x=\"770\" y=\"268\" width=\"240\" height=\"40\" rx=\"20\" fill=\"none\" class=\"s-mute\" stroke-width=\"2\"/>".into());
        c.text("PROVISIONAL DATA", 890.0, 295.0, 19.0, Sans, "mute", "middle", 1.5);
    }
    // the Stand name: the card's one diagonal, largest size that keeps its tilted box in the header; the outline and the fill share the tilt
    let np = place_name(&c.name, &d.name);
    {
        let dpath = c.name.path(&np.text, np.x, np.y, np.size, NAME_LS);
        c.els.push(format!(
            "<g transform=\"rotate({} {} {})\"><path d=\"{dpath}\" class=\"o\" stroke-width=\"{NAME_OUTLINE}\"/><path d=\"{dpath}\" class=\"f-paper\"/></g>",
            NAME_TILT, n1(np.x), n1(np.y)
        ));
    }
    c.text("【STAND MASTER】", 70.0, 238.0 - UP, 20.0, Display, "ink", "start", 2.0);
    let label = "本体―";
    c.text(label, 350.0, 240.0 - UP, 30.0, Display, "ink", "start", 0.0);
    let lw = c.display.width(label, 30.0, 0.0);
    let user = c.sans.fit(&d.user, 34.0, 470.0 - lw - 10.0);
    c.text(&user, 350.0 + lw + 10.0, 240.0 - UP, 34.0, Sans, "ink", "start", 0.0);
    c.text(&d.type_jp, 70.0, 288.0 - UP, 26.0, Display, "ink", "start", 0.0);
    let tw = c.display.width(&d.type_jp, 26.0, 0.0);
    c.text(&format!(" · {}", d.type_en), 70.0 + tw, 288.0 - UP, 26.0, Sans, "mute", "start", 0.0);
    for (i, ln) in note_lines(&c.sans, &d.type_note).iter().enumerate() {
        c.text(ln, 70.0, NOTE_Y + i as f64 * 16.0, NOTE_SIZE, Sans, "mute", "start", 0.0);
    }

    // the chart, after the Part 5 tailpiece: a paper hexagon, a ring per grade, three spokes, a dot on each spoke at each ring, the
    // circled A to E up the top axis
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
    for (kk, l) in "EDCBA".chars().enumerate() {
        let y = cy - r_chart * (kk + 1) as f64 / 6.0;
        c.els.push(format!("<circle cx=\"{cx}\" cy=\"{y:.1}\" r=\"10\" class=\"f-paper s-bk\" stroke-width=\"1.5\"/>"));
        c.centred(&l.to_string(), cx, y, 13.0, Display, "bk");
    }
    for i in 0..6 {
        if d.unknown[i] {
            c.els.push(format!("<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"7\" class=\"f-paper s-bk\" stroke-width=\"2.5\"/>", vals[i].0, vals[i].1));
        }
    }
    // the circled grade past each vertex, and the label past it: at the top and bottom above and below, at the four sides to the right
    // or left, so the labels stand where the Part 5 chart has them (破壊力 top, スピード upper right, 射程距離 lower right, 持続力
    // bottom, 精密動作性 lower left, 成長性 upper left)
    for i in 0..6 {
        let a = (i as f64 * 60.0).to_radians();
        let r_b = r_chart + BADGE_OFF;
        let (bx, by) = (cx + r_b * a.sin(), cy - r_b * a.cos());
        let g = d.grades[i].as_str();
        if g == "完成" {
            c.els.push(format!("<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{}\" height=\"{}\" rx=\"{BADGE_R}\" class=\"f-paper s-bk\" stroke-width=\"3\"/>", bx - 31.0, by - BADGE_R, 62.0, 2.0 * BADGE_R));
            c.centred(g, bx, by, 24.0, Display, "bk");
        } else {
            c.els.push(format!("<circle cx=\"{bx:.1}\" cy=\"{by:.1}\" r=\"{BADGE_R}\" class=\"f-paper s-bk\" stroke-width=\"3\"/>"));
            c.centred(g, bx, by, 30.0, Display, "bk");
        }
        let half = if g == "完成" { 31.0 } else { BADGE_R };
        match i {
            0 => c.text(JP[i], bx, by - BADGE_R - 8.0, LABEL_SIZE, Display, "ink", "middle", 0.0),
            3 => c.text(JP[i], bx, by + BADGE_R + 8.0 + LABEL_SIZE * 0.8, LABEL_SIZE, Display, "ink", "middle", 0.0),
            _ => {
                let right = a.sin() > 0.0;
                let (ux, uy) = if right { (bx + half + 10.0, by) } else { (bx - half - 10.0, by) };
                let (up, down) = c.display.ink_extent(JP[i], LABEL_SIZE);
                c.text(JP[i], ux, uy + (up - down) / 2.0, LABEL_SIZE, Display, "ink", if right { "start" } else { "end" }, 0.0);
            }
        }
    }

    // legend, rule, six stat cells (2 columns x 3 rows)
    let ab = c.sans.wrap(&d.ability, ABILITY_SIZE, TEXT_WIDTH);
    let wk = c.sans.wrap(&d.weakness, WEAK_SIZE, TEXT_WIDTH);
    let (al, wl) = (ab.len().min(3), wk.len().min(2));
    let why = why_rows(&c.sans, &d.why);
    let lay = layout(al, wl, why.len());
    let (pitch, top, hbox) = (lay.pitch, lay.box_top, lay.box_h);
    let y0 = CELLS_Y;
    let legend = LEGEND.iter().map(|(a, b)| format!("{a}―{b}")).collect::<Vec<_>>().join("  ");
    c.text(&legend, W / 2.0, LEGEND_Y, 18.0, Display, "ink", "middle", 0.0);
    c.els.push(format!("<line x1=\"70\" y1=\"{}\" x2=\"{}\" y2=\"{}\" class=\"s-ink\" stroke-width=\"3\"/>", y0 - 22.0, W - 70.0, y0 - 22.0));
    for i in 0..6 {
        let (col, row) = (i % 2, i / 2);
        let (x, y) = (70.0 + col as f64 * 470.0, y0 + row as f64 * pitch);
        let head = format!("{}―{}{}", JP[i], d.grades[i], if d.prov[i] { "*" } else { "" });
        c.text(&head, x, y + 20.0, 24.0, Display, "ink", "start", 0.0);
        if let Some(cv) = d.coverage.get(i) {
            let cv = c.sans.fit(cv, 15.0, 270.0);
            c.text(&cv, x + 440.0, y + 19.0, 15.0, Sans, "mute", "end", 0.0);
        }
        let m = c.sans.fit(&d.evidence[i].0, 18.0, 440.0);
        c.text(&m, x, y + 42.0, 18.0, Sans, "mute", "start", 0.0);
        let f = c.sans.fit_title(&d.evidence[i].1, 18.0, 440.0);
        c.text(&f, x, y + 61.0, 18.0, Sans, "ink", "start", 0.0);
    }

    // ability and weakness box: white paper with the black frame of a manga panel, black text
    c.els.push(format!("<rect x=\"70\" y=\"{top}\" width=\"{}\" height=\"{hbox}\" rx=\"2\" class=\"f-paper s-bk\" stroke-width=\"3\"/>", W - 140.0));
    c.text("能力―", 88.0, top + 30.0, 22.0, Display, "bk", "start", 0.0);
    for (i, ln) in ab.iter().take(3).enumerate() {
        let ln = if i == 2 && ab.len() > 3 { c.sans.fit(&format!("{ln} …"), ABILITY_SIZE, TEXT_WIDTH) } else { ln.clone() };
        c.text(&ln, 170.0, top + 30.0 + i as f64 * ABILITY_PITCH, ABILITY_SIZE, Sans, "bk", "start", 0.0);
    }
    let wy = top + 30.0 + (al as f64 - 1.0) * ABILITY_PITCH + 28.0;
    c.text("弱点―", 88.0, wy, 20.0, Display, "pmute", "start", 0.0);
    for (i, ln) in wk.iter().take(2).enumerate() {
        let ln = if i == 1 && wk.len() > 2 { c.sans.fit(&format!("{ln} …"), WEAK_SIZE, TEXT_WIDTH) } else { ln.clone() };
        c.text(&ln, 170.0, wy + i as f64 * WEAK_PITCH, WEAK_SIZE, Sans, "pmute", "start", 0.0);
    }

    // bottom rows: nearest canon, signature film, the why line in full, one credit line
    let near = c.sans.fit(&format!("Nearest canon: {}", d.nearest.join(" · ")), 22.0, 940.0);
    c.text(&near, 70.0, lay.near, 22.0, Sans, "ink", "start", 0.0);
    let sig = c.sans.fit_signature(&d.signature, 20.0, 940.0);
    c.text(&sig, 70.0, lay.sig, 20.0, Sans, "mute", "start", 0.0);
    for (i, ln) in why.iter().enumerate() {
        c.text(ln, 70.0, lay.why + i as f64 * WHY_PITCH, WHY_SIZE, Sans, "ink", "start", 0.0);
    }
    c.text(FOOTER, 70.0, FOOT_Y, 16.0, Sans, "mute", "start", 0.0);

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
            why: "Why 「DIRTY WEATHER」: verbatim: \"DIRTY WEATHER\", track 3 of Some Band's Lighthouse score (2019); Lighthouse is one of your Favourite Films (4.5★). \"weather\" fits your Wind Reading ability.".into(),
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
    fn bottom_rows_do_not_collide() {
        // for every number of ability (1 to 3), weakness (1 to 2) and why rows (1 to 3): the box ends above the nearest-canon row
        // (22 px type, cap height about 17), the rows keep their order, the last why row clears the credit line, and the stat
        // cells keep a pitch that does not let two rows touch
        for al in 1..=3 {
            for wl in 1..=2 {
                for wy in 1..=WHY_MAX_ROWS {
                    let l = layout(al, wl, wy);
                    assert!(l.box_top + l.box_h + 4.0 <= l.near - 17.0, "{al}+{wl}+{wy}: box ends {}, nearest canon at {}", l.box_top + l.box_h, l.near);
                    assert!(l.pitch >= MIN_PITCH && l.box_top > CELLS_Y + 2.0 * l.pitch + 61.0 + 10.0, "the box touches the last stat row");
                    assert!(l.near + 6.0 < l.sig - 14.0 && l.sig + 5.0 < l.why - 15.0, "rows overlap");
                    assert!(l.why + (wy as f64 - 1.0) * WHY_PITCH + 5.0 < FOOT_Y - 11.0, "why rows reach the credit line");
                }
            }
        }
        assert!(FOOT_Y + 4.0 < H - 34.0, "the credit line is outside the inner frame");
        // the legend clears the chart's bottom label and the rule clears the legend
        assert!(LEGEND_Y - 15.0 > CHART_BOTTOM + 4.0 && LEGEND_Y + 5.0 < CELLS_Y - 22.0, "chart ends {CHART_BOTTOM}, legend at {LEGEND_Y}");
    }

    #[test]
    fn the_credit_line_and_why_row_are_readable() {
        // one credit line, one line of text (no TMDB disclaimer on the card), and it fits in the frame at 16 px
        assert!(!FOOTER.contains('\n'));
        assert!(!FOOTER.contains("endorsed"));
        assert!(font(FontKind::Sans).width(FOOTER, 16.0, 0.0) < 940.0);
        // the why row is body size: the same as the evidence lines
        assert!(WHY_SIZE >= 20.0);
    }

    #[test]
    fn the_badge_is_not_red() {
        let s = card_svg(&sample(), Theme::Light);
        let badge = &s[s.find("<rect x=\"770\"").unwrap()..];
        let badge = &badge[..badge.find("/>").unwrap()];
        assert!(badge.contains("s-mute") && !badge.contains("s-fill"), "{badge}");
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
        // and each ゴゴゴ (8 of them)
        let s = card_svg(&sample(), Theme::Light);
        assert_eq!(s.matches("class=\"o\"").count(), 1 + 8);
        let mut pairs = 0;
        let parts: Vec<&str> = s.split("<path d=\"").skip(1).collect();
        for (k, part) in parts.iter().enumerate() {
            let (d, rest) = part.split_once('"').unwrap();
            if rest.starts_with(" class=\"o\" stroke-width=\"") {
                assert!(parts[k + 1].starts_with(&format!("{d}\" class=\"f-")), "the fill path of an outline must follow it, with the same glyphs");
                pairs += 1;
            }
        }
        assert_eq!(pairs, 9);
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
        // hexagonal plate and rings (7 polygons with the value polygon), the circled A to E on the top axis, one circled grade per vertex
        let s = card_svg(&sample(), Theme::Light);
        assert_eq!(s.matches("<polygon").count(), 1 + 5 + 1);
        assert!(!s.contains("stroke-dasharray=\"6 5\"") || sample().unknown.iter().any(|u| *u));
        assert_eq!(s.matches("r=\"22\" class=\"f-paper s-bk\"").count(), 6, "six circled grades");
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
        let f = font(FontKind::Sans);
        for ln in f.wrap(&sample().ability, ABILITY_SIZE, TEXT_WIDTH) {
            assert!(f.width(&ln, ABILITY_SIZE, 0.0) <= TEXT_WIDTH + 0.01);
        }
        assert!(f.fit("a very long film title ".repeat(10).as_str(), 19.0, 440.0).ends_with('…'));
    }

    #[test]
    fn no_card_label_is_cut() {
        let c = Ctx { sans: font(FontKind::Sans), name: font(FontKind::Name), display: font(FontKind::Display), els: vec![] };
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
        // the note stays left of the chart's top label (the circled grade and 破壊力 stand over the axis at x = 540) on every line
        for (i, ln) in note_lines(&c.sans, &d.type_note).iter().enumerate() {
            assert!(70.0 + c.sans.width(ln, NOTE_SIZE, 0.0) < 540.0 - 60.0, "line {i} reaches the chart's top label");
        }
        for i in 0..6 {
            uncut(&c.sans, &d.coverage[i], 15.0, 270.0);
            uncut(&c.sans, &d.evidence[i].0, 18.0, 440.0);
            uncut(&c.sans, &d.evidence[i].1, 18.0, 440.0);
        }
        uncut(&c.sans, &format!("Nearest canon: {}", d.nearest.join(" · ")), 22.0, 940.0);
        uncut(&c.sans, &d.signature, 20.0, 940.0);
        uncut(&c.sans, &d.user, 34.0, 470.0);
        assert!(c.sans.wrap(&d.ability, ABILITY_SIZE, TEXT_WIDTH).len() <= 3 && c.sans.wrap(&d.weakness, WEAK_SIZE, TEXT_WIDTH).len() <= 2);
    }
}
