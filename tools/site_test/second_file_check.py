"""Two files in a row, with motion on (headless Chromium, Playwright; the same setup as record_reveal.py). run.sh runs with reduced motion, which
skips the trace, the To Be Continued arrow and the halftone freeze, so it cannot see whether they show. This does: for each of the two files it
takes the size of the trace, the arrow (its box, its words) and the freeze at the moment each is on, and fails if any has no size or is not whole in
the window. A zone that is hidden (display: none, or inside a hidden parent) has size 0, which is what the second file used to get; with the zone
shown but under the card, below the fold, the arrow is there and nobody sees it.
Usage: python tools/site_test/second_file_check.py [zip]     (default zip: the synthetic export in crates/standuser-core/tests/fixtures/synth)
Serves site/ on a local port; exits 1 on any failure."""
import functools, http.server, shutil, sys, tempfile, threading
from pathlib import Path
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[2]
ZIP = ROOT / "crates/standuser-core/tests/fixtures/synth/letterboxd-synthuser-2026-06-30-12-00-utc.zip"
SELECTORS = {"trace": "#trace", "trace line": "#trace rect", "arrow box": "#tbc", "arrow": "#tbc .arrow", "words": "#tbc .words", "freeze": "#freeze"}

# Installed before a file is dropped. When #drop gets the class `tracing` (the line travels the border) and `reading` (the arrow, the freeze) the
# sizes are read at once, in the mutation callback, so they are those of the frame that is about to paint.
OBSERVE = """(sel) => {
  window.__seen = {};
  const read = (phase) => {
    const o = {};
    for (const [k, s] of Object.entries(sel)) {
      const e = document.querySelector(s), r = e.getBoundingClientRect();
      o[k] = { w: Math.round(r.width), h: Math.round(r.height), display: getComputedStyle(e).display, inView: r.top >= 0 && r.bottom <= innerHeight && r.left >= 0 && r.right <= innerWidth };
    }
    window.__seen[phase] = o;
  };
  const drop = document.getElementById('drop');
  new MutationObserver(() => {
    for (const phase of ['tracing', 'reading']) if (drop.classList.contains(phase) && !window.__seen[phase]) read(phase);
  }).observe(drop, { attributes: true, attributeFilter: ['class'] });
}"""


def serve():
    class Quiet(http.server.SimpleHTTPRequestHandler):
        def log_message(self, *a):
            pass
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(Quiet, directory=str(ROOT / "site")))
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv


def play(page, z):
    """Drops z and waits until the card is shown and the reveal is over; returns {phase: {element: size}} and the ms from the drop to the card."""
    page.evaluate(OBSERVE, SELECTORS)
    page.evaluate("window.__t0 = performance.now()")
    page.set_input_files("#file", str(z))
    # the card of the first file is still on show, so "the card is shown" is true at once: wait for this file to be taken in, then for it to be done
    page.wait_for_function("document.getElementById('status').textContent === 'Reading the export…'", timeout=5000)
    page.wait_for_function("document.getElementById('status').textContent === '' && !document.getElementById('result').hidden", timeout=15000)
    ms = page.evaluate("Math.round(performance.now() - window.__t0)")
    page.wait_for_function("!document.getElementById('cardlive')", timeout=5000)
    return page.evaluate("window.__seen"), ms


def main():
    z = Path(sys.argv[1]) if len(sys.argv) > 1 else ZIP
    srv = serve()
    fails = []
    with sync_playwright() as p:
        browser = p.chromium.launch()
        ctx = browser.new_context(viewport={"width": 1280, "height": 720}, reduced_motion="no-preference")
        page = ctx.new_page()
        page.goto(f"http://127.0.0.1:{srv.server_port}/index.html")
        page.wait_for_function("document.getElementById('status').textContent === '' && !document.getElementById('drop').classList.contains('loading')")
        tmp = Path(tempfile.mkdtemp())
        # two paths: Chrome fires no change event for the same file picked again (the page clears the input for that, selftest.js checks it)
        files = {1: z, 2: Path(shutil.copy(z, tmp / "second.zip"))}
        for n in (1, 2):
            seen, ms = play(page, files[n])
            print(f"file {n}: card shown {ms} ms after the drop")
            for phase, names in (("tracing", ["trace", "trace line"]), ("reading", ["arrow box", "arrow", "words", "freeze"])):
                if phase not in seen:
                    fails.append(f"file {n}: the zone never got the class {phase}")
                    continue
                for k in names:
                    s = seen[phase][k]
                    # the arrow itself slides in from the left (tbcIn, 700 ms), so at the first frame it is out of the window; its box and its words are not
                    ok = s["w"] > 0 and s["h"] > 0 and s["display"] != "none" and (s["inView"] or k == "arrow")
                    print(f"  {phase:8} {k:10} {s['w']} x {s['h']}  display {s['display']:6} whole in the window: {s['inView']}  {'ok' if ok else 'FAIL'}")
                    if not ok:
                        fails.append(f"file {n}: {k} while {phase} has size {s['w']} x {s['h']} (display {s['display']}, whole in the window: {s['inView']})")
            if n == 1:
                page.wait_for_timeout(300)
        browser.close()
        shutil.rmtree(tmp)
    if fails:
        print("\nFAILED:\n" + "\n".join(fails))
        sys.exit(1)
    print("\nthe trace, the arrow and the freeze have a size for both files")


main()
