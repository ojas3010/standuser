//! Browser entry points. Everything runs client-side: the export ZIP never leaves the page.

use standuser_core::{analysis, lookup::Lookup, parse, svg};
use wasm_bindgen::prelude::*;

/// Analyse an export entirely in the page: no network. `table_json` is the film lookup table; films missing from it are
/// skipped by every stat that needs film data. `names_json` is names.json (the track-title name candidates; "" leaves the composer route). `today` ("YYYY-MM-DD") is only used to warn about an out-of-date table. `joined` ("YYYY-MM-DD") and `username` override the profile.csv values when non-empty.
/// Returns JSON: { analysis, svgs: { light, dark, auto } } or { error }.
#[wasm_bindgen]
pub fn run(zip: &[u8], file_name: &str, table_json: &str, names_json: &str, joined: &str, username: &str, today: &str) -> String {
    run_inner(zip, file_name, table_json, names_json, joined, username, today)
}

pub fn run_inner(zip: &[u8], file_name: &str, table_json: &str, names_json: &str, joined: &str, username: &str, today: &str) -> String {
    let go = || -> Result<serde_json::Value, String> {
        let mut ex = parse::parse_zip(zip, Some(file_name)).map_err(|e| e.to_string())?;
        if !joined.is_empty() {
            ex.joined = standuser_core::dates::parse_date(joined);
        }
        if !username.is_empty() {
            ex.username = Some(username.to_string());
        }
        let mut lk = Lookup::from_json(table_json)?;
        if !names_json.is_empty() {
            lk.load_names(names_json)?;
        }
        let a = analysis::analyze_at(&ex, &lk, standuser_core::dates::parse_date(today));
        let svgs = serde_json::json!({
            "light": svg::card_svg(&a.card, svg::Theme::Light),
            "dark": svg::card_svg(&a.card, svg::Theme::Dark),
            "auto": svg::card_svg(&a.card, svg::Theme::Auto),
        });
        Ok(serde_json::json!({ "analysis": a, "svgs": svgs }))
    };
    match go() {
        Ok(v) => v.to_string(),
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_on_the_synthetic_export() {
        let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../standuser-core/tests/fixtures/synth");
        let zip = std::fs::read(d.join("letterboxd-synthuser-2026-06-30-12-00-utc.zip")).unwrap();
        let table = std::fs::read_to_string(d.join("table.json")).unwrap();
        let out: serde_json::Value = serde_json::from_str(&run_inner(&zip, "letterboxd-synthuser-2026-06-30-12-00-utc.zip", &table, "", "", "", "")).unwrap();
        assert_eq!(out["analysis"]["username"], "synthuser");
        // the dark card is a different ground from the light one (the ground follows the user's signature)
        let (dark, light) = (out["svgs"]["dark"].as_str().unwrap(), out["svgs"]["light"].as_str().unwrap());
        let bg = |s: &str| s[s.find(".f-bg{fill:").unwrap()..][..18].to_string();
        assert_ne!(bg(dark), bg(light));
        assert!(out["svgs"]["auto"].as_str().unwrap().contains("prefers-color-scheme"));
        // errors come back as JSON, not a panic
        let bad: serde_json::Value = serde_json::from_str(&run_inner(b"not a zip", "x.zip", &table, "", "", "", "")).unwrap();
        assert!(bad["error"].as_str().unwrap().contains("ZIP"));
        // join day and username overrides
        let m: serde_json::Value = serde_json::from_str(&run_inner(&zip, "x.zip", &table, "", "2023-01-05", "bob", "")).unwrap();
        assert_eq!(m["analysis"]["username"], "bob");
        assert!(m["analysis"]["coverage"]["not_in_table"].as_u64().unwrap() >= 3);
        assert_eq!(m["analysis"]["coverage"]["per_stat"].as_array().unwrap().len(), 6);
    }
}
