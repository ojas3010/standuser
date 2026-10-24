#!/usr/bin/env python3
"""collect.py - measurement script for the Letterboxd -> JoJo Stand calibration.

This is a one-off data collector, not the product.  It runs on a normal internet connection
and produces three files that can be shared back:

  films_meta.jsonl   one record per film (own watched films, then the calibration sample)
  histograms.jsonl   Letterboxd rating histograms (own films + a random subset of the sample)
  run_summary.json   counts, resolution paths, failures

Inputs
  --export DIR          Letterboxd export folder (only watched.csv is read; profile.csv is never opened)
  TMDB_API_KEY          environment variable: a TMDB v3 API key or a v4 "read access token"
  --sample-slugs FILE   optional CSV/TXT of Letterboxd slugs (the samlearner sample's movie_id column)

Everything fetched is cached in <out>/cache.sqlite, so an interrupted run resumes where it stopped and a
finished run can be re-assembled offline (--assemble-only).

Live-site assumptions (see RUN.md): film pages carry <body data-tmdb-id data-tmdb-type>, a TMDB and an IMDb
anchor (data-track-action), and JSON-LD with aggregateRating; boxd.it/<code> answers with a 302 whose Location
is /film/<slug>/; /csi/film/<slug>/rating-histogram/ returns li.rating-histogram-bar tooltips.
"""
from __future__ import annotations

import argparse
import csv
import difflib
import json
import math
import os
import random
import re
import sqlite3
import sys
import time
import unicodedata
from collections import Counter
from collections.abc import Callable
from dataclasses import dataclass, field
from datetime import UTC, datetime
from pathlib import Path
from typing import Any
from urllib.parse import quote, unquote, urljoin, urlparse

__version__ = "1.0.0"

B62 = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"
LB_HOST = "https://letterboxd.com"
BOXD_HOST = "https://boxd.it"
TMDB_BASE = "https://api.themoviedb.org/3"
CANARY_SLUG = "the-zone-of-interest"
CANARY_TMDB_ID = 467244
RETRY_STATUS = {403, 408, 425, 429, 500, 502, 503, 504, 520, 521, 522, 523, 524}
REDIRECT_STATUS = {301, 302, 303, 307, 308}
COMPOSER_JOBS = ("Original Music Composer", "Music")
STAR_STEPS = [0.5 * i for i in range(1, 11)]
MISSING = object()


class UsageError(Exception):
    """Bad invocation or bad input folder; reported without a traceback."""


class TmdbAuthError(Exception):
    """TMDB rejected the API key; nothing else can work."""


# --------------------------------------------------------------------------------------
# small helpers
# --------------------------------------------------------------------------------------

def decode_lid(code: str) -> tuple[int, int]:
    """boxd.it code -> (internal uid, type digit).  0 = film, 1 = diary/review entry, 2 = list."""
    n = 0
    for ch in code:
        n = n * 62 + B62.index(ch)  # ValueError on a character outside the alphabet
    return divmod(n, 10)


def lid_from_uri(uri: str | None) -> str | None:
    m = re.search(r"boxd\.it/([0-9A-Za-z]+)", uri or "")
    return m.group(1) if m else None


def norm_title(s: str | None) -> str:
    """ASCII-fold, lower-case, drop punctuation and a leading article, so 'The Intouchables' == 'Intouchables'."""
    s = unicodedata.normalize("NFKD", s or "").encode("ascii", "ignore").decode("ascii").lower()
    s = s.replace("&", " and ")
    s = re.sub(r"[^a-z0-9]+", " ", s).strip()
    return re.sub(r"^(the|a|an) ", "", s)


def year_of(date_str: Any) -> int | None:
    m = re.match(r"(\d{4})", str(date_str or ""))
    return int(m.group(1)) if m else None


def slug_from_url(url: str | None) -> str | None:
    if not url:
        return None
    try:
        p = urlparse(url)
    except ValueError:
        return None
    allowed = {urlparse(LB_HOST).netloc.lower(), urlparse(BOXD_HOST).netloc.lower(), "www.letterboxd.com"}
    if p.netloc and p.netloc.lower() not in allowed:
        return None
    m = re.search(r"/film/([^/?#]+)", p.path)
    return unquote(m.group(1)) if m else None


def now_iso() -> str:
    return datetime.now(UTC).isoformat(timespec="seconds")


def parse_count(num: str, suffix: str | None) -> tuple[int, bool]:
    """'4,580' -> (4580, False); '13.5' + 'K' -> (13500, True).  Raises ValueError on garbage."""
    val = float(num.replace(",", ""))
    if suffix:
        val *= 1000 if suffix.lower() == "k" else 1_000_000
        return round(val), True
    return round(val), False


def stars_to_value(label: str) -> float | None:
    """'half-★' -> 0.5, '★' -> 1, '★½' -> 1.5, '★★★★★' -> 5."""
    label = label.strip()
    if "half" in label.lower() or label == "½":
        return 0.5
    n = label.count("★")
    if n == 0:
        return None
    return n + (0.5 if "½" in label else 0.0)


def rating_from_href(href: str) -> float | None:
    m = re.search(r"/rated/([^/]+)/", href or "")
    if not m:
        return None
    raw = unquote(m.group(1)).strip()
    if raw == "½":
        return 0.5
    m2 = re.fullmatch(r"(\d)(½)?", raw)
    if not m2:
        return None
    v = int(m2.group(1)) + (0.5 if m2.group(2) else 0.0)
    return v if 0.5 <= v <= 5 else None


# --------------------------------------------------------------------------------------
# HTML parsing (Letterboxd film page and rating histogram)
# --------------------------------------------------------------------------------------

def make_soup(html: str):
    from bs4 import BeautifulSoup

    try:
        return BeautifulSoup(html, "lxml")
    except Exception:  # lxml missing or parser error: fall back to the stdlib parser  # noqa: BLE001 - parsing/network must never crash the run
        return BeautifulSoup(html, "html.parser")


_BAR_RE = re.compile(
    r"^\s*(?P<n>no|\d[\d,]*(?:\.\d+)?)\s*(?P<suf>[kKmM])?\s+(?P<stars>(?:half-)?★+½?|½)\s*ratings?\b",
    re.IGNORECASE,
)
_AVG_TOOLTIP_RE = re.compile(
    r"average of\s+(?P<avg>\d(?:\.\d+)?)\s+based on\s+(?P<n>\d[\d,]*(?:\.\d+)?)\s*(?P<suf>[kKmM])?\s+ratings?",
    re.IGNORECASE,
)


def parse_histogram_soup(soup) -> dict | None:
    """Return {counts{'0.5':n..}, raw[], complete, approx, total, plain_mean, displayed_avg, displayed_count} or None.

    Counts are read from the tooltip of every li.rating-histogram-bar > a.  The star value comes from the tooltip
    text, else from the /rated/<x>/ href, else (only when there are exactly 10 bars) from the position.
    Counts printed with a K or M suffix are rounded; the result is flagged approx.
    """
    # 2026 markup: table.chart > tr.column > a.barcolumn (older pages: li.rating-histogram-bar > a.bar)
    bars = soup.select("li.rating-histogram-bar, .rating-histogram tr.column")
    if not bars:
        return None
    counts: dict[float, int] = {}
    raw: list[str] = []
    approx = False
    for idx, li in enumerate(bars):
        a = li.find("a")
        if a is None:
            a = li
        label = a.get("data-original-title") or a.get("title") or a.get_text(" ", strip=True) or ""
        label = re.sub(r"\s+", " ", label.replace("\xa0", " ")).strip()
        raw.append(label)
        m = _BAR_RE.match(label)
        value = None
        n = None
        if m:
            value = stars_to_value(m.group("stars"))
            if m.group("n").lower() == "no":
                n = 0
            else:
                try:
                    n, ap = parse_count(m.group("n"), m.group("suf"))
                    approx = approx or ap
                except ValueError:
                    n = None
        if value is None:
            value = rating_from_href(a.get("href", ""))
        if value is None and len(bars) == 10:
            value = STAR_STEPS[idx]
        if n is None:
            m2 = re.match(r"\s*(\d[\d,]*)", label)
            if m2:
                n = int(m2.group(1).replace(",", ""))
        if value is None or n is None:
            continue
        counts[value] = n
    if not counts:
        return None
    total = sum(counts.values())
    plain = sum(v * c for v, c in counts.items()) / total if total else None
    disp_avg = disp_n = None
    tip = soup.select_one(".average-rating .display-rating, a.display-rating")
    if tip is not None:
        t = re.sub(r"\s+", " ", (tip.get("data-original-title") or tip.get("title") or "").replace("\xa0", " "))
        m3 = _AVG_TOOLTIP_RE.search(t)
        if m3:
            disp_avg = float(m3.group("avg"))
            try:
                disp_n = parse_count(m3.group("n"), m3.group("suf"))[0]
            except ValueError:
                disp_n = None
    return {
        "counts": {f"{v:.1f}": counts[v] for v in sorted(counts)},
        "raw": raw,
        "complete": len(counts) == 10,
        "approx": approx,
        "total": total,
        "plain_mean": round(plain, 6) if plain is not None else None,
        "displayed_avg": disp_avg,
        "displayed_count": disp_n,
    }


