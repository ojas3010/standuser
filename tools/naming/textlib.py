"""Stand-name candidates from track titles: spec stages 5 and 6 (docs/naming-spec.md), offline.

Stage 5: normalise, bilingual titles, strip brackets and subtitles, split cue sheets, verbatim or tail-truncated.
Stage 6: hard filters, including the canon-collision filter. Pure functions; the only data they read are WordNet (noun test
for the tail rule), the LDNOOBW list (profanity) and the canon name list.

Data directory: $NAMING_DATA or ~/naming_data (WordNet in nltk_data/, ldnoobw_en.txt). Nothing from it is shipped.
"""
import csv
import os
import re
import unicodedata
from functools import lru_cache

DATA = os.environ.get("NAMING_DATA", os.path.expanduser("~/naming_data"))
HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))

MAX_CHARS = 28
MAX_WORDS = 5

FUNCTION_WORDS = set("""a an the of in on at by for from with without to into onto over under about after before between through during
against among around behind beyond down up off out near upon within toward towards across along like than per via
i me my mine myself you your yours he him his she her hers it its we us our ours they them their theirs this that these those
who whom whose which what another one and or but nor so if as when while because
be am is are was were been being have has had having do does did doing done
can could will would shall should may might must""".split())


# ---------------------------------------------------------------- WordNet
@lru_cache(maxsize=1)
def wordnet():
    import nltk
    path = os.path.join(DATA, "nltk_data")
    if path not in nltk.data.path:
        nltk.data.path.insert(0, path)
    from nltk.corpus import wordnet as wn
    wn.ensure_loaded()
    return wn


@lru_cache(maxsize=None)
def wn_pos(word):
    """The set of WordNet parts of speech ('n','v','a','r') the lower-case word has, through morphy."""
    wn = wordnet()
    w = word.lower()
    return frozenset(p for p in ("n", "v", "a", "r") if wn.morphy(w, p) is not None or (p == "a" and wn.morphy(w, "s") is not None))


def is_noun(word):
    """Noun test for the tail rule. WordNet noun, or a word WordNet does not know at all (a capitalised title word that is not
    a common word is a proper noun). A function word is never a noun, whatever WordNet says of it (it does not know "the")."""
    w = re.sub(r"[^A-Za-z']", "", word)
    if not w or w.lower() in FUNCTION_WORDS:
        return False
    pos = wn_pos(w)
    return "n" in pos or not pos


# ---------------------------------------------------------------- folding
_SPECIAL = {"ø": "o", "Ø": "O", "æ": "ae", "Æ": "AE", "œ": "oe", "Œ": "OE", "ß": "ss", "ẞ": "SS", "đ": "d", "Đ": "D", "ð": "d", "Ð": "D",
            "þ": "th", "Þ": "TH", "ł": "l", "Ł": "L", "ı": "i", "ħ": "h", "Ħ": "H"}
_PUNCT = {"‘": "'", "’": "'", "`": "'", "´": "'", "“": '"', "”": '"', "–": "-", "—": "-", "−": "-",
          "‐": "-", "‑": "-", "‒": "-", "―": "-", "…": "..."}


def fold_ascii(s):
    """NFKD, accents removed, special letters spelled out, curly quotes and dashes made ASCII. Case is kept."""
    out = []
    for c in unicodedata.normalize("NFKD", s):
        if unicodedata.combining(c):
            continue
        out.append(_SPECIAL.get(c) or _PUNCT.get(c) or c)
    return "".join(out)


DRAWABLE = re.compile(r"^[A-Z0-9 .'\"\-&,!?]+$")


def display_name(s):
    """Upper-case name the card font can draw (the same alphabet as name::ascii_fold in the Rust core), or None."""
    t = " ".join(fold_ascii(s).upper().split())
    return t if t and DRAWABLE.match(t) and re.search(r"[A-Z0-9]", t) else None


def alnum_words(s):
    return re.findall(r"[a-z0-9]+", fold_ascii(s).lower())


def alnum_key(s):
    return "".join(alnum_words(s))


# ---------------------------------------------------------------- stage 5
def _latin_ratio(s):
    letters = [c for c in s if c.isalpha()]
    return sum(c.isascii() for c in letters) / len(letters) if letters else 0.0


def bilingual(title):
    """'native = English' keeps the Latin side; a title that is only CJK/other script has no Latin side and is dropped."""
    if " = " in title:
        for part in title.split(" = "):
            if _latin_ratio(part) >= 0.8 and any(c.isascii() and c.isalpha() for c in part):
                return part.strip()
        return None
    return title


_BRACKETS = re.compile(r"\([^()]*\)|\[[^\[\]]*\]")
_FEAT = re.compile(r"\s+(?:feat\.?|ft\.?|featuring)\s.*$", re.I)


