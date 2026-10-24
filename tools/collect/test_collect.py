"""Offline tests for collect.py.  No network: HTTP is faked, fixtures are constructed from the documented fields.

Run:  uv run --with-requirements requirements.txt --with pytest pytest -q test_collect.py
"""
# ruff: noqa: RUF059, RUF012
from __future__ import annotations

import builtins
import io
import itertools
import json
import random
from pathlib import Path

import pytest

import collect as c

HERE = Path(__file__).parent
FX = HERE / "fixtures"
EMAIL_SENTINEL = "secret.person+sentinel@example.invalid"
REAL_SAMLEARNER = Path("/home/claude/lb/swarm/samlearner/repo/tests/testdata/html/sample_letterboxd_film.html")


def fx(name: str) -> str:
    return (FX / name).read_text(encoding="utf-8")


def fxj(name: str) -> dict:
    return json.loads(fx(name))


# ----------------------------------------------------------------------------- fakes

class FakeTime:
    def __init__(self):
        self.t = 1000.0
        self.sleeps: list[float] = []

    def clock(self):
        return self.t

    def sleep(self, s):
        self.sleeps.append(s)
        self.t += s


class FakeResp:
    def __init__(self, status=200, text="", url="", headers=None):
        self.status_code, self.text, self.url, self.headers = status, text, url, headers or {}


class FakeSession:
    """session.get(url, **kw) -> FakeResp, driven by handler(url, kw)."""

    def __init__(self, handler):
        self.handler, self.calls = handler, []

    def get(self, url, **kw):
        self.calls.append((url, kw))
        r = self.handler(url, kw)
        if isinstance(r, BaseException):
            raise r
        if isinstance(r, FakeResp) and not r.url:
            r.url = url
        return r

    def urls(self, prefix=""):
        return [u for u, _ in self.calls if u.startswith(prefix)]


def seq_handler(responses):
    """Return responses in order; the last one repeats."""
    state = {"i": 0}

    def h(url, kw):
        r = responses[min(state["i"], len(responses) - 1)]
        state["i"] += 1
        return r
    return h


def make_lb_client(session, ft, **kw):
    kw.setdefault("retries", 3)
    return c.LetterboxdClient(session, c.RateLimiter(1.0, 0.0, ft.clock, ft.sleep, random.Random(0)),
                              sleep=ft.sleep, rng=random.Random(0), **kw)


PAGES = {  # slug -> fixture
    "the-zone-of-interest": "film_zone_of_interest.html",
    "great-teacher-onizuka": "film_tv_gto.html",
    "friends-1993": "film_no_tmdb_link.html",
    "obscure-short-feature": "film_low_activity_anchor_only.html",
}
BOXD = {"gJsA": "the-zone-of-interest", "qfq0": "great-teacher-onizuka", "5VZ0": "friends-1993",
        "cJds": "whiplash-2013", "zzz1": None}


class World:
    """A tiny fake Letterboxd + TMDB.  Count calls per URL, flip behaviours per test."""

    def __init__(self):
        self.lb_block_slugs: set[str] = set()      # film pages answering 403 forever
        self.lb_raise_on_nth_page: int | None = None
        self.page_requests = 0
        self.tmdb_search = {
            ("zone of interest", "2023"): [{"id": 467244, "title": "The Zone of Interest",
                                            "original_title": "The Zone of Interest", "release_date": "2023-12-15",
                                            "vote_count": 3100}],
            ("whiplash", "2013"): fxj("tmdb_search_whiplash_2013.json")["results"],
            ("intouchables", "2011"): fxj("tmdb_search_intouchables.json")["results"],
        }
        self.tmdb_movies = {
            467244: fxj("tmdb_movie_467244.json"),
            555001: fxj("tmdb_movie_555001.json"),
            367412: {"id": 367412, "title": "Whiplash", "original_title": "Whiplash", "release_date": "2013-10-18",
                     "runtime": 18, "production_countries": [{"iso_3166_1": "US", "name": "United States of America"}],
                     "original_language": "en",
                     "credits": {"crew": [{"id": 1, "name": "Damien Chazelle", "job": "Director"}]},
                     "release_dates": {"results": []}},
            77338: {"id": 77338, "title": "Intouchables", "original_title": "Intouchables", "release_date": "2011-11-02",
                    "runtime": 112, "production_countries": [{"iso_3166_1": "FR", "name": "France"}],
                    "original_language": "fr", "credits": {"crew": []}, "release_dates": {"results": []}},
        }
        self.lb = FakeSession(self.lb_handler)
        self.tm = FakeSession(self.tmdb_handler)

    def lb_handler(self, url, kw):
        if url.startswith("https://boxd.it/"):
            code = url.rsplit("/", 1)[1]
            slug = BOXD.get(code)
            if slug is None:
                return FakeResp(404, "not found")
            assert kw.get("allow_redirects") is False
            return FakeResp(302, "", headers={"Location": f"https://letterboxd.com/film/{slug}/"})
        if "/csi/film/" in url:
            slug = url.split("/csi/film/")[1].split("/")[0]
            if slug == "the-zone-of-interest":
                return FakeResp(200, fx("csi_hist_small.html"))
            return FakeResp(200, fx("csi_hist_abbreviated.html"))
        if "/film/" in url:
            slug = url.split("/film/")[1].strip("/")
            self.page_requests += 1
            if self.lb_raise_on_nth_page and self.page_requests == self.lb_raise_on_nth_page:
                return KeyboardInterrupt()
            if slug in self.lb_block_slugs or slug == "whiplash-2013":
                return FakeResp(403, fx("cloudflare_challenge.html"))
            if slug in PAGES:
                return FakeResp(200, fx(PAGES[slug]))
            return FakeResp(404, "nope")
        raise AssertionError(url)

    def tmdb_handler(self, url, kw):
        path = url.replace(c.TMDB_BASE, "")
        p = kw.get("params", {})
        if path.startswith("/movie/"):
            mid = int(path.split("/")[2])
            return FakeResp(200, json.dumps(self.tmdb_movies[mid])) if mid in self.tmdb_movies else FakeResp(404, "{}")
        if path == "/search/movie":
            q = c.norm_title(p["query"])
            year = p.get("primary_release_year") or p.get("year")
            res = self.tmdb_search.get((q, year if p.get("primary_release_year") else None), None)
            if res is None and "year" in p:  # year= stage: same table keyed on the year
                res = self.tmdb_search.get((q, p["year"]))
            res = res or []
            return FakeResp(200, json.dumps({"results": res}))
        if path == "/search/tv":
            return FakeResp(200, json.dumps({"results": []}))
        raise AssertionError(url)


def write_export(tmp_path: Path, rows: list[tuple[str, int | str, str]]) -> Path:
    exp = tmp_path / "export"
    exp.mkdir()
    lines = ["Date,Name,Year,Letterboxd URI"] + [f"2022-07-06,{n},{y},{u}" for n, y, u in rows]
    (exp / "watched.csv").write_text("\n".join(lines) + "\n", encoding="utf-8")
    (exp / "profile.csv").write_text(
        "Date Joined,Username,Given Name,Family Name,Email Address,Location,Website,Bio,Pronoun,Favorite Films\n"
        f"2022-07-06,someone,A,B,{EMAIL_SENTINEL},X,,,,\n", encoding="utf-8")
    return exp


