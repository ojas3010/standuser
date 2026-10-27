"""Screen recordings of the reveal, with Playwright (headless Chromium): the idle page, a file dragged over the drop zone, the drop and the whole reveal,
at 1280 x 720 and 390 x 844. Usage: python tools/site_test/record_reveal.py <out_dir> [zip]     (default zip: the third of local/review/zips10)
Serves site/ on a local port; writes reveal-1280x720.webm and reveal-390.webm. Only the synthetic zips in local/review/zips10 are meant as input."""
import functools, http.server, sys, threading
from pathlib import Path
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[2]
SIZES = {"reveal-1280x720": (1280, 720), "reveal-390": (390, 844)}


def serve():
    class Quiet(http.server.SimpleHTTPRequestHandler):
        def log_message(self, *a):
            pass
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(Quiet, directory=str(ROOT / "site")))
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv


def main():
    out = Path(sys.argv[1]); out.mkdir(parents=True, exist_ok=True)
    zips = sorted((ROOT / "local/review/zips10").glob("*.zip"))
    z = Path(sys.argv[2]) if len(sys.argv) > 2 else zips[2]
    srv = serve()
    base = f"http://127.0.0.1:{srv.server_port}"
    with sync_playwright() as p:
        browser = p.chromium.launch()
        for name, (w, h) in SIZES.items():
            ctx = browser.new_context(viewport={"width": w, "height": h}, record_video_dir=str(out / "tmp"), record_video_size={"width": w, "height": h})
            page = ctx.new_page()
            page.goto(base + "/index.html")
            page.wait_for_function("document.getElementById('status').textContent === '' && !document.getElementById('drop').classList.contains('loading')")
            page.wait_for_timeout(1200)                                  # the idle page: the sample card's colour, dots and ゴ
            page.dispatch_event("#drop", "dragenter")                    # a file over the zone: ゴゴゴ beside it
            page.wait_for_timeout(1200)
            page.dispatch_event("#drop", "dragleave")
            page.wait_for_timeout(300)
            page.set_input_files("#file", str(z))                        # the drop: the arrow while it is read, then the reveal
            page.wait_for_function("document.getElementById('cardlive')")
            page.wait_for_function("!document.getElementById('cardlive')", timeout=5000)
            page.wait_for_timeout(1500)                                  # the end state
            video = page.video
            ctx.close()
            video.save_as(str(out / f"{name}.webm"))
        browser.close()
    for f in (out / "tmp").glob("*"):
        f.unlink()
    (out / "tmp").rmdir()


main()
