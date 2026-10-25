"""Subset Overpass (variable, OFL) to Latin for the page chrome and write site/fonts/overpass.woff2.
Usage: ~/lb_venv/bin/python tools/make_site_font.py <Overpass[wght].ttf> <OFL.txt>
The weight axis is limited to 400-800 (body 400, buttons 600, the wordmark 800). Characters outside the subset fall back to the
system sans; the page never requests a font from anywhere but its own folder.
"""
import shutil, sys
from pathlib import Path
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer
from fontTools import subset

OUT = Path(__file__).resolve().parent.parent / "site/fonts"
OUT.mkdir(exist_ok=True)
f = TTFont(sys.argv[1])
f = instancer.instantiateVariableFont(f, {"wght": (400, 800)}, inplace=False)
chars = "".join(chr(c) for c in range(0x20, 0x7F)) + "".join(chr(c) for c in range(0xA0, 0x100)) + "–—‘’“”…•·−×★→"
o = subset.Options()
o.flavor = "woff2"
o.layout_features = ["kern", "liga", "calt", "ccmp", "locl", "mark", "mkmk", "tnum", "lnum", "pnum"]
o.name_IDs = [0, 1, 2, 3, 4, 6, 13, 14]
o.notdef_outline = True
s = subset.Subsetter(o)
s.populate(text=chars)
s.subset(f)
f.flavor = "woff2"
f.save(OUT / "overpass.woff2")
shutil.copy(sys.argv[2], OUT / "OFL-Overpass.txt")
print("overpass.woff2", (OUT / "overpass.woff2").stat().st_size, "bytes")
