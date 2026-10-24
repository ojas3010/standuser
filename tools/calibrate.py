"""Recalibrate the grade cut-points against the films in our table, using the pooled Letterboxd users of
sdl60660/letterboxd_recommendations (rating samples; join on the Letterboxd film slug).

  ~/lb_venv/bin/python -I tools/calibrate.py --pool ~/samlearner/repo/data_processing \
      --collect ~/lb_collect_out [--collect another_dir ...] [--source avg|hist] --out tools/calibration.json

Recalibrated: Power, Precision (with a new difficulty curve h(gap) and population accuracy), Range and Potential.
Speed and Stamina keep their MovieTweetings cuts (tools/build_tables.py). The result is merged into cuts.json by
tools/build_tables.py. Procedure follows the prototypes in swarm/stats2/rating (p1, p5, p6, p7).

  Power      user offset d = rating - crowd over films with >= 30,000 Letterboxd ratings; users with >= 50 such films;
             shrunk with lambda 6 toward the population mean offset
  Precision  DPA from 0.05-star gap bins; h(gap) = user-equal pooled hit rate, monotone (pool-adjacent-violators)
  Range      rarefied effective countries: 20 draws of 50 films with fractional countries; users with >= 50 films
  Potential  Good-Turing share of director credits seen once on 20 draws of 60 films; users with >= 60 director-credited
             films, graded within film-count bands (60-100, 100-200, 200+; a band under 1,000 users merges into its
             neighbour), with the Spearman correlation with film count reported per band
"""
import argparse, json, sys, time
from pathlib import Path

import numpy as np
import pandas as pd

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE / "golden/proto"))
import rating_fns as RF  # noqa: E402

QS = (0.01, 0.02, 0.05, 0.10, 0.25, 0.30, 0.50, 0.70, 0.75, 0.90, 0.95, 0.98, 0.99)
CANON_CUM = dict(PWR=[26.3, 38.7, 58.4, 77.4], PRC=[38.7, 56.3, 83.8, 91.5], RNG=[15.6, 31.2, 59.6, 75.2], DEV=[27.9, 42.2, 70.1, 81.5])
LAM, MIN_RATINGS = 6.0, 30000
MIN_BAND_USERS = 1000


def read_jsonl(p):
    return [json.loads(l) for l in open(p, encoding="utf-8") if l.strip()] if Path(p).exists() else []


def load_films(dirs):
    films = {}
    for d in dirs:
        hist = {h["slug"]: h for h in read_jsonl(Path(d) / "histograms.jsonl") if h.get("complete")}
        for r in read_jsonl(Path(d) / "films_meta.jsonl"):
            slug = r.get("lb_slug")
            if r.get("tmdb_type") != "movie" or not slug or slug in films:
                continue
            h = hist.get(slug)
            films[slug] = dict(
                avg=r.get("lb_avg"), hist=h["plain_mean"] if h and h.get("plain_mean") else None,
                count=r.get("lb_rating_count") or (h["total"] if h else None),
                countries=r.get("production_countries") or [], dirs=sorted({x["id"] for x in (r.get("directors") or [])}),
                short=bool(r.get("runtime")) and r["runtime"] <= 40)
    return films


def load_pool(root):
    root = Path(root)
    parts = []
    for f in sorted((root / "data/training_data_samples").glob("*.parquet")):
        parts.append(pd.read_parquet(f, columns=["user_id", "movie_id", "rating_val"]))
    t = root / "testing/test_user_data.parquet"
    if t.exists():
        parts.append(pd.read_parquet(t, columns=["user_id", "movie_id", "rating_val"]))
    o = root / "data/old_training_data_for_testing.csv"
    if o.exists():
        parts.append(pd.read_csv(o, usecols=["user_id", "movie_id", "rating_val"]))
    p = pd.concat(parts, ignore_index=True).drop_duplicates(["user_id", "movie_id"], keep="last")
    return p


def isotonic(y, w, lo=0.5, hi=1.0):
    """Weighted pool-adjacent-violators for an increasing fit, clipped to [lo, hi]."""
    blocks = []
    for yi, wi in zip(y, w):
        blocks.append([yi * wi, wi, 1])
        while len(blocks) > 1 and blocks[-2][0] / blocks[-2][1] > blocks[-1][0] / blocks[-1][1]:
            b = blocks.pop()
            blocks[-1][0] += b[0]; blocks[-1][1] += b[1]; blocks[-1][2] += b[2]
    out = []
    for s, wt, n in blocks:
        out += [min(max(s / wt, lo), hi)] * n
    return np.array(out)


