"""Design-pass screenshots and request log, with Playwright (headless Chromium).
Usage: python tools/site_test/screenshots.py <out_dir> [zip_dir]
Serves site/ on a local port, then for each width (1280, 390): one shot of the idle page and, for each synthetic zip in
zip_dir, one full-page shot after the drop. Every request the page makes is logged; the report (requests.json) lists what was
requested at load, what was requested after the drop, and the hosts. Needs: pip install playwright && playwright install chromium.
Only the synthetic zips in local/review/zips10 are meant as input.
"""
import functools, http.server, json, sys, threading
from pathlib import Path
from urllib.parse import urlparse
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[2]
WIDTHS = {"desktop": (1280, 900), "phone": (390, 844)}


def serve():
    class Quiet(http.server.SimpleHTTPRequestHandler):
        def log_message(self, *a):
            pass
    h = functools.partial(Quiet, directory=str(ROOT / "site"))
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), h)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv


def main():
    out = Path(sys.argv[1]); out.mkdir(parents=True, exist_ok=True)
    zips = sorted(Path(sys.argv[2] if len(sys.argv) > 2 else ROOT / "local/review/zips10").glob("*.zip"))
    srv = serve()
    base = f"http://127.0.0.1:{srv.server_port}"
    report = {}
    with sync_playwright() as p:
        browser = p.chromium.launch()
        for wname, (w, h) in WIDTHS.items():
            ctx = browser.new_context(viewport={"width": w, "height": h}, device_scale_factor=1, reduced_motion="no-preference")
            for i, z in enumerate(zips):
                page = ctx.new_page()
                reqs = {"load": [], "drop": []}
                phase = ["load"]
                page.on("request", lambda r: reqs[phase[0]].append(r.url))
                page.goto(base + "/index.html")
                page.wait_for_function("document.getElementById('status').textContent === '' && !document.getElementById('drop').classList.contains('loading')")
                page.wait_for_timeout(400)  # fonts settle
                if i == 0:
                    page.screenshot(path=out / f"{wname}-idle.png", full_page=True)
                phase[0] = "drop"
                page.set_input_files("#file", str(z))
                page.wait_for_function("!document.getElementById('result').hidden && document.getElementById('status').textContent === ''")
                page.evaluate("document.getElementById('card').decode()")
                page.wait_for_timeout(1500)  # reveal, hold and palette shift have finished
                page.screenshot(path=out / f"{wname}-{i}.png", full_page=True)
                report[f"{wname}-{i}"] = {"load": reqs["load"], "drop": reqs["drop"]}
                page.close()
            ctx.close()
        browser.close()
    # blob: and data: URLs are the page's own memory (the card image), not network requests
    net = lambda u: urlparse(u).scheme in ("http", "https")
    hosts = sorted({urlparse(u).netloc for r in report.values() for k in r.values() for u in k if net(u)})
    summary = {"hosts": hosts, "drop_requests": sorted({u for r in report.values() for u in r["drop"] if net(u)}),
               "load_requests": sorted({u.replace(base, "") for r in report.values() for u in r["load"]})}
    (out / "requests.json").write_text(json.dumps({"summary": summary, "pages": report}, indent=1))
    print(json.dumps(summary, indent=1))


main()
