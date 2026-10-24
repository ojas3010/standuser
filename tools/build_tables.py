"""Build the calibration tables shipped inside standuser-core (crates/standuser-core/data/).

Inputs are the research outputs under ../swarm (spec: reports/Letterboxd JoJo Stand stats 2.md).
Run: python3 -I tools/build_tables.py     (pure standard library)
"""
import csv, json, shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SW = ROOT.parent / "swarm"
OUT = ROOT / "crates/standuser-core/data"
OUT.mkdir(parents=True, exist_ok=True)


def quantile(xs, p):  # numpy "linear" definition
    xs = sorted(xs)
    k = (len(xs) - 1) * p
    f = int(k)
    c = min(f + 1, len(xs) - 1)
    return xs[f] + (xs[c] - xs[f]) * (k - f)


def dedupe(knots):
    out = []
    for v, p in knots:
        if out and v <= out[-1][0]:
            continue
        out.append([round(v, 6), p])
    return out


# MovieTweetings users with >= 20 clean events and >= 12 months (reproduces the spec cut-points)
mt = [r for r in csv.DictReader(open(SW / "stats2/dates/out/mt_user_metrics_N8.csv"))
      if float(r["n_win"]) >= 20 and float(r["L"]) >= 12]
QS = (0.05, 0.10, 0.25, 0.30, 0.50, 0.70, 0.75, 0.90, 0.95)
speed = dedupe([(quantile([float(r["pace_active"]) for r in mt], p), p) for p in QS])
stamina = dedupe([(quantile([float(r["share"]) for r in mt], p), p) for p in QS])

cuts = {
    "PWR": dict(knots=[[-0.4105, .10], [-0.1651, .30], [-0.0175, .50], [0.1360, .70], [0.3824, .90]],
                letter_cuts=[-0.4105, -0.1651, 0.1360, 0.3824], canon_cuts=[-0.2017, -0.1031, 0.0396, 0.2010],
                canon_cum=[26.3, 38.7, 58.4, 77.4], status="measured",
                source="LB pool, 24,929 users, pool-scale crowd (spec table; recalibrate on Letterboxd plain means)"),
    "SPD": dict(knots=speed, letter_cuts=[1.818, 2.400, 4.042, 6.483], canon_cuts=[2.077, 2.333, 3.167, 4.948],
                canon_cum=[18.5, 28.1, 52.8, 80.2], status="provisional",
                source="MovieTweetings proxy, 6,518 users"),
    "RNG": dict(knots=[[1.795, .05], [1.925, .10], [2.164, .25], [2.23, .30], [2.472, .50], [2.755, .70], [2.848, .75], [3.284, .90], [3.597, .95]],
                letter_cuts=[1.925, 2.230, 2.755, 3.284], canon_cuts=None,
                canon_cum=[15.6, 31.2, 59.6, 75.2], status="provisional",
                source="TMDB-5000 subset of the LB pool, 19,824 users (US-heavy proxy; spec: no valid cut-points yet)"),
    "STA": dict(knots=stamina, letter_cuts=[0.286, 0.436, 0.698, 0.867], canon_cuts=[0.210, 0.250, 0.375, 0.527],
                canon_cum=[4.0, 7.3, 21.2, 44.4], status="provisional",
                source="MovieTweetings proxy, 6,518 users"),
    "PRC": dict(knots=[[0.614, .05], [0.6527, .10], [0.7234, .30], [0.7656, .50], [0.8023, .70], [0.8475, .90], [0.865, .95]],
                letter_cuts=[0.6527, 0.7234, 0.8023, 0.8475], canon_cuts=[0.7442, 0.7783, 0.8313, 0.8538],
                canon_cum=[38.7, 56.3, 83.8, 91.5], status="measured",
                source="LB pool, 24,929 users (spec table)"),
    "DEV": dict(knots=None, letter_cuts=None, canon_cuts=None,
                canon_cum=[27.9, 42.2, 70.1, 81.5], status="ungraded",
                source="no valid cut-points (spec: MT and ML medians disagree)"),
}
mu_b, pa_pop = 0.0139, 0.7562
gap = list(csv.DictReader(open(SW / "stats2/rating/out/gap_curve_K100.csv")))
gap_lo, gap_h = [float(r["gap_lo"]) for r in gap], [round(float(r["hit_rate_iso"]), 6) for r in gap]
calib = ROOT / "tools/calibration.json"
meta = dict(calibrated=False)
if calib.exists():   # recalibration against the films in our table (tools/calibrate.py)
    c = json.loads(calib.read_text())
    for k, v in c["stats"].items():
        # Potential stays provisional while any film-count band still correlates with film count (|Spearman| > 0.2)
        # Range is a full grade from 100 films (stats.rs marks fewer films provisional); Potential stays provisional while
        # any film-count band still correlates with film count
        prov = k == "DEV" and v.get("any_band_over_0_2", True)
        cuts[k].update(knots=v["knots"], letter_cuts=v["letter_cuts"], canon_cuts=v["canon_cuts"],
                       status="provisional" if prov else "measured",
                       source=f"pooled Letterboxd users on our film table (built {c['built']}, {c['table_films']} films, {c['users']})")
    if "bands" in c["stats"].get("DEV", {}):
        cuts["DEV"]["bands"] = c["stats"]["DEV"]["bands"]
        cuts["DEV"]["any_band_over_0_2"] = c["stats"]["DEV"]["any_band_over_0_2"]
    if "RNG" in c["stats"]:
        cuts["RNG"]["inf_cut"] = c["inf_cut_RNG"]
    mu_b, pa_pop, gap_h = c["mu_b"], c["pa_pop"], c["h"]
    meta = dict(calibrated=True, built=c["built"], table_films=c["table_films"], users=c["users"], crowd=c["source"])
json.dump(dict(stats=cuts, mu_b=mu_b, lambda_=6.0, pa_pop=pa_pop, crowd_min_ratings=30000, calibration=meta,
               speed_stamina_filter="MT users with >=20 clean events and >=12 months"),
          open(OUT / "cuts.json", "w"), indent=1)
json.dump(dict(gap_lo=gap_lo, h=gap_h), open(OUT / "gap_h.json", "w"))

shutil.copy(SW / "canon/stand_stats.csv", OUT / "canon.csv")
src = ROOT.parent / "research_notes/Stand naming algorithm/canon_name_sources.csv"
rows = list(csv.DictReader(open(src, encoding="utf-8")))
with open(OUT / "canon_names.txt", "w", encoding="utf-8") as f:
    for n in sorted({r["display_name"] for r in rows}):
        f.write(n + "\n")
print("speed knots", speed)
print("stamina knots", stamina)
print("gap bins", len(gap), "canon names", len({r['display_name'] for r in rows}))
