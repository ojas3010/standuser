"""Sample the card palette from the JoJo reference images in local/ref/ (kept out of git; this script reads them and writes numbers only).
Usage: python3 -I tools/ref_palette.py [out.json]        (needs Pillow and numpy; default out: local/review/ref-palette/samples.json)
Method per image: resize to 200 px on the long side (box filter); every pixel with saturation >= 0.45 and value >= 0.40 goes in a
histogram of 24 hue x 6 saturation x 6 value bins; each bin's colour is the mean of its pixels, its area the share of the whole image. The
largest bins are the cover's flat grounds and big fills (black and white outlines, paper, skin and shadow fall out by the floors). Bins
within 18 degrees of hue and 0.2 of saturation and value of a larger bin are merged into it.
Then one ground is picked per hue family (the six accent families of palette.rs): the largest-area bin inside the family's hue band that
is saturated (s >= 0.6, v >= 0.55). `picks` is what docs/ref-palette.json carries and what a test in palette.rs compares with.
"""
import json, sys
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "local/ref"
Image.MAX_IMAGE_PIXELS = None

# hue bands (degrees) of the six accent families, in the chart order PWR SPD RNG STA PRC DEV
FAMILIES = {
    "PWR": (322, 360, "hot pink"),
    "SPD": (38, 62, "yellow"),
    "RNG": (180, 232, "blue and teal"),
    "STA": (85, 165, "green"),
    "PRC": (262, 321, "purple and magenta"),
    "DEV": (14, 37, "orange"),
}


def hsv(rgb):
    r, g, b = [x / 255 for x in rgb]
    mx, mn = max(r, g, b), min(r, g, b)
    d = mx - mn
    if d == 0:
        h = 0.0
    elif mx == r:
        h = (60 * ((g - b) / d)) % 360
    elif mx == g:
        h = 60 * ((b - r) / d) + 120
    else:
        h = 60 * ((r - g) / d) + 240
    return h, (0 if mx == 0 else d / mx), mx


def hexc(rgb):
    return "#%02x%02x%02x" % tuple(int(round(x)) for x in rgb)


def load(path):
    im = Image.open(path).convert("RGB")
    w, h = im.size
    s = 200 / max(w, h)
    return np.asarray(im.resize((max(1, int(w * s)), max(1, int(h * s))), Image.BOX), dtype=float)


def modes(a, top=8):
    px = a.reshape(-1, 3)
    hs = np.array([hsv(p) for p in px])
    idx = np.where((hs[:, 1] >= 0.45) & (hs[:, 2] >= 0.40))[0]
    key = (np.minimum(hs[idx, 0] // 15, 23) * 36 + np.minimum(hs[idx, 1] // (1 / 6), 5) * 6 + np.minimum(hs[idx, 2] // (1 / 6), 5)).astype(int)
    bins = []
    for k in np.unique(key):
        m = idx[key == k]
        bins.append((len(m) / len(px), px[m].mean(0), hs[m].mean(0)))
    bins.sort(key=lambda t: -t[0])
    out = []
    for ar, rgb, (h, s, v) in bins:
        for o in out:
            dh = min(abs(o["h"] - h), 360 - abs(o["h"] - h))
            if dh <= 18 and abs(o["s"] - s) <= 0.2 and abs(o["v"] - v) <= 0.2:
                o["area"] = round(o["area"] + ar, 3)
                break
        else:
            out.append({"hex": hexc(rgb), "area": round(ar, 3), "h": round(float(h)), "s": round(float(s), 2), "v": round(float(v), 2)})
    return sorted(out, key=lambda o: -o["area"])[:top]


def main():
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "local/review/ref-palette/samples.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    seen, report, pool = set(), {}, []
    for f in sorted(REF.iterdir()):
        if f.name.endswith("Identifier"):
            continue
        try:
            a = load(f)
        except Exception as e:
            print("skip", f.name, e)
            continue
        sig = hash(a.round(-1).tobytes())
        if sig in seen:  # Volume_41.webp and Volume_977.docx are copies of Volume_41.png and Volume_97.png
            print("duplicate of an earlier image:", f.name)
            continue
        seen.add(sig)
        clusters = modes(a)
        for o in clusters:
            pool.append((f.name, o["hex"], o["area"], o["h"], o["s"], o["v"]))
        report[f.name] = {"bins": clusters}
        print(f"{f.name[:42]:42} " + " ".join(f"{x['hex']}({x['area']:.2f})" for x in clusters[:5]))
    picks = {}
    for key, (lo, hi, label) in FAMILIES.items():
        cand = sorted((p for p in pool if lo <= p[3] <= hi and p[4] >= 0.6 and p[5] >= 0.55), key=lambda p: -p[2])
        picks[key] = {
            "family": label, "from": cand[0][0], "hex": cand[0][1], "area": cand[0][2], "hsv": [cand[0][3], cand[0][4], cand[0][5]],
            "runners_up": [{"from": p[0], "hex": p[1], "area": p[2]} for p in cand[1:5]],
        }
    json.dump({"images": report, "picks": picks}, out.open("w"), indent=1)
    print("\npicks (the largest flat saturated fill of each family):")
    for k, v in picks.items():
        print(f"  {k} {v['hex']}  area {v['area']:.2f}  {v['from']}  hsv={v['hsv']}   next: " + " ".join(r["hex"] for r in v["runners_up"]))
    print("wrote", out)


if __name__ == "__main__":
    main()
