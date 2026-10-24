//! Golden tests: the Rust port must reproduce the Python reference (tools/golden/ref.py, which runs the research
//! prototypes) on the same export. Two fixtures:
//!   synth  - committed, synthetic export + table (always runs)
//!   real   - the author's real export, kept outside the repository (local/, gitignored). It is an #[ignore]d test, so a
//!            normal run reports it as ignored, never as passed. Run it with
//!            STANDUSER_OJAS_EXPORT=<zip> STANDUSER_OJAS_TABLE=<films.json> STANDUSER_OJAS_GOLDEN=<golden.json> cargo test --test golden -- --ignored

use serde_json::Value;
use standuser_core::{analysis, dates, lookup::Lookup, parse, svg};
use std::path::{Path, PathBuf};

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn close(a: &Value, b: &Value, tol: f64, what: &str) {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => assert!((x - y).abs() <= tol * y.abs().max(1.0), "{what}: rust {x} vs python {y}"),
        _ => assert_eq!(a, b, "{what}"),
    }
}

fn check(a: &analysis::Analysis, g: &Value) {
    let r = serde_json::to_value(a).unwrap();
    // ---- timeline
    let t = &g["timeline"];
    assert_eq!(r["timeline"]["events"].as_array().unwrap().len(), t["events"].as_u64().unwrap() as usize, "events");
    for k in ["dropped_bulk_mark", "dropped_no_clean_view", "rows_on_bulk_mark_days"] {
        assert_eq!(r["timeline"][k], t[k], "timeline.{k}");
    }
    let bulk: Vec<String> = r["timeline"]["bulk_mark_days"].as_array().unwrap().iter().map(|d| dates::iso(d.as_i64().unwrap())).collect();
    assert_eq!(serde_json::to_value(bulk).unwrap(), t["bulk_mark_days"], "bulk days");
    match (&t["metrics"], &r["timeline"]["metrics"]) {
        (Value::Null, m) => assert!(m.is_null()),
        (gm, rm) => {
            for k in ["months", "events_in_window", "active", "streak", "peak_n", "peak_month", "silent", "silent_run", "start"] {
                assert_eq!(rm[k], gm[k], "metrics.{k}");
            }
            for k in ["share", "pace"] {
                close(&rm[k], &gm[k], 1e-12, k);
            }
        }
    }
    // ---- facts
    for (k, v) in g["facts"].as_object().unwrap() {
        if k == "events" || k == "n_watched" {
            continue;
        }
        let rv = &r["stats"]["facts"][k];
        if k == "pairs" {
            assert_eq!(rv, v, "facts.{k}");
        } else {
            close(rv, v, 1e-9, &format!("facts.{k}"));
        }
    }
    // ---- grades
    let keys = [("PWR", "power"), ("SPD", "speed"), ("RNG", "range"), ("STA", "stamina"), ("PRC", "precision"), ("DEV", "potential")];
    for (code, name) in keys {
        let (gg, rg) = (&g["grades"][code], &r["stats"][name]);
        assert_eq!(rg["letter"], gg["letter"], "{code} letter");
        assert_eq!(rg["status"], gg["status"], "{code} status");
        close(&rg["canon"], &gg["canon"], 1e-12, &format!("{code} canon"));
        close(&rg["plot"], &gg["plot"], 1e-12, &format!("{code} plot"));
        close(&rg["value"], &gg["value"], 1e-9, &format!("{code} value"));
        close(&rg["p"], &gg["p"], 2e-6, &format!("{code} percentile"));
    }
    // ---- type, nearest, ability
    assert_eq!(r["kind"], g["type"], "type");
    let (rn, gn) = (r["nearest"].as_array().unwrap(), g["nearest"].as_array().unwrap());
    assert_eq!(rn.len(), gn.len(), "nearest count");
    for (x, y) in rn.iter().zip(gn) {
        assert_eq!(x["name"], y["name"], "nearest name");
        close(&x["dist"], &y["dist"], 1e-9, "nearest dist");
    }
    for k in ["ability", "weakness", "why", "film"] {
        assert_eq!(r["ability"][k], g["ability"][k], "ability.{k}");
    }
    assert_eq!(r["ability"]["template"], g["ability"]["template"]);
}

fn run(zip: &Path, table: &Path, joined: &str, golden: &Path) {
    let bytes = std::fs::read(zip).unwrap();
    let mut ex = parse::parse_zip(&bytes, zip.file_name().and_then(|n| n.to_str())).unwrap();
    if !joined.is_empty() {
        ex.joined = dates::parse_date(joined);
    }
    let lk = Lookup::from_json(&std::fs::read_to_string(table).unwrap()).unwrap();
    let a = analysis::analyze(&ex, &lk);
    let g: Value = serde_json::from_str(&std::fs::read_to_string(golden).unwrap()).unwrap();
    assert_eq!(g["inputs"]["username"], a.username.as_str());
    if joined.is_empty() {
        assert_eq!(a.joined, g["inputs"]["joined"].as_str().unwrap(), "join day read from the fixture's profile.csv");
    }
    check(&a, &g);
    if joined.is_empty() {
        // the join day came from the zip's own profile.csv, and no other profile field reached the output
        assert!(a.notes.iter().all(|n| !n.contains("Date Joined")), "{:?}", a.notes);
        let all = serde_json::to_string(&a).unwrap() + &svg::card_svg(&a.card, svg::Theme::Auto);
        for field in ["synthetic@example.invalid", "Thetic", "\"Syn\""] {
            assert!(!all.contains(field), "profile field {field} leaked");
        }
    }
    // the card for this analysis renders and keeps the colour classes
    let s = svg::card_svg(&a.card, svg::Theme::Auto);
    assert!(s.contains("class=\"f-ink\"") && !s.contains("class=\"ink\""));
}

#[test]
fn synthetic_export_matches_python_reference() {
    let d = dir().join("synth");
    // join day comes from the fixture's own profile.csv (allowlisted columns only)
    run(&d.join("letterboxd-synthuser-2026-06-30-12-00-utc.zip"), &d.join("table.json"), "", &d.join("golden.json"));
}

#[test]
#[ignore = "needs the author's real export: set STANDUSER_OJAS_EXPORT, STANDUSER_OJAS_TABLE and STANDUSER_OJAS_GOLDEN, then run with --ignored"]
fn author_export_matches_python_reference() {
    let var = |n: &str| std::env::var(n).unwrap_or_else(|_| panic!("{n} is not set; this test only means something with the real export"));
    // the author's zip carries no profile.csv in this setup, so the join day is passed in (2022-07-06 per the spec)
    run(Path::new(&var("STANDUSER_OJAS_EXPORT")), Path::new(&var("STANDUSER_OJAS_TABLE")), "2022-07-06", Path::new(&var("STANDUSER_OJAS_GOLDEN")));
}
