# standuser

Letterboxd export in, JoJo Stand card out.

![A sample Stand card](docs/sample-card.png)

*Sample card for a synthetic user (no real data).*

## Objective

Turn the viewing history in a Letterboxd export ZIP into a canon-faithful JoJo's Bizarre Adventure Stand card: six graded stats on the Part 5 chart, a Stand type, the nearest canon Stand, an ability, a weakness, a signature film and a name. It runs on any Letterboxd user's export, covers movies only, and uses no generated images, video or voice. It is a fan project and a portfolio piece.

## Description

One Rust crate, `standuser-core`, does all the work. A command-line tool and a static web page (the same crate compiled to WebAssembly) are thin front ends. There is no database and no account. On the web page the ZIP never leaves the browser.

The pipeline, one module per stage:

| Stage | What it does |
|---|---|
| `parse` | Reads `watched.csv`, `ratings.csv`, `diary.csv` from the ZIP, and only the `Date Joined` and `Username` columns of `profile.csv`. Every other `profile.csv` column (the e-mail address included) is skipped as raw bytes and never stored. |
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

- **Rust** (stable, 2021 edition): `standuser-core` (library), `standuser` (CLI), `standuser-wasm`. Crates: `zip`, `csv`, `serde`, `serde_json`, `unicode-normalization`, `ttf-parser`, `resvg` (PNG, CLI only), `wasm-bindgen`.
- **WebAssembly**: `wasm32-unknown-unknown` target, built with `wasm-pack`.
- **Static site**: plain HTML, CSS and JavaScript (ES modules). No bundler, no Node.
- **Vercel**: static hosting plus one serverless function, `api/tmdb.js`, a TMDB proxy for films missing from the built-in table. It reads `TMDB_API_KEY` from the environment.
- **Python 3** (tooling only): the data collector (`tools/collect`, with `curl_cffi`, `beautifulsoup4`, `lxml`), the table builder, the font subsetter (`fonttools`) and the golden-test reference (`numpy`).
- **Data**: your Letterboxd export; Letterboxd public film pages (rating histograms); TMDB API (countries, directors, composers, runtimes); the JoJo Wiki Stand Stats table (canon charts); Noto Sans JP and Noto Serif JP (SIL OFL) for the outlined text.

## Execution steps

### Use the web page

Open the deployed site, drop `letterboxd-<name>-<date>-utc.zip` on it (Letterboxd → Settings → Data → Export your data), download the PNG or SVG.

### Use the command line

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

### Build and serve the site

```bash
rustup target add wasm32-unknown-unknown && cargo install wasm-pack
tools/build_site.sh                  # writes site/pkg/*.wasm|js and site/films.json
python3 -m http.server -d site 8000  # then open http://localhost:8000
```

`site/pkg` and `site/films.json` are committed, so Vercel needs no build step. Without the proxy (plain `http.server`), untick "Look up films missing from the built-in table".

### Deploy to Vercel

1. Push this repository to GitHub.
2. In the Vercel dashboard choose **Add New → Project** and import the repository. Leave the build settings alone: `vercel.json` sets `site/` as the output directory and there is no build command.
3. In **Settings → Environment Variables** add `TMDB_API_KEY` (a TMDB v3 API key or v4 read access token) for Production and Preview, then redeploy.

### Refresh the film table

The table in `crates/standuser-core/data/films.json` comes from the collector. See `tools/collect/RUN.md`.

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

For each export: `card.svg`, `card.png` and `stand.json`. The console summary looks like this (synthetic user):

```
「COMPOSER 09」  Long-Distance Operation  (synthuser)
Power Speed Range Stamina Precision Potential: C B* ∞* A* A ?
Fictional Manifestation. Fiction from every nation walks into the world: 7.5 effective production countries per 50 films, across 270 films. [provisional]
Nearest canon: All Along Watchtower · Marilyn Manson · Clash
```

`stand.json` holds the timeline, every stat value, grade, percentile and status, the type, the three nearest Stands with distances, the ability text and its "why", and how many of your films the film table covered. It also lists the films behind the numbers with your own ratings, so do not publish it if you want those private; the card itself shows only a few titles.

## Limits

The choices behind these are written down in [docs/DECISIONS.md](docs/DECISIONS.md).

- **Cut-points.** Power and Precision use measured percentile cuts from a pool of 24,929 Letterboxd users, on a pool-scale crowd; they should be recalibrated on Letterboxd plain means. Speed and Stamina use MovieTweetings cuts and stay provisional until real dated Letterboxd exports exist. Range uses a US-heavy TMDB-5000 proxy and is provisional. **Potential has no valid cut-points, so it is shown as `?` with its raw value.**
- **Film coverage.** Crowd averages come from a built-in table of films. A film missing from it counts for Range and Potential (through the TMDB proxy) but not for Power or Precision. If more than half of your rated films are missing, Power and Precision show `?`.
- **Names.** The music-title naming route needs soundtrack track lists from MusicBrainz or Discogs, which cannot ship in a static site and are not fetched. Names therefore come from the composer or Part 3 routes.
- **Text.** Characters outside Latin, kana and the card's own kanji (for example Korean or Cyrillic titles) are drawn as □.
- Browser and proxy testing: `tools/site_test/run.sh` drives the real page in headless Chrome (file input, card, stats table, PNG/SVG/JSON downloads, theme change, lookup merge) and runs `api/tmdb.js` against a mocked TMDB. The live TMDB call and the Vercel deployment itself have not been exercised from this repository.

## Layout

```
crates/standuser-core   the library (parse … svg) and the golden test
crates/standuser-cli    standuser export.zip -> card.svg, card.png
crates/standuser-wasm   wasm-bindgen entry point
site/                   static page (index.html, app.js, style.css, pkg/, films.json)
api/tmdb.js             Vercel function: TMDB proxy
tools/collect           data collector (Letterboxd pages + TMDB) and its tests
tools/golden            Python reference, prototypes and fixture generators
tools/site_test         headless-browser tests for the page and the function
tools/*.py              table, font and calibration-table builders
```

## Credits and licence

MIT licence. Fan work, not affiliated with Letterboxd, TMDB, or the rights holders of JoJo's Bizarre Adventure. Film data: TMDB (this product uses the TMDB API but is not endorsed or certified by TMDB). Ratings data: Letterboxd.
