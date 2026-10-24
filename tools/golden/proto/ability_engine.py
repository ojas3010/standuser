"""Deterministic Stand ability engine (final design, round 3).

Run:  python3 -I ability_engine.py          -> self-tests, Ojas provisional card, 17-template gallery
Inputs are plain dicts (see OJAS below). No randomness, no network, no clock.
Rules implemented:
  signature stat   max |z| over graded stats (z = Phi^-1(clip(p,.005,.995)); inf=+3, complete=-3; ties canon order);
                   BALANCED if max|z| < TAU_BAL (1.0)
  template         (stat, direction, symbol) -> one of 17 templates
  signature film   largest |own - crowd| among films with >= FLOOR Letterboxd ratings (ties: rounded gap, count, URI)
  weakness         lowest-percentile graded stat other than the signature stat; tiered by card letter
  why              slots only; every number is traceable
"""
import re
from statistics import NormalDist

STATS = ["PWR", "SPD", "RNG", "STA", "PRC", "DEV"]
LABEL = {"PWR": "Power", "SPD": "Speed", "RNG": "Range", "STA": "Stamina", "PRC": "Precision", "DEV": "Potential"}
TAU_BAL = 1.0           # z; about 10% of users when six graded stats are independent
FLOOR = 30000           # Letterboxd ratings a film needs to be eligible (same floor as the Power/Precision crowd)
GAP_MIN_DIRECTIONAL = 1.0
_nd = NormalDist()


def z_of(s):
    if s.get("symbol") == "inf":
        return 3.0
    if s.get("symbol") == "complete":
        return -3.0
    p = min(max(s["p"], 0.005), 0.995)
    return _nd.inv_cdf(p)


def graded(profile):
    """Stats that may be chosen: have a percentile (cut-points exist) or a special symbol; not '?' / 'none'."""
    return {k: v for k, v in profile["stats"].items() if v.get("status") in ("measured", "provisional")}


def pick_signature(profile):
    g = graded(profile)
    if not g:
        return None
    zs = {k: z_of(v) for k, v in g.items()}
    order = sorted(zs, key=lambda k: (-abs(zs[k]), STATS.index(k)))   # ties: canon order
    top = order[0]
    runner = order[1] if len(order) > 1 else None
    if abs(zs[top]) < TAU_BAL:
        return dict(stat="BAL", z=zs[top], top=top, runner=runner, zs=zs)
    return dict(stat=top, z=zs[top], runner=runner, zs=zs)


