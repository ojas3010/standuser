"""Vectorised nearest-canon-Stand engine over all 5^6 grade profiles, with several matching variants.
Uses the same numeric mapping as swarm/canon/match.py (A=5..E=1, inf=6, Complete=1, empty-set=0, ?=missing, >=4 known)."""
import sys, itertools, csv
from pathlib import Path
CANON_CSV = str(Path(__file__).resolve().parents[3] / "crates/standuser-core/data/canon.csv")  # patched path
import numpy as np
HERE = None
STATS = ["PWR", "SPD", "RNG", "STA", "PRC", "DEV"]
LET = {"A": 5, "B": 4, "C": 3, "D": 2, "E": 1}

def load_canon(inf=6.0, none=0.0, min_known=4):
    rows = list(csv.DictReader(open(CANON_CSV, encoding="utf-8")))
    names, vecs, rowid, parts, users = [], [], [], [], []
    excluded = []
    for r in rows:
        v = []
        for s in STATS:
            g = r[s].strip()
            v.append(float(LET[g]) if g in LET else inf if g == "∞" else 1.0 if g == "Complete" else
                     (np.nan if none is None else none) if g == "∅" else np.nan)
        v = np.array(v)
        if np.isfinite(v).sum() >= min_known:
            names.append(r["display_name"]); vecs.append(v); rowid.append(int(r["wiki_row"])); parts.append(r["part"]); users.append(r["user"])
        else:
            excluded.append(r["display_name"])
    return names, np.array(vecs), np.array(rowid), parts, users, excluded

def all_profiles():
    return np.array(list(itertools.product([1, 2, 3, 4, 5], repeat=6)), dtype=float)   # col order PWR..DEV, E=1..A=5

BANDS_POP = np.array([0.10, 0.20, 0.40, 0.20, 0.10])             # E D C B A
def canon_bands():
    """per-stat grade shares over lettered canon cells (inf folded into A, Complete into E), shape (6,5) E..A"""
    rows = list(csv.DictReader(open(CANON_CSV, encoding="utf-8")))
    out = np.zeros((6, 5))
    for j, s in enumerate(STATS):
        for r in rows:
            g = r[s].strip(); g = "A" if g == "∞" else "E" if g == "Complete" else g
            if g in LET: out[j, LET[g] - 1] += 1
        out[j] /= out[j].sum()
    return out

def profile_weights(P, bands):
    """bands: (5,) shared or (6,5) per-stat probabilities for grade 1..5"""
    b = np.tile(bands, (6, 1)) if bands.ndim == 1 else bands
    w = np.ones(len(P))
    for j in range(6):
        w *= b[j][P[:, j].astype(int) - 1]
    return w

class Engine:
    def __init__(self, inf=6.0, none=0.0):
        self.names, self.V, self.rowid, self.parts, self.users, self.excluded = load_canon(inf, none)
        self.T = len(self.names); self.P = all_profiles()
        self.mask = np.isfinite(self.V); self.k = self.mask.sum(1)
        self.V0 = np.where(self.mask, self.V, 0.0)
        # identical-chart twin groups (identical numeric vector incl. missing pattern)
        key = [tuple(np.where(self.mask[i], self.V[i], -9)) for i in range(self.T)]
        gmap = {}; self.twin = np.array([gmap.setdefault(k, len(gmap)) for k in key]); self.ngroups = len(gmap)
        # difference tensor (P,T,6)
        self.diff = self.P[:, None, :] - self.V0[None, :, :]
        self.diff = np.where(self.mask[None], self.diff, 0.0)

    # ---- primary distances ----
    def d_l2(self, wfun=None):
        U = self.P[:, None, :]
        if wfun is None: w = np.ones_like(self.diff)
        else: w = wfun(self.P)[:, None, :] * np.ones((1, self.T, 1))
        w = np.where(self.mask[None], w, 0.0)
        return np.sqrt(6.0 * (w * self.diff ** 2).sum(2) / w.sum(2))
    def d_l1(self): return 6.0 * np.abs(self.diff).sum(2) / self.k[None]
    def d_cos(self, centre=3.0, mean_centre=False):
        a = self.P - (self.P.mean(1, keepdims=True) if mean_centre else centre)
        c = np.where(self.mask, self.V - centre, 0.0)
        if mean_centre:
            c = np.where(self.mask, self.V - np.nanmean(np.where(self.mask, self.V, np.nan), 1, keepdims=True), 0.0)
        am = np.where(self.mask[None], a[:, None, :], 0.0)
        num = (am * c[None]).sum(2); den = np.sqrt((am ** 2).sum(2)) * np.sqrt((c ** 2).sum(1))[None]
        with np.errstate(invalid="ignore", divide="ignore"):
            cs = np.where(den > 1e-12, num / den, np.nan)
        return 1.0 - cs      # 0 = identical direction; nan where undefined
    # ---- tie-break keys (same chain as production match.py after the primary distance) ----
    def tiekeys(self):
        sal = 6.0 / self.k[None] * (np.abs(self.P[:, None, :] - 3) * self.diff ** 2).sum(2)
        mx = np.abs(self.diff).max(2)
        exact = ((self.diff == 0) & self.mask[None]).sum(2)
        return [np.round(sal, 9), mx, -exact.astype(float), -np.broadcast_to(self.k[None], sal.shape).astype(float),
                np.broadcast_to(self.rowid[None], sal.shape).astype(float)]
    def pick(self, D, keys=None):
        """lexicographic argmin over (D, *tiekeys); returns index per profile"""
        keys = self.tiekeys() if keys is None else keys
        cand = np.ones(D.shape, bool)
        for K in [np.round(D, 9)] + keys:
            m = np.where(cand, K, np.inf).min(1, keepdims=True)
            cand &= (K == m)
        return cand.argmax(1)
    def topk(self, D, k=3):
        keys = self.tiekeys(); out = np.zeros((len(D), k), int)
        for i in range(len(D)):
            out[i] = np.lexsort((keys[4][i], keys[3][i], keys[2][i], keys[1][i], keys[0][i], np.round(D[i], 9)))[:k]
        return out

def gini(x):
    x = np.sort(np.asarray(x, float)); n = len(x); s = x.sum()
    if s == 0: return float("nan")
    return float((2 * np.arange(1, n + 1) - n - 1).dot(x) / (n * s))
