//! Stand name (naming algorithm, docs/naming-spec.md with the 25 Oct changes in docs/DECISIONS.md).
//!
//! First the track-title route (`pool`): the user's top films by affinity that have soundtrack candidates in names.json,
//! scored S = 0.45 Cool + 0.35 F + 0.20 A. Fallbacks in order: the composer route (the composer of your rated films with the
//! highest shrunk mean, else your most frequent), then Part 3 mode (colour + Tarot card).

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
    /// "titles" | "composer" | "tarot" | "none"
    pub route: String,
    pub why: String,
    /// The five best candidates of the track-title route (empty on the other routes), the pick marked.
    #[serde(default)]
    pub top: Vec<crate::pool::Scored>,
    /// What the first-track rule would have named the Stand (track-title route only).
    #[serde(default)]
    pub first_track: Option<crate::pool::FirstTrack>,
}

impl NameResult {
    fn plain(name: String, route: &str, why: String) -> NameResult {
        NameResult { name, route: route.into(), why, top: vec![], first_track: None }
    }
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

/// Fold a name to the plain capitals the card font can draw: accents are removed, special letters are spelled out
/// (Ø to O, Æ to AE, ß to SS, Ł to L), curly quotes and dashes become ASCII. Returns None when the name has letters with
/// no ASCII form (CJK, Cyrillic, Arabic...) or nothing drawable at all.
pub fn ascii_fold(name: &str) -> Option<String> {
    let mut out = String::new();
    for c in name.nfkd() {
        if unicode_normalization::char::is_combining_mark(c) {
            continue;
        }
        match c {
            'ø' | 'Ø' => out.push('O'),
            'æ' | 'Æ' => out.push_str("AE"),
            'œ' | 'Œ' => out.push_str("OE"),
            'ß' | 'ẞ' => out.push_str("SS"),
            'đ' | 'Đ' | 'ð' | 'Ð' => out.push('D'),
            'þ' | 'Þ' => out.push_str("TH"),
            'ł' | 'Ł' => out.push('L'),
            'ı' => out.push('I'),
            'ħ' | 'Ħ' => out.push('H'),
            '\u{2018}' | '\u{2019}' | '`' | '\u{00B4}' => out.push('\''),
            '\u{2010}'..='\u{2015}' | '\u{2212}' => out.push('-'),
            c if c.is_ascii_alphabetic() => out.push(c.to_ascii_uppercase()),
            c if c.is_ascii_digit() => out.push(c),
            c if c.is_whitespace() => out.push(' '),
            '.' | '\'' | '"' | '-' | '&' | ',' | '!' | '?' => out.push(c),
            _ => return None,
        }
    }
    let out = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if out.chars().any(|c| c.is_ascii_alphanumeric()) { Some(out) } else { None }
}

/// Every Tarot-route name the card can show (colour of each stat extreme, plus the card), upper case.
pub fn tarot_names() -> Vec<String> {
    let mut v: Vec<String> = CARDS.iter().map(|c| c.to_uppercase()).collect();
    for (hi, lo) in COLOURS {
        for colour in [hi, lo] {
            for card in CARDS {
                v.push(format!("{colour} {card}").to_uppercase());
            }
        }
    }
    v
}

struct Cand {
    id: u32,
    rated_sum: f64,
    rated_n: usize,
    watched_n: usize,
    crowd_sum: f64,
    crowd_n: usize,
}

/// A usable composer name: foldable to drawable ASCII and not a canon Stand name.
fn usable(lk: &Lookup, id: u32) -> Option<(String, String)> {
    let who = lk.person(id);
    let folded = ascii_fold(&who)?;
    if collides_with_canon(&folded) { None } else { Some((who, folded)) }
}

pub fn name_stand(ex: &Export, lk: &Lookup, sig: Option<&Signature>, template_id: Option<u8>) -> NameResult {
    let Some(sig) = sig else {
        return NameResult::plain("???".into(), "none", "No stat could be graded.".into());
    };
    if let Some(p) = crate::pool::pick(ex, lk, template_id.unwrap_or(17), ex.username.as_deref().unwrap_or("unknown")) {
        return NameResult { name: p.name, route: "titles".into(), why: p.why, top: p.top, first_track: p.first_track };
    }
    // ---- composer pool: the user's rated films, and their watched films (for users without ratings)
    let mut by: BTreeMap<u32, Cand> = BTreeMap::new();
    let ok_runtime = |m: &crate::lookup::FilmMeta| m.runtime == 0 || m.runtime >= crate::stats::MIN_RUNTIME;
    for r in &ex.ratings {
        if let Some(m) = lk.find(boxd_code(&r.uri), &r.name, r.year) {
            if !ok_runtime(m) {
                continue;
            }
            let mut ids = m.composers.clone();
            ids.sort();
            ids.dedup();
            for c in ids {
                let e = by.entry(c).or_insert(Cand { id: c, rated_sum: 0.0, rated_n: 0, watched_n: 0, crowd_sum: 0.0, crowd_n: 0 });
                e.rated_sum += r.rating;
                e.rated_n += 1;
            }
        }
    }
    for w in &ex.watched {
        if let Some(m) = lk.find(boxd_code(&w.uri), &w.name, w.year) {
            if !ok_runtime(m) {
                continue;
            }
            let mut ids = m.composers.clone();
            ids.sort();
            ids.dedup();
            for c in ids {
                let e = by.entry(c).or_insert(Cand { id: c, rated_sum: 0.0, rated_n: 0, watched_n: 0, crowd_sum: 0.0, crowd_n: 0 });
                e.watched_n += 1;
                if let Some(a) = m.lb_avg {
                    e.crowd_sum += a;
                    e.crowd_n += 1;
                }
            }
        }
    }
    let cands: Vec<&Cand> = by.values().collect();
    let mu = if ex.ratings.is_empty() { 0.0 } else { ex.ratings.iter().map(|r| r.rating).sum::<f64>() / ex.ratings.len() as f64 };
    let shrunk = |c: &Cand| (c.rated_sum + 3.0 * mu) / (c.rated_n as f64 + 3.0);
    let crowd = |c: &Cand| if c.crowd_n > 0 { c.crowd_sum / c.crowd_n as f64 } else { 0.0 };

    // 1. users with ratings: composers on at least two rated films, ranked by the shrunk mean of the user's ratings
    let mut rated: Vec<&Cand> = cands.iter().copied().filter(|c| c.rated_n >= 2).collect();
    rated.sort_by(|a, b| shrunk(b).total_cmp(&shrunk(a)).then(b.rated_n.cmp(&a.rated_n)).then(a.id.cmp(&b.id)));
    for c in rated {
        if let Some((who, name)) = usable(lk, c.id) {
            return NameResult::plain(
                name.clone(),
                "composer",
                format!("Why 「{name}」: {} of your rated films credit {who}; their shrunk mean of {:.2}★ makes {who} your top film composer.", c.rated_n, shrunk(c)),
            );
        }
    }
    // 2. users without ratings (and the anchor fallback): the most frequent composers among watched films, ties to the
    //    higher Letterboxd average of those films, then the lower id
    let mut freq: Vec<&Cand> = cands.iter().copied().filter(|c| c.watched_n >= 1).collect();
    freq.sort_by(|a, b| b.watched_n.cmp(&a.watched_n).then(crowd(b).total_cmp(&crowd(a))).then(a.id.cmp(&b.id)));
    for c in freq {
        if let Some((who, name)) = usable(lk, c.id) {
            return NameResult::plain(
                name.clone(),
                "composer",
                format!("Why 「{name}」: {} of your watched films credit {who}, your most frequent film composer (their films average {:.2}★ on Letterboxd).", c.watched_n, crowd(c)),
            );
        }
    }
    // ---- Part 3 mode, only when no composer credit is usable: colour of the runner-up stat's extreme + Tarot card
    let t = template_id.unwrap_or(17) as usize;
    let card = CARDS[t.clamp(1, 17) - 1];
    let (stat, z) = match &sig.runner {
        Some(r) => (r.as_str(), sig.zs.iter().find(|(s, _)| s == r).map(|(_, z)| *z).unwrap_or(0.0)),
        None => (sig.top.as_deref().unwrap_or(sig.stat.as_str()), sig.z),
    };
    let ix = STATS.iter().position(|s| *s == stat).unwrap_or(0);
    let colour = if z >= 0.0 { COLOURS[ix].0 } else { COLOURS[ix].1 };
    let name = format!("{colour} {card}").to_uppercase();
    NameResult::plain(name.clone(), "tarot", format!("Why 「{name}」: none of your films carries a usable composer credit, so the name is the colour of your {} extreme plus the Tarot card of your ability.", crate::grading::LABEL[ix]))
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

    fn lk3() -> Lookup {
        Lookup::from_json(r#"{"people":{"9":"Antonín Dvořák","8":"久石 譲","7":"Hans Zimmer"},"films":[
          ["a|2000","",1,2000,100,"en","US","","9",3.0,null,100000],
          ["b|2001","",2,2001,100,"en","US","","9,8",4.0,null,100000],
          ["c|2002","",3,2002,100,"en","US","","8",4.5,null,100000],
          ["d|2003","",4,2003,100,"en","US","","7",3.5,null,100000]]}"#).unwrap()
    }
    fn watched(names: &[(&str, i32)]) -> Export {
        let mut ex = Export::default();
        for (i, (n, y)) in names.iter().enumerate() {
            ex.watched.push(crate::parse::Watched { day: 19000 + i as i64, name: (*n).into(), year: *y, uri: String::new() });
        }
        ex
    }
    fn sig() -> Signature {
        Signature { stat: "PRC".into(), z: 2.0, top: None, runner: None, zs: vec![] }
    }