OWN_ROWS = [
    ("The Zone of Interest", 2023, "https://boxd.it/gJsA"),
    ("Great Teacher Onizuka", 1998, "https://boxd.it/qfq0"),
    ("Friends", 1993, "https://boxd.it/5VZ0"),
    ("Whiplash", 2013, "https://boxd.it/cJds"),
    ("Intouchables", 2011, "https://boxd.it/zzz1"),
    ("The Zone of Interest", 2023, "https://boxd.it/gJsA"),  # duplicate row
]


def run_main(tmp_path, world, extra=(), env_key="TESTKEY", monkeypatch=None, exp=None, out=None):
    exp = exp or write_export(tmp_path, OWN_ROWS)
    out = out or (tmp_path / "out")
    ft = FakeTime()
    if monkeypatch is not None:
        monkeypatch.setenv("TMDB_API_KEY", env_key)
    deps = c.Deps(lb_session=world.lb, tmdb_session=world.tm, sleep=ft.sleep, clock=ft.clock, rng=random.Random(3))
    code = c.main(["--export", str(exp), "--out", str(out), "--retries", "1", "--no-preflight", *extra], deps)
    return code, out, ft


def read_jsonl(p: Path) -> list[dict]:
    return [json.loads(line) for line in p.read_text(encoding="utf-8").splitlines() if line.strip()]


# ----------------------------------------------------------------------------- pure helpers

@pytest.mark.parametrize("code,uid,td", [("728", 2704, 0), ("glS", 6286, 0), ("qfq0", 625580, 0), ("jUk4", 474474, 0)])
def test_decode_lid_known_films(code, uid, td):
    assert c.decode_lid(code) == (uid, td)


def test_decode_lid_rejects_bad_characters():
    with pytest.raises(ValueError):
        c.decode_lid("ab-c")


def test_lid_from_uri():
    assert c.lid_from_uri("https://boxd.it/jUk4") == "jUk4"
    assert c.lid_from_uri("http://boxd.it/728") == "728"
    assert c.lid_from_uri("") is None and c.lid_from_uri(None) is None


@pytest.mark.parametrize("a,b", [("The Intouchables", "Intouchables"), ("Mr. & Mrs. Smith", "mr and mrs smith"),
                                 ("Amélie", "Amelie"), ("A Fistful of Dollars", "Fistful of Dollars")])
def test_norm_title_equivalences(a, b):
    assert c.norm_title(a) == c.norm_title(b)


@pytest.mark.parametrize("label,val", [("half-★", 0.5), ("★", 1.0), ("★½", 1.5), ("★★★★★", 5.0), ("★★★½", 3.5), ("½", 0.5)])
def test_stars_to_value(label, val):
    assert c.stars_to_value(label) == val


@pytest.mark.parametrize("href,val", [("/film/x/ratings/rated/%C2%BD/by/rating/", 0.5), ("/film/x/ratings/rated/1/by/rating/", 1.0),
                                      ("/film/x/ratings/rated/4%C2%BD/by/rating/", 4.5), ("/film/x/", None)])
def test_rating_from_href(href, val):
    assert c.rating_from_href(href) == val


def test_parse_count_suffixes():
    assert c.parse_count("4,580", None) == (4580, False)
    assert c.parse_count("13.5", "K") == (13500, True)
    assert c.parse_count("1.2", "m") == (1_200_000, True)


@pytest.mark.parametrize("url,slug", [("https://letterboxd.com/film/forrest-gump/", "forrest-gump"),
                                      ("/film/whiplash-2013/", "whiplash-2013"),
                                      ("https://letterboxd.com/jas3344/film/cars/1/", "cars"),
                                      ("https://example.com/film/x/", None), ("https://letterboxd.com/list/x/", None)])
def test_slug_from_url(url, slug):
    assert c.slug_from_url(url) == slug


# ----------------------------------------------------------------------------- HTML parsing

def test_parse_full_film_page_fixture():
    d = c.parse_film_page(fx("film_zone_of_interest.html"), "https://letterboxd.com/film/the-zone-of-interest/")
    assert d["is_film_page"] and d["slug"] == "the-zone-of-interest"
    assert (d["tmdb_type"], d["tmdb_id"], d["imdb_id"]) == ("movie", 467244, "tt7160372")
    assert d["lb_avg"] == 3.86 and d["lb_rating_count"] == 871061 and d["lb_review_count"] == 232198
    assert d["countries"] == ["Poland", "UK", "USA"] and d["directors"] == ["Jonathan Glazer"]
    assert d["title"] == "The Zone of Interest" and d["year"] == 2023
    h = d["hist_inline"]
    assert h["complete"] and not h["approx"] and h["total"] == 871061
    assert h["counts"]["0.5"] == 4580 and h["counts"]["4.0"] == 269875 and h["counts"]["5.0"] == 127637
    assert h["plain_mean"] == pytest.approx(3.804379, abs=1e-6)
    assert (h["displayed_avg"], h["displayed_count"]) == (3.86, 871061)


def test_parse_tv_page_gives_type_tv():
    d = c.parse_film_page(fx("film_tv_gto.html"), "https://letterboxd.com/film/great-teacher-onizuka/")
    assert (d["tmdb_type"], d["tmdb_id"]) == ("tv", 43017)
    assert d["hist_inline"] is None


def test_tmdb_anchor_fallback_when_body_attributes_missing():
    d = c.parse_film_page(fx("film_low_activity_anchor_only.html"), "https://letterboxd.com/film/obscure-short-feature/")
    assert (d["tmdb_type"], d["tmdb_id"], d["imdb_id"]) == ("movie", 555001, "tt1234567")
    assert d["lb_avg"] is None and d["lb_rating_count"] is None  # low-activity film: no aggregateRating
    assert d["countries"] == ["Brazil"]


def test_page_without_tmdb_link_is_not_fooled_by_themoviedb_host():
    # the page has a nav link to https://www.themoviedb.org/ (host contains "movie"), but no /movie/<id>/ link
    d = c.parse_film_page(fx("film_no_tmdb_link.html"), "https://letterboxd.com/film/friends-1993/")
    assert d["is_film_page"] and d["tmdb_id"] is None and d["tmdb_type"] is None
    assert (d["title"], d["year"], d["lb_avg"]) == ("Friends", 1993, 2.4)


def test_average_falls_back_to_twitter_meta_without_jsonld():
    d = c.parse_film_page(fx("film_no_jsonld.html"), "https://letterboxd.com/film/some-film/")
    assert d["lb_avg"] == 2.75 and "avg_from_twitter_meta" in d["notes"] and d["tmdb_id"] == 777


def test_challenge_detector_does_not_flag_a_film_titled_access_denied():
    page = fx("film_tv_gto.html").replace("Great Teacher Onizuka (2023) directed", "Access Denied (2023) directed")
    assert not c.looks_like_challenge(200, page, {})
    assert c.looks_like_challenge(200, fx("cloudflare_challenge.html"), {})
    assert c.looks_like_challenge(200, "<html></html>", {"cf-mitigated": "challenge"})
    assert not c.looks_like_challenge(200, fx("csi_hist_small.html"), {})


def test_cloudflare_page_is_not_a_film_page():
    d = c.parse_film_page(fx("cloudflare_challenge.html"), "https://letterboxd.com/film/x/")
    assert not d["is_film_page"]