# ------------------------------------------------------------------ templates
# canon_grade is the cited Stand's grade for the stat in /home/claude/lb/swarm/canon/stand_stats.csv
T = {
 ("PWR", "hi"):  dict(id=1, name="Bomb Transmutation", canon="Killer Queen", cg="A",
    text="Every film it touches becomes a bomb: it rates {b_abs:.2f}★ below the crowd on average, across {n_elig:,} films.",
    quote="Killer Queen can turn anything it touches into a bomb.", limit="Killer Queen can only charge one object at a time."),
 ("PWR", "lo"):  dict(id=2, name="Curative Cooking", canon="Pearl Jam", cg="E",
    text="It heals what the crowd wounded: it rates {b_abs:.2f}★ above the crowd on average, across {n_elig:,} films.",
    quote="it can heal incurable illnesses provided the right ingredients are involved", limit="if the eater does not have the specific ailment a dish is made for, nothing will happen"),
 ("SPD", "hi"):  dict(id=3, name="Hora Rush", canon="Silver Chariot", cg="A",
    text="A barrage too fast to follow: {pace:.1f} films per active month, peaking at {peak_n} films in {peak_month}.",
    quote="usually relying on attacking enemies with barrages of sword slashes and thrusts", limit="Silver Chariot has the distinct weakness of being unable to see whatever Polnareff cannot see"),
 ("SPD", "inf"): dict(id=4, name="Time Acceleration", canon="Made in Heaven", cg="∞",
    text="Time speeds up around it: {pace:.1f} films per active month, peaking at {peak_n} in {peak_month}, a pace almost no other user reaches.",
    quote="The only living beings who can move freely within time's acceleration are Made in Heaven itself and its user.", limit="he retains all of his human weaknesses"),
 ("SPD", "lo"):  dict(id=5, name="Accelerated Aging", canon="The Grateful Dead", cg="E",
    text="A slow mist drifts rather than strikes: {pace:.1f} films per active month, peaking at {peak_n} in {peak_month}.",
    quote="The activation of this ability manifests itself as mist spreading from The Grateful Dead.", limit="However, the aging can be slowed."),
 ("RNG", "hi"):  dict(id=6, name="Coiled Body", canon="Hierophant Green", cg="A",
    text="It unravels into tendrils that cross borders: {D:.1f} effective production countries per 50 films, led by {top_country} ({top_share:.0%} of films).",
    quote="being able to distance itself from its user at least 100 meters", limit="it is relatively weak in direct hand-to-hand combat"),
 ("RNG", "inf"): dict(id=7, name="Fictional Manifestation", canon="Bohemian Rhapsody", cg="∞",
    text="Fiction from every nation walks into the world: {D:.1f} effective production countries per 50 films, across {n_films_cty:,} films.",
    quote="a Stand ability that breaks the barrier between fiction and reality", limit="has a glaring flaw in that it indiscriminately brings any and all fiction to life"),
 ("RNG", "lo"):  dict(id=8, name="Position Locking", canon="Kraft Work", cg="E",
    text="It locks everything in place: only {D:.1f} effective production countries per 50 films, and {top_share:.0%} of your films come from {top_country}.",
    quote="the ability to lock an object's position, completely freezing it in place", limit="If Sale moves far enough from immobilized objects or people, the effects of Kraft Work's ability will eventually dissipate."),
 ("STA", "hi"):  dict(id=9, name="Heat-Seeking Detonation", canon="Sheer Heart Attack", cg="A",
    text="It never stops tracking: films in {active} of {M} months, with a streak of {streak} in a row.",
    quote="it will continue to seek out targets until it explodes from a human", limit="main weakness is the simplistic way it prioritizes targets from the heat they emit"),
 ("STA", "inf"): dict(id=10, name="Energy Absorption", canon="Notorious B.I.G", cg="∞",
    text="It keeps growing on whatever it touches: films in every one of the {M} months since {start}.",
    quote="one of the few Stands able to exist even after its user's death", limit="its simplicity can be exploited"),
 ("STA", "lo"):  dict(id=11, name="Time Erasure", canon="King Crimson", cg="E",
    text="Time vanishes and only the result remains: {silent} of {M} months hold no film, and the longest blank runs {silent_run} months.",
    quote="erase time for up to around ten seconds", limit="poor effective range (about 2–3 meters)"),
 ("PRC", "hi"):  dict(id=12, name="Wind Reading", canon="Manhattan Transfer", cg="A",
    text="It reads the air and answers it: it agrees with the crowd on {pa25:.0%} of the {pairs:,} film pairs it can compare (difficulty-adjusted score {dpa:.2f}).",
    quote="floating around and detecting the air movements near it", limit="it is possible to fool Johngalli A. into misreading the wind"),
 ("PRC", "lo"):  dict(id=13, name="Killer Virus Infection", canon="Purple Haze", cg="E",
    text="Its verdicts scatter without regard for the crowd: it agrees with the crowd on only {pa25:.0%} of the {pairs:,} film pairs it can compare (difficulty-adjusted score {dpa:.2f}).",
    quote="Because the virus indiscriminately affects friends and foes", limit="not even Fugo himself could cancel the effect of his power"),
 ("DEV", "hi"):  dict(id=14, name="Life Giver", canon="Gold Experience", cg="A",
    text="Every unfamiliar name becomes a new life: {n1} of your {credits:,} director credits are directors you have seen only once ({gt:.0%}).",
    quote="the ability to create life from inanimate material", limit="he has no control over them and they behave with their own free will"),
 ("DEV", "lo"):  dict(id=15, name="Divination", canon="Hermit Purple", cg="E",
    text="It divines through familiar vines: only {gt:.0%} of your {credits:,} director credits are one-offs, and it keeps returning to {top_dir} ({top_dir_n} films).",
    quote="It allows its user to divine information and messages from other locations.", limit="The vines have limited resistance"),
 ("DEV", "complete"): dict(id=16, name="Complete Mastery", canon="Star Platinum (Part 6)", cg="Complete",
    text="Complete: across {credits:,} director credits the frontier has closed, with only {gt:.0%} one-offs.",
    quote="It decreases in rank as the user masters their Stand.", limit="(no canon limit; Potential Complete means nothing is left to learn)"),
 ("BAL", "bal"): dict(id=17, name="Mimicry", canon="Surface", cg="B B C B C C",
    text="It copies whoever stands before it: your strongest stat, {top_label} {top_letter}, sits at the {top_pct} percentile and nothing else stands out.",
    quote="Surface's primary ability is to copy the appearance of another person.", limit="Surface and the victim must stand face-to-face for it to work."),
}