    #[test]
    fn unrated_users_get_their_most_frequent_composer_not_tarot() {
        let ex = watched(&[("A", 2000), ("B", 2001), ("C", 2002), ("D", 2003)]);
        let n = name_stand(&ex, &lk3(), Some(&sig()), Some(12));
        // 久石 譲 (3 credits) has no ASCII form and is skipped; Antonín Dvořák (2) is next, folded for the font
        assert_eq!((n.route.as_str(), n.name.as_str()), ("composer", "ANTONIN DVORAK"), "{n:?}");
        assert!(n.why.contains("watched films"));
    }

    #[test]
    fn frequency_ties_go_to_the_higher_crowd_average_then_the_lower_id() {
        let lk = Lookup::from_json(r#"{"people":{"1":"Ann One","2":"Bob Two"},"films":[
          ["a|2000","",1,2000,100,"en","US","","1",3.0,null,100000],["b|2001","",2,2001,100,"en","US","","2",4.2,null,100000]]}"#).unwrap();
        assert_eq!(name_stand(&watched(&[("A", 2000), ("B", 2001)]), &lk, Some(&sig()), None).name, "BOB TWO");
        let lk = Lookup::from_json(r#"{"people":{"1":"Ann One","2":"Bob Two"},"films":[
          ["a|2000","",1,2000,100,"en","US","","1",3.0,null,100000],["b|2001","",2,2001,100,"en","US","","2",3.0,null,100000]]}"#).unwrap();
        assert_eq!(name_stand(&watched(&[("A", 2000), ("B", 2001)]), &lk, Some(&sig()), None).name, "ANN ONE");
    }

    #[test]
    fn rated_users_without_a_repeat_composer_still_get_a_composer_name() {
        let mut ex = watched(&[("D", 2003)]);
        ex.ratings.push(crate::parse::Rating { name: "D".into(), year: 2003, rating: 4.0, uri: String::new() });
        let n = name_stand(&ex, &lk3(), Some(&sig()), Some(12));
        assert_eq!((n.route.as_str(), n.name.as_str()), ("composer", "HANS ZIMMER"), "{n:?}");
    }

    #[test]
    fn tarot_only_when_no_composer_credit_is_usable() {
        // the only composer on these films has no ASCII form
        let lk = Lookup::from_json(r#"{"people":{"8":"久石 譲"},"films":[["c|2002","",3,2002,100,"en","US","","8",4.5,null,100000]]}"#).unwrap();
        let n = name_stand(&watched(&[("C", 2002)]), &lk, Some(&sig()), Some(12));
        assert_eq!(n.route, "tarot");
        let n = name_stand(&watched(&[("zzz", 1999)]), &lk, Some(&sig()), Some(12));
        assert_eq!(n.route, "tarot");
    }

    #[test]
    fn folding() {
        assert_eq!(ascii_fold("Antonín Dvořák").as_deref(), Some("ANTONIN DVORAK"));
        assert_eq!(ascii_fold("O’Brien – Jr.").as_deref(), Some("O'BRIEN - JR."));
        assert_eq!(ascii_fold("久石 譲"), None);
        assert_eq!(tarot_names().len(), 17 + 6 * 2 * 17);
    }

    #[test]
    fn fallback_order_titles_then_composer_then_tarot() {
        let ex = watched(&[("A", 2000), ("B", 2001), ("C", 2002), ("D", 2003)]);
        // 1. a watched film has track-title candidates: the title route
        let mut lk = lk3();
        lk.load_names(r#"{"v":1,"names":{"1":["Some Band","o",[["WOUNDED HUNTER",70,3,"v","",[]],["STRANDED",60,5,"v","",[]]]]}}"#).unwrap();
        let n = name_stand(&ex, &lk, Some(&sig()), Some(12));
        assert_eq!(n.route, "titles", "{n:?}");
        assert!(n.name == "WOUNDED HUNTER" || n.name == "STRANDED");
        assert!(n.why.starts_with("Why 「"), "{}", n.why);
        assert!(!n.top.is_empty() && n.top.iter().filter(|t| t.picked).count() == 1);
        // 2. names exist, but none for this user's films: the composer route
        let mut lk = lk3();
        lk.load_names(r#"{"v":1,"names":{"99":["x","o",[["NOT YOURS",70,3,"v","",[]]]]}}"#).unwrap();
        let n = name_stand(&ex, &lk, Some(&sig()), Some(12));
        assert_eq!((n.route.as_str(), n.name.as_str()), ("composer", "ANTONIN DVORAK"), "{n:?}");
        assert!(n.top.is_empty());
        // 3. no names and no usable composer: Tarot plus colour
        let lk = Lookup::from_json(r#"{"people":{"8":"久石 譲"},"films":[["c|2002","",3,2002,100,"en","US","","8",4.5,null,100000]]}"#).unwrap();
        assert_eq!(name_stand(&watched(&[("C", 2002)]), &lk, Some(&sig()), Some(12)).route, "tarot");
        // 4. nothing graded: ???
        assert_eq!(name_stand(&ex, &lk3(), None, None).route, "none");
    }
}
