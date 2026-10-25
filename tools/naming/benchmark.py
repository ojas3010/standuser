"""Item 6: tune the Cool weights on the canon album benchmark and report them.

Usage: python3 -I tools/naming/benchmark.py [--candidates local/naming/candidates.csv] [--out tools/naming/data/cool_weights.json]
Each canon song-Stand is ranked within its source album: every track scores the best Cool among its stage 5 and 6 candidates
(canon-collision and film-title filters off, since the target is a canon name); a track with no candidate scores -1. Ties count
as the expected rank over the tied positions. Albums are excluded by rule: more than 30 tracks, or an album title with
live / tour / session(s) / unplugged / broadcast / concert / compilation / greatest hits / best of / collection / anthology.
Train on Parts 4 and 5, test on Parts 6 to 8. The shipped Cool uses equal weights (cool.raw_equal): the grid search below finds
no weight set with support at this sample size, so its result is printed as a diagnostic only. The test-split table is a record,
not a gate: n=15 cannot separate Cool from the first-track rule (docs/DECISIONS.md).
"""
import argparse
import csv
import itertools
import json
import os
import random
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import cool as C  # noqa: E402
import textlib as t  # noqa: E402

EXCLUDE_TITLE = re.compile(r"\b(live|tours?|sessions?|unplugged|broadcast|concert|compilation|greatest hits|best of|collection|anthology)\b", re.I)
MAX_TRACKS = 30
TRAIN, TEST = {"4", "5"}, {"6", "7", "8"}
BOOT, SEED = 10000, 1
STEP = 0.25
LAMBDAS = (0.0, 0.25, 0.5)
POS = ["rarity", "shape", "conc", "emo", "prior"]


def split_albums(resolved):
    keep, dropped = [], []
    for r in resolved:
        why = []
        if len(r["tracks"]) > MAX_TRACKS:
            why.append(f"{len(r['tracks'])} tracks")
        if EXCLUDE_TITLE.search(r["album"]):
            why.append(f"live/tour/session release '{r['album']}'")
        (dropped if why else keep).append((r, why))
    return [r for r, _ in keep], dropped


def track_candidates(title):
    out = []
    for text, kind in t.stage5(title):
        if t.stage6(text, canon=False, film=False) or t.extra_checks(text):
            continue
        out.append((text, kind))
    return out


def prepare(albums, df, n):
    """Per Stand: (part, stand, target track index or None, k, [per-track [feature dicts]])."""
    items = []
    for r in albums:
        tracks = r["tracks"]
        tgt = tracks.index(r["source_track"]) if r["source_track"] in tracks else None
        per = [[C.features(text, kind, df, n) for text, kind in track_candidates(tt)] for tt in tracks]
        items.append((r["part"], r["stand"], tgt, len(tracks), per))
    return items


def expected_ranks(scores, tgt):
    g = sum(s > scores[tgt] for s in scores)
    ties = sum(s == scores[tgt] for s in scores)
    return g, ties


def metrics_of(g, ties):
    top1 = max(0, min(1 - g, ties)) / ties
    top5 = max(0, min(5 - g, ties)) / ties
    rr = sum(1 / (g + j) for j in range(1, ties + 1)) / ties
    return top1, top5, rr


def eval_weights(items, w):
    out = []
    for _, _, tgt, k, per in items:
        if tgt is None:
            out.append((0.0, 0.0, 1 / k))
            continue
        scores = [max((C.cool(f, w) for f in fs), default=-1.0) for fs in per]
        out.append(metrics_of(*expected_ranks(scores, tgt)))
    return out


def baseline_first(items):
    return [metrics_of(tgt, 1) if tgt is not None else (0.0, 0.0, 1 / k) for _, _, tgt, k, _ in items]


def baseline_random(items):
    out = []
    for _, _, _, k, _ in items:
        out.append((1 / k, min(5, k) / k, sum(1 / j for j in range(1, k + 1)) / k))
    return out


def mean(xs):
    return sum(xs) / len(xs)


def boot_ci(vals, rng):
    n = len(vals)
    ms = sorted(mean([vals[rng.randrange(n)] for _ in range(n)]) for _ in range(BOOT))
    return ms[int(0.025 * BOOT)], ms[int(0.975 * BOOT) - 1]


def table(rows, rng):
    res = {}
    for name, ms in rows.items():
        res[name] = {}
        for i, key in enumerate(("top1", "top5", "mrr")):
            v = [m[i] for m in ms]
            lo, hi = boot_ci(v, rng)
            res[name][key] = {"mean": mean(v), "lo": lo, "hi": hi}
    return res


