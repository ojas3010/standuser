"""Instantiate Noto Sans JP / Noto Serif JP (variable, OFL) at weight 700 and subset them for the card.
Usage: ~/lb_venv/bin/python tools/make_fonts.py <NotoSansJP.ttf> <NotoSerifJP.ttf>
Output: crates/standuser-core/data/sans.ttf, serif.ttf
"""
import io, sys
from pathlib import Path
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer
from fontTools import subset

OUT = Path(__file__).resolve().parent.parent / "crates/standuser-core/data"
chars = set(chr(c) for c in range(0x20, 0x7F))
chars |= set(chr(c) for c in range(0xA0, 0x180))          # Latin-1 + Latin Extended-A
chars |= set("–—‘’“”…•·−×½★∞∅□→←↑↓✓※「」【】・―〜ー")
fixed = ("破壊力スピード射程距離持続力精密動作性成長性超スゴイ人間並ニガテゴ完成本体能力弱点型"
         "遠隔自動操縦近パワー未分類スタンド名")
chars |= set(fixed)
chars |= set("ΑΒΓΔΕΖΗΘΙΚΛΜΝΞΟΠΡΣΤΥΦΧΨΩαβγδεζηθικλμνξοπρστυφχψω")   # Greek, for the odd title
chars |= set(chr(c) for c in range(0x400, 0x460))         # Cyrillic basics

for src, dst, fam in ((sys.argv[1], "sans.ttf", "Noto Sans JP"), (sys.argv[2], "serif.ttf", "Noto Serif JP")):
    f = TTFont(src)
    f = instancer.instantiateVariableFont(f, {"wght": 700}, inplace=False)
    o = subset.Options()
    o.layout_features = []
    o.glyph_names = False
    o.hinting = False
    o.notdef_outline = True
    o.name_IDs = [0, 1, 2, 3, 4, 6, 13, 14]
    o.drop_tables += ["DSIG"]
    s = subset.Subsetter(o)
    s.populate(text="".join(sorted(chars)))
    s.subset(f)
    for r in list(f["name"].names):
        if r.nameID in (1, 16):
            r.string = fam
        if r.nameID in (2, 17):
            r.string = "Bold"
        if r.nameID == 4:
            r.string = fam + " Bold"
    buf = io.BytesIO()
    f.save(buf)
    (OUT / dst).write_bytes(buf.getvalue())
    missing = [c for c in sorted(chars) if ord(c) not in f.getBestCmap()]
    print(dst, len(buf.getvalue()) // 1024, "KB", "missing:", "".join(missing))