# Weakness phrases per stat, by tier. {x} etc. are slots.
WEAK = {
 "PWR": dict(weak_lo="gentle hands (Power {L}). It rates {b_abs:.2f}★ above the crowd on average, so its blows rarely leave a mark.",
             weak_hi="blunt hands (Power {L}). It rates {b_abs:.2f}★ below the crowd on average, so little survives it.",
             mid="ordinary hands (Power C, “same as a human”). It rates {b_abs:.2f}★ {b_dir} the crowd on average, about what a human would."),
 "SPD": dict(weak="slow hands (Speed {L}). {pace:.1f} films per active month, so anything faster outruns it.",
             mid="ordinary pace (Speed C, “same as a human”). {pace:.1f} films per active month, about what a human would."),
 "RNG": dict(weak="short reach (Range {L}). {top_share:.0%} of your films come from {top_country}, so it loses its grip beyond it.",
             mid="ordinary reach (Range C, “same as a human”). {D:.1f} effective production countries per 50 films."),
 "STA": dict(weak="short breath (Stamina {L}). Films in {active} of {M} months, so the effect fades between bursts.",
             mid="ordinary breath (Stamina C, “same as a human”). Films in {active} of {M} months."),
 "PRC": dict(weak="loose aim (Precision {L}). It agrees with the crowd on {pa25:.0%} of film pairs, so it can hit what it did not mean to.",
             mid="ordinary aim (Precision C, “same as a human”). It agrees with the crowd on {pa25:.0%} of film pairs."),
 "DEV": dict(weak="nothing left to learn (Potential {L}). Only {gt:.0%} of your director credits are one-offs, so there is little room to grow.",
             mid="ordinary growth (Potential C, “same as a human”). {gt:.0%} of your director credits are one-offs."),
}
STATUS_TAG = {"provisional": " [provisional]", "measured": "", "ungraded": ""}


def ordinal(p):
    n = round(p * 100, 1)
    s = f"{n:g}"
    return s + ("th" if not s.endswith(("1", "2", "3")) or s.endswith(("11", "12", "13")) else {"1": "st", "2": "nd", "3": "rd"}[s[-1]])


def template_key(sig, profile):
    if sig["stat"] == "BAL":
        return ("BAL", "bal")
    s = profile["stats"][sig["stat"]]
    if s.get("symbol") == "inf":
        return (sig["stat"], "inf")
    if s.get("symbol") == "complete":
        return (sig["stat"], "complete")
    return (sig["stat"], "hi" if sig["z"] > 0 else "lo")


