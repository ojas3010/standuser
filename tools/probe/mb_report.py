"""Summarise tools/probe/out/results.json from mb_probe.py."""
import gzip, json, random, statistics, sys
R = json.load(open(sys.argv[1] if len(sys.argv) > 1 else "tools/probe/out/results.json"))
S = [r for r in R if not r["check"]]; C = [r for r in R if r["check"]]
def stats(rs):
    acc = [r for r in rs if r["tracks"]]
    return acc
withc = [r for r in S if r["composers"]]
acc = [r for r in S if r["accepted"]]; tr = [r for r in S if r["tracks"]]
print(f"sample {len(S)}: with composer {len(withc)}; accepted {len(acc)} ({100*len(acc)/len(S):.1f}% of all, {100*len(acc)/max(1,len(withc)):.1f}% of films with composer); tracklist fetched {len(tr)}")
n = [len(r["tracks"]) for r in tr]
print(f"5+ tracks: {sum(x>=5 for x in n)} ({100*sum(x>=5 for x in n)/len(S):.1f}% of sample); median tracks {statistics.median(n) if n else 0}")
print(f"rejected near-matches: {sum(len(r['rejected'])>0 for r in S)} films")
for r in random.Random(1).sample(acc, min(20, len(acc))):
    a = r["accepted"]; print("HAND", r["key"], "|", r["composers"], "<>", a["title"], "/", a["artist"], a["year"])
print("--- check set")
for r in C:
    print(r["key"], "composers", r["composers"], "accepted", r["accepted"] and (r["accepted"]["title"], r["accepted"]["artist"]), "rej", [(m["title"], m["artist"]) for m in r["rejected"]], "tracks", len(r["tracks"] or []), (r["tracks"] or [])[:4])
g = len(gzip.compress(json.dumps([r["tracks"] for r in tr], ensure_ascii=False).encode()))
print(f"gzipped tracklists for sample: {g} B -> x{9787/len(S):.1f} = {g*9787/len(S)/1e6:.2f} MB for 9,787 films; run time ~ {9787*(1+len(acc)/len(S))*1.05/3600:.1f} h (+TMDB titles)")
