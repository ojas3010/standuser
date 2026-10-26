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

/// A composer-route Stand name is drawn in the name font (Archivo Black). Every composer in the table must give a name the card can draw
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
        let miss = missing(&format!("「{}」", a.name.name), &[FontKind::Name]);
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
    assert_eq!(missing("「WAYNE \"THE TRAIN\" HANCOCK」", &[FontKind::Name]), "", "and the card font draws it");
    assert_eq!(ascii_fold("久石 譲"), None, "a name with no ASCII form cannot be drawn");
    assert_eq!(ascii_fold("Сталкер"), None);
    assert_eq!(ascii_fold("   "), None);
}

/// Every fixed label the card draws exists in the font it is drawn with.
#[test]
fn every_fixed_card_label_has_its_glyphs() {
    use standuser_core::{grading, name, svg, types};
    // the manga lettering is drawn in the display font (Dela Gothic One): the stat labels, the legend, the type names and the labels
    let display: Vec<String> = svg::JP.iter().map(|s| s.to_string())
        .chain(svg::LEGEND.iter().map(|(a, b)| format!("{a}―{b}")))
        .chain(["ゴ", "完成", "∞", "∅", "?", "*", "本体―", "能力―", "弱点―", "A", "B", "C", "D", "E", "【STAND NAME】", "【STAND MASTER】"].map(String::from))
        .chain(svg::JP.iter().flat_map(|j| ["A", "B", "C", "D", "E", "?", "∞", "∅", "完成"].map(|g| format!("{j}―{g}*"))))
        .chain([types::StandType::Automatic, types::StandType::LongDistance, types::StandType::CloseRange, types::StandType::Unclassified].iter().map(|t| t.jp().to_string()))
        .collect();
    for s in &display {
        assert_eq!(missing(s, &[FontKind::Display]), "", "display: {s}");
    }
    let sans_only: Vec<String> = ["PROVISIONAL DATA", "A B C D E", "E D C B A"].map(String::from).into_iter()
        .chain([types::StandType::Automatic, types::StandType::LongDistance, types::StandType::CloseRange, types::StandType::Unclassified].iter().map(|t| t.en().to_string()))
        .chain(grading::LABEL.iter().map(|s| s.to_string()))
        .collect();
    for s in &sans_only {
        assert_eq!(missing(s, &[FontKind::Sans]), "", "sans: {s}");
    }
    let serif: Vec<String> = ["「」", "…"].map(String::from).into_iter()
        .chain(name::tarot_names())
        .collect();
    for s in &serif {
        assert_eq!(missing(s, &[FontKind::Name]), "", "name font: {s}");
        // as drawn on the card: inside the corner brackets
        assert_eq!(missing(&format!("「{s}」"), &[FontKind::Name]), "", "name font, bracketed: {s}");
    }
    // footer and text templates
    for s in [svg::FOOTER, svg::TYPE_NOTE, svg::TYPE_NOTE_LOW] {
        assert_eq!(missing(s, &[FontKind::Sans]), "", "footer: {s}");
    }
}

/// Every ability, weakness and coverage sentence the engine can write draws without a hole (they use ★, curly quotes,
/// the ellipsis and the minus sign). The ability box is drawn in the sans.
#[test]
fn every_ability_and_weakness_sentence_has_its_glyphs() {
    for s in standuser_core::ability::all_sample_sentences() {
        assert_eq!(missing(&s, &[FontKind::Sans]), "", "{s}");
    }
    for s in ["graded on 450 of 512 films", "224 of 533 dated watches", "only 4 of 5 dated watches, needs 12", "−0.13★ vs crowd", "DPA 0.88, top 3%", "47% directors seen once", "· from marking dates"] {
        assert_eq!(missing(s, &[FontKind::Sans]), "", "{s}");
    }
}

/// The why line carries an artist credit from names.json, drawn in the sans. Every character of every credit must have a
/// glyph (Japanese, Korean and Chinese names, Latin Extended letters, Greek with tonos): the names are not folded or dropped.
#[test]
fn every_artist_credit_in_names_json_is_drawable() {
    let names: serde_json::Value = serde_json::from_str(include_str!("../data/names.json")).unwrap();
    let (mut credits, mut bad) = (0, vec![]);
    for v in names["names"].as_object().unwrap().values() {
        for artist in v[0].as_array().unwrap() {
            let a = artist[0].as_str().unwrap();
            credits += 1;
            let miss = missing(a, &[FontKind::Sans]);
            if !miss.is_empty() {
                bad.push(format!("{a:?} missing {miss:?}"));
            }
        }
    }
    assert!(credits > 1000);
    assert!(bad.is_empty(), "{} credits would draw a box, e.g. {:?}", bad.len(), &bad[..bad.len().min(5)]);
}

/// The name font is for the Stand name only: Basic Latin, Latin-1 and the brackets, not the kana, Greek or Cyrillic that the sans
/// draws. The display font is the manga lettering: kana, the kanji of the labels, Basic Latin and 【】, not Greek or Cyrillic.
#[test]
fn the_display_fonts_keep_to_their_job() {
    let name = font(FontKind::Name);
    assert!("ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 '&-.,!?:\"「」…".chars().all(|c| name.has(c)));
    assert!(!name.has('ひ') && !name.has('Ж'));
    let display = font(FontKind::Display);
    assert!("ゴゴゴスピードニガテひらがな【】「」ABCDE".chars().all(|c| display.has(c)));
    assert!(!display.has('Ж') && !display.has('ά'));
}

/// A track-title Stand name is drawn in the name font inside 「」. Every character of every candidate title in names.json must
/// have a glyph in name.ttf; a failure lists each missing character with the names that use it.
#[test]
fn every_stand_name_in_names_json_is_drawable_in_the_name_font() {
    let names: serde_json::Value = serde_json::from_str(include_str!("../data/names.json")).unwrap();
    let serif = font(FontKind::Name);
    let mut by_char: BTreeMap<char, Vec<String>> = BTreeMap::new();
    let mut total = 0;
    for v in names["names"].as_object().unwrap().values() {
        for cand in v[1].as_array().unwrap() {
            let title = cand[0].as_str().unwrap();
            total += 1;
            for c in format!("「{title}」").chars().filter(|c| !c.is_whitespace() && !serif.has(*c)) {
                let e = by_char.entry(c).or_default();
                if !e.contains(&title.to_string()) {
                    e.push(title.to_string());
                }
            }
        }
    }
    assert!(total > 20000, "{total}");
    let report: Vec<String> = by_char.iter().map(|(c, n)| format!("{c:?} U+{:04X} ({} names): {:?}", *c as u32, n.len(), &n[..n.len().min(8)])).collect();
    assert!(by_char.is_empty(), "{} characters missing from name.ttf over {total} names:\n{}", by_char.len(), report.join("\n"));
}
