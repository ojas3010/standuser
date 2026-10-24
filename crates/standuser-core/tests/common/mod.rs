//! Helpers shared by the review regression tests: exports built from real table films.
#![allow(dead_code)]

use standuser_core::{analysis, lookup::Lookup, parse};
use std::collections::BTreeMap;

pub const TABLE: &str = include_str!("../../data/films.json");

/// Up to `n` real table films that have a composer, countries, directors, a crowd average and >= 30,000 ratings.
pub fn films(n: usize) -> Vec<(String, i32, f64)> {
    let t: serde_json::Value = serde_json::from_str(TABLE).unwrap();
    let mut out = vec![];
    for r in t["films"].as_array().unwrap() {
        let r = r.as_array().unwrap();
        let ok = !r[8].as_str().unwrap().is_empty() && !r[6].as_str().unwrap().is_empty() && !r[7].as_str().unwrap().is_empty()
            && r[9].is_number() && r[11].as_u64().unwrap_or(0) >= 30000 && r[4].as_u64().unwrap() > 60;
        if ok { out.push((r[0].as_str().unwrap().to_string(), r[3].as_i64().unwrap() as i32, r[9].as_f64().unwrap())); }
        if out.len() == n { break; }
    }
    out
}

/// Export rows for table films: the table key "title|year" is itself a valid Name (it normalises to the same key).
pub fn export(n: usize, rating: Option<f64>, diary: bool) -> BTreeMap<String, Vec<u8>> {
    let fs = films(n);
    let (mut w, mut r, mut d) = (String::from("Date,Name,Year,Letterboxd URI\n"), String::from("Date,Name,Year,Letterboxd URI,Rating\n"),
        String::from("Date,Name,Year,Letterboxd URI,Rating,Rewatch,Tags,Watched Date\n"));
    for (i, (k, y, _)) in fs.iter().enumerate() {
        let name = k.split('|').next().unwrap();
        // two films a week from 2022-01-03 on, never 8 on one day
        let day = 18995 + (i as i64) * 4; // 2022-01-03 + 4 days each
        let date = standuser_core::dates::iso(day);
        w.push_str(&format!("{date},{name},{y},\n"));
        if let Some(x) = rating { r.push_str(&format!("{date},{name},{y},,{x}\n")); }
        if diary { d.push_str(&format!("{date},{name},{y},,,,,{date}\n")); }
    }
    let mut f = BTreeMap::new();
    f.insert("watched.csv".into(), w.into_bytes());
    if rating.is_some() { f.insert("ratings.csv".into(), r.into_bytes()); }
    if diary { f.insert("diary.csv".into(), d.into_bytes()); }
    f
}

pub fn run(f: &BTreeMap<String, Vec<u8>>) -> analysis::Analysis {
    let mut ex = parse::parse_files(f, Some("letterboxd-spec-2026-10-19-13-22-utc.zip")).unwrap();
    ex.joined = standuser_core::dates::parse_date("2021-12-01");
    analysis::analyze(&ex, &Lookup::from_json(TABLE).unwrap())
}

