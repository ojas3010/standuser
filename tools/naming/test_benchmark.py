"""Tests for the benchmark's metric and exclusion rule.

Run:  python3 -I -m pytest -q tools/naming/test_benchmark.py
"""
import os
import sys

import pytest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import benchmark as B  # noqa: E402


def test_tied_scores_share_the_expected_rank():
    assert B.metrics_of(0, 1) == (1.0, 1.0, 1.0)
    top1, top5, rr = B.metrics_of(0, 4)  # four tied at the top
    assert top1 == pytest.approx(0.25) and top5 == 1.0 and rr == pytest.approx((1 + 1 / 2 + 1 / 3 + 1 / 4) / 4)
    assert B.metrics_of(5, 1) == (0.0, 0.0, pytest.approx(1 / 6))


def test_expected_ranks_counts_strictly_greater_and_ties():
    assert B.expected_ranks([0.9, 0.5, 0.5, 0.1], 1) == (1, 2)
    assert B.expected_ranks([-1.0, -1.0], 0) == (0, 2)


def test_random_baseline_is_1_over_k_and_h_k_over_k():
    (top1, top5, mrr), = B.baseline_random([(None, None, None, 10, None)])
    assert top1 == 0.1 and top5 == 0.5 and mrr == pytest.approx(0.2929, abs=1e-4)


def test_exclusion_rule_drops_long_and_live_albums_by_rule():
    mk = lambda album, n: {"stand": album, "album": album, "tracks": ["x"] * n}
    keep, dropped = B.split_albums([mk("Machine Head", 7), mk("Rockin' With Elvis", 31), mk("Tour '90", 15), mk("The Nashville Sessions", 20),
                                    mk("Greatest Hits", 12), mk("Sheer Heart Attack", 13), mk("Delivered", 30)])
    assert [r["album"] for r in keep] == ["Machine Head", "Sheer Heart Attack", "Delivered"]
    assert len(dropped) == 4


def test_grid_weights_sum_to_one_and_include_the_penalty():
    ws = list(B.grid())
    assert len(ws) == 70 * 3
    assert all(abs(sum(w[p] for p in B.POS) - 1) < 1e-9 for w in ws)


def test_equal_weight_cool_is_the_sum_minus_the_penalty_and_scales_to_the_unit_interval():
    import cool as C
    f = dict(rarity=0.5, shape=1.0, conc=0.5, emo=0.5, prior=1.0, proper=0.0)
    assert C.raw_equal(f) == pytest.approx(3.5)
    assert C.raw_equal({**f, "proper": 1.0}) == pytest.approx(2.5)
    assert C.scale(3.5, 0.5, 4.5) == pytest.approx(0.75)
    assert C.scale(-1, 0.5, 4.5) == 0.0 and C.scale(9, 0.5, 4.5) == 1.0