def grid():
    k = int(round(1 / STEP))
    for combo in itertools.product(range(k + 1), repeat=len(POS)):
        if sum(combo) != k:
            continue
        for lam in LAMBDAS:
            w = {p: c * STEP for p, c in zip(POS, combo)}
            w["proper"] = lam
            yield w


def nonzero(w):
    return sum(1 for v in w.values() if v > 0)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--candidates", default=os.path.join(t.REPO, "local", "naming", "candidates.csv"))
    ap.add_argument("--albums", default=os.path.join(t.HERE, "data", "canon_albums.json"))
    ap.add_argument("--out", default=os.path.join(t.HERE, "data", "cool_weights.json"))
    ap.add_argument("--report", default=os.path.join(t.REPO, "local", "naming", "benchmark.json"))
    a = ap.parse_args()

    df, n = C.document_frequencies([r["name"] for r in csv.DictReader(open(a.candidates, encoding="utf-8"))])
    albums, dropped = split_albums(json.load(open(a.albums, encoding="utf-8"))["resolved"])
    print(f"benchmark: {len(albums)} Stands kept, {len(dropped)} excluded by rule")
    for r, why in dropped:
        print(f"  excluded: Part {r['part']} {r['stand']} ({'; '.join(why)})")
    if len(albums) < 30:
        sys.exit(f"STOP: {len(albums)} Stands under the 30-Stand floor")
    items = prepare(albums, df, n)
    train = [i for i in items if i[0] in TRAIN]
    test = [i for i in items if i[0] in TEST]
    other = [i for i in items if i[0] not in TRAIN | TEST]
    unrank = [i[1] for i in items if i[2] is None or not any(i[4][i[2]])]
    print(f"train {len(train)} (Parts 4-5), test {len(test)} (Parts 6-8), unused {len(other)} (spin-offs); target has no candidate: {unrank}")

    rng = random.Random(SEED)
    scored = sorted(((mean([m[2] for m in eval_weights(train, w)]), w) for w in grid()), key=lambda x: (-x[0], nonzero(x[1])))
    best_mrr, best_w = scored[0]
    lo, hi = boot_ci([m[2] for m in eval_weights(train, best_w)], rng)
    inside = [(m, w) for m, w in scored if m >= lo]
    tuned_w = min(inside, key=lambda x: (nonzero(x[1]), -x[0]))[1]
    print(f"diagnostic grid search on train: best MRR {best_mrr:.3f} [{lo:.3f}, {hi:.3f}], {len(inside)} of {len(scored)} sets inside the interval; simplest {tuned_w}")
    equal_w = {**{k: 1.0 for k in POS}, "proper": 1.0}

    out = {}
    for split, its in (("train", train), ("test", test)):
        rows = {"random": baseline_random(its), "first-track": baseline_first(its), "equal-weight Cool (shipped)": eval_weights(its, equal_w),
                "train-tuned Cool (diagnostic)": eval_weights(its, tuned_w)}
        out[split] = table(rows, random.Random(SEED))
        print(f"\n{split} (n={len(its)})  top-1 / top-5 / MRR, bootstrap 95%")
        for name, res in out[split].items():
            print(f"  {name:30s} " + "   ".join(f"{res[k]['mean']:.2f} [{res[k]['lo']:.2f},{res[k]['hi']:.2f}]" for k in ("top1", "top5", "mrr")))
    for name, w in (("equal-weight", equal_w), ("train-tuned", tuned_w)):
        d = [x[2] - y[2] for x, y in zip(eval_weights(test, w), baseline_first(test))]
        dlo, dhi = boot_ci(d, random.Random(SEED))
        out[f"test_mrr_diff_vs_first_track_{name}"] = {"mean": mean(d), "lo": dlo, "hi": dhi}
        print(f"test MRR, {name} minus first-track (paired): {mean(d):+.3f} [{dlo:+.3f}, {dhi:+.3f}]")

    os.makedirs(os.path.dirname(a.report), exist_ok=True)
    json.dump({"excluded": [[r["stand"], w] for r, w in dropped], "results": out, "diagnostic_tuned_weights": tuned_w}, open(a.report, "w"), indent=1)
    json.dump({"weights": equal_w, "features": C.FEATURES, "benchmark_stands": len(albums),
               "reason": "no weight set has support in the data (n=9 train, n=15 test), so equal weights are the uninformed default",
               "scale": "raw = sum of five features - proper, min-max scaled over the candidates table (scale_lo, scale_hi in names meta)"},
              open(a.out, "w"), indent=1)


if __name__ == "__main__":
    main()
