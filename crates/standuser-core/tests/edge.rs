//! Degenerate exports must produce a card, not a panic.

use standuser_core::{analysis::analyze, lookup::Lookup, parse, svg};
use std::collections::BTreeMap;

fn files(w: &str, r: Option<&str>, d: Option<&str>) -> BTreeMap<String, Vec<u8>> {
    let mut f = BTreeMap::new();
    f.insert("watched.csv".to_string(), w.as_bytes().to_vec());
    if let Some(r) = r {
        f.insert("ratings.csv".to_string(), r.as_bytes().to_vec());
    }
    if let Some(d) = d {
        f.insert("diary.csv".to_string(), d.as_bytes().to_vec());
    }
    f
}

fn card(f: &BTreeMap<String, Vec<u8>>) -> (standuser_core::analysis::Analysis, String) {
    let ex = parse::parse_files(f, Some("letterboxd-edge-2026-10-19-13-22-utc.zip")).unwrap();
    let a = analyze(&ex, &Lookup::default());
    let s = svg::card_svg(&a.card, svg::Theme::Auto);
    assert!(s.starts_with("<svg") && s.ends_with("</svg>"));
    (a, s)
}

#[test]
fn empty_watched_list() {
    let (a, _) = card(&files("Date,Name,Year,Letterboxd URI\n", None, None));
    assert_eq!(a.card.grades, vec!["∅", "∅", "∅", "∅", "∅", "∅"]);
    assert_eq!(a.name.name, "???");
    assert_eq!(a.kind, standuser_core::types::StandType::Unclassified);
    // ∅ is a value (0) in the matcher, so an empty export lands on the canon Stand whose cells are all ∅
    assert_eq!(a.nearest[0].name, "Gold Experience Requiem");
}

#[test]
fn watched_only_with_an_empty_table() {
    let w = "Date,Name,Year,Letterboxd URI\n2024-01-05,A,2000,https://boxd.it/a\n2024-02-05,B,2001,https://boxd.it/b\n";
    let (a, _) = card(&files(w, None, None));
    // no ratings -> Power and Precision are ∅; two events -> Speed and Stamina are "?"
    assert_eq!(a.card.grades[0], "∅");
    assert_eq!(a.card.grades[4], "∅");
    assert_eq!(a.card.grades[1], "?");
    assert_eq!(a.coverage.not_in_table, 2);
    assert!(a.ability.template.is_none());
    assert!(a.card.nearest[0].contains("needs 4"));
}

#[test]
fn ratings_without_table_are_unknown_not_zero() {
    let w = "Date,Name,Year,Letterboxd URI\n2024-01-05,A,2000,https://boxd.it/a\n";
    let r = "Date,Name,Year,Letterboxd URI,Rating\n2024-01-05,A,2000,https://boxd.it/a,4\n";
    let (a, _) = card(&files(w, Some(r), None));
    assert_eq!(a.card.grades[0], "?");
    assert_eq!(a.card.grades[4], "?");
    assert!(a.notes.iter().any(|n| n.contains("not in the lookup table")));
}

#[test]
fn unicode_titles_and_quotes_render() {
    let w = "Date,Name,Year,Letterboxd URI\n2024-01-05,\"Amélie \"\"Fabuleux\"\" <&> 한국어\",2001,https://boxd.it/a\n";
    let (a, s) = card(&files(w, None, None));
    assert!(!s.contains("<&>"), "text is outlined and escaped");
    assert_eq!(a.coverage.not_in_table, 1);
}

#[test]
fn zip_without_profile_uses_defaults() {
    let w = "Date,Name,Year,Letterboxd URI\n2024-01-05,A,2000,https://boxd.it/a\n";
    let (a, _) = card(&files(w, None, None));
    assert_eq!(a.joined, "2024-01-05");
    assert_eq!(a.as_of, "2026-10-19");
    assert_eq!(a.username, "edge");
    assert!(a.notes.iter().any(|n| n.contains("no Date Joined found")));
}

