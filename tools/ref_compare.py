"""One side-by-side image per reference: the reference on the left, our card on the right (local/review/ref-compare/<name>.png).
Usage: python3 -I tools/ref_compare.py <card_svg_dir> [out_dir]      (needs Pillow, numpy, playwright with a Chromium)
<card_svg_dir> is what `cargo run -p standuser-core --example card_palettes -- <dir>` writes (<KEY>-light.svg, <KEY>-dark.svg).
For each reference image in local/ref it takes the colour of its largest saturated flat fill (tools/ref_palette.py's method), picks the
card (accent and theme) whose ground is nearest that colour, renders it with headless Chromium and puts the two side by side at the same
height. The references stay in local/ (git-ignored); the output goes to local/review and is never committed.
"""
import json, sys
from pathlib import Path

import numpy as np
from PIL import Image
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "local/ref"
KEYS = ["PWR", "SPD", "RNG", "STA", "PRC", "DEV", "NONE"]
Image.MAX_IMAGE_PIXELS = None
H = 1100


def ground_of(svg):
    import re
    return re.search(r"\.f-bg\{fill:(#[0-9a-f]{6})\}", svg.read_text()).group(1)


def rgb(h):
    return np.array([int(h[i:i + 2], 16) for i in (1, 3, 5)], dtype=float)


def main():
    cards = Path(sys.argv[1])
    out = Path(sys.argv[2]) if len(sys.argv) > 2 else ROOT / "local/review/ref-compare"
    out.mkdir(parents=True, exist_ok=True)
    samples = json.loads((ROOT / "local/review/ref-palette/samples.json").read_text())["images"]
    variants = [(f"{k}-{t}", rgb(ground_of(cards / f"{k}-{t}.svg"))) for k in KEYS for t in ("light", "dark")]
    png = {}
    with sync_playwright() as p:
        b = p.chromium.launch()
        pg = b.new_page(viewport={"width": 1080, "height": 1350})
        for name, _ in variants:
            pg.goto(f"file://{cards / (name + '.svg')}")
            pg.screenshot(path=str(out / f".{name}.png"))
            png[name] = Image.open(out / f".{name}.png").convert("RGB")
        b.close()
    for f in sorted(REF.iterdir()):
        if f.name.endswith("Identifier"):
            continue
        try:
            ref = Image.open(f).convert("RGB")
        except Exception:
            continue
        bins = samples.get(f.name, {}).get("bins") or next((v["bins"] for k, v in samples.items() if k.startswith(f.stem[:8])), [])
        sat = [x for x in bins if x["s"] >= 0.6 and x["v"] >= 0.6]
        if sat:
            top = max(sat, key=lambda x: x["area"])["hex"]
            name = min(variants, key=lambda v: np.linalg.norm(v[1] - rgb(top)))[0]
        else:  # a black and white page or a pale one: the reference has no flat saturated fill to follow
            top, name = None, "NONE-light"
        card = png[name].resize((int(1080 * H / 1350), H), Image.LANCZOS)
        s = min(H / ref.height, 1500 / ref.width)
        ref = ref.resize((max(1, int(ref.width * s)), max(1, int(ref.height * s))), Image.LANCZOS)
        sheet = Image.new("RGB", (ref.width + 40 + card.width + 40, H + 40), (236, 236, 236))
        sheet.paste(ref, (20, 20 + (H - ref.height) // 2))
        sheet.paste(card, (ref.width + 40, 20))
        sheet.save(out / f"{f.stem}{'' if f.suffix == '.png' else f.suffix.replace('.', '-')}.png")
        print(f"{f.name[:44]:44} ref fill {top or '-':8} -> card {name}")
    for name, _ in variants:
        (out / f".{name}.png").unlink()


if __name__ == "__main__":
    main()
