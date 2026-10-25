"""Tests for build_names_data.py: the per-album artist and flag, and the shipped names.json.

Run:  python3 -I -m pytest -q tools/naming/test_build_names_data.py
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import build_names_data as B  # noqa: E402
import cool as C  # noqa: E402
import textlib as t  # noqa: E402

NAMES = os.path.join(t.REPO, "crates", "standuser-core", "data", "names.json")


class NoMatch:
    def matches(self, name):
        return [(0.0, -1)] * 17


def row(key, tmdb, flag, track, name):
    return {"key": key, "tmdb": tmdb, "film": key, "flag": flag, "rg": flag, "track": str(track), "src": name.title(), "name": name, "kind": "v"}


def build(rows, artists, monkeypatch):
    monkeypatch.setattr(C, "features", lambda text, kind, df, n: dict(rarity=len(text) / 40, shape=0.0, conc=0.5, emo=0.5, prior=0.5, proper=0.0))
    return B.build(rows, artists, NoMatch(), {}, 1)[0]


def test_each_candidate_points_at_the_album_it_came_from(monkeypatch):
    rows = [row("12ys|2013", "76203", "composer", 17, "LETTER WRITING"), row("12ys|2013", "76203", "various", 3, "FREIGHT TRAIN"),
            row("12ys|2013", "76203", "composer", 18, "SOLOMON BURNS THE LETTER")]
    films = build(rows, {("12ys|2013", "composer"): "Hans Zimmer", ("12ys|2013", "various"): "Various Artists"}, monkeypatch)
    albums, cands = films["76203"]
    assert albums == [["Hans Zimmer", "c"], ["Various Artists", "v"]]
    assert [(c[0], c[6]) for c in cands] == [("LETTER WRITING", 0), ("FREIGHT TRAIN", 1), ("SOLOMON BURNS THE LETTER", 0)]


def test_a_film_with_a_single_album_has_one_album_entry(monkeypatch):
    films = build([row("a|2000", "1", "other", 1, "ONE"), row("a|2000", "1", "other", 2, "THREE")], {("a|2000", "other"): "Some Band"}, monkeypatch)
    assert films["1"][0] == [["Some Band", "o"]]
    assert {c[6] for c in films["1"][1]} == {0}


def test_strip_keeps_only_albums_that_still_have_a_candidate_and_reindexes(monkeypatch):
    rows = [row("f|2000", "9", "composer", 1, "SCORE ONE"), row("f|2000", "9", "various", 2, "SONG ONE"), row("f|2000", "9", "other", 3, "OTHER ONE")]
    films = build(rows, {("f|2000", "composer"): "C", ("f|2000", "various"): "Various Artists", ("f|2000", "other"): "O"}, monkeypatch)
    films["9"][1] = [c for c in films["9"][1] if c[0] != "SCORE ONE"]  # a cut removed the only candidate of the first album
    albums, cands = B.strip(films)["9"]
    assert albums == [["Various Artists", "v"], ["O", "o"]]
    assert [(c[0], c[6]) for c in cands] == [("SONG ONE", 0), ("OTHER ONE", 1)]
    assert all(len(c) == 7 for c in cands), "the helper max L is not shipped"


def shipped():
    return json.load(open(NAMES, encoding="utf-8"))


def test_12_years_a_slave_credits_each_track_to_its_own_album():
    # LETTER WRITING is on Hans Zimmer's score; FREIGHT TRAIN is on the songs album "Music from and Inspired by", not credited to Zimmer
    albums, cands = shipped()["names"]["76203"]
    by = {c[0]: albums[c[6]] for c in cands}
    assert by["LETTER WRITING"] == ["Hans Zimmer", "c"]
    assert by["FREIGHT TRAIN"][0] != "Hans Zimmer" and by["FREIGHT TRAIN"][1] != "c"
    assert by["FREIGHT TRAIN"] == ["Various Artists", "v"]


def test_shipped_file_is_format_2_with_valid_album_indexes():
    d = shipped()
    assert d["v"] == 2
    multi = 0
    for k, (albums, cands) in d["names"].items():
        assert albums and cands, k
        assert all(len(a) == 2 and a[1] in "cvo" for a in albums), (k, albums)
        assert all(len(c) == 7 and 0 <= c[6] < len(albums) for c in cands), k
        assert {c[6] for c in cands} == set(range(len(albums))), (k, "an album nobody points at")
        multi += len(albums) > 1
    assert multi > 300, multi
