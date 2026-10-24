//! Regression tests from the independent review (each reproduced a finding and failed before its fix).

mod common;
use common::*;

/// Spec 1: Range is provisional only below 100 films with country data; at 100 or more it is a full grade.
#[test]
fn range_is_provisional_only_below_100_films() {
    let few = run(&export(60, Some(3.5), true));
    assert_eq!(few.stats.range.status, standuser_core::grading::Status::Provisional, "60 films with countries: provisional");
    assert!(few.card.prov[2]);
    let many = run(&export(150, Some(3.5), true));
    assert!(many.stats.facts.n_films_cty >= 100, "{}", many.stats.facts.n_films_cty);
    assert_eq!(many.stats.range.status, standuser_core::grading::Status::Measured, "{:?}", many.stats.range);
    assert!(!many.card.prov[2], "no asterisk on a Range graded on 100 or more films");
}
