"""Instantiate Noto Serif JP (variable, OFL) at weight 700 and subset it for the card's header and corner.
Usage: ~/lb_venv/bin/python tools/make_serif_font.py <NotoSerifJP[wght].ttf>
Output: crates/standuser-core/data/serif.ttf

serif.ttf is the face of the anime eyecatch's lettering (docs/DESIGN.md section 4): the Stand name, 【STAND NAME】, 【STAND MASTER】 and the
user name under it. It holds every character of every Stand name and artist credit in names.json (read from the file, so a rebuild of the
names keeps them drawable; tests/review_font.rs fails if one is not), the card's fixed labels, and Basic Latin, Latin-1 and Latin
Extended-A for a user name. A character Noto Serif JP lacks (Hangul and simplified hanzi in a few credits) is not drawn from it: the
card draws a user name in the sans for such a character, and no Stand name has one.
"""
import io, json, sys
from pathlib import Path
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer
from fontTools import subset

OUT = Path(__file__).resolve().parent.parent / "crates/standuser-core/data"


def names_chars():
    names = json.loads((OUT / "names.json").read_text())["names"]
    chars = set()
    for v in names.values():
        chars |= {ch for artist in v[0] for ch in artist[0]}
        chars |= {ch for cand in v[1] for ch in cand[0]}
    return chars


ascii_ = set(chr(c) for c in range(0x20, 0x7F))
latin = set(chr(c) for c in range(0xA0, 0x180))
punct = set("–—‘’“”…•·−×★∞∅□→←↑↓✓※「」【】・―〜ー")
labels = set("【STAND NAME】【STAND MASTER】")
want = ascii_ | latin | punct | labels | names_chars()

f = instancer.instantiateVariableFont(TTFont(sys.argv[1]), {"wght": 700}, inplace=False)
assert f["head"].unitsPerEm == 1000
have = {chr(c) for c in f.getBestCmap()}
o = subset.Options()
o.layout_features = []
o.glyph_names = False
o.hinting = False
o.notdef_outline = True
o.name_IDs = [0, 1, 2, 3, 4, 6, 13, 14]
o.drop_tables += ["DSIG", "vhea", "vmtx", "VORG"]   # no vertical text on the card
s = subset.Subsetter(o)
s.populate(text="".join(sorted(want & have)))
s.subset(f)
for r in list(f["name"].names):
    if r.nameID in (1, 16):
        r.string = "Noto Serif JP"
    if r.nameID in (2, 17):
        r.string = "Bold"
    if r.nameID == 4:
        r.string = "Noto Serif JP Bold"
buf = io.BytesIO()
f.save(buf)
(OUT / "serif.ttf").write_bytes(buf.getvalue())
missing = sorted(want - have)
print("serif.ttf", len(buf.getvalue()) // 1024, "KB;", len(want & have), "characters;", len(missing), "wanted characters Noto Serif JP lacks:", "".join(missing)[:80])
