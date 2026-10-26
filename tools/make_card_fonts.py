"""Build the card's two display fonts (both SIL OFL, embedded as glyph outlines like the sans, so the SVG needs no font file).
Usage: python3 -I tools/make_card_fonts.py <DelaGothicOne-Regular.ttf> <ArchivoBlack-Regular.ttf> <OFL-DelaGothicOne.txt> <OFL-ArchivoBlack.txt>
Output (crates/standuser-core/data/): display.ttf, name.ttf, OFL-DelaGothicOne.txt, OFL-ArchivoBlack.txt

display.ttf  Dela Gothic One, subset: the katakana of the manga lettering (ゴゴゴ, スピード, the legend), the kanji of the stat labels and the
             Stand types, Basic Latin, and the punctuation the labels use (【】「」・―). All JP lettering on the card that is not body text.
name.ttf     Archivo Black, subset: Basic Latin, Latin-1, Latin Extended-A and punctuation, for the Stand name. Archivo Black has no 「」, so those
             two glyphs are copied from Dela Gothic One (both 1000 units per em). The face was picked against the Part 3, 5 and 6 logos
             (docs/ref-compare notes): squared, wide, very heavy; the card's one tilt gives it the logos' slant.
"""
import io, shutil, sys
from pathlib import Path
from fontTools.ttLib import TTFont
from fontTools import subset
from fontTools.pens.recordingPen import DecomposingRecordingPen
from fontTools.pens.ttGlyphPen import TTGlyphPen

OUT = Path(__file__).resolve().parent.parent / "crates/standuser-core/data"

ascii_ = set(chr(c) for c in range(0x20, 0x7F))
latin = set(chr(c) for c in range(0xA0, 0x180))
punct = set("–—‘’“”…•·−×★∞∅□→←↑↓✓※「」【】・―〜ー")
kana = set(chr(c) for c in range(0x3041, 0x3097)) | set(chr(c) for c in range(0x309D, 0x30A0)) | set(chr(c) for c in range(0x30A1, 0x30FB)) | set(chr(c) for c in range(0x30FC, 0x3100))
kanji = set("破壊力スピード射程距離持続力精密動作性成長性超スゴイ人間並ニガテゴ完成本体能力弱点型遠隔自動操縦近パワー未分類スタンド名")


def cut(f, chars, fam):
    o = subset.Options()
    o.layout_features = []
    o.glyph_names = False
    o.hinting = False
    o.notdef_outline = True
    o.name_IDs = [0, 1, 2, 3, 4, 6, 13, 14]
    o.drop_tables += ["DSIG", "vhea", "vmtx", "VORG"]
    s = subset.Subsetter(o)
    s.populate(text="".join(sorted(chars)))
    s.subset(f)
    for r in list(f["name"].names):
        if r.nameID in (1, 16):
            r.string = fam
        if r.nameID == 4:
            r.string = fam + " Regular"
    return f


def graft(base, donor, chars):
    gs, dcm = donor.getGlyphSet(), donor.getBestCmap()
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
            if t.isUnicode():
                t.cmap[ord(c)] = name
    base.setGlyphOrder(order)
    base["glyf"].glyphOrder = order


def to_bytes(f):
    buf = io.BytesIO()
    f.save(buf)
    return buf.getvalue()


dela_p, archivo_p, ofl_d, ofl_a = sys.argv[1:5]
dela_full, archivo_full = TTFont(dela_p), TTFont(archivo_p)
assert dela_full["head"].unitsPerEm == 1000 and archivo_full["head"].unitsPerEm == 1000
display = cut(TTFont(dela_p), ascii_ | latin & set(chr(k) for k in dela_full.getBestCmap()) | punct | kana | kanji, "Dela Gothic One")
(OUT / "display.ttf").write_bytes(to_bytes(display))
want = ascii_ | latin | punct
name = cut(archivo_full, {c for c in want if ord(c) in archivo_full.getBestCmap()}, "Archivo Black")
graft(name, dela_full, {c for c in want if ord(c) not in name.getBestCmap() and ord(c) in dela_full.getBestCmap() and c in "「」【】"})
(OUT / "name.ttf").write_bytes(to_bytes(name))
shutil.copy(ofl_d, OUT / "OFL-DelaGothicOne.txt")
shutil.copy(ofl_a, OUT / "OFL-ArchivoBlack.txt")
for label, f in (("display", display), ("name", name)):
    cm = f.getBestCmap()
    print(f"{label}.ttf {len(to_bytes(f)) // 1024} KB, {len(cm)} characters; Basic Latin drawn: {sum(ord(c) in cm for c in ascii_)} of {len(ascii_)}")
print("name.ttf has 「」:", all(ord(c) in name.getBestCmap() for c in "「」"), "…:", ord("…") in name.getBestCmap())