def parse_histogram_html(html: str) -> dict | None:
    return parse_histogram_soup(make_soup(html))


def _ld_names(v: Any) -> list[str]:
    if v is None:
        return []
    if isinstance(v, (str, dict)):
        v = [v]
    out = []
    for x in v:
        name = x.get("name") if isinstance(x, dict) else x
        if isinstance(name, str) and name.strip():
            out.append(name.strip())
    return out


def _load_jsonld(soup) -> dict | None:
    for tag in soup.find_all("script", attrs={"type": "application/ld+json"}):
        text = tag.string if tag.string is not None else tag.get_text()
        if not text:
            continue
        text = re.sub(r"/\*\s*<!\[CDATA\[\s*\*/", "", text)
        text = re.sub(r"/\*\s*\]\]>\s*\*/", "", text).strip()
        data = None
        try:
            data = json.loads(text)
        except ValueError:
            i, j = text.find("{"), text.rfind("}")
            if i != -1 and j > i:
                try:
                    data = json.loads(text[i : j + 1])
                except ValueError:
                    data = None
        if isinstance(data, list):
            data = next((d for d in data if isinstance(d, dict)), None)
        if isinstance(data, dict):
            return data
    return None


def _num(v: Any, cast=float):
    try:
        return cast(str(v).replace(",", ""))
    except (TypeError, ValueError):
        return None


def parse_film_page(html: str, url: str = "") -> dict:
    """Extract everything the calibration needs from a Letterboxd film page.  Never raises on odd markup."""
    out: dict[str, Any] = {
        "is_film_page": False, "slug": None, "title": None, "year": None,
        "tmdb_type": None, "tmdb_id": None, "imdb_id": None,
        "lb_avg": None, "lb_rating_count": None, "lb_review_count": None,
        "countries": [], "directors": [], "hist_inline": None, "notes": [],
    }
    soup = make_soup(html)
    body = soup.body

    # TMDB id and type: body attributes first, then the anchor href (path regex, not a substring test:
    # the host name themoviedb.org itself contains "movie").
    tid = ttype = None
    if body is not None:
        tid = _num(body.get("data-tmdb-id"), int)
        ttype = (body.get("data-tmdb-type") or "").strip().lower() or None
    if tid is None:
        m = re.search(r'data-tmdb-id="(\d+)"', html)
        tid = int(m.group(1)) if m else None
    if ttype is None:
        m = re.search(r'data-tmdb-type="(movie|tv)"', html)
        ttype = m.group(1) if m else None
    a_tmdb = soup.select_one('a[data-track-action="TMDB"]')
    if a_tmdb is not None:
        m = re.search(r"themoviedb\.org/(movie|tv)/(\d+)", a_tmdb.get("href", ""))
        if m:
            if tid is None:
                tid, ttype = int(m.group(2)), m.group(1)
            elif int(m.group(2)) != tid or (ttype and m.group(1) != ttype):
                out["notes"].append("tmdb_body_anchor_mismatch")
            elif ttype is None:
                ttype = m.group(1)
    if ttype not in ("movie", "tv"):
        if ttype:
            out["notes"].append(f"odd_tmdb_type:{ttype}")
        ttype = None
    out["tmdb_id"], out["tmdb_type"] = tid, (ttype if tid is not None else None)

    a_imdb = soup.select_one('a[data-track-action="IMDb"]') or soup.select_one('a[href*="imdb.com/title/"]')
    if a_imdb is not None:
        m = re.search(r"/title/(tt\d+)", a_imdb.get("href", ""))
        out["imdb_id"] = m.group(1) if m else None

    ld = _load_jsonld(soup)
    if ld:
        out["title"] = re.sub(r"\s+", " ", ld["name"]).strip() if isinstance(ld.get("name"), str) else None
        agg = ld.get("aggregateRating")
        if isinstance(agg, dict):
            out["lb_avg"] = _num(agg.get("ratingValue"))
            out["lb_rating_count"] = _num(agg.get("ratingCount"), int)
            out["lb_review_count"] = _num(agg.get("reviewCount"), int)
        out["countries"] = _ld_names(ld.get("countryOfOrigin"))
        out["directors"] = _ld_names(ld.get("director"))
        ev = ld.get("releasedEvent")
        if isinstance(ev, dict):
            ev = [ev]
        if isinstance(ev, list) and ev and isinstance(ev[0], dict):
            out["year"] = year_of(ev[0].get("startDate"))
        out["slug"] = slug_from_url(ld.get("@id"))
    else:
        out["notes"].append("no_jsonld")

    if out["lb_avg"] is None:
        meta = soup.find("meta", attrs={"name": "twitter:data2"})
        if meta is not None:
            m = re.match(r"\s*(\d(?:\.\d+)?)\s+out of 5", meta.get("content", ""))
            if m:
                out["lb_avg"] = float(m.group(1))
                out["notes"].append("avg_from_twitter_meta")
    if out["title"] is None:
        h1 = soup.select_one("section.production-masthead h1, h1.headline-1")
        if h1 is not None:
            out["title"] = re.sub(r"\s+", " ", h1.get_text(" ", strip=True)).strip() or None
    if out["year"] is None:
        a = soup.select_one("span.releasedate a")
        if a is not None:
            out["year"] = year_of(a.get_text())

    og = soup.find("meta", attrs={"property": "og:url"})
    out["slug"] = slug_from_url(url) or slug_from_url(og.get("content") if og is not None else None) or out["slug"]

    out["hist_inline"] = parse_histogram_soup(soup)
    out["is_film_page"] = bool(
        ld or tid is not None or (body is not None and body.get("data-type") == "film")
        or soup.select_one("section.production-masthead")
    )
    return out


def looks_like_challenge(status: int, text: str, headers: dict) -> bool:
    """True for a Cloudflare interstitial.  A page that carries film markers is never a challenge, so a film
    called 'Access Denied' cannot be mistaken for one."""
    if str(headers.get("cf-mitigated", "")).lower() == "challenge":
        return True
    head = (text or "")[:30000].lower()
    if any(m in head for m in ("data-tmdb-id", "application/ld+json", "production-masthead", "rating-histogram-bar", "barcolumn")):
        return False
    if re.search(r"<title>\s*(just a moment|attention required|access denied)", head):
        return True
    return "challenges.cloudflare.com" in head or "cf-chl" in head


# --------------------------------------------------------------------------------------
# rate limiting, cache, HTTP plumbing
# --------------------------------------------------------------------------------------

class RateLimiter:
    """Minimum gap between request starts: (delay + U(0, jitter)) * penalty.  Penalty grows on 429/403."""

    def __init__(self, delay: float, jitter: float = 0.0, clock: Callable[[], float] = time.monotonic,
                 sleep: Callable[[float], None] = time.sleep, rng: random.Random | None = None):
        self.delay, self.jitter = delay, jitter
        self.clock, self.sleep = clock, sleep
        self.rng = rng or random.Random()
        self.penalty = 1.0
        self._last: float | None = None

    def wait(self) -> None:
        interval = (self.delay + self.rng.uniform(0, self.jitter)) * self.penalty
        if self._last is not None:
            remaining = self._last + interval - self.clock()
            if remaining > 0:
                self.sleep(remaining)
        self._last = self.clock()

    def slow_down(self) -> None:
        self.penalty = min(self.penalty * 1.5, 4.0)

    def speed_up(self) -> None:
        self.penalty = max(1.0, self.penalty * 0.95)