def test_histogram_abbreviated_counts_flag_approx():
    h = c.parse_histogram_html(fx("csi_hist_abbreviated.html"))
    assert h["complete"] and h["approx"]
    assert h["counts"]["0.5"] == 1850 and h["counts"]["2.0"] == 31500 and h["counts"]["5.0"] == 68700
    assert h["displayed_avg"] == 3.44 and h["displayed_count"] == 517000


def test_histogram_small_film_with_no_and_singular_labels():
    h = c.parse_histogram_html(fx("csi_hist_small.html"))
    assert not h["approx"] and h["complete"] and h["total"] == 61
    assert h["counts"]["0.5"] == 0 and h["counts"]["1.0"] == 1 and h["counts"]["2.0"] == 2


def test_histogram_empty_fragment_returns_none():
    assert c.parse_histogram_html(fx("csi_hist_empty.html")) is None


def test_histogram_rating_from_href_when_label_unparseable():
    html = ('<ul>' + "".join(
        f'<li class="rating-histogram-bar"><a href="/film/x/ratings/rated/{h}/by/rating/" '
        f'data-original-title="{n} Bewertungen">x</a></li>' for h, n in [("%C2%BD", 5), ("1", 7)]) + '</ul>')
    h = c.parse_histogram_html(html)
    assert h["counts"] == {"0.5": 5, "1.0": 7} and not h["complete"]


@pytest.mark.skipif(not REAL_SAMLEARNER.exists(), reason="real samlearner fixture not present on this machine")
def test_parser_on_real_saved_letterboxd_page():
    d = c.parse_film_page(REAL_SAMLEARNER.read_text(encoding="utf-8"), "https://letterboxd.com/film/the-zone-of-interest/")
    assert (d["tmdb_type"], d["tmdb_id"], d["imdb_id"]) == ("movie", 467244, "tt7160372")
    assert d["lb_avg"] == 3.86 and d["lb_rating_count"] == 871061
    assert d["hist_inline"]["complete"] and d["hist_inline"]["total"] == 871061


# ----------------------------------------------------------------------------- rate limiter and cache

def test_rate_limiter_enforces_gap_and_jitter():
    ft = FakeTime()
    rl = c.RateLimiter(1.0, 0.5, ft.clock, ft.sleep, random.Random(1))
    starts = []
    for _ in range(6):
        rl.wait()
        starts.append(ft.t)
        ft.t += 0.2  # a 0.2 s request
    gaps = [b - a - 0.2 for a, b in itertools.pairwise(starts)]
    assert all(0.8 - 1e-9 <= g + 0.2 <= 1.5 + 1e-9 for g in gaps)  # start-to-start gap in [1.0, 1.5]
    assert len({round(g, 3) for g in gaps}) > 1  # jitter actually varies


def test_rate_limiter_penalty_grows_and_decays():
    ft = FakeTime()
    rl = c.RateLimiter(1.0, 0.0, ft.clock, ft.sleep, random.Random(1))
    for _ in range(10):
        rl.slow_down()
    assert rl.penalty == 4.0
    for _ in range(100):
        rl.speed_up()
    assert rl.penalty == pytest.approx(1.0)


def test_cache_roundtrip_negative_and_failure_clearing(tmp_path):
    ca = c.Cache(tmp_path / "x.sqlite")
    assert ca.get("a") is c.MISSING
    ca.put("a", {"x": [1, 2]})
    assert ca.get("a") == {"x": [1, 2]}
    ca.neg("b", "gone")
    assert c.is_neg(ca.get("b"))
    ca.fail("k", "stage", "http_403")
    ca.fail("k", "stage", "http_403")
    assert ca.fail_info("k") == "http_403" and ca.open_failures() == [("k", "stage", "http_403", 2)]
    ca.put("k", {"ok": 1})  # success clears the failure
    assert ca.open_failures() == []
    ca.close()
    # persists across connections
    ca2 = c.Cache(tmp_path / "x.sqlite")
    assert ca2.get("a") == {"x": [1, 2]}


# ----------------------------------------------------------------------------- Letterboxd client

def test_lb_retries_429_then_succeeds_with_growing_backoff():
    ft = FakeTime()
    sess = FakeSession(seq_handler([FakeResp(429), FakeResp(429, headers={"Retry-After": "30"}),
                                    FakeResp(200, fx("film_zone_of_interest.html"))]))
    lb = make_lb_client(sess, ft)
    res = lb.fetch_page("the-zone-of-interest")
    assert res.kind == "ok" and res.value["tmdb_id"] == 467244
    assert lb.stats["requests"] == 3 and lb.stats["retries"] == 2 and lb.stats["http_429"] == 2
    backoffs = [s for s in ft.sleeps if s > 2.5]
    assert len(backoffs) >= 2 and backoffs[1] >= 30  # Retry-After honoured
    assert lb.limiter.penalty > 1.0 or lb.limiter.penalty == pytest.approx(1.0, abs=0.5)


@pytest.mark.parametrize("status", [403, 503])
def test_lb_gives_up_after_retries_and_reports_reason(status):
    ft = FakeTime()
    sess = FakeSession(seq_handler([FakeResp(status, fx("cloudflare_challenge.html"))]))
    lb = make_lb_client(sess, ft, retries=2)
    res = lb.fetch_page("some-film")
    assert res.kind == "fail" and res.reason == f"http_{status}"
    assert len(sess.calls) == 3


def test_lb_200_challenge_page_is_retried_not_parsed():
    ft = FakeTime()
    sess = FakeSession(seq_handler([FakeResp(200, fx("cloudflare_challenge.html")),
                                    FakeResp(200, fx("film_tv_gto.html"))]))
    lb = make_lb_client(sess, ft)
    res = lb.fetch_page("great-teacher-onizuka")
    assert res.kind == "ok" and res.value["tmdb_type"] == "tv" and len(sess.calls) == 2


def test_lb_404_is_permanent_and_not_retried():
    ft = FakeTime()
    sess = FakeSession(seq_handler([FakeResp(404, "nope")]))
    lb = make_lb_client(sess, ft)
    res = lb.fetch_page("gone")
    assert res.kind == "neg" and len(sess.calls) == 1


def test_lb_network_error_is_retried_and_secrets_not_needed():
    ft = FakeTime()
    sess = FakeSession(seq_handler([ConnectionError("boom"), FakeResp(200, fx("film_tv_gto.html"))]))
    lb = make_lb_client(sess, ft)
    assert lb.fetch_page("great-teacher-onizuka").kind == "ok"


def test_lb_non_film_200_page_is_a_failure_and_saved_for_debugging():
    ft = FakeTime()
    saved = []
    sess = FakeSession(seq_handler([FakeResp(200, "<html><body><p>Something else</p></body></html>")]))
    lb = make_lb_client(sess, ft, on_bad_page=lambda k, s, t: saved.append((k, s)))
    res = lb.fetch_page("weird")
    assert res.kind == "fail" and res.reason == "not_a_film_page" and saved == [("page", "weird")]


def test_boxd_redirect_resolves_slug_without_following():
    ft = FakeTime()
    sess = FakeSession(lambda url, kw: FakeResp(302, headers={"location": "/film/forrest-gump/"}))
    lb = make_lb_client(sess, ft)
    res = lb.resolve_boxd("728")
    assert res.kind == "ok" and res.value == "forrest-gump"
    assert sess.calls[0][1]["allow_redirects"] is False


