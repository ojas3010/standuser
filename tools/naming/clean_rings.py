"""Thin the 17 WordNet rings by two rules, the same for every template (docs/DECISIONS.md, "Ring cleanup").

Usage: python3 -I tools/naming/clean_rings.py [--candidates local/naming/candidates.csv] [--out tools/naming/data/ring_drop.json]
 (a) Ring links that exist only through a WordNet-definition top-up core word are dropped (lexicons.ring_for).
 (b) For every remaining ring lemma the frequency is the share of the candidate rows (the candidates table, composer-name rows left
     out) whose name contains the lemma. The pairs (template, lemma) of all templates form one distribution; pairs above its 90th
     percentile (nearest rank) are dropped.
Writes the dropped lemmas per template, so build_names_data.py can apply them. The count reads only the candidates table and the core
lexicons, never names.json (which the drop list itself shapes), so a second run writes the same file.
"""
import argparse
import csv
import json
import math
import os
import sys
from collections import Counter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lexicons as L  # noqa: E402
import textlib as t  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--candidates", default=os.path.join(t.REPO, "local", "naming", "candidates.csv"))
    ap.add_argument("--out", default=L.RING_DROP)
    a = ap.parse_args()
    lex = json.load(open(os.path.join(t.HERE, "data", "lexicons.json"), encoding="utf-8"))
    cands = [r["name"] for r in csv.DictReader(open(a.candidates, encoding="utf-8")) if r["flag"] != "artist"]
    n = len(cands)
    cnt = Counter()
    for name in cands:
        cnt.update({L.word_lemma(w) for w in L.name_words(name) if w not in L.STOP and len(w) >= 2})
    sizes, pairs = {}, []
    for x in lex:
        full = set(L.ring_of([c["lemma"] for c in x["core"]]))
        after_a = L.ring_for(x["core"])
        sizes[x["id"]] = {"before": len(full), "after_a": len(after_a)}
        pairs += [(x["id"], w, cnt[w] / n) for w in after_a]
    freqs = sorted(p[2] for p in pairs)
    thr = freqs[math.ceil(0.9 * len(freqs)) - 1]
    dropped = {}
    for tid, w, f in pairs:
        if f > thr:
            dropped.setdefault(str(tid), []).append(w)
    for x in lex:
        sizes[x["id"]]["after_b"] = sizes[x["id"]]["after_a"] - len(dropped.get(str(x["id"]), []))
    json.dump({"rule": "a: no ring links through WordNet-definition top-up core words; b: drop ring lemmas whose share of the candidates table is above the 90th percentile over all (template, lemma) pairs",
               "candidates": n, "pairs_after_a": len(pairs), "p90_share": thr, "sizes": sizes,
               "dropped": {k: sorted(v) for k, v in sorted(dropped.items(), key=lambda kv: int(kv[0]))}}, open(a.out, "w", encoding="utf-8"), indent=1)
    print(f"{n} candidate rows; {len(pairs)} ring pairs after rule (a); p90 share {thr:.5f}; rule (b) drops {sum(len(v) for v in dropped.values())} pairs")
    print("template: ring before / after (a) / after (a)+(b)")
    for x in lex:
        s = sizes[x["id"]]
        print(f"  {x['id']:>2} {x['name']:<26} {s['before']:>4} {s['after_a']:>4} {s['after_b']:>4}")


if __name__ == "__main__":
    main()
