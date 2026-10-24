"""How much of a typical user's rated films the film table covers, measured on the pooled Letterboxd users.

  ~/lb_venv/bin/python -I tools/coverage.py --pool ~/samlearner/repo/data_processing --collect <collector out dir>

For each pooled user: share of their rated movies (TV removed) that are in the table, and the share that are also
eligible for Power and Precision (at least 30,000 Letterboxd ratings and a displayed average).
"""
import argparse, sys
from pathlib import Path

import numpy as np
import pandas as pd

sys.path.insert(0, str(Path(__file__).resolve().parent))
import calibrate as C  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pool", required=True)
    ap.add_argument("--collect", action="append", required=True)
    a = ap.parse_args()
    films = C.load_films(a.collect)
    pool = C.load_pool(a.pool)
    tv = set()
    for f in (Path(a.pool) / "data/rich_movie_data").glob("*.parquet"):
        m = pd.read_parquet(f, columns=["movie_id", "content_type"])
        tv |= set(m.loc[m.content_type == "tv", "movie_id"])
    pool = pool[~pool.movie_id.isin(tv)]
    elig = {k for k, v in films.items() if v["avg"] is not None and (v["count"] or 0) >= C.MIN_RATINGS}
    pool["in_table"] = pool.movie_id.isin(films)
    pool["eligible"] = pool.movie_id.isin(elig)
    g = pool.groupby("user_id").agg(n=("movie_id", "size"), t=("in_table", "sum"), e=("eligible", "sum"))
    print(f"table: {len(films)} movies, {len(elig)} eligible; pool: {len(g)} users, {int(g.n.sum())} movie ratings (TV removed)")
    for lo in (20, 50, 100):
        s = g[g.n >= lo]
        share, eshare = s.t / s.n, s.e / s.n
        q = lambda x: [round(float(np.quantile(x, p)) * 100, 1) for p in (0.10, 0.25, 0.50, 0.75, 0.90)]
        print(f"users with >= {lo} rated films ({len(s)}): share in table  p10/p25/median/p75/p90 = {q(share)}%;  "
              f"share usable for Power and Precision = {q(eshare)}%;  median eligible films {int(s.e.median())}")
    s = g[g.n >= 50]
    print(f"users with >= 50 eligible films (what Power and Precision need): {int((g.e >= 50).sum())} of {len(g[g.n >= 50])} users with >= 50 ratings")
    print(f"users where more than half of rated films are missing (Power/Precision would show ?): {float(((s.n - s.t) * 2 > s.n).mean()) * 100:.1f}% of users with >= 50 ratings")


if __name__ == "__main__":
    main()