def test_boxd_redirect_to_non_film_is_negative():
    ft = FakeTime()
    sess = FakeSession(lambda url, kw: FakeResp(302, headers={"location": "https://letterboxd.com/jas3344/list/x/"}))
    assert make_lb_client(sess, ft).resolve_boxd("VlBNg").kind == "neg"


def test_boxd_client_that_already_followed_the_redirect():
    ft = FakeTime()
    sess = FakeSession(lambda url, kw: FakeResp(200, "<html></html>", url="https://letterboxd.com/film/forrest-gump/"))
    res = make_lb_client(sess, ft).resolve_boxd("728")
    assert res.kind == "ok" and res.value == "forrest-gump"


def test_circuit_breaker_cools_down_then_disables():
    ft = FakeTime()
    logs = []
    sess = FakeSession(seq_handler([FakeResp(403)]))
    lb = make_lb_client(sess, ft, retries=0, breaker_threshold=3, breaker_trips=2, cooldown=100, log=logs.append)
    for i in range(3):
        assert lb.fetch_page(f"s{i}").kind == "fail"
    assert not lb.disabled and 100 in ft.sleeps  # first trip: cooldown 100 s
    for i in range(3):
        lb.fetch_page(f"t{i}")
    assert lb.disabled and any("DISABLED" in m for m in logs)
    n = len(sess.calls)
    res = lb.fetch_page("after")
    assert res.kind == "fail" and res.reason == "letterboxd_disabled" and len(sess.calls) == n


def test_histogram_request_has_referer_and_xhr_headers_and_parses():
    ft = FakeTime()
    sess = FakeSession(lambda url, kw: FakeResp(200, fx("csi_hist_small.html")))
    res = make_lb_client(sess, ft).fetch_histogram("tiny-film")
    assert res.kind == "ok" and res.value["total"] == 61
    url, kw = sess.calls[0]
    assert url == "https://letterboxd.com/csi/film/tiny-film/rating-histogram/"
    assert kw["headers"]["X-Requested-With"] == "XMLHttpRequest" and "/film/tiny-film/" in kw["headers"]["Referer"]


def test_histogram_with_no_bars_is_a_permanent_negative():
    ft = FakeTime()
    sess = FakeSession(lambda url, kw: FakeResp(200, fx("csi_hist_empty.html")))
    res = make_lb_client(sess, ft).fetch_histogram("unrated")
    assert res.kind == "neg" and res.reason == "no_histogram_bars"


# ----------------------------------------------------------------------------- TMDB client and extraction

def make_tmdb(sess, ft, key="abc123"):
    return c.TmdbClient(key, sess, c.RateLimiter(0.0, 0.0, ft.clock, ft.sleep), sleep=ft.sleep)


def test_tmdb_v3_key_goes_in_query_and_v4_token_in_header():
    ft = FakeTime()
    s3 = FakeSession(lambda u, k: FakeResp(200, "{}"))
    make_tmdb(s3, ft, "0123456789abcdef0123456789abcdef").get("/movie/1")
    assert s3.calls[0][1]["params"]["api_key"] == "0123456789abcdef0123456789abcdef"
    assert "Authorization" not in s3.calls[0][1]["headers"]
    s4 = FakeSession(lambda u, k: FakeResp(200, "{}"))
    make_tmdb(s4, ft, "eyJhbGciOi.payload.sig").get("/movie/1")
    assert s4.calls[0][1]["headers"]["Authorization"] == "Bearer eyJhbGciOi.payload.sig"
    assert "api_key" not in s4.calls[0][1]["params"]


def test_tmdb_401_is_fatal_404_is_negative():
    ft = FakeTime()
    with pytest.raises(c.TmdbAuthError):
        make_tmdb(FakeSession(lambda u, k: FakeResp(401, "{}")), ft).get("/movie/1")
    assert make_tmdb(FakeSession(lambda u, k: FakeResp(404, "{}")), ft).get("/movie/1").kind == "neg"


def test_tmdb_429_honours_retry_after_then_succeeds():
    ft = FakeTime()
    sess = FakeSession(seq_handler([FakeResp(429, headers={"Retry-After": "7"}), FakeResp(200, '{"id": 1}')]))
    res = make_tmdb(sess, ft).get("/movie/1")
    assert res.kind == "ok" and res.value == {"id": 1} and 7.0 in ft.sleeps


def test_tmdb_network_error_message_never_contains_the_key():
    ft = FakeTime()
    secret = "SUPERSECRETKEY0123456789abcdef00"
    sess = FakeSession(lambda u, k: ConnectionError(f"HTTPSConnectionPool: /3/movie/1?api_key={secret}&x=1 failed"))
    cl = c.TmdbClient(secret, sess, c.RateLimiter(0, 0, ft.clock, ft.sleep), retries=1, sleep=ft.sleep)
    res = cl.get("/movie/1")
    assert res.kind == "fail" and secret not in res.reason
    assert secret not in c.redact(f"url?api_key={secret}", [secret])


def test_extract_movie_record_from_fixture():
    r = c.extract_movie_record(fxj("tmdb_movie_467244.json"))
    assert r["production_countries"] == ["US", "GB", "PL"]
    assert r["production_country_names"][0] == "United States of America"
    assert r["original_language"] == "de" and r["release_date"] == "2023-12-15" and r["runtime"] == 105
    assert r["directors"] == [{"id": 59310, "name": "Jonathan Glazer"}]  # deduped, Assistant Director excluded
    assert r["composers"] == [{"id": 1227011, "name": "Mica Levi", "job": "Original Music Composer"},
                              {"id": 1227011, "name": "Mica Levi", "job": "Music"}]
    assert r["earliest_theatrical_date"] == "2023-12-08"  # type 2 beats type 3
    assert r["earliest_premiere_date"] == "2023-05-19"
    assert r["earliest_release_any"] == "2023-05-19"  # empty release_date entry ignored
    assert r["imdb_id"] == "tt7160372" and r["spoken_languages"] == ["de", "pl"]


def test_extract_movie_record_tolerates_missing_everything():
    r = c.extract_movie_record(fxj("tmdb_movie_555001.json"))
    assert r["release_date"] is None and r["runtime"] is None and r["imdb_id"] is None
    assert r["production_countries"] == [] and r["directors"] == [] and r["composers"] == []
    assert r["earliest_theatrical_date"] is None
    assert c.extract_movie_record({"id": 5})["directors"] == []


# ----------------------------------------------------------------------------- title+year matcher

class StubTmdb:
    def __init__(self, table):
        self.table, self.calls = table, []
        self.stats = c.Counter()

    def get(self, path, params=None):
        params = params or {}
        extra = next((f"{k}={params[k]}" for k in ("primary_release_year", "year", "first_air_date_year") if k in params), "")
        self.calls.append((path, params["query"], extra))
        return c.Result("ok", {"results": self.table.get((path, params["query"], extra), [])})


def collector_with(table, tmp_path):
    return c.Collector(c.Cache(tmp_path / "m.sqlite"), None, StubTmdb(table))


