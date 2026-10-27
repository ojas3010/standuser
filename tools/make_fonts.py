"""Instantiate Noto Sans JP (variable, OFL) at weight 700 and subset it for the card.
Usage: ~/lb_venv/bin/python tools/make_fonts.py <NotoSansJP.ttf> <NotoSansKR.ttf> <NotoSansSC.ttf> <NotoSans.ttf>
Output: crates/standuser-core/data/sans.ttf (the display faces are built by tools/make_card_fonts.py)

sans.ttf is the card's Japanese face below 48 px (the stat labels, 本体, the type names, 完成): Noto Sans JP Bold, no outline. The card
draws no Latin from it (Latin is Archivo, see make_card_fonts.py); the page's name choice still measures its why line with its Latin
glyphs, so they stay. Every character of every artist credit in names.json is added to
it, from Noto Sans JP when it has the glyph, else from Noto Sans KR (Hangul), Noto Sans SC (simplified hanzi) or Noto Sans
(Latin Extended, Greek with tonos, the combining diaeresis). The credits are read from names.json, so a rebuild of the names
keeps them drawable; tests/review_font.rs fails if one is not.
"""
import io, json, sys
from pathlib import Path
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer
from fontTools import subset
from fontTools.pens.recordingPen import DecomposingRecordingPen
from fontTools.pens.ttGlyphPen import TTGlyphPen

OUT = Path(__file__).resolve().parent.parent / "crates/standuser-core/data"


def credit_chars():
    names = json.loads((OUT / "names.json").read_text())["names"]
    return {ch for v in names.values() for artist in v[0] for ch in artist[0]}


def instance(path, wdth=None):
    f = TTFont(path)
    axes = {"wght": 700}
    if "wdth" in [a.axisTag for a in f["fvar"].axes]:
        axes["wdth"] = 100
    return instancer.instantiateVariableFont(f, axes, inplace=False)


def cut(f, chars, fam):
    o = subset.Options()
    o.layout_features = []
    o.glyph_names = False
    o.hinting = False
    o.notdef_outline = True
    o.name_IDs = [0, 1, 2, 3, 4, 6, 13, 14]
    o.drop_tables += ["DSIG", "vhea", "vmtx", "VORG"]   # no vertical text on the card
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
    return f


def to_bytes(f):
    buf = io.BytesIO()
    f.save(buf)
    return buf.getvalue()


ascii_ = set(chr(c) for c in range(0x20, 0x7F))
latin1 = set(chr(c) for c in range(0xA0, 0x180))   # Latin-1 + Latin Extended-A (Noto JP lacks most of the latter)
punct = set("–—‘’“”…•·−×½★∞∅□→←↑↓✓※「」【】・―〜ー")
kana = set(chr(c) for c in range(0x3041, 0x3097)) | set(chr(c) for c in range(0x309D, 0x30A0)) | set(chr(c) for c in range(0x30A1, 0x30FB)) | set(chr(c) for c in range(0x30FC, 0x3100))
fixed = set("破壊力スピード射程距離持続力精密動作性成長性超スゴイ人間並ニガテゴ完成本体能力弱点型遠隔自動操縦近パワー未分類スタンド名")
greek = set("ΑΒΓΔΕΖΗΘΙΚΛΜΝΞΟΠΡΣΤΥΦΧΨΩαβγδεζηθικλμνξοπρστυφχψω")   # for the odd title
cyrillic = set(chr(c) for c in range(0x400, 0x460))

jp, kr, sc, latin = sys.argv[1:5]
want = ascii_ | latin1 | punct | kana | fixed | greek | cyrillic | credit_chars()

# sans: the Noto Sans JP subset, plus the glyphs it lacks copied in (outlines decomposed) from the other sources; all use 1000 units per em
def graft(base, donor, chars):
    gs, dcm, bcm = donor.getGlyphSet(), donor.getBestCmap(), base.getBestCmap()
    order = list(base.getGlyphOrder())
    for c in sorted(chars):
        src, name = dcm[ord(c)], "uni%04X" % ord(c)
        rec = DecomposingRecordingPen(gs)
        gs[src].draw(rec)
        pen = TTGlyphPen(None)
        rec.replay(pen)
        base["glyf"][name] = pen.glyph()
        base["hmtx"].metrics[name] = donor["hmtx"].metrics[src]
        order.append(name)
        for t in base["cmap"].tables:
            if t.isUnicode() and ord(c) <= 0xFFFF or (t.isUnicode() and t.format == 12):
                t.cmap[ord(c)] = name
    base.setGlyphOrder(order)
    base["glyf"].glyphOrder = order


sans = cut(instance(jp), want, "Noto Sans JP")
assert sans["head"].unitsPerEm == 1000
for path in (kr, sc, latin):
    f = instance(path)
    assert f["head"].unitsPerEm == 1000
    take = {c for c in want if ord(c) in f.getBestCmap()} - {chr(k) for k in sans.getBestCmap()}
    if take:
        graft(sans, f, take)
cmap = sans.getBestCmap()
cmap = sans.getBestCmap()
still = [c for c in sorted(want) if ord(c) not in cmap]
(OUT / "sans.ttf").write_bytes(to_bytes(sans))
print("sans.ttf", len(to_bytes(sans)) // 1024, "KB; credit characters still missing:", "".join(c for c in still if c in credit_chars()) or "none")

for name, f in (("sans", sans),):
    cm = f.getBestCmap()
    for label, lo, hi in (("Basic Latin", 0x20, 0x7E), ("Latin-1 letters", 0xC0, 0xFF), ("Latin Extended-A", 0x100, 0x17F),
                          ("Greek", 0x391, 0x3C9), ("Cyrillic basics", 0x400, 0x45F), ("Hiragana", 0x3041, 0x3096), ("Katakana", 0x30A1, 0x30FA)):
        letters = [chr(c) for c in range(lo, hi + 1) if chr(c).isalpha() or label == "Basic Latin"]
        have = sum(ord(c) in cm for c in letters)
        print(f"  {name:5s} {label:18s} {have:3d} of {len(letters):3d} drawn")
