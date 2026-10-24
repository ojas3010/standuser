"""Turn the collector's films_meta.jsonl + histograms.jsonl into the compact lookup table shipped with the site.

Usage: python3 -I tools/build_table.py <out_dir_of_collector>... [--out crates/standuser-core/data/films.json]
                                       [--include-own] [--built YYYY-MM-DD] [--log tools/table_collisions.json]

The shipped table holds the shared film list only (the collector's 'sample' group) and carries no Letterboxd codes, so it
does not reveal anybody's watch list. `--include-own` adds the author's own films for a local test table. Films whose
"title|year" key is shared by two different films are dropped and logged (see tools/table_lib.py).

Format (see crates/standuser-core/src/lookup.rs):
  {"v":1,"built":"YYYY-MM-DD","countries":{code:name},"people":{id:name},
   "films":[[key,"",tmdb,year,runtime,lang,"US|PL","dirIds","composerIds",avg,histMean,count], ...]}
(the second column, a Letterboxd short code, is always empty in the shipped table)
"""
import datetime, json, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import table_lib as TL  # noqa: E402


def main(argv):
    out = Path("crates/standuser-core/data/films.json")
    built = datetime.date.today().isoformat()   # TMDB data may be cached for at most six months from this date
    include_own, log_path, dirs = False, Path("tools/table_collisions.json"), []
    i = 0
    while i < len(argv):
        a = argv[i]
        if a == "--out":
            out = Path(argv[i + 1]); i += 2
        elif a == "--built":
            built = argv[i + 1]; i += 2
        elif a == "--log":
            log_path = Path(argv[i + 1]); i += 2
        elif a == "--include-own":
            include_own = True; i += 1
        else:
            dirs.append(Path(a)); i += 1
    films, log = TL.load(dirs, include_own=include_own)
    countries, people, rows = {}, {}, []
    no_count = with_hist = 0
    for f in films:
        for c, n in zip(f["codes"], f["code_names"]):
            countries[c] = n
        for x in f["directors"]:
            people[str(x["id"])] = x["name"]
        comps = [x for x in f["composers"] if x.get("job") == "Original Music Composer"] or f["composers"]
        for x in comps:
            people[str(x["id"])] = x["name"]
        no_count += f["count"] is None
        with_hist += f["hist"] is not None
        rows.append([f["key"], "", f["tmdb"], f["year"], f["runtime"], f["lang"], "|".join(f["codes"]),
                     ",".join(str(x["id"]) for x in f["directors"]), ",".join(str(x["id"]) for x in comps),
                     f["avg"], f["hist"], f["count"]])
    out.write_text(json.dumps(dict(v=1, built=built, countries=dict(sorted(countries.items())),
                                  people=dict(sorted(people.items(), key=lambda kv: int(kv[0]))), films=rows),
                              ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
    log_path.write_text(json.dumps(dict(built=built, kept=len(rows), **{k: v for k, v in log.items()}), indent=1, ensure_ascii=False), encoding="utf-8")
    for c in log["collisions"]:
        print(f"collision dropped: {c['key']} ({c['films']} different films)", file=sys.stderr)
    print(dict(records=log["records"], kept=len(rows), collisions_dropped=len(log["collisions"]), not_movie=log["not_movie"], no_tmdb=log["no_tmdb"],
               no_year=log["no_year"], own_only_left_out=log["own_only_left_out"], no_count=no_count, with_hist=with_hist), "->", out, f"{out.stat().st_size/1024:.0f} KB")


if __name__ == "__main__":
    main(sys.argv[1:])