def strip_title(title):
    """Remove (...) and [...], then split cue-sheet merges on ' / '. Returns (parts, changed) where parts are
    (text, split) pairs with anything after ' - ' or ': ' cut off."""
    t = fold_ascii(title)
    prev = None
    while prev != t:
        prev = t
        t = _BRACKETS.sub(" ", t)
    t = " ".join(t.split())
    pieces = [p.strip() for p in t.split(" / ")] if " / " in t else [t]
    split = len(pieces) > 1
    parts = []
    for p in pieces:
        p = re.split(r" - |: ", p)[0]
        p = _FEAT.sub("", p).strip(" -:,;")
        if p:
            parts.append((p, split))
    return parts


def words(s):
    return s.split()


def tail_span(text):
    """Shortest 2- or 3-word tail that does not start with a function word and ends in a noun, else None."""
    w = words(text)
    for n in (2, 3):
        if len(w) <= n:
            continue
        span = w[-n:]
        first = re.sub(r"[^a-z']", "", span[0].lower())
        if first in FUNCTION_WORDS:
            continue
        if is_noun(span[-1]):
            return " ".join(span)
    return None


def stage5(title):
    """Candidates from one track title: [(text, kind)] with kind v (verbatim), s (brackets or subtitle stripped),
    c (one cue of a merged title), t (tail of a longer title)."""
    t0 = bilingual(title)
    if t0 is None:
        return []
    plain = " ".join(fold_ascii(t0).split())
    out = []
    for text, split in strip_title(t0):
        changed = " ".join(text.split()).lower() != plain.lower()
        if len(words(text)) <= MAX_WORDS and len(text) <= MAX_CHARS:
            kind = "c" if split else ("s" if changed else "v")
            out.append((text, kind))
        else:
            tail = tail_span(text)
            if tail and len(tail) <= MAX_CHARS:
                out.append((tail, "t"))
    return out


# ---------------------------------------------------------------- stage 6
GENERIC = re.compile(r"(?i)^(main|end|opening|closing|title|love|final|finale|intro|outro)( titles?| credits| theme| music)?$")
BAD_WORDS = re.compile(r"(?i)\b(suites?|overtures?|prologues?|epilogues?|interludes?|reprises?|medleys?|variations?|trailers?|untitled|cues?)\b")
THEME_FROM = re.compile(r"(?i)\btheme from\b")
THEME = re.compile(r"(?i)\btheme\b")
POSS_THEME = re.compile(r"^([A-Z][A-Za-z.'\-]*)'s Theme$")
TRAIL_NUM = re.compile(r"(?:^|\s)\d{1,2}$")
TRAIL_ROMAN = re.compile(r"(?:^|\s)(?=[IVXLC]{2,}$)(?:XC|XL|L?X{0,3})(?:IX|IV|V?I{0,3})$")
TRAIL_PART = re.compile(r"(?i)\b(?:part|pt\.?)\s*(?:\d+|[ivxlc]+|one|two|three|four|five|six|seven|eight|nine|ten)$")
CODE_START = re.compile(r"(?i)^(?:\d+m\d+|track \d+$)")


@lru_cache(maxsize=1)
def profanity():
    path = os.path.join(DATA, "ldnoobw_en.txt")
    words_ = [w.strip().lower() for w in open(path, encoding="utf-8") if w.strip()]
    # hand list: slurs LDNOOBW lacks or spells differently
    words_ += ["nigga", "nigger", "faggot", "retard", "retarded", "chink", "spic", "kike", "tranny"]
    return [re.compile(r"(?<![a-z0-9])" + re.escape(w) + r"(?![a-z0-9])") for w in sorted(set(words_))]


SENSITIVE_FILE = os.path.join(HERE, "data", "sensitive_phrases.txt")


@lru_cache(maxsize=1)
def sensitive_phrases():
    """Compiled whole-phrase patterns of tools/naming/data/sensitive_phrases.txt, matched against the folded, punctuation-free words."""
    lines = [l.strip().lower() for l in open(SENSITIVE_FILE, encoding="utf-8")]
    return [re.compile(r"(?<![a-z0-9])" + re.escape(" ".join(l.split())) + r"(?![a-z0-9])") for l in lines if l and not l.startswith("#")]


def is_sensitive(text):
    low = " ".join(alnum_words(text))
    return any(p.search(low) for p in sensitive_phrases())


def _canon_norm(name):
    w = alnum_words(name)
    if w and w[0] == "the" and len(w) > 1:
        w = w[1:]
    if w:
        last = w[-1]
        if last.endswith("ies") and len(last) > 4:
            last = last[:-3] + "y"
        elif last.endswith("s") and not last.endswith("ss") and len(last) > 3:
            last = last[:-1]
        w[-1] = last
    return "".join(w)


