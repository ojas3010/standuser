//! Stand name (offline route of the naming algorithm, reports/Stand naming algorithm.md).
//!
//! Stages 3 and 9 run here: the personal pool by composer, then the fallbacks Route A (the anchor composer) and
//! Part 3 mode (colour + Tarot card). Soundtrack track titles (stages 4 to 8) need MusicBrainz/Discogs lookups and
//! are not available offline, so music-title names are not produced; see README "Limits".

use crate::ability::Signature;
use crate::grading::STATS;
use crate::lookup::Lookup;
use crate::parse::{boxd_code, Export};
use serde::Serialize;
use std::collections::BTreeMap;
use unicode_normalization::UnicodeNormalization;

const CANON_NAMES: &str = include_str!("../data/canon_names.txt");

#[derive(Debug, Clone, Serialize)]
pub struct NameResult {
    /// Upper-case English name without the 「」 brackets; "???" when no stat could be graded.
    pub name: String,
    /// "composer" | "tarot" | "none"
    pub route: String,
    pub why: String,
}

const CARDS: [&str; 17] = [
    "Tower", "Temperance", "Chariot", "World", "Hanged Man", "Hierophant", "Sun", "Strength", "Wheel of Fortune", "Death", "High Priestess", "Justice", "Moon", "Empress", "Hermit", "Star", "Fool",
];
const COLOURS: [(&str, &str); 6] = [("Red", "Gray"), ("Silver", "Ebony"), ("Green", "Yellow"), ("Black", "Crimson"), ("Platinum", "White"), ("Gold", "Purple")];

fn norm(s: &str) -> String {
    let t: String = s.nfkd().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
    t.strip_prefix("the").map(String::from).unwrap_or(t)
}

fn edit_le1(a: &str, b: &str) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let (mut i, mut j, mut diff) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            i += 1;
            j += 1;
        } else {
            diff += 1;
            if diff > 1 {
                return false;
            }
            if a.len() > b.len() {
                i += 1;
            } else if b.len() > a.len() {
                j += 1;
            } else {
                i += 1;
                j += 1;
            }
        }
    }
    diff + (a.len() - i) + (b.len() - j) <= 1
}

/// Canon collision: equal after normalisation, or one edit apart when the name has 8+ characters.
pub fn collides_with_canon(name: &str) -> bool {
    let n = norm(name);
    CANON_NAMES.lines().any(|c| {
        // "Star Platinum (Part 3)" collides as "Star Platinum"
        let c = norm(c.split(" (").next().unwrap_or(c));
        c == n || (n.chars().count() >= 8 && edit_le1(&c, &n))
    })
}

pub fn name_stand(ex: &Export, lk: &Lookup, sig: Option<&Signature>, template_id: Option<u8>) -> NameResult {
    let Some(sig) = sig else {
        return NameResult { name: "???".into(), route: "none".into(), why: "No stat could be graded.".into() };
    };
    // ---- Route A: the anchor composer (shrunk mean of the user's ratings per composer, n_c >= 2)
    let mu = if ex.ratings.is_empty() { 0.0 } else { ex.ratings.iter().map(|r| r.rating).sum::<f64>() / ex.ratings.len() as f64 };
    let mut by: BTreeMap<u32, (f64, usize)> = BTreeMap::new();
    for r in &ex.ratings {
        if let Some(m) = lk.find(boxd_code(&r.uri), &r.name, r.year) {
            if m.runtime != 0 && m.runtime < crate::stats::MIN_RUNTIME {
                continue;
            }
            let mut ids = m.composers.clone();
            ids.sort();
            ids.dedup();
            for c in ids {
                let e = by.entry(c).or_default();
                e.0 += r.rating;
                e.1 += 1;
            }
        }
    }
    let mut ranked: Vec<(u32, f64, usize)> = by.into_iter().filter(|(_, (_, n))| *n >= 2).map(|(id, (s, n))| (id, (s + 3.0 * mu) / (n as f64 + 3.0), n)).collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(b.2.cmp(&a.2)).then(a.0.cmp(&b.0)));
    for (id, m, n) in ranked {
        let who = lk.person(id);
        if collides_with_canon(&who) {
            continue;
        }
        return NameResult {
            name: who.to_uppercase(),
            route: "composer".into(),
            why: format!("Why 「{}」: {} of your rated films credit {}; their shrunk mean of {:.2}★ makes {} your top film composer.", who.to_uppercase(), n, who, m, who),
        };
    }
    // ---- Part 3 mode: colour of the runner-up stat's extreme + the Tarot card of the ability template
    let t = template_id.unwrap_or(17) as usize;
    let card = CARDS[t.clamp(1, 17) - 1];
    let (stat, z) = match &sig.runner {
        Some(r) => (r.as_str(), sig.zs.iter().find(|(s, _)| s == r).map(|(_, z)| *z).unwrap_or(0.0)),
        None => (sig.top.as_deref().unwrap_or(sig.stat.as_str()), sig.z),
    };
    let ix = STATS.iter().position(|s| *s == stat).unwrap_or(0);
    let colour = if z >= 0.0 { COLOURS[ix].0 } else { COLOURS[ix].1 };
    let name = format!("{colour} {card}").to_uppercase();
    NameResult { name: name.clone(), route: "tarot".into(), why: format!("Why 「{name}」: no composer credited on at least two of your rated films, so the name is the colour of your {} extreme plus the Tarot card of your ability.", crate::grading::LABEL[ix]) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collisions() {
        assert!(collides_with_canon("Star Platinum"));
        assert!(collides_with_canon("the world"));
        assert!(!collides_with_canon("Howard Shore"));
        assert!(edit_le1("kraftwork", "kraftwerk"));
        assert!(!edit_le1("abc", "xyz"));
    }

    #[test]
    fn tarot_fallback() {
        let ex = Export::default();
        let lk = Lookup::default();
        let sig = Signature { stat: "PRC".into(), z: 2.0, top: None, runner: Some("RNG".into()), zs: vec![("PRC".into(), 2.0), ("RNG".into(), -1.05)] };
        let n = name_stand(&ex, &lk, Some(&sig), Some(12));
        assert_eq!(n.name, "YELLOW JUSTICE");
        assert_eq!(n.route, "tarot");
        assert_eq!(name_stand(&ex, &lk, None, None).name, "???");
    }

    #[test]
    fn composer_route() {
        let mut ex = Export::default();
        let lk = Lookup::from_json(r#"{"people":{"9":"Howard Shore"},"films":[["a|2000","a",1,2000,100,"en","US","","9",null,null,null],["b|2001","b",2,2001,100,"en","US","","9",null,null,null]]}"#).unwrap();
        for (n, y, uri) in [("A", 2000, "https://boxd.it/a"), ("B", 2001, "https://boxd.it/b")] {
            ex.ratings.push(crate::parse::Rating { name: n.into(), year: y, rating: 4.5, uri: uri.into() });
        }
        let sig = Signature { stat: "PRC".into(), z: 2.0, top: None, runner: None, zs: vec![] };
        let n = name_stand(&ex, &lk, Some(&sig), Some(12));
        assert_eq!(n.name, "HOWARD SHORE");
        assert_eq!(n.route, "composer");
    }
}
