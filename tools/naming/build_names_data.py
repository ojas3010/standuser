"""Item 7 payload: the shipped candidate data, a separate file (crates/standuser-core/data/names.json), built from the candidates table.

Usage: python3 -I tools/naming/build_names_data.py [--candidates local/naming/candidates.csv]
        [--local ~/lb_collect_out/mb_local.jsonl] [--out crates/standuser-core/data/names.json] [--budget 614400] [--dry]
Writes {"v":2,"meta":{...},"names":{tmdb_id: [[[artist, "c"|"v"|"o"], ...albums], [[NAME, cool 0-100, track, kind, source title or "", [codes], album], ...]]}}.
A film has one entry per soundtrack album its candidates come from (a score, a songs album, a various-artists album); a candidate's last
element is the index of its album, so the artist and the score/soundtrack flag in its why line are its own album's. films.json is untouched. A code is template * 16 + word_index * 2 + (1 when the lexicon match is a core word, 0 for a ring word).
Cool is the equal-weight raw score (cool.raw_equal) min-max scaled over the candidates table. Composer-name candidates are not
shipped: the composer route is the first fallback. If the gzipped names exceed the budget, cut in order: (1) candidates below the
global 40th percentile of Cool, (2) all but the top 6 per film by Cool + max L. Each cut's effect is printed.
"""
import argparse
import csv
import gzip
import json
import os
import sys
from collections import defaultdict

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import cool as C  # noqa: E402
import lexicons as L  # noqa: E402
import textlib as t  # noqa: E402

HOME = os.path.expanduser("~")
FLAG = {"composer": "c", "various": "v", "other": "o"}


def gz_size(obj):
    return len(gzip.compress(json.dumps(obj, ensure_ascii=False, separators=(",", ":")).encode("utf-8"), 9, mtime=0))


def pct(sorted_vals, q):
    return sorted_vals[min(len(sorted_vals) - 1, int(q * len(sorted_vals)))]


def build(rows, artists, scorer, df, n):
    """rows: candidate CSV rows (no composer-name rows). artists: {(film key, flag): album artist}.
    Returns (films by tmdb id: [albums [artist, flag], candidates], raw lo, raw hi).
    A candidate is [name, cool, track, kind, source, codes, album index, max L]; the album index is its album's position in the film's albums."""
    cache, raws = {}, {}
    for r in rows:
        nm = r["name"]
        if nm not in cache:
            f = C.features(nm, r["kind"], df, n)
            cache[nm] = (C.raw_equal(f), scorer.matches(nm))
    lo = min(v[0] for v in cache.values())
    hi = max(v[0] for v in cache.values())
    films = defaultdict(lambda: [[], []])
    for r in rows:
        raw, m = cache[r["name"]]
        codes = []
        for ti, (lv, w) in enumerate(m, 1):
            if lv > 0:
                # a code is template * 16 + word * 2 + core: the word index has 3 bits, and a larger one would change the template
                assert 0 <= w < 8, f"word index {w} of {r['name']!r} (template {ti}) does not fit a lexicon code"
                codes.append(ti * 16 + w * 2 + (1 if lv == 1.0 else 0))
        same = t.fold_ascii(r["src"]).upper().split() == r["name"].split()
        albums, cands = films[r["tmdb"]]
        flag = FLAG[r["flag"]]
        flags = [a[1] for a in albums]
        if flag not in flags:
            albums.append([artists.get((r["key"], r["flag"]), ""), flag])
            flags.append(flag)
        cands.append([r["name"], round(100 * C.scale(raw, lo, hi)), int(r["track"]), r["kind"], "" if same else r["src"], codes, flags.index(flag), max([lv for lv, _ in m] or [0])])
    return films, lo, hi


