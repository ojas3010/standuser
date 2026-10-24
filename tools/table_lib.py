"""Shared reading of the collector's output for the table builder and the calibration tools.

The shipped table is the shared film list (the 'sample' group of the collector), not anybody's watch list: the author's
own films are left out unless a local table is asked for (`include_own`). Two different films that normalise to the same
"title|year" key cannot be told apart by an export (it carries no film code the table knows), so every row of an ambiguous
key is dropped and logged.
"""
import json, unicodedata
from collections import defaultdict
from pathlib import Path


def film_key(name, year):
    t = "".join(c for c in unicodedata.normalize("NFKD", name) if c.isalnum()).lower()
    return f"{t}|{year}"


def read_jsonl(p):
    p = Path(p)
    return [json.loads(l) for l in p.open(encoding="utf-8") if l.strip()] if p.exists() else []


def load(dirs, include_own=False):
    """Returns (films, log). films: list of dicts (one per shipped film), log: collision and drop counts.

    Each film: slug, key, name, year, lid, tmdb, runtime, lang, codes, code_names, directors [{id,name}], composers
    [{id,name,job}], avg, hist, count, group.
    """
    seen_slug = {}
    hist = {}
    for d in dirs:
        for h in read_jsonl(Path(d) / "histograms.jsonl"):
            if h.get("complete") and h.get("slug") not in hist:
                hist[h["slug"]] = h
    log = dict(records=0, not_movie=0, no_tmdb=0, no_year=0, no_slug=0, collisions=[], own_only_left_out=0)
    for d in dirs:
        for r in read_jsonl(Path(d) / "films_meta.jsonl"):
            log["records"] += 1
            if r.get("tmdb_type") != "movie":
                log["not_movie" if r.get("tmdb_type") else "no_tmdb"] += 1
                continue
            if not r.get("tmdb_id"):
                log["no_tmdb"] += 1
                continue
            slug = r.get("lb_slug")
            name, year = r.get("name") or r.get("title"), r.get("year")
            if not slug:
                log["no_slug"] += 1
                continue
            if not name or not year:
                log["no_year"] += 1
                continue
            # a histogram mean is shipped only for the films the collector chose for it before looking at anyone's list
            # (hist_wanted); otherwise its presence would mark the author's own films
            h = hist.get(slug) if r.get("hist_wanted") else None
            rec = dict(
                slug=slug, key=film_key(name, int(year)), name=name, year=int(year), lid=r.get("lid") or "", group=r.get("group"),
                tmdb=int(r["tmdb_id"]), runtime=int(r.get("runtime") or 0), lang=r.get("original_language") or "",
                codes=r.get("production_countries") or [], code_names=r.get("production_country_names") or [],
                directors=r.get("directors") or [], composers=r.get("composers") or [], avg=r.get("lb_avg"),
                hist=round(h["plain_mean"], 4) if h and h.get("plain_mean") else None,
                count=r.get("lb_rating_count") or (h["total"] if h else None))
            prev = seen_slug.get(slug)
            if prev is None or (prev["group"] == "own" and rec["group"] != "own"):
                seen_slug[slug] = rec   # one record per film; the shared list wins over the author's own copy
    # collisions: different films (different slugs) with the same key, counting the author's own films as evidence
    by_key = defaultdict(list)
    for rec in seen_slug.values():
        by_key[rec["key"]].append(rec)
    ambiguous = {k for k, v in by_key.items() if len(v) > 1}
    for k in sorted(ambiguous):
        shared = sorted(r["slug"] for r in by_key[k] if r["group"] != "own")
        log["collisions"].append(dict(key=k, films=len(by_key[k]), shared_list_slugs=shared))
    films = []
    for rec in seen_slug.values():
        if rec["key"] in ambiguous:
            continue
        if rec["group"] == "own" and not include_own:
            log["own_only_left_out"] += 1
            continue
        films.append(rec)
    films.sort(key=lambda r: r["key"])
    return films, log
