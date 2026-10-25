"""Match films against the local MusicBrainz Soundtrack Parquet (see ~/mbdump). Read-only; changes no product code.

Usage: python3 -I tools/mbdump/mb_local_match.py [--parquet ~/mbdump/parquet] [--out ~/mbdump/local_match.jsonl]
           [--jsonl ~/lb_collect_out/mb_tracks.jsonl]            films (titles, composers, year) taken from the API collector's output
           [--films site/films.json --titles titles.json ...]    or every film in the table, titles from TMDB caches (fetch_titles.py)
Per film: normalised-title match (title_forms from mb_probe), first-release year within 1, composer flag
(composer / various / other, as in mb_collect.py), then a fuzzy fallback (rapidfuzz, >= FUZZ) on the title.
"""
import argparse, json, re, unicodedata
from collections import defaultdict
from pathlib import Path

FUZZ = 92
MIN_LOSSY = 6  # a title that lost letters in ASCII folding ("秒速5センチメートル" -> "5") needs a longer residue to count

def norm(s):
    s = unicodedata.normalize("NFKD", s).encode("ascii", "ignore").decode().lower()
    return re.sub(r"[^a-z0-9]", "", s)

SUFFIX = re.compile(r"[\(\[][^\)\]]*(soundtrack|score|music|ost|motion picture|film)[^\)\]]*[\)\]]|[:\-–]\s*(original\s+)?(motion picture\s+)?(soundtrack|score|music).*$", re.I)

def title_forms(t):
    out = {norm(t), norm(SUFFIX.sub("", t))}
    out.discard("")
    return out

def artist_ok(credit, composers):
    c = norm(credit)
    return any(norm(p) and (norm(p) in c or c in norm(p) and len(c) > 3) for p in composers)

def lossy(t):
    """True if NFKD folding to ASCII dropped letters or digits (CJK, Cyrillic, ...). 8½ -> 8 1 2 is not lossy."""
    return any(c.isalnum() and not c.isascii() for c in unicodedata.normalize("NFKD", t))

def numbers(t):
    return set(re.findall(r"\d+", unicodedata.normalize("NFKD", t).encode("ascii", "ignore").decode()))

def safe_forms(t):
    fs = title_forms(t)
    if lossy(t): fs = {f for f in fs if len(f) >= MIN_LOSSY}
    return fs

def flag_of(credit, comps):
    if norm(credit) == "variousartists": return "various"
    return "composer" if comps and artist_ok(credit, comps) else "other"

def load_groups(pq):
    import duckdb
    con = duckdb.connect()
    rows = con.execute(f"select rg_gid, title, year, artist_credit from read_parquet('{pq}/release_groups.parquet') where year is not null").fetchall()
    by_form, fuzz_pool = defaultdict(list), []
    for gid, title, year, credit in rows:
        g = dict(id=gid, title=title, year=year, artist=credit)
        for f in safe_forms(title): by_form[f].append(g)
        if safe_forms(title): fuzz_pool.append((norm(title), g))
    return con, by_form, fuzz_pool

def tracks_of(con, pq, gid):
    r = con.execute(f"select title from read_parquet('{pq}/tracks.parquet') where rg_gid = ? order by medium_position, position", [gid]).fetchall()
    return [t[0] for t in r]

def match_film(rec, by_form, fuzz_pool, choices):
    from rapidfuzz import process, fuzz
    year, comps = rec["year"], rec["composers"]
    want = set().union(*(safe_forms(t) for t in rec["titles"])) if rec["titles"] else set()
    tn = {norm(t) for t in rec["titles"] if not lossy(t)}
    seen, cands = set(), []
    for w in want:
        for g in by_form.get(w, []):
            if g["id"] in seen or abs(g["year"] - year) > 1: continue
            seen.add(g["id"])
            cands.append(dict(g, exact=norm(g["title"]) in tn, via="norm"))
    if not cands:  # fuzzy fallback, year gate still applies
        for t in rec["titles"]:
            for w in safe_forms(t):
                for _, score, idx in process.extract(w, choices, scorer=fuzz.ratio, score_cutoff=FUZZ, limit=10):
                    g = fuzz_pool[idx][1]
                    if g["id"] in seen or abs(g["year"] - year) > 1: continue
                    if numbers(g["title"]) != numbers(t): continue  # "Brother 2" is not "Brother"
                    seen.add(g["id"])
                    cands.append(dict(g, exact=False, via=f"fuzzy{score:.0f}"))
    for c in cands: c["flag"] = flag_of(c["artist"], comps)
    best = {}
    for flag in ("composer", "various", "other"):
        cs = [c for c in cands if c["flag"] == flag]
        if cs: best[flag] = min(cs, key=lambda c: (not c["exact"], abs(c["year"] - year)))
    return cands, best

def records(a):
    if a.films:
        d = json.load(open(a.films)); people = d["people"]; titles = {}
        for p in a.titles: titles.update(json.load(open(p)))
        for f in d["films"]:
            tt = [t for t in titles.get(str(f[2]), []) if t]
            yield dict(key=f[0], year=f[3], titles=tt, composers=[people[c] for c in f[8].split(",") if c])
        return
    for line in open(a.jsonl):
        try: yield json.loads(line)
        except ValueError: continue  # torn last line while the collector is writing

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--parquet", default=str(Path.home() / "mbdump" / "parquet"))
    ap.add_argument("--jsonl", default=str(Path.home() / "lb_collect_out" / "mb_tracks.jsonl"))
    ap.add_argument("--out", default=str(Path.home() / "mbdump" / "local_match.jsonl"))
    ap.add_argument("--films"); ap.add_argument("--titles", nargs="*", default=[])
    a = ap.parse_args()
    con, by_form, fuzz_pool = load_groups(a.parquet)
    choices = [p[0] for p in fuzz_pool]
    n = 0
    with open(a.out, "w") as out:
        for rec in records(a):
            cands, best = match_film(rec, by_form, fuzz_pool, choices)
            loc = {fl: dict(rg=c["id"], title=c["title"], year=c["year"], artist=c["artist"], via=c["via"],
                            tracks=tracks_of(con, a.parquet, c["id"])) for fl, c in best.items()}
            out.write(json.dumps(dict(key=rec["key"], local=loc, n_cands=len(cands)), ensure_ascii=False) + "\n"); n += 1
    print(f"{n} films -> {a.out}")

if __name__ == "__main__":
    main()
