//! The card's per-user palette (docs/DESIGN.md section 4).
//!
//! The page's accent is the card's ground, read back from the card's SVG (site/accent.js); it is keyed to the user's
//! signature stat. The card's ground is the cover colour of that stat's family: Power hot pink, Speed yellow, Range blue, Stamina green,
//! Precision purple, Potential orange. Every ground, and every radar fill, is a colour sampled from the reference covers by
//! tools/ref_palette.py; docs/ref-palette.json names the image each came from and a test checks that this table and the file agree.
//! The grounds are flat and saturated, never brown or near-black. The light card takes the bright cover colour, the dark card a deeper
//! one (Speed and Potential cannot go deep and stay yellow or orange, so their dark grounds are the covers' plum and navy). The ink of
//! each ground is black or white, whichever reads better, and the muted tint is that ink pulled toward the ground as far as keeps 5:1.
//! tools/site_test/card_contrast.py checks every text colour on the real SVGs.

/// "PWR".."DEV" in chart order: the keys a card's ground and fill are chosen by.
pub const KEYS: [&str; 6] = ["PWR", "SPD", "RNG", "STA", "PRC", "DEV"];

/// The accent a card carries, from the signature: the stat the ability is built on, or for a balanced chart the stat that stood out
/// most (the rule of accentKey in site/accent.js). None when nothing was graded.
pub fn accent_key(stat: &str, top: Option<&str>) -> Option<String> {
    let key = if stat == "BAL" { top? } else { stat };
    KEYS.iter().find(|k| **k == key).map(|k| k.to_string())
}

/// Colours of one card theme.
pub struct Pal {
    /// The ground: one flat saturated cover colour.
    pub bg: &'static str,
    /// Text, rules and the inner frame on the ground: black or white, whichever reads better on it.
    pub ink: &'static str,
    /// The ground's secondary text.
    pub mute: &'static str,
    /// The ability box and the chart plate: white paper, as in the manga's chart.
    pub paper: &'static str,
    /// Black: the outline of all lettering and shapes, and the text on the paper.
    pub bk: &'static str,
    /// Secondary text on the paper (the weakness line).
    pub pmute: &'static str,
    /// The radar fill and the ゴゴゴ: a second cover colour, the complement of the ground.
    pub fill: &'static str,
}

const BLACK: &str = "#0b0b0b";
const WHITE: &str = "#ffffff";
const PAPER_MUTE: &str = "#3a3a3a";

/// One ground: its colour, its ink and muted ink, and the colour of the radar fill and ゴゴゴ on it.
struct Ground {
    bg: &'static str,
    ink: &'static str,
    mute: &'static str,
    fill: &'static str,
}

const fn g(bg: &'static str, ink: &'static str, mute: &'static str, fill: &'static str) -> Ground {
    Ground { bg, ink, mute, fill }
}

/// (key, light card, dark card). With no signature (nothing was graded) the card is the teal and the blue-teal.
const GROUNDS: [(&str, Ground, Ground); 7] = [
    ("PWR", g("#f32267", BLACK, BLACK, "#57b82f"), g("#c61544", WHITE, "#faeaee", "#57b82f")),
    ("SPD", g("#f0ab0b", BLACK, "#543e0b", "#c22cb5"), g("#884294", WHITE, "#ebdfed", "#f0ab0b")),
    ("RNG", g("#1d9efa", BLACK, "#0f2b40", "#ec834a"), g("#036ab1", WHITE, "#ebf3f9", "#ec834a")),
    ("STA", g("#57b82f", BLACK, "#1f3814", "#f32267"), g("#0e724d", WHITE, "#e4efeb", "#f32267")),
    ("PRC", g("#c22cb5", WHITE, WHITE, "#f0ab0b"), g("#6834c0", WHITE, "#dcd0f1", "#f0ab0b")),
    ("DEV", g("#ec834a", BLACK, "#43291b", "#1d9efa"), g("#064677", WHITE, "#a8becf", "#ec834a")),
    ("NONE", g("#51bc99", BLACK, "#1f3e34", "#f0ab0b"), g("#136f97", WHITE, "#eef5f8", "#f0ab0b")),
];