class Cache:
    """SQLite key/value cache.  Only successes and permanent negatives live in kv; transient failures go to fails."""

    def __init__(self, path: str | Path):
        self.db = sqlite3.connect(str(path))
        self.db.execute("PRAGMA journal_mode=WAL")
        self.db.execute("PRAGMA synchronous=NORMAL")
        self.db.execute("CREATE TABLE IF NOT EXISTS kv (key TEXT PRIMARY KEY, val TEXT NOT NULL, ts REAL)")
        self.db.execute(
            "CREATE TABLE IF NOT EXISTS fails (key TEXT PRIMARY KEY, stage TEXT, error TEXT, n INTEGER, ts REAL)"
        )
        self.db.commit()

    def get(self, key: str):
        row = self.db.execute("SELECT val FROM kv WHERE key=?", (key,)).fetchone()
        return MISSING if row is None else json.loads(row[0])

    def put(self, key: str, value: Any) -> None:
        self.db.execute(
            "INSERT OR REPLACE INTO kv (key, val, ts) VALUES (?,?,?)",
            (key, json.dumps(value, ensure_ascii=False), time.time()),
        )
        self.db.execute("DELETE FROM fails WHERE key=?", (key,))
        self.db.commit()

    def neg(self, key: str, reason: str) -> None:
        self.put(key, {"_neg": reason})

    def fail(self, key: str, stage: str, error: str) -> None:
        self.db.execute(
            "INSERT INTO fails (key, stage, error, n, ts) VALUES (?,?,?,1,?) "
            "ON CONFLICT(key) DO UPDATE SET stage=excluded.stage, error=excluded.error, n=n+1, ts=excluded.ts",
            (key, stage, error, time.time()),
        )
        self.db.commit()

    def fail_info(self, key: str) -> str | None:
        row = self.db.execute("SELECT error FROM fails WHERE key=?", (key,)).fetchone()
        return row[0] if row else None

    def open_failures(self) -> list[tuple[str, str, str, int]]:
        return self.db.execute(
            "SELECT key, stage, error, n FROM fails WHERE key NOT IN (SELECT key FROM kv) ORDER BY ts"
        ).fetchall()

    def count_prefix(self, prefix: str) -> int:
        return self.db.execute("SELECT COUNT(*) FROM kv WHERE key LIKE ?", (prefix + "%",)).fetchone()[0]

    def close(self) -> None:
        self.db.close()


def is_neg(v: Any) -> bool:
    return isinstance(v, dict) and "_neg" in v


@dataclass
class Resp:
    status: int
    text: str = ""
    url: str = ""
    headers: dict = field(default_factory=dict)
    error: str | None = None


@dataclass
class Result:
    kind: str  # ok | neg | fail
    value: Any = None
    reason: str = ""


def redact(msg: Any, secrets: list[str]) -> str:
    s = str(msg)
    for sec in secrets:
        if sec and len(sec) >= 8:  # a real key is 32+ chars; avoid mangling text for toy values
            s = s.replace(sec, "***")
    return re.sub(r"(api_key=)[^&\s'\"]+", r"\1***", s)


def do_get(session, url: str, secrets: list[str], **kw) -> Resp:
    """Call session.get and normalise: any exception becomes Resp(status=0, error=...)."""
    try:
        r = session.get(url, **kw)
    except Exception as e:  # network error, TLS error, timeout...  # noqa: BLE001 - parsing/network must never crash the run
        return Resp(0, error=redact(f"{type(e).__name__}: {e}", secrets)[:300])
    try:
        headers = {str(k).lower(): str(v) for k, v in dict(r.headers).items()}
    except Exception:  # noqa: BLE001 - parsing/network must never crash the run
        headers = {}
    try:
        text = r.text
    except Exception:  # noqa: BLE001 - parsing/network must never crash the run
        text = ""
    return Resp(int(r.status_code), text or "", str(getattr(r, "url", "") or url), headers)


def retry_after(resp: Resp) -> float | None:
    try:
        return float(resp.headers.get("retry-after", ""))
    except ValueError:
        return None


class LetterboxdClient:
    """Polite Letterboxd fetcher: pacing, retry with backoff on 403/429/503, circuit breaker."""

    def __init__(self, session, limiter: RateLimiter, *, retries: int = 4, timeout: float = 30,
                 sleep: Callable[[float], None] = time.sleep, rng: random.Random | None = None,
                 breaker_threshold: int = 8, breaker_trips: int = 3, cooldown: float = 120,
                 on_bad_page: Callable[[str, str, str], None] | None = None,
                 log: Callable[[str], None] = lambda m: None):
        self.session, self.limiter = session, limiter
        self.retries, self.timeout, self.sleep = retries, timeout, sleep
        self.rng = rng or random.Random()
        self.breaker_threshold, self.breaker_trips, self.cooldown = breaker_threshold, breaker_trips, cooldown
        self.on_bad_page, self.log = on_bad_page, log
        self.disabled = False
        self.consec_fail = 0
        self.success_streak = 0
        self.trips = 0
        self.stats = Counter()  # requests, retries, cooldowns, http_<status>

    # -- plumbing ---------------------------------------------------------------------
    def _classify(self, resp: Resp, allow_redirects: bool) -> str:
        s = resp.status
        if s == 0:
            return "retry"
        if s in (404, 410):
            return "notfound"
        if not allow_redirects and s in REDIRECT_STATUS:
            return "redirect"
        if s == 200:
            return "retry" if looks_like_challenge(s, resp.text, resp.headers) else "ok"
        if s in RETRY_STATUS:
            return "retry"
        return "fatal"

    def _backoff(self, attempt: int, resp: Resp) -> float:
        wait = min(120.0, 4.0 * 2 ** attempt) * self.rng.uniform(0.8, 1.3)
        ra = retry_after(resp)
        if ra is not None:
            wait = max(wait, min(ra, 120.0))
        return wait

    def request(self, url: str, *, allow_redirects: bool, headers: dict | None = None) -> tuple[Resp, str]:
        last: tuple[Resp, str] = (Resp(0, error="no attempt"), "retry")
        for attempt in range(self.retries + 1):
            self.limiter.wait()
            self.stats["requests"] += 1
            kw: dict[str, Any] = {"timeout": self.timeout, "allow_redirects": allow_redirects}
            if headers:
                kw["headers"] = headers
            resp = do_get(self.session, url, [], **kw)
            kind = self._classify(resp, allow_redirects)
            if kind in ("ok", "redirect", "notfound"):
                self.limiter.speed_up()
                self.consec_fail = 0
                self.success_streak += 1
                if self.success_streak >= 20:
                    self.trips = 0
                return resp, kind
            if kind == "fatal":
                self.stats[f"http_{resp.status}"] += 1
                return resp, kind
            last = (resp, kind)
            self.stats[f"http_{resp.status}"] += 1
            self.success_streak = 0
            if attempt < self.retries:
                self.stats["retries"] += 1
                if resp.status in (403, 429, 503):
                    self.limiter.slow_down()
                self.sleep(self._backoff(attempt, resp))
        self._register_failure()
        return last

    def _register_failure(self) -> None:
        self.consec_fail += 1
        if self.consec_fail < self.breaker_threshold:
            return
        self.trips += 1
        self.consec_fail = 0
        self.stats["cooldowns"] += 1
        if self.trips >= self.breaker_trips:
            self.disabled = True
            self.log("LETTERBOXD DISABLED for the rest of this run: repeated blocks. Own films fall back to TMDB "
                     "search; sample films cannot be resolved. Re-run later (the cache resumes).")
            return
        wait = self.cooldown * self.trips
        self.log(f"Letterboxd blocked {self.breaker_threshold} fetches in a row; cooling down {wait:.0f}s "
                 f"(trip {self.trips}/{self.breaker_trips})")
        self.sleep(wait)

    @staticmethod
    def _reason(resp: Resp, kind: str) -> str:
        if resp.status == 0:
            return f"network:{resp.error or 'error'}"[:120]
        if kind == "retry" and resp.status == 200:
            return "cloudflare_challenge"
        return f"http_{resp.status}"

    # -- public calls -----------------------------------------------------------------
    def resolve_boxd(self, lid: str) -> Result:
        if self.disabled:
            return Result("fail", reason="letterboxd_disabled")
        url = f"{BOXD_HOST}/{lid}"
        resp, kind = self.request(url, allow_redirects=False)
        if kind == "redirect":
            loc = resp.headers.get("location", "")
            slug = slug_from_url(urljoin(url, loc))
            if slug:
                return Result("ok", slug)
            return Result("neg", reason=f"redirect_not_a_film:{loc[:80]}")
        if kind == "ok":  # the client followed the redirect on its own
            slug = slug_from_url(resp.url)
            return Result("ok", slug) if slug else Result("neg", reason=f"no_redirect:{resp.url[:80]}")
        if kind == "notfound":
            return Result("neg", reason=f"boxd_http_{resp.status}")
        return Result("fail", reason=self._reason(resp, kind))

    def fetch_page(self, slug: str) -> Result:
        if self.disabled:
            return Result("fail", reason="letterboxd_disabled")
        resp, kind = self.request(f"{LB_HOST}/film/{quote(slug, safe='-_.~')}/", allow_redirects=True)
        if kind == "notfound":
            return Result("neg", reason=f"film_http_{resp.status}")
        if kind != "ok":
            return Result("fail", reason=self._reason(resp, kind))
        parsed = parse_film_page(resp.text, resp.url)
        parsed["requested_slug"] = slug
        if not parsed["is_film_page"]:
            if self.on_bad_page:
                self.on_bad_page("page", slug, resp.text)
            return Result("fail", reason="not_a_film_page")
        if parsed["tmdb_id"] is None and parsed["lb_avg"] is None and self.on_bad_page:
            self.on_bad_page("page_no_ids", slug, resp.text)
        parsed["slug"] = parsed["slug"] or slug
        return Result("ok", parsed)

    def fetch_histogram(self, slug: str) -> Result:
        if self.disabled:
            return Result("fail", reason="letterboxd_disabled")
        q = quote(slug, safe="-_.~")
        hdrs = {"Referer": f"{LB_HOST}/film/{q}/", "X-Requested-With": "XMLHttpRequest"}
        resp, kind = self.request(f"{LB_HOST}/csi/film/{q}/rating-histogram/", allow_redirects=True, headers=hdrs)
        if kind == "notfound":
            return Result("neg", reason=f"hist_http_{resp.status}")
        if kind != "ok":
            return Result("fail", reason=self._reason(resp, kind))
        hist = parse_histogram_html(resp.text)
        if hist is None:
            if self.on_bad_page:
                self.on_bad_page("hist", slug, resp.text)
            return Result("neg", reason="no_histogram_bars")
        return Result("ok", hist)


