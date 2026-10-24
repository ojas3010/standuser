#!/usr/bin/env python3
"""Golden reference for standuser-core.

Runs the research prototypes (ability_engine.py, tl.py, the pair-bin functions of common.py, match_engine.py in
tools/golden/proto) plus the spec formulas in plain Python on a Letterboxd export, and writes the JSON that the Rust
tests compare against.  profile.csv is never opened: the join day and username come from the command line.

  ~/lb_venv/bin/python -I tools/golden/ref.py <export_dir_or_zip> --table films.json --joined 2022-07-06 \
      --as-of 2026-10-19 --username jas3344 --out golden.json
"""
import argparse, csv, datetime as dt, io, json, math, sys, unicodedata, zipfile
from collections import Counter, defaultdict
from pathlib import Path
from statistics import NormalDist

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE / "proto"))
import ability_engine as AE  # noqa: E402
import match_engine as ME  # noqa: E402
import rating_fns as RF  # noqa: E402
import tl as TL  # noqa: E402

DATA = HERE.parent.parent / "crates/standuser-core/data"
M64 = (1 << 64) - 1
STATS = ["PWR", "SPD", "RNG", "STA", "PRC", "DEV"]


# ----------------------------------------------------------------------------------------------- helpers
def day(s):
    y, m, d = (int(x) for x in s[:10].split("-"))
    return dt.date(y, m, d).toordinal() - 719163


def iso(d):
    return dt.date.fromordinal(d + 719163).isoformat()


def film_key(name, year):
    return "".join(c for c in unicodedata.normalize("NFKD", name) if c.isalnum()).lower() + f"|{year}"


def boxd(uri):
    return uri.strip().rstrip("/").rsplit("/", 1)[-1] if uri.strip() else None


class SplitMix64:
    def __init__(self, seed):
        self.s = seed & M64

    def next(self):
        self.s = (self.s + 0x9E3779B97F4A7C15) & M64
        z = self.s
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & M64
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & M64
        return z ^ (z >> 31)

    def below(self, n):
        return ((self.next() >> 11) * n) >> 53

    def sample(self, n, k):
        v = list(range(n))
        for i in range(min(k, n)):
            j = i + self.below(n - i)
            v[i], v[j] = v[j], v[i]
        return v[:min(k, n)]


def fnv1a(s):
    h = 0xCBF29CE484222325
    for b in s.lower().encode():
        h = ((h ^ b) * 0x100000001B3) & M64
    return h


def read_export(path):
    p = Path(path)
    files = {}
    if p.is_dir():
        for n in ("watched.csv", "ratings.csv", "diary.csv"):
            if (p / n).exists():
                files[n] = (p / n).read_text(encoding="utf-8")
    else:
        with zipfile.ZipFile(p) as z:
            for n in z.namelist():
                base = n.rsplit("/", 1)[-1].lower()
                if base in ("watched.csv", "ratings.csv", "diary.csv") and n.count("/") <= 1 and not n.startswith(("deleted", "orphaned", "likes", "lists")):
                    files[base] = z.read(n).decode("utf-8")
    rd = lambda t: list(csv.DictReader(io.StringIO(t)))
    W = [dict(day=day(r["Date"]), name=r["Name"], year=int(r["Year"]), uri=r.get("Letterboxd URI", "")) for r in rd(files["watched.csv"])]
    R = [dict(name=r["Name"], year=int(r["Year"]), rating=float(r["Rating"]), uri=r.get("Letterboxd URI", "")) for r in rd(files.get("ratings.csv", "Date,Name,Year,Letterboxd URI,Rating\n")) if r["Rating"].strip()]
    D = []
    for r in rd(files.get("diary.csv", "Date,Name,Year,Letterboxd URI,Rating,Rewatch,Tags,Watched Date\n")):
        l = day(r["Date"])
        D.append(dict(l=l, name=r["Name"], year=int(r["Year"]), v=day(r["Watched Date"]) if r.get("Watched Date", "").strip() else l))
    return W, R, D


