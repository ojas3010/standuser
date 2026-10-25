"""Offline tests for textlib.py (spec stages 5 and 6).

Run:  python3 -I -m pytest -q tools/naming/test_textlib.py
Needs WordNet and the LDNOOBW list under $NAMING_DATA (default ~/naming_data); the tests that read them skip when it is absent.
The canon tests read crates/standuser-core/data/canon_names.txt and ../research_notes/Stand naming algorithm/canon_name_sources.csv.
"""
import os
import sys

import pytest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import textlib as t  # noqa: E402

needs_wordnet = pytest.mark.skipif(not os.path.isdir(os.path.join(t.DATA, "nltk_data")), reason="WordNet not in $NAMING_DATA")
needs_ldnoobw = pytest.mark.skipif(not os.path.isfile(os.path.join(t.DATA, "ldnoobw_en.txt")), reason="LDNOOBW not in $NAMING_DATA")
needs_csv = pytest.mark.skipif(
    not os.path.isfile(os.path.join(os.path.dirname(t.REPO), "research_notes", "Stand naming algorithm", "canon_name_sources.csv")),
    reason="canon CSV not found")


# ---------------------------------------------------------------- folding and display
def test_fold_ascii_keeps_case_and_straightens_punctuation():
    assert t.fold_ascii("Café Ørsted’s “Æon” – ß") == 'Cafe Orsted\'s "AEon" - ss'


def test_display_name_upper_cases_and_rejects_undrawable():
    assert t.display_name("  hey   ya! ") == "HEY YA!"
    assert t.display_name("Soft & Wet") == "SOFT & WET"
    assert t.display_name("夜に駆ける") is None
    assert t.display_name("a_b") is None  # underscore is not in the card alphabet
    assert t.display_name("...") is None  # nothing drawable but punctuation


def test_extra_checks():
    assert t.extra_checks("Stranded") is None
    assert t.extra_checks("Go") == "too-short"
    assert t.extra_checks("x" * 29) == "too-long"
    assert t.extra_checks("a_b_c") == "undrawable"


# ---------------------------------------------------------------- stage 5
def test_bilingual_keeps_latin_side_and_drops_cjk_only():
    assert t.bilingual("夜に駆ける = Yoru ni Kakeru") == "Yoru ni Kakeru"
    assert t.bilingual("Yoru ni Kakeru = 夜に駆ける") == "Yoru ni Kakeru"
    assert t.bilingual("夜に駆ける = 駆ける") is None
    assert t.bilingual("Plain Title") == "Plain Title"
    assert t.stage5("夜に駆ける = 駆ける") == []


def test_strip_brackets_and_subtitles():
    assert t.stage5("Voodoo Child (Slight Return)") == [("Voodoo Child", "s")]
    assert t.stage5("Scary Monsters (and Super Creeps)") == [("Scary Monsters", "s")]
    assert t.stage5("Rain [Live] (Remastered 2011)") == [("Rain", "s")]
    assert t.stage5("Dirty Weather - Main Theme") == [("Dirty Weather", "s")]
    assert t.stage5("Dirty Weather: Part Two") == [("Dirty Weather", "s")]
    assert t.stage5("Stranded feat. Someone Else") == [("Stranded", "s")]


def test_cue_sheet_split_keeps_each_cue():
    assert t.stage5("Dirty Weather / Stranded") == [("Dirty Weather", "c"), ("Stranded", "c")]


def test_verbatim_limits_are_five_words_and_28_characters():
    assert t.stage5("Another One Bites the Dust") == [("Another One Bites the Dust", "v")]  # 5 words, 26 characters
    assert t.stage5("Aaaaaaaaaa Bbbbbbbbbb Cccccccccc") == [("Bbbbbbbbbb Cccccccccc", "t")]  # 32 characters: unknown words count as nouns
    assert t.stage5("on in at of to by up") == []  # seven function words: no tail may start with one


@needs_wordnet
def test_over_long_title_becomes_a_tail():
    # six words: verbatim is out, the shortest noun-ending tail not starting with a function word is kept
    out = t.stage5("Dancing in the Light of Heaven's Door")
    assert out == [("Heaven's Door", "t")]


