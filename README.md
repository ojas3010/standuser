# standuser

A website where you drop your Letterboxd export and get a JoJo Stand card. It runs entirely in your browser.

![A sample Stand card](docs/sample-card.png)

*Sample card for a synthetic user (no real data).*

## Objective

Turn the viewing history in a Letterboxd export ZIP into a canon-faithful JoJo's Bizarre Adventure Stand card: six graded stats on the Part 5 chart, a Stand type, the nearest canon Stand, an ability, a weakness, a signature film and a name. It runs on any Letterboxd user's export, covers movies only, and uses no generated images, video or voice. It is a fan project and a portfolio piece.

## Description

The product is a static website. One Rust crate, `standuser-core`, does all the work and runs in the browser as WebAssembly; a command-line tool built on the same crate exists for development (see Development). There is no database, no account and no server: your export is read in the browser and never leaves it, and neither the page nor the CLI makes any network request while it works. Film data (countries, directors, composers, Letterboxd averages) comes from a table built ahead of time and shipped with the program.

The pipeline, one module per stage:

| Stage | What it does |
|---|---|
| `parse` | Reads `watched.csv`, `ratings.csv`, `diary.csv` from the ZIP, and only the `Date Joined` column of `profile.csv`, in memory. Every other `profile.csv` column (the e-mail address and the username included) is never decoded, stored, shown or logged. Without `profile.csv` or that column the join day is the earliest watch date, and the username comes from the ZIP file name. |
| `timeline` | Builds a clean activity timeline: drops the join day and bulk-marking days (14 or more films, or 8 or more with fewer than half corroborated by the diary), one event per film, rewatches ignored. |
| `stats` | Power, Speed, Range, Stamina, Precision, Potential from the timeline, your ratings and film data (see below). |
| `grading` | Card letters E to A from population percentiles (cuts at 10, 30, 70 and 90), plus a canon-equivalent grade used only for the type and the match. Rare symbols: ∞, ∅, and `?` below a stat's minimum data. |
| `types` | Close-Range Power, Long-Distance Operation, Automatic or Unclassified. About 60% leave-one-group-out agreement with canon, so the card calls it an archetype by stats. |
| `matching` | Nearest canon Stand (157 rows, 128 fully lettered) by L2 distance, with unknown canon cells costed at their expected error. Identical-chart twins are split by a hash of the username. The top 3 distinct charts are shown. |
| `ability` | 17 deterministic text templates keyed by your most extreme stat, a weakness from your lowest, and a signature film (largest gap to the Letterboxd crowd). Every number in the text comes from a slot, and a "why" line cites it. |
| `name` | Offline naming route: your top film composer (shrunk mean of your ratings, at least two films), else Part 3 style colour plus Tarot card. |
| `svg` | The card as one self-contained SVG. All text is drawn as outlines, so it renders the same in browsers, GitHub READMEs and PNG renderers. |

The six stats:

| Stat | Signal |
|---|---|
| Power | How far below the Letterboxd crowd you rate: minus the shrunk mean of (your rating − crowd mean), shrinkage 6 |
| Speed | Films per active month on the clean timeline |
| Range | Rarefied effective number of production countries (20 seeded draws of 50 films) |
| Stamina | Share of complete months with at least one film |
| Precision | Difficulty-adjusted pairwise agreement with the crowd |
| Potential | Share of director credits seen only once (Good-Turing) |

