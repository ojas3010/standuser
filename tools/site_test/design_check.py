"""Checks docs/DESIGN.md against the live page (Playwright, headless Chromium) and prints a markdown table of rule, result, evidence.
Usage: python tools/site_test/design_check.py [zip_dir]     (synthetic zips only; default local/review/zips10)
Measured in the browser: computed styles of every element, request log, timeline of the reveal, contrast of the accent.
Items that need eyes (composition) are not here; they are marked in docs/design-pass-checklist.md from the screenshots.
"""
import functools, http.server, json, re, sys, threading
from pathlib import Path
from urllib.parse import urlparse
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[2]
SITE = ROOT / "site"
rows = []


def check(rule, ok, evidence):
    rows.append((rule, "pass" if ok else "FAIL", evidence))


def lum(rgb):
    f = lambda c: c / 12.92 if c <= 0.03928 else ((c + 0.055) / 1.055) ** 2.4
    r, g, b = [f(v / 255) for v in rgb]
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def cr(a, b):
    x, y = sorted([lum(a), lum(b)], reverse=True)
    return (x + 0.05) / (y + 0.05)


def rgb(s):
    return [int(float(v)) for v in re.findall(r"[\d.]+", s)[:3]]


def serve():
    class Quiet(http.server.SimpleHTTPRequestHandler):
        def log_message(self, *a):
            pass
    h = functools.partial(Quiet, directory=str(SITE))
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), h)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv


def load(browser, base, w, h, scheme="light", motion="no-preference"):
    ctx = browser.new_context(viewport={"width": w, "height": h}, color_scheme=scheme, reduced_motion=motion)
    page = ctx.new_page()
    reqs = {"load": [], "drop": []}
    phase = ["load"]
    page.on("request", lambda r: reqs[phase[0]].append(r.url))
    page.goto(base + "/index.html")
    page.wait_for_function("document.getElementById('status').textContent === '' && !document.getElementById('drop').classList.contains('loading')")
    page.wait_for_timeout(300)
    return ctx, page, reqs, phase


def drop(page, phase, z):
    phase[0] = "drop"
    page.set_input_files("#file", str(z))
    page.wait_for_function("!document.getElementById('result').hidden")


ALL = """() => [...document.querySelectorAll('body, body *')].map(e => { const s = getComputedStyle(e); return {
  tag: e.tagName, id: e.id, cls: e.className && e.className.baseVal === undefined ? e.className : '',
  shadow: s.boxShadow, tshadow: s.textShadow, bgimg: s.backgroundImage, filter: s.filter, transform: s.transform,
  td: s.transitionDuration, ad: s.animationDuration, font: s.fontFamily, radius: s.borderRadius, h: e.getBoundingClientRect().height,
  hidden: e.closest('[hidden]') !== null }; })"""


def secs(v):
    return max(float(x.strip().rstrip("s")) * (0.001 if x.strip().endswith("ms") else 1) for x in v.split(","))


