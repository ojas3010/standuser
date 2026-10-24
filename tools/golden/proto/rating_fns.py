"""Copy of the pair-bin / DPA functions of swarm/stats2/rating/common.py (that module imports pandas and scipy,
which the golden reference does not need). Only numpy is required. Function bodies are verbatim."""
import numpy as np

GAP_T = np.round(np.arange(0.25, 2.0001, 0.05), 4)          # lower edges of 36 gap bins; last bin is >= 2.0
GAP_MID = np.r_[(GAP_T[:-1] + GAP_T[1:]) / 2, 2.25]


def pair_bins(rv, c):
    """Return (hits[k], N[k]) over film pairs whose crowd gap falls in bin k (gap >= 0.25).
    hits = concordant + 0.5 * user-tied pairs.  Vectorised over thresholds."""
    rv = np.asarray(rv, int); c = np.asarray(c, float)
    n = len(c)
    o = np.argsort(c, kind="stable")
    cs, rs = c[o], rv[o]
    t = cs[:, None] - GAP_T[None, :]                      # (n, T): partner must have c_j <= c_i - delta
    cnt = np.empty((10, n, len(GAP_T)), dtype=np.int32)
    for l in range(10):
        cl = cs[rs == l + 1]
        cnt[l] = np.searchsorted(cl, t, side="right") if len(cl) else 0
    cum = np.cumsum(cnt, axis=0)                           # cum[l] = films with level <= l+1
    a = rs - 1
    idx = np.arange(n)
    below = np.where((a > 0)[:, None], cum[np.maximum(a - 1, 0), idx, :], 0)   # partners with lower rating: concordant
    tie = cnt[a, idx, :]
    tot = cum[9]
    conc_c = below.sum(0).astype(float)
    tie_c = tie.sum(0).astype(float)
    tot_c = tot.sum(0).astype(float)                        # cumulative (>= delta_k) pair totals
    hits_cum = conc_c + 0.5 * tie_c
    hits = hits_cum - np.r_[hits_cum[1:], 0.0]
    N = tot_c - np.r_[tot_c[1:], 0.0]
    return hits, N


def dpa_score(hits, N, h, pa_pop):
    """Difficulty-adjusted agreement: observed minus expected hit-rate on the same pairs, re-centred on the population PA."""
    tot = N.sum()
    if tot < 1:
        return np.nan
    return float((hits.sum() - (h * N).sum()) / tot + pa_pop)


def probit_pa(hits, N, beta_grid=None, at=0.5):
    """Quasi-ML probit discrimination: P(agree | gap g) = Phi(beta g).  Returns predicted agreement at gap `at` (stars)."""
    from scipy.stats import norm
    if N.sum() < 1:
        return np.nan, np.nan
    if beta_grid is None:
        beta_grid = np.linspace(0.02, 12.0, 600)
    p = norm.cdf(np.outer(beta_grid, GAP_MID))              # (B, K)
    p = np.clip(p, 1e-9, 1 - 1e-9)
    ll = (hits[None, :] * np.log(p) + (N - hits)[None, :] * np.log(1 - p)).sum(1)
    b = beta_grid[int(np.argmax(ll))]
    return float(norm.cdf(b * at)), float(b)
