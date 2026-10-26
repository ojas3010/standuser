//! Writes the sample card for every accent (and none) in both card themes to a directory, for tools/site_test/card_contrast.py:
//! `cargo run -p standuser-core --example card_palettes -- <out_dir>` gives <KEY>-light.svg and <KEY>-dark.svg.
use standuser_core::palette::ACCENTS;
use standuser_core::svg::{card_svg, CardData, Theme, TYPE_NOTE, TYPE_NOTE_LOW};

fn sample() -> CardData {
    CardData {
        name: "YORKTOWN AND THE RETURN HOME".into(),
        why: "Why 「YORKTOWN AND THE RETURN HOME」: verbatim: \"YORKTOWN AND THE RETURN HOME\", track 3 of Some Band's Lighthouse score (2019); Lighthouse is one of your Favourite Films (4.5★).".into(),
        user: "synthetic".into(),
        type_jp: "近距離パワー型".into(),
        type_en: "Close-Range Power".into(),
        type_note: format!("{TYPE_NOTE_LOW} · {TYPE_NOTE}"),
        grades: ["A", "B", "C", "D", "E", "?"].iter().map(|s| s.to_string()).collect(),
        plot: vec![5.0, 4.0, 3.0, 2.0, 1.0, 3.0],
        prov: vec![true, false, true, false, true, false],
        unknown: vec![false, false, false, false, false, true],
        evidence: (0..6).map(|i| (format!("metric {i}"), format!("Film {i}"))).collect(),
        coverage: (0..6).map(|i| format!("graded on {} of 300 films", 200 + i)).collect(),
        ability: "Wind Reading. It reads the air and answers it.".into(),
        ability_name: "Wind Reading".into(),
        ability_line: "It reads the air and answers it: it agrees with the crowd on 89% of film pairs.".into(),
        credit: standuser_core::svg::CREDIT.into(),
        weakness: "Soft spot: ordinary hands.".into(),
        nearest: vec!["D4C Love Train".into(), "Dolly Dagger".into(), "The World".into()],
        signature: "Signature film: Synthetic Film One (2008), 1.0★ against the crowd's 3.54★".into(),
        badge: true,
        accent: None,
    }
}

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).expect("usage: card_palettes <out_dir>"));
    std::fs::create_dir_all(&out).unwrap();
    let keys: Vec<Option<&str>> = ACCENTS.iter().map(|a| Some(a.0)).chain([None]).collect();
    for key in keys {
        let mut d = sample();
        d.accent = key.map(String::from);
        for (t, name) in [(Theme::Light, "light"), (Theme::Dark, "dark")] {
            std::fs::write(out.join(format!("{}-{name}.svg", key.unwrap_or("NONE"))), card_svg(&d, t)).unwrap();
        }
    }
}
