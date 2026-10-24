//! Regression tests from the independent review: glyph coverage (Bug 3). Everything the card can show on its own
//! (labels, type names, ability text, Stand names) must render without a missing glyph. Only user-supplied text (film
//! titles, username) may fall back to □.

use standuser_core::font::{font, FontKind};
use standuser_core::lookup::Lookup;
use standuser_core::{analysis, parse};
use std::collections::BTreeMap;

const TABLE: &str = include_str!("../data/films.json");

fn missing(s: &str, kinds: &[FontKind]) -> String {
    let mut out = String::new();
    for k in kinds {
        let f = font(*k);
        for c in s.chars().filter(|c| !c.is_whitespace() && !f.has(*c)) {
            if !out.contains(c) {
                out.push(c);
            }
        }
    }
    out
}

/// A composer-route Stand name is drawn in the serif font. Every composer in the table must give a name the card can draw
/// (accents folded to ASCII, or the composer skipped), never a name with a hole in it.
#[test]
fn every_composer_in_the_table_gives_a_drawable_stand_name() {
    let t: serde_json::Value = serde_json::from_str(TABLE).unwrap();
    let mut by_composer: BTreeMap<String, (String, String)> = BTreeMap::new(); // person id -> (film key, name)
    for r in t["films"].as_array().unwrap() {
        for id in r[8].as_str().unwrap().split(',').filter(|s| !s.is_empty()) {
            by_composer.entry(id.to_string()).or_insert((r[0].as_str().unwrap().to_string(), t["people"][id].as_str().unwrap_or("").to_string()));
        }
    }
    assert!(by_composer.len() > 1000);
    let lk = Lookup::from_json(TABLE).unwrap();
    let (mut bad, mut checked) = (vec![], 0);
    for (key, name) in by_composer.values() {
        // a user who rated two films of this composer (the composer route needs at least two) with the same rating
        let mk = |k: &str| -> (String, i32) {
            let (n, y) = k.rsplit_once('|').unwrap();
            (n.to_string(), y.parse().unwrap())
        };
        let (n, y) = mk(key);
        let mut w = String::from("Date,Name,Year,Letterboxd URI\n");
        let mut r = String::from("Date,Name,Year,Letterboxd URI,Rating\n");
        for i in 0..2 {
            w.push_str(&format!("2024-0{}-05,{n},{y},\n", i + 1));
            r.push_str(&format!("2024-0{}-05,{n},{y},,5\n", i + 1));
        }
        let mut f = BTreeMap::new();
        f.insert("watched.csv".to_string(), w.into_bytes());
        f.insert("ratings.csv".to_string(), r.into_bytes());
        let ex = parse::parse_files(&f, Some("letterboxd-g-2026-10-19-13-22-utc.zip")).unwrap();
        let a = analysis::analyze(&ex, &lk);
        // the card draws the name inside 「」 in the serif font, so the brackets are part of what must render
        let miss = missing(&format!("「{}」", a.name.name), &[FontKind::Serif]);
        checked += 1;
        if !miss.is_empty() {
            bad.push(format!("{name:?} -> 「{}」 missing {miss:?}", a.name.name));
        }
    }
    assert!(checked > 1000);
    assert!(bad.is_empty(), "{} of {checked} composer-route names would show a missing glyph, e.g. {:?}", bad.len(), &bad[..bad.len().min(5)]);
}

/// Names go through `ascii_fold`: accents and special letters become plain capitals; anything that cannot is rejected.
#[test]
fn stand_names_are_folded_to_ascii() {
    use standuser_core::name::ascii_fold;
    assert_eq!(ascii_fold("Antonín Dvořák").as_deref(), Some("ANTONIN DVORAK"));
    assert_eq!(ascii_fold("Hans Zimmer").as_deref(), Some("HANS ZIMMER"));
    assert_eq!(ascii_fold("Björk").as_deref(), Some("BJORK"));
    assert_eq!(ascii_fold("Søren Ærø").as_deref(), Some("SOREN AERO"));
    assert_eq!(ascii_fold("Łukasz Błaszczyk").as_deref(), Some("LUKASZ BLASZCZYK"));
    assert_eq!(ascii_fold("Ennio Morricone").as_deref(), Some("ENNIO MORRICONE"));
    assert_eq!(ascii_fold("Wayne \"The Train\" Hancock").as_deref(), Some("WAYNE \"THE TRAIN\" HANCOCK"), "a straight double quote is allowed");
    assert_eq!(missing("「WAYNE \"THE TRAIN\" HANCOCK」", &[FontKind::Serif]), "", "and the card font draws it");
    assert_eq!(ascii_fold("久石 譲"), None, "a name with no ASCII form cannot be drawn");
    assert_eq!(ascii_fold("Сталкер"), None);
    assert_eq!(ascii_fold("   "), None);
}

/// Every fixed label the card draws exists in the font it is drawn with.
#[test]
fn every_fixed_card_label_has_its_glyphs() {
    use standuser_core::{grading, name, svg, types};
    let sans_only: Vec<String> = svg::JP.iter().map(|s| s.to_string())
        .chain(svg::LEGEND.iter().map(|(a, b)| format!("{a}―{b}")))
        .chain(["PROVISIONAL DATA", "ゴ", "完成", "∞", "∅", "?", "*", "本体 ―", "A B C D E", "E D C B A"].map(String::from))
        .chain([types::StandType::Automatic, types::StandType::LongDistance, types::StandType::CloseRange, types::StandType::Unclassified].iter().flat_map(|t| [t.jp().to_string(), t.en().to_string()]))
        .chain(grading::LABEL.iter().map(|s| s.to_string()))
        .collect();
    for s in &sans_only {
        assert_eq!(missing(s, &[FontKind::Sans]), "", "sans: {s}");
    }
    let serif: Vec<String> = ["【STAND NAME】", "【STAND MASTER】", "「」", "能力―", "弱点―", "…"].map(String::from).into_iter()
        .chain(name::tarot_names())
        .collect();
    for s in &serif {
        assert_eq!(missing(s, &[FontKind::Serif]), "", "serif: {s}");
        // as drawn on the card: inside the corner brackets
        assert_eq!(missing(&format!("「{s}」"), &[FontKind::Serif]), "", "serif, bracketed: {s}");
    }
    // footer and text templates
    for s in [svg::FOOTER, svg::TYPE_NOTE, svg::TYPE_NOTE_LOW] {
        assert_eq!(missing(s, &[FontKind::Sans]), "", "footer: {s}");
    }
}

/// Every ability, weakness and coverage sentence the engine can write draws without a hole (they use ★, curly quotes,
/// the ellipsis and the minus sign).
#[test]
fn every_ability_and_weakness_sentence_has_its_glyphs() {
    for s in standuser_core::ability::all_sample_sentences() {
        assert_eq!(missing(&s, &[FontKind::Serif]), "", "{s}");
    }
    for s in ["graded on 450 of 512 films", "224 of 533 dated watches", "only 4 of 5 dated watches, needs 12", "−0.13★ vs crowd", "DPA 0.88, top 3%", "47% directors seen once", "· from marking dates"] {
        assert_eq!(missing(s, &[FontKind::Sans]), "", "{s}");
    }
}
