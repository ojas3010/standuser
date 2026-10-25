#!/usr/bin/env bash
# Browser test for the static page, in headless Chrome or Edge (no Node needed).
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
cp "$FIX/table.json" "$W/films.json"   # the page is tested against the synthetic table and its synthetic name candidates
cp "$FIX/names.json" "$W/names.json"
# malformed input the page must survive: a corrupt file, and a valid zip with a non-ASCII date and a NaN rating
python3 - "$W" <<'PY'
import sys, zipfile
w = sys.argv[1]
open(f"{w}/bad.zip", "wb").write(b"this is not a zip file")
with zipfile.ZipFile(f"{w}/weird.zip", "w") as z:
    z.writestr("watched.csv", "Date,Name,Year,Letterboxd URI\n2022-07-0\u00e9,A,2000,u\n" + "".join(f"2024-{1 + i % 12:02d}-{3 + i:02d},F{i},2001,u\n" for i in range(15)))
    z.writestr("ratings.csv", "Date,Name,Year,Letterboxd URI,Rating\n2024-01-01,A,2000,u,NaN\n2024-01-01,F1,2001,u,4\n")
PY
cp tools/site_test/selftest.js "$W/"
sed 's#</body>#<script type="module" src="selftest.js"></script></body>#' site/index.html > "$W/test.html"
(cd "$W" && exec python3 -m http.server 8765 >/dev/null 2>&1) & SRV=$!
sleep 1
for page in test; do
  timeout 90 "$CHROME" --headless=new --disable-gpu --no-first-run --user-data-dir="$PROFILE$page" --virtual-time-budget=20000 \
    --dump-dom "http://localhost:8765/$page.html" > "$W/$page.dom" 2>/dev/null || true
  echo "== $page"
  sed -n '/<pre id="\(selftest\|out\)">/,/<\/pre>/p' "$W/$page.dom" | sed 's/<[^>]*>//g' | tee "$W/$page.out"
  # every line is asserted by selftest.js; a missing report, a FAIL or an ERROR fails the run
  grep -q '\[ok\]' "$W/$page.out" || { echo "no test report"; exit 1; }
  if grep -q 'FAIL\|^ERROR' "$W/$page.out"; then echo "site test FAILED"; exit 1; fi
done
echo "site test passed"