fn pal(key: Option<&str>, light: bool) -> Pal {
    let (_, l, d) = GROUNDS.iter().find(|(k, _, _)| Some(*k) == key).unwrap_or(&GROUNDS[6]);
    let g = if light { l } else { d };
    Pal { bg: g.bg, ink: g.ink, mute: g.mute, paper: WHITE, bk: BLACK, pmute: PAPER_MUTE, fill: g.fill }
}

pub fn light(key: Option<&str>) -> Pal {
    pal(key, true)
}

pub fn dark(key: Option<&str>) -> Pal {
    pal(key, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_accent_follows_the_signature() {
        assert_eq!(accent_key("PRC", None).as_deref(), Some("PRC"));
        assert_eq!(accent_key("BAL", Some("SPD")).as_deref(), Some("SPD"));
        assert_eq!(accent_key("BAL", None), None);
        assert_eq!(accent_key("??", None), None);
    }

    #[test]
    fn every_ground_and_fill_is_a_colour_sampled_from_the_covers() {
        // docs/ref-palette.json is written from tools/ref_palette.py's samples: each ground and fill names the cover it was sampled from
        let doc: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/ref-palette.json")).unwrap()).unwrap();
        for (key, l, d) in &GROUNDS {
            for (theme, gr) in [("light", l), ("dark", d)] {
                let e = &doc["grounds"][*key][theme];
                assert_eq!(e["bg"], gr.bg, "{key} {theme} ground");
                assert_eq!(e["fill"], gr.fill, "{key} {theme} fill");
                assert!(e["bg_from"].as_str().is_some_and(|s| !s.is_empty()) && e["fill_from"].as_str().is_some_and(|s| !s.is_empty()), "{key} {theme} sources");
            }
        }
    }

    #[test]
    fn grounds_are_saturated_and_flat_colours_not_brown_or_near_black() {
        // HSV of every ground: saturation of at least 0.5 (not grey or pastel), value of at least 0.4 (not near-black), and not in the
        // brown-brick band (orange-red hues at low value)
        fn hsv(h: &str) -> (f64, f64, f64) {
            let c: Vec<f64> = (0..3).map(|i| u8::from_str_radix(&h[1 + 2 * i..3 + 2 * i], 16).unwrap() as f64 / 255.0).collect();
            let (mx, mn) = (c[0].max(c[1]).max(c[2]), c[0].min(c[1]).min(c[2]));
            let d = mx - mn;
            let hue = if d == 0.0 { 0.0 } else if mx == c[0] { (60.0 * ((c[1] - c[2]) / d)).rem_euclid(360.0) } else if mx == c[1] { 60.0 * ((c[2] - c[0]) / d) + 120.0 } else { 60.0 * ((c[0] - c[1]) / d) + 240.0 };
            (hue, if mx == 0.0 { 0.0 } else { d / mx }, mx)
        }
        for (key, l, d) in &GROUNDS {
            for (theme, gr) in [("light", l), ("dark", d)] {
                let (h, s, v) = hsv(gr.bg);
                assert!(s >= 0.5 && v >= 0.4, "{key} {theme} {}: s {s:.2} v {v:.2}", gr.bg);
                assert!(!((0.0..=45.0).contains(&h) && v < 0.8), "{key} {theme} {} is brown or brick (hue {h:.0}, value {v:.2})", gr.bg);
            }
        }
    }

    #[test]
    fn the_light_and_dark_card_differ_and_the_fill_is_not_the_ground() {
        for (key, l, d) in &GROUNDS {
            assert_ne!(l.bg, d.bg, "{key}");
            assert_ne!(l.fill, l.bg, "{key}");
            assert_ne!(d.fill, d.bg, "{key}");
        }
        assert_eq!(light(Some("zzz")).bg, light(None).bg);
    }
}
