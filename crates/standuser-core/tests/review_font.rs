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

/// A composer-route Stand name is drawn in the serif (Noto Serif JP Bold). Every composer in the table must give a name the card can draw
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
        let miss = missing(&a.name.name, &[FontKind::Serif]);
        checked += 1;
        if !miss.is_empty() {
            bad.push(format!("{name:?} -> {} missing {miss:?}", a.name.name));
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
    assert_eq!(missing("WAYNE \"THE TRAIN\" HANCOCK", &[FontKind::Serif]), "", "and the card font draws it");
    assert_eq!(ascii_fold("久石 譲"), None, "a name with no ASCII form cannot be drawn");
    assert_eq!(ascii_fold("Сталкер"), None);
    assert_eq!(ascii_fold("   "), None);
}

/// Every fixed label the card draws exists in the font it is drawn with.
#[test]
fn every_fixed_card_label_has_its_glyphs() {
    use standuser_core::{grading, name, svg, types};
    // Japanese below 48 px is Noto Sans JP Bold: the stat labels, the type names, the 完成 grade
    let jp: Vec<String> = svg::JP.iter().map(|s| s.to_string())
        .chain(["完成"].map(String::from))
        .chain([types::StandType::Automatic, types::StandType::LongDistance, types::StandType::CloseRange, types::StandType::Unclassified].iter().map(|t| t.jp().to_string()))
        .collect();
    for s in &jp {
        assert_eq!(missing(s, &[FontKind::Jp]), "", "jp: {s}");
    }
    // Japanese at 48 px or larger is Dela Gothic One: the ゴ
    assert_eq!(missing("ゴ", &[FontKind::Display]), "");
    // the eyecatch's labels are Noto Serif JP Bold, with their brackets
    for s in [svg::NAME_LABEL, svg::MASTER_LABEL] {
        assert_eq!(missing(s, &[FontKind::Serif]), "", "serif: {s}");
    }
    // Latin display text is Archivo Black: the grades, with the asterisk, and the ability names
    let grades: Vec<String> = ["A", "B", "C", "D", "E", "?", "∞", "∅", "A*", "B*", "C*", "D*", "E*", "?*", "∞*"].map(String::from).into();
    let abilities: Vec<String> = (1..=17u8).map(|i| standuser_core::ability::template_by_id(i).name.to_string()).chain(["Not yet revealed", "Ability"].map(String::from)).collect();
    for s in grades.iter().chain(&abilities) {
        assert_eq!(missing(s, &[FontKind::Name]), "", "latin display: {s}");
    }
    // all other Latin text is Archivo 700: the type names in English, the footer, the nearest canon line
    let body: Vec<String> = [types::StandType::Automatic, types::StandType::LongDistance, types::StandType::CloseRange, types::StandType::Unclassified].iter().map(|t| t.en().to_string())
        .chain(grading::LABEL.iter().map(|s| s.to_string()))
        .chain(svg::FOOTER_ROWS.map(String::from))
        .chain([svg::FOOTER.to_string(), "Nearest canon: ".into()])
        .collect();
    for s in &body {
        assert_eq!(missing(s, &[FontKind::Body]), "", "body: {s}");
    }
    let canon: Vec<String> = include_str!("../data/canon_names.txt").lines().map(String::from).collect();
    for s in &canon {
        assert_eq!(missing(s, &[FontKind::Body]), "", "nearest canon: {s}");
    }
    // the Stand name is drawn in the serif, the ellipsis of a cut name too
    let name_font: Vec<String> = ["…"].map(String::from).into_iter().chain(name::tarot_names()).collect();
    for s in &name_font {
        assert_eq!(missing(s, &[FontKind::Serif]), "", "serif: {s}");
    }
}

/// Every sentence the card draws under an ability name draws without a hole in Archivo 700 (no ★: Archivo has none).
#[test]
fn every_card_sentence_has_its_glyphs() {
    for s in standuser_core::ability::all_card_lines() {
        assert_eq!(missing(&s, &[FontKind::Body]), "", "{s}");
    }
}

/// Each face keeps to its job. The name font: Basic Latin, Latin-1 and the brackets, not kana, Greek or Cyrillic. The display font is
/// Dela Gothic One for Japanese: kana, the kanji of the labels and 【】, and no Latin at all. The body font is Archivo: Latin, no kana.
#[test]
fn the_card_fonts_keep_to_their_job() {
    let name = font(FontKind::Name);
    assert!("ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 '&-.,!?:\"「」…∞∅*".chars().all(|c| name.has(c)));
    assert!(!name.has('ひ') && !name.has('Ж'));
    let serif = font(FontKind::Serif);
    assert!("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 '&-.,!?:\"…【】".chars().all(|c| serif.has(c)));
    assert!(serif.has('ひ') && !serif.has('ع') && !serif.has('ก'), "the serif is cut to what names.json (its artist credits too) and the labels use");
    let display = font(FontKind::Display);
    assert!("ゴスピードニガテひらがな【】「」破壊力".chars().all(|c| display.has(c)));
    assert!(!display.has('Ж') && !display.has('ά') && !"ABCDEabcde0123456789".chars().any(|c| display.has(c)), "Dela Gothic One is for Japanese only");
    let body = font(FontKind::Body);
    assert!("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 '&-.,!?:\"()·–—…%*".chars().all(|c| body.has(c)));
    assert!(!body.has('ひ') && !body.has('★'));
    let jp = font(FontKind::Jp);
    assert!("破壊力スピード射程距離持続力精密動作性成長性完成近距離パワー型遠隔自動操縦未分類".chars().all(|c| jp.has(c)));
}

/// A track-title Stand name is drawn in the serif. Every character of every candidate title in names.json must have a glyph in serif.ttf;
/// a failure lists each missing character with the names that use it. (The artist credits in names.json are in serif.ttf too wherever
/// Noto Serif JP has the character: see tools/make_serif_font.py; none of them is drawn on the card.)
#[test]
fn every_stand_name_in_names_json_is_drawable_in_the_serif() {
    let names: serde_json::Value = serde_json::from_str(include_str!("../data/names.json")).unwrap();
    let serif = font(FontKind::Serif);
    let mut by_char: BTreeMap<char, Vec<String>> = BTreeMap::new();
    let mut total = 0;
    for v in names["names"].as_object().unwrap().values() {
        for cand in v[1].as_array().unwrap() {
            let title = cand[0].as_str().unwrap();
            total += 1;
            for c in title.chars().filter(|c| !c.is_whitespace() && !serif.has(*c)) {
                let e = by_char.entry(c).or_default();
                if !e.contains(&title.to_string()) {
                    e.push(title.to_string());
                }
            }
        }
    }
    assert!(total > 20000, "{total}");
    let report: Vec<String> = by_char.iter().map(|(c, n)| format!("{c:?} U+{:04X} ({} names): {:?}", *c as u32, n.len(), &n[..n.len().min(8)])).collect();
    assert!(by_char.is_empty(), "{} characters missing from serif.ttf over {total} names:\n{}", by_char.len(), report.join("\n"));
}