#[test]
fn very_long_username_and_ability_fit() {
    let w = "Date,Name,Year,Letterboxd URI\n2024-01-05,A,2000,https://boxd.it/a\n";
    let mut ex = parse::parse_files(&files(w, None, None), None).unwrap();
    ex.username = Some("x".repeat(300));
    let a = analyze(&ex, &Lookup::default());
    let s = svg::card_svg(&a.card, svg::Theme::Light);
    assert!(s.len() < 1_500_000);
}

#[test]
fn coverage_is_reported_and_below_minimum_is_a_question_mark() {
    // 5 films, all in the table with countries and directors: far below the minimums of Range (50) and Potential (40)
    let mut rows = String::new();
    let mut w = String::from("Date,Name,Year,Letterboxd URI\n");
    for i in 0..5 {
        rows.push_str(&format!("[\"film{i}|2000\",\"f{i}\",{i},2000,100,\"en\",\"US|FR\",\"1\",\"2\",3.5,null,100000],"));
        w.push_str(&format!("2024-0{}-05,Film{i},2000,https://boxd.it/f{i}\n", i + 1));
    }
    let table = format!("{{\"v\":1,\"films\":[{}]}}", rows.trim_end_matches(','));
    let ex = parse::parse_files(&files(&w, None, None), Some("letterboxd-cov-2026-10-19-13-22-utc.zip")).unwrap();
    let a = analyze(&ex, &Lookup::from_json(&table).unwrap());
    let by = |s: &str| a.coverage.per_stat.iter().find(|c| c.stat == s).unwrap().clone();
    assert_eq!((by("RNG").used, by("RNG").of, by("RNG").minimum, by("RNG").meets_minimum), (5, 5, 50, false));
    assert_eq!(by("DEV").text(), "only 5 of 5 films, needs 40");
    // Speed and Stamina count dated watches, not table films (the first date is the join day, so 4 of 5 count)
    assert_eq!(by("SPD").unit, "dated watches");
    assert!(a.card.coverage[1].contains("dated watches") && !a.card.coverage[1].contains("films"), "{}", a.card.coverage[1]);
    assert_eq!(a.card.coverage[3], "only 4 of 5 dated watches, needs 12");
    assert_eq!(a.card.grades[2], "?");
    assert_eq!(a.card.grades[5], "?");
    assert_eq!(a.card.coverage.len(), 6);
    assert_eq!(a.coverage.not_in_table, 0);
}

#[test]
fn films_outside_the_table_are_skipped_and_counted() {
    let w = "Date,Name,Year,Letterboxd URI\n2024-01-05,A,2000,https://boxd.it/a\n2024-02-05,B,2001,https://boxd.it/b\n";
    let r = "Date,Name,Year,Letterboxd URI,Rating\n2024-01-05,A,2000,https://boxd.it/a,4\n2024-02-05,B,2001,https://boxd.it/b,3\n";
    let (a, s) = card(&files(w, Some(r), None));
    let pw = a.coverage.per_stat.iter().find(|c| c.stat == "PWR").unwrap();
    assert_eq!((pw.used, pw.of, pw.meets_minimum), (0, 2, false));
    assert_eq!(a.card.coverage[0], "only 0 of 2 films, needs 20");
    assert!(s.contains("class=\"f-mute\""));
}

#[test]
fn unreadable_rating_cells_are_ignored_with_a_note() {
    let w = "Date,Name,Year,Letterboxd URI\n2024-01-05,A,2000,https://boxd.it/a\n";
    let r = "Date,Name,Year,Letterboxd URI,Rating\n2024-01-05,A,2000,https://boxd.it/a,NaN\n2024-01-05,B,2000,https://boxd.it/b,inf\n";
    let (a, _) = card(&files(w, Some(r), None));
    assert!(a.notes.iter().any(|n| n.contains("2 rating cell(s)") && n.contains("ignored")), "{:?}", a.notes);
}
