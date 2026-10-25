"""The candidates table: every film's stage 5 and 6 candidates from the local MusicBrainz match (spec stages 4 to 6, offline).

Usage: python3 -I tools/naming/build_candidates.py [--films site/films.json] [--titles titles.json ...] [--local mb_local.jsonl]
                                                   [--out local/naming/candidates.csv]
One row per surviving candidate: film key, TMDB id, film title, source flag (composer / various / other / artist), release
group, track number, source track title, name, kind (v verbatim, s stripped, c cue, t tail, a composer). The composer of the
film is added as a candidate of kind a, as stage 5 asks. Prints the reject reasons and per-film counts.
"""
import argparse
import csv
import json
import os
import sys
from collections import Counter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import textlib as t  # noqa: E402

HOME = os.path.expanduser("~")
COLUMNS = ["key", "tmdb", "film", "flag", "rg", "track", "src", "name", "kind"]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--films", default=os.path.join(t.REPO, "site", "films.json"))
    ap.add_argument("--titles", nargs="*", default=[os.path.join(HOME, "lb_collect_out", "titles.json"), os.path.join(HOME, "mbdump", "titles_extra.json")])
    ap.add_argument("--local", default=os.path.join(HOME, "lb_collect_out", "mb_local.jsonl"))
    ap.add_argument("--out", default=os.path.join(t.REPO, "local", "naming", "candidates.csv"))
    a = ap.parse_args()

    d = json.load(open(a.films, encoding="utf-8"))
    people = d["people"]
    titles = {}
    for p in a.titles:
        titles.update(json.load(open(p, encoding="utf-8")))
    films = {f[0]: f for f in d["films"]}

    os.makedirs(os.path.dirname(a.out), exist_ok=True)
    rejects, per_film, flags = Counter(), Counter(), Counter()
    rows = 0
    with open(a.out, "w", newline="", encoding="utf-8") as fh:
        w = csv.writer(fh)
        w.writerow(COLUMNS)
        for line in open(a.local, encoding="utf-8"):
            rec = json.loads(line)
            f = films.get(rec["key"])
            if f is None:
                continue
            ftitles = [x for x in titles.get(str(f[2]), []) if x]
            shown = ftitles[0] if ftitles else rec["key"]
            seen, n = set(), 0
            for flag in ("composer", "various", "other"):
                alb = rec["local"].get(flag)
                if not alb:
                    continue
                rj = []
                for c in t.candidates_for_album(alb["tracks"], ftitles, rejects=rj):
                    if c["name"] in seen:
                        continue
                    seen.add(c["name"])
                    w.writerow([rec["key"], f[2], shown, flag, alb["rg"], c["track"], c["src"], c["name"], c["kind"]])
                    n += 1
                    flags[flag] += 1
                rejects.update(rj)
            for cid in f[8].split(","):
                name = people.get(cid) if cid else None
                if not name or t.stage6(name, ftitles, film=True) or t.extra_checks(name):
                    continue
                dn = t.display_name(name)
                if dn in seen:
                    continue
                seen.add(dn)
                w.writerow([rec["key"], f[2], shown, "artist", "", "", name, dn, "a"])
                n += 1
                flags["artist"] += 1
            per_film[n] += 1
            rows += n
    print(f"{rows} candidates -> {a.out}")
    print("by source:", dict(flags))
    print("films by candidate count: 0:", per_film[0], " 1-2:", sum(v for k, v in per_film.items() if 1 <= k <= 2),
          " 3+:", sum(v for k, v in per_film.items() if k >= 3), " of", sum(per_film.values()))
    print("rejected:", dict(rejects.most_common()))


if __name__ == "__main__":
    main()