def edit_le1(a, b):
    if abs(len(a) - len(b)) > 1:
        return False
    if len(a) == len(b):
        return sum(x != y for x, y in zip(a, b)) <= 1
    if len(a) > len(b):
        a, b = b, a
    i = 0
    while i < len(a) and a[i] == b[i]:
        i += 1
    return a[i:] == b[i + 1:]


@lru_cache(maxsize=1)
def canon_names():
    """The 157 canon names the Rust core embeds (crates/standuser-core/data/canon_names.txt), normalised."""
    path = os.path.join(REPO, "crates", "standuser-core", "data", "canon_names.txt")
    names = [l.strip() for l in open(path, encoding="utf-8") if l.strip()]
    return names


@lru_cache(maxsize=1)
def canon_norms():
    return sorted({_canon_norm(re.split(r" \(", n)[0]) for n in canon_names()})


def collides_with_canon(name):
    n = _canon_norm(name)
    for c in canon_norms():
        if c == n or (len(n) >= 8 and edit_le1(c, n)):
            return True
    return False


def film_title_hit(cand, film_titles):
    """The candidate contains the film title, or a subtitle of it, as a run of whole words."""
    cw = alnum_words(cand)
    for ft in film_titles:
        if not ft:
            continue
        pieces = {ft} | {p for p in re.split(r": | - ", ft) if p}
        for p in pieces:
            pw = alnum_words(p)
            if not pw:
                continue
            for i in range(len(cw) - len(pw) + 1):
                if cw[i:i + len(pw)] == pw:
                    return True
    return False


def is_proper_x(x, film_titles):
    """For the "X's Theme" exception: X is capitalised, not a common word, and not part of the film title."""
    if x.lower() in FUNCTION_WORDS or wn_pos(x):
        return False
    return not any(x.lower() in alnum_words(ft) for ft in film_titles if ft)


def stage6(text, film_titles=(), canon=True, film=True, profane=True, sensitive=True):
    """Return the first filter that rejects `text`, or None when it survives."""
    t = text.strip()
    poss = POSS_THEME.match(t)
    poss_ok = bool(poss) and is_proper_x(poss.group(1), film_titles)
    if GENERIC.match(t) and not poss_ok:
        return "generic"
    if BAD_WORDS.search(t):
        return "cue-word"
    if (THEME_FROM.search(t) or THEME.search(t)) and not poss_ok:
        return "theme"
    if TRAIL_NUM.search(t) or TRAIL_ROMAN.search(t) or TRAIL_PART.search(t) or CODE_START.match(t):
        return "number"
    toks = t.split()
    if toks and re.fullmatch(r"[A-Z]{3,}", re.sub(r"[^A-Za-z]", "", toks[0])) and toks[0].isupper() and len(re.sub(r"[^A-Za-z]", "", toks[0])) >= 3:
        rest = " ".join(toks[1:])
        if re.search(r"[a-z]", rest):
            return "caps-start"
    core = [c for c in t if not c.isspace() and not re.match(r"[^\w]", c)]
    if core and sum(c.isascii() and c.isalnum() for c in core) / len(core) < 0.8:
        return "non-ascii"
    if profane:
        low = " ".join(alnum_words(t))
        if any(p.search(low) for p in profanity()):
            return "profanity"
    if sensitive and is_sensitive(t):
        return "sensitive"
    if film and film_title_hit(t, film_titles):
        return "film-title"
    if canon and collides_with_canon(t):
        return "canon"
    return None


def extra_checks(text):
    """Rules this build adds to stage 6: drawable on the card, within the 28-character cap, at least three letters or digits."""
    d = display_name(text)
    if d is None:
        return "undrawable"
    if len(d) > MAX_CHARS:
        return "too-long"
    if len(re.findall(r"[A-Z0-9]", d)) < 3:
        return "too-short"
    return None


def candidates_for_album(tracks, film_titles=(), canon=True, film=True, rejects=None):
    """All surviving candidates of one album: dicts with name (display form), text, src (source title), track (1-based),
    kind. A name already produced by an earlier track of the album is skipped. `rejects`, if a list, receives the reason of
    every candidate stage 6 or the card checks threw out."""
    seen, out = set(), []
    for i, title in enumerate(tracks, 1):
        for text, kind in stage5(title):
            why = stage6(text, film_titles, canon=canon, film=film) or extra_checks(text)
            if why:
                if rejects is not None:
                    rejects.append(why)
                continue
            d = display_name(text)
            if d in seen:
                continue
            seen.add(d)
            out.append({"name": d, "text": text, "src": title, "track": i, "kind": kind})
    return out


def load_canon_rows():
    p = os.path.join(os.path.dirname(REPO), "research_notes", "Stand naming algorithm", "canon_name_sources.csv")
    return list(csv.DictReader(open(p, encoding="utf-8")))
