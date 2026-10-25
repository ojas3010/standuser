#!/bin/bash
# Stream the latest MusicBrainz full export through lbzip2 and keep only the tables the soundtrack pipeline needs.
# Usage: tools/mbdump/fetch.sh [workdir]   (default ~/mbdump; keep it off /mnt/c). lbzip2 must be on PATH.
set -euo pipefail
W=${1:-$HOME/mbdump}; B=https://data.metabrainz.org/pub/musicbrainz/data/fullexport
V=$(curl -sf $B/LATEST); mkdir -p "$W/dump" "$W/parquet"; cd "$W/dump"
M=mbdump
echo "export $V"; s=$(date +%s)
curl -sf $B/$V/mbdump.tar.bz2 | lbzip2 -dc | tar -x --wildcards -f - SCHEMA_SEQUENCE TIMESTAMP \
  $M/release_group $M/release_group_secondary_type $M/release_group_secondary_type_join \
  $M/artist_credit $M/artist_credit_name $M/artist $M/release $M/release_status $M/medium $M/track
echo "mbdump.tar.bz2: $(( $(date +%s) - s ))s"; s=$(date +%s)
# release_group_meta (first release year) is in the derived archive, not in mbdump.tar.bz2
curl -sf $B/$V/mbdump-derived.tar.bz2 | lbzip2 -dc | tar -x --wildcards -f - $M/release_group_meta
echo "mbdump-derived.tar.bz2: $(( $(date +%s) - s ))s"
echo "schema $(cat SCHEMA_SEQUENCE); compare with ACTIVE_SCHEMA_SEQUENCE in lib/DBDefs/Default.pm at the matching musicbrainz-server tag, and re-check build.py COLS against admin/sql/CreateTables.sql"
