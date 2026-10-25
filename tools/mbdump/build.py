"""Build Soundtrack-only Parquet from the extracted dump. Usage: python3 -I tools/mbdump/build.py DUMPDIR PARQUETDIR"""
import sys, time, duckdb
D, P = sys.argv[1], sys.argv[2]
COLS = {
 "release_group": "id,gid,name,artist_credit,type,comment,edits_pending,last_updated",
 "release_group_secondary_type": "id,name,parent,child_order,description,gid",
 "release_group_secondary_type_join": "release_group,secondary_type,created",
 "release_group_meta": "id,release_count,first_release_date_year,first_release_date_month,first_release_date_day,rating,rating_count",
 "artist_credit": "id,name,artist_count,ref_count,created,edits_pending,gid",
 "artist_credit_name": "artist_credit,position,artist,name,join_phrase",
 "artist": "id,gid,name,sort_name,begin_date_year,begin_date_month,begin_date_day,end_date_year,end_date_month,end_date_day,type,area,gender,comment,edits_pending,last_updated,ended,begin_area,end_area",
 "release": "id,gid,name,artist_credit,release_group,status,packaging,language,script,barcode,comment,edits_pending,quality,last_updated",
 "release_status": "id,name,parent,child_order,description,gid",
 "medium": "id,release,position,format,name,edits_pending,last_updated,track_count,gid",
 "track": "id,gid,recording,medium,position,number,name,artist_credit,length,edits_pending,last_updated,is_data_track",
}
con = duckdb.connect(); con.execute("set threads=8; set memory_limit='12GB'")
def src(t):
    cols = COLS[t].split(",")
    spec = ", ".join(f"'{c}': 'VARCHAR'" for c in cols)
    return (f"read_csv('{D}/mbdump/{t}', delim='\t', quote='', escape='', header=false, auto_detect=false, "
            f"columns={{{spec}}}, nullstr='\\N', strict_mode=false)")
def step(name, sql):
    t = time.time(); con.execute(sql); print(f"{name}: {time.time()-t:.1f}s", flush=True)

step("rg_soundtrack", f"""create table rg as
 select rg.id::int id, rg.gid::uuid gid, rg.name title, m.first_release_date_year::int "year", rg.artist_credit::int ac
 from {src('release_group')} rg
 join {src('release_group_secondary_type_join')} j on j.release_group = rg.id
 join {src('release_group_secondary_type')} st on st.id = j.secondary_type and st.name = 'Soundtrack'
 left join {src('release_group_meta')} m on m.id = rg.id""")
step("rg_parquet", f"""copy (
 select rg.gid::varchar rg_gid, rg.id rg_id, rg.title, rg.year, ac.name artist_credit,
        (select list(a.name order by acn.position) from {src('artist_credit_name')} acn join {src('artist')} a on a.id = acn.artist
          where acn.artist_credit = rg.ac::varchar) artist_names
 from rg join {src('artist_credit')} ac on ac.id = rg.ac::varchar
) to '{P}/release_groups.parquet' (format parquet, compression zstd)""")
step("one_release", f"""create table rel as
 select * from (
  select r.id::int rid, r.gid::varchar rgid_release, r.release_group::int rg_id, rs.name status,
         row_number() over (partition by r.release_group order by (rs.name = 'Official') desc, r.id::int) rn
  from {src('release')} r join rg on rg.id = r.release_group::int
  left join {src('release_status')} rs on rs.id = r.status) where rn = 1""")
step("tracks_parquet", f"""copy (
 select rg.gid::varchar rg_gid, rel.rgid_release release_gid, md.position::int medium_position, t.position::int position,
        t.name title, t.length::int length_ms
 from rel join rg on rg.id = rel.rg_id
 join {src('medium')} md on md.release = rel.rid::varchar
 join {src('track')} t on t.medium = md.id
) to '{P}/tracks.parquet' (format parquet, compression zstd)""")
step("releases_parquet", f"copy (select rg.gid::varchar rg_gid, rel.rgid_release release_gid, rel.status from rel join rg on rg.id = rel.rg_id) to '{P}/releases.parquet' (format parquet, compression zstd)")
for t in ("release_groups", "releases", "tracks"):
    print(t, con.execute(f"select count(*) from read_parquet('{P}/{t}.parquet')").fetchone()[0])
