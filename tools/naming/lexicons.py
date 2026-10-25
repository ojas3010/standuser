"""Item 4: the 17 core lexicons, their one-hop WordNet rings, and the lexicon-only fit L (docs/DECISIONS.md, Agreed v1 design).

Usage: python3 -I tools/naming/lexicons.py TEMPLATES_JSON [--md docs/naming-lexicons.md] [--json tools/naming/data/lexicons.json]
TEMPLATES_JSON is the output of `cargo run -p standuser-core --example dump_templates`.
Core words are the content words of each template's name, canon ability quote and card sentence (in that order, quote words
that only restate the canon Stand's own name left out), at most 30, at least 10, each with the clause it came from.
The ring is every single-word lemma one WordNet hop from the first sense of a core lemma (per part of speech) (same synset, hypernym, hyponym, similar,
also-see, attribute, derivational form, member or part relative). L = 1 for a core lemma, 0.5 for a ring lemma, else 0.
"""
import argparse
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import textlib as t  # noqa: E402

MIN_CORE, MAX_CORE = 10, 30

# words that carry no ability meaning: function words, auxiliaries and the card's own metric vocabulary
STOP = t.FUNCTION_WORDS | set("""can could will would may might must shall should is are was were be been being am do does did done has have had
not no yes only usually every each any all some such very more most less least just also even still ever never always much many
able provided involved right near far kind sort own same other others whatever whoever anything everything something nothing
film films rate rates rating crowd average across month months active peak peaks peaking stat stats user effective production credit
credits pair pairs compare comparable difficulty adjusted percentile strongest sits stands standing nothing time times
star stars rank ranks ranking
below above itself too few off rather until since once else almost ten seven second seconds ability activation become becomes come comes
keep hold run long unite united state states new stand
limit limited page heading lists source wiki jojo
director precision score name see close answer agree being
there several cannot however himself enough eventually possible direct current way main face work two polnareff sale johngalli fugo
weakness unable relatively extremely incredibly distinct simple small full make happen specific lower free primary combat power
word make many""".split())

REL = ("hypernyms", "hyponyms", "similar_tos", "also_sees", "attributes", "member_holonyms", "part_holonyms", "member_meronyms", "part_meronyms")


def lemma(word):
    wn = t.wordnet()
    w = word.lower()
    v = wn.morphy(w, "v")
    if v and v != w:
        return v
    for pos in ("n", "a", "v"):
        m = wn.morphy(w, pos)
        if m:
            return m
    return w


def clauses(text):
    return [c.strip() for c in re.split(r"[,:;.]", text) if c.strip()]


def content_words(text):
    """(lemma, original word, clause) for the content words of `text`, in order."""
    out = []
    for c in clauses(text):
        for w in re.findall(r"[A-Za-z]+", c):
            lw = w.lower()
            if len(lw) < 3 or lw in STOP:
                continue
            lem = lemma(lw)
            if lem in STOP:
                continue
            out.append((lem, lw, c))
    return out


NOTES = os.path.join(os.path.dirname(t.REPO), "research_notes", "Stand ability and card design", "ability_system.md")


def canon_quotes(canon, notes=NOTES):
    """The wiki phrases the ability research note quotes for a canon Stand (its 'Cited Findings' line), as one text."""
    stand = re.sub(r" \(.*\)$", "", canon)
    for line in open(notes, encoding="utf-8"):
        if line.startswith(f"- {stand}, "):
            return " ; ".join(re.findall(r'"([^"]+)"', line.split("Source:")[0]))
    return ""


def core_lexicon(tpl, notes=NOTES):
    canon = {lemma(w.lower()) for w in re.findall(r"[A-Za-z]+", tpl["canon"])}
    seen, core = set(), []
    sentence = re.sub(r"[\d.,]+★?|\b[A-Z][a-z]{2} \d{4}\b|the United States|Director Seven|Synthetic Film One", " ", tpl["sentence"])
    for src, text in (("name", tpl["name"]), ("quote", tpl["quote"]), ("card sentence", sentence), ("canon quote (research note)", canon_quotes(tpl["canon"], notes))):
        for lem, w, clause in content_words(text):
            if lem in seen or (src.startswith(("quote", "canon")) and lem in canon):
                continue
            seen.add(lem)
            core.append({"lemma": lem, "word": w, "from": src, "phrase": clause})
    core = core[:MAX_CORE]
    # templates whose own text is too short: top up with the content words of the WordNet definition of their core words
    wn = t.wordnet()
    for item in list(core):
        if len(core) >= MIN_CORE:
            break
        for pos in ("n", "v", "a"):
            syn = wn.synsets(item["lemma"], pos=pos)
            if not syn:
                continue
            for lem, w, clause in content_words(syn[0].definition()):
                if len(core) < MIN_CORE and lem not in seen:
                    seen.add(lem)
                    core.append({"lemma": lem, "word": w, "from": f"WordNet definition of '{item['lemma']}' (top-up)", "phrase": clause})
            break
    return core