def test_matcher_prefers_exact_year_over_adjacent_year(tmp_path):
    table = {("/search/movie", "Whiplash", "primary_release_year=2013"): fxj("tmdb_search_whiplash_2013.json")["results"]}
    col = collector_with(table, tmp_path)
    d, _ = col.ensure_match("Whiplash", 2013, True)
    assert d["found"] and d["tmdb_id"] == 367412 and d["stage"] == "primary_release_year=2013"
    # asking for 2014: the stage-1/2 lookups miss, the year-1 stage returns both films and the exact-year one wins
    d1, _ = col.ensure_match("Whiplash", 2014, True)
    assert d1["found"] and d1["tmdb_id"] == 1964 and d1["stage"] == "primary_release_year=2013"
    d2, _ = col.ensure_match("Whiplash", 2016, True)  # two years off from both films: rejected
    assert not d2["found"]


def test_matcher_article_insensitive_and_year_stage_cascade(tmp_path):
    table = {("/search/movie", "The Intouchables", "year=2011"): fxj("tmdb_search_intouchables.json")["results"]}
    col = collector_with(table, tmp_path)
    d, _ = col.ensure_match("The Intouchables", 2011, True)
    assert d["found"] and d["tmdb_id"] == 77338 and d["stage"] == "year=2011"
    assert [x[2] for x in col.tmdb.calls] == ["primary_release_year=2011", "year=2011"]


def test_matcher_accepts_year_off_by_one_but_not_two(tmp_path):
    res = [{"id": 1, "title": "Same Name", "original_title": "Same Name", "release_date": "2012-05-05", "vote_count": 5}]
    table = {("/search/movie", "Same Name", "primary_release_year=2013"): res,
             ("/search/movie", "Same Name 2", "primary_release_year=2015"): res}
    col = collector_with(table, tmp_path)
    assert col.ensure_match("Same Name", 2013, True)[0]["found"]
    assert not col.ensure_match("Same Name 2", 2015, True)[0]["found"]


def test_matcher_ambiguous_picks_most_voted_and_flags_it(tmp_path):
    res = [{"id": 1, "title": "Twin", "original_title": "Twin", "release_date": "2000-01-01", "vote_count": 10},
           {"id": 2, "title": "Twin", "original_title": "Twin", "release_date": "2000-06-01", "vote_count": 500}]
    col = collector_with({("/search/movie", "Twin", "primary_release_year=2000"): res}, tmp_path)
    d, _ = col.ensure_match("Twin", 2000, True)
    assert d["tmdb_id"] == 2 and d["ambiguous"] and d["n_candidates"] == 2


def test_matcher_fuzzy_only_when_unique_and_close(tmp_path):
    res = [{"id": 9, "title": "Harry Potter and the Philosopher's Stone", "original_title": "Harry Potter and the Philosopher's Stone",
            "release_date": "2001-11-16", "vote_count": 99}]
    col = collector_with({("/search/movie", "Harry Potter and the Philosophers Stone", "primary_release_year=2001"): res}, tmp_path)
    d, _ = col.ensure_match("Harry Potter and the Philosophers Stone", 2001, True)
    assert d["found"] and d["fuzzy"] and d["tmdb_id"] == 9


def test_matcher_tv_fallback_only_when_no_movie_matches(tmp_path):
    table = {("/search/tv", "Great Teacher Onizuka", "first_air_date_year=1998"): fxj("tmdb_search_tv_gto.json")["results"]}
    col = collector_with(table, tmp_path)
    d, _ = col.ensure_match("Great Teacher Onizuka", 1998, True)
    assert d["found"] and d["tmdb_type"] == "tv" and d["tmdb_id"] == 43017


def test_matcher_without_year_and_cache_hit_makes_no_second_call(tmp_path):
    res = [{"id": 3, "title": "Solo", "original_title": "Solo", "release_date": "2018-05-15", "vote_count": 7}]
    col = collector_with({("/search/movie", "Solo", ""): res}, tmp_path)
    d, _ = col.ensure_match("Solo", None, True)
    assert d["found"] and d["stage"] == "no_year"
    n = len(col.tmdb.calls)
    col.ensure_match("Solo", None, True)
    assert len(col.tmdb.calls) == n


# ----------------------------------------------------------------------------- input loaders

def test_load_watched_handles_bom_missing_year_duplicates_and_no_uri(tmp_path):
    f = tmp_path / "watched.csv"
    f.write_bytes("﻿Date,Name,Year,Letterboxd URI\n2022-07-06,A,2000,https://boxd.it/aaa\n"
                  "2022-07-06,A again,2000,https://boxd.it/aaa\n2022-07-06,B,,https://boxd.it/bbb\n"
                  "2022-07-06,No Uri,1999,\n2022-07-06,No Uri,1999,\n".encode())
    rows, info = c.load_watched(tmp_path)
    assert [(r["name"], r["year"], r["lid"]) for r in rows] == [("A", 2000, "aaa"), ("B", None, "bbb"), ("No Uri", 1999, None)]
    assert info["duplicates_dropped"] == 2 and info["without_uri"] == 1


def test_load_watched_finds_nested_folder_and_errors_clearly(tmp_path):
    nested = tmp_path / "letterboxd-user-2026"
    nested.mkdir()
    (nested / "watched.csv").write_text("Date,Name,Year,Letterboxd URI\n2022-07-06,A,2000,https://boxd.it/aaa\n")
    assert len(c.load_watched(tmp_path)[0]) == 1
    with pytest.raises(c.UsageError):
        c.load_watched(tmp_path / "nope")


def test_load_slugs_variants(tmp_path):
    a = tmp_path / "a.csv"
    a.write_text("movie_id,n\nbarbie,5\nparasite-2019,4\nbarbie,3\n\n#comment\nhttps://letterboxd.com/film/casablanca/,1\n")
    assert c.load_slugs(a) == ["barbie", "parasite-2019", "casablanca"]
    b = tmp_path / "b.txt"
    b.write_text("barbie\nparasite-2019\n")
    assert c.load_slugs(b) == ["barbie", "parasite-2019"]  # headerless single column
    d = tmp_path / "d.csv"
    d.write_text("foo,bar\n1,2\n")
    with pytest.raises(c.UsageError):
        c.load_slugs(d)
    assert c.load_slugs(HERE / "pool_films_ge100.csv")[:1] == ["barbie"]


def test_loader_never_opens_profile_csv(tmp_path, monkeypatch):
    exp = write_export(tmp_path, OWN_ROWS)
    opened = []
    real_open = builtins.open

    def spy(file, *a, **k):
        opened.append(str(file))
        return real_open(file, *a, **k)
    monkeypatch.setattr(builtins, "open", spy)
    monkeypatch.setattr(io, "open", spy)
    c.load_watched(exp)
    assert any(p.endswith("watched.csv") for p in opened)
    assert not any("profile" in p for p in opened)


# ----------------------------------------------------------------------------- end to end

