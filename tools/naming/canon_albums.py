"""Resolve the canon Stands named after songs to the album the song is on, using the local MusicBrainz dump.

Usage: python -I tools/naming/canon_albums.py CANON_CSV DUMPDIR OUT_JSON
Rule: among the artist's Album release groups with no Compilation/Live/Remix/Demo/... secondary type and a first release
year from the song's year - 1 to + 3 (the CSV year, which research_notes marks unverified for 70 rows), take the earliest
(first release year, then lowest id) that has an Official release with a track of the song's title; the tracklist is the
smallest such release (the original edition rather than a bonus-track reissue; ties to the lowest id), discs in order, and
it needs at least 4 tracks. Rows that do not resolve are listed, never guessed.
"""
import csv, json, re, sys, unicodedata
import duckdb

CSV_PATH, D, OUT = sys.argv[1:4]
BAD_SECONDARY = {"Compilation", "Live", "Remix", "DJ-mix", "Mixtape/Street", "Demo", "Audiobook", "Spokenword", "Interview",
                 "Audio drama", "Field recording"}


def fold(s):
    s = unicodedata.normalize("NFKD", s)
    s = "".join(c for c in s if not unicodedata.combining(c)).lower().replace("&", " and ")
    return re.sub(r"[^a-z0-9]+", " ", s).strip()


def artist_variants(a):
    a = re.sub(r"\(.*?\)", "", a).split(";")[0].strip()
    f = fold(a)
    v = {f, re.sub(r"^the ", "", f), "the " + re.sub(r"^the ", "", f)}
    return {x for x in v if x}


def title_key(t):
    t = re.sub(r"[\(\[].*?[\)\]]", " ", t)
    return fold(t)


COLS = {
    "release_group": "id,gid,name,artist_credit,type,comment,edits_pending,last_updated",
    "release_group_secondary_type": "id,name,parent,child_order,description,gid",
    "release_group_secondary_type_join": "release_group,secondary_type,created",
    "release_group_meta": "id,release_count,first_release_date_year,first_release_date_month,first_release_date_day,rating,rating_count",
    "artist_credit": "id,name,artist_count,ref_count,created,edits_pending,gid",
    "release": "id,gid,name,artist_credit,release_group,status,packaging,language,script,barcode,comment,edits_pending,quality,last_updated",
    "release_status": "id,name,parent,child_order,description,gid",
    "medium": "id,release,position,format,name,edits_pending,last_updated,track_count,gid",
    "track": "id,gid,recording,medium,position,number,name,artist_credit,length,edits_pending,last_updated,is_data_track",
}


def src(t):
    spec = ", ".join(f"'{c}': 'VARCHAR'" for c in COLS[t].split(","))
    return (f"read_csv('{D}/mbdump/{t}', delim='\t', quote='', escape='', header=false, auto_detect=false, "
            f"columns={{{spec}}}, nullstr='\\N', strict_mode=false)")


rows = [r for r in csv.DictReader(open(CSV_PATH, encoding="utf-8")) if r["row_kind"] == "base" and r["source_type"] == "song"]
targets = []
for r in rows:
    yr = int(r["year"]) if r["year"].isdigit() else None
    targets.append({"stand": r["stand"], "part": r["part"], "song": r["source_title"], "artist": r["artist"], "year": yr,
                    "modification": r["modification"], "variants": sorted(artist_variants(r["artist"])), "tkey": title_key(r["source_title"])})
windows = [(v, t["year"] - 1, t["year"] + 3) for t in targets if t["year"] for v in t["variants"]]

con = duckdb.connect()
con.execute("set threads=8; set memory_limit='12GB'")
con.execute("create table win(v varchar, lo int, hi int)")
con.executemany("insert into win values (?, ?, ?)", windows)
con.execute(f"""create table ac as select a.id, trim(lower(regexp_replace(replace(a.name, '&', ' and '), '[^A-Za-z0-9]+', ' ', 'g'))) k
 from {src('artist_credit')} a where trim(lower(regexp_replace(replace(a.name, '&', ' and '), '[^A-Za-z0-9]+', ' ', 'g'))) in (select v from win)""")
print("credits", con.execute("select count(*) from ac").fetchone())
con.execute(f"""create table rg0 as
 select g.id::int id, g.name, trim(ac.k) k, coalesce(m.first_release_date_year::int, 9999) y
 from {src('release_group')} g join ac on ac.id = g.artist_credit
 left join {src('release_group_meta')} m on m.id = g.id
 where g.type = '1'
   and not exists (select 1 from {src('release_group_secondary_type_join')} j join {src('release_group_secondary_type')} s on s.id = j.secondary_type
                   where j.release_group = g.id and s.name in ({",".join("'%s'" % b for b in BAD_SECONDARY)}))""")
con.execute("create table rg as select distinct r.* from rg0 r join win w on w.v = r.k and r.y between w.lo and w.hi")
print("album groups", con.execute("select count(*) from rg").fetchone())
con.execute(f"""create table rel as
  select r.id::int rid, r.release_group::int rg
  from {src('release')} r join rg on rg.id = r.release_group::int join {src('release_status')} rs on rs.id = r.status and rs.name = 'Official'""")
con.execute(f"""create table trk as
 select rel.rg, rel.rid, md.position::int disc, t.position::int pos, t.name title
 from rel join {src('medium')} md on md.release = rel.rid::varchar join {src('track')} t on t.medium = md.id""")
print("tracks", con.execute("select count(*) from trk").fetchone())
rg = {r[0]: r for r in con.execute("select id, name, k, y from rg").fetchall()}
rels = {}
for g, rid, disc, pos, title in con.execute("select rg, rid, disc, pos, title from trk order by rg, rid, disc, pos").fetchall():
    rels.setdefault(g, {}).setdefault(rid, []).append(title)

out, miss = [], []
for t in targets:
    best = None
    for gid, (_, name, k, y) in rg.items():
        if k not in t["variants"] or not t["year"] or not (t["year"] - 1 <= y <= t["year"] + 3):
            continue
        hit = []
        for rid, trs in rels.get(gid, {}).items():
            for tr in trs:
                tk = title_key(tr)
                if tk == t["tkey"] or (tk.startswith(t["tkey"] + " ") and len(t["tkey"]) >= 5):
                    if len(trs) >= 4:
                        hit.append((len(trs), rid, tr))
                    break
        if hit:
            n, rid, tr = min(hit)
            cand = (y, gid, name, rid, tr)
            if best is None or cand < best:
                best = cand
    if best is None:
        miss.append(f'{t["part"]} {t["stand"]} ({t["artist"]}: {t["song"]})')
        continue
    y, gid, name, rid, tr = best
    out.append({"stand": t["stand"], "part": t["part"], "song": t["song"], "artist": t["artist"], "modification": t["modification"],
                "rg_id": gid, "album": name, "album_year": y, "source_track": tr, "tracks": rels[gid][rid]})
json.dump({"resolved": out, "unresolved": miss}, open(OUT, "w"), ensure_ascii=False, indent=1)
print(len(out), "resolved,", len(miss), "unresolved")
for o in out:
    print(f'{o["part"]:>8} {o["stand"][:26]:26} -> {o["album"][:34]:34} {o["album_year"]} ({len(o["tracks"])} tr)')
print("UNRESOLVED:", *miss, sep="\n  ")