def strip(films, with_artist=True):
    """The shipped form: the albums that still have a candidate (re-indexed), and the candidates without their helper max L."""
    out = {}
    for k, (albums, cands) in films.items():
        used = sorted({c[6] for c in cands})
        new = {old: i for i, old in enumerate(used)}
        out[k] = [[[albums[o][0] if with_artist else "", albums[o][1]] for o in used], [c[:6] + [new[c[6]]] for c in cands]]
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--candidates", default=os.path.join(t.REPO, "local", "naming", "candidates.csv"))
    ap.add_argument("--local", default=os.path.join(HOME, "lb_collect_out", "mb_local.jsonl"))
    ap.add_argument("--out", default=os.path.join(t.REPO, "crates", "standuser-core", "data", "names.json"))
    ap.add_argument("--budget", type=int, default=614400)
    ap.add_argument("--dry", action="store_true")
    a = ap.parse_args()

    allrows = list(csv.DictReader(open(a.candidates, encoding="utf-8")))
    df, n = C.document_frequencies([r["name"] for r in allrows])
    rows = [r for r in allrows if r["flag"] != "artist"]
    artists = {}
    for line in open(a.local, encoding="utf-8"):
        rec = json.loads(line)
        for fl in ("composer", "various", "other"):
            if rec["local"].get(fl):
                artists[(rec["key"], fl)] = rec["local"][fl]["artist"]
    lx = lexicons_with_rings()
    scorer = L.Scorer(lx)
    films, lo, hi = build(rows, artists, scorer, df, n)
    total = sum(len(v[1]) for v in films.values())
    print(f"{total} candidates in {len(films)} films; raw Cool range [{lo:.3f}, {hi:.3f}]")

    report = []

    def measure(label, fl, with_artist=True):
        s = strip(fl, with_artist)
        g = gz_size({"v": 2, "names": s})
        cnt = sum(len(v[1]) for v in fl.values())
        print(f"{label}: {cnt} candidates, {len(fl)} films, +{g / 1024:.0f} KiB gzipped as names.json")
        report.append({"step": label, "candidates": cnt, "films": len(fl), "gz_added": g})
        return s, g

    s, g = measure("full", films)
    cur = films
    if g > a.budget:
        s2, g2 = measure("full, without album artist", films, False)
        # the artist stays out of the why line only if it is the difference between fitting and not; try the cuts first
        cools = sorted(c[1] for v in cur.values() for c in v[1])
        cut = pct(cools, 0.40)
        cur = {k: [v[0], [c for c in v[1] if c[1] >= cut]] for k, v in cur.items()}
        cur = {k: v for k, v in cur.items() if v[1]}
        s, g = measure(f"cut 1: Cool below the global 40th percentile ({cut}) dropped", cur)
        if g > a.budget:
            cur = {k: [v[0], sorted(v[1], key=lambda c: (-(c[1] + 100 * c[7]), c[2], c[0]))[:6]] for k, v in cur.items()}
            s, g = measure("cut 2: top 6 per film by Cool + max L", cur)
    shipped = g <= a.budget
    print("within budget" if shipped else f"STOP: over the {a.budget} byte budget")
    if a.dry or not shipped:
        json.dump(report, open(os.path.join(t.REPO, "local", "naming", "payload_report.json"), "w"), indent=1)
        sys.exit(0 if shipped else 3)
    out = {"v": 2, "meta": {"cool_raw_lo": round(lo, 4), "cool_raw_hi": round(hi, 4), "weights": "equal", "templates": 17, "gz_bytes": g,
                            "source": "MusicBrainz core data (track titles and artist credits)"}, "names": s}
    json.dump(out, open(a.out, "w", encoding="utf-8"), ensure_ascii=False, separators=(",", ":"))
    json.dump(report, open(os.path.join(t.REPO, "local", "naming", "payload_report.json"), "w"), indent=1)


def lexicons_with_rings():
    core = json.load(open(os.path.join(t.HERE, "data", "lexicons.json"), encoding="utf-8"))
    drop = L.load_drop()
    return [{**x, "ring": L.ring_for(x["core"], drop.get(str(x["id"]), ()))} for x in core]


if __name__ == "__main__":
    main()