def signature_film(films, sign=None, floor=FLOOR):
    """films: dicts name, year, own, crowd, n (Letterboxd ratings behind the crowd mean), uri.
    Eligible: n >= floor. Key: |gap| rounded to 0.01 desc, n desc, uri asc. sign=+1/-1 restricts to that direction
    when a film with |gap| >= 1.0 exists in it (Power templates); otherwise unrestricted."""
    e = [dict(f, gap=f["own"] - f["crowd"]) for f in films if f["n"] >= floor]
    if not e:
        return None
    if sign:
        d = [f for f in e if f["gap"] * sign > 0 and abs(f["gap"]) >= GAP_MIN_DIRECTIONAL]
        e = d or e
    e.sort(key=lambda f: (-round(abs(f["gap"]), 2), -f["n"], f["uri"]))
    return e[0]


def weakness(profile, sig):
    g = graded(profile)
    cands = {k: v for k, v in g.items() if k != sig["stat"] and v.get("symbol") is None}
    if sig["stat"] == "BAL":
        cands = {k: v for k, v in cands.items() if k != sig.get("top")}
    if not cands:
        return dict(text="Weakness: not yet revealed (?).", stat=None)
    k = min(cands, key=lambda s: (cands[s]["p"], STATS.index(s)))
    v = cands[k]; L = v["letter"]; f = profile["facts"]
    slots = dict(f, L=L, b_abs=abs(f["b"]), b_dir="below" if f["b"] < 0 else "above")
    if L in ("D", "E"):
        if k == "PWR":
            ph = WEAK["PWR"]["weak_lo"] if f["b"] > 0 else WEAK["PWR"]["weak_hi"]
        else:
            ph = WEAK[k]["weak"]
        txt = "Weakness: " + ph.format(**slots)
    elif L == "C":
        txt = "Soft spot: " + WEAK[k]["mid"].format(**slots)
    else:
        txt = f"No weakness found: your lowest graded stat, {LABEL[k]}, is still {L}."
    return dict(text=txt + STATUS_TAG.get(v["status"], ""), stat=k, letter=L, p=v["p"], status=v["status"])


def render(profile):
    sig = pick_signature(profile)
    if sig is None:
        return dict(name="「???」", ability="Ability: not yet revealed (?).", film=None, weakness="Weakness: not yet revealed (?).", why="No stat could be graded.", template=None)
    key = template_key(sig, profile)
    t = T[key]; f = profile["facts"]
    slots = dict(f, b_abs=abs(f["b"]))
    if key[0] == "BAL":
        top = sig["top"]; s = profile["stats"][top]
        slots.update(top_label=LABEL[top], top_letter=s["letter"], top_pct=ordinal(s["p"]))
    ability = f"{t['name']}. " + t["text"].format(**slots)
    if key[0] != "BAL" and profile["stats"][sig["stat"]].get("status") == "provisional":
        ability += STATUS_TAG["provisional"]
    sign = None
    if key == ("PWR", "hi"): sign = -1
    if key == ("PWR", "lo"): sign = +1
    sf = signature_film(profile.get("films", []), sign)
    film = None
    if sf:
        film = f"Signature film: {sf['name']} ({sf['year']}), {sf['own']:.1f}★ against the crowd's {sf['crowd']:.2f}★ ({sf['gap']:+.2f}), {sf['n']:,} ratings."
    wk = weakness(profile, sig)
    # why
    if key[0] == "BAL":
        why = (f"Why: no graded stat leaves the central band (largest |z| {abs(sig['z']):.2f} < {TAU_BAL}), so the balanced template applies; "
               f"{t['name']} follows {t['canon']} (canon chart {t['cg']}). ")
    else:
        st = sig["stat"]; s = profile["stats"][st]
        rn = ""
        if sig["runner"]:
            r = sig["runner"]; rn = f"; runner-up {LABEL[r]} z {sig['zs'][r]:+.2f}"
        stat_txt = (f"{LABEL[st]} {s['letter']}" + (f" ({ordinal(s['p'])} percentile, z {sig['z']:+.2f}{rn})" if s.get("p") is not None else ""))
        why = (f"Why: {stat_txt} is your most extreme graded stat, which selects {t['name']} after {t['canon']}, "
               f"whose canon {LABEL[st]} is {t['cg']}. Canon: “{t['quote'].rstrip('.')}”. ")
    if wk["stat"]:
        why += f"{LABEL[wk['stat']]} {wk['letter']} is your lowest graded stat, so the limitation comes from it."
    return dict(name=profile.get("name"), template=t["id"], ability=ability, film=film, weakness=wk["text"], why=why, sig=sig, key=key)


