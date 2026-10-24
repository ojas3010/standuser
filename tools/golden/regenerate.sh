#!/usr/bin/env bash
# Regenerate the golden fixtures from the Python reference (research prototypes in tools/golden/proto).
# Needs Python 3 with numpy:  python3 -m venv .venv && .venv/bin/pip install numpy
# profile.csv is never opened: pass the join day on the command line.
#
#   tools/golden/regenerate.sh synth
#   tools/golden/regenerate.sh ojas <export_dir_or_zip> <table.json> <joined YYYY-MM-DD> <as_of YYYY-MM-DD> <username>
set -euo pipefail
cd "$(dirname "$0")/../.."
PY=${PYTHON:-python3}
case "${1:-}" in
  synth)
    $PY -I tools/golden/make_synth.py
    $PY -I tools/golden/ref.py crates/standuser-core/tests/fixtures/synth/files --table crates/standuser-core/tests/fixtures/synth/table.json \
      --joined 2023-01-05 --as-of 2026-06-30 --username synthuser --out crates/standuser-core/tests/fixtures/synth/golden.json ;;
  ojas)
    mkdir -p crates/standuser-core/tests/fixtures/ojas
    $PY -I tools/golden/ref.py "$2" --table "$3" --joined "$4" --as-of "$5" --username "$6" --out crates/standuser-core/tests/fixtures/ojas/golden.json ;;
  *) echo "usage: $0 synth | ojas <export> <table.json> <joined> <as_of> <username>"; exit 2 ;;
esac
