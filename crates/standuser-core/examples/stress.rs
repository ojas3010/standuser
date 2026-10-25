//! Writes a worst-case card to stdout (long text, special grades, unsupported characters) for visual checks.
use standuser_core::svg::{card_svg, CardData, Theme};

fn main() {
    let d = CardData {
        name: "THE QUICK BROWN COMPOSER OF MANY LONG NAMES AND HOMAGES".into(),
        why: "Why 「THE QUICK BROWN COMPOSER OF MANY LONG NAMES AND HOMAGES」: tail of \"A Very Long Source Track Title That Goes On And On\", track 12 of Кириллица's 한국어 제목 score (1999); the film is one of your Favourite Films (4.5★). \"homage\" fits your Wind Reading ability; fit rank 1 of 40 and then some more words to overflow.".into(),
        user: "a_very_long_letterboxd_username_for_testing_layout_limits".into(),
        type_jp: "遠隔自動操縦型".into(),
        type_en: "Automatic, Far-Distance Autopilot".into(),
        type_note: "low confidence · archetype by stats · about 60% agreement with canon".into(),
        grades: ["∞", "∅", "完成", "E", "A", "?"].iter().map(|s| s.to_string()).collect(),
        plot: vec![6.0, 0.0, 1.0, 1.0, 5.0, 3.0],
        prov: vec![true, false, true, true, false, false],
        unknown: vec![false, false, false, false, false, true],
        evidence: vec![
            ("−0.16★ vs crowd with a very long metric line that goes on and on".into(), "Borat: Cultural Learnings of America for Make Benefit Glorious Nation of Kazakhstan".into()),
            ("∅ no clean timeline events".into(), "".into()),
            ("3.3 effective countries".into(), "한국어 제목 and Кириллица (Korea)".into()),
            ("45 of 50 months active".into(), "Ocean's Eleven".into()),
            ("DPA 0.90, top 1%".into(), "Interstellar".into()),
            ("47% directors seen once".into(), "A Silent Voice: The Movie".into()),
        ],
        coverage: vec!["graded on 450 of 512 films".into(), "only 3 of 533 films, needs 12".into(), "graded on 505 of 533 films".into(), "graded on 241 of 533 films".into(), "graded on 450 of 512 films".into(), "only 12 of 533 films, needs 40 and a long tail".into()],
        ability: "Time Acceleration. Time speeds up around it: 12.3 films per active month, peaking at 31 in September 2022, a pace almost no other user reaches, and then some more words to force a third line of text in the box. [provisional]".into(),
        weakness: "Weakness: loose aim (Precision D). It agrees with the crowd on 61% of film pairs, so it can hit what it did not mean to, and this sentence keeps going so that it wraps onto a second and even a third line. [provisional]".into(),
        nearest: vec!["Star Platinum: The World".into(), "Dirty Deeds Done Dirt Cheap".into(), "Killer Queen (JoJolion)".into()],
        signature: "Signature film: Borat: Cultural Learnings of America for Make Benefit Glorious Nation of Kazakhstan (2006), 5.0★ against the crowd's 3.67★ (+1.33), 864,600 ratings.".into(),
        badge: true,
        accent: Some("SPD".into()),
    };
    print!("{}", card_svg(&d, if std::env::args().any(|a| a == "dark") { Theme::Dark } else { Theme::Light }));
}
