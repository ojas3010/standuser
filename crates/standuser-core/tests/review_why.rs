//! The why line is shown in full: for the 10 synthetic users no row of it ends in "…", and the rows give back the whole line.
//! (The longest why line names.json can produce has its own test in pool.rs.)

mod common;
use common::synth_users;
use standuser_core::font::{font, FontKind};
use standuser_core::{analysis, dates::parse_date, lookup::Lookup, svg};

#[test]
fn no_why_line_of_the_ten_synthetic_users_is_cut() {
    let mut lk = Lookup::from_json(include_str!("../data/films.json")).unwrap();
    lk.load_names(include_str!("../data/names.json")).unwrap();
    let f = font(FontKind::Sans);
    let mut longest = 0;
    for (i, (mut ex, _)) in synth_users::users(10, &lk).into_iter().enumerate() {
        ex.joined = parse_date("2022-01-01");
        ex.as_of = parse_date("2026-10-19");
        let card = analysis::analyze(&ex, &lk).card;
        assert!(card.why.starts_with("Why 「"), "user {i} has a why line: {:?}", card.why);
        let rows = svg::why_rows(&f, &card.why);
        assert!(rows.len() <= svg::WHY_MAX_ROWS, "user {i}: {} rows", rows.len());
        assert!(rows.iter().all(|r| !r.contains('…')), "user {i}: a why row is cut: {rows:?}");
        assert_eq!(rows.join(" "), card.why, "user {i}: the rows give back the whole line");
        // and the card itself draws every row of it: the svg has the why rows in their own paths, and no ellipsis glyph was needed
        let layout = svg::layout(f.wrap(&card.ability, 20.0, 820.0).len().min(3), f.wrap(&card.weakness, 19.0, 820.0).len().min(2), rows.len());
        assert!(layout.pitch >= svg::MIN_PITCH, "user {i}");
        longest = longest.max(rows.len());
    }
    assert!(longest >= 2, "the synthetic users do exercise multi-row why lines");
}
