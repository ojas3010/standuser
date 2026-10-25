"""Compare mb_local_match.py output with the API collector's mb_tracks.jsonl, and report coverage.

Usage: python3 -I tools/mbdump/cmp.py API.jsonl LOCAL.jsonl [key ...]
Agreement is over films present in both files; coverage is over LOCAL.jsonl. Trailing keys are listed individually.
"""
import json, sys, collections, random
api_p, loc_p, *keys = sys.argv[1:]
def rows(p):
    d = {}
    for l in open(p):
        try: r = json.loads(l)
        except ValueError: continue
        d[r["key"]] = r
    return d
api, loc = rows(api_p), rows(loc_p)
def prim(d):
    for f in ("composer", "various", "other"):
        if f in d: return f, d[f]["rg"]
cat = collections.Counter(); ex = collections.defaultdict(list)
for k, a in api.items():
    if k not in loc: continue
    A = {f: t["rg"] for f, t in a["tracklists"].items()}; L = {f: t["rg"] for f, t in loc[k]["local"].items()}
    if A == L: c = "identical" if A else "both_none"
    elif not A: c = "local_only"
    elif not L: c = "api_only"
    elif prim(a["tracklists"]) == prim(loc[k]["local"]): c = "same_primary_other_flags_differ"
    else: c = "primary_differs"
    cat[c] += 1; ex[c].append(k)
n = sum(cat.values())
print(f"agreement over {n} films: {dict(cat)}")
print(f"film-level agreement (identical + both_none) = {100*(cat['identical']+cat['both_none'])/n:.1f}%")
te = collections.Counter()
for k, a in api.items():
    for f, t in a["tracklists"].items():
        l = loc.get(k, {}).get("local", {}).get(f)
        if l and l["rg"] == t["rg"]: te["same" if l["tracks"] == t["tracks"] else "diff"] += 1
print("tracklists where both picked the same group:", dict(te))
random.seed(1)
for c in ("api_only", "primary_differs", "local_only"):
    for k in random.sample(ex[c], min(5, len(ex[c]))):
        print(c, k, api[k]["titles"][:1], "API", {f: t["rg"][:8] for f, t in api[k]["tracklists"].items()},
              "LOC", {f: (t["title"][:35], t["year"], t["artist"][:22], t["via"]) for f, t in loc[k]["local"].items()})
N = len(loc)
anyf = sum(1 for r in loc.values() if r["local"])
comp = sum(1 for r in loc.values() if "composer" in r["local"])
five = sum(1 for r in loc.values() if any(len(t["tracks"]) >= 5 for t in r["local"].values()))
comp5 = sum(1 for r in loc.values() if "composer" in r["local"] and len(r["local"]["composer"]["tracks"]) >= 5)
print(f"coverage over {N} films: any flag {100*anyf/N:.1f}%  composer {100*comp/N:.1f}%  5+ tracks (any flag) {100*five/N:.1f}%  composer+5+ {100*comp5/N:.1f}%")
for k in keys:
    r = loc.get(k)
    print(k, {f: (t["title"][:40], t["year"], t["artist"][:25], len(t["tracks"]), t["via"]) for f, t in r["local"].items()} if r else "not in file")
