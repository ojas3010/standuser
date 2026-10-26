//! Browser entry points. Everything runs client-side: the export ZIP never leaves the page.

use standuser_core::{analysis, lookup::Lookup, parse, share, svg};
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
        let svgs = svgs_of(&a.card);
        Ok(serde_json::json!({ "analysis": a, "svgs": svgs }))
    };
    match go() {
        Ok(v) => v.to_string(),
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    }
}

/// Render a card from its data alone (`card_json` is `analysis.card`): no export, no table. Returns JSON { svgs: { light, dark, auto } } or
/// { error }. The page uses it for the sample card.
#[wasm_bindgen]
pub fn render_card(card_json: &str) -> String {
    render_card_inner(card_json)
}

pub fn render_card_inner(card_json: &str) -> String {
    match serde_json::from_str::<svg::CardData>(card_json) {
        Ok(card) => serde_json::json!({ "card": card, "svgs": svgs_of(&card) }).to_string(),
        Err(e) => serde_json::json!({ "error": format!("not a card: {e}") }).to_string(),
    }
}

/// The payload of the share link for a card (`card_json` is `analysis.card`): the page puts it after `#c=`. Empty when the data is not a card.
#[wasm_bindgen]
pub fn share_encode(card_json: &str) -> String {
    serde_json::from_str::<svg::CardData>(card_json).map(|c| share::encode(&c)).unwrap_or_default()
}

/// Open a share link: `hash` is `location.hash`. Returns JSON { card, svgs: { light, dark, auto } }, or { error } for anything that is not a
/// well-formed card link (the page then shows its normal state).
#[wasm_bindgen]
pub fn share_decode(hash: &str) -> String {
    share_decode_inner(hash)
}

pub fn share_decode_inner(hash: &str) -> String {
    match share::decode_fragment(hash) {
        Ok(card) => serde_json::json!({ "card": card, "svgs": svgs_of(&card) }).to_string(),
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    }
}

fn svgs_of(card: &svg::CardData) -> serde_json::Value {
    serde_json::json!({
        "light": svg::card_svg(card, svg::Theme::Light),
        "dark": svg::card_svg(card, svg::Theme::Dark),
        "auto": svg::card_svg(card, svg::Theme::Auto),
    })
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

    /// The card no longer draws the stat value lines, film titles, graded-on lines, weakness, signature film, why line, archetype caveat,
    /// PROVISIONAL DATA pill or data credit. The page still gets every one of them in `analysis.card`.
    #[test]
    fn the_page_still_gets_every_item_the_card_dropped() {
        let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../standuser-core/tests/fixtures/synth");
        let name = "letterboxd-synthuser-2026-06-30-12-00-utc.zip";
        let zip = std::fs::read(d.join(name)).unwrap();
        let table = std::fs::read_to_string(d.join("table.json")).unwrap();
        let names = std::fs::read_to_string(d.join("names.json")).unwrap();
        let out: serde_json::Value = serde_json::from_str(&run_inner(&zip, name, &table, &names, "", "", "")).unwrap();
        let card = &out["analysis"]["card"];
        let text = |k: &str| card[k].as_str().unwrap_or_else(|| panic!("card.{k} is not text: {card}")).to_string();
        let six = |k: &str| card[k].as_array().unwrap_or_else(|| panic!("card.{k} is not a list")).len();
        // stat value lines and film titles: one (metric, film) pair per stat; graded-on: one line per stat
        assert_eq!(six("evidence"), 6);
        assert!(card["evidence"].as_array().unwrap().iter().all(|e| !e[0].as_str().unwrap().is_empty()), "{}", card["evidence"]);
        assert!(card["evidence"].as_array().unwrap().iter().any(|e| !e[1].as_str().unwrap().is_empty()), "no example film at all: {}", card["evidence"]);
        assert_eq!(six("coverage"), 6);
        assert!(card["coverage"].as_array().unwrap().iter().all(|c| !c.as_str().unwrap().is_empty()));
        // weakness, signature film, why line, archetype caveat, data credit
        assert!(text("weakness").contains("Weakness") || text("weakness").contains("Soft spot") || text("weakness").contains("No weakness"), "{}", text("weakness"));
        assert!(text("signature").starts_with("Signature film:"));
        assert!(text("why").starts_with("Why 「"), "{}", text("why"));
        assert!(text("type_note").contains("archetype by stats"));
        assert_eq!(text("credit"), "Data: Letterboxd, TMDB, MusicBrainz");
        // the PROVISIONAL DATA pill: the flag that drew it
        assert!(card["badge"].is_boolean());
        // and the card itself (the SVGs) draws none of them
        let svg = out["svgs"]["light"].as_str().unwrap();
        assert!(!svg.contains("<text") && svg.len() > 1000);
    }

    #[test]
    fn a_card_renders_from_its_data_alone() {
        let json = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../site/sample.json")).unwrap();
        let out: serde_json::Value = serde_json::from_str(&render_card_inner(&json)).unwrap();
        for k in ["light", "dark", "auto"] {
            assert!(out["svgs"][k].as_str().unwrap().starts_with("<svg"), "{k}");
        }
        assert_eq!(out["card"]["user"], "synthetic3");
        // something that is not a card comes back as an error, not a panic
        assert!(serde_json::from_str::<serde_json::Value>(&render_card_inner("{\"name\":1}")).unwrap()["error"].is_string());
        assert!(serde_json::from_str::<serde_json::Value>(&render_card_inner("nonsense")).unwrap()["error"].is_string());
    }

    #[test]
    fn a_link_opens_to_the_same_card_and_a_bad_one_is_an_error() {
        let json = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../site/sample.json")).unwrap();
        let shown: serde_json::Value = serde_json::from_str(&render_card_inner(&json)).unwrap();
        let payload = share_encode(&json);
        assert!(!payload.is_empty() && !payload.contains(['=', '+', '/']));
        let opened: serde_json::Value = serde_json::from_str(&share_decode_inner(&format!("#c={payload}"))).unwrap();
        assert_eq!(opened["svgs"], shown["svgs"], "the link draws the very card that made it");
        for bad in ["", "#", "#c=", "#c=!!!", "#c=e30", "#x=abc", &format!("#c={}", &payload[..payload.len() / 2])] {
            let v: serde_json::Value = serde_json::from_str(&share_decode_inner(bad)).unwrap();
            assert!(v["error"].is_string() && v.get("svgs").is_none(), "{bad:?}");
        }
        assert_eq!(share_encode("nonsense"), "");
    }
}