def test_end_to_end_own_films(tmp_path, monkeypatch, capsys):
    world = World()
    exp = write_export(tmp_path, OWN_ROWS)  # created before the spy so only the script's own opens are recorded
    opened = []
    real_open = builtins.open

    def spy(file, *a, **k):
        opened.append(str(file))
        return real_open(file, *a, **k)
    monkeypatch.setattr(builtins, "open", spy)
    monkeypatch.setattr(io, "open", spy)
    code, out, _ = run_main(tmp_path, world, monkeypatch=monkeypatch, exp=exp)
    assert code == 0
    assert any(Path(p).name == "watched.csv" for p in opened)
    recs = read_jsonl(out / "films_meta.jsonl")
    assert len(recs) == 5  # duplicate watched row dropped
    by = {r["name"]: r for r in recs}

    z = by["The Zone of Interest"]
    assert z["resolve_path"] == "lb_page" and (z["tmdb_type"], z["tmdb_id"]) == ("movie", 467244)
    assert z["lb_slug"] == "the-zone-of-interest" and z["imdb_id"] == "tt7160372"
    assert z["lb_avg"] == 3.86 and z["lb_rating_count"] == 871061
    assert z["production_countries"] == ["US", "GB", "PL"] and z["directors"][0]["name"] == "Jonathan Glazer"
    assert z["composers"][0]["name"] == "Mica Levi" and z["runtime"] == 105 and z["original_language"] == "de"
    assert z["search_check"]["agrees"] is True
    assert (z["lid_uid"], z["lid_type_digit"]) == c.decode_lid("gJsA") and z["lid_type_digit"] == 0

    g = by["Great Teacher Onizuka"]
    assert g["resolve_path"] == "lb_page" and g["tmdb_type"] == "tv" and g["status"]["tmdb"] == "skipped:tv"
    assert g["production_countries"] is None and g["search_check"] is None

    f = by["Friends"]  # Letterboxd page without a TMDB link -> falls back to search -> nothing found
    assert f["resolve_path"] == "unresolved" and f["status"]["page_ids"] == "no_tmdb_link" and f["lb_avg"] == 2.4

    w = by["Whiplash"]  # Letterboxd page blocked (403) -> TMDB title+year search
    assert w["resolve_path"] == "tmdb_search" and w["tmdb_id"] == 367412 and w["runtime"] == 18
    assert w["status"]["page"].startswith("fail:http_403") and w["match"]["stage"] == "primary_release_year=2013"

    i = by["Intouchables"]  # boxd.it 404 -> search
    assert i["resolve_path"] == "tmdb_search" and i["tmdb_id"] == 77338 and i["status"]["redirect"].startswith("neg:")

    assert len({tuple(sorted(r)) for r in recs}) == 1  # identical schema on every record

    hists = read_jsonl(out / "histograms.jsonl")  # TV skipped; the TMDB-less page still has a histogram
    assert [(h["slug"], h["source"]) for h in hists] == [("the-zone-of-interest", "inline"), ("friends-1993", "csi")]
    assert hists[0]["total"] == 871061 and hists[0]["lid"] == "gJsA" and hists[1]["approx"] is True
    assert not any("/csi/film/the-zone" in u for u in world.lb.urls())  # inline histogram complete: no extra request
    assert any("/csi/film/friends-1993/" in u for u in world.lb.urls())

    s = json.loads((out / "run_summary.json").read_text())
    assert s["own"]["films"] == 5 and s["own"]["resolve_path"] == {"lb_page": 2, "tmdb_search": 2, "unresolved": 1}
    assert s["own"]["tmdb_type"]["tv"] == 1 and s["own"]["movie_details_ok"] == 3
    assert s["own"]["with_production_countries"] == 3 and s["own"]["with_directors"] == 2
    assert s["own"]["runtime_le_40_min"] == 1
    assert s["own"]["search_validation"]["compared"] == 1 and s["own"]["search_validation"]["agree"] == 1
    assert s["inputs"]["duplicates_dropped"] == 1 and s["open_failures"]["count"] >= 1
    assert "TESTKEY" not in json.dumps(s)

    # privacy: the e-mail never appears in outputs, logs, stdout, stderr; profile.csv never opened
    cap = capsys.readouterr()
    blob = cap.out + cap.err + "".join(p.read_text(errors="ignore") for p in out.glob("*") if p.is_file() and p.suffix != ".sqlite")
    blob += (out / "cache.sqlite").read_bytes().decode("latin-1")
    assert EMAIL_SENTINEL not in blob and "TESTKEY" not in blob
    assert not [p for p in opened if Path(p).name == "profile.csv"], opened


def test_resume_makes_no_network_calls_and_gives_identical_output(tmp_path, monkeypatch):
    world = World()
    code, out, _ = run_main(tmp_path, world, monkeypatch=monkeypatch)
    first = (out / "films_meta.jsonl").read_text()
    lb_calls, tm_calls = len(world.lb.calls), len(world.tm.calls)
    world2 = World()
    # transient failures are retried on resume, permanent results are not: only the 403 film hits Letterboxd again
    code2, _, _ = run_main(tmp_path, world2, monkeypatch=monkeypatch, exp=tmp_path / "export", out=out)
    assert code2 == 0
    assert world2.lb.urls("https://letterboxd.com/film/whiplash-2013/")  # retried
    assert not world2.lb.urls("https://boxd.it/")  # redirects cached
    assert not [u for u in world2.lb.urls("https://letterboxd.com/film/") if "whiplash" not in u]
    assert not world2.tm.calls  # every TMDB answer cached
    assert (out / "films_meta.jsonl").read_text() == first
    assert lb_calls > 0 and tm_calls > 0


def test_assemble_only_needs_no_key_and_no_network(tmp_path, monkeypatch):
    world = World()
    code, out, _ = run_main(tmp_path, world, monkeypatch=monkeypatch)
    before = (out / "films_meta.jsonl").read_text()
    (out / "films_meta.jsonl").unlink()
    monkeypatch.delenv("TMDB_API_KEY")
    rc = c.main(["--export", str(tmp_path / "export"), "--out", str(out), "--assemble-only"], c.Deps())
    assert rc == 0
    assert (out / "films_meta.jsonl").read_text() == before


def test_interrupt_writes_partial_outputs_and_resume_completes(tmp_path, monkeypatch):
    world = World()
    world.lb_raise_on_nth_page = 3  # Ctrl-C while fetching the 3rd film page
    code, out, _ = run_main(tmp_path, world, monkeypatch=monkeypatch)
    assert code == 130
    s = json.loads((out / "run_summary.json").read_text())
    assert s["interrupted"] is True
    partial = read_jsonl(out / "films_meta.jsonl")
    assert len(partial) == 5 and any(r["resolve_path"] == "unresolved" or r["status"].get("page") == "not_fetched" for r in partial)
    world2 = World()
    code2, out2, _ = run_main(tmp_path, world2, monkeypatch=monkeypatch, exp=tmp_path / "export", out=out)
    assert code2 == 0 and json.loads((out / "run_summary.json").read_text())["interrupted"] is False
    done = {r["name"]: r for r in read_jsonl(out / "films_meta.jsonl")}
    assert done["Friends"]["status"]["page_ids"] == "no_tmdb_link" and done["Intouchables"]["tmdb_id"] == 77338


def test_no_validate_search_skips_extra_search_calls(tmp_path, monkeypatch):
    world = World()
    code, out, _ = run_main(tmp_path, world, ["--no-validate-search"], monkeypatch=monkeypatch)
    z = {r["name"]: r for r in read_jsonl(out / "films_meta.jsonl")}["The Zone of Interest"]
    assert z["search_check"] is None
    assert not any(kw["params"].get("query") == "The Zone of Interest" for _, kw in world.tm.calls if "params" in kw and "query" in kw["params"])


def test_skip_letterboxd_uses_tmdb_search_only(tmp_path, monkeypatch):
    world = World()
    code, out, _ = run_main(tmp_path, world, ["--skip-letterboxd"], monkeypatch=monkeypatch)
    assert code == 0 and not world.lb.calls
    recs = {r["name"]: r for r in read_jsonl(out / "films_meta.jsonl")}
    assert recs["The Zone of Interest"]["resolve_path"] == "tmdb_search" and recs["The Zone of Interest"]["tmdb_id"] == 467244
    assert recs["Whiplash"]["tmdb_id"] == 367412
    assert read_jsonl(out / "histograms.jsonl") == []


