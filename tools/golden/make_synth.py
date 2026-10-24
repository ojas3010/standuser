#!/usr/bin/env python3
"""Synthetic Letterboxd export + lookup table (no personal data) for the committed golden test.

  python3 -I tools/golden/make_synth.py   -> crates/standuser-core/tests/fixtures/synth/{export.zip, table.json, files/*.csv}
Deterministic: every random choice comes from SplitMix64.
"""
import csv, io, json, zipfile
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent.parent / "crates/standuser-core/tests/fixtures/synth"
M64 = (1 << 64) - 1


class R:
    def __init__(self, s):
        self.s = s

    def u64(self):
        self.s = (self.s + 0x9E3779B97F4A7C15) & M64
        z = self.s
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & M64
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & M64
        return z ^ (z >> 31)

    def f(self):
        return (self.u64() >> 11) / (1 << 53)

    def i(self, n):
        return int(self.f() * n)

    def norm(self):
        return sum(self.f() for _ in range(12)) - 6


import datetime as dt
rng = R(20261019)
NAMES = ["Quiet", "Harbor", "Lantern", "Orchard", "Meridian", "Ember", "Glass", "Paper", "Salt", "Winter", "Copper", "Velvet", "Static", "Tide", "Marble", "Signal"]
NOUNS = ["Road", "Garden", "Letters", "Engine", "Season", "Window", "Echo", "Border", "Mirror", "River", "Station", "Orchestra"]
COUNTRIES = [("US", "United States of America", 0.46), ("GB", "United Kingdom", 0.10), ("FR", "France", 0.09), ("JP", "Japan", 0.07), ("KR", "South Korea", 0.05),
             ("IN", "India", 0.06), ("DE", "Germany", 0.05), ("IT", "Italy", 0.04), ("ES", "Spain", 0.03), ("MX", "Mexico", 0.03), ("DK", "Denmark", 0.02)]
people, films = {}, []
for i in range(260):
    people[str(1000 + i)] = f"Director {i:02d}"
for i in range(12):
    people[str(5000 + i)] = f"Composer {i:02d}"

N = 300
for i in range(N):
    name = f"{NAMES[rng.i(len(NAMES))]} {NOUNS[rng.i(len(NOUNS))]} {i:03d}"
    year = 1960 + rng.i(65)
    r = rng.f()
    acc, cs = 0.0, []
    for code, _, w in COUNTRIES:
        acc += w
        if r < acc:
            cs = [code]
            break
    cs = cs or ["US"]
    if rng.f() < 0.25:
        cs.append(COUNTRIES[rng.i(len(COUNTRIES))][0])
    cs = sorted(set(cs))
    crowd = round(min(4.6, max(1.8, 3.4 + 0.5 * rng.norm())), 2)
    cnt = 12000 + rng.i(900000) if rng.f() > 0.12 else 2000 + rng.i(20000)
    dirs = [1000 + (rng.i(8) if rng.f() < 0.35 else rng.i(260))]
    comps = [5000 + rng.i(12)]
    runtime = 95 + rng.i(60)
    films.append(dict(i=i, name=name, year=year, lid=f"s{i:03d}", countries=cs, crowd=crowd, cnt=cnt, dirs=dirs, comps=comps, runtime=runtime))

table = dict(v=1, countries={c: n for c, n, _ in COUNTRIES}, people=people, films=[])
for f in films:
    key = "".join(ch for ch in f["name"] if ch.isalnum()).lower() + f"|{f['year']}"
    hist = f["crowd"] if f["i"] % 3 else None
    table["films"].append([key, f["lid"], 100000 + f["i"], f["year"], f["runtime"], "en", "|".join(f["countries"]), ",".join(map(str, f["dirs"])), ",".join(map(str, f["comps"])),
                           round(f["crowd"] + 0.03, 2), hist, f["cnt"]])
# one short film that must be ignored for countries and directors
table["films"][0][4] = 12

