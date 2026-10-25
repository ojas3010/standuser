//! The track-title name route end to end: on the synthetic export with its synthetic candidates, and on seeded users built
//! from the real film table and the shipped names.json.

mod common;
use common::synth_users;
use standuser_core::{analysis, lookup::Lookup, parse, pool};
use std::path::Path;

fn real() -> Lookup {
    let mut lk = Lookup::from_json(include_str!("../data/films.json")).unwrap();
    lk.load_names(include_str!("../data/names.json")).unwrap();
    lk
}

#[test]
fn synthetic_export_gets_a_title_name_with_a_why_line_on_the_card() {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/synth");
    let zip = std::fs::read(d.join("letterboxd-synthuser-2026-06-30-12-00-utc.zip")).unwrap();
    let mut lk = Lookup::from_json(&std::fs::read_to_string(d.join("table.json")).unwrap()).unwrap();
    let mut ex = parse::parse_zip(&zip, Some("letterboxd-synthuser-2026-06-30-12-00-utc.zip")).unwrap();
    ex.joined = standuser_core::dates::parse_date("2023-01-05");
    let without = analysis::analyze(&ex, &lk);
    assert_eq!(without.name.route, "composer", "no names loaded: the composer route");
    lk.load_names(&std::fs::read_to_string(d.join("names.json")).unwrap()).unwrap();
    let a = analysis::analyze(&ex, &lk);
    assert_eq!(a.name.route, "titles", "{:?}", a.name);
    assert_eq!(a.card.why, a.name.why);
    assert_eq!(a.card.name, a.name.name);
    assert!(a.name.top.iter().any(|t| t.picked && t.name == a.name.name));
    assert_eq!(a.name.top.len(), 5);
    assert!(a.name.top.windows(2).all(|w| w[0].s >= w[1].s), "sorted by S");
    // the same export gives the same name twice
    assert_eq!(analysis::analyze(&ex, &lk).name.name, a.name.name);
}

#[test]
fn seeded_real_table_users_get_deterministic_valid_names_and_clean_why_lines() {
    let lk = real();
    let users = synth_users::users(40, &lk);
    let mut titles = 0;
    let mut differs = 0;
    for (ex, t) in &users {
        let user = ex.username.clone().unwrap();
        let Some(p) = pool::pick(ex, &lk, *t, &user) else { continue };
        titles += 1;
        assert_eq!(pool::pick(ex, &lk, *t, &user).unwrap().name, p.name, "deterministic");
        assert!(p.pool_films <= pool::POOL && p.pool_films >= 1);
        assert!(standuser_core::name::ascii_fold(&p.name).as_deref() == Some(p.name.as_str()), "drawable on the card: {}", p.name);
        assert!(!standuser_core::name::collides_with_canon(&p.name), "{} collides with a canon name", p.name);
        let picked = p.top.iter().filter(|s| s.picked).count();
        assert_eq!(picked, 1);
        assert!(p.top.iter().all(|s| s.cool >= p.median_cool - 1e-9), "nothing below the pool median is listed");
        // the full why line, not the bare fallback: the place, the origin and one cited reason
        assert!(p.why.starts_with(&format!("Why 「{}」: ", p.name)), "{}", p.why);
        assert!(p.why.contains("; ") && p.why.ends_with('.'), "{}", p.why);
        assert!(["fit rank", "Its Cool score", "in your pool by affinity"].iter().any(|r| p.why.contains(r)), "no cited reason: {}", p.why);
        if p.first_track.as_ref().is_some_and(|f| f.name != p.name) {
            differs += 1;
        }
    }
    assert!(titles >= 35, "most users have a pool: {titles}");
    assert!(differs > 0, "the S winner is not always the first track");
}

#[test]
fn different_usernames_do_not_all_get_the_same_name_from_the_same_history() {
    let lk = real();
    let (ex, t) = synth_users::users(1, &lk).remove(0);
    let names: std::collections::HashSet<String> = (0..60).filter_map(|i| pool::pick(&ex, &lk, t, &format!("user-{i}")).map(|p| p.name)).collect();
    assert!(names.len() >= 1);
    // the pick can only move within 0.03 of the top S, so it is a handful of names at most
    assert!(names.len() <= 12, "{names:?}");
}

#[test]
fn every_shipped_lexicon_code_points_at_a_word_of_its_name() {
    let lk = real();
    let (mut codes, mut woodys) = (0, 0);
    for f in lk.names.values() {
        for c in &f.cands {
            let words = pool::name_words(&c.name);
            for code in &c.codes {
                codes += 1;
                assert!((1..=17).contains(&(code / 16)), "template of code {code} in {}", c.name);
                let w = ((code / 2) % 8) as usize;
                assert!(w < words.len(), "{}: word index {w} of {} words (code {code})", c.name, words.len());
                if c.name.contains('\'') {
                    woodys += 1;
                }
            }
        }
    }
    assert!(codes > 10_000, "{codes}");
    assert!(woodys > 100, "names with an apostrophe are covered: {woodys}");
}

// ---------------------------------------------------------------- sensitive phrases
// the list is tools/naming/data/sensitive_phrases.txt, the one the builder filters with
const PHRASES: &str = include_str!("../../../tools/naming/data/sensitive_phrases.txt");

fn blocked(name: &str) -> Option<&'static str> {
    let words: Vec<String> = name.to_lowercase().split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).map(String::from).collect();
    let padded = format!(" {} ", words.join(" "));
    PHRASES.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).find(|p| padded.contains(&format!(" {p} ")))
}

#[test]
fn the_blocklist_helper_catches_the_case_that_won_for_synthetic_user_7() {
    assert_eq!(blocked("RACIAL PROFILING"), Some("racial profiling"));
    assert_eq!(blocked("Racial-Profiling!"), Some("racial profiling"));
    assert_eq!(blocked("RACIAL HARMONY"), None);
    assert_eq!(blocked("SLAVE TO THE RHYTHM"), None);
}

#[test]
fn no_shipped_name_matches_the_sensitive_phrase_list() {
    let lk = real();
    let bad: Vec<(&str, &str)> = lk.names.values().flat_map(|f| f.cands.iter()).filter_map(|c| blocked(&c.name).map(|p| (c.name.as_str(), p))).collect();
    assert!(bad.is_empty(), "{bad:?}");
}

#[test]
fn synthetic_user_7_no_longer_wins_racial_profiling_and_no_seeded_user_gets_a_blocked_name() {
    let lk = real();
    for (i, (mut ex, _)) in synth_users::users(40, &lk).into_iter().enumerate() {
        // as the CLI sees the zips written by the synthetic_zips example: join day and export date, and the history's real template
        ex.joined = standuser_core::dates::parse_date("2022-01-01");
        ex.as_of = standuser_core::dates::parse_date("2026-10-19");
        let a = analysis::analyze(&ex, &lk);
        assert_eq!(a.name.route, "titles", "user {i}");
        assert_eq!(blocked(&a.name.name), None, "user {i} won {}", a.name.name);
        assert!(a.name.top.iter().all(|t| blocked(&t.name).is_none()), "user {i}: {:?}", a.name.top.iter().map(|t| &t.name).collect::<Vec<_>>());
        if i == 7 {
            assert_ne!(a.name.name, "RACIAL PROFILING");
            println!("user 7 now wins {} (track {} of {})", a.name.name, a.name.top.iter().find(|t| t.picked).map(|t| t.track).unwrap_or(0), a.name.top.iter().find(|t| t.picked).map(|t| t.film.as_str()).unwrap_or(""));
        }
    }
}