Grades marked `*` on the card are provisional. See [Limits](#limits) for what is and is not calibrated.

## Tools

- **Rust** (stable, 2021 edition): `standuser-core` (library), `standuser-wasm` (the website's engine), `standuser` (development CLI). Crates: `zip`, `csv`, `serde`, `serde_json`, `unicode-normalization`, `ttf-parser`, `resvg` (PNG, CLI only), `wasm-bindgen`.
- **WebAssembly**: `wasm32-unknown-unknown` target, built with `wasm-pack`.
- **Static site**: plain HTML, CSS and JavaScript (ES modules). No bundler, no Node.
- **Cloudflare Pages**: static hosting of the `site/` folder. No functions, no build step in the dashboard.
- **Python 3** (build-time tooling only, never at run time): the data collector (`tools/collect`, with `curl_cffi`, `beautifulsoup4`, `lxml`), the table builder, the font subsetter (`fonttools`) and the golden-test reference (`numpy`).
- **Data**: your Letterboxd export; Letterboxd public film pages (rating histograms); TMDB API (countries, directors, composers, runtimes); the JoJo Wiki Stand Stats table (canon charts); Noto Sans JP and Noto Serif JP (SIL OFL) for the outlined text.

## Execution steps

### Use the website

Open the deployed site, drop `letterboxd-<name>-<date>-utc.zip` on it (Letterboxd → Settings → Data → Export your data), and download the card as PNG or SVG, or the numbers behind it as JSON. The export is read in your browser and never leaves it; the page makes no network request while it works.

### Deploy to Cloudflare Pages

1. Push this repository to GitHub.
2. In the Cloudflare dashboard choose **Workers & Pages → Create → Pages → Connect to Git** and pick the repository.
3. Framework preset **None**; build command **empty**; build output directory **`site`**. Nothing else to set: there are no environment variables and no functions. `site/_headers` supplies the security headers (a strict content security policy that only allows the page's own files).

To update the site, run `tools/build_site.sh`, commit `site/` and push.

## Development

Everything below is for working on the project, not for using it.

### Build and serve the site locally

```bash
rustup target add wasm32-unknown-unknown && cargo install wasm-pack
tools/build_site.sh                  # writes site/pkg/*.wasm|js and site/films.json
python3 -m http.server -d site 8000  # then open http://localhost:8000
```

`site/` is the whole site and is committed (`pkg/`, `films.json`, `_headers`, the page), so hosting needs no build.

### Command-line tool

A command-line front end to the same crate, handy for scripting and for checking output without a browser.

```bash
cargo build --release -p standuser
./target/release/standuser letterboxd-yourname-2026-10-19-13-22-utc.zip --out out/
```

Writes `out/card.svg` (light, with a dark-mode override), `out/card.png` (1080×1350) and `out/stand.json` (every number behind the card). Options: `--theme dark`, `--joined YYYY-MM-DD` and `--username` (when the ZIP has no `profile.csv`), `--table films.json` (a different film table), `--json`.

### Run the tests

```bash
cargo test --workspace
```

The golden test compares the Rust port with a Python reference that runs the research prototypes (`tools/golden/proto`) on the same export. The committed fixture is a synthetic export. To also check a real export:

```bash
STANDUSER_OJAS_EXPORT=/path/to/export.zip STANDUSER_OJAS_TABLE=/path/to/films.json cargo test -p standuser-core --test golden
```

Regenerate fixtures with `tools/golden/regenerate.sh`. Browser tests: `tools/site_test/run.sh` (headless Chrome or Edge).

### Refresh the film table

The table in `crates/standuser-core/data/films.json` comes from the collector. See `tools/collect/RUN.md`. It is stamped with its build date. TMDB's terms allow caching its data for at most six months, so the CLI and the page add a note to `stand.json` once the table is older than that, and `tools/refresh_table.sh` rebuilds it (collector output in, table, site and CLI out). The collector and `tools/calibrate.py` are the only parts that touch the network, and only when you run them.

```bash
cd tools/collect
python3 -m venv .venv && .venv/bin/pip install -r requirements.txt pytest
.venv/bin/python -m pytest -q test_collect.py
TMDB_API_KEY=... .venv/bin/python collect.py --export <unzipped export> --out ~/lb_collect_out \
  --sample-slugs pool_films_ge100.csv --sample-limit 3000 --hist-n 1000
python3 -I tools/build_table.py ~/lb_collect_out
tools/build_site.sh
```

## Output

From the website, for each export: `card.png` (1080×1350), `card.svg` (light, with a dark-mode override) and `stand.json`. The command-line tool writes the same three files and prints a summary like this (synthetic user):

```
「COMPOSER 09」  Long-Distance Operation  (synthuser)
Power Speed Range Stamina Precision Potential: C B* ∞* A* A ?
Fictional Manifestation. Fiction from every nation walks into the world: 7.5 effective production countries per 50 films, across 270 films. [provisional]
Nearest canon: All Along Watchtower · Marilyn Manson · Clash
```

`stand.json` holds the timeline, every stat value, grade, percentile and status, the type, the three nearest Stands with distances, the ability text and its "why", and, for each stat, how many of your films it was graded on (`coverage.per_stat`). It also lists the films behind the numbers with your own ratings, so do not publish it if you want those private; the card itself shows only a few titles.

## Limits

The choices behind these are written down in [docs/DECISIONS.md](docs/DECISIONS.md).

- **Cut-points.** Power, Precision, Range and Potential are cut on pooled Letterboxd users restricted to the films in the table (`tools/calibrate.py`); Speed and Stamina use MovieTweetings cuts and stay provisional until real dated Letterboxd exports exist. Potential is graded on a volume-matched measure against users with a similar number of films. See [docs/DECISIONS.md](docs/DECISIONS.md) for the evidence.
- **Film coverage.** Every stat needs film data from the built-in table except Speed and Stamina, which read only your viewing dates. A film missing from the table is skipped by every stat that needs film data. On the pooled Letterboxd users the table holds a median of 96% of a user's rated films (10th percentile 81%); that is favourable by construction, since the table is the pool's most-rated films. Each card cell says "graded on N of M films" (for Speed and Stamina, "N of M dated watches", bulk-import days excluded), and a stat shows `?` when N is below the spec's minimum (Power 20 rated films, Precision 50, Range 50 with countries, Potential 40 with directors, Speed and Stamina 12 events). If more than half of your rated films are missing, Power and Precision show `?`.
- **Names.** The music-title naming route needs soundtrack track lists from MusicBrainz or Discogs, which cannot ship in a static site and are not fetched. Names therefore come from the composer or Part 3 routes.
- **Text.** Characters outside Latin, kana and the card's own kanji (for example Korean or Cyrillic titles) are drawn as □.
- Browser testing: `tools/site_test/run.sh` drives the real page in headless Chrome (file input, card, stats table with coverage, PNG/SVG/JSON downloads, theme change) and checks that no network request is made while an export is processed. The Cloudflare Pages deployment itself has not been exercised from this repository.

## Layout

```
crates/standuser-core   the library (parse … svg) and the golden test
crates/standuser-cli    development CLI: standuser export.zip -> card.svg, card.png
crates/standuser-wasm   the website's engine (wasm-bindgen entry point)
site/                   the static site, deployed as is (index.html, app.js, style.css, pkg/, films.json, _headers)
tools/collect           data collector (Letterboxd pages + TMDB) and its tests
tools/golden            Python reference, prototypes and fixture generators
tools/site_test         headless-browser test of the page
tools/*.py              table, font and calibration-table builders
```

## Credits and licence

MIT licence. Fan work, not affiliated with Letterboxd, TMDB, or the rights holders of JoJo's Bizarre Adventure. Ratings data: Letterboxd. Film data: TMDB. This website uses TMDB and the TMDB APIs but is not endorsed, certified, or otherwise approved by TMDB.
