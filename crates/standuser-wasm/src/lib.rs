//! Browser entry points. Everything runs client-side: the export ZIP never leaves the page.

use standuser_core::{analysis, lookup::Lookup, parse, svg};
use wasm_bindgen::prelude::*;

/// Analyse an export. `table_json` is the film lookup table; `extra_json` (may be empty) is what the TMDB proxy returned
/// for films missing from it. `joined` ("YYYY-MM-DD") and `username` override the profile.csv values when non-empty.
/// Returns JSON: { analysis, svgs: { light, dark, auto } } or { error }.
#[wasm_bindgen]
pub fn run(zip: &[u8], file_name: &str, table_json: &str, extra_json: &str, joined: &str, username: &str) -> String {
    run_inner(zip, file_name, table_json, extra_json, joined, username)
}

pub fn run_inner(zip: &[u8], file_name: &str, table_json: &str, extra_json: &str, joined: &str, username: &str) -> String {
    let go = || -> Result<serde_json::Value, String> {
        let mut ex = parse::parse_zip(zip, Some(file_name)).map_err(|e| e.to_string())?;
        if !joined.is_empty() {
            ex.joined = standuser_core::dates::parse_date(joined);
        }
        if !username.is_empty() {
            ex.username = Some(username.to_string());
        }
        let mut lk = Lookup::from_json(table_json)?;
        if !extra_json.trim().is_empty() {
            lk.merge_json(extra_json)?;
        }
        let a = analysis::analyze(&ex, &lk);
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
        let out: serde_json::Value = serde_json::from_str(&run_inner(&zip, "letterboxd-synthuser-2026-06-30-12-00-utc.zip", &table, "", "", "")).unwrap();
        assert_eq!(out["analysis"]["username"], "synthuser");
        assert!(out["svgs"]["dark"].as_str().unwrap().contains("#101014"));
        assert!(out["svgs"]["auto"].as_str().unwrap().contains("prefers-color-scheme"));
        // errors come back as JSON, not a panic
        let bad: serde_json::Value = serde_json::from_str(&run_inner(b"not a zip", "x.zip", &table, "", "", "")).unwrap();
        assert!(bad["error"].as_str().unwrap().contains("ZIP"));
        // proxy rows merge
        let extra = r#"{"v":1,"films":[["unlistedshortstory|2011","",1,2011,100,"en","FR","","",null,null,null]]}"#;
        let m: serde_json::Value = serde_json::from_str(&run_inner(&zip, "x.zip", &table, extra, "2023-01-05", "bob")).unwrap();
        assert_eq!(m["analysis"]["username"], "bob");
        assert!(m["analysis"]["coverage"]["unknown_films"].as_array().unwrap().len() < 3);
    }
}
