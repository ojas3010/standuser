"""Matching reachability and concentration over all 15,625 letter profiles (spec: "Method H spreads matches with no loss
of fit"): how many canon Stands can be the nearest one, and how concentrated the matches are.

  ~/lb_venv/bin/python -I tools/reachability.py

Profiles are the 5^6 canon-equivalent letter grades weighted by canon's per-stat grade shares. Every variant uses Method
H distance (unknown canon cells costed at their expected error) and the tie-break keys below; remaining exact ties split
the mass evenly (identical-chart twins; the username hash does this across users).

  old       distance, salience-weighted error, largest gap, exact matches, known stats, canon order; then twins share the pick
  new       distance, salience-weighted error, largest gap, exact matches, known stats, continuous percentile distance,
            then username+Stand hash (exact ties split evenly)
  new/spread  as new, with the user's percentile drawn uniformly inside each canon band (8 draws per profile) instead of
            at the band midpoint, which is what real users do
"""
import json, sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE / "golden/proto"))
import match_engine as ME  # noqa: E402

CUM = {s: json.load(open(HERE.parent / "crates/standuser-core/data/cuts.json"))["stats"][s]["canon_cum"] for s in ME.STATS}
CUMA = np.array([CUM[s] for s in ME.STATS])  # 6 x 4 (percent)


def gini(p):
    x = np.sort(p); n = len(x)
    return float((2 * np.arange(1, n + 1) - n - 1).dot(x) / (n * x.sum()))


def stats(p, label):
    top = np.sort(p)[::-1]
    print(f"{label:34s} reachable {int((p > 1e-12).sum()):3d}   top-1 {top[0]*100:5.2f}%   top-10 {top[:10].sum()*100:5.2f}%   Gini {gini(p):.3f}   effective Stands {1/(p**2).sum():5.1f}")
    return dict(reachable=int((p > 1e-12).sum()), top1=float(top[0]), top10=float(top[:10].sum()), gini=gini(p), eff=float(1 / (p ** 2).sum()))


def band_bounds(j):
    b = np.r_[0.0, CUMA[j], 100.0] / 100.0   # 6 bounds -> 5 bands
    return b


def mid_of(j, g):
    """band midpoint for grade values g (array); 0 -> 0.0, >=6 -> 0.9995"""
    b = band_bounds(j)
    gi = np.clip(np.rint(g).astype(int), 1, 5)
    m = (b[gi - 1] + b[gi]) / 2
    return np.where(g <= 0, 0.0, np.where(g >= 6, 0.9995, m))


def main():
    E = ME.Engine()
    P, T = E.P, E.T
    CB = ME.canon_bands()
    w = ME.profile_weights(P, CB)
    # Method H distance: unknown canon cells at their expected squared error
    grades = np.arange(1, 6)
    Em = np.zeros((len(P), 6))
    for j in range(6):
        Em[:, j] = ((P[:, [j]] - grades[None]) ** 2 * CB[j][None]).sum(1)
    D = np.sqrt(np.where(E.mask[None], E.diff ** 2, Em[:, None, :]).sum(2))
    sal, mx, nex, nk, rowid = E.tiekeys()
    r9 = lambda x: np.round(x, 9)
    base_keys = [r9(D), sal, mx, nex, nk]

    def pick_ties(extra_keys):
        cand = np.ones(D.shape, bool)
        for K in base_keys + extra_keys:
            m = np.where(cand, K, np.inf).min(1, keepdims=True)
            cand &= K == m
        return cand

    def mass_ties(cand, ww):
        return (cand / cand.sum(1, keepdims=True) * ww[:, None]).sum(0) / ww.sum()

    # ---- old: canon order picks one Stand; its twins share the mass
    cand = pick_ties([rowid])
    pick = cand.argmax(1)
    m_old = np.zeros(T)
    for i in range(len(P)):
        g = np.where(E.twin == E.twin[pick[i]])[0]
        m_old[g] += w[i] / len(g)
    m_old /= w.sum()
    pick_strict = np.zeros(T); np.add.at(pick_strict, pick, w); pick_strict /= w.sum()
    r_old_strict = stats(pick_strict, "old, strict nearest (no twin split)")
    r_old = stats(m_old, "old: canon order + twin split")

    # ---- new: continuous distance at band midpoints, then hash (even split of exact ties)
    Pm = np.stack([mid_of(j, P[:, j]) for j in range(6)], 1)                 # user percentile = own band midpoint
    Cm = np.stack([mid_of(j, E.V0[:, j]) for j in range(6)], 1)              # canon band midpoints
    cont = np.sqrt(np.where(E.mask[None], (Pm[:, None, :] - Cm[None]) ** 2, 0.0).sum(2))
    m_new = mass_ties(pick_ties([r9(cont)]), w)
    r_new = stats(m_new, "new: continuous + hash (band midpoints)")

    # ---- new/spread: percentile uniform inside the band, 8 draws per profile
    rng = np.random.default_rng(7)
    acc = np.zeros(T)
    DRAWS = 8
    for _ in range(DRAWS):
        Pu = np.zeros_like(P)
        for j in range(6):
            b = band_bounds(j)
            gi = np.clip(P[:, j].astype(int), 1, 5)
            Pu[:, j] = b[gi - 1] + rng.random(len(P)) * (b[gi] - b[gi - 1])
        cont_u = np.sqrt(np.where(E.mask[None], (Pu[:, None, :] - Cm[None]) ** 2, 0.0).sum(2))
        acc += mass_ties(pick_ties([r9(cont_u)]), w)
    m_spread = acc / DRAWS
    r_spread = stats(m_spread, "new/spread: percentile within band")

    # which Stands changed reachability
    lost = [E.names[i] for i in range(T) if m_old[i] > 1e-12 and m_new[i] <= 1e-12]
    gained = [E.names[i] for i in range(T) if m_old[i] <= 1e-12 and m_new[i] > 1e-12]
    print("\neligible canon rows:", T, "(+ rows with fewer than 4 known cells are not matchable)")
    print("reachable under old but not new:", lost)
    print("reachable under new but not old:", gained)
    print("unreachable under new:", [E.names[i] for i in range(T) if m_new[i] <= 1e-12])
    print("unreachable under new/spread:", [E.names[i] for i in range(T) if m_spread[i] <= 1e-12])
    json.dump(dict(old_strict=r_old_strict, old=r_old, new=r_new, new_spread=r_spread), open(HERE / "reachability.json", "w"), indent=1)


if __name__ == "__main__":
    main()
