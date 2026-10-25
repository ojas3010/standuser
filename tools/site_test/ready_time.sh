#!/usr/bin/env bash
# Time to ready under Fast 4G (see ready_time.js). Serves a copy of a site directory and measures it in the Windows Chrome seen from WSL.
#   tools/site_test/ready_time.sh <site_dir> [runs] [want_names:0|1]
set -euo pipefail
cd "$(dirname "$0")/../.."
SITE=${1:?site dir}; RUNS=${2:-3}; export WANT_NAMES=${3:-0}
CHROME=${CHROME:-"/mnt/c/Program Files/Google/Chrome/Application/chrome.exe"}
NODE=${NODE:-"/mnt/c/Program Files/nodejs/node.exe"}
W=$(mktemp -d); trap 'kill $SRV 2>/dev/null || true; rm -rf "$W"' EXIT
cp -r "$SITE"/. "$W/"
PORT=$((8800 + RANDOM % 100))
(cd "$W" && exec python3 "$OLDPWD/tools/site_test/serve_gz.py" "$PORT" >/dev/null 2>&1) & SRV=$!
sleep 1
JS=/mnt/c/Users/Public/standuser_ready_time.js
cp tools/site_test/ready_time.js "$JS"
WSLENV=WANT_NAMES "$NODE" 'C:\Users\Public\standuser_ready_time.js' 'C:\Program Files\Google\Chrome\Application\chrome.exe' "http://localhost:$PORT/index.html" "$RUNS"
rm -f "$JS"