def merge_edges(cnt, edges=(60, 100, 200)):
    """Band lower edges; a band with fewer than MIN_BAND_USERS users merges into its upper neighbour (the top one downward)."""
    edges = list(edges)
    while len(edges) > 1:
        sizes = [int(((cnt >= lo) & ((cnt < edges[i + 1]) if i + 1 < len(edges) else True)).sum()) for i, lo in enumerate(edges)]
        small = [i for i, n in enumerate(sizes) if n < MIN_BAND_USERS]
        if not small:
            break
        i = small[0]
        edges.pop(i + 1) if i + 1 < len(edges) else edges.pop()
    return edges


def knots(x, qs=QS):
    out = []
    for q in qs:
        v = float(np.quantile(x, q))
        if not out or v > out[-1][0] + 1e-9:
            out.append([round(v, 6), q])
    return out


def cuts_at(x, ps):
    return [round(float(np.quantile(x, p)), 6) for p in ps]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pool", required=True)
    ap.add_argument("--collect", action="append", required=True)
    ap.add_argument("--source", choices=["avg", "hist"], default="avg")
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    t0 = time.time()
    films = load_films(a.collect)
    pool = load_pool(a.pool)
    print(f"table films with a slug: {len(films)}; pool ratings {len(pool)}, users {pool.user_id.nunique()}, films {pool.movie_id.nunique()}")
    cov_all = pool.movie_id.isin(films).mean()
    pool = pool[pool.movie_id.isin(films)].copy()
    print(f"ratings on table films: {len(pool)} ({cov_all:.1%} of pool ratings), users {pool.user_id.nunique()}")
    fm = pd.DataFrame.from_dict(films, orient="index")
    pool["r"] = pool.rating_val / 2.0
    crowd = fm["avg" if a.source == "avg" else "hist"]
    pool["c"] = pool.movie_id.map(crowd)
    pool["n"] = pool.movie_id.map(fm["count"]).fillna(0)
    pool = pool.sort_values("user_id", kind="stable").reset_index(drop=True)
    codes, _ = pd.factorize(pool.user_id)
    starts = np.flatnonzero(np.r_[True, codes[1:] != codes[:-1]])
    ends = np.r_[starts[1:], len(pool)]
    mid = pool.movie_id.to_numpy()
    r, c, rv, n_cnt = pool.r.to_numpy(), pool.c.to_numpy(), pool.rating_val.to_numpy(), pool.n.to_numpy()
    ctry = fm.countries.to_dict(); dirs = fm.dirs.to_dict(); short = fm.short.to_dict()
    GAP = len(RF.GAP_T)
    U = dict(pw_n=[], pw_b=[], H=[], N=[], rng=[], gt60=[], gt_raw=[], pot_n=[])
    for ui, (s, e) in enumerate(zip(starts, ends)):
        sl = slice(s, e)
        el = (n_cnt[sl] >= MIN_RATINGS) & ~np.isnan(c[sl])
        if el.sum() >= 50:
            rr, cc, vv = r[sl][el], c[sl][el], rv[sl][el]
            U["pw_n"].append(len(rr)); U["pw_b"].append(float((rr - cc).mean()))
            h, n = RF.pair_bins(vv, cc)
            U["H"].append(h); U["N"].append(n)
        films_u = [m for m in mid[sl] if not short[m]]
        cl = [ctry[m] for m in films_u if ctry[m]]
        if len(cl) >= 50:
            rng = np.random.default_rng(1000 + ui)
            vals = []
            for _ in range(20):
                idx = rng.choice(len(cl), 50, replace=False)
                w = {}
                for i in idx:
                    k = len(cl[i])
                    for cc_ in cl[i]:
                        w[cc_] = w.get(cc_, 0.0) + 1.0 / k
                p = np.array(list(w.values())) / 50.0
                vals.append(float(np.exp(-(p * np.log(p)).sum())))
            U["rng"].append(float(np.mean(vals)))
        dl = [dirs[m] for m in films_u if dirs[m]]
        if len(dl) >= 60:
            rng = np.random.default_rng(5000 + ui)
            vals = []
            for _ in range(20):
                cnt = {}
                for i in rng.choice(len(dl), 60, replace=False):
                    for d in dl[i]:
                        cnt[d] = cnt.get(d, 0) + 1
                vals.append(sum(1 for v in cnt.values() if v == 1) / sum(cnt.values()))
            U["gt60"].append(float(np.mean(vals)))
            allc = {}
            for ds in dl:
                for d in ds:
                    allc[d] = allc.get(d, 0) + 1
            U["gt_raw"].append(sum(1 for v in allc.values() if v == 1) / sum(allc.values())); U["pot_n"].append(len(dl))
    print(f"users: power/precision {len(U['pw_n'])}, range {len(U['rng'])}, potential {len(U['gt60'])}  ({time.time()-t0:.0f}s)")
    out = dict(source=a.source, built=time.strftime("%Y-%m-%d"), table_films=len(films), pool_ratings_on_table=int(len(pool)),
               users=dict(power_precision=len(U["pw_n"]), range=len(U["rng"]), potential=len(U["gt60"])), stats={})
    # ---- Power
    nn, bb = np.array(U["pw_n"]), np.array(U["pw_b"])
    if len(nn) >= 200:
        mu = float(bb.mean())
        power = -((bb * nn + LAM * mu) / (nn + LAM))
        out["mu_b"] = round(mu, 6)
        out["stats"]["PWR"] = dict(knots=knots(power), letter_cuts=cuts_at(power, (.1, .3, .7, .9)), canon_cuts=cuts_at(power, [x / 100 for x in CANON_CUM["PWR"]]),
                                   median_n=float(np.median(nn)))
        # ---- Precision
        H, N = np.array(U["H"]), np.array(U["N"])
        tot = N.sum(1)
        keep = tot >= 500
        H, N, tot = H[keep], N[keep], tot[keep]
        w = 1.0 / tot[:, None]
        h_raw = (H * w).sum(0) / np.maximum((N * w).sum(0), 1e-12)
        h = isotonic(h_raw, (N * w).sum(0))
        pa_pop = float(np.mean(H.sum(1) / tot))
        dpa = np.array([RF.dpa_score(H[i], N[i], h, pa_pop) for i in range(len(H))])
        out["pa_pop"] = round(pa_pop, 6)
        out["h"] = [round(float(x), 6) for x in h]
        out["stats"]["PRC"] = dict(knots=knots(dpa), letter_cuts=cuts_at(dpa, (.1, .3, .7, .9)), canon_cuts=cuts_at(dpa, [x / 100 for x in CANON_CUM["PRC"]]))
        print(f"Power: users {len(nn)}, mu_b {mu:+.4f}, cuts {out['stats']['PWR']['letter_cuts']}")
        print(f"Precision: users {len(dpa)}, pa_pop {pa_pop:.4f}, cuts {out['stats']['PRC']['letter_cuts']}")
    if len(U["rng"]) >= 200:
        x = np.array(U["rng"])
        out["inf_cut_RNG"] = round(float(np.quantile(x, 0.999)), 6)
        out["stats"]["RNG"] = dict(knots=knots(x), letter_cuts=cuts_at(x, (.1, .3, .7, .9)), canon_cuts=cuts_at(x, [v / 100 for v in CANON_CUM["RNG"]]))
        print(f"Range: users {len(x)}, cuts {out['stats']['RNG']['letter_cuts']}, median {np.median(x):.3f}")
    if len(U["gt60"]) >= 200:
        x = np.array(U["gt60"])
        cnt = np.array(U["pot_n"])
        # film-count bands (director-credited films): 60-100, 100-200, 200+; a band under 1,000 users merges upward
        edges = merge_edges(cnt)
        bands = []
        for i, lo in enumerate(edges):
            hi = edges[i + 1] if i + 1 < len(edges) else None
            m = (cnt >= lo) & ((cnt < hi) if hi else True)
            xb = x[m]
            rho = float(pd.Series(cnt[m]).rank().corr(pd.Series(xb).rank()))
            bands.append(dict(lo=lo, hi=hi, users=int(m.sum()), knots=knots(xb), letter_cuts=cuts_at(xb, (.1, .3, .7, .9)),
                              canon_cuts=cuts_at(xb, [v / 100 for v in CANON_CUM["DEV"]]), spearman_with_film_count=round(rho, 4),
                              median=round(float(np.median(xb)), 4)))
            print(f"Potential band {lo}-{hi or 'max'}: users {m.sum()}, median {np.median(xb):.3f}, cuts {bands[-1]['letter_cuts']}, Spearman with film count {rho:+.3f}")
        out["stats"]["DEV"] = dict(knots=knots(x), letter_cuts=cuts_at(x, (.1, .3, .7, .9)), canon_cuts=cuts_at(x, [v / 100 for v in CANON_CUM["DEV"]]),
                                   bands=bands, band_edges=edges, any_band_over_0_2=any(abs(b["spearman_with_film_count"]) > 0.2 for b in bands),
                                   median_credit_films=float(np.median(cnt)), median_raw_gt=float(np.median(U["gt_raw"])))
        print(f"Potential (GT on 60 films), all users pooled (fallback): users {len(x)}, cuts {out['stats']['DEV']['letter_cuts']}")
    Path(a.out).write_text(json.dumps(out, indent=1))
    print("wrote", a.out)


if __name__ == "__main__":
    main()
