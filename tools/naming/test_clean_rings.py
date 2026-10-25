"""Test for clean_rings.py: it counts from the candidates table, and a second run gives the same file.

Run:  python3 -I -m pytest -q tools/naming/test_clean_rings.py
"""
import csv
import hashlib
import json
import os
import subprocess
import sys

import pytest

HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.environ.get("NAMING_DATA", os.path.expanduser("~/naming_data"))
COLUMNS = ["key", "tmdb", "film", "flag", "rg", "track", "src", "name", "kind"]


def write_candidates(path, rows):
    with open(path, "w", newline="", encoding="utf-8") as fh:
        w = csv.writer(fh)
        w.writerow(COLUMNS)
        for i, (flag, name) in enumerate(rows, 1):
            w.writerow([f"f{i}|2000", i, f"F{i}", flag, "", i if flag != "artist" else "", name, name, "v" if flag != "artist" else "a"])


def run(cands, out):
    r = subprocess.run([sys.executable, "-I", os.path.join(HERE, "clean_rings.py"), "--candidates", str(cands), "--out", str(out)], capture_output=True, text=True)
    assert r.returncode == 0, r.stderr
    return r.stdout


@pytest.mark.skipif(not os.path.isdir(os.path.join(DATA, "nltk_data")), reason="WordNet not in $NAMING_DATA")
def test_two_runs_write_the_same_file_and_composer_name_rows_are_not_counted(tmp_path):
    rows = [("composer", "WIND AND WEATHER"), ("composer", "TRAVEL SOUTH"), ("various", "GOING HOME"), ("other", "WORM RIDE"), ("artist", "HANS ZIMMER")]
    cands = tmp_path / "c.csv"
    write_candidates(cands, rows)
    outs = []
    for i in (1, 2):
        run(cands, tmp_path / f"drop{i}.json")
        outs.append(hashlib.sha256((tmp_path / f"drop{i}.json").read_bytes()).hexdigest())
    assert outs[0] == outs[1]
    d = json.load(open(tmp_path / "drop1.json", encoding="utf-8"))
    assert d["candidates"] == 4, "the artist row is not a shipped candidate"
    assert "shipped_candidates" not in d


def test_the_script_has_no_names_json_input():
    src = open(os.path.join(HERE, "clean_rings.py"), encoding="utf-8").read()
    assert "names.json" not in src.split('"""')[2], "the count must not depend on the file the drop list shapes"
    assert "--names" not in src