# ------------------------------------------------------------------ slot guard (also used to vet optional LLM rewrites)
NUM = re.compile(r"\d[\d,]*\.?\d*")
def numbers_in(text):
    return {n.rstrip(".,") for n in NUM.findall(text)}

def guard(text, rendered_reference):
    """Every number in `text` must appear in the deterministic rendering it rephrases."""
    return numbers_in(text) <= numbers_in(rendered_reference)


# ------------------------------------------------------------------ SAMPLE PROFILE (synthetic)
# The research prototype's own fixture described a real user; this copy keeps its shape with invented values and films.
OJAS = dict(
    user="synthuser", name="「SAMPLE NAME」",
    stats=dict(
        PWR=dict(p=0.597, letter="C", status="measured", value=0.052),
        SPD=dict(p=0.803, letter="B", status="provisional", value=4.2),
        RNG=dict(p=None, letter="C", status="ungraded", value=2.9),
        STA=dict(p=0.923, letter="A", status="provisional", value=0.91),
        PRC=dict(p=0.978, letter="A", status="measured", value=0.883),
        DEV=dict(p=None, letter="C", status="ungraded", value=0.5)),
    facts=dict(b=-0.0612, n_elig=410, pace=4.2, peak_n=12, peak_month="Mar 2023", M=44, active=40, streak=18,
               start="Feb 2023", silent=4, silent_run=2, D=2.9, top_country="the United States", top_share=0.8,
               n1=120, credits=240, gt=0.5, top_dir="Director Seven", top_dir_n=6,
               pa25=0.894, pairs=70211, dpa=0.883, n_films_cty=200),
    films=[
        dict(name="Synthetic Film One", year=2008, own=1.0, crowd=3.544, n=166934, uri="synthetic-one"),
        dict(name="Synthetic Film Two: The Long Subtitle of a Film", year=2006, own=5.0, crowd=3.67, n=2879 * 300, uri="synthetic-two"),
        dict(name="Synthetic Film Three", year=2010, own=1.5, crowd=2.82, n=4020 * 300, uri="synthetic-three"),
        dict(name="Synthetic Film Four", year=2000, own=2.0, crowd=3.24, n=347 * 300, uri="synthetic-four"),
        dict(name="Synthetic Film Five", year=2017, own=5.0, crowd=3.84, n=5287 * 300, uri="synthetic-five"),
    ])


# ------------------------------------------------------------------ synthetic gallery (one profile per template)
def synth(stat, mode):
    base = dict(PWR=dict(p=.5, letter="C", status="measured"), SPD=dict(p=.5, letter="C", status="provisional"),
                RNG=dict(p=.5, letter="C", status="provisional"), STA=dict(p=.5, letter="C", status="provisional"),
                PRC=dict(p=.5, letter="C", status="measured"), DEV=dict(p=.5, letter="C", status="provisional"))
    st = {k: dict(v) for k, v in base.items()}
    if stat != "BAL":
        d = st[stat]
        if mode == "hi": d.update(p=.96, letter="A")
        if mode == "lo": d.update(p=.04, letter="E")
        if mode == "inf": d.update(p=.999, letter="∞", symbol="inf")
        if mode == "complete": d.update(p=.001, letter="完成", symbol="complete")
        # give the weakest stat a clear low so the weakness line renders
        low = "PRC" if stat != "PRC" else "PWR"
        st[low].update(p=.15, letter="D")
    else:
        st["PWR"].update(p=.70, letter="B"); st["SPD"].update(p=.31, letter="C")
    facts = dict(b=-0.21 if mode != "lo" or stat != "PWR" else 0.21, n_elig=312, pace=(1.6 if (stat == "SPD" and mode == "lo") else 9.3),
                 peak_n=14, peak_month="Mar 2024", M=48, active=(21 if stat == "STA" and mode == "lo" else 43), streak=17, start="Jan 2023",
                 silent=(27 if stat == "STA" and mode == "lo" else 5), silent_run=6, D=(1.6 if mode == "lo" else 6.4),
                 top_country="France" if mode != "lo" else "the United States", top_share=(0.81 if mode == "lo" else 0.22), n_films_cty=263,
                 pa25=(0.58 if stat == "PRC" and mode == "lo" else 0.74), pairs=51234, dpa=0.69 if stat == "PRC" and mode == "lo" else 0.79,
                 n1=212, credits=266, gt=0.80 if mode == "hi" else 0.21, top_dir="Akira Kurosawa", top_dir_n=19)
    if stat == "DEV" and mode == "complete":
        facts.update(credits=1210, gt=0.06)
    return dict(user="demo", name=None, stats=st, facts=facts,
                films=[dict(name="Example Film", year=2001, own=1.0, crowd=3.4, n=250000, uri="x")])


