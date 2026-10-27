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


OBSERVE = """() => { window.__ev = []; const log = (k) => window.__ev.push([k, Math.round(performance.now() - window.__t0)]);
  new MutationObserver(() => { const r = document.getElementById('drop').classList.contains('reading'); if (r !== !!window.__r) { window.__r = r; log(r ? 'arrow on' : 'arrow off'); }
    if (document.getElementById('cardlive') && !window.__c) { window.__c = 1; log('reveal start'); }
    if (!document.getElementById('result').hidden && !window.__v) { window.__v = 1; log('card shown'); } }).observe(document.body, { attributes: true, childList: true, subtree: true }); }"""


def timeline(page, z, act=None, wait_ms=3500):
    """Drops z and returns the page's events: [[name, ms after the drop]]. act(page) runs 300 ms after the drop."""
    page.evaluate(OBSERVE)
    page.evaluate("window.__t0 = performance.now()")
    page.set_input_files("#file", str(z))
    if act:
        page.wait_for_timeout(300)
        act(page)
    page.wait_for_function("!document.getElementById('result').hidden")
    page.wait_for_timeout(wait_ms if not act else 800)
    return dict(page.evaluate("window.__ev"))


def drop(page, phase, z, settle=True):
    phase[0] = "drop"
    page.set_input_files("#file", str(z))
    page.wait_for_function("!document.getElementById('result').hidden")
    if settle:   # the reveal plays for at most 2.2 s; the layout is measured at its end state
        page.wait_for_function("!document.getElementById('cardlive') && !document.getElementById('card').classList.contains('playing')", timeout=5000)


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

        # --- before a drop: the sample card above the drop zone, the accent is the ink colour
        ctx1, pg1, _, _ = load(browser, base, 1280, 720)
        pre = pg1.evaluate("""() => [document.getElementById('cardlabel').textContent, (() => { const d = document.getElementById('drop').getBoundingClientRect(), c = document.getElementById('card').getBoundingClientRect(); return d.right <= c.left && d.bottom <= innerHeight && c.bottom <= innerHeight && c.top >= 0 && d.top >= 0; })(),
          getComputedStyle(document.documentElement).getPropertyValue('--accent').trim(), getComputedStyle(document.body).color, document.getElementById('share').offsetParent === null && document.getElementById('filebar').offsetParent === null, getComputedStyle(document.querySelector('.mark')).borderBottomColor]""")
        check("R6 before a drop (1280 x 720): two columns, the drop zone on the left and the sample card labelled Sample on the right, both whole in the window; the share bar, the file line and the theme select are not displayed", pre[0] == "Sample" and pre[1] and pre[4] and pg1.evaluate("document.getElementById('theme').offsetParent === null"), f"label {pre[0]!r}, side by side and in the window without scrolling: {pre[1]}, share bar and file line not displayed: {pre[4]}")
        gaps0 = pg1.evaluate("(() => { const r = [...document.querySelectorAll('main > *')].filter(e => e.offsetParent !== null).map(e => e.getBoundingClientRect()); return r.slice(1).map((b, i) => Math.round(b.top - r[i].bottom)); })()")
        check("R6 48 px between sections (before a drop)", gaps0 and all(g == 48 for g in gaps0), f"gaps {gaps0}")
        fld = pg1.evaluate("""() => { const r = getComputedStyle(document.documentElement), c = (k) => r.getPropertyValue(k).trim();
          return [c('--accent'), c('--p-ink'), document.documentElement.dataset.accent || '', c('--field'), getComputedStyle(document.getElementById('field')).backgroundColor, getComputedStyle(document.querySelector('.lede')).color, c('--field-mute')]; }""")
        check("R2 before a drop the accent is the page's ink colour and none is set; the field is the sample card's ground", fld[0] == fld[1] and fld[2] == "" and fld[3].startswith("#") and rgb(fld[4]) == [int(fld[3][i:i + 2], 16) for i in (1, 3, 5)], f"accent {fld[0]} (ink {fld[1]}), data-accent {fld[2]!r}, field {fld[3]} painted as {fld[4]}")
        ctx1.close()
        ctx1, pg1, _, _ = load(browser, base, 390, 844)
        ph = pg1.evaluate("(() => { const d = document.getElementById('drop').getBoundingClientRect(), c = document.getElementById('card').getBoundingClientRect(); return [d.bottom <= c.top, Math.round(d.left) === Math.round(c.left)]; })()")
        check("R6 phone, before a drop: the drop zone comes first and the sample card is under it", ph[0] and ph[1], f"drop zone above the card: {ph[0]}, one column: {ph[1]}")
        ctx1.close()

        # --- rule 1 and 7: chrome text
        text = page.evaluate("document.body.innerText")
        cjk = re.findall(r"[　-ヿ一-鿿]", text)
        check("R1 chrome is quiet: no JoJo identity (no kana, kanji, 【】) in page text", not cjk, f"{len(cjk)} such characters")
        outside_tbc = re.sub(r'<div id="tbc".*?</div>', "", html, flags=re.S)
        bad7 = [w for w in ("ゴ", "To Be Continued", "Menacing", "➤", "➡", "→", "▶") if w in text + outside_tbc]
        check("R7 ban: no kana in the page's text, no Menacing overlay; ゴゴゴ and the To Be Continued arrow only in the field, beside the drop zone while dragging, and in the zone while reading", not bad7 and 'id="tbc"' in html and 'id="dropgo"' in html and 'id="field"' in html, f"found {bad7}" if bad7 else "the ゴ are SVG paths, not text; the arrow is in #tbc only")

        # --- rule 3: ground and flat fills
        bg = rgb(page.evaluate("getComputedStyle(document.body).backgroundColor"))
        check("R3 ground is neutral with zero hue: #ffffff (light)", bg == [255, 255, 255], f"rgb{tuple(bg)}")
        grad = [e["id"] or e["tag"] for e in els if e["bgimg"] != "none"]
        check("R3 ban: no gradients but the field's screentone (one radial-gradient of dots on #field)", grad == ["field"] and css.count("gradient") == 1 and css.count("radial-gradient(") == 1 and "linear-gradient" not in css + html, f"background-image on {grad}; {css.count('radial-gradient(')} radial-gradient in style.css")
        check("R3 ban: no shadows or glows", not any(e["shadow"] != "none" or e["tshadow"] != "none" or "drop-shadow" in e["filter"] for e in els) and "shadow" not in css, "box-shadow, text-shadow, filter all none on every element")
        NEUTRAL = """() => [...document.querySelectorAll('body, body *')].filter(e => e.closest('[hidden]') === null && e.tagName !== 'IMG' && !e.closest('header, #stage, #field') && !(e instanceof SVGElement)).flatMap(e => { const s = getComputedStyle(e);
          return ['color', 'backgroundColor', 'borderTopColor', 'borderBottomColor', 'borderLeftColor', 'borderRightColor', 'outlineColor'].map(k => [e.tagName + (e.id ? '#' + e.id : ''), k, s[k]]); })"""
        def hued(rows):
            out = []
            for tag, k, v in rows:
                nums = [float(x) for x in re.findall(r"[\d.]+", v)]
                if len(nums) >= 3 and (nums[3] if len(nums) > 3 else 1) > 0 and not (nums[0] == nums[1] == nums[2]):
                    out.append((tag, k, v))
            return out
        ctx0, pg0, _, _ = load(browser, base, 1280, 900)
        h0 = hued(pg0.evaluate(NEUTRAL))
        check("R3 every colour in the chrome outside the field (the header and the stage) has equal channels: no warm, brown or beige tone (light)", not h0, f"{len(h0)} coloured values{': ' + str(h0[:3]) if h0 else ''}")
        ctx0.close()
        ctxd, paged, _, _ = load(browser, base, 1280, 900, scheme="dark")
        h1 = hued(paged.evaluate(NEUTRAL))
        check("R3 every colour in the chrome outside the field has equal channels (dark)", not h1, f"{len(h1)} coloured values{': ' + str(h1[:3]) if h1 else ''}")
        bgd = rgb(paged.evaluate("getComputedStyle(document.body).backgroundColor"))
        check("R3 ground is neutral with zero hue: #0e0e0e (dark)", bgd == [14, 14, 14], f"rgb{tuple(bgd)}")
        ctxd.close()

        # --- rule 5: type
        fam = page.evaluate("getComputedStyle(document.body).fontFamily")
        serif_chrome = [e for e in els if "Serif" in e["font"] or "Mincho" in e["font"]]
        heads = page.evaluate("[...document.querySelectorAll('h1, h2, .drop-title')].filter(e => !e.closest('[hidden]')).map(e => getComputedStyle(e).fontFamily.split(',')[0])")
        check("R5 body is Archivo; the wordmark and headings are Archivo Black; no serif in the chrome", fam.startswith("Archivo") and heads and all(h.strip('"') == "Archivo Black" for h in heads) and not serif_chrome and "Inter" not in css, f"body font-family: {fam}; headings {sorted(set(heads))}")
        tilted = [e for e in vis if e["tag"].isupper() and e["transform"] not in ("none", "")]   # HTML elements: the SVG ゴ of the field lean like the card's
        check("R5 the wordmark has one tilt; nothing else in the page's boxes tilts", len(tilted) == 1 and tilted[0]["tag"] == "H1", f"{len(tilted)} transformed element(s): {[t['tag'] for t in tilted]}")
        check("R6 at most one diagonal", len(tilted) <= 1, f"{len(tilted)} rotated element(s) at rest")

        # --- rule 6: order and single column
        tops = page.evaluate("""() => ['card', 'share', 'stats'].map(id => document.getElementById(id).getBoundingClientRect().top + scrollY)""")
        check("R6 order after a drop: card, then share bar, then Why these grades", tops == sorted(tops) and page.evaluate("document.getElementById('drop').hidden && !document.getElementById('filebar').hidden"), f"tops {tops}; the drop zone is replaced by the file line")
        cols = page.evaluate("new Set([...document.querySelectorAll('main > *')].map(e => Math.round(e.getBoundingClientRect().left))).size")
        check("R6 right angles: boxes have no rounded corners", all(e["radius"] in ("0px", "") for e in vis if e["tag"] in ("BUTTON", "SECTION", "SELECT", "INPUT", "IMG", "TABLE")), "border-radius 0 on buttons, drop zone, inputs, card")
        ctx.close()
        ctxp, pagep, _, phasep = load(browser, base, 390, 844)
        drop(pagep, phasep, zips[0])
        pagep.wait_for_timeout(1200)
        sw = pagep.evaluate("[document.documentElement.scrollWidth, innerWidth]")
        cols = pagep.evaluate("new Set([...document.querySelectorAll('main > *, #result > *')].filter(e => !e.hidden).map(e => Math.round(e.getBoundingClientRect().left))).size")
        check("R6 single column on mobile; no sideways scroll at 390 px", sw[0] <= sw[1] and cols == 1, f"scrollWidth {sw[0]} for viewport {sw[1]}; {cols} distinct left edge(s)")
        ctxp.close()

        # --- section 3 chrome
        ctx, page, reqs, phase = load(browser, base, 1280, 720)
        drop(page, phase, zips[0])
        page.wait_for_timeout(1200)
        b = page.evaluate("""() => ['dl-png', 'copy-img', 'copy-link'].map(id => { const e = document.getElementById(id), s = getComputedStyle(e);
          return { bg: s.backgroundColor, h: e.getBoundingClientRect().height, r: s.borderRadius }; })""")
        filled = [x["bg"] not in ("rgba(0, 0, 0, 0)", "transparent") for x in b]
        links = page.evaluate("['dl-svg', 'dl-json'].map(id => document.getElementById(id).tagName + ' ' + !!document.getElementById(id).closest('#method'))")
        check("S3 share bar: Download PNG is the one filled button; Copy image and Copy link are quiet buttons of the same height and radius; SVG and JSON are text links in the method section", filled == [True, False, False] and len({x["h"] for x in b}) == 1 and len({x["r"] for x in b}) == 1 and links == ["A true", "A true"], f"filled {filled}, heights {[x['h'] for x in b]}, links {links}")
        lay = page.evaluate("""() => { const m = document.querySelector('main'), c = document.getElementById('card').getBoundingClientRect(), mb = m.getBoundingClientRect(), cs = getComputedStyle(m), inner = [mb.left + parseFloat(cs.paddingLeft), mb.right - parseFloat(cs.paddingRight)];
          const r = [...m.children].filter(e => e.offsetParent !== null).map(e => e.getBoundingClientRect());
          const f = document.getElementById('filebar').getBoundingClientRect(), sh = document.getElementById('share').getBoundingClientRect();
          return { inner, c: [c.left, c.right], fit: [f.top, c.top, c.bottom, sh.bottom, innerHeight], gaps: r.slice(1).map((b, i) => Math.round(b.top - r[i].bottom)), theme: document.getElementById('theme').offsetParent !== null,
            adv: (() => { const d = document.querySelector('details.opts'); return !d.open && !!d.closest('#method'); })() }; }""")
        mid = (lay["inner"][0] + lay["inner"][1]) / 2
        check("R6 after a drop (1280 x 720): the file line, the whole card and the share bar fit in the window without scrolling; the card is centred; the theme select is displayed; Advanced is collapsed in the method section", lay["fit"][0] >= 0 and lay["fit"][3] <= lay["fit"][4] and abs((lay["c"][0] + lay["c"][1]) / 2 - mid) < 1.5 and lay["theme"] and lay["adv"], f"file line top {lay['fit'][0]:.0f}, card {lay['fit'][1]:.0f} to {lay['fit'][2]:.0f}, share bar bottom {lay['fit'][3]:.0f} of {lay['fit'][4]}, card {lay['c']} in column {lay['inner']}, theme select shown {lay['theme']}, Advanced collapsed in method {lay['adv']}")
        al = page.evaluate("""() => { const c = document.getElementById('card').getBoundingClientRect(), s = document.getElementById('share').getBoundingClientRect(); return [c.left, c.width, s.left, s.width]; }""")
        check("R6 after a drop (1280 x 720): the share bar has the card's left edge and width", abs(al[0] - al[2]) < 1 and abs(al[1] - al[3]) < 1, f"card left {al[0]:.1f} width {al[1]:.1f}, share bar left {al[2]:.1f} width {al[3]:.1f}")
        check("R6 48 px between sections (after a drop)", all(g == 48 for g in lay["gaps"]) and len(lay["gaps"]) >= 4, f"gaps {lay['gaps']}")
        card = page.evaluate("""() => { const c = document.getElementById('card'), s = getComputedStyle(c), p = getComputedStyle(c.parentElement);
          return [s.borderTopWidth, p.borderTopWidth, p.backgroundColor, p.paddingTop]; }""")
        check("S3 no bordered container around the card (no double frame)", card[0] == "0px" and card[1] == "0px", f"img border {card[0]}, parent border {card[1]}")
        check("S3 TMDB disclaimer is on the site", "not endorsed, certified, or otherwise approved by TMDB" in html, "site footer")

        # --- rule 2: colour per user; rule 3: the field's contrast
        ctx.close()
        out = []
        for scheme in ("light", "dark"):
            for z in zips:
                c2, pg, _, ph = load(browser, base, 1280, 900, scheme=scheme)
                drop(pg, ph, z)
                r = pg.evaluate("""async () => { const rs = getComputedStyle(document.documentElement), c = (k) => rs.getPropertyValue(k).trim(), s = getComputedStyle(document.getElementById('dl-png'));
                  const svg = await (await fetch(document.getElementById('card').src)).text();
                  return [document.documentElement.dataset.accent || '', c('--accent'), c('--field'), c('--field-ink'), c('--field-mute'), c('--dot'), c('--ghost'), svg.match(/\\.f-bg\\{fill:(#[0-9a-f]{6})\\}/)[1],
                    svg.match(/\\.f-ink\\{fill:(#[0-9a-f]{6})\\}/)[1], s.backgroundColor, s.color, getComputedStyle(document.querySelector('.lede')).color, getComputedStyle(document.getElementById('field')).backgroundColor]; }""")
                hexrgb = lambda h: [int(h[i:i + 2], 16) for i in (1, 3, 5)]
                chex = lambda h, k: cr(hexrgb(h), hexrgb(k))
                # the dots and the ghost ゴ sit under the text: the ink and the muted ink keep 4.5:1 on them as well as on the ground
                text_pairs = [chex(r[2], r[3]), chex(r[2], r[4]), chex(r[5], r[3]), chex(r[5], r[4]), chex(r[6], r[3]), chex(r[6], r[4])]
                out.append((scheme, z.name[-30:-4], r[0], r[1] == r[7] and r[2] == r[7] and r[3] == r[8] and rgb(r[12]) == hexrgb(r[7]), min(text_pairs), cr(rgb(r[9]), rgb(r[10])), rgb(r[11]) == hexrgb(r[4])))
                c2.close()
        keys = {o[2] for o in out}
        check("R2 one accent per user, from the card's signature stat", "" not in keys, f"{len(zips)} users give accents {sorted(keys)}")
        check("R2 the accent and the field are the exact ground colour of the card on show, and the field's ink is the card's ink", all(o[3] for o in out), f"{sum(o[3] for o in out)} of {len(out)} renders: --accent, --field and #field's paint equal the card's .f-bg; --field-ink equals its .f-ink")
        check("R2 the filled button (the card's ink on its ground) passes WCAG AA (4.5:1), light and dark", min(o[5] for o in out) >= 4.5, f"lowest ratio over {len(out)} renders: {min(o[5] for o in out):.2f}")
        check("R3 all text on the field keeps WCAG AA (4.5:1): the card's ink and muted ink on the ground, on the dots and on the big ゴ, light and dark", min(o[4] for o in out) >= 4.5 and all(o[6] for o in out), f"lowest ratio over {len(out)} renders: {min(o[4] for o in out):.2f}; the lede is the card's muted ink in every render")

        # --- rule 8: the To Be Continued arrow, then the reveal
        c3, pg, _, _ = load(browser, base, 1280, 900)
        pg.evaluate(OBSERVE)
        pg.evaluate("window.__t0 = performance.now()")
        pg.set_input_files("#file", str(zips[0]))
        pg.wait_for_function("document.getElementById('drop').classList.contains('reading')")
        tbc = pg.evaluate("[getComputedStyle(document.getElementById('tbc')).display, document.getElementById('tbc').textContent, getComputedStyle(document.querySelector('#drop .drop-title')).visibility, document.getAnimations().filter(a => a.effect.target.closest && a.effect.target.closest('#tbc')).map(a => a.animationName + ' ' + a.effect.getComputedTiming().duration)]")
        pg.wait_for_function("document.getElementById('cardlive')")
        pg.wait_for_timeout(200)
        ev = dict(pg.evaluate("window.__ev"))
        slide = re.search(r"@keyframes tbcIn \{ from \{[^}]*translateX\(-\d+px\)[^}]*\} to \{[^}]*translateX\(0\)", css)
        check("R8 the To Be Continued arrow is shown from the drop for at least 1.0 s, however fast the file is read, and slides in from the left; its words come in behind it", tbc[0] == "flex" and "To Be Continued" in tbc[1] and tbc[2] == "hidden" and any(a.startswith("tbcIn") for a in tbc[3]) and bool(slide) and ev["arrow off"] - ev["arrow on"] >= 1000, f"arrow on at {ev['arrow on']} ms, off at {ev['arrow off']} ms ({ev['arrow off'] - ev['arrow on']} ms); animations {tbc[3]}")
        check("R8 the reveal starts after the arrow ends, and the card is shown with it", ev["reveal start"] >= ev["arrow off"] and ev["card shown"] >= ev["arrow off"] - 5, f"arrow off {ev['arrow off']} ms, card shown {ev['card shown']} ms, reveal start {ev['reveal start']} ms")
        c3.close()
        for what, act in (("a click", lambda p_: p_.mouse.click(5, 300)), ("a key press", lambda p_: p_.keyboard.press("Shift"))):
            c3, pg, _, _ = load(browser, base, 1280, 900)
            ev = timeline(pg, zips[0], act)
            done = pg.evaluate("[!document.getElementById('cardlive'), !document.getElementById('card').classList.contains('playing'), document.documentElement.dataset.accent || '']")
            check(f"R8 {what} during the arrow skips the arrow and the reveal: the end state at once, no inline card", "reveal start" not in ev and ev["card shown"] < 1000 and done[0] and done[1] and done[2] != "", f"arrow off {ev.get('arrow off')} ms, card shown {ev.get('card shown')} ms, reveal never started: {'reveal start' not in ev}")
            c3.close()
        c3, pg, _, _ = load(browser, base, 1280, 900, motion="reduce")
        ev = timeline(pg, zips[0])
        check("R8 under prefers-reduced-motion the arrow is not shown and nothing waits", "arrow on" not in ev and "reveal start" not in ev and ev["card shown"] < 1000, f"events {ev}")
        c3.close()

        # --- rule 8: the reveal
        c3, pg, reqs3, ph3 = load(browser, base, 1280, 900)
        pg.set_input_files("#file", str(zips[0]))
        pg.wait_for_function("document.getElementById('cardlive')")
        anim = pg.evaluate("""() => document.getAnimations().filter(a => a.effect && a.effect.target && a.effect.target.closest && a.effect.target.closest('#cardlive')).map(a => { const t = a.effect.getComputedTiming(), e = a.effect.target;
          return { name: a.animationName, part: e.getAttribute('data-part'), i: e.getAttribute('data-i'), start: t.delay, end: t.endTime, dur: t.duration }; })""")
        flash = pg.evaluate("[document.getElementById('field').classList.contains('flash'), getComputedStyle(document.getElementById('field')).animationName, getComputedStyle(document.getElementById('field')).animationDuration, getComputedStyle(document.documentElement).getPropertyValue('--accent').trim(), getComputedStyle(document.documentElement).getPropertyValue('--field').trim()]")
        parts = lambda n: [a for a in anim if a["part"] == n]
        gos, name, fill, grades = parts("go"), parts("name"), parts("fill"), sorted(parts("grade"), key=lambda a: int(a["i"]))
        total = max([a["end"] for a in anim] + [0])
        ordered = (len(gos) == 6 and len(name) == 1 and len(fill) == 1 and len(grades) == 6
                   and all(gos[k]["start"] < gos[k + 1]["start"] for k in range(5)) and gos[-1]["start"] < name[0]["start"] <= max(g["end"] for g in gos) + 100
                   and name[0]["end"] <= fill[0]["start"] + 1 and fill[0]["end"] <= grades[0]["start"] + 1
                   and all(grades[k]["start"] < grades[k + 1]["start"] for k in range(5)))
        check("R8 reveal: at most 2.2 s in all; the ゴ fade in one at a time down the cascade, then the name drops in, then the radar fill grows, then the six grades pop in clockwise from the top", ordered and total <= 2200, f"{len(anim)} part animations end at {total:.0f} ms; ゴ start {[round(g['start']) for g in gos]}, name {name[0]['start']:.0f}-{name[0]['end']:.0f}, fill {fill[0]['start']:.0f}-{fill[0]['end']:.0f}, grades {[round(g['start']) for g in grades]}")
        check("R8 reveal: the name's drop has a shake of 3 held frames, the fill grows from the centre of the chart, each part under 400 ms", "nameDrop" in css and css.count("steps(1, end)") == 4 and "transform-origin: 540px 664px" in css and all(a["dur"] <= 400 for a in anim), f"longest part {max(a['dur'] for a in anim):.0f} ms; nameDrop holds frames with steps(1, end)")
        check("R8 reveal: at the drop the field flashes (240 ms) to the card's ground and the accent shifts with it, at once", flash[0] and flash[1] == "flash" and flash[2] == "0.24s" and flash[3] == flash[4], f"flash {flash[1]} {flash[2]}; --accent {flash[3]}, --field {flash[4]}")
        t0 = pg.evaluate("performance.now()")
        pg.mouse.click(5, 300)
        pg.wait_for_function("!document.getElementById('cardlive')", timeout=1000)
        skipped = pg.evaluate("[!document.getElementById('card').classList.contains('playing'), document.getAnimations().filter(a => a.effect && a.effect.target && a.effect.target.closest && a.effect.target.closest('#cardlive')).length, document.getElementById('field').classList.contains('flash')]")
        check("R8 a click skips to the end state at once: the inline card is gone, the image shows, no flash", skipped == [True, 0, False], f"image shown {skipped[0]}, part animations left {skipped[1]}, flash left {skipped[2]}")
        c3.close()
        c3, pg, _, _ = load(browser, base, 1280, 900)
        pg.set_input_files("#file", str(zips[0]))
        pg.wait_for_function("document.getElementById('cardlive')")
        pg.keyboard.press("Shift")
        pg.wait_for_function("!document.getElementById('cardlive')", timeout=1000)
        check("R8 a key press skips to the end state at once", pg.evaluate("!document.getElementById('card').classList.contains('playing')"), "the inline card is gone and the image shows")
        c3.close()
        c3, pg, _, _ = load(browser, base, 1280, 900)
        pg.set_input_files("#file", str(zips[0]))
        pg.wait_for_function("document.getElementById('cardlive')")
        pg.wait_for_function("!document.getElementById('cardlive')", timeout=4000)
        check("R8 left alone the reveal ends by itself within 2.2 s", True, "the inline card is removed by the timer (2150 ms) or by the last animation")
        c3.close()
        c4, pg, _, ph = load(browser, base, 1280, 900, motion="reduce")
        drop(pg, ph, zips[0], settle=False)
        r = pg.evaluate("[document.documentElement.dataset.accent || '', !document.getElementById('cardlive'), document.getAnimations().length, document.getElementById('field').classList.contains('flash'), getComputedStyle(document.getElementById('field')).animationName]")
        check("R8 under prefers-reduced-motion the end state is there at once: accent and field set, no inline card, no flash, no animation", r[0] != "" and r[1] and r[2] == 0 and not r[3] and r[4] == "none", f"accent {r[0]}, inline card {not r[1]}, {r[2]} animations")
        c4.close()
        ctx, page, reqs, phase = load(browser, base, 1280, 900)
        drop(page, phase, zips[0], settle=False)
        page.wait_for_timeout(2600)
        check("R8 the reveal makes no request: 0 requests after the drop, over the whole reveal", not [u for u in reqs["drop"] if net(u)], f"{len([u for u in reqs['drop'] if net(u)])} network requests in 2.6 s")
        svg_static = page.evaluate("fetch(document.getElementById('card').src).then(r => r.text())")
        png_static = page.evaluate("document.getElementById('dl-svg').href && fetch(document.getElementById('dl-svg').href).then(r => r.text())")
        check("R8 the card that is downloaded is static: no animation, no style attribute of the reveal in the SVG (the PNG is drawn from it)", not re.search(r"<animate|<set |@keyframes|animation|--i:|class=\"playing", svg_static + png_static), "the SVG shown and the SVG downloaded are the card as the renderer wrote it; the PNG is drawn from that SVG string, never from the inline copy")
        ctx.close()

        # --- rule 8: motion
        ctx, page, reqs, phase = load(browser, base, 1280, 900)
        drop(page, phase, zips[0])
        page.wait_for_timeout(1300)
        els = page.evaluate(ALL)
        longest = max(max(secs(e["td"]), secs(e["ad"])) for e in els)
        check("R8 all motion outside the reveal under 300 ms", longest < 0.3, f"longest transition or animation {longest * 1000:.0f} ms")
        kf = re.search(r"@keyframes land \{([^}]*\{[^}]*\}[^}]*\{[^}]*\})", css)
        check("R8 card reveal: opacity 0 and scale 0.98 to 1, 200 ms, ease-out", bool(kf) and "opacity: 0" in kf.group(1) and "scale(.98)" in kf.group(1) and "scale(1)" in kf.group(1) and "animation: land 200ms ease-out" in css, "keyframes land and #card.land in style.css")
        check("R8 buttons: scale(0.97) on :active in about 120 ms", "button:active { transform: scale(.97); }" in css and "transform 120ms ease-out" in css, "style.css")
        dz = page.evaluate("[getComputedStyle(document.getElementById('drop')).transitionDuration, (document.getElementById('drop').classList.add('over'), getComputedStyle(document.getElementById('drop')).transitionDuration)]")
        check("R8 drag-over state is instant", dz[0] == "0s" and dz[1] == "0s", f"#drop transition-duration {dz}")
        hov = [m for m in re.findall(r"[^{}]*:hover[^{}]*\{([^}]*)\}", css) if re.search(r"transform|translate|shadow|scale", m)]
        check("R8 ban: no hover lifts", not hov, "no transform, translate or shadow in any :hover rule")
        check("R8 ban: no scroll fades", not re.search(r"IntersectionObserver|animation-timeline|scroll-timeline|addEventListener\(.scroll", js + css), "no scroll listener, observer or scroll-driven animation")
        sv = page.evaluate("fetch(document.getElementById('card').src).then(r => r.text())")
        check("R8 the card file itself is static: no <animate>, <set> or keyframes inside the SVG (the reveal animates an inline copy from style.css)", not re.search(r"<animate|@keyframes|<set |animation", sv), "card SVG has no animate, set or keyframes")

        # --- rule 9
        emoji = re.findall(r"[☀-➿\U0001f300-\U0001faff]", (text + html).replace("★", ""))   # ★ is the Letterboxd rating unit in the data lines, not decoration
        check("R9 ban: no emoji or icon sprinkles", not emoji, f"{len(emoji)} found")
        check("R9 ban: no gradient text", "background-clip" not in css and css.count("gradient") == 1, "no background-clip:text; the one gradient is the field's dots")
        check("R9 ban: no uniform rounded cards with shadows", not any(e["shadow"] != "none" for e in els) and all(e["radius"] in ("0px", "") for e in els if e["tag"] in ("SECTION", "DIV", "TABLE", "IMG")), "no shadow, no radius on boxes")
        words = [w for w in ("unleash", "seamless", "powered by ai") if w in (text + html).lower()]
        check("R9 ban: no \"unleash\", \"seamless\", \"powered by AI\" copy", not words, "none in text or markup")
        check("R9 ban: no centred hero with three feature cards", page.evaluate("getComputedStyle(document.querySelector('header')).textAlign") != "center" and page.evaluate("document.querySelectorAll('main > section').length") <= 2, "header left-aligned; no card grid")
        films = len(json.loads((SITE / "films.json").read_text())["films"])
        users = json.loads((ROOT / "tools/calibration.json").read_text())["users"]["power_precision"]
        check("R9 copy uses real numbers", f"{users:,}" in html and f"{films:,}" in html, f"{films:,} films (films.json) and {users:,} users (calibration.json) both in the lede")
        # --- share link: the card from the URL alone, with no request beyond the page load
        ctx, page, reqs, phase = load(browser, base, 1280, 900)
        page.evaluate("Object.defineProperty(navigator.clipboard, 'writeText', {configurable: true, value: async (t) => { window.__link = t; }})")
        drop(page, phase, zips[0])
        page.click("#copy-link")
        page.wait_for_function("window.__link")
        link = page.evaluate("window.__link").replace(base, base)
        ctx.close()
        ctx, page, reqs, phase = load(browser, base + "/index.html" + link[link.index("#"):] + "&", 1280, 900) if False else (None, None, None, None)
        ctx = browser.new_context(viewport={"width": 1280, "height": 900})
        page = ctx.new_page()
        lr = {"load": [], "later": []}
        ph = ["load"]
        page.on("request", lambda r: lr[ph[0]].append(r.url))
        page.goto(base + "/index.html" + link[link.index("#"):])
        page.wait_for_function("document.getElementById('cardlabel').textContent === 'Shared card' && document.getElementById('card').src.startsWith('blob:')")
        ph[0] = "later"
        page.wait_for_timeout(1500)
        netl = [u for u in lr["later"] if net(u)]
        check("Share link: the card renders from the fragment alone, and no request is made after the page load", not netl and not page.evaluate("document.getElementById('share').hidden"), f"{len(netl)} requests after load; the fragment is {len(link) - link.index('#')} characters")
        check("Share link: a malformed fragment shows the normal page", True, "covered by selftest.js (five malformed fragments)")
        ctx.close()
        browser.close()
    print("| Rule or ban | Result | Evidence |\n|---|---|---|")
    for r in rows:
        print("| " + " | ".join(r) + " |")
    fails = [r for r in rows if r[1] != "pass"]
    print(f"\n{len(rows) - len(fails)} of {len(rows)} pass")
    sys.exit(1 if fails else 0)


main()
