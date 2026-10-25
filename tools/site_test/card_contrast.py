"""WCAG contrast of every text colour on the card (docs/DESIGN.md section 4, rule 7), measured on the SVGs the card renderer writes.
Usage: python3 -I tools/site_test/card_contrast.py [out_dir] [--no-build]     (default: a temp dir; builds the SVGs with the card_palettes
example; --no-build reads the SVGs already in out_dir, to try a hand-edited ground)
For each of the 6 accents (12 accent values) and the no-signature card, in the light and dark card themes, it reads the card's own
<style> (each class's colour), finds every outlined text (a <path class="f-*">), and checks its colour against the ground (f-bg), the
ability box's paper (f-paper) and, for the scale letters that lie over the radar, the radar's blended fill. Text drawn at an opacity
is decorative (the ゴゴゴ, rule 6 asks for low contrast) and is reported apart. Prints a table and every failure; exit 1 on any failure.
"""
import re, subprocess, sys, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
AA = 4.5


def lin(c):
    return c / 12.92 if c <= 0.03928 else ((c + 0.055) / 1.055) ** 2.4


def rgb(h):
    return [int(h[i:i + 2], 16) for i in (1, 3, 5)]


def lum(h):
    r, g, b = (lin(x / 255) for x in rgb(h))
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def ratio(a, b):
    hi, lo = sorted((lum(a), lum(b)), reverse=True)
    return (hi + 0.05) / (lo + 0.05)


def blend(top, under, a):
    return "#%02x%02x%02x" % tuple(round(t * a + u * (1 - a)) for t, u in zip(rgb(top), rgb(under)))


def classes(css):
    return {k: v.lower() for k, v in re.findall(r"\.f-(\w+)\{fill:(#[0-9a-fA-F]{6})\}", css)}


def check(svg_path, theme):
    svg = svg_path.read_text()
    style = re.search(r"<style>(.*?)</style>", svg, re.S).group(1)
    cols = classes(style)
    fill_op = float(re.search(r"\.radar\{fill-opacity:([\d.]+)\}", style).group(1))
    bg, paper = cols["bg"], cols["paper"]
    over_fill = blend(cols["pair"], bg, fill_op)
    texts = re.findall(r'<path d="[^"]*" class="f-(\w+)"([^>]*)/>', svg)
    used = {}
    decor = {}
    for cls, rest in texts:
        op = re.search(r'opacity="([\d.]+)"', rest)
        if op:
            decor[cls] = float(op.group(1))
        else:
            used[cls] = used.get(cls, 0) + 1
    rows, fails = [], []
    for cls in sorted(used):
        for ground_name, ground in (("ground", bg), ("box paper", paper)) + ((("radar fill",  over_fill),) if cls == "ink" else ()):
            r = ratio(cols[cls], ground)
            rows.append((cls, cols[cls], ground_name, ground, r))
            if r < AA:
                fails.append((cls, cols[cls], ground_name, ground, r))
    gogogo = [(c, o, ratio(blend(cols[c], bg, o), bg)) for c, o in decor.items()]
    return rows, fails, gogogo, cols


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    out = Path(args[0]) if args else Path(tempfile.mkdtemp(prefix="card_palettes_"))
    if "--no-build" not in sys.argv:
        subprocess.run(["cargo", "run", "-q", "-p", "standuser-core", "--example", "card_palettes", "--", str(out)], cwd=ROOT, check=True)
    all_fails = []
    print("| accent | card | accent text vs ground | vs box paper | ink | mute | ゴゴゴ vs ground |\n|---|---|---|---|---|---|---|")
    for key in ["PWR", "SPD", "RNG", "STA", "PRC", "DEV", "NONE"]:
        for theme in ("light", "dark"):
            rows, fails, goes, cols = check(out / f"{key}-{theme}.svg", theme)
            get = lambda cls, g: min(r for c, _, gn, _, r in rows if c == cls and gn == g)
            ink = min(r for c, _, _, _, r in rows if c == "ink")
            mute = min(r for c, _, _, _, r in rows if c == "mute")
            print(f"| {key} {cols['acc']} | {theme} | {get('acc', 'ground'):.2f} | {get('acc', 'box paper'):.2f} | {ink:.2f} | {mute:.2f} | {goes[0][2]:.2f} (decor) |")
            all_fails += [(key, theme) + f for f in fails]
    if all_fails:
        print(f"\n{len(all_fails)} FAIL (below {AA}:1):")
        for key, theme, cls, col, gn, g, r in all_fails:
            print(f"  accent {key}, {theme} card: text colour {cls} {col} on {gn} {g} = {r:.2f}")
        sys.exit(1)
    print(f"\nall text colours pass WCAG AA ({AA}:1) for 6 accents (12 values) + none, light and dark")


main()
