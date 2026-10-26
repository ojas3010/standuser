"""Build the card's three display and body fonts (all SIL OFL, embedded as glyph outlines like the sans, so the SVG needs no font file).
Usage: python3 -I tools/make_card_fonts.py <DelaGothicOne-Regular.ttf> <ArchivoBlack-Regular.ttf> <Archivo[wdth,wght].ttf> \\
           <OFL-DelaGothicOne.txt> <OFL-ArchivoBlack.txt> <OFL-Archivo.txt> <OFL-NotoSansJP.txt>
Output (crates/standuser-core/data/): display.ttf, name.ttf, body.ttf and the four licence texts. sans.ttf (Noto Sans JP Bold) comes from
tools/make_fonts.py.

The card's four faces, and what each is for (docs/DESIGN.md section 4, rule 7):
display.ttf  Dela Gothic One, subset: kana, the kanji of the labels and the Stand types, 【】「」・―. Japanese at 48 px or larger, with the
             black outline (the ゴ). No Latin: the Latin display text is Archivo Black.
name.ttf     Archivo Black, subset: Basic Latin, Latin-1, Latin Extended-A and punctuation. The Stand name and the other Latin display text
             (the ability's name, the grades). Archivo Black has no 「」, so those two glyphs are copied from Dela Gothic One (both 1000 units per
             em), and no ∅, which is copied from Archivo at weight 900 (the same family).
body.ttf     Archivo (variable font) cut to weight 700, width 100, same character set. All other Latin text.
sans.ttf     Noto Sans JP Bold, no outline. Japanese below 48 px (made by make_fonts.py; its licence is OFL-NotoSansJP.txt).
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


dela_p, archivo_p, var_p, ofl_d, ofl_a, ofl_v, ofl_n = sys.argv[1:8]
dela_full, archivo_full = TTFont(dela_p), TTFont(archivo_p)
from fontTools.varLib import instancer
def archivo(w):
    return instancer.instantiateVariableFont(TTFont(var_p), {"wght": w, "wdth": 100}, inplace=False)
heavy = archivo(900)
assert dela_full["head"].unitsPerEm == 1000 and archivo_full["head"].unitsPerEm == 1000
display = cut(TTFont(dela_p), (punct | kana | kanji) & set(chr(k) for k in dela_full.getBestCmap()), "Dela Gothic One")
(OUT / "display.ttf").write_bytes(to_bytes(display))
want = ascii_ | latin | punct
name = cut(archivo_full, {c for c in want if ord(c) in archivo_full.getBestCmap()}, "Archivo Black")
graft(name, dela_full, {c for c in want if ord(c) not in name.getBestCmap() and ord(c) in dela_full.getBestCmap() and c in "「」【】"})
graft(name, heavy, {c for c in "∅" if ord(c) in heavy.getBestCmap() and ord(c) not in name.getBestCmap()})
(OUT / "name.ttf").write_bytes(to_bytes(name))
bold = archivo(700)
body = cut(bold, {c for c in want if ord(c) in bold.getBestCmap()}, "Archivo")
(OUT / "body.ttf").write_bytes(to_bytes(body))
shutil.copy(ofl_d, OUT / "OFL-DelaGothicOne.txt")
shutil.copy(ofl_a, OUT / "OFL-ArchivoBlack.txt")
shutil.copy(ofl_v, OUT / "OFL-Archivo.txt")
shutil.copy(ofl_n, OUT / "OFL-NotoSansJP.txt")
for label, f in (("display", display), ("name", name), ("body", body)):
    cm = f.getBestCmap()
    print(f"{label}.ttf {len(to_bytes(f)) // 1024} KB, {len(cm)} characters; Basic Latin drawn: {sum(ord(c) in cm for c in ascii_)} of {len(ascii_)}")
print("name.ttf has 「」 and ∅:", all(ord(c) in name.getBestCmap() for c in "「」∅"), "…:", ord("…") in name.getBestCmap())
