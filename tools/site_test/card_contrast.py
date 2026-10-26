"""WCAG contrast of every text colour on the card (docs/DESIGN.md section 4, rule 7), measured on the SVGs the card renderer writes.
Usage: python3 -I tools/site_test/card_contrast.py [out_dir] [--no-build]     (default: a temp dir; builds the SVGs with the card_palettes
example; --no-build reads the SVGs already in out_dir)
For each of the 6 accents and the no-signature card, in the light and dark card themes, it reads the card's own <style> (each class's
colour) and every text path (<path class="f-*">), and checks each against what it is drawn on:
  f-ink, f-mute        on the ground (f-bg)                       normal text, 4.5:1
  f-bk, f-pmute        on the paper (f-paper: the ability box and the circled grades)   normal text, 4.5:1
  f-paper (the Stand name, drawn white with a black outline)       its black outline (class o), 4.5:1
  f-fill  (the ゴゴゴ, large lettering with a black outline)         its black outline, 3:1 (large text); the sound effect is decoration
For the outlined lettering the outline is what the glyphs sit against, so that is the measured pair; the ratio against the ground is
printed beside it for information. Prints a table and every failure; exit 1 on any failure.
"""
import re, subprocess, sys, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
AA, AA_LARGE = 4.5, 3.0


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


def check(svg_path):
    svg = svg_path.read_text()
    style = re.search(r"<style>(.*?)</style>", svg, re.S).group(1)
    cols = {k: v.lower() for k, v in re.findall(r"\.f-(\w+)\{fill:(#[0-9a-fA-F]{6})\}", style)}
    outline = re.search(r"\.o\{fill:(#[0-9a-fA-F]{6})", style).group(1).lower()
    used = {}
    for cls in re.findall(r'<path d="[^"]*" class="f-(\w+)"/>', svg):
        used[cls] = used.get(cls, 0) + 1
    # what each text class sits on, the threshold it must meet, and the name it is reported under
    rules = {
        "ink": ("ground", cols["bg"], AA), "mute": ("ground", cols["bg"], AA),
        "bk": ("paper", cols["paper"], AA), "pmute": ("paper", cols["paper"], AA),
        "paper": ("outline", outline, AA), "fill": ("outline", outline, AA_LARGE),
    }
    rows, fails = [], []
    for cls, n in sorted(used.items()):
        if cls not in rules:
            fails.append((cls, "?", "unknown text class", "", 0.0, 0))
            continue
        on, ground, need = rules[cls]
        r = ratio(cols[cls], ground)
        vs_bg = ratio(cols[cls], cols["bg"])
        rows.append((cls, cols[cls], on, ground, r, need, vs_bg, n))
        if r < need:
            fails.append((cls, cols[cls], on, ground, r, need))
    return rows, fails, cols


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    out = Path(args[0]) if args else Path(tempfile.mkdtemp(prefix="card_palettes_"))
    if "--no-build" not in sys.argv:
        subprocess.run(["cargo", "run", "-q", "-p", "standuser-core", "--example", "card_palettes", "--", str(out)], cwd=ROOT, check=True)
    all_fails, n_checks = [], 0
    print("| accent | card | ground | ink | mute | on paper (bk, pmute) | name vs outline | ゴゴゴ vs outline (vs ground) |\n|---|---|---|---|---|---|---|---|")
    for key in ["PWR", "SPD", "RNG", "STA", "PRC", "DEV", "NONE"]:
        for theme in ("light", "dark"):
            rows, fails, cols = check(out / f"{key}-{theme}.svg")
            n_checks += len(rows)
            by = {c: r for c, _, _, _, r, _, _, _ in rows}
            gg = next(r for c, _, _, _, _, _, r, _ in rows if c == "fill")
            print(f"| {key} | {theme} | {cols['bg']} | {by['ink']:.2f} | {by['mute']:.2f} | {min(by['bk'], by['pmute']):.2f} | {by['paper']:.2f} | {by['fill']:.2f} ({gg:.2f}) |")
            all_fails += [(key, theme) + f for f in fails]
    if all_fails:
        print("\nFAIL:")
        for key, theme, cls, col, on, g, r, need in all_fails:
            print(f"  {key} {theme} card: text colour {cls} {col} on {on} {g} = {r:.2f}, needs {need}")
        sys.exit(1)
    print(f"\nall {n_checks} text colour checks pass WCAG AA (4.5:1; 3:1 for the ゴゴゴ, large display lettering) for 6 accents + none, light and dark")


main()