class TmdbClient:
    def __init__(self, key: str, session, limiter: RateLimiter, *, retries: int = 5, timeout: float = 20,
                 sleep: Callable[[float], None] = time.sleep):
        self.key, self.session, self.limiter = key, session, limiter
        self.retries, self.timeout, self.sleep = retries, timeout, sleep
        self.bearer = key.startswith("eyJ")  # v4 read-access token (a JWT)
        self.stats = Counter()

    def get(self, path: str, params: dict | None = None) -> Result:
        params = dict(params or {})
        headers = {"Accept": "application/json"}
        if self.bearer:
            headers["Authorization"] = f"Bearer {self.key}"
        else:
            params["api_key"] = self.key
        last_reason = "unknown"
        for attempt in range(self.retries + 1):
            self.limiter.wait()
            self.stats["requests"] += 1
            resp = do_get(self.session, TMDB_BASE + path, [self.key], params=params, headers=headers,
                          timeout=self.timeout)
            s = resp.status
            if s == 200:
                try:
                    return Result("ok", json.loads(resp.text))
                except ValueError:
                    last_reason = "bad_json"
            elif s == 404:
                return Result("neg", reason="tmdb_404")
            elif s == 401:
                raise TmdbAuthError("TMDB rejected the API key (HTTP 401). Check TMDB_API_KEY.")
            elif s == 429 or s == 0 or s >= 500:
                self.stats[f"http_{s}"] += 1
                last_reason = f"http_{s}" if s else f"network:{resp.error or 'error'}"[:120]
                if s == 429:
                    self.limiter.slow_down()
            else:
                self.stats[f"http_{s}"] += 1
                return Result("fail", reason=f"http_{s}")
            if attempt < self.retries:
                self.stats["retries"] += 1
                wait = retry_after(resp) if s == 429 else None
                self.sleep(min(60.0, wait if wait is not None else 1.5 * 2 ** attempt))
        return Result("fail", reason=last_reason)


# --------------------------------------------------------------------------------------
# TMDB extraction and title+year matching
# --------------------------------------------------------------------------------------

def _dedupe_people(items: list[dict], with_job: bool) -> list[dict]:
    seen, out = set(), []
    for c in items:
        key = (c.get("id"), c.get("job") if with_job else None)
        if key in seen:
            continue
        seen.add(key)
        rec = {"id": c.get("id"), "name": c.get("name")}
        if with_job:
            rec["job"] = c.get("job")
        out.append(rec)
    return out


def extract_movie_record(js: dict) -> dict:
    crew = (js.get("credits") or {}).get("crew") or []
    directors = _dedupe_people([c for c in crew if c.get("job") == "Director"], False)
    composers = _dedupe_people([c for c in crew if c.get("job") in COMPOSER_JOBS], True)
    theatrical, premiere, anyd = [], [], []
    for entry in (js.get("release_dates") or {}).get("results") or []:
        for rd in entry.get("release_dates") or []:
            d = (rd.get("release_date") or "")[:10]
            if not re.fullmatch(r"\d{4}-\d{2}-\d{2}", d):
                continue
            anyd.append(d)
            if rd.get("type") in (2, 3):
                theatrical.append(d)
            elif rd.get("type") == 1:
                premiere.append(d)
    countries = js.get("production_countries") or []
    return {
        "tmdb_id": js.get("id"),
        "imdb_id": js.get("imdb_id") or None,
        "title": js.get("title"),
        "original_title": js.get("original_title"),
        "release_date": js.get("release_date") or None,
        "runtime": js.get("runtime") or None,
        "original_language": js.get("original_language") or None,
        "production_countries": [c.get("iso_3166_1") for c in countries if c.get("iso_3166_1")],
        "production_country_names": [c.get("name") for c in countries if c.get("name")],
        "spoken_languages": [x.get("iso_639_1") for x in js.get("spoken_languages") or [] if x.get("iso_639_1")],
        "genres": [g.get("name") for g in js.get("genres") or [] if g.get("name")],
        "directors": directors,
        "composers": composers,
        "earliest_theatrical_date": min(theatrical) if theatrical else None,
        "earliest_premiere_date": min(premiere) if premiere else None,
        "earliest_release_any": min(anyd) if anyd else None,
        "vote_average": js.get("vote_average"),
        "vote_count": js.get("vote_count"),
        "status": js.get("status"),
        "adult": js.get("adult"),
    }


def pick_candidate(results: list[dict], title: str, year: int | None, kind: str = "movie") -> dict | None:
    """Exact normalised title (title or original title) with release year within +-1 of the export year."""
    tkeys = ("title", "original_title") if kind == "movie" else ("name", "original_name")
    dkey = "release_date" if kind == "movie" else "first_air_date"
    nq = norm_title(title)
    if not nq:
        return None
    cands = []
    for r in results:
        if nq not in {norm_title(r.get(k)) for k in tkeys}:
            continue
        ry = year_of(r.get(dkey))
        if year is not None:
            if ry is None or abs(ry - year) > 1:
                continue
            dy = abs(ry - year)
        else:
            dy = 0
        cands.append((dy, -(r.get("vote_count") or 0), r))
    if not cands:
        return None
    cands.sort(key=lambda t: (t[0], t[1]))
    best = cands[0][2]
    tied = [c for c in cands if c[0] == cands[0][0] and c[2].get("id") != best.get("id")]
    return {"hit": best, "n_candidates": len(cands), "ambiguous": bool(tied)}


def fuzzy_candidate(results: list[dict], title: str, year: int | None) -> dict | None:
    nq = norm_title(title)
    hits = []
    for r in results:
        ry = year_of(r.get("release_date"))
        if year is not None and (ry is None or abs(ry - year) > 1):
            continue
        ratio = max(difflib.SequenceMatcher(None, nq, norm_title(r.get(k))).ratio()
                    for k in ("title", "original_title"))
        if ratio >= 0.92:
            hits.append(r)
    return hits[0] if len(hits) == 1 else None


# --------------------------------------------------------------------------------------
# input loading
# --------------------------------------------------------------------------------------

def load_watched(export_dir: str | Path) -> tuple[list[dict], dict]:
    """Read watched.csv only (Name, Year, Letterboxd URI).  profile.csv is deliberately never opened."""
    base = Path(export_dir).expanduser()
    f = base / "watched.csv"
    if not f.is_file():
        found = sorted(base.glob("*/watched.csv")) if base.is_dir() else []
        if len(found) == 1:
            f = found[0]
        else:
            raise UsageError(f"watched.csv not found in {base} (give the folder that contains watched.csv)")
    rows, seen = [], set()
    info = {"rows_in_file": 0, "duplicates_dropped": 0, "without_uri": 0}
    with open(f, encoding="utf-8-sig", newline="") as fh:
        reader = csv.DictReader(fh)
        cols = {(h or "").strip().lower(): h for h in (reader.fieldnames or [])}
        if "name" not in cols:
            raise UsageError("watched.csv has no 'Name' column")
        for r in reader:
            info["rows_in_file"] += 1
            name = (r.get(cols["name"]) or "").strip()
            ym = re.search(r"\d{4}", r.get(cols.get("year", ""), "") or "")
            year = int(ym.group(0)) if ym else None
            uri = (r.get(cols.get("letterboxd uri", ""), "") or "").strip()
            lid = lid_from_uri(uri)
            if not name and not lid:
                continue
            key = lid or (name.lower(), year)
            if key in seen:
                info["duplicates_dropped"] += 1
                continue
            seen.add(key)
            if not lid:
                info["without_uri"] += 1
            rows.append({"name": name, "year": year, "lid": lid})
    return rows, info