class Table:
    def __init__(self, path):
        t = json.load(open(path, encoding="utf-8"))
        self.countries, self.people = t.get("countries", {}), t.get("people", {})
        self.films, self.by_key, self.by_lid = [], {}, {}
        for r in t["films"]:
            if r[0] in self.by_key:
                continue
            f = dict(key=r[0], lid=r[1], tmdb=r[2], year=r[3], runtime=r[4], lang=r[5],
                     countries=[c for c in r[6].split("|") if c], dirs=[int(x) for x in r[7].split(",") if x],
                     comps=[int(x) for x in r[8].split(",") if x], avg=r[9], hist=r[10], count=r[11])
            self.by_key[r[0]] = f
            if r[1]:
                self.by_lid.setdefault(r[1], f)
            self.films.append(f)

    def find(self, uri, name, year):
        c = boxd(uri)
        if c and c in self.by_lid:
            return self.by_lid[c]
        return self.by_key.get(film_key(name, year))

    def person(self, i):
        return self.people.get(str(i), f"#{i}")

    def phrase(self, code):
        n = self.countries.get(code, code)
        n = "United States" if n == "United States of America" else n
        return "the " + n if n in ("United States", "United Kingdom", "Netherlands", "Philippines", "Czech Republic", "United Arab Emirates") else n


