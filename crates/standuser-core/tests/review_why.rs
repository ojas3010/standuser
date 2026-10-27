//! The why line (on the page, not on the card) is in full: for the 10 synthetic users no row of it ends in "…", and the rows give back the whole line.
//! (The longest why line names.json can produce has its own test in pool.rs.)

mod common;
use common::synth_users;
use standuser_core::font::{font, FontKind};
use standuser_core::{analysis, dates::parse_date, lookup::Lookup, pool};

#[test]
fn no_why_line_of_the_ten_synthetic_users_is_cut() {
    let mut lk = Lookup::from_json(include_str!("../data/films.json")).unwrap();
    lk.load_names(include_str!("../data/names.json")).unwrap();
    let f = font(FontKind::Jp);
    let mut longest = 0;
    for (i, (mut ex, _)) in synth_users::users(10, &lk).into_iter().enumerate() {
        ex.joined = parse_date("2022-01-01");
        ex.as_of = parse_date("2026-10-19");
        let card = analysis::analyze(&ex, &lk).card;
        assert!(card.why.starts_with("Why 「"), "user {i} has a why line: {:?}", card.why);
        let rows = pool::why_rows(&f, &card.why);
        assert!(rows.len() <= pool::WHY_MAX_ROWS, "user {i}: {} rows", rows.len());
        assert!(rows.iter().all(|r| !r.contains('…')), "user {i}: a why row is cut: {rows:?}");
        assert_eq!(rows.join(" "), card.why, "user {i}: the rows give back the whole line");
        longest = longest.max(rows.len());
    }
    assert!(longest >= 2, "the synthetic users do exercise multi-row why lines");
}