SLUG_HEADERS = ("slug", "movie_id", "letterboxd_slug", "film_slug", "letterboxd_id", "id")
_SLUG_RE = re.compile(r"^[^\s/?#,\"']+$")


def clean_slug(cell: str) -> str | None:
    cell = (cell or "").strip()
    if not cell or cell.startswith("#"):
        return None
    m = re.search(r"/film/([^/?#\s]+)", cell)
    if m:
        cell = unquote(m.group(1))
    return cell if _SLUG_RE.match(cell) else None


def load_slugs(path: str | Path) -> list[str]:
    """CSV with a slug/movie_id column, or a headerless one-column file, or one slug per line.  Order-preserving, deduped."""
    p = Path(path).expanduser()
    if not p.is_file():
        raise UsageError(f"sample slug file not found: {p}")
    with open(p, encoding="utf-8-sig", newline="") as fh:
        rows = [r for r in csv.reader(fh) if r and any(c.strip() for c in r)]
    if not rows:
        return []
    first = [c.strip().lower() for c in rows[0]]
    col, start = 0, 0
    for h in SLUG_HEADERS:
        if h in first:
            col, start = first.index(h), 1
            break
    else:
        if len(first) > 1 or not (clean_slug(rows[0][0]) or "").replace("-", "").replace("_", "").isalnum():
            raise UsageError(f"cannot find a slug column in {p.name}; expected a header named one of {SLUG_HEADERS}")
    seen, out = set(), []
    for r in rows[start:]:
        s = clean_slug(r[col]) if col < len(r) else None
        if s and s not in seen:
            seen.add(s)
            out.append(s)
    return out


# --------------------------------------------------------------------------------------
# progress
# --------------------------------------------------------------------------------------

class Progress:
    def __init__(self, label: str, total: int, stream=None, clock=time.monotonic):
        self.label, self.total, self.clock = label, total, clock
        self.stream = stream or sys.stderr
        self.t0 = clock()
        self.last = 0.0
        self.tty = hasattr(self.stream, "isatty") and self.stream.isatty()

    def update(self, done: int, extra: str = "", force: bool = False) -> None:
        t = self.clock()
        if not force and t - self.last < (1.0 if self.tty else 30.0):
            return
        self.last = t
        el = t - self.t0
        eta = (el / done * (self.total - done)) if done else 0
        line = (f"[{self.label}] {done}/{self.total} ({100 * done / max(self.total, 1):.1f}%) "
                f"elapsed {el / 60:.1f}m eta {eta / 60:.1f}m {extra}")
        print(("\r" + line + "   ") if self.tty else line, file=self.stream, end="" if self.tty else "\n", flush=True)

    def done(self) -> None:
        if self.tty:
            print(file=self.stream)


# --------------------------------------------------------------------------------------
# the collector
# --------------------------------------------------------------------------------------

MOVIE_FIELDS = ("title", "original_title", "release_date", "runtime", "original_language", "production_countries",
                "production_country_names", "spoken_languages", "genres", "directors", "composers",
                "earliest_theatrical_date", "earliest_premiere_date", "earliest_release_any",
                "vote_average", "vote_count", "status", "adult")


