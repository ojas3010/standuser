"""Subset Archivo Black and Archivo (variable, both OFL) to Latin for the page chrome and write site/fonts/*.woff2.
Usage: python3 -I tools/make_site_font.py <ArchivoBlack-Regular.ttf> <Archivo[wdth,wght].ttf> <OFL-ArchivoBlack.txt> <OFL-Archivo.txt>
archivo-black.woff2  Archivo Black: the wordmark and the headings.
archivo.woff2        Archivo, weight axis 400-800, width fixed at 100: body text (400), buttons and labels (600), emphasis (700).
Characters outside the subset fall back to the system sans; the page never requests a font from anywhere but its own folder.
"""
import shutil, sys
from pathlib import Path
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer
from fontTools import subset

OUT = Path(__file__).resolve().parent.parent / "site/fonts"
OUT.mkdir(exist_ok=True)
chars = "".join(chr(c) for c in range(0x20, 0x7F)) + "".join(chr(c) for c in range(0xA0, 0x180)) + "–—‘’“”…•·−×→"


def cut(f, name):
    o = subset.Options()
    o.flavor = "woff2"
    o.layout_features = ["kern", "liga", "calt", "ccmp", "locl", "mark", "mkmk", "tnum", "lnum", "pnum"]
    o.name_IDs = [0, 1, 2, 3, 4, 6, 13, 14]
    o.notdef_outline = True
    s = subset.Subsetter(o)
    s.populate(text=chars)
    s.subset(f)
    f.flavor = "woff2"
    f.save(OUT / name)
    print(name, (OUT / name).stat().st_size, "bytes")


black = TTFont(sys.argv[1])
cut(black, "archivo-black.woff2")
body = instancer.instantiateVariableFont(TTFont(sys.argv[2]), {"wght": (400, 800), "wdth": 100}, inplace=False)
cut(body, "archivo.woff2")
shutil.copy(sys.argv[3], OUT / "OFL-ArchivoBlack.txt")
shutil.copy(sys.argv[4], OUT / "OFL-Archivo.txt")
