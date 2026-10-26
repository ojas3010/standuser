"""Full MusicBrainz soundtrack collection for site/films.json. Resumable; one JSON line per film.

Usage: TMDB_KEY=... python3 tools/probe/mb_collect.py [--out ~/lb_collect_out/mb_tracks.jsonl]
Reuses the helpers from mb_probe.py. Run from the repo root (standuser/).
"""
import argparse, json, os, sys, urllib.parse
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
from mb_probe import get, title_forms, year_of, artist_ok, norm

def flag_of(credit, comps):
    if norm(credit) == "variousartists": return "various"
    return "composer" if comps and artist_ok(credit, comps) else "other"

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=str(Path.home() / "lb_collect_out" / "mb_tracks.jsonl"))
    a = ap.parse_args()
    out = Path(a.out).expanduser(); out.parent.mkdir(parents=True, exist_ok=True)
    key = os.environ["TMDB_KEY"]
    d = json.load(open("site/films.json")); people = d["people"]
    done = set()
    if out.exists():
        for line in open(out):
            try: done.add(json.loads(line)["key"])
            except (ValueError, KeyError): pass  # torn last line: film is redone
    tp = out.parent / "titles.json"
    titles = json.load(open(tp)) if tp.exists() else {}
    todo = [f for f in d["films"] if f[0] not in done]
    print(f"{len(done)} done, {len(todo)} to go", flush=True)
    with open(out, "a") as fh:
        for i, f in enumerate(todo):
            k, tmdb, year = f[0], f[2], f[3]
            comps = [people[c] for c in f[8].split(",") if c]
            if str(tmdb) not in titles:
                j = get(f"https://api.themoviedb.org/3/movie/{tmdb}?api_key={key}", mb=False)
                if j is None: print(f"skip {k}: tmdb lookup failed", flush=True); continue
                titles[str(tmdb)] = [j.get("title", ""), j.get("original_title", "")]
                json.dump(titles, open(tp, "w"))
            tt = [t for t in titles[str(tmdb)] if t]
            rec = dict(key=k, tmdb=tmdb, year=year, titles=tt, composers=comps, candidates=[], tracklists={})
            if tt:
                q = f'releasegroup:"{tt[0]}" AND secondarytype:soundtrack'
                j = get("https://musicbrainz.org/ws/2/release-group?fmt=json&limit=25&query=" + urllib.parse.quote(q))
                if j is None: print(f"skip {k}: search failed", flush=True); continue
                want = set().union(*(title_forms(t) for t in tt))
                for rg in j.get("release-groups", []):
                    y = year_of(rg.get("first-release-date"))
                    if not (title_forms(rg["title"]) & want) or y is None or abs(y - year) > 1: continue
                    credit = "".join(c.get("name", "") + c.get("joinphrase", "") for c in rg.get("artist-credit", []))
                    rec["candidates"].append(dict(id=rg["id"], title=rg["title"], year=y, artist=credit, flag=flag_of(credit, comps),
                                                  exact=norm(rg["title"]) in {norm(t) for t in tt}))
                for flag in ("composer", "various", "other"):
                    cs = [c for c in rec["candidates"] if c["flag"] == flag]
                    if not cs: continue
                    best = min(cs, key=lambda c: (not c["exact"], abs(c["year"] - year)))
                    r = get(f"https://musicbrainz.org/ws/2/release?fmt=json&limit=1&inc=recordings&release-group={best['id']}")
                    rel = (r or {}).get("releases") or []
                    if rel:
                        rec["tracklists"][flag] = dict(rg=best["id"], release=rel[0]["id"],
                            tracks=[t["title"] for m in rel[0].get("media", []) for t in m.get("tracks", [])])
            fh.write(json.dumps(rec, ensure_ascii=False) + "\n"); fh.flush()
            if i % 50 == 0: print(f"{len(done)+i+1}/{len(d['films'])} {k}", flush=True)
    print("complete", flush=True)

if __name__ == "__main__":
    main()