if __name__ == "__main__":
    import copy, random
    r = render(OJAS)
    print("=== OJAS (PROVISIONAL) ===")
    for k in ("name", "ability", "film", "weakness", "why"):
        print(f"{k.upper():9s} {r[k]}")
    # tests
    assert render(OJAS) == r, "not deterministic"
    p2 = copy.deepcopy(OJAS); random.Random(3).shuffle(p2["films"]); assert render(p2)["film"] == r["film"], "film order dependence"
    assert pick_signature(OJAS)["stat"] == "PRC" and pick_signature(OJAS)["runner"] == "STA"
    # tie in |z| -> canon order
    tie = copy.deepcopy(OJAS); tie["stats"]["STA"].update(p=0.978); tie["stats"]["PRC"].update(p=0.978)
    assert pick_signature(tie)["stat"] == "STA", "canon-order tie-break (STA before PRC)"
    # film tie-break
    f = [dict(name="B", year=1, own=1, crowd=3.0, n=40000, uri="b"), dict(name="A", year=1, own=1, crowd=3.0, n=50000, uri="z"),
         dict(name="C", year=1, own=1, crowd=3.0, n=50000, uri="a")]
    assert signature_film(f)["uri"] == "a", "tie: larger n, then uri ascending"
    assert signature_film([dict(name="Obscure", year=1, own=0.5, crowd=4.0, n=900, uri="o")]) is None, "floor"
    # unrated user: Power and Precision are 'none' (excluded); film line absent; still renders
    nr = copy.deepcopy(OJAS); nr["films"] = []
    for k in ("PWR", "PRC"): nr["stats"][k] = dict(p=None, letter="\u2205", status="none")
    rn = render(nr); assert rn["film"] is None and rn["template"] == 9, rn   # STA A (z 1.43) becomes signature
    assert "[provisional]" in rn["ability"]
    # nothing gradable
    nn = copy.deepcopy(OJAS)
    for k in nn["stats"]: nn["stats"][k] = dict(p=None, letter="?", status="none")
    assert render(nn)["template"] is None
    print("\nself-tests passed")
    print("unrated-user card:", rn["ability"], "|", rn["weakness"])
    print("\n=== GALLERY (synthetic data, one per template) ===")
    words = []
    for (stat, mode), t in T.items():
        pr = synth(stat, mode); rr = render(pr)
        assert rr["template"] == t["id"], (stat, mode, rr["template"])
        n = len(rr["ability"].split()); words.append(n)
        assert guard(rr["ability"], rr["ability"])
        print(f"[T{t['id']:02d} {stat} {mode}] ({n} words) {rr['ability']}")
        print(f"      {rr['weakness']}")
        print(f"      {rr['why']}")
    print("ability word counts min/median/max:", min(words), sorted(words)[len(words)//2], max(words))
