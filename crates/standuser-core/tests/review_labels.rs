//! Regression tests from the independent review (each reproduced a finding and failed before its fix).

mod common;
use common::*;
use standuser_core::svg;

/// Spec: "A one-value rater gets computed grades, shown as E with a note, not ∅."
#[test]
fn one_value_rater_gets_no_note() {
    let a = run(&export(200, Some(3.0), true));
    println!("grades {:?}; PRC note {:?}; notes {:?}", a.card.grades, a.stats.precision.note, a.notes);
    let all = format!("{:?} {:?}", a.notes, a.card);
    assert!(all.to_lowercase().contains("same rating") || all.to_lowercase().contains("one value") || all.to_lowercase().contains("single rating"), "no note about the one-value rater");
}

/// Spec, clean timeline: users without a diary get the watched.csv-only detector "and the card says 'from marking dates'".
#[test]
fn no_diary_user_is_not_told_dates_are_marking_dates() {
    let a = run(&export(200, Some(3.5), false));
    let s = svg::card_svg(&a.card, svg::Theme::Light);
    let txt = format!("{:?} {:?} {:?}", a.notes, a.card, a.stats.speed.note);
    println!("SPD {} STA {}; notes {:?}; SPD coverage {:?}", a.card.grades[1], a.card.grades[3], a.notes, a.card.coverage[1]);
    assert!(a.stats.speed.letter != "?" && a.stats.speed.letter != "∅");
    assert!(txt.contains("marking dates") || s.contains("marking dates"), "nothing tells a diary-less user that Speed/Stamina come from marking dates");
}


/// Spec: the card calls the type an "archetype by stats" and states the ~60% canon agreement; Automatic is flagged low confidence.
#[test]
fn card_labels_the_type_as_an_archetype_by_stats() {
    use standuser_core::{analysis, lookup::Lookup, parse, types::StandType};
    let d = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/synth"));
    let zip = std::fs::read(d.join("letterboxd-synthuser-2026-06-30-12-00-utc.zip")).unwrap();
    let ex = parse::parse_zip(&zip, Some("letterboxd-synthuser-2026-06-30-12-00-utc.zip")).unwrap();
    let a = analysis::analyze(&ex, &Lookup::from_json(&std::fs::read_to_string(d.join("table.json")).unwrap()).unwrap());
    assert!(a.card.type_note.contains("archetype by stats") && a.card.type_note.contains("60%"), "{}", a.card.type_note);
    assert!(!a.card.type_note.contains("low confidence"), "only Automatic is low confidence (this export is {:?})", a.kind);
    // the rule the flag comes from, and the text it adds
    assert!(StandType::Automatic.low_confidence() && !StandType::CloseRange.low_confidence());
    let low = format!("{} · {}", svg::TYPE_NOTE_LOW, svg::TYPE_NOTE);
    assert!(low.contains("low confidence") && low.contains("archetype by stats"));
    // and the note is drawn: the card has more outlined text with it than without
    let with = svg::card_svg(&a.card, svg::Theme::Light);
    let mut blank = a.card.clone();
    blank.type_note.clear();
    assert!(with.len() > svg::card_svg(&blank, svg::Theme::Light).len());
}
