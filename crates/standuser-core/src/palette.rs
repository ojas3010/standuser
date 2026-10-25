//! The card's per-user palette (docs/DESIGN.md section 4).
//!
//! The accent is the page's: ACCENTS repeats site/accent.js (a test reads that file and fails if the two differ), keyed to the user's
//! signature stat. The pair is the accent plus its complement, the accent of the opposite axis on the chart: Power/Stamina pink and
//! green, Speed/Precision yellow and purple, Range/Potential blue and orange.
//! The grounds are muted and warm, mixed like brick: a pale clay for the light card, a deep brick brown for the dark one. Both were
//! chosen so that all twelve accent values pass WCAG AA (4.5:1) on them unchanged; tools/site_test/card_contrast.py checks it on the
//! real SVGs.

/// "PWR".."DEV" in chart order, each with its light-ground and dark-ground value (the same as site/accent.js).
pub const ACCENTS: [(&str, &str, &str); 6] = [
    ("PWR", "#b0104c", "#ff6da3"),
    ("SPD", "#7a5f00", "#f0c02c"),
    ("RNG", "#17659a", "#6cbcf0"),
    ("STA", "#1d7438", "#5ccd80"),
    ("PRC", "#6a3cb4", "#b69aff"),
    ("DEV", "#a94700", "#ff964a"),
];

/// The accent a card carries, from the signature: the stat the ability is built on, or for a balanced chart the stat that stood out
/// most (the rule of accentKey in site/accent.js). None when nothing was graded.
pub fn accent_key(stat: &str, top: Option<&str>) -> Option<String> {
    let key = if stat == "BAL" { top? } else { stat };
    ACCENTS.iter().find(|(k, _, _)| *k == key).map(|(k, _, _)| k.to_string())
}

/// Colours of one card theme.
pub struct Pal {
    pub bg: &'static str,
    pub ink: &'static str,
    pub mute: &'static str,
    /// The box under the ability text.
    pub paper: &'static str,
    /// The user's accent: rings, grade letters, the 能力 label, the ゴゴゴ.
    pub acc: &'static str,
    /// The accent's complement: the radar fill.
    pub pair: &'static str,
}

/// With no signature (nothing was graded) the card is brick and ink.
const NEUTRAL_LIGHT: (&str, &str) = ("#8a3b2a", "#6b5b52");
const NEUTRAL_DARK: (&str, &str) = ("#f0a58f", "#b9a59b");

pub fn light(key: Option<&str>) -> Pal {
    let (acc, pair) = colours(key, true);
    Pal { bg: "#f0e1d7", ink: "#2a140c", mute: "#5e4337", paper: "#f8eee7", acc, pair }
}

pub fn dark(key: Option<&str>) -> Pal {
    let (acc, pair) = colours(key, false);
    Pal { bg: "#4f281f", ink: "#f6ede4", mute: "#d8c4b6", paper: "#3f1f18", acc, pair }
}

fn colours(key: Option<&str>, light: bool) -> (&'static str, &'static str) {
    let pick = |i: usize| if light { ACCENTS[i].1 } else { ACCENTS[i].2 };
    match key.and_then(|k| ACCENTS.iter().position(|(a, _, _)| *a == k)) {
        Some(i) => (pick(i), pick((i + 3) % 6)),
        None if light => NEUTRAL_LIGHT,
        None => NEUTRAL_DARK,
    }
}

/// Fill opacity of the radar polygon: a flat tint of the complement, light enough that the scale letters over it stay readable.
pub const FILL_OPACITY: f64 = 0.35;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accents_are_the_pages() {
        // site/accent.js is the source of the mapping: every "KEY: { light: '#..', dark: '#..' }" line must match
        let js = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../site/accent.js")).unwrap();
        for (k, l, d) in ACCENTS {
            let line = js.lines().find(|ln| ln.trim_start().starts_with(&format!("{k}:"))).unwrap_or_else(|| panic!("{k} not in accent.js"));
            assert!(line.contains(&format!("light: '{l}'")) && line.contains(&format!("dark: '{d}'")), "{k}: {line}");
        }
    }

    #[test]
    fn the_pair_is_the_opposite_axis() {
        for (key, other) in [("PWR", "STA"), ("SPD", "PRC"), ("RNG", "DEV"), ("STA", "PWR"), ("PRC", "SPD"), ("DEV", "RNG")] {
            assert_eq!(light(Some(key)).pair, light(Some(other)).acc, "{key}");
            assert_eq!(dark(Some(key)).pair, dark(Some(other)).acc, "{key}");
        }
    }

    #[test]
    fn the_accent_follows_the_signature() {
        assert_eq!(accent_key("PRC", None).as_deref(), Some("PRC"));
        assert_eq!(accent_key("BAL", Some("SPD")).as_deref(), Some("SPD"));
        assert_eq!(accent_key("BAL", None), None);
        assert_eq!(accent_key("??", None), None);
    }
}
