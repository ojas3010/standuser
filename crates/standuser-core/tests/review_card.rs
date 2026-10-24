//! Regression tests from the independent review (each reproduced a finding and failed before its fix).

use standuser_core::{analysis, dates, lookup::Lookup, parse, svg};
use std::collections::BTreeMap;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/synth");

fn files(w: &str, r: Option<&str>, d: Option<&str>) -> BTreeMap<String, Vec<u8>> {
    let mut f = BTreeMap::new();
    f.insert("watched.csv".into(), w.as_bytes().to_vec());
    if let Some(r) = r { f.insert("ratings.csv".into(), r.as_bytes().to_vec()); }
    if let Some(d) = d { f.insert("diary.csv".into(), d.as_bytes().to_vec()); }
    f
}

/// A lapsed user: one film a month 2023-01..2024-12, export taken 2026-06. Saved as "(1).zip", the export date falls
/// back to the last watch, the 18 trailing silent months vanish from the window and Stamina jumps.
#[test]
fn duplicate_suffix_changes_stamina_for_a_lapsed_user() {
    let mut w = String::from("Date,Name,Year,Letterboxd URI\n");
    for m in 0..24 {
        let (y, mo) = (2023 + m / 12, 1 + m % 12);
        w.push_str(&format!("{y}-{mo:02}-15,F{m},2000,u\n"));
        w.push_str(&format!("{y}-{mo:02}-16,G{m},2000,u\n"));
    }
    let f = files(&w, None, None);
    let lk = Lookup::default();
    let a = analysis::analyze(&parse::parse_files(&f, Some("letterboxd-lapsed-2026-06-30-12-00-utc.zip")).unwrap(), &lk);
    let b = analysis::analyze(&parse::parse_files(&f, Some("letterboxd-lapsed-2026-06-30-12-00-utc (1).zip")).unwrap(), &lk);
    println!("normal: user={} as_of={} STA {} value {:?}", a.username, a.as_of, a.stats.stamina.letter, a.stats.stamina.value);
    println!("(1):    user={} as_of={} STA {} value {:?}", b.username, b.as_of, b.stats.stamina.letter, b.stats.stamina.value);
    assert_eq!((a.username.as_str(), a.stats.stamina.letter.as_str()), (b.username.as_str(), b.stats.stamina.letter.as_str()));
}

/// Speed/Stamina coverage says the minimum is met (it counts all clean events) while the grade is "?" because fewer
/// than 12 events fall inside the complete-month window.
#[test]
fn speed_coverage_claims_minimum_met_while_grade_is_unknown() {
    let mut w = String::from("Date,Name,Year,Letterboxd URI\n");
    // 10 films on separate days in Jan 2024 (T0's month, outside the window), then 8 films in Mar..Oct 2024
    for i in 0..10 { w.push_str(&format!("2024-01-{:02},J{i},2000,u\n", 2 + 2 * i)); }
    for m in 3..=10 { w.push_str(&format!("2024-{m:02}-10,M{m},2000,u\n")); }
    let ex = parse::parse_files(&files(&w, None, None), Some("letterboxd-cov-2026-10-19-13-22-utc.zip")).unwrap();
    let mut ex = ex;
    ex.joined = dates::parse_date("2023-12-01");
    let a = analysis::analyze(&ex, &Lookup::default());
    let c = &a.coverage.per_stat[1];
    println!("Speed letter {} note {:?}; coverage used={} meets_minimum={} card text {:?}", a.stats.speed.letter, a.stats.speed.note, c.used, c.meets_minimum, a.card.coverage[1]);
    assert!(!(a.stats.speed.letter == "?" && c.meets_minimum), "coverage says the minimum is met but Speed is '?'");
}

/// The SVG <desc> promises asterisks on provisional grades but writes none.
#[test]
fn svg_desc_does_not_mark_provisional_grades() {
    let d = std::path::Path::new(FIXTURES);
    let zip = std::fs::read(d.join("letterboxd-synthuser-2026-06-30-12-00-utc.zip")).unwrap();
    let ex = parse::parse_zip(&zip, Some("letterboxd-synthuser-2026-06-30-12-00-utc.zip")).unwrap();
    let lk = Lookup::from_json(&std::fs::read_to_string(d.join("table.json")).unwrap()).unwrap();
    let a = analysis::analyze(&ex, &lk);
    let s = svg::card_svg(&a.card, svg::Theme::Auto);
    let desc = &s[s.find("<desc").unwrap()..s.find("</desc>").unwrap()];
    println!("prov flags {:?}\n{desc}", a.card.prov);
    assert!(a.card.prov.iter().any(|p| *p));
    assert!(desc.contains('*'), "desc says provisional grades are marked with an asterisk, but none is");
}
