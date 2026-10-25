"""Tests for lexicons.py against the committed core lexicons (tools/naming/data/lexicons.json).

Run:  python3 -I -m pytest -q tools/naming/test_lexicons.py
"""
import json
import os
import sys

import pytest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lexicons as L  # noqa: E402
import textlib as t  # noqa: E402

PATH = os.path.join(t.HERE, "data", "lexicons.json")
needs_wordnet = pytest.mark.skipif(not os.path.isdir(os.path.join(t.DATA, "nltk_data")), reason="WordNet not in $NAMING_DATA")


def lex():
    return json.load(open(PATH, encoding="utf-8"))


def test_seventeen_lexicons_of_ten_to_thirty_words_each_with_a_source_phrase():
    lx = lex()
    assert [x["id"] for x in lx] == list(range(1, 18))
    for x in lx:
        assert L.MIN_CORE <= len(x["core"]) <= L.MAX_CORE, x["name"]
        lemmas = [c["lemma"] for c in x["core"]]
        assert len(set(lemmas)) == len(lemmas)
        for c in x["core"]:
            assert c["phrase"] and c["from"] and c["word"] in c["phrase"].lower(), (x["name"], c)


def test_markdown_lists_every_core_word():
    lx = lex()
    for x in lx:
        x["ring"] = []
    md = L.markdown(lx)
    assert md.count("\n## ") == 17
    for x in lx:
        for c in x["core"]:
            assert f"| {c['lemma']} | {c['word']} |" in md


@needs_wordnet
def test_l_is_one_for_core_half_for_ring_zero_otherwise():
    lx = [{**x, "ring": L.ring_of([c["lemma"] for c in x["core"]])} for x in lex()]
    sc = L.Scorer(lx)
    bomb = [x for x in lx if x["name"] == "Bomb Transmutation"][0]
    assert not set(c["lemma"] for c in bomb["core"]) & set(bomb["ring"])
    assert sc.l_values("Big Bomb")[0] == 1.0
    assert sc.l_values("Bombs Away")[0] == 1.0  # lemmatised
    ring_word = next(w for w in bomb["ring"] if w not in L.STOP and sc.l_values(w)[0] == 0.5)
    assert sc.l_values(ring_word)[0] == 0.5
    assert sc.l_values("Xylophone Quartz")[0] == 0.0
    assert len(sc.l_values("Anything")) == 17


@needs_wordnet
def test_matches_report_the_word_index_and_core_versus_ring():
    lx = [{**x, "ring": L.ring_of([c["lemma"] for c in x["core"]])} for x in lex()]
    sc = L.Scorer(lx)
    assert sc.matches("BIG BOMBS")[0] == (1.0, 1)
    assert sc.matches("XYLOPHONE QUARTZ")[0] == (0.0, -1)
    assert [m[0] for m in sc.matches("BIG BOMBS")] == sc.l_values("BIG BOMBS")


def test_name_words_match_the_rust_split_and_keep_apostrophes_inside_a_word():
    assert L.name_words("WOODY'S GONE") == ["woody's", "gone"]
    assert L.name_words("DON'T PANIC!") == ["don't", "panic"]
    assert L.name_words("T1000 TERMINATED") == ["t", "terminated"]
    assert L.name_words("SPIDER-MAN 'ROUND") == ["spider", "man", "round"]
    assert L.name_words("ROCKIN' ON") == ["rockin", "on"]


@needs_wordnet
def test_matches_index_the_same_words_the_rust_side_indexes_woodys_gone():
    sc = L.Scorer([{"core": [{"lemma": "go"}], "ring": []}, {"core": [{"lemma": "woody"}], "ring": []}])
    assert sc.matches("WOODY'S GONE") == [(1.0, 1), (1.0, 0)]  # 'gone' -> go at word 1; "woody's" -> woody at word 0
    assert L.word_lemma("woody's") == "woody"
