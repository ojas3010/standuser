"""Test for build_candidates.py on a two-film fixture.

Run:  python3 -I -m pytest -q tools/naming/test_build_candidates.py
"""
import csv
import json
import os
import subprocess
import sys

import pytest

HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.environ.get("NAMING_DATA", os.path.expanduser("~/naming_data"))


@pytest.mark.skipif(not os.path.isdir(os.path.join(DATA, "nltk_data")), reason="WordNet not in $NAMING_DATA")
def test_driver_writes_surviving_candidates_and_composer(tmp_path):
    films = {"people": {"7": "Hans Zimmer", "8": "Pat Metheny"},
             "films": [["lighthouse|2019", "", 11, 2019, 109, "en", "US", "1", "7", 2.1, None, 5],
                       ["nomusic|2001", "", 12, 2001, 90, "en", "US", "1", "8", 2.0, None, 5]]}
    local = [{"key": "lighthouse|2019", "n_cands": 1, "local": {"composer": {"rg": "g1", "title": "The Lighthouse", "year": 2019, "artist": "X",
              "via": "norm", "tracks": ["Main Title", "Stranded", "Lighthouse Keeper", "Killer Queen", "Dirty Weather (Live)", "Dirty Weather (Reprise)"]}}},
             {"key": "nomusic|2001", "n_cands": 0, "local": {}}]
    (tmp_path / "films.json").write_text(json.dumps(films))
    (tmp_path / "titles.json").write_text(json.dumps({"11": ["Lighthouse"], "12": ["No Music"]}))
    (tmp_path / "local.jsonl").write_text("\n".join(json.dumps(r) for r in local) + "\n")
    out = tmp_path / "out" / "c.csv"
    r = subprocess.run([sys.executable, "-I", os.path.join(HERE, "build_candidates.py"), "--films", str(tmp_path / "films.json"),
                        "--titles", str(tmp_path / "titles.json"), "--local", str(tmp_path / "local.jsonl"), "--out", str(out)],
                       capture_output=True, text=True)
    assert r.returncode == 0, r.stderr
    rows = list(csv.DictReader(open(out, encoding="utf-8")))
    got = [(x["key"], x["flag"], x["name"], x["track"]) for x in rows]
    assert got == [("lighthouse|2019", "composer", "STRANDED", "2"), ("lighthouse|2019", "composer", "DIRTY WEATHER", "5"),
                   ("lighthouse|2019", "artist", "HANS ZIMMER", ""), ("nomusic|2001", "artist", "PAT METHENY", "")]
    # Main Title is generic, Lighthouse Keeper repeats the film title, Killer Queen is canon, and the bracketed (Reprise) is a cue
