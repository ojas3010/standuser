//! Regression tests from the independent review: title|year collisions (Bug 2) and what the shipped table reveals.

use standuser_core::lookup::Lookup;

const TABLE: &str = include_str!("../data/films.json");

/// Two different films share "title|year". The table must not hand every user one of them as if it were the other:
/// an ambiguous key is dropped, so a user's "Leo (2023)" is skipped rather than given another film's data.
#[test]
fn title_year_collisions_do_not_merge_distinct_films() {
    let lk = Lookup::from_json(TABLE).unwrap();
    for (name, year) in [("Leo", 2023), ("Reality", 2014), ("Cobweb", 2023)] {
        assert!(lk.find(None, name, year).is_none(), "{name} ({year}) is ambiguous between two films and must not resolve");
    }
}

/// Only films the table can tell apart are shipped: no key appears twice.
#[test]
fn every_key_is_unique() {
    let t: serde_json::Value = serde_json::from_str(TABLE).unwrap();
    let mut seen = std::collections::HashSet::new();
    for r in t["films"].as_array().unwrap() {
        assert!(seen.insert(r[0].as_str().unwrap().to_string()), "duplicate key {}", r[0]);
    }
}

/// The public table must not reveal which films belong to the author: no Letterboxd short codes at all (only the
/// author's own films ever had one), and the builder ships the shared film list, not anyone's watch list.
#[test]
fn shipped_table_carries_no_letterboxd_codes() {
    let t: serde_json::Value = serde_json::from_str(TABLE).unwrap();
    let coded = t["films"].as_array().unwrap().iter().filter(|r| !r[1].as_str().unwrap().is_empty()).count();
    assert_eq!(coded, 0, "{coded} rows carry a Letterboxd code");
    assert!(t["built"].as_str().is_some(), "the table is stamped with its build date");
}
