"""Cache TMDB titles (title, original_title) for every film in site/films.json that the caches lack. 8 threads.

Usage: TMDB_KEY=... python3 -I tools/mbdump/fetch_titles.py site/films.json out.json [existing_titles.json ...]
Existing caches are read, never written; out.json holds only the new entries (resumable).
"""
import json, os, sys, time, urllib.error, urllib.request
from concurrent.futures import ThreadPoolExecutor
films, out, *have = sys.argv[1:]
key = os.environ["TMDB_KEY"]
titles = {}
for p in have: titles.update(json.load(open(p)))
new = json.load(open(out)) if os.path.exists(out) else {}
titles.update(new)
todo = [f[2] for f in json.load(open(films))["films"] if str(f[2]) not in titles]
print(f"{len(todo)} to fetch", flush=True)

def one(tmdb):
    for attempt in range(4):
        try:
            with urllib.request.urlopen(f"https://api.themoviedb.org/3/movie/{tmdb}?api_key={key}", timeout=30) as r:
                j = json.load(r); return str(tmdb), [j.get("title", ""), j.get("original_title", "")]
        except urllib.error.HTTPError as e:
            if e.code == 404: return str(tmdb), ["", ""]
            time.sleep(3 * (attempt + 1))
        except OSError:
            time.sleep(3)
    return None

with ThreadPoolExecutor(8) as ex:
    for i, r in enumerate(ex.map(one, todo)):
        if r: new[r[0]] = r[1]
        if i % 500 == 499: json.dump(new, open(out, "w")); print(i + 1, flush=True)
json.dump(new, open(out, "w")); print("done", len(new))
