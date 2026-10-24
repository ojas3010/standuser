"""Turn the collector's films_meta.jsonl + histograms.jsonl into the compact lookup table shipped with the site.

Usage: python3 -I tools/build_table.py <out_dir_of_collector>... [--out crates/standuser-core/data/films.json]
Several collector output folders can be given; the first record for a film wins (own films before sample films).

Format (see crates/standuser-core/src/lookup.rs):
  {"v":1,"countries":{code:name},"people":{id:name},
   "films":[[key,lid,tmdb,year,runtime,lang,"US|PL","dirIds","composerIds",avg,histMean,count], ...]}
"""
import json, sys, unicodedata
from pathlib import Path


def film_key(name, year):
    t = "".join(c for c in unicodedata.normalize("NFKD", name) if c.isalnum()).lower()
    return f"{t}|{year}"


def read_jsonl(p):
    out = []
    if p.exists():
        for line in p.open(encoding="utf-8"):
            line = line.strip()
            if line:
                out.append(json.loads(line))
    return out


def main(argv):
    out = Path("crates/standuser-core/data/films.json")
    dirs = []
    i = 0
    while i < len(argv):
        if argv[i] == "--out":
            out = Path(argv[i + 1]); i += 2
        else:
            dirs.append(Path(argv[i])); i += 1
    countries, people, films, seen = {}, {}, [], set()
    stats = dict(records=0, kept=0, no_tmdb=0, not_movie=0, dup=0, no_count=0, with_hist=0)
    for d in dirs:
        hist = {h["slug"]: h for h in read_jsonl(d / "histograms.jsonl") if h.get("complete")}
        for r in read_jsonl(d / "films_meta.jsonl"):
            stats["records"] += 1
            if r.get("tmdb_type") != "movie" or not r.get("tmdb_id"):
                stats["not_movie" if r.get("tmdb_type") else "no_tmdb"] += 1
                continue
            name, year = r.get("name") or r.get("title"), r.get("year")
            if not name or not year:
                continue
            key = film_key(name, int(year))
            if key in seen:
                stats["dup"] += 1
                continue
            seen.add(key)
            codes = r.get("production_countries") or []
            for c, n in zip(codes, r.get("production_country_names") or []):
                countries[c] = n
            dirs_ = [x for x in (r.get("directors") or [])]
            for x in dirs_:
                people[str(x["id"])] = x["name"]
            comps = r.get("composers") or []
            omc = [x for x in comps if x.get("job") == "Original Music Composer"] or comps
            for x in omc:
                people[str(x["id"])] = x["name"]
            h = hist.get(r.get("lb_slug"))
            hm = round(h["plain_mean"], 4) if h and h.get("plain_mean") else None
            count = r.get("lb_rating_count") or (h["total"] if h else None)
            if count is None:
                stats["no_count"] += 1
            if hm is not None:
                stats["with_hist"] += 1
            films.append([key, r.get("lid") or "", int(r["tmdb_id"]), int(year), int(r.get("runtime") or 0),
                          r.get("original_language") or "", "|".join(codes),
                          ",".join(str(x["id"]) for x in dirs_), ",".join(str(x["id"]) for x in omc),
                          r.get("lb_avg"), hm, count])
            stats["kept"] += 1
    films.sort(key=lambda f: f[0])
    out.write_text(json.dumps(dict(v=1, countries=dict(sorted(countries.items())), people=dict(sorted(people.items(), key=lambda kv: int(kv[0]))),
                                  films=films), ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
    print(stats, "->", out, f"{out.stat().st_size/1024:.0f} KB")


if __name__ == "__main__":
    main(sys.argv[1:])
