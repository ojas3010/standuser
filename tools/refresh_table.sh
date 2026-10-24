#!/usr/bin/env bash
# Rebuild the film table from collector output, then rebuild the site and the CLI and run the tests.
#   tools/refresh_table.sh ~/lb_collect_out
set -euo pipefail
cd "$(dirname "$0")/.."
[ $# -ge 1 ] || { echo "usage: $0 <collector out dir>..."; exit 2; }
python3 -I tools/build_table.py "$@" --out crates/standuser-core/data/films.json
tools/build_site.sh >/dev/null
cargo build --release -p standuser
cargo test --workspace
