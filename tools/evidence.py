"""Evidence for the two deviations in docs/DECISIONS.md.

  ~/lb_venv/bin/python -I tools/evidence.py --pool ~/samlearner/repo/data_processing --collect <collector out dir> [...]

1. Crowd baseline: how far the Letterboxd displayed average is from the histogram plain mean, and how many table films
   have each.
2. Potential: how raw Good-Turing and the volume-matched version (20 draws of 60 films) depend on the number of
   director-credited films, over the pooled users restricted to the films in the table.
"""
import argparse, sys
from pathlib import Path

import numpy as np
import pandas as pd

sys.path.insert(0, str(Path(__file__).resolve().parent))
import calibrate as C  # noqa: E402


def spearman(x, y):
    rx, ry = pd.Series(x).rank().to_numpy(), pd.Series(y).rank().to_numpy()
    return float(np.corrcoef(rx, ry)[0, 1])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pool", required=True)
    ap.add_argument("--collect", action="append", required=True)
    a = ap.parse_args()
    films = C.load_films(a.collect)
    n = len(films)
    both = [(f["avg"], f["hist"]) for f in films.values() if f["avg"] is not None and f["hist"] is not None]
    avg_n = sum(f["avg"] is not None for f in films.values())
    hist_n = sum(f["hist"] is not None for f in films.values())
    d = np.array([x - y for x, y in both])
    print(f"== crowd baseline ({n} table films with a slug)")
    print(f"films with a displayed average: {avg_n} ({avg_n/n:.0%}); with a histogram plain mean: {hist_n} ({hist_n/n:.0%})")
    print(f"displayed average minus plain mean on {len(both)} films: mean {d.mean():+.4f}, RMSE {np.sqrt((d**2).mean()):.4f}, max |diff| {np.abs(d).max():.3f}")

    pool = C.load_pool(a.pool)
    pool = pool[pool.movie_id.isin(films)]
    pool = pool.sort_values("user_id", kind="stable").reset_index(drop=True)
    codes, _ = pd.factorize(pool.user_id)
    starts = np.flatnonzero(np.r_[True, codes[1:] != codes[:-1]])
    ends = np.r_[starts[1:], len(pool)]
    mid = pool.movie_id.to_numpy()
    dirs = {k: v["dirs"] for k, v in films.items()}
    short = {k: v["short"] for k, v in films.items()}
    vol, raw, g60 = [], [], []
    for ui, (s, e) in enumerate(zip(starts, ends)):
        dl = [dirs[m] for m in mid[s:e] if dirs[m] and not short[m]]
        if len(dl) < 60:
            continue
        cnt = {}
        for ds in dl:
            for x in ds:
                cnt[x] = cnt.get(x, 0) + 1
        raw.append(sum(1 for v in cnt.values() if v == 1) / sum(cnt.values()))
        rng = np.random.default_rng(5000 + ui)
        vals = []
        for _ in range(20):
            c2 = {}
            for i in rng.choice(len(dl), 60, replace=False):
                for x in dl[i]:
                    c2[x] = c2.get(x, 0) + 1
            vals.append(sum(1 for v in c2.values() if v == 1) / sum(c2.values()))
        g60.append(float(np.mean(vals))); vol.append(len(dl))
    vol, raw, g60 = np.array(vol), np.array(raw), np.array(g60)
    print(f"\n== Potential ({len(vol)} pooled users with >= 60 director-credited table films; median {np.median(vol):.0f} films)")
    print(f"Spearman with film count: raw Good-Turing {spearman(vol, raw):+.2f}, volume-matched (60 films) {spearman(vol, g60):+.2f}")
    for lo, hi in ((60, 100), (100, 200), (200, 10**6)):
        m = (vol >= lo) & (vol < hi)
        if m.sum() > 20:
            print(f"  films {lo}-{hi if hi < 10**6 else 'max'}: users {int(m.sum())}, median raw {np.median(raw[m]):.3f}, median volume-matched {np.median(g60[m]):.3f}")


if __name__ == "__main__":
    main()
