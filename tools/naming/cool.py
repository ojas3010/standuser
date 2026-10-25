"""Item 5: Cool(n) in [0,1], computed offline per candidate (docs/DECISIONS.md, Agreed v1 design).

Features, each in [0,1]:
  rarity   mean IDF of the content words, over every distinct candidate name in the candidates table, scaled by ln(N+1)
  shape    1 when a verb, a pronoun or a negation is present (the phrase reads as an act, an address or a refusal)
  conc     mean Glasgow concreteness of the rated content words, (CNC-1)/6, 0.5 when none is rated
  emo      mean Glasgow emotion of the rated content words, (|valence-5|/4 + (arousal-1)/8)/2, 0.5 when none is rated
  prior    the spec's canon word-count prior W = {1: .49, 2: 1, 3: .34, 4: .08, 5: .04}
  proper   1 when every word is unknown to WordNet (a name made only of proper nouns): the lone-proper-name penalty
Shipped Cool: equal weights, raw = rarity + shape + conc + emo + prior - proper, then min-max scaled to [0,1] over the candidates
table (`scale`). No weight set has support in the 24-Stand benchmark, so equal weights are the uninformed default (docs/DECISIONS.md).
`cool(f, w)` is the weighted form the benchmark's grid search uses.
Glasgow Norms: Scott et al. 2019, Behavior Research Methods 51:1258, CC BY 4.0, read from $NAMING_DATA/glasgow (not shipped).
"""
import csv
import math
import os
import re
import sys
from functools import lru_cache

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lexicons as L  # noqa: E402
import textlib as t  # noqa: E402

FEATURES = ["rarity", "shape", "conc", "emo", "prior", "proper"]
W_PRIOR = {1: 0.49, 2: 1.0, 3: 0.34, 4: 0.08, 5: 0.04}
PRONOUNS = set("i me my mine myself you your yours yourself he him his himself she her hers herself it its itself we us our ours ourselves "
               "they them their theirs themselves".split())
NEGATION = re.compile(r"\b(no|not|never|nothing|nobody|none|nowhere|neither|nor|without|cannot)\b|n't\b")
GLASGOW = os.path.join(t.DATA, "glasgow", "13428_2018_1099_MOESM2_ESM.csv")


@lru_cache(maxsize=1)
def glasgow():
    """word -> (concreteness 1-7, valence 1-9, arousal 1-9)."""
    rows = list(csv.reader(open(GLASGOW, encoding="utf-8")))
    out = {}
    for r in rows[2:]:
        try:
            out[r[0].lower()] = (float(r[11]), float(r[5]), float(r[2]))
        except (ValueError, IndexError):
            continue
    return out


def tokens(text):
    return t.alnum_words(text)


def content(toks):
    c = [w for w in toks if w not in L.STOP and len(w) > 1]
    return c or [w for w in toks if len(w) > 1] or toks


def is_verb(w):
    wn = t.wordnet()
    nv, nn = len(wn.synsets(w, pos="v")), len(wn.synsets(w, pos="n"))
    return nv > nn or (w.endswith(("ing", "ed")) and bool(wn.morphy(w, "v")) and nv > 0)


def document_frequencies(names):
    """df over distinct candidate names: word -> number of names containing it."""
    df, n = {}, 0
    for name in set(names):
        n += 1
        for w in set(tokens(name)):
            df[w] = df.get(w, 0) + 1
    return df, n


def features(text, kind, df, n):
    toks = tokens(text)
    cw = content(toks)
    lo = t.fold_ascii(text).lower()
    idf = [math.log((n + 1) / (df.get(w, 0) + 1)) for w in cw]
    rarity = (sum(idf) / len(idf)) / math.log(n + 1) if idf else 0.0
    shape = float(any(w in PRONOUNS for w in toks) or bool(NEGATION.search(lo)) or any(is_verb(w) for w in toks))
    g = glasgow()
    rated = [g[w] for w in (L.lemma(x) if x not in g else x for x in cw) if w in g]
    conc = sum((c - 1) / 6 for c, _, _ in rated) / len(rated) if rated else 0.5
    emo = sum((abs(v - 5) / 4 + (a - 1) / 8) / 2 for _, v, a in rated) / len(rated) if rated else 0.5
    prior = W_PRIOR.get(len(text.split()), 0.04)
    wn = t.wordnet()
    alpha = [w for w in toks if w.isalpha()]
    proper = float(bool(alpha) and all(not t.wn_pos(w) for w in alpha)) or float(kind == "a")
    return dict(rarity=rarity, shape=shape, conc=conc, emo=emo, prior=prior, proper=proper)


def cool(f, w):
    x = sum(w.get(k, 0.0) * f[k] for k in FEATURES if k != "proper") - w.get("proper", 0.0) * f["proper"]
    return min(1.0, max(0.0, x))


EQUAL = {k: 1.0 for k in FEATURES}


def raw_equal(f):
    """Equal weights, the lone-proper-name penalty negative: the sum of the five features minus `proper`, in [-1, 5]."""
    return sum(f[k] for k in FEATURES if k != "proper") - f["proper"]


def scale(raw, lo, hi):
    """Min-max scale to [0,1] with the bounds observed on the candidates table; values outside are clipped."""
    return min(1.0, max(0.0, (raw - lo) / (hi - lo)))