join = dt.date(2023, 1, 5)
as_of = dt.date(2026, 6, 30)
watched, ratings, diary = [], [], []
# join day: 40 films marked in bulk
order = list(range(N))
for k in range(N - 1, 0, -1):
    j = rng.i(k + 1)
    order[k], order[j] = order[j], order[k]
unknown_cut = int(N * 0.9)  # the last 10% of the table films are not in the export at all
pos = 0
for idx in order[:40]:
    f = films[idx]
    watched.append((join, f))
d = join + dt.timedelta(days=9)
span = (as_of - dt.timedelta(days=40) - d).days
for idx in order[40:unknown_cut]:
    f = films[idx]
    d = join + dt.timedelta(days=9 + int(rng.f() ** 1.3 * span))
    # skip two silent stretches
    if dt.date(2024, 3, 1) <= d < dt.date(2024, 5, 20) or dt.date(2025, 8, 1) <= d < dt.date(2025, 9, 15):
        d += dt.timedelta(days=90)
    watched.append((d, f))
for d, f in watched:
    live = rng.f() < 0.7
    if d != join and live:
        diary.append((d, f, d))
    elif d != join and rng.f() < 0.2:
        diary.append((d + dt.timedelta(days=int(rng.f() * 3)), f, d))
ratings_pool = [f for _, f in watched if rng.f() < 0.9]
for f in ratings_pool:
    own = min(5.0, max(0.5, round((f["crowd"] + 0.25 * rng.norm() + 0.1) * 2) / 2))
    ratings.append((f, own))

# films the export has but the table does not know (renamed, obscure)
extras = [("Unlisted Short Story", 2011), ("Nameless Reel", 1999), ("A Film Not In Any Table", 2020)]
for k, (n, y) in enumerate(extras):
    watched.append((dt.date(2025, 3, 1 + k * 4), dict(name=n, year=y, lid=f"u{k}")))
    ratings.append((dict(name=n, year=y, lid=f"u{k}"), 3.5))


def csv_text(header, rows):
    b = io.StringIO()
    w = csv.writer(b, lineterminator="\n")
    w.writerow(header)
    w.writerows(rows)
    return b.getvalue()


files = {
    "watched.csv": csv_text(["Date", "Name", "Year", "Letterboxd URI"], [(d.isoformat(), f["name"], f["year"], f"https://boxd.it/{f['lid']}") for d, f in sorted(watched, key=lambda x: (x[0], x[1]["name"]))]),
    "ratings.csv": csv_text(["Date", "Name", "Year", "Letterboxd URI", "Rating"], [(as_of.isoformat(), f["name"], f["year"], f"https://boxd.it/{f['lid']}", r) for f, r in ratings]),
    "diary.csv": csv_text(["Date", "Name", "Year", "Letterboxd URI", "Rating", "Rewatch", "Tags", "Watched Date"],
                          [(l.isoformat(), f["name"], f["year"], f"https://boxd.it/{f['lid']}", "", "", "", v.isoformat()) for l, f, v in sorted(diary, key=lambda x: (x[0], x[1]["name"]))]),
    "profile.csv": csv_text(["Date Joined", "Username", "Given Name", "Family Name", "Email Address", "Location", "Website", "Bio", "Pronoun", "Favorite Films"],
                            [(join.isoformat(), "synthuser", "Syn", "Thetic", "synthetic@example.invalid", "", "", "", "", "")]),
}
OUT.mkdir(parents=True, exist_ok=True)
(OUT / "files").mkdir(exist_ok=True)
for n, t in files.items():
    (OUT / "files" / n).write_text(t, encoding="utf-8")
with zipfile.ZipFile(OUT / "letterboxd-synthuser-2026-06-30-12-00-utc.zip", "w", zipfile.ZIP_DEFLATED) as z:
    for n, t in files.items():
        z.writestr(n, t)
    z.writestr("likes/films.csv", "Date,Name,Year,Letterboxd URI\n")
(OUT / "table.json").write_text(json.dumps(table, separators=(",", ":")), encoding="utf-8")
print(len(watched), "watched", len(ratings), "ratings", len(diary), "diary entries", len(table["films"]), "table films")
