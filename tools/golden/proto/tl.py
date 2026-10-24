"""Timeline helpers for the date-based Stand stats (Speed, Stamina).

All functions are pure and take plain numpy arrays of calendar-day numbers
(days since 1970-01-01), one entry per film-first-watch event.
"""
import numpy as np

BULK_N = 8           # day with >= BULK_N films and < half live-logged -> bulk
BULK_CAP = 14        # day with >= BULK_CAP films -> always bulk (24 h / ~105 min per film = 13.7)


def month_of(days):
    """Month index (months since 1970-01) for calendar-day numbers."""
    d = np.asarray(days, dtype="int64").astype("datetime64[D]")
    return d.astype("datetime64[M]").astype("int64")


def complete_month_window(days, as_of_day, t0_day=None):
    """Months strictly after the month of T0 and strictly before the month of as_of.

    T0 defaults to the first event.  Returns (m0, m1) inclusive or None if empty.
    """
    days = np.asarray(days)
    if len(days) == 0:
        return None
    t0 = days.min() if t0_day is None else t0_day
    m0 = month_of([t0])[0] + 1
    m1 = month_of([as_of_day])[0] - 1
    if m1 < m0:
        return None
    return int(m0), int(m1)


def longest_run(flags):
    best = cur = 0
    for f in flags:
        cur = cur + 1 if f else 0
        best = max(best, cur)
    return best


def run_lengths(flags):
    out, cur = [], 0
    for f in flags:
        if f:
            cur += 1
        elif cur:
            out.append(cur)
            cur = 0
    if cur:
        out.append(cur)
    return out


def burstiness(gaps):
    """Goh-Barabasi B and Kim-Jo finite-size corrected A_n for n = len(gaps)."""
    g = np.asarray(gaps, dtype=float)
    n = len(g)
    if n < 3:
        return np.nan, np.nan
    mu, sd = g.mean(), g.std(ddof=0)
    if mu + sd == 0:
        return np.nan, np.nan
    b = (sd - mu) / (sd + mu)
    r = sd / mu if mu > 0 else np.nan
    # Kim & Jo (2016) Eq. 22: A_n = (sqrt(n+1)*r - sqrt(n-1)) / ((sqrt(n+1)-2)*r + sqrt(n-1))
    num = np.sqrt(n + 1) * r - np.sqrt(n - 1)
    den = (np.sqrt(n + 1) - 2) * r + np.sqrt(n - 1)
    a = num / den if den != 0 else np.nan
    return b, a


def user_metrics(days, as_of_day, t0_day=None, min_events=1):
    """Candidate Speed and Stamina measures on one clean event timeline.

    days: calendar-day numbers of clean events (any order); as_of_day: calendar day.
    """
    days = np.sort(np.asarray(days, dtype="int64"))
    w = complete_month_window(days, as_of_day, t0_day)
    if w is None or len(days) < min_events:
        return None
    m0, m1 = w
    L = m1 - m0 + 1
    months = month_of(days)
    inwin = (months >= m0) & (months <= m1)
    ev = days[inwin]
    em = months[inwin]
    if len(ev) == 0:
        return dict(L=L, n_win=0, active=0, share=0.0)
    counts = np.bincount(em - m0, minlength=L)
    flags = counts > 0
    a = int(flags.sum())
    d_unique = np.unique(ev)
    gaps = np.diff(d_unique)
    b, an = burstiness(gaps) if len(gaps) >= 3 else (np.nan, np.nan)
    runs = run_lengths(flags)
    # peak 30-day throughput: max number of events in any window of 30 consecutive days
    j = np.searchsorted(ev, ev + 30, side="left")
    peak30 = int((j - np.arange(len(ev))).max())
    # per active week
    weeks = np.unique(ev // 7)
    return dict(
        L=L, n_win=int(len(ev)), active=a, share=a / L,
        streak=int(max(runs)) if runs else 0,
        streak_share=(max(runs) / L) if runs else 0.0,
        median_run=float(np.median(runs)) if runs else 0.0,
        pace_active=len(ev) / a,                 # events per active month
        pace_median=float(np.median(counts[flags])),
        rate=len(ev) / L,                        # events per month incl. inactive
        pace_week=len(ev) / len(weeks),          # events per active week
        density=len(ev) / len(d_unique),         # events per active day
        active_days=int(len(d_unique)),
        peak30=peak30,
        B=b, A_n=an,
        quarters_share=_quarter_share(flags),
    )


def _quarter_share(flags):
    f = np.asarray(flags, dtype=bool)
    q = len(f) // 3
    if q < 1:
        return np.nan
    return float(f[: q * 3].reshape(q, 3).any(axis=1).mean())