def test_hist_source_csi_fetches_endpoint_even_when_inline_exists(tmp_path, monkeypatch):
    world = World()
    code, out, _ = run_main(tmp_path, world, ["--hist-source", "csi"], monkeypatch=monkeypatch)
    h = read_jsonl(out / "histograms.jsonl")
    assert h and h[0]["source"] == "csi" and h[0]["total"] == 61  # csi_hist_small is served for the canary slug
    assert any("/csi/film/the-zone-of-interest/" in u for u in world.lb.urls())


def test_max_own_limits_trial_runs(tmp_path, monkeypatch):
    world = World()
    code, out, _ = run_main(tmp_path, world, ["--max-own", "2"], monkeypatch=monkeypatch)
    assert len(read_jsonl(out / "films_meta.jsonl")) == 2


def test_cli_errors(tmp_path, monkeypatch, capsys):
    exp = write_export(tmp_path, OWN_ROWS)
    monkeypatch.delenv("TMDB_API_KEY", raising=False)
    assert c.main(["--export", str(exp), "--out", str(tmp_path / "o1")], c.Deps()) == 2
    assert "TMDB_API_KEY" in capsys.readouterr().err
    monkeypatch.setenv("TMDB_API_KEY", "k")
    assert c.main(["--export", str(tmp_path / "missing"), "--out", str(tmp_path / "o2")], c.Deps()) == 2
    assert c.main(["--export", str(exp), "--out", str(tmp_path / "o3"), "--skip-letterboxd", "--sample-slugs",
                   str(HERE / "pool_films_ge100.csv")], c.Deps()) == 2


# ----------------------------------------------------------------------------- preflight

def test_preflight_ok_then_main_proceeds(tmp_path, monkeypatch, capsys):
    world = World()
    exp = write_export(tmp_path, OWN_ROWS[:1])
    monkeypatch.setenv("TMDB_API_KEY", "k")
    ft = FakeTime()
    deps = c.Deps(lb_session=world.lb, tmdb_session=world.tm, sleep=ft.sleep, clock=ft.clock, rng=random.Random(1))
    assert c.main(["--export", str(exp), "--out", str(tmp_path / "o"), "--retries", "1"], deps) == 0
    err = capsys.readouterr().err
    assert "preflight: ok   Letterboxd film page: TMDB movie/467244" in err and "preflight: ok   TMDB movie/467244" in err


def test_preflight_cloudflare_block_aborts_with_exit_3(tmp_path, monkeypatch, capsys):
    world = World()
    world.lb_block_slugs.add("the-zone-of-interest")
    exp = write_export(tmp_path, OWN_ROWS[:1])
    monkeypatch.setenv("TMDB_API_KEY", "k")
    ft = FakeTime()
    deps = c.Deps(lb_session=world.lb, tmdb_session=world.tm, sleep=ft.sleep, clock=ft.clock, rng=random.Random(1))
    assert c.main(["--export", str(exp), "--out", str(tmp_path / "o"), "--retries", "1"], deps) == 3
    assert "preflight failed" in capsys.readouterr().err
    assert not (tmp_path / "o" / "films_meta.jsonl").exists()


def test_preflight_bad_tmdb_key_aborts_with_exit_3(tmp_path, monkeypatch, capsys):
    world = World()
    world.tm = FakeSession(lambda u, k: FakeResp(401, "{}"))
    exp = write_export(tmp_path, OWN_ROWS[:1])
    monkeypatch.setenv("TMDB_API_KEY", "badkey")
    ft = FakeTime()
    deps = c.Deps(lb_session=world.lb, tmdb_session=world.tm, sleep=ft.sleep, clock=ft.clock, rng=random.Random(1))
    assert c.main(["--export", str(exp), "--out", str(tmp_path / "o"), "--retries", "1"], deps) == 3
    err = capsys.readouterr().err
    assert "401" in err and "badkey" not in err


# ----------------------------------------------------------------------------- calibration sample

SAMPLE = ["the-zone-of-interest", "great-teacher-onizuka", "obscure-short-feature", "friends-1993", "does-not-exist"]


def write_slugs(tmp_path) -> Path:
    p = tmp_path / "slugs.csv"
    p.write_text("slug\n" + "\n".join(SAMPLE) + "\n")
    return p


def test_sample_pipeline_histogram_subset_and_paths(tmp_path, monkeypatch):
    world = World()
    slugs = write_slugs(tmp_path)
    code, out, _ = run_main(tmp_path, world, ["--skip-own", "--sample-slugs", str(slugs), "--hist-n", "2", "--seed", "7"],
                            monkeypatch=monkeypatch, exp=write_export(tmp_path, OWN_ROWS))
    assert code == 0
    order = list(SAMPLE)
    random.Random(7).shuffle(order)
    recs = read_jsonl(out / "films_meta.jsonl")
    assert [r["lb_slug"] for r in recs] == order  # processed and written in the seeded random order
    by = {r["lb_slug"]: r for r in recs}
    assert by["the-zone-of-interest"]["tmdb_id"] == 467244 and by["the-zone-of-interest"]["production_countries"] == ["US", "GB", "PL"]
    assert by["great-teacher-onizuka"]["tmdb_type"] == "tv" and by["great-teacher-onizuka"]["directors"] is None
    assert by["obscure-short-feature"]["tmdb_id"] == 555001 and by["obscure-short-feature"]["lb_avg"] is None
    assert by["friends-1993"]["resolve_path"] == "unresolved"  # no TMDB link on the page and the title+year search finds nothing
    assert by["does-not-exist"]["resolve_path"] == "unresolved" and by["does-not-exist"]["status"]["page"].startswith("neg:")
    assert all(r["group"] == "sample" for r in recs)
    hists = read_jsonl(out / "histograms.jsonl")
    wanted = set(order[:2])
    expected = {s for s in wanted if s in ("the-zone-of-interest", "obscure-short-feature")}  # movies with a page
    assert {h["slug"] for h in hists} == expected
    assert all(h["group"] == "sample" for h in hists)
    s = json.loads((out / "run_summary.json").read_text())
    assert s["sample"]["films"] == 5 and s["sample"]["histograms_target"] == 2
    assert s["sample"]["histograms_missing_of_target"] == 2 - len(hists)
    assert s["inputs"]["sample_slugs_used"] == 5


def test_sample_limit_takes_a_random_prefix(tmp_path, monkeypatch):
    world = World()
    slugs = write_slugs(tmp_path)
    code, out, _ = run_main(tmp_path, world, ["--skip-own", "--sample-slugs", str(slugs), "--sample-limit", "2", "--seed", "1"],
                            monkeypatch=monkeypatch)
    order = list(SAMPLE)
    random.Random(1).shuffle(order)
    assert [r["lb_slug"] for r in read_jsonl(out / "films_meta.jsonl")] == order[:2]


def test_own_and_sample_share_the_cache(tmp_path, monkeypatch):
    world = World()
    slugs = write_slugs(tmp_path)
    run_main(tmp_path, world, ["--sample-slugs", str(slugs)], monkeypatch=monkeypatch)
    zone_page_requests = [u for u in world.lb.urls("https://letterboxd.com/film/the-zone-of-interest/")]
    assert len(zone_page_requests) == 1  # fetched once, used for the own film and the sample film
    recs = read_jsonl(tmp_path / "out" / "films_meta.jsonl")
    assert len(recs) == 5 + 5


