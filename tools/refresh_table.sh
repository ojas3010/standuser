#!/usr/bin/env bash
# Rebuild the film table from collector output, RECALIBRATE the cut-points on it, then rebuild the site and the CLI
# and run the tests. TMDB's terms allow caching its data for at most six months, so run this before the table's build
# date is six months old (the CLI and the page warn once it is).
#
#   tools/refresh_table.sh ~/lb_collect_out [more collector dirs]
#
# Needs a Python with numpy, pandas and pyarrow (PYTHON=...), and the pooled-user data for the calibration
# (POOL=<sdl60660/letterboxd_recommendations>/data_processing, default ~/samlearner/repo/data_processing).
# Without the pooled data the script stops: a new table with the old cut-points would be graded against a different
# film set. SKIP_CALIBRATION=1 overrides that on purpose and prints a warning.
set -euo pipefail
cd "$(dirname "$0")/.."
[ $# -ge 1 ] || { echo "usage: $0 <collector out dir>..."; exit 2; }
PY=${PYTHON:-python3}
POOL=${POOL:-$HOME/samlearner/repo/data_processing}
python3 -I tools/build_table.py "$@" --out crates/standuser-core/data/films.json
if [ "${SKIP_CALIBRATION:-0}" = "1" ]; then
  echo "WARNING: calibration skipped. The cut-points in cuts.json still come from the previous table." >&2
elif [ -d "$POOL/data/training_data_samples" ]; then
  collect=(); for d in "$@"; do collect+=(--collect "$d"); done
  "$PY" -I tools/calibrate.py --pool "$POOL" "${collect[@]}" --out tools/calibration.json
  python3 -I tools/build_tables.py
  PYTHON="$PY" tools/golden/regenerate.sh synth
else
  echo "pooled-user data not found at $POOL; set POOL=... or SKIP_CALIBRATION=1" >&2
  exit 1
fi
tools/build_site.sh >/dev/null
cargo build --release -p standuser
cargo test --workspace
