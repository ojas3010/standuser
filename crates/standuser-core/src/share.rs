//! Share links: the data the card needs, packed into a URL fragment (`#c=<base64url>`), so a link renders the card with no file and
//! no request (a fragment is never sent to a server). The payload is JSON with short keys, then base64url without padding.
//!
//! It carries what the card draws and nothing else: the Stand name, the username, the type, the six grades with their asterisks and
//! chart steps, the ability name and sentence, the nearest canon names and the accent key. It does not carry film titles, the stat
//! values, the why line or anything read from the export. Decoding checks every field (lengths, grade letters, chart steps, accent
//! key, control characters), so a hand-made or damaged fragment is an error and never reaches the renderer.

use crate::palette::KEYS;
use crate::svg::CardData;
use serde::{Deserialize, Serialize};

/// The fragment key: `#c=...`.
pub const KEY: &str = "c=";
/// The payload may not be longer than this, in bytes of base64url and of JSON.
pub const MAX_PAYLOAD: usize = 3000;
const MAX_JSON: usize = 2200;
const GRADES: [&str; 9] = ["A", "B", "C", "D", "E", "∞", "∅", "?", "完成"];

#[derive(Serialize, Deserialize)]
struct Packed {
    v: u8,
    /// Stand name
    n: String,
    /// username
    u: String,
    /// type in Japanese, in English
    t: (String, String),
    /// the six grade letters
    g: Vec<String>,
    /// the six chart steps
    p: Vec<f64>,
    /// provisional flags, "0" or "1" per grade
    s: String,
    /// ability name, ability sentence
    a: (String, String),
    /// nearest canon names
    c: Vec<String>,
    /// accent key, or "" for none
    k: String,
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn b64_encode(b: &[u8]) -> String {
    let mut out = String::with_capacity(b.len() * 4 / 3 + 3);
    for ch in b.chunks(3) {
        let n = (ch[0] as u32) << 16 | (*ch.get(1).unwrap_or(&0) as u32) << 8 | *ch.get(2).unwrap_or(&0) as u32;
        for i in 0..=ch.len() {
            out.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

fn b64_decode(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 4 == 1 {
        return Err("not base64url".into());
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes() {
        let v = B64.iter().position(|x| *x == c).ok_or("not base64url")? as u32;
        acc = acc << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits & 0xff) as u8);
        }
    }
    // the unused low bits of the last character must be zero, so that one card has one link
    if acc & ((1 << bits) - 1) != 0 {
        return Err("not base64url".into());
    }
    Ok(out)
}

/// The payload of the link for this card (without the `c=`).
pub fn encode(card: &CardData) -> String {
    let p = Packed {
        v: 1,
        n: card.name.clone(),
        u: card.user.clone(),
        t: (card.type_jp.clone(), card.type_en.clone()),
        g: card.grades.clone(),
        p: card.plot.clone(),
        s: card.prov.iter().map(|b| if *b { '1' } else { '0' }).collect(),
        a: (card.ability_name.clone(), card.ability_line.clone()),
        c: card.nearest.clone(),
        k: card.accent.clone().unwrap_or_default(),
    };
    b64_encode(serde_json::to_string(&p).expect("plain data").as_bytes())
}

fn text(what: &str, s: &str, min: usize, max: usize) -> Result<(), String> {
    let n = s.chars().count();
    if n < min || n > max {
        return Err(format!("{what} has {n} characters"));
    }
    if s.chars().any(|c| c.is_control()) {
        return Err(format!("{what} has a control character"));
    }
    Ok(())
}

/// The card a payload stands for, or why it is not one. The fields the card does not draw come back empty.
pub fn decode(payload: &str) -> Result<CardData, String> {
    if payload.len() > MAX_PAYLOAD {
        return Err("too long".into());
    }
    let bytes = b64_decode(payload)?;
    if bytes.len() > MAX_JSON {
        return Err("too long".into());
    }
    let p: Packed = serde_json::from_slice(&bytes).map_err(|e| format!("not a card: {e}"))?;
    if p.v != 1 {
        return Err(format!("version {}", p.v));
    }
    text("name", &p.n, 1, 80)?;
    text("username", &p.u, 1, 60)?;
    text("type", &p.t.0, 1, 40)?;
    text("type", &p.t.1, 1, 60)?;
    text("ability name", &p.a.0, 1, 80)?;
    text("ability sentence", &p.a.1, 1, 300)?;
    if p.g.len() != 6 || p.p.len() != 6 || p.s.len() != 6 {
        return Err("six grades are needed".into());
    }
    if !p.g.iter().all(|g| GRADES.contains(&g.as_str())) {
        return Err("unknown grade".into());
    }
    if !p.p.iter().all(|v| v.is_finite() && (0.0..=6.0).contains(v)) {
        return Err("chart step out of range".into());
    }
    if !p.s.chars().all(|c| c == '0' || c == '1') {
        return Err("bad provisional flags".into());
    }
    if p.c.len() > 3 {
        return Err("too many canon names".into());
    }
    for n in &p.c {
        text("canon name", n, 1, 60)?;
    }
    if !p.k.is_empty() && !KEYS.contains(&p.k.as_str()) {
        return Err("unknown accent".into());
    }
    let prov: Vec<bool> = p.s.chars().map(|c| c == '1').collect();
    Ok(CardData {
        name: p.n,
        why: String::new(),
        user: p.u,
        type_jp: p.t.0,
        type_en: p.t.1,
        type_note: String::new(),
        unknown: p.g.iter().map(|g| g == "?").collect(),
        badge: prov.iter().any(|b| *b),
        grades: p.g,
        plot: p.p,
        prov,
        evidence: vec![],
        coverage: vec![],
        ability: String::new(),
        ability_name: p.a.0,
        ability_line: p.a.1,
        weakness: String::new(),
        nearest: p.c,
        signature: String::new(),
        credit: String::new(),
        accent: if p.k.is_empty() { None } else { Some(p.k) },
    })
}

/// Like `decode`, for what `location.hash` holds: `#c=<payload>` (the `#` is optional).
pub fn decode_fragment(hash: &str) -> Result<CardData, String> {
    let h = hash.strip_prefix('#').unwrap_or(hash);
    decode(h.strip_prefix(KEY).ok_or("not a card link")?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svg::{card_svg, Theme};

    fn sample() -> CardData {
        serde_json::from_str(include_str!("../../../site/sample.json")).unwrap()
    }

    #[test]
    fn base64url_round_trips_every_length_and_byte() {
        for len in 0..40 {
            let b: Vec<u8> = (0..len).map(|i| (i * 37 + 11) as u8).collect();
            let e = b64_encode(&b);
            assert!(!e.contains(['+', '/', '=']), "{e}");
            assert_eq!(b64_decode(&e).unwrap(), b, "{len}");
        }
        let all: Vec<u8> = (0..=255).collect();
        assert_eq!(b64_decode(&b64_encode(&all)).unwrap(), all);
        // the standard alphabet, padding and a stray length are not base64url
        for bad in ["ab+/", "ab==", "a", "abcde", "ab cd", "ab\n", "é"] {
            assert!(b64_decode(bad).is_err(), "{bad:?}");
        }
        // non-canonical trailing bits are refused: one card, one link
        assert!(b64_decode("QR").is_err() && b64_decode("QQ").is_ok());
    }

    #[test]
    fn a_card_round_trips_to_the_same_card() {
        let c = sample();
        let payload = encode(&c);
        assert!(payload.len() < 700, "{} characters", payload.len());
        let d = decode(&payload).unwrap();
        // everything the card draws comes back, so the three SVGs are byte for byte the same
        for t in [Theme::Light, Theme::Dark, Theme::Auto] {
            assert_eq!(card_svg(&c, t), card_svg(&d, t));
        }
        assert_eq!((&d.name, &d.user, &d.grades, &d.prov, &d.accent), (&c.name, &c.user, &c.grades, &c.prov, &c.accent));
        // the fragment form
        let e = decode_fragment(&format!("#c={payload}")).unwrap();
        assert_eq!(e.name, c.name);
        assert_eq!(decode_fragment(&format!("c={payload}")).unwrap().user, c.user);
        // nothing read from the export travels: no film, no value, no why line
        assert!(d.evidence.is_empty() && d.coverage.is_empty() && d.why.is_empty() && d.signature.is_empty() && d.weakness.is_empty());
        assert!(!payload.contains("Film") && !String::from_utf8(b64_decode(&payload).unwrap()).unwrap().contains("evidence"));
    }

    #[test]
    fn cards_with_unusual_content_round_trip() {
        let mut c = sample();
        c.name = "A".into();
        c.user = "ユーザー名 ünï".into();
        c.grades = ["∞", "∅", "?", "完成", "E", "A"].map(String::from).into();
        c.plot = vec![6.0, 0.0, 3.0, 1.0, 1.0, 5.0];
        c.prov = vec![true, false, false, true, true, false];
        c.unknown = vec![false, false, true, false, false, false];
        c.accent = None;
        c.nearest = vec!["Kiss".into()];
        let d = decode(&encode(&c)).unwrap();
        assert_eq!(card_svg(&c, Theme::Light), card_svg(&d, Theme::Light));
        assert_eq!(d.accent, None);
    }

    fn pack(f: impl Fn(&mut serde_json::Value)) -> String {
        let mut v: serde_json::Value = serde_json::from_slice(&b64_decode(&encode(&sample())).unwrap()).unwrap();
        f(&mut v);
        b64_encode(v.to_string().as_bytes())
    }

    #[test]
    fn a_malformed_fragment_is_an_error_not_a_card() {
        let good = encode(&sample());
        assert!(decode(&good).is_ok());
        // not a payload at all
        for bad in ["", "!!!", "abc", "####", "a b", "%E3%81%82", "e30", "bnVsbA", "W10"] {
            assert!(decode(bad).is_err(), "{bad:?}");
        }
        // cut short, and with a byte changed
        assert!(decode(&good[..good.len() / 2]).is_err());
        assert!(decode(&format!("{good}AAAA")).is_err(), "bytes after the JSON");
        assert!(decode(&good.replacen('e', "!", 1)).is_err());
        // too long
        assert!(decode(&"A".repeat(MAX_PAYLOAD + 1)).is_err());
        assert!(decode(&b64_encode("x".repeat(MAX_JSON + 1).as_bytes())).is_err());
        // wrong shape: version, grades, steps, flags, accent, lengths, control characters
        let cases: Vec<(&str, Box<dyn Fn(&mut serde_json::Value)>)> = vec![
            ("version", Box::new(|v| v["v"] = 2.into())),
            ("five grades", Box::new(|v| { v["g"].as_array_mut().unwrap().pop(); })),
            ("bad grade", Box::new(|v| v["g"][0] = "Z".into())),
            ("grade object", Box::new(|v| v["g"][0] = serde_json::json!({"a": 1}))),
            ("step 9", Box::new(|v| v["p"][1] = 9.into())),
            ("step -1", Box::new(|v| v["p"][1] = (-1).into())),
            ("flags", Box::new(|v| v["s"] = "01x010".into())),
            ("short flags", Box::new(|v| v["s"] = "01".into())),
            ("accent", Box::new(|v| v["k"] = "XYZ".into())),
            ("empty name", Box::new(|v| v["n"] = "".into())),
            ("long name", Box::new(|v| v["n"] = "N".repeat(81).into())),
            ("long sentence", Box::new(|v| v["a"][1] = "w ".repeat(200).into())),
            ("control", Box::new(|v| v["u"] = "a\u{0}b".into())),
            ("newline", Box::new(|v| v["n"] = "A\nB".into())),
            ("four canon", Box::new(|v| v["c"] = serde_json::json!(["a", "b", "c", "d"]))),
            ("canon number", Box::new(|v| v["c"] = serde_json::json!([1]))),
            ("missing key", Box::new(|v| { v.as_object_mut().unwrap().remove("t"); })),
        ];
        for (what, f) in cases {
            assert!(decode(&pack(f)).is_err(), "{what} was accepted");
        }
        // the fragment form: no key, another key, empty
        assert!(decode_fragment("").is_err() && decode_fragment("#").is_err() && decode_fragment("#x=abc").is_err() && decode_fragment("#c=").is_err());
        assert!(decode_fragment(&format!("#d={good}")).is_err());
    }
}
