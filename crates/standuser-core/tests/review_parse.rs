//! Regression tests from the independent review (each reproduced a finding and failed before its fix).

use standuser_core::{analysis, dates, lookup::Lookup, parse};
use std::collections::BTreeMap;

fn base(w: &str, r: Option<&str>) -> BTreeMap<String, Vec<u8>> {
    let mut f = BTreeMap::new();
    f.insert("watched.csv".into(), w.as_bytes().to_vec());
    if let Some(r) = r { f.insert("ratings.csv".into(), r.as_bytes().to_vec()); }
    f
}

/// Browsers save a second download as "name (1).zip"; the username and export date should survive that.
#[test]
fn duplicate_download_suffix_breaks_username_and_date() {
    let (u, d) = parse::parse_file_name("letterboxd-jas3344-2026-10-19-13-22-utc (1).zip");
    assert_eq!(u.as_deref(), Some("jas3344"), "username");
    assert_eq!(d.map(dates::iso).as_deref(), Some("2026-10-19"), "export date");
}

/// A malformed date with a multi-byte character at byte 9 must not panic.
#[test]
fn multibyte_date_does_not_panic() {
    let r = std::panic::catch_unwind(|| dates::parse_date("2022-07-0é"));
    assert!(r.is_ok(), "parse_date panicked on a non-ASCII date");
}

#[test]
fn multibyte_date_in_watched_csv_does_not_panic() {
    let w = "Date,Name,Year,Letterboxd URI\n2022-07-0\u{e9},A,2000,https://boxd.it/a\n";
    let r = std::panic::catch_unwind(|| parse::parse_files(&base(w, None), None));
    assert!(r.is_ok(), "parse_files panicked on a non-ASCII date cell");
}

/// A non-numeric rating such as "NaN" is accepted by f64::from_str; it must not crash the analysis.
#[test]
fn nan_rating_does_not_panic() {
    let mut rows = String::new();
    let mut w = String::from("Date,Name,Year,Letterboxd URI\n");
    let mut r = String::from("Date,Name,Year,Letterboxd URI,Rating\n");
    for i in 0..60 {
        rows.push_str(&format!("[\"film{i}|2000\",\"f{i}\",{i},2000,100,\"en\",\"US\",\"{i}\",\"\",{:.2},null,100000],", 2.0 + (i as f64) * 0.04));
        w.push_str(&format!("2024-01-{:02},Film{i},2000,https://boxd.it/f{i}\n", 1 + i % 28));
        r.push_str(&format!("2024-01-{:02},Film{i},2000,https://boxd.it/f{i},{}\n", 1 + i % 28, if i == 0 { "NaN".to_string() } else { format!("{}", 1 + i % 5) }));
    }
    let table = format!("{{\"v\":1,\"films\":[{}]}}", rows.trim_end_matches(','));
    let ex = parse::parse_files(&base(&w, Some(&r)), Some("letterboxd-nan-2026-10-19-13-22-utc.zip")).unwrap();
    let lk = Lookup::from_json(&table).unwrap();
    let res = std::panic::catch_unwind(|| analysis::analyze(&ex, &lk));
    assert!(res.is_ok(), "analysis panicked on a NaN rating");
}
