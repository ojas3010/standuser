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
# The page runs under prefers-reduced-motion (--force-prefers-reduced-motion), so the reveal and the flash add no time to the test. The
# virtual-time budget is 60 s. Chrome's virtual clock sometimes jumps 30 to 120 s while a Blob is read back (File.arrayBuffer(), a canvas
# PNG): about one run in ten, on any page. selftest.js reads its PNG through an object URL, which is steady, but the page's own read of a
# dropped file cannot be changed for a test. A run whose clock ran out (no report, or a "timed out waiting" error) says nothing about the
# page, so it is run again, up to 4 times with the same budget; a FAIL or any other ERROR is never run again.
for page in test; do
  for attempt in 1 2 3 4; do
    timeout 90 "$CHROME" --headless=new --disable-gpu --no-first-run --user-data-dir="$PROFILE$page$attempt" --virtual-time-budget=60000 --force-prefers-reduced-motion \
      --dump-dom "http://localhost:8765/$page.html" > "$W/$page.dom" 2>/dev/null || true
    sed -n '/<pre id="\(selftest\|out\)">/,/<\/pre>/p' "$W/$page.dom" | sed 's/<[^>]*>//g' > "$W/$page.out"
    if grep -q '\[ok\]' "$W/$page.out" && ! grep -q '^ERROR: Error: timed out waiting' "$W/$page.out"; then break; fi
    echo "the virtual clock ran out (run $attempt of 4)"
  done
  echo "== $page"
  cat "$W/$page.out"
  # every line is asserted by selftest.js; a missing report, a FAIL or an ERROR fails the run
  grep -q '\[ok\]' "$W/$page.out" || { echo "no test report"; exit 1; }
  if grep -q 'FAIL\|^ERROR' "$W/$page.out"; then echo "site test FAILED"; exit 1; fi
done
echo "site test passed"
