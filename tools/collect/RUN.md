# Running the calibration collector (WSL Ubuntu, uv)

`collect.py` is a measurement script, not the product. It needs a normal internet connection, your Letterboxd export folder and a free TMDB API key. It reads `watched.csv` only; `profile.csv` is never opened, so your e-mail address cannot end up in any output.

## What it writes (in `--out`)

| File | Share it back? | Content |
|---|---|---|
| `films_meta.jsonl` | yes | One record per film: your watched films first, then the calibration sample. Resolution path (`lb_page` / `tmdb_search` / `unresolved`), TMDB type and id, IMDb id, Letterboxd average and rating count, production countries, original language, release date, runtime, directors, composers (`Original Music Composer` and `Music`), earliest theatrical date |
| `histograms.jsonl` | yes | Letterboxd rating histograms (ten half-star counts, plain mean, displayed average, `approx` flag) for your films and for the first `--hist-n` sample films |
| `run_summary.json` | yes | Counts, match paths, failures, title+year search precision against the Letterboxd ids, plain-mean vs displayed-average error |
| `cache.sqlite` | no | Everything fetched. Makes the run resumable |
| `collect.log`, `debug/` | only if asked | Log, and up to 3 saved pages the parser could not read |

None of these files contains your key or e-mail. They do contain your film titles.

## 1. One-time setup

Put these files in one folder: `collect.py`, `test_collect.py`, `requirements.txt`, `pool_films_ge100.csv`, `fixtures/`.

```bash
mkdir -p ~/lb_collect && cd ~/lb_collect          # copy the files here
uv --version                                       # uv must exist; Python 3.14 is fetched if missing
export LB_EXPORT="/mnt/c/Users/<you>/Downloads/<unzipped letterboxd export folder>"
ls "$LB_EXPORT/watched.csv"                        # must exist
read -rs TMDB_API_KEY && export TMDB_API_KEY       # paste the key, press Enter (not saved in shell history)
```

The TMDB key is on themoviedb.org, Settings, API. Either the v3 "API Key" or the v4 "API Read Access Token" works.

Keep `--out` on the Linux filesystem (`~/...`), not under `/mnt/c`: SQLite is slow and fragile there.

## 2. Run the offline tests (about 3 seconds, no network)

```bash
cd ~/lb_collect
uv run --no-project --python 3.14 --with-requirements requirements.txt --with pytest pytest -q test_collect.py
```

Expected: `90 passed`. If the live-library tests fail, `curl_cffi` is not installable on your setup; stop and report.

## 3. Trial run: 10 of your films plus the preflight (about 1 minute)

```bash
uv run --no-project --python 3.14 --with-requirements requirements.txt \
  python collect.py --export "$LB_EXPORT" --out ~/lb_collect_trial --max-own 10
```

The preflight fetches one known film page, its histogram and one TMDB movie before doing anything else. It prints `ok` or `FAIL` lines and exits with code 3 on a failure.

- `FAIL Letterboxd film page ... http_403` or `cloudflare_challenge`: Cloudflare is blocking the client. Try later, or upgrade the fingerprint library with `--refresh` added to the `uv run` command. `--skip-letterboxd` still runs the TMDB-search-only path for your films.
- `FAIL ... no TMDB id found`: the page layout changed. Rerun with `--debug-html 3`, then share `~/lb_collect_trial/debug/*.html` (these are public pages).
- `HTTP 401`: wrong TMDB key.

Open `~/lb_collect_trial/run_summary.json` and check `own.resolve_path` is mostly `lb_page`.

## 4. Your 533 films (about 35 to 45 minutes)

```bash
uv run --no-project --python 3.14 --with-requirements requirements.txt \
  python collect.py --export "$LB_EXPORT" --out ~/lb_collect_out
```

Per film: one `boxd.it` redirect, one film page, one histogram request only if the film page has no complete inline histogram, then about three TMDB calls (details plus the title+year search used to measure the fallback matcher). Letterboxd requests are paced at 1.0 s plus 0 to 0.5 s random jitter. These times are arithmetic, not measured.

## 5. Calibration sample (staged)

`pool_films_ge100.csv` holds the 9,814 films that have at least 100 raters in the samlearner pool (slug, rater count, title, year; built from the pool's film table). The script shuffles it with `--seed 42`, so any prefix is a random sample and a bigger `--sample-limit` later reuses everything already cached.

```bash
# 3,000 films: about 1.2 hours (1 page request each, plus histograms for the first 1,000)
uv run --no-project --python 3.14 --with-requirements requirements.txt \
  python collect.py --export "$LB_EXPORT" --out ~/lb_collect_out \
  --sample-slugs pool_films_ge100.csv --sample-limit 3000 --hist-n 1000

# the rest (about 3.5 hours more): same command with --sample-limit 9814 (or drop the flag)
```

To use your own slug list, any CSV with a `slug` or `movie_id` column works, as does a plain list of slugs, one per line. To rebuild the list from the samlearner repo yourself: `git clone https://github.com/sdl60660/letterboxd_recommendations`, run `git lfs pull`, then count raters per `movie_id` in `data_processing/data/training_data_samples/*.parquet` and write the slugs with at least 100 raters to a CSV.

## 6. Interrupting, resuming, re-assembling

- `Ctrl-C` is safe. The script writes the three output files from what is cached and exits with code 130. Run the same command again to resume.
- Successes and permanent failures (404) are cached. Blocks and network errors are retried on every run.
- After 8 blocked fetches in a row it cools down (2 minutes, then 4); the third such streak disables Letterboxd for the rest of the run, and your films finish via TMDB search. Rerun later to fill the gaps.
- Rebuild the outputs without any network or key: add `--assemble-only`.
- Keep Windows awake during the sample run, or WSL pauses with it. A resume loses at most one film.

## 7. Send back

`films_meta.jsonl`, `histograms.jsonl`, `run_summary.json` from `~/lb_collect_out/`.

## Knobs

`--lb-delay 1.0 --lb-jitter 0.5` (Letterboxd pacing), `--tmdb-delay 0.05`, `--retries 4` (Letterboxd), `--hist-source csi` (always use the histogram endpoint instead of the inline histogram), `--no-validate-search`, `--skip-own`, `--skip-sample`, `--no-preflight`, `--seed`.