class Collector:
    def __init__(self, cache: Cache, lb: LetterboxdClient | None, tmdb: TmdbClient | None, *,
                 hist_source: str = "auto", validate_search: bool = True,
                 log: Callable[[str], None] = lambda m: None):
        self.cache, self.lb, self.tmdb = cache, lb, tmdb
        self.hist_source, self.validate_search, self.log = hist_source, validate_search, log

    # -- cached fetch steps ----------------------------------------------------------
    def _lb_step(self, key: str, stage: str, fn: Callable[[], Result], online: bool) -> tuple[Any, str]:
        v = self.cache.get(key)
        if v is not MISSING:
            return (None, "neg:" + v["_neg"]) if is_neg(v) else (v, "ok")
        if not online:
            f = self.cache.fail_info(key)
            return None, ("fail:" + f if f else "not_fetched")
        if self.lb is None:
            return None, "skipped:letterboxd_off"
        if self.lb.disabled:
            return None, "skipped:letterboxd_disabled"
        res = fn()
        if res.kind == "ok":
            self.cache.put(key, res.value)
            return res.value, "ok"
        if res.kind == "neg":
            self.cache.neg(key, res.reason)
            return None, "neg:" + res.reason
        self.cache.fail(key, stage, res.reason)
        return None, "fail:" + res.reason

    def ensure_slug(self, lid: str, online: bool) -> tuple[str | None, str]:
        def fetch() -> Result:
            r = self.lb.resolve_boxd(lid)
            return Result("ok", {"slug": r.value}) if r.kind == "ok" else r

        v, st = self._lb_step(f"lbredir:{lid}", "boxd_redirect", fetch, online)
        return (v["slug"] if v else None), st

    def ensure_page(self, slug: str, online: bool) -> tuple[dict | None, str]:
        return self._lb_step(f"lbpage:{slug}", "film_page", lambda: self.lb.fetch_page(slug), online)

    def ensure_hist(self, page: dict, online: bool) -> tuple[dict | None, str, str | None]:
        """Histogram for a parsed page: inline when complete (hist_source auto), else the csi endpoint."""
        inline = page.get("hist_inline")
        if inline and inline.get("complete") and self.hist_source == "auto":
            return inline, "ok", "inline"
        slug = page.get("slug") or page.get("requested_slug")
        v, st = self._lb_step(f"lbhist:{slug}", "histogram", lambda: self.lb.fetch_histogram(slug), online)
        if v:
            return v, "ok", "csi"
        if inline:
            return inline, "ok", ("inline" if inline.get("complete") else "inline_partial")
        return None, st, None

    def ensure_tmdb_movie(self, tmdb_id: int, online: bool) -> tuple[dict | None, str]:
        key = f"tmdbmovie:{tmdb_id}"
        v = self.cache.get(key)
        if v is not MISSING:
            return (None, "neg:" + v["_neg"]) if is_neg(v) else (v, "ok")
        if not online or self.tmdb is None:
            f = self.cache.fail_info(key)
            return None, ("fail:" + f if f else "not_fetched")
        res = self.tmdb.get(f"/movie/{tmdb_id}", {"append_to_response": "credits,release_dates", "language": "en-US"})
        if res.kind == "ok":
            rec = extract_movie_record(res.value)
            self.cache.put(key, rec)
            return rec, "ok"
        if res.kind == "neg":
            self.cache.neg(key, res.reason)
            return None, "neg:" + res.reason
        self.cache.fail(key, "tmdb_movie", res.reason)
        return None, "fail:" + res.reason

    def ensure_match(self, title: str, year: int | None, online: bool) -> tuple[dict | None, str]:
        """TMDB title+year search.  Cached decision: {'found': bool, ...}."""
        key = f"match:{title}|{year}"
        v = self.cache.get(key)
        if v is not MISSING:
            return v, "ok"
        if not online or self.tmdb is None:
            f = self.cache.fail_info(key)
            return None, ("fail:" + f if f else "not_fetched")
        if not norm_title(title):
            return {"found": False, "reason": "empty_title"}, "ok"
        decision = self._search(title, year)
        if decision is None:
            return None, "fail:" + (self.cache.fail_info(key) or "search_failed")
        self.cache.put(key, decision)
        return decision, "ok"

    def _search(self, title: str, year: int | None) -> dict | None:
        key = f"match:{title}|{year}"
        stages: list[tuple[str | None, int | None]]
        stages = ([("primary_release_year", year), ("year", year), ("primary_release_year", year - 1),
                   ("primary_release_year", year + 1)] if year else [(None, None)])
        seen: dict[Any, dict] = {}
        for pname, pval in stages:
            params = {"query": title, "include_adult": "false", "language": "en-US"}
            if pname:
                params[pname] = str(pval)
            res = self.tmdb.get("/search/movie", params)
            if res.kind != "ok":
                self.cache.fail(key, "tmdb_search", res.reason)
                return None
            results = res.value.get("results") or []
            for r in results:
                seen.setdefault(r.get("id"), r)
            cand = pick_candidate(results, title, year)
            if cand:
                h = cand["hit"]
                return {"found": True, "tmdb_type": "movie", "tmdb_id": h["id"],
                        "stage": f"{pname}={pval}" if pname else "no_year", "title": h.get("title"),
                        "release_date": h.get("release_date"), "n_candidates": cand["n_candidates"],
                        "ambiguous": cand["ambiguous"], "fuzzy": False, "vote_count": h.get("vote_count")}
        fz = fuzzy_candidate(list(seen.values()), title, year)
        if fz:
            return {"found": True, "tmdb_type": "movie", "tmdb_id": fz["id"], "stage": "fuzzy",
                    "title": fz.get("title"), "release_date": fz.get("release_date"), "n_candidates": 1,
                    "ambiguous": False, "fuzzy": True, "vote_count": fz.get("vote_count")}
        # no movie matched: is it a TV show?
        params = {"query": title, "include_adult": "false", "language": "en-US"}
        if year:
            params["first_air_date_year"] = str(year)
        res = self.tmdb.get("/search/tv", params)
        if res.kind != "ok":
            self.cache.fail(key, "tmdb_search", res.reason)
            return None
        cand = pick_candidate(res.value.get("results") or [], title, year, kind="tv")
        if cand:
            h = cand["hit"]
            return {"found": True, "tmdb_type": "tv", "tmdb_id": h["id"], "stage": "tv_search", "title": h.get("name"),
                    "release_date": h.get("first_air_date"), "n_candidates": cand["n_candidates"],
                    "ambiguous": cand["ambiguous"], "fuzzy": False, "vote_count": h.get("vote_count")}
        return {"found": False, "reason": "no_match"}

    # -- per-film pipelines (online=True fetches, online=False only reads the cache) -----------
    def _movie_fields(self, rec: dict | None) -> dict:
        return {k: (rec.get(k) if rec else None) for k in MOVIE_FIELDS}

    def _identity_from_page(self, page: dict | None) -> tuple[str | None, int | None]:
        if page and page.get("tmdb_id") is not None and page.get("tmdb_type") in ("movie", "tv"):
            return page["tmdb_type"], page["tmdb_id"]
        return None, None

    def process_own(self, row: dict, online: bool) -> tuple[dict, dict | None]:
        name, year, lid = row["name"], row["year"], row.get("lid")
        rec: dict[str, Any] = {"group": "own", "lid": lid, "name": name, "year": year,
                               "lid_uid": None, "lid_type_digit": None, "hist_wanted": None}
        if lid:
            try:
                uid, td = decode_lid(lid)
                rec.update(lid_uid=uid, lid_type_digit=td)
            except ValueError:
                rec.update(lid_uid=None, lid_type_digit=None)
        status: dict[str, str] = {}
        slug = page = None
        if lid:
            slug, status["redirect"] = self.ensure_slug(lid, online)
            if slug:
                page, status["page"] = self.ensure_page(slug, online)
        else:
            status["redirect"] = "skipped:no_uri"
        rec["lb_slug"] = (page or {}).get("slug") or slug
        rec["lb_url"] = f"{LB_HOST}/film/{rec['lb_slug']}/" if rec["lb_slug"] else None
        ttype, tid = self._identity_from_page(page)
        path = None
        match = None
        if tid is not None:
            path = "lb_page"
        if page is not None and tid is None:
            status["page_ids"] = "no_tmdb_link"
        if path is None or (self.validate_search and ttype == "movie"):
            match, status["search"] = self.ensure_match(name, year, online)
        if path is None and match and match.get("found"):
            path, ttype, tid = "tmdb_search", match["tmdb_type"], match["tmdb_id"]
        rec["resolve_path"] = path or "unresolved"
        rec["tmdb_type"], rec["tmdb_id"] = ttype, tid
        rec["match"] = match if path != "lb_page" else None
        if path == "lb_page" and self.validate_search and ttype == "movie":
            rec["search_check"] = {
                "found": bool(match and match.get("found")),
                "tmdb_type": match.get("tmdb_type") if match else None,
                "tmdb_id": match.get("tmdb_id") if match else None,
                "stage": match.get("stage") if match else None,
                "agrees": (bool(match.get("found") and match.get("tmdb_type") == ttype and match.get("tmdb_id") == tid)
                           if match else None),
            }
        else:
            rec["search_check"] = None
        # Letterboxd page fields
        rec["imdb_id"] = (page or {}).get("imdb_id")
        rec["lb_avg"] = (page or {}).get("lb_avg")
        rec["lb_rating_count"] = (page or {}).get("lb_rating_count")
        rec["lb_review_count"] = (page or {}).get("lb_review_count")
        rec["lb_countries"] = (page or {}).get("countries") or []
        rec["lb_directors"] = (page or {}).get("directors") or []
        # TMDB movie details
        mrec = None
        if ttype == "movie" and tid is not None:
            mrec, status["tmdb"] = self.ensure_tmdb_movie(tid, online)
        elif ttype == "tv":
            status["tmdb"] = "skipped:tv"
        rec.update(self._movie_fields(mrec))
        rec["imdb_id"] = rec["imdb_id"] or (mrec or {}).get("imdb_id")
        # histogram
        hrec = None
        if page is not None and ttype != "tv":
            hist, hst, src = self.ensure_hist(page, online)
            status["hist"] = hst
            if hist:
                hrec = self._hist_record("own", page, hist, src, rec)
        rec["status"] = status
        return rec, hrec

    def process_sample(self, slug: str, want_hist: bool, online: bool) -> tuple[dict, dict | None]:
        rec: dict[str, Any] = {"group": "sample", "lid": None, "name": None, "year": None,
                               "lid_uid": None, "lid_type_digit": None, "lb_slug": slug,
                               "lb_url": f"{LB_HOST}/film/{slug}/", "hist_wanted": want_hist}
        status: dict[str, str] = {}
        page, status["page"] = self.ensure_page(slug, online)
        if page:
            rec["name"], rec["year"] = page.get("title"), page.get("year")
            rec["lb_slug"] = page.get("slug") or slug
        ttype, tid = self._identity_from_page(page)
        path = "lb_page" if tid is not None else None
        match = None
        if page and tid is None and page.get("title"):
            status["page_ids"] = "no_tmdb_link"
            match, status["search"] = self.ensure_match(page["title"], page.get("year"), online)
            if match and match.get("found"):
                path, ttype, tid = "tmdb_search", match["tmdb_type"], match["tmdb_id"]
        rec["resolve_path"] = path or "unresolved"
        rec["tmdb_type"], rec["tmdb_id"] = ttype, tid
        rec["match"] = match if path != "lb_page" else None
        rec["search_check"] = None
        rec["imdb_id"] = (page or {}).get("imdb_id")
        rec["lb_avg"] = (page or {}).get("lb_avg")
        rec["lb_rating_count"] = (page or {}).get("lb_rating_count")
        rec["lb_review_count"] = (page or {}).get("lb_review_count")
        rec["lb_countries"] = (page or {}).get("countries") or []
        rec["lb_directors"] = (page or {}).get("directors") or []
        mrec = None
        if ttype == "movie" and tid is not None:
            mrec, status["tmdb"] = self.ensure_tmdb_movie(tid, online)
        elif ttype == "tv":
            status["tmdb"] = "skipped:tv"
        rec.update(self._movie_fields(mrec))
        rec["imdb_id"] = rec["imdb_id"] or (mrec or {}).get("imdb_id")
        hrec = None
        if want_hist and page is not None and ttype != "tv":
            hist, hst, src = self.ensure_hist(page, online)
            status["hist"] = hst
            if hist:
                hrec = self._hist_record("sample", page, hist, src, rec)
        rec["status"] = status
        return rec, hrec

    @staticmethod
    def _hist_record(group: str, page: dict, hist: dict, source: str | None, rec: dict) -> dict:
        return {
            "group": group, "slug": page.get("slug") or page.get("requested_slug"), "lid": rec.get("lid"),
            "name": rec.get("name"), "year": rec.get("year"), "tmdb_id": rec.get("tmdb_id"),
            "counts": hist["counts"], "total": hist["total"], "plain_mean": hist["plain_mean"],
            "displayed_avg_jsonld": page.get("lb_avg"), "displayed_avg_tooltip": hist.get("displayed_avg"),
            "jsonld_rating_count": page.get("lb_rating_count"), "tooltip_count": hist.get("displayed_count"),
            "complete": hist["complete"], "approx": hist["approx"], "source": source, "raw": hist["raw"],
        }

    # -- preflight -------------------------------------------------------------------
    def preflight(self) -> tuple[list[str], bool]:
        """Returns (messages, fatal).  Fetches one known film from each service before spending an hour."""
        msgs, fatal = [], False
        if self.lb is not None:
            page, st = self.ensure_page(CANARY_SLUG, True)
            if page is None:
                msgs.append(f"FAIL Letterboxd film page ({CANARY_SLUG}): {st}")
                fatal = True
            elif page.get("tmdb_id") is None:
                msgs.append("FAIL Letterboxd page fetched but no TMDB id found: selectors may have changed "
                            "(run with --debug-html 3 and share the saved page)")
                fatal = True
            else:
                bits = f"TMDB {page['tmdb_type']}/{page['tmdb_id']}, avg {page['lb_avg']}, count {page['lb_rating_count']}"
                if page["tmdb_id"] != CANARY_TMDB_ID:
                    bits += f" (expected {CANARY_TMDB_ID}; warning only)"
                msgs.append(f"ok   Letterboxd film page: {bits}; inline histogram: "
                            f"{'yes' if (page.get('hist_inline') or {}).get('complete') else 'no'}")
                v, st2 = self._lb_step(f"lbhist:{CANARY_SLUG}", "histogram",
                                       lambda: self.lb.fetch_histogram(CANARY_SLUG), True)
                if v:
                    msgs.append(f"ok   csi histogram: total {v['total']}, plain mean {v['plain_mean']}, "
                                f"complete={v['complete']}, approx={v['approx']}")
                else:
                    msgs.append(f"WARN csi histogram endpoint failed ({st2}); will use inline histograms where present")
        if self.tmdb is not None:
            try:
                res = self.tmdb.get(f"/movie/{CANARY_TMDB_ID}", {"append_to_response": "credits,release_dates"})
            except TmdbAuthError as e:
                res = Result("fail", reason=str(e))
            if res.kind == "ok":
                r = extract_movie_record(res.value)
                msgs.append(f"ok   TMDB movie/{CANARY_TMDB_ID}: {r['title']}, countries {r['production_countries']}, "
                            f"directors {[d['name'] for d in r['directors']]}")
            else:
                msgs.append(f"FAIL TMDB movie/{CANARY_TMDB_ID}: {res.kind} {res.reason}")
                fatal = True
        return msgs, fatal