def main():
    zips = sorted(Path(sys.argv[1] if len(sys.argv) > 1 else ROOT / "local/review/zips10").glob("*.zip"))
    srv = serve()
    base = f"http://127.0.0.1:{srv.server_port}"
    css = re.sub(r"/\*.*?\*/", "", (SITE / "style.css").read_text(), flags=re.S)   # comments are not rules
    js = (SITE / "app.js").read_text()
    html = (SITE / "index.html").read_text()
    with sync_playwright() as p:
        browser = p.chromium.launch()

        # --- requests: load and after the drop
        ctx, page, reqs, phase = load(browser, base, 1280, 900)
        drop(page, phase, zips[0])
        page.wait_for_timeout(1200)
        net = lambda u: urlparse(u).scheme in ("http", "https")
        hosts = {urlparse(u).netloc for u in reqs["load"] + reqs["drop"] if net(u)}
        check("Self-host: no request outside the site's own files at load", hosts == {urlparse(base).netloc}, f"hosts: {sorted(hosts)} (the local server stands for standuser.pages.dev)")
        check("0 requests after the drop", not [u for u in reqs["drop"] if net(u)], f"{len([u for u in reqs['drop'] if net(u)])} network requests (blob: card images are in-memory)")
        fonts = page.evaluate("[...document.fonts].map(f => f.family + ' ' + f.status)")
        check("Fonts self-hosted: Archivo Black and Archivo load from fonts/, Overpass is gone", any("Archivo Black loaded" in f for f in fonts) and any("Archivo loaded" in f for f in fonts) and "/fonts/archivo-black.woff2" in "".join(reqs["load"]) and "/fonts/archivo.woff2" in "".join(reqs["load"]) and "overpass" not in (css + html + js).lower() and not (SITE / "fonts/overpass.woff2").exists(), str(fonts))
        check("CSP allows only the site (no font or script host)", "default-src 'self'" in (SITE / "_headers").read_text() and "http" not in re.sub(r"https?://www\.themoviedb\.org", "", css + js), "default-src 'self'; no absolute URL in style.css or app.js")
        els = page.evaluate(ALL)
        vis = [e for e in els if not e["hidden"]]

        # --- rule 1 and 7: chrome text
        text = page.evaluate("document.body.innerText")
        cjk = re.findall(r"[　-ヿ一-鿿]", text)
        check("R1 chrome is quiet: no JoJo identity (no kana, kanji, 【】) in page text", not cjk, f"{len(cjk)} such characters")
        bad7 = [w for w in ("ゴ", "To Be Continued", "Menacing", "➤", "➡", "→", "▶") if w in text + html]
        check("R7 ban: no ゴゴゴ, To Be Continued arrow or Menacing overlay in the chrome", not bad7, f"found {bad7}" if bad7 else "none in text or markup")

        # --- rule 3: ground and flat fills
        bg = rgb(page.evaluate("getComputedStyle(document.body).backgroundColor"))
        check("R3 ground is neutral with zero hue: #ffffff (light)", bg == [255, 255, 255], f"rgb{tuple(bg)}")
        check("R3 ban: no gradients", not any(e["bgimg"] != "none" for e in els) and "gradient" not in css + html, "background-image none on every element; no gradient in CSS")
        check("R3 ban: no shadows or glows", not any(e["shadow"] != "none" or e["tshadow"] != "none" or "drop-shadow" in e["filter"] for e in els) and "shadow" not in css, "box-shadow, text-shadow, filter all none on every element")
        pre_ok = True
        ctxd, paged, _, _ = load(browser, base, 1280, 900, scheme="dark")
        bgd = rgb(paged.evaluate("getComputedStyle(document.body).backgroundColor"))
        check("R3 ground is neutral with zero hue: #0e0e0e (dark)", bgd == [14, 14, 14], f"rgb{tuple(bgd)}")
        ctxd.close()

        # --- rule 5: type
        fam = page.evaluate("getComputedStyle(document.body).fontFamily")
        serif_chrome = [e for e in els if "Serif" in e["font"] or "Mincho" in e["font"]]
        heads = page.evaluate("[...document.querySelectorAll('h1, h2, .drop-title')].filter(e => !e.closest('[hidden]')).map(e => getComputedStyle(e).fontFamily.split(',')[0])")
        check("R5 body is Archivo; the wordmark and headings are Archivo Black; no serif in the chrome", fam.startswith("Archivo") and heads and all(h.strip('"') == "Archivo Black" for h in heads) and not serif_chrome and "Inter" not in css, f"body font-family: {fam}; headings {sorted(set(heads))}")
        tilted = [e for e in vis if e["transform"] not in ("none", "")]
        check("R5 the wordmark has one tilt; nothing else tilts", len(tilted) == 1 and tilted[0]["tag"] == "H1", f"{len(tilted)} transformed element(s): {[t['tag'] for t in tilted]}")
        check("R6 at most one diagonal", len(tilted) <= 1, f"{len(tilted)} rotated element(s) at rest")

        # --- rule 6: order and single column
        tops = page.evaluate("""() => ['drop', 'card', 'stats'].map(id => document.getElementById(id).getBoundingClientRect().top + scrollY)""")
        check("R6 order: stage (drop zone), then subject (card), then method (table)", tops == sorted(tops), f"tops {tops}")
        cols = page.evaluate("new Set([...document.querySelectorAll('main > *')].map(e => Math.round(e.getBoundingClientRect().left))).size")
        check("R6 right angles: boxes have no rounded corners", all(e["radius"] in ("0px", "") for e in vis if e["tag"] in ("BUTTON", "SECTION", "SELECT", "INPUT", "IMG", "TABLE")), "border-radius 0 on buttons, drop zone, inputs, card")
        ctx.close()
        ctxp, pagep, _, phasep = load(browser, base, 390, 844)
        drop(pagep, phasep, zips[0])
        pagep.wait_for_timeout(1200)
        sw = pagep.evaluate("[document.documentElement.scrollWidth, innerWidth]")
        cols = pagep.evaluate("new Set([...document.querySelectorAll('main > *, #result > *')].map(e => Math.round(e.getBoundingClientRect().left))).size")
        check("R6 single column on mobile; no sideways scroll at 390 px", sw[0] <= sw[1] and cols == 1, f"scrollWidth {sw[0]} for viewport {sw[1]}; {cols} distinct left edge(s)")
        ctxp.close()

        # --- section 3 chrome
        ctx, page, reqs, phase = load(browser, base, 1280, 900)
        drop(page, phase, zips[0])
        page.wait_for_timeout(1200)
        b = page.evaluate("""() => ['dl-png', 'dl-svg', 'dl-json'].map(id => { const e = document.getElementById(id), s = getComputedStyle(e);
          return { bg: s.backgroundColor, h: e.getBoundingClientRect().height, r: s.borderRadius }; })""")
        filled = [x["bg"] not in ("rgba(0, 0, 0, 0)", "transparent") for x in b]
        check("S3 Download PNG is the one filled button; SVG and JSON are quiet text buttons of the same height and radius", filled == [True, False, False] and len({x["h"] for x in b}) == 1 and len({x["r"] for x in b}) == 1, f"filled {filled}, heights {[x['h'] for x in b]}, radii {[x['r'] for x in b]}")
        card = page.evaluate("""() => { const c = document.getElementById('card'), s = getComputedStyle(c), p = getComputedStyle(c.parentElement);
          return [s.borderTopWidth, p.borderTopWidth, p.backgroundColor, p.paddingTop]; }""")
        check("S3 no bordered container around the card (no double frame)", card[0] == "0px" and card[1] == "0px", f"img border {card[0]}, parent border {card[1]}")
        check("S3 TMDB disclaimer is on the site", "not endorsed, certified, or otherwise approved by TMDB" in html, "site footer")

        # --- rule 2 and 4: colour per user, contrast, timeline
        ctx.close()
        out = []
        for scheme in ("light", "dark"):
            for z in zips:
                c2, pg, _, ph = load(browser, base, 1280, 900, scheme=scheme)
                drop(pg, ph, z)
                pg.wait_for_timeout(1300)
                r = pg.evaluate("""async () => { const s = getComputedStyle(document.getElementById('dl-png')); const svg = await (await fetch(document.getElementById('card').src)).text();
                  return [document.documentElement.dataset.accent || '', s.backgroundColor, s.color, svg.match(/\\.f-bg\\{fill:(#[0-9a-f]{6})\\}/)[1]]; }""")
                hexrgb = lambda h: [int(h[i:i + 2], 16) for i in (1, 3, 5)]
                out.append((scheme, z.name[-30:-4], r[0], rgb(r[1]) == hexrgb(r[3]), cr(rgb(r[1]), rgb(r[2]))))
                c2.close()
        keys = {o[2] for o in out}
        worst = min(o[4] for o in out)
        check("R2 one accent per user, from the card's signature stat", "" not in keys, f"{len(zips)} users give accents {sorted(keys)}")
        check("R2 the accent is the exact ground colour of the card on show", all(o[3] for o in out), f"{sum(o[3] for o in out)} of {len(out)} renders: the filled button equals the card's .f-bg")
        check("R2 the text on the accent (the card's ink) passes WCAG AA (4.5:1), light and dark", worst >= 4.5, f"lowest ratio over {len(out)} renders: {worst:.2f}")

        c3, pg, _, ph = load(browser, base, 1280, 900)
        pg.evaluate("window.__t = []; new MutationObserver(() => window.__t.push(['accent', performance.now()])).observe(document.documentElement, {attributes: true, attributeFilter: ['data-accent']})")
        t0 = pg.evaluate("performance.now()")
        pg.set_input_files("#file", str(zips[0]))
        pg.wait_for_function("!document.getElementById('result').hidden")
        early = pg.evaluate("[document.documentElement.dataset.accent || '', getComputedStyle(document.getElementById('card')).animationName, getComputedStyle(document.getElementById('card')).animationDuration, getComputedStyle(document.getElementById('card')).animationTimingFunction]")
        pg.wait_for_timeout(1300)
        t = pg.evaluate("window.__t")
        lag = (t[0][1] - t0) if t else None
        check("R4 colour event: the card lands, holds, then one palette shift", early[0] in ("", "none") and early[1] == "land" and bool(t) and len(t) == 1 and 800 <= lag <= 2500, f"accent unset while the card lands ({early[1]} {early[2]} {early[3]}); one shift {lag:.0f} ms after the drop" if t else "no shift")
        c3.close()
        c4, pg, _, ph = load(browser, base, 1280, 900, motion="reduce")
        drop(pg, ph, zips[0])
        pg.wait_for_timeout(100)
        r = pg.evaluate("[document.documentElement.dataset.accent || '', getComputedStyle(document.getElementById('card')).animationName]")
        check("R4 the colour event is off under prefers-reduced-motion", r[0] != "" and r[1] == "none", f"accent at once ({r[0]}), animation {r[1]}")
        c4.close()

        # --- rule 8: motion
        ctx, page, reqs, phase = load(browser, base, 1280, 900)
        drop(page, phase, zips[0])
        page.wait_for_timeout(1300)
        els = page.evaluate(ALL)
        longest = max(max(secs(e["td"]), secs(e["ad"])) for e in els)
        check("R8 all motion under 300 ms", longest < 0.3, f"longest transition or animation {longest * 1000:.0f} ms")
        kf = re.search(r"@keyframes land \{([^}]*\{[^}]*\}[^}]*\{[^}]*\})", css)
        check("R8 card reveal: opacity 0 and scale 0.98 to 1, 200 ms, ease-out", bool(kf) and "opacity: 0" in kf.group(1) and "scale(.98)" in kf.group(1) and "scale(1)" in kf.group(1) and "animation: land 200ms ease-out" in css, "keyframes land and #card.land in style.css")
        check("R8 buttons: scale(0.97) on :active in about 120 ms", "button:active { transform: scale(.97); }" in css and "transform 120ms ease-out" in css, "style.css")
        dz = page.evaluate("[getComputedStyle(document.getElementById('drop')).transitionDuration, (document.getElementById('drop').classList.add('over'), getComputedStyle(document.getElementById('drop')).transitionDuration)]")
        check("R8 drag-over state is instant", dz[0] == "0s" and dz[1] == "0s", f"#drop transition-duration {dz}")
        hov = [m for m in re.findall(r"[^{}]*:hover[^{}]*\{([^}]*)\}", css) if re.search(r"transform|translate|shadow|scale", m)]
        check("R8 ban: no hover lifts", not hov, "no transform, translate or shadow in any :hover rule")
        check("R8 ban: no scroll fades", not re.search(r"IntersectionObserver|animation-timeline|scroll-timeline|addEventListener\(.scroll", js + css), "no scroll listener, observer or scroll-driven animation")
        sv = page.evaluate("fetch(document.getElementById('card').src).then(r => r.text())")
        check("R8 ban: no animated radar", not re.search(r"<animate|@keyframes|<set |animation", sv), "card SVG has no animate, set or keyframes")

        # --- rule 9
        emoji = re.findall(r"[☀-➿\U0001f300-\U0001faff]", text + html)
        check("R9 ban: no emoji or icon sprinkles", not emoji, f"{len(emoji)} found")
        check("R9 ban: no gradient text", "background-clip" not in css and "gradient" not in css, "no background-clip:text, no gradient")
        check("R9 ban: no uniform rounded cards with shadows", not any(e["shadow"] != "none" for e in els) and all(e["radius"] in ("0px", "") for e in els if e["tag"] in ("SECTION", "DIV", "TABLE", "IMG")), "no shadow, no radius on boxes")
        words = [w for w in ("unleash", "seamless", "powered by ai") if w in (text + html).lower()]
        check("R9 ban: no \"unleash\", \"seamless\", \"powered by AI\" copy", not words, "none in text or markup")
        check("R9 ban: no centred hero with three feature cards", page.evaluate("getComputedStyle(document.querySelector('header')).textAlign") != "center" and page.evaluate("document.querySelectorAll('main > section').length") <= 2, "header left-aligned; no card grid")
        films = len(json.loads((SITE / "films.json").read_text())["films"])
        users = json.loads((ROOT / "tools/calibration.json").read_text())["users"]["power_precision"]
        check("R9 copy uses real numbers", f"{users:,}" in html and f"{films:,}" in html, f"{films:,} films (films.json) and {users:,} users (calibration.json) both in the lede")
        browser.close()
    print("| Rule or ban | Result | Evidence |\n|---|---|---|")
    for r in rows:
        print("| " + " | ".join(r) + " |")
    fails = [r for r in rows if r[1] != "pass"]
    print(f"\n{len(rows) - len(fails)} of {len(rows)} pass")
    sys.exit(1 if fails else 0)


main()