def test_summary_plain_vs_displayed_stat_uses_only_exact_complete_histograms(tmp_path, monkeypatch):
    world = World()
    code, out, _ = run_main(tmp_path, world, monkeypatch=monkeypatch)
    s = json.loads((out / "run_summary.json").read_text())
    assert s["own"]["plain_vs_displayed_n"] == 1
    assert s["own"]["plain_vs_displayed_rmse"] == pytest.approx(abs(3.804379 - 3.86), abs=1e-3)
    assert s["own"]["histograms_ok"] == 2 and s["own"]["histograms_approx"] == 1
    assert s["own"]["histogram_sources"] == {"inline": 1, "csi": 1}


# ----------------------------------------------------------------------------- real libraries against a local server
# These exercise the real curl_cffi (Chrome impersonation) and requests clients over loopback HTTP, so the
# call signatures, redirect handling (allow_redirects=False vs True), gzip decoding and final-URL reporting are
# proven to work together.  They skip if the libraries are not installed.

import gzip
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class _Handler(BaseHTTPRequestHandler):
    log: list = []
    flaky = {"n": 0}

    def log_message(self, *a):  # silence
        pass

    def _send(self, status, body=b"", headers=None, gz=False):
        if gz:
            body = gzip.compress(body)
        self.send_response(status)
        for k, v in (headers or {}).items():
            self.send_header(k, v)
        if gz:
            self.send_header("Content-Encoding", "gzip")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        _Handler.log.append((self.path, dict(self.headers)))
        path = self.path.split("?")[0]
        if path.startswith("/boxd/"):
            return self._send(302, headers={"Location": "/film/old-slug/"} if path.endswith("old") else
                              {"Location": "/film/the-zone-of-interest/"})
        if path == "/film/old-slug/":
            return self._send(301, headers={"Location": "/film/the-zone-of-interest/"})
        if path == "/film/the-zone-of-interest/":
            return self._send(200, fx("film_zone_of_interest.html").encode(), {"Content-Type": "text/html; charset=utf-8"}, gz=True)
        if path == "/film/flaky/":
            _Handler.flaky["n"] += 1
            if _Handler.flaky["n"] == 1:
                return self._send(429, b"slow down", {"Retry-After": "0"})
            return self._send(200, fx("film_tv_gto.html").encode(), {"Content-Type": "text/html; charset=utf-8"})
        if path.startswith("/csi/film/"):
            return self._send(200, fx("csi_hist_abbreviated.html").encode(), {"Content-Type": "text/html; charset=utf-8"})
        if path == "/3/movie/467244":
            return self._send(200, json.dumps(fxj("tmdb_movie_467244.json")).encode(), {"Content-Type": "application/json"})
        return self._send(404, b"nope")


@pytest.fixture()
def local_server(monkeypatch):
    _Handler.log = []
    _Handler.flaky["n"] = 0
    srv = ThreadingHTTPServer(("127.0.0.1", 0), _Handler)
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    base = f"http://127.0.0.1:{srv.server_address[1]}"
    for var in ("HTTP_PROXY", "HTTPS_PROXY", "http_proxy", "https_proxy", "ALL_PROXY", "all_proxy"):
        monkeypatch.delenv(var, raising=False)
    monkeypatch.setenv("NO_PROXY", "127.0.0.1,localhost")
    monkeypatch.setattr(c, "BOXD_HOST", base + "/boxd")
    monkeypatch.setattr(c, "LB_HOST", base)
    monkeypatch.setattr(c, "TMDB_BASE", base + "/3")
    yield base
    srv.shutdown()


def test_real_curl_cffi_client_end_to_end(local_server):
    cr = pytest.importorskip("curl_cffi.requests")
    sess = cr.Session(impersonate="chrome")
    lb = c.LetterboxdClient(sess, c.RateLimiter(0.0, 0.0), retries=2, sleep=lambda s: None)
    r = lb.resolve_boxd("gJsA")
    assert (r.kind, r.value) == ("ok", "the-zone-of-interest")  # 302 not followed, Location parsed
    r = lb.resolve_boxd("old")  # Location points at a slug that itself redirects; only the first hop is read
    assert (r.kind, r.value) == ("ok", "old-slug")
    page = lb.fetch_page("old-slug")  # film page request follows the 301 and reports the canonical slug
    assert page.kind == "ok" and page.value["slug"] == "the-zone-of-interest" and page.value["requested_slug"] == "old-slug"
    assert page.value["tmdb_id"] == 467244 and page.value["hist_inline"]["total"] == 871061  # gzip decoded
    assert lb.fetch_page("flaky").kind == "ok" and lb.stats["http_429"] == 1  # retried after 429
    h = lb.fetch_histogram("king-kong-2005")
    assert h.kind == "ok" and h.value["approx"]
    seen = {p: hd for p, hd in _Handler.log}
    csi_headers = {k.lower(): v for k, v in seen["/csi/film/king-kong-2005/rating-histogram/"].items()}
    assert csi_headers["x-requested-with"] == "XMLHttpRequest" and "/film/king-kong-2005/" in csi_headers["referer"]
    assert "chrome" in csi_headers["user-agent"].lower()  # impersonation sends a Chrome UA
    assert lb.fetch_page("missing").kind == "neg"


def test_real_requests_client_for_tmdb(local_server):
    requests = pytest.importorskip("requests")
    tm = c.TmdbClient("0123456789abcdef0123456789abcdef", requests.Session(), c.RateLimiter(0.0, 0.0), sleep=lambda s: None)
    res = tm.get("/movie/467244", {"append_to_response": "credits,release_dates"})
    assert res.kind == "ok" and c.extract_movie_record(res.value)["directors"][0]["name"] == "Jonathan Glazer"
    path = next(p for p, _ in _Handler.log if p.startswith("/3/movie/467244"))
    assert "api_key=0123456789abcdef0123456789abcdef" in path and "append_to_response=credits%2Crelease_dates" in path
    assert tm.get("/movie/1").kind == "neg"


def test_unparseable_page_is_saved_under_debug_dir_up_to_the_limit(tmp_path, monkeypatch):
    world = World()
    orig = world.lb_handler

    def handler(url, kw):
        if "/film/" in url and "/csi/" not in url:
            return FakeResp(200, "<html><body><p>layout changed</p></body></html>")
        return orig(url, kw)
    world.lb = FakeSession(handler)
    code, out, _ = run_main(tmp_path, world, ["--debug-html", "2"], monkeypatch=monkeypatch)
    saved = sorted(p.name for p in (out / "debug").glob("*.html"))
    assert len(saved) == 2 and saved[0].startswith("page_")
    s = json.loads((out / "run_summary.json").read_text())
    assert s["open_failures"]["by_stage_reason"].get("film_page:not_a_film_page", 0) >= 3


def test_parse_histogram_table_markup_2026():
    h = c.parse_histogram_html(fx("csi_hist_table_2026.html"))
    assert h and h["complete"] and not h["approx"] and h["total"] == 4387536
    assert h["counts"]["0.5"] == 10129 and h["counts"]["4.0"] == 1261311
    assert abs(h["plain_mean"] - 4.1962) < 1e-3