# --------------------------------------------------------------------------------------
# outputs
# --------------------------------------------------------------------------------------

def write_jsonl(path: Path, records: list[dict]) -> None:
    tmp = path.with_suffix(path.suffix + ".tmp")
    with open(tmp, "w", encoding="utf-8") as fh:
        fh.writelines(json.dumps(r, ensure_ascii=False) + "\n" for r in records)
    os.replace(tmp, path)


def write_json(path: Path, obj: Any) -> None:
    tmp = path.with_suffix(path.suffix + ".tmp")
    with open(tmp, "w", encoding="utf-8") as fh:
        json.dump(obj, fh, ensure_ascii=False, indent=2)
        fh.write("\n")
    os.replace(tmp, path)


def _stat_status(records: list[dict], key: str) -> dict:
    return dict(sorted(Counter(r["status"][key] for r in records if key in r["status"]).items()))


def summarize_group(records: list[dict], hists: list[dict]) -> dict:
    n = len(records)
    movies = [r for r in records if r["tmdb_type"] == "movie"]
    with_details = [r for r in movies if r["status"].get("tmdb") == "ok"]
    out: dict[str, Any] = {
        "films": n,
        "resolve_path": dict(Counter(r["resolve_path"] for r in records)),
        "tmdb_type": dict(Counter(r["tmdb_type"] or "none" for r in records)),
        "movie_details_ok": len(with_details),
        "movie_details_missing": len(movies) - len(with_details),
        "with_production_countries": sum(1 for r in with_details if r["production_countries"]),
        "with_directors": sum(1 for r in with_details if r["directors"]),
        "with_composers": sum(1 for r in with_details if r["composers"]),
        "with_runtime": sum(1 for r in with_details if r["runtime"]),
        "runtime_le_40_min": sum(1 for r in with_details if r["runtime"] and r["runtime"] <= 40),
        "lb_page_status": _stat_status(records, "page"),
        "lb_redirect_status": _stat_status(records, "redirect"),
        "tmdb_status": _stat_status(records, "tmdb"),
        "histograms_ok": len(hists),
        "histograms_approx": sum(1 for h in hists if h["approx"]),
        "histograms_incomplete": sum(1 for h in hists if not h["complete"]),
        "histogram_sources": dict(Counter(h["source"] for h in hists)),
    }
    pairs = [(h["plain_mean"], h["displayed_avg_jsonld"]) for h in hists
             if not h["approx"] and h["complete"] and h["plain_mean"] is not None
             and h["displayed_avg_jsonld"] is not None]
    if pairs:
        out["plain_vs_displayed_rmse"] = round(math.sqrt(sum((a - b) ** 2 for a, b in pairs) / len(pairs)), 4)
        out["plain_vs_displayed_n"] = len(pairs)
        out["plain_minus_displayed_mean"] = round(sum(a - b for a, b in pairs) / len(pairs), 4)
    checks = [r["search_check"] for r in records if r.get("search_check")]
    if checks:
        agree = sum(1 for c in checks if c["agrees"])
        out["search_validation"] = {
            "compared": len(checks), "agree": agree, "disagree": len(checks) - agree,
            "search_found_nothing": sum(1 for c in checks if not c["found"]),
            "precision_when_found": round(agree / max(1, sum(1 for c in checks if c["found"])), 4),
            "agreement_overall": round(agree / len(checks), 4),
        }
        out["search_disagreements"] = [
            {"name": r["name"], "year": r["year"], "lb_tmdb_id": r["tmdb_id"], "search": r["search_check"]}
            for r in records if r.get("search_check") and not r["search_check"]["agrees"]
        ][:40]
    return out


def build_summary(own: list[dict], own_h: list[dict], smp: list[dict], smp_h: list[dict], *, cache: Cache,
                  lb: LetterboxdClient | None, tmdb: TmdbClient | None, meta: dict) -> dict:
    summary: dict[str, Any] = dict(meta)
    summary["own"] = summarize_group(own, own_h) if own else {}
    if smp:
        s = summarize_group(smp, smp_h)
        wanted = sum(1 for r in smp if r.get("hist_wanted"))
        s["histograms_target"] = wanted
        s["histograms_missing_of_target"] = wanted - len(smp_h)
        summary["sample"] = s
    else:
        summary["sample"] = {}
    fails = cache.open_failures()
    reasons = Counter(f"{stage}:{err}" for _k, stage, err, _n in fails)
    summary["open_failures"] = {
        "count": len(fails), "by_stage_reason": dict(reasons.most_common()),
        "examples": [{"key": k, "stage": st, "error": e, "attempts": n} for k, st, e, n in fails[:50]],
    }
    summary["network_this_run"] = {
        "letterboxd": ({**dict(lb.stats), "disabled": lb.disabled, "final_rate_penalty": round(lb.limiter.penalty, 2)}
                       if lb else "off"),
        "tmdb": dict(tmdb.stats) if tmdb else "off",
    }
    warnings = []
    if lb is not None and lb.disabled:
        warnings.append("Letterboxd was disabled mid-run by the circuit breaker; re-run to fill gaps.")
    if own:
        unresolved = summary["own"]["resolve_path"].get("unresolved", 0)
        if unresolved:
            warnings.append(f"{unresolved} own films unresolved")
    if fails:
        warnings.append(f"{len(fails)} items still failing (re-run retries them)")
    summary["warnings"] = warnings
    return summary


# --------------------------------------------------------------------------------------
# CLI
# --------------------------------------------------------------------------------------

def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description="Collect TMDB and Letterboxd calibration data (resumable).",
                                formatter_class=argparse.ArgumentDefaultsHelpFormatter)
    p.add_argument("--export", required=True, help="Letterboxd export folder (contains watched.csv)")
    p.add_argument("--out", default="collect_out", help="output + cache folder")
    p.add_argument("--sample-slugs", help="CSV/TXT of Letterboxd slugs for the calibration sample")
    p.add_argument("--sample-limit", type=int, default=None, help="use only this many (random) sample films")
    p.add_argument("--hist-n", type=int, default=1000, help="sample films that also get a histogram")
    p.add_argument("--seed", type=int, default=42, help="seed for the random sample order")
    p.add_argument("--lb-delay", type=float, default=1.0, help="seconds between Letterboxd requests")
    p.add_argument("--lb-jitter", type=float, default=0.5, help="extra random 0..J seconds per request")
    p.add_argument("--tmdb-delay", type=float, default=0.05, help="seconds between TMDB requests")
    p.add_argument("--retries", type=int, default=4, help="retries after the first Letterboxd attempt")
    p.add_argument("--hist-source", choices=("auto", "csi"), default="auto",
                   help="auto: use the histogram inside the film page when complete; csi: always fetch the csi endpoint")
    p.add_argument("--max-own", type=int, default=None, help="only the first N watched films (trial run)")
    p.add_argument("--skip-own", action="store_true")
    p.add_argument("--skip-sample", action="store_true")
    p.add_argument("--skip-letterboxd", action="store_true", help="TMDB search only (own films); sample impossible")
    p.add_argument("--no-validate-search", action="store_true",
                   help="do not run title+year search on films that Letterboxd already resolved")
    p.add_argument("--no-preflight", action="store_true")
    p.add_argument("--assemble-only", action="store_true", help="no network: rebuild outputs from the cache")
    p.add_argument("--debug-html", type=int, default=3, help="save up to N unparseable pages to <out>/debug/")
    return p


