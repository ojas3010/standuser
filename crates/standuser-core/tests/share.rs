//! Share links for real analyses: every one of the ten synthetic users' cards survives the trip through a link with its three SVGs
//! unchanged, and the link carries nothing that was read from the export beyond what the card draws.

mod common;
use common::synth_users;
use standuser_core::{analysis, dates::parse_date, lookup::Lookup, share, svg};

#[test]
fn the_ten_synthetic_cards_round_trip_through_a_link() {
    let mut lk = Lookup::from_json(include_str!("../data/films.json")).unwrap();
    lk.load_names(include_str!("../data/names.json")).unwrap();
    for (i, (mut ex, _)) in synth_users::users(10, &lk).into_iter().enumerate() {
        ex.joined = parse_date("2022-01-01");
        ex.as_of = parse_date("2026-10-19");
        let a = analysis::analyze(&ex, &lk);
        let payload = share::encode(&a.card);
        assert!(payload.len() < 900, "user {i}: {} characters", payload.len());
        let d = share::decode(&payload).unwrap_or_else(|e| panic!("user {i}: {e}"));
        for t in [svg::Theme::Light, svg::Theme::Dark, svg::Theme::Auto] {
            assert_eq!(svg::card_svg(&a.card, t), svg::card_svg(&d, t), "user {i}");
        }
        let json = String::from_utf8(base64url(&payload)).unwrap();
        for private in a.card.evidence.iter().flat_map(|(m, f)| [m.as_str(), f.as_str()]).filter(|s| s.len() > 6) {
            assert!(!json.contains(private), "user {i}: the link carries {private:?}");
        }
        assert!(!json.contains(&a.card.weakness) && !json.contains(&a.card.signature));
    }
}

/// The payload's JSON, by decoding it through the public API's own alphabet (url-safe base64, no padding).
fn base64url(p: &str) -> Vec<u8> {
    let v = |c: u8| b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_".iter().position(|x| *x == c).unwrap() as u32;
    let (mut out, mut acc, mut bits) = (vec![], 0u32, 0);
    for c in p.bytes() {
        acc = acc << 6 | v(c);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits & 0xff) as u8);
        }
    }
    out
}