def ring_of(lemmas):
    """Single-word lemmas one hop from the first (most frequent) sense of each core lemma, per part of speech."""
    wn = t.wordnet()
    ring = set()
    for lem in lemmas:
        for pos in ("n", "v", "a"):
            syn = wn.synsets(lem, pos=pos)[:1]
            for s in syn:
                near = {s}
                for rel in REL:
                    near.update(getattr(s, rel)())
                for x in s.lemmas():
                    near.update(l.synset() for l in x.derivationally_related_forms() if l.synset().pos() == s.pos())
                for n in near:
                    for x in n.lemmas():
                        name = x.name().lower()
                        if "_" not in name and "-" not in name and name.isalpha():
                            ring.add(name)
    return sorted(ring - set(lemmas))


def build(templates):
    out = []
    for tpl in templates:
        core = core_lexicon(tpl)
        if len(core) < MIN_CORE:
            raise SystemExit(f"template {tpl['id']} {tpl['name']}: only {len(core)} core words (need {MIN_CORE})")
        lems = [c["lemma"] for c in core]
        out.append({"id": tpl["id"], "name": tpl["name"], "canon": tpl["canon"], "core": core, "ring": ring_of(lems)})
    return out


class Scorer:
    """L per template for a candidate: max over its content-word lemmas."""

    def __init__(self, lexicons):
        self.core = [{c["lemma"] for c in lx["core"]} for lx in lexicons]
        self.ring = [set(lx["ring"]) for lx in lexicons]

    def lemmas(self, text):
        return {lemma(w) for w in re.findall(r"[a-z]+", t.fold_ascii(text).lower()) if w not in STOP and len(w) >= 2}

    def matches(self, name):
        """Per template: (L, index of the first matching word of `name`), (0.0, -1) when nothing matches."""
        words = [w for w in re.findall(r"[a-z]+", t.fold_ascii(name).lower())]
        lem = [None if (w in STOP or len(w) < 2) else lemma(w) for w in words]
        out = []
        for c, r in zip(self.core, self.ring):
            hit = next((i for i, x in enumerate(lem) if x in c), None)
            if hit is not None:
                out.append((1.0, hit))
                continue
            hit = next((i for i, x in enumerate(lem) if x in r), None)
            out.append((0.5, hit) if hit is not None else (0.0, -1))
        return out

    def l_values(self, text):
        ls = self.lemmas(text)
        return [1.0 if ls & c else (0.5 if ls & r else 0.0) for c, r in zip(self.core, self.ring)]


def markdown(lexicons):
    L = ["# Core lexicons for the naming fit F", "",
         "Generated by `tools/naming/lexicons.py` from the 17 ability templates in `crates/standuser-core/src/ability.rs` (name, canon ability quote, card sentence).",
         "L = 1 when a content-word lemma of a candidate is a core lemma below, 0.5 when it is in the one-hop WordNet ring (Princeton WordNet 3.0, first sense of each core word per part of speech: same synset, hypernym, hyponym, similar, also-see, attribute, derivational form, member or part relative), else 0.",
         "No encoder is used. Words that only restate the canon Stand's own name are left out of the quote words; the card's metric vocabulary (film, crowd, month, rate...) is a stop list.", ""]
    for lx in lexicons:
        L += [f"## {lx['id']}. {lx['name']} (canon: {lx['canon']})", "", f"{len(lx['core'])} core words, {len(lx['ring'])} ring words.", "",
              "| Lemma | Word | Source | Source phrase |", "|---|---|---|---|"]
        for c in lx["core"]:
            L.append(f"| {c['lemma']} | {c['word']} | {c['from']} | {c['phrase'].replace('|', '/')} |")
        L += ["", "Ring sample: " + ", ".join(lx["ring"][:15]) + (" ..." if len(lx["ring"]) > 15 else ""), ""]
    return "\n".join(L)


def load(path):
    return json.load(open(path, encoding="utf-8"))


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("templates")
    ap.add_argument("--md", default=os.path.join(t.REPO, "docs", "naming-lexicons.md"))
    ap.add_argument("--json", default=os.path.join(t.HERE, "data", "lexicons.json"))
    a = ap.parse_args()
    lx = build(load(a.templates))
    open(a.md, "w", encoding="utf-8").write(markdown(lx) + "\n")
    json.dump([{**x, "ring": None} for x in lx], open(a.json, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    for x in lx:
        print(x["id"], x["name"], len(x["core"]), len(x["ring"]))