@dataclass
class Deps:
    lb_session: Any = None
    tmdb_session: Any = None
    sleep: Callable[[float], None] = time.sleep
    clock: Callable[[], float] = time.monotonic
    rng: random.Random | None = None


def make_lb_session():
    try:
        from curl_cffi import requests as cr
    except ImportError as e:  # pragma: no cover - environment problem
        raise UsageError("curl_cffi is not installed (see RUN.md: uv pip install -r requirements.txt)") from e
    return cr.Session(impersonate="chrome")


def make_tmdb_session():
    import requests

    return requests.Session()


def main(argv: list[str] | None = None, deps: Deps | None = None) -> int:
    args = build_parser().parse_args(argv)
    deps = deps or Deps()
    started = time.time()
    key = os.environ.get("TMDB_API_KEY", "").strip()
    out_dir = Path(args.out).expanduser()
    log_lines: list[str] = []

    def log(msg: str) -> None:
        msg = redact(msg, [key])
        print(msg, file=sys.stderr, flush=True)
        log_lines.append(f"{now_iso()} {msg}")

    try:
        if not key and not args.assemble_only:
            raise UsageError("TMDB_API_KEY is not set (export TMDB_API_KEY=... first)")
        if args.skip_letterboxd and args.sample_slugs and not args.skip_sample:
            raise UsageError("--skip-letterboxd cannot be combined with a sample (slug -> TMDB id needs the page)")
        rows, info = ([], {}) if args.skip_own else load_watched(args.export)
        if args.max_own is not None:
            rows = rows[: args.max_own]
        slugs: list[str] = []
        sample_in_file = 0
        if args.sample_slugs and not args.skip_sample:
            slugs = load_slugs(args.sample_slugs)
            sample_in_file = len(slugs)
            random.Random(args.seed).shuffle(slugs)
            if args.sample_limit is not None:
                slugs = slugs[: args.sample_limit]
        out_dir.mkdir(parents=True, exist_ok=True)
    except UsageError as e:
        print(f"error: {e}", file=sys.stderr)
        return 2

    cache = Cache(out_dir / "cache.sqlite")
    debug_left = [max(0, args.debug_html)]

    def on_bad_page(kind: str, slug: str, text: str) -> None:
        if debug_left[0] <= 0:
            return
        debug_left[0] -= 1
        d = out_dir / "debug"
        d.mkdir(exist_ok=True)
        safe = re.sub(r"[^A-Za-z0-9_.-]+", "_", slug)[:80]
        (d / f"{kind}_{safe}.html").write_text(text, encoding="utf-8")
        log(f"saved unparseable {kind} page for {slug!r} to {d}")

    lb = tmdb = None
    if not args.assemble_only:
        if not args.skip_letterboxd:
            lb_session = deps.lb_session if deps.lb_session is not None else make_lb_session()
            lb = LetterboxdClient(
                lb_session, RateLimiter(args.lb_delay, args.lb_jitter, deps.clock, deps.sleep, deps.rng),
                retries=args.retries, sleep=deps.sleep, rng=deps.rng, on_bad_page=on_bad_page, log=log)
        tmdb_session = deps.tmdb_session if deps.tmdb_session is not None else make_tmdb_session()
        tmdb = TmdbClient(key, tmdb_session, RateLimiter(args.tmdb_delay, 0.0, deps.clock, deps.sleep), sleep=deps.sleep)
    col = Collector(cache, lb, tmdb, hist_source=args.hist_source,
                    validate_search=not args.no_validate_search, log=log)

    interrupted = False
    exit_code = 0
    try:
        if not args.assemble_only and not args.no_preflight:
            msgs, fatal = col.preflight()
            for m in msgs:
                log("preflight: " + m)
            if fatal:
                log("preflight failed. Fix the above, or use --skip-letterboxd (TMDB-only for own films), "
                    "or --no-preflight to push on anyway.")
                cache.close()
                return 3
        if not args.assemble_only:
            if rows:
                prog = Progress("own", len(rows))
                for i, row in enumerate(rows, 1):
                    col.process_own(row, online=True)
                    prog.update(i, _prog_extra(lb, tmdb))
                prog.update(len(rows), _prog_extra(lb, tmdb), force=True)
                prog.done()
            if slugs:
                prog = Progress("sample", len(slugs))
                for i, slug in enumerate(slugs, 1):
                    col.process_sample(slug, want_hist=(i <= args.hist_n), online=True)
                    prog.update(i, _prog_extra(lb, tmdb))
                prog.update(len(slugs), _prog_extra(lb, tmdb), force=True)
                prog.done()
    except KeyboardInterrupt:
        interrupted = True
        exit_code = 130
        log("interrupted: writing outputs from what is cached; re-run the same command to resume")
    except TmdbAuthError as e:
        log(f"fatal: {e}")
        exit_code = 4
        interrupted = True

    # assemble outputs from the cache (offline, deterministic)
    own_recs, own_h, smp_recs, smp_h = [], [], [], []
    for row in rows:
        r, h = col.process_own(row, online=False)
        own_recs.append(r)
        if h:
            own_h.append(h)
    for i, slug in enumerate(slugs, 1):
        r, h = col.process_sample(slug, want_hist=(i <= args.hist_n), online=False)
        smp_recs.append(r)
        if h:
            smp_h.append(h)
    write_jsonl(out_dir / "films_meta.jsonl", own_recs + smp_recs)
    write_jsonl(out_dir / "histograms.jsonl", own_h + smp_h)
    meta = {
        "script_version": __version__, "started_at": datetime.fromtimestamp(started, UTC).isoformat(timespec="seconds"),
        "finished_at": now_iso(), "elapsed_sec": round(time.time() - started, 1), "interrupted": interrupted,
        "python": sys.version.split()[0], "libs": _lib_versions(),
        "args": {k: v for k, v in vars(args).items() if k not in ("export", "out", "sample_slugs")},
        "inputs": {**info, "watched_films_used": len(rows), "sample_slugs_in_file": sample_in_file,
                   "sample_slugs_used": len(slugs)},
    }
    summary = build_summary(own_recs, own_h, smp_recs, smp_h, cache=cache, lb=lb, tmdb=tmdb, meta=meta)
    write_json(out_dir / "run_summary.json", summary)
    if log_lines:
        with open(out_dir / "collect.log", "a", encoding="utf-8") as fh:
            fh.write("\n".join(log_lines) + "\n")
    cache.close()
    print_final(summary, out_dir)
    return exit_code


def _prog_extra(lb: LetterboxdClient | None, tmdb: TmdbClient | None) -> str:
    parts = []
    if lb is not None:
        parts.append(f"LB req {lb.stats['requests']} retries {lb.stats['retries']}"
                     f"{' DISABLED' if lb.disabled else ''}")
    if tmdb is not None:
        parts.append(f"TMDB req {tmdb.stats['requests']}")
    return "| " + " | ".join(parts) if parts else ""


def _lib_versions() -> dict:
    out = {}
    for mod in ("curl_cffi", "requests", "bs4", "lxml"):
        try:
            m = __import__(mod)
            out[mod] = getattr(m, "__version__", None) or getattr(getattr(m, "etree", None), "LXML_VERSION", "?")
        except Exception:  # noqa: BLE001 - parsing/network must never crash the run
            out[mod] = None
    return {k: (".".join(map(str, v)) if isinstance(v, tuple) else v) for k, v in out.items()}


def print_final(s: dict, out_dir: Path) -> None:
    o, sm = s["own"], s["sample"]
    lines = ["", "=== run summary ==="]
    if o:
        lines.append(f"own films: {o['films']}  paths {o['resolve_path']}  types {o['tmdb_type']}")
        lines.append(f"  movie details {o['movie_details_ok']} ok / {o['movie_details_missing']} missing; "
                     f"countries {o['with_production_countries']}, directors {o['with_directors']}, "
                     f"composers {o['with_composers']}; histograms {o['histograms_ok']}")
        if "search_validation" in o:
            lines.append(f"  title+year search vs Letterboxd ids: {o['search_validation']}")
    if sm:
        lines.append(f"sample films: {sm['films']}  paths {sm['resolve_path']}  types {sm['tmdb_type']}")
        lines.append(f"  movie details {sm['movie_details_ok']} ok; countries {sm['with_production_countries']}; "
                     f"histograms {sm['histograms_ok']}/{sm['histograms_target']}")
    lines.append(f"open failures: {s['open_failures']['count']}  {s['open_failures']['by_stage_reason']}")
    for w in s["warnings"]:
        lines.append(f"WARNING: {w}")
    lines.append(f"outputs in {out_dir}: films_meta.jsonl histograms.jsonl run_summary.json"
                 f"{'  (interrupted: re-run to resume)' if s['interrupted'] else ''}")
    print("\n".join(lines))


if __name__ == "__main__":
    sys.exit(main())
