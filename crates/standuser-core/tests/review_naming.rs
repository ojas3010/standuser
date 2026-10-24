//! Regression tests from the independent review (each reproduced a finding and failed before its fix).

mod common;
use common::*;

/// Naming spec stage 3 / 9(c): unrated users use their most frequent composers; Part 3 (Tarot) mode only runs
/// "with no composer credits at all".
#[test]
fn unrated_user_with_composer_credits_gets_tarot_name() {
    let a = run(&export(200, None, true));
    println!("grades {:?}; name {:?} route {} why {}", a.card.grades, a.name.name, a.name.route, a.name.why);
    assert_ne!(a.name.route, "tarot", "the user's films carry composer credits, so the Tarot fallback should not apply");
}

