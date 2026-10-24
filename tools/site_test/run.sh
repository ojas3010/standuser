#!/usr/bin/env bash
# Browser tests for the static page and the Vercel function, in headless Chrome or Edge (no Node needed).
#   tools/site_test/run.sh            # needs site/pkg built (tools/build_site.sh)
# CHROME can point at any Chromium-based browser; the default is the Windows Chrome seen from WSL.
set -euo pipefail
cd "$(dirname "$0")/../.."
CHROME=${CHROME:-"/mnt/c/Program Files/Google/Chrome/Application/chrome.exe"}
PROFILE=${PROFILE:-'C:\Users\Public\standuser_chrome_'$RANDOM}
W=$(mktemp -d)
trap 'kill $SRV 2>/dev/null || true; rm -rf "$W"' EXIT
cp -r site/. "$W/"
FIX=crates/standuser-core/tests/fixtures/synth
cp "$FIX/letterboxd-synthuser-2026-06-30-12-00-utc.zip" "$W/synth.zip"
cp "$FIX/table.json" "$W/films.json"          # the page is tested against the synthetic table
cp api/tmdb.js "$W/tmdb_fn.js"
cp tools/site_test/selftest.js tools/site_test/fn.html "$W/"
sed 's#</body>#<script type="module" src="selftest.js"></script></body>#' site/index.html > "$W/test.html"
ROOT=$PWD; (cd "$W" && exec python3 "$ROOT/tools/site_test/server.py" 8765 >/dev/null 2>&1) & SRV=$!
sleep 1
for page in test fn; do
  timeout 90 "$CHROME" --headless=new --disable-gpu --no-first-run --user-data-dir="$PROFILE$page" --virtual-time-budget=20000 \
    --dump-dom "http://localhost:8765/$page.html" > "$W/$page.dom" 2>/dev/null || true
  echo "== $page"
  sed -n '/<pre id="\(selftest\|out\)">/,/<\/pre>/p' "$W/$page.dom" | sed 's/<[^>]*>//g'
done