# ----------------------------------------------------------------------------------------------- timeline
def timeline(W, D, J, as_of):
    entries = defaultdict(list)
    for d in D:
        entries[(d["name"], d["year"])].append((d["l"], d["v"]))
    cW, kW = Counter(), Counter()
    for w in W:
        cW[w["day"]] += 1
        if (w["name"], w["year"]) in entries and any(abs(v - w["day"]) <= 1 for _, v in entries[(w["name"], w["year"])]):
            kW[w["day"]] += 1

    def mark_bulk(d):
        c = cW.get(d, 0)
        if d == J and kW[d] < c / 2:
            return True
        return c >= 14 or (c >= 8 and kW[d] < c / 2)

    cV, liveV = Counter(), Counter()
    for es in entries.values():
        for l, v in es:
            cV[v] += 1
            liveV[v] += abs(l - v) <= 1

    def view_bulk(d):
        c = cV[d]
        return c >= 14 or (c >= 8 and liveV[d] < c / 2)

    ev, dropped_bulk, dropped_view = [], 0, 0
    for i, w in enumerate(W):
        k = (w["name"], w["year"])
        if k in entries:
            cands = [v for _, v in entries[k] if v >= J and not view_bulk(v)]
            if cands:
                ev.append((i, min(cands), "diary"))
            else:
                dropped_view += 1
        elif mark_bulk(w["day"]):
            dropped_bulk += 1
        else:
            ev.append((i, w["day"], "mark"))
    bulk = sorted(d for d in cW if mark_bulk(d))
    days = np.array([e[1] for e in ev], dtype="int64")
    m = TL.user_metrics(days, as_of) if len(days) else None
    out = dict(events=len(ev), dropped_bulk_mark=dropped_bulk, dropped_no_clean_view=dropped_view,
               rows_on_bulk_mark_days=int(sum(cW[d] for d in bulk)), bulk_mark_days=[iso(d) for d in bulk], metrics=None)
    if m:
        m0 = int(TL.month_of([days.min()])[0]) + 1
        counts = np.bincount(TL.month_of(days)[(TL.month_of(days) >= m0) & (TL.month_of(days) <= m0 + m["L"] - 1)] - m0, minlength=m["L"])
        peak_i = int(np.argmax(counts))
        srun = best = 0
        for c in counts:
            srun = srun + 1 if c == 0 else 0
            best = max(best, srun)
        label = lambda mm: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"][mm % 12] + f" {1970 + mm // 12}"
        out["metrics"] = dict(months=int(m["L"]), events_in_window=int(m["n_win"]), active=int(m["active"]), share=m["share"], pace=m["pace_active"],
                              streak=int(m["streak"]), peak_n=int(counts[peak_i]), peak_month=label(m0 + peak_i), silent=int(m["L"] - m["active"]),
                              silent_run=best, start=label(m0))
    return out, ev


# ----------------------------------------------------------------------------------------------- grading
CUTS = json.load(open(DATA / "cuts.json"))
GAP_H = np.array(json.load(open(DATA / "gap_h.json"))["h"])
ND = NormalDist()


def percentile(stat, v):
    k = CUTS["stats"][stat]["knots"]
    if not k:
        return None
    z = [ND.inv_cdf(p) for _, p in k]
    x = [a for a, _ in k]
    n = len(x)
    if v <= x[0]:
        i = 0
    elif v >= x[-1]:
        i = n - 2
    else:
        i = max(j for j in range(n) if x[j] <= v)
        i = min(i, n - 2)
    return ND.cdf(z[i] + (z[i + 1] - z[i]) * (v - x[i]) / (x[i + 1] - x[i]))


def band_cuts(stat, n):
    """Film-count band for stats graded within bands (Potential); the first band for fewer films than it starts at."""
    sc = dict(CUTS["stats"][stat])
    bands = sc.get("bands")
    if not bands:
        return sc, None
    b = [x for x in bands if n >= x["lo"]]
    b = b[-1] if b else bands[0]
    sc.update(knots=b["knots"], letter_cuts=b["letter_cuts"], canon_cuts=b["canon_cuts"])
    return sc, (f"{b['lo']}-{b['hi']}" if b["hi"] else f"{b['lo']}+")


def percentile_of(sc, v):
    k = sc["knots"]
    z = [ND.inv_cdf(p) for _, p in k]
    x = [a for a, _ in k]
    n = len(x)
    i = 0 if v <= x[0] else (n - 2 if v >= x[-1] else min(max(j for j in range(n) if x[j] <= v), n - 2))
    return ND.cdf(z[i] + (z[i + 1] - z[i]) * (v - x[i]) / (x[i + 1] - x[i]))


def grade(stat, v, provisional, note="", sc=None):
    sc = sc or CUTS["stats"][stat]
    if not sc["letter_cuts"]:
        return dict(stat=stat, value=v, p=None, letter="?", status="ungraded", canon=None, plot=3.0)
    g = 1 + sum(v >= c for c in sc["letter_cuts"])
    if sc["canon_cuts"]:
        cg = 1 + sum(v >= c for c in sc["canon_cuts"])
    else:
        p = percentile_of(sc, v) * 100
        cg = 1 + sum(p >= c for c in sc["canon_cum"])
    status = "provisional" if (provisional or sc["status"] == "provisional") else "measured"
    return dict(stat=stat, value=v, p=percentile_of(sc, v), letter="EDCBA"[g - 1], status=status, canon=float(cg), plot=float(g))


def unknown(stat, v=None):
    return dict(stat=stat, value=v, p=None, letter="?", status="unknown", canon=None, plot=3.0)


def none_(stat):
    return dict(stat=stat, value=None, p=None, letter="∅", status="none", canon=0.0, plot=0.0)


def inf(g):
    g = dict(g)
    g.update(letter="∞", symbol="inf", canon=6.0, plot=6.0, p=0.999)
    return g


# ----------------------------------------------------------------------------------------------- stats
def compute(W, R, D, T, tl_out, ev, J, as_of):
    f = dict(n_watched=len(W), events=len(ev))
    f["n_watched_in_table"] = sum(T.find(w["uri"], w["name"], w["year"]) is not None for w in W)
    minn = CUTS["crowd_min_ratings"]
    pool, missing = [], 0
    for r in R:
        m = T.find(r["uri"], r["name"], r["year"])
        (pool.append((r, m)) if m else None)
        missing += m is None
    f["crowd_source"] = "Letterboxd displayed average"
    rated = []
    for r, m in pool:
        c = m["avg"]
        n = m["count"] or 0
        if c is not None and n >= minn:
            rated.append(dict(name=r["name"], year=r["year"], own=r["rating"], crowd=c, n=n, uri=boxd(r["uri"]) or ""))
    f.update(n_rated=len(R), n_rated_missing=missing, n_elig=len(rated))
    n = len(rated)
    sparse = len(R) > 0 and missing * 2 > len(R)
    f.update(b=0.0, pa25=0.0, pairs=0, dpa=0.0)
    if not R:
        power, prec = none_("PWR"), none_("PRC")
    elif sparse or n < 20:
        power, prec = unknown("PWR"), unknown("PRC")
    else:
        d = sum(x["own"] - x["crowd"] for x in rated)
        shrunk = (d + CUTS["lambda_"] * CUTS["mu_b"]) / (n + CUTS["lambda_"])
        power = grade("PWR", -shrunk, n < 30)
        rv = np.array([int(round(x["own"] * 2)) for x in rated])
        c = np.array([x["crowd"] for x in rated])
        hits, N = RF.pair_bins(rv, c)
        pairs = float(N.sum())
        f["pairs"] = int(pairs)
        f["pa25"] = float(hits.sum() / pairs) if pairs > 0 else 0.0
        dpa = RF.dpa_score(hits, N, GAP_H, CUTS["pa_pop"]) if pairs >= 1 else float("nan")
        if n >= 50 and pairs >= 500:
            f["dpa"] = float(dpa)
            prec = grade("PRC", float(dpa), n < 100)
        else:
            prec = unknown("PRC")
    if n > 0:
        f["b"] = sum(x["own"] - x["crowd"] for x in rated) / n
    m = tl_out["metrics"]
    if m is None:
        speed, stamina = (none_("SPD"), none_("STA")) if tl_out["events"] == 0 else (unknown("SPD"), unknown("STA"))
    else:
        f.update(pace=m["pace"], peak_n=m["peak_n"], peak_month=m["peak_month"], months=m["months"], active=m["active"], streak=m["streak"],
                 start=m["start"], silent=m["silent"], silent_run=m["silent_run"])
        ev_n = m["events_in_window"]
        if ev_n < 12 or m["active"] < 6:
            speed = unknown("SPD", m["pace"])
        else:
            speed = grade("SPD", m["pace"], True)
            if m["pace"] >= 26.2 and m["months"] >= 24 and ev_n >= 40:
                speed = inf(speed)
        if m["months"] < 12 or m["active"] < 6 or ev_n < 12:
            stamina = unknown("STA", m["share"])
        else:
            stamina = grade("STA", m["share"], True)
            if m["months"] >= 60 and m["active"] == m["months"]:
                stamina = inf(stamina)
    cty_films, cty_count, dir_count, credit_films, dir_films = [], Counter(), Counter(), 0, []
    for w in W:
        t = T.find(w["uri"], w["name"], w["year"])
        if not t or (t["runtime"] != 0 and t["runtime"] < 41):
            continue
        if t["countries"]:
            cty_films.append(t["countries"])
            for c in t["countries"]:
                cty_count[c] += 1
        if t["dirs"]:
            credit_films += 1
            dir_films.append(sorted(set(t["dirs"])))
            for d in dir_films[-1]:
                dir_count[d] += 1
    f["n_films_cty"] = len(cty_films)
    if not W:
        rng = none_("RNG")
    elif len(cty_films) < 20:
        rng = unknown("RNG")
    else:
        size = min(50, len(cty_films))
        r = SplitMix64(5)
        tot = 0.0
        for _ in range(20):
            wts = {}
            for i in r.sample(len(cty_films), size):
                cs = cty_films[i]
                for c in cs:
                    wts[c] = wts.get(c, 0.0) + 1.0 / len(cs)
            h = 0.0
            for k in sorted(wts):
                p = wts[k] / size
                h -= p * math.log(p)
            tot += math.exp(h)
        dval = tot / 20
        f["d_range"] = dval
        top = sorted(cty_count.items(), key=lambda kv: (-kv[1], kv[0]))[0]
        f["top_country"], f["top_share"] = T.phrase(top[0]), top[1] / len(cty_films)
        if len(cty_films) < 50:
            rng = unknown("RNG", dval)
        else:
            rng = grade("RNG", dval, len(cty_films) < 100)
            if dval >= (CUTS['stats']['RNG'].get('inf_cut') or 5.7) and len(cty_films) >= 150:
                rng = inf(rng)
    f.update(films_with_credits=credit_films, credits=sum(dir_count.values()), n1=sum(1 for v in dir_count.values() if v == 1))
    f["gt"] = f["n1"] / f["credits"] if f["credits"] else 0.0
    if dir_count:
        did = sorted(dir_count.items(), key=lambda kv: (-kv[1], T.person(kv[0])))[0]
        f["top_dir"], f["top_dir_n"] = T.person(did[0]), did[1]
    if not W:
        pot = none_("DEV")
    elif credit_films < 40:
        pot = unknown("DEV", f["gt"] if credit_films else None)
    else:
        size = min(60, len(dir_films))
        r = SplitMix64(6)
        tot = 0.0
        for _ in range(20):
            cnt = Counter()
            for i in r.sample(len(dir_films), size):
                for d in dir_films[i]:
                    cnt[d] += 1
            tot += sum(1 for v in cnt.values() if v == 1) / sum(cnt.values())
        f["gt60"] = tot / 20
        sc, band = band_cuts("DEV", credit_films)
        f["potential_band"] = band or ""
        pot = grade("DEV", f["gt60"], credit_films < 60, sc=sc)
        if credit_films >= 1000 and pot["letter"] == "E":
            pot = dict(pot, letter="完成", symbol="complete", canon=1.0, plot=1.0, p=0.001)
    return dict(PWR=power, SPD=speed, RNG=rng, STA=stamina, PRC=prec, DEV=pot), f, rated


# ----------------------------------------------------------------------------------------------- type, match, ability, name
def type_rule(canon):
    cap = lambda x: None if x is None or x < 1 else min(x, 5.0)
    p, r, c = cap(canon["PWR"]), cap(canon["RNG"]), cap(canon["PRC"])
    if r is not None and c is not None and r == 5 and c == 1:
        return "automatic"
    if r is not None and p is not None:
        if r >= 4 and p <= 3:
            return "long_distance"
        if r <= 3 and p >= 2:
            return "close_range"
    return "unclassified"


def band_mid(cum, v):
    if v <= 0:
        return 0.0
    if v >= 6:
        return 0.9995
    g = min(max(int(round(v)), 1), 5)
    lo = 0.0 if g == 1 else cum[g - 2]
    hi = 100.0 if g == 5 else cum[g - 1]
    return (lo + hi) / 2 / 100


def nearest(u, pct, username, top=3):
    """Letter distance (Method H), then salience-weighted error, largest gap, exact matches, known cells, then the
    continuous distance between percentiles and canon band midpoints, then a hash of username|name (which also splits
    identical-chart twins)."""
    known = [i for i in range(6) if u[i] is not None]
    if len(known) < 4:
        return []
    names, V, rowid, parts, users, _ = ME.load_canon()
    rows = list(csv.DictReader(open(ME.CANON_CSV, encoding="utf-8")))
    grades_by_name = {r["display_name"]: [r[s] for s in STATS] for r in rows}
    bands = ME.canon_bands()
    uj = [3.0 if x is None else x for x in u]
    em = [sum(bands[j][g] * (uj[j] - (g + 1)) ** 2 for g in range(5)) for j in range(6)]
    cum = [CUTS["stats"][s]["canon_cum"] for s in STATS]
    r9 = lambda x: int(round(x * 1e9))
    ranked = []
    for t in range(len(names)):
        v = V[t]
        kc = int(np.isfinite(v).sum())
        if kc < 4:
            continue
        cost = sal = mx = 0.0
        ex = kb = 0
        for i in known:
            if np.isfinite(v[i]):
                dd = u[i] - v[i]
                cost += dd * dd
                sal += abs(u[i] - 3) * dd * dd
                mx = max(mx, abs(dd))
                ex += dd == 0
                kb += 1
            else:
                cost += em[i]
        dist = math.sqrt(6.0 / len(known) * cost)
        sal = 6.0 / kb * sal if kb else 0.0
        cont = math.sqrt(sum((pct[i] - band_mid(cum[i], v[i])) ** 2 for i in range(6) if pct[i] is not None and np.isfinite(v[i])))
        tie = fnv1a(f"{username}|{names[t]}")
        ranked.append(((r9(dist), r9(sal), r9(mx), -ex, -kc, r9(cont), tie), dist, names[t], tuple(None if not np.isfinite(x) else int(x * 10) for x in v)))
    ranked.sort(key=lambda r: r[0])
    out, taken = [], []
    for r in ranked:
        if r[3] in taken:
            continue
        taken.append(r[3])
        out.append(dict(name=r[2], dist=r[1], grades=grades_by_name[r[2]]))
        if len(out) == top:
            break
    return out


def ability(grades, f, rated):
    stats = {}
    for s in STATS:
        g = grades[s]
        d = dict(p=g["p"], letter=g["letter"], status=g["status"] if g["status"] in ("measured", "provisional") else "ungraded", value=g.get("value"))
        if g.get("symbol"):
            d["symbol"] = g["symbol"]
        stats[s] = d
    facts = {k: f.get(k, 0) for k in ("b", "n_elig", "pace", "peak_n", "peak_month", "M", "active", "streak", "start", "silent", "silent_run", "D", "top_country", "top_share",
                                       "n_films_cty", "pa25", "pairs", "dpa", "n1", "credits", "gt", "top_dir", "top_dir_n")}
    facts["M"], facts["D"] = f.get("months", 0), f.get("d_range", 0)
    films = [dict(name=r["name"], year=r["year"], own=r["own"], crowd=r["crowd"], n=r["n"], uri=r["uri"]) for r in rated]
    r = AE.render(dict(user="x", name=None, stats=stats, facts=facts, films=films))
    return dict(template=r["template"], ability=r["ability"], film=r["film"], weakness=r["weakness"], why=r["why"]), r


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("export")
    ap.add_argument("--table", required=True)
    ap.add_argument("--joined", required=True)
    ap.add_argument("--as-of", required=True)
    ap.add_argument("--username", required=True)
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    W, R, D = read_export(a.export)
    T = Table(a.table)
    J, as_of = day(a.joined), day(a.as_of)
    tl_out, ev = timeline(W, D, J, as_of)
    grades, f, rated = compute(W, R, D, T, tl_out, ev, J, as_of)
    canon = {s: grades[s]["canon"] for s in STATS}
    u = [canon[s] for s in STATS]
    ab, _ = ability(grades, f, rated)
    out = dict(
        inputs=dict(username=a.username, joined=a.joined, as_of=a.as_of, watched=len(W), ratings=len(R), diary=len(D), table_films=len(T.films)),
        timeline=tl_out, facts=f,
        grades={s: {k: v for k, v in grades[s].items()} for s in STATS},
        type=type_rule(canon), nearest=nearest(u, [grades[s]["p"] for s in STATS], a.username), ability=ab,
    )
    Path(a.out).write_text(json.dumps(out, indent=1, ensure_ascii=False, default=float), encoding="utf-8")
    g = " ".join(grades[s]["letter"] for s in STATS)
    print(f"{a.out}: grades {g}; type {out['type']}; nearest {[n['name'] for n in out['nearest']]}")


if __name__ == "__main__":
    main()