@needs_wordnet
def test_tail_rule_reproduces_the_three_function_word_canon_truncations():
    assert t.tail_span("Shine On You Crazy Diamond") == "Crazy Diamond"
    assert t.tail_span("Knockin' on Heaven's Door") == "Heaven's Door"
    assert t.tail_span("Another One Bites the Dust") == "Bites the Dust"


@needs_wordnet
def test_tail_skips_function_word_starts_and_non_noun_ends():
    assert t.tail_span("Two Words") is None  # not longer than the tail
    assert t.tail_span("Walking Along Quickly") is None  # 'quickly' is not a noun
    assert t.tail_span("Run for the") is None  # ends in a function word


@needs_wordnet
def test_is_noun_treats_unknown_capitalised_words_as_proper_nouns():
    assert t.is_noun("Door")
    assert t.is_noun("Zzyzxville")
    assert not t.is_noun("quickly")
    assert not t.is_noun("1-5")


# ---------------------------------------------------------------- stage 6 filters
@pytest.mark.parametrize("text,why", [
    ("Main Title", "generic"), ("End Credits", "generic"), ("Love Theme", "generic"), ("Intro", "generic"),
    ("Grand Suite", "cue-word"), ("Moon Overture", "cue-word"), ("Cue Ball", "cue-word"), ("Untitled", "cue-word"),
    ("Theme From Rocky", "theme"), ("Hunter Theme Song", "theme"),
    ("Dirty Weather 2", "number"), ("Rocky III", "number"), ("Fight Part 2", "number"), ("1M4 Rooftop", "number"),
    ("Track 7", "number"), ("Tomb of the Boom 1 2 3", "number"),
    ("ABC Walk Away", "caps-start"), ("NASA Moon", "caps-start"),
])
def test_stage6_hard_filters(text, why):
    assert t.stage6(text, canon=False, film=False, profane=False) == why


@pytest.mark.parametrize("text", ["Stranded", "Rain Man", "ABC", "NASA", "Walk Away", "Marty's Theme", "Hey Ya!"])
def test_stage6_lets_good_names_through(text):
    # an all-caps name passes; only an all-caps first token followed by lower-case text is rejected. "Hey Ya!" falls to the canon filter
    assert t.stage6(text, canon=False, film=False, profane=False) is None


def test_possessive_theme_exception_needs_a_proper_noun_that_is_not_the_film():
    assert t.stage6("Marty's Theme", ["Back to the Future"], canon=False, profane=False) is None
    assert t.stage6("Rocky's Theme", ["Rocky"], canon=False, profane=False) == "theme"
    assert t.stage6("Love's Theme", [], canon=False, profane=False) == "theme"


def test_non_ascii_ratio():
    assert t.stage6("Ça Ira Ça Ira", canon=False, film=False, profane=False) is None  # accents fold; letters stay ASCII-class
    assert t.stage6("チョコレイト Disco", canon=False, film=False, profane=False) == "non-ascii"


@needs_ldnoobw
def test_profanity_is_whole_word_only():
    assert t.stage6("Fuck You", canon=False, film=False) == "profanity"
    assert t.stage6("Scunthorpe Road", canon=False, film=False) is None  # substring of a listed word must not trip it


def test_film_title_hit_matches_whole_words_and_subtitles():
    assert t.film_title_hit("Rocky Road", ["Rocky"])
    assert not t.film_title_hit("Rockyard", ["Rocky"])
    assert t.film_title_hit("The Winter Soldier Road", ["Captain America: The Winter Soldier"])  # subtitle after ': '
    assert not t.film_title_hit("Winter Soldier", ["Captain America: The Winter Soldier"])  # part of the subtitle is not a hit
    assert t.stage6("Rocky Road", ["Rocky"], canon=False, profane=False) == "film-title"
    assert t.stage6("Rocky Road", ["Rocky"], canon=False, film=False, profane=False) is None


