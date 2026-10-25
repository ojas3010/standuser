# Local MusicBrainz soundtrack match

Offline replacement for the rate-limited MusicBrainz web-service search in `tools/probe/mb_collect.py`.
Everything is built under a work directory (default `~/mbdump`; keep it on the Linux filesystem, not `/mnt/c`).
Nothing here touches product code.

## Rebuild

1. Tools: `lbzip2` on PATH (`apt install lbzip2`, or `apt-get download lbzip2 && dpkg -x` into a local prefix without root), and
   `python3 -m venv ~/mbdump/venv && ~/mbdump/venv/bin/pip install duckdb rapidfuzz`.
2. Download and extract (about 10 minutes, 11 GB on disk; no sign-in needed):
   `tools/mbdump/fetch.sh` — streams `mbdump.tar.bz2` of the `LATEST` export through lbzip2 and keeps only
   `SCHEMA_SEQUENCE TIMESTAMP release_group release_group_secondary_type release_group_secondary_type_join artist_credit
   artist_credit_name artist release release_status medium track`. `release_group_meta` (first release year) is only in
   `mbdump-derived.tar.bz2`, which the script also streams.
3. Check the schema: `SCHEMA_SEQUENCE` must equal `ACTIVE_SCHEMA_SEQUENCE` in `lib/DBDefs/Default.pm` at the musicbrainz-server tag
   you take `admin/sql/CreateTables.sql` from, and the `COLS` lists in `build.py` must match that file's column order
   (the dump files have no header). Last built: schema 31, tag `v-2026-09-21.0`, export `20261024-102842`.
4. Parquet: `~/mbdump/venv/bin/python -I tools/mbdump/build.py ~/mbdump/dump ~/mbdump/parquet` (under a minute) writes
   `release_groups`, `releases` (one per group: Official first, then lowest id) and `tracks` for Soundtrack release groups only.
5. Match: titles come from TMDB, so first
   `TMDB_KEY=... python3 -I tools/mbdump/fetch_titles.py site/films.json ~/mbdump/titles_extra.json <existing titles.json>`, then
   `~/mbdump/venv/bin/python -I tools/mbdump/mb_local_match.py --films site/films.json --titles <titles.json> ~/mbdump/titles_extra.json --out ~/lb_collect_out/mb_local.jsonl`.
   Without `--films` it reads the API collector's `mb_tracks.jsonl` instead.
6. Compare with the API results and get coverage: `python3 -I tools/mbdump/cmp.py mb_tracks.jsonl mb_local.jsonl [film key ...]`.

## Matching rules

Normalised title (NFKD, ASCII-folded, soundtrack/score suffix stripped), first release year within 1 of the film,
flag `composer` / `various` / `other` from the credit as in `mb_collect.py`, fuzzy fallback (ratio >= 92) only when
nothing matches exactly and only if the numbers in the two titles are equal. Titles that lose letters in ASCII folding
(CJK, Cyrillic) need a 6+ character residue, otherwise `秒速5センチメートル` would match every group titled "5".
