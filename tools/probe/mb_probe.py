"""Probe MusicBrainz soundtrack tracklist coverage for site/films.json. Read-only; changes no product code.

Usage: TMDB_KEY=... python3 -I tools/probe/mb_probe.py [--n 200] [--out tools/probe/out]
The table holds only slugs, so titles are fetched from TMDB (cached in out/titles.json).
"""
import argparse, json, os, random, re, sys, time, unicodedata, urllib.parse, urllib.request
from pathlib import Path

UA = "standuser-probe/0.1 ( https://github.com/ojas3010/standuser )"
CHECK = ["thelighthouse|2019", "chungkingexpress|1994", "anotherround|2020", "callmebyyourname|2017", "se7en|1995"]
_last = [0.0]

def get(url, mb=True):
    if mb:
        wait = 1.05 - (time.time() - _last[0])
        if wait > 0: time.sleep(wait)
    for attempt in range(4):
        _last[0] = time.time()
        try:
            req = urllib.request.Request(url, headers={"User-Agent": UA, "Accept": "application/json"})
            with urllib.request.urlopen(req, timeout=30) as r:
                return json.load(r)
        except urllib.error.HTTPError as e:
            if e.code in (429, 503): time.sleep(5 * (attempt + 1)); continue
            if e.code == 404: return None
            raise
        except OSError:
            time.sleep(3)
    return None

def norm(s):
    s = unicodedata.normalize("NFKD", s).encode("ascii", "ignore").decode().lower()
    return re.sub(r"[^a-z0-9]", "", s)

SUFFIX = re.compile(r"[\(\[][^\)\]]*(soundtrack|score|music|ost|motion picture|film)[^\)\]]*[\)\]]|[:\-–]\s*(original\s+)?(motion picture\s+)?(soundtrack|score|music).*$", re.I)
def title_forms(t):
    out = {norm(t), norm(SUFFIX.sub("", t))}
    out.discard("")
    return out

def year_of(d):
    m = re.match(r"(\d{4})", d or "")
    return int(m.group(1)) if m else None

def artist_ok(credit, composers):
    c = norm(credit)
    return any(norm(p) and (norm(p) in c or c in norm(p) and len(c) > 3) for p in composers)

def main():
    ap = argparse.ArgumentParser(); ap.add_argument("--n", type=int, default=200); ap.add_argument("--out", default="tools/probe/out")
    a = ap.parse_args()
    out = Path(a.out); out.mkdir(parents=True, exist_ok=True)
    key = os.environ["TMDB_KEY"]
    d = json.load(open("site/films.json")); people = d["people"]
    films = {f[0]: f for f in d["films"]}
    rng = random.Random(42)
    sample = rng.sample(d["films"], a.n)
    keys = [f[0] for f in sample] + [k for k in CHECK if k not in {f[0] for f in sample}]
    tp = out / "titles.json"
    titles = json.load(open(tp)) if tp.exists() else {}
    results = []
    for i, k in enumerate(keys):
        f = films[k]; tmdb, year = f[2], f[3]
        comps = [people[c] for c in f[8].split(",") if c]
        if str(tmdb) not in titles:
            j = get(f"https://api.themoviedb.org/3/movie/{tmdb}?api_key={key}", mb=False)
            titles[str(tmdb)] = [j.get("title", ""), j.get("original_title", "")] if j else ["", ""]
            json.dump(titles, open(tp, "w"))
        tt = [t for t in titles[str(tmdb)] if t]
        rec = dict(key=k, year=year, titles=tt, composers=comps, check=k in CHECK, accepted=None, rejected=[], tracks=None)
        if tt:
            q = f'releasegroup:"{tt[0]}" AND secondarytype:soundtrack'
            j = get("https://musicbrainz.org/ws/2/release-group?fmt=json&limit=25&query=" + urllib.parse.quote(q)) or {}
            want = set().union(*(title_forms(t) for t in tt))
            for rg in j.get("release-groups", []):
                y = year_of(rg.get("first-release-date"))
                if not (title_forms(rg["title"]) & want) or y is None or abs(y - year) > 1: continue
                credit = "".join(c.get("name", "") + c.get("joinphrase", "") for c in rg.get("artist-credit", []))
                m = dict(id=rg["id"], title=rg["title"], year=y, artist=credit)
                if comps and artist_ok(credit, comps):
                    if not rec["accepted"]: rec["accepted"] = m
                else:
                    rec["rejected"].append(m)
            if rec["accepted"]:
                r = get(f"https://musicbrainz.org/ws/2/release?fmt=json&limit=1&inc=recordings&release-group={rec['accepted']['id']}")
                rel = (r or {}).get("releases") or []
                if rel:
                    rec["tracks"] = [t["title"] for m in rel[0].get("media", []) for t in m.get("tracks", [])]
        results.append(rec)
        print(f"{i+1}/{len(keys)} {k} acc={bool(rec['accepted'])} rej={len(rec['rejected'])} tr={len(rec['tracks'] or [])}", file=sys.stderr, flush=True)
        json.dump(results, open(out / "results.json", "w"), indent=1)

if __name__ == "__main__":
    main()