# ---------------------------------------------------------------- stage 6 canon collision
def test_edit_le1():
    assert t.edit_le1("lock", "lock") and t.edit_le1("lock", "luck") and t.edit_le1("lock", "locke") and t.edit_le1("locke", "lock")
    assert not t.edit_le1("lock", "lick x") and not t.edit_le1("lock", "link!") and not t.edit_le1("ab", "ba")


def test_canon_norm_folds_the_and_the_last_plural():
    assert t._canon_norm("The Lock") == "lock"
    assert t._canon_norm("Locks") == "lock"
    assert t._canon_norm("Killer Queens") == t._canon_norm("Killer Queen")
    assert t._canon_norm("Bite the Duties") == "bitetheduty"
    assert t._canon_norm("Soft & Wet") == "softwet"  # '&' is dropped, not spelled out


def test_canon_list_is_the_embedded_one():
    names = t.canon_names()
    assert len(names) == 157
    assert "Crazy Diamond" in names and "Killer Queen" in names


def test_every_canon_name_collides_with_itself():
    for n in t.canon_names():
        assert t.collides_with_canon(n.split(" (")[0]), n


@pytest.mark.parametrize("text", ["Killer Queen", "THE LOCK", "Killer Queens", "crazy diamond", "Bites the Dust", "Hey Ya!"])
def test_exact_and_normalised_canon_matches_are_rejected(text):
    assert t.stage6(text, canon=True, film=False, profane=False) == "canon"


def test_long_near_misses_are_rejected_short_ones_kept():
    assert t.collides_with_canon("Purple Haz")  # 9 letters, one deletion from Purple Haze
    assert t.collides_with_canon("Crazy Diamonde")  # one insertion
    assert not t.collides_with_canon("Lick")  # 4 letters, one substitution from "lock" is kept per the spec
    assert not t.collides_with_canon("Stranded")


def test_canon_filter_can_be_switched_off():
    assert t.stage6("Killer Queen", canon=False, film=False, profane=False) is None


@needs_csv
def test_stage6_rejects_no_canon_song_or_album_source_title_once_stage5_has_cleaned_it():
    rows = [r for r in t.load_canon_rows() if r["row_kind"] == "base" and r["source_type"] in ("song", "album")]
    assert len(rows) == 63
    bad = []
    for r in rows:
        for text, _ in t.stage5(r["source_title"]):
            why = t.stage6(text, canon=False, film=False, profane=False)
            if why:
                bad.append((r["source_title"], text, why))
    assert bad == []


@needs_csv
def test_canon_filter_rejects_the_canon_names_themselves():
    names = [r["stand"] for r in t.load_canon_rows() if r["row_kind"] == "base"]
    assert len(names) == 144
    missed = [n for n in names if not t.collides_with_canon(n)]
    assert missed == []


@needs_csv
@needs_wordnet
def test_all_five_canon_truncations_come_out_of_stage5_or_the_tail_rule():
    rows = {r["stand"]: r for r in t.load_canon_rows() if r["row_kind"] == "base" and r["modification"] == "truncated"}
    assert set(rows) == {"Crazy Diamond", "Heaven's Door", "Bites the Dust", "Scary Monsters", "Voodoo Child"}
    for stand, r in rows.items():
        produced = {text for text, _ in t.stage5(r["source_title"])} | {t.tail_span(t.strip_title(r["source_title"])[0][0]) or ""}
        assert stand in produced, (stand, produced)


# ---------------------------------------------------------------- album driver helper
@needs_wordnet
@needs_ldnoobw
def test_candidates_for_album_filters_dedupes_and_numbers_tracks():
    tracks = ["Main Title", "Stranded", "Heaven's Door", "Track 4", "Dirty Weather (Reprise)", "Dirty Weather", "Lighthouse Keeper"]
    got = t.candidates_for_album(tracks, ["Lighthouse"])
    names = [c["name"] for c in got]
    assert names == ["STRANDED", "DIRTY WEATHER"]  # generic, canon, number, cue-word and film-title candidates are gone
    # the bracket on track 5 is stripped before stage 6, so the earliest track keeps the name and the plain repeat on 6 is dropped
    assert [c["track"] for c in got] == [2, 5]
    assert [c["kind"] for c in got] == ["v", "s"]
