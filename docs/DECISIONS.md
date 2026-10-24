# Decisions made while porting the spec

The Python prototypes and reports under `swarm/`, `reports/` and `research_notes/` were the specification. Where they were silent, contradictory or needed data that is not on the build machine, these are the choices made. Each is easy to change; the file that holds it is named.

## Stack

- **TMDB proxy runs on Vercel, not Cloudflare.** HANDOFF.md named a Cloudflare Worker; the v1 request named a Vercel function. `api/tmdb.js`.
- **Text is outlined, not embedded.** The card draws every glyph as a path from Noto Sans/Serif JP Bold subsets, so there is no `<text>` and no font-loading problem. The cost is a larger SVG (about 650 KB) and no support for scripts outside the subset (Latin, Greek, Cyrillic basics, kana and the card's own kanji); other characters become □. `crates/standuser-core/src/font.rs`, `tools/make_fonts.py`.
- **`gen_card.py` line 121 bug.** In embed mode the text lost the `f-` prefix on its class and rendered black. The Rust renderer has only the outlined mode and always emits `class="f-{cls}"`; a unit test fails if any bare class appears.
- **The site table ships with the page** (`site/films.json`) and the CLI embeds the same file. Films outside it can be filled in through the proxy, which returns the same JSON format.

## Deviations from the spec

Two choices contradict the written spec ("Letterboxd JoJo Stand stats 2"). Figures below come from `tools/evidence.py` run on the pooled Letterboxd users (sdl60660/letterboxd_recommendations) restricted to the films in our table; they are from the partial table of 920 films and are refreshed when the table is rebuilt (the final-table figures replace them).

### 1. Potential is graded on volume-matched Good-Turing (60 random films), not on the raw share

- **Spec:** Potential = N1 / director credits over all of a user's films, with no cut-points; the volume-matched value was only an evidence line.
- **Now:** the card letter comes from the mean of 20 seeded draws of 60 director-credited films (N1 / credits per draw). The raw share stays in the card text ("47% directors seen once"). The 完成 symbol needs 1,000 credited films and the lowest band, as in the spec.
- **Reason:** the raw share falls with the number of films, so a population cut from users with 60 to 300 films would grade a 500-film export as low only because it is long, and a user's letter would drift as they log more films. The spec itself says Potential "is meant to fall with volume" for mastery, but then the lowest band would be reached by volume alone, which the 完成 volume gate was meant to separate from narrowness.
- **Evidence** (21,268 pooled users with at least 60 director-credited table films, median 89 films):

| | Spearman with film count | median, 60-100 films | median, 100-200 films | median, 200+ films |
|---|---|---|---|---|
| raw Good-Turing | −0.59 | 0.709 | 0.586 | 0.486 |
| volume-matched, 60 films | −0.05 | 0.762 | 0.747 | 0.810 |

- **Cost:** the letter no longer reflects "how much is left to explore" at the user's real volume; the evidence line does. Users with 40 to 59 credited films are drawn on all their films and marked provisional.

### 2. The crowd baseline is Letterboxd's displayed average, not the histogram plain mean

- **Spec:** the crowd is the plain mean from the rating histogram ("production uses the plain mean"), else the displayed average, else a bundled pool table, one source per grade.
- **Now:** every Power and Precision grade uses the displayed average (`lb_avg`), and the cut-points are calibrated on it.
- **Reason:** the collector reads a histogram for every one of the user's own films but, as the run was specified (`--hist-n 1000`), for only the first 1,000 sample films. Most table films therefore have a displayed average and no plain mean, and a cut-point is only valid for the crowd it was calibrated on. Mixing sources inside one grade is what the spec forbids; using the plain mean alone would leave most other users with no eligible films.
- **Evidence:**
  - Coverage by construction: histograms for the author's own films and the first 1,000 sample films (about 15% of a 9,814-film table); displayed average for every film.
  - Agreement on the 917 films that have both: displayed minus plain mean averages −0.020 stars, RMSE 0.084, largest difference 0.51 (the spec measured RMSE 0.056 on 15 films).
  - The cut-points moved with the baseline, as the spec expected: pooled users' mean offset to the displayed average is about −0.05 stars, so the Power cuts sit higher than the spec's pool-scale ones.
- **Cost:** the displayed average is Letterboxd's weighted score, which discounts unusual rating patterns, so it is slightly less exact than the plain mean. The histograms stay in the film table so the baseline can be switched back once they cover most of it (`crowd` in `stats.rs`, then recalibrate with `tools/calibrate.py --source hist`).

## Calibration (what the card letters rest on)

- **Power, Precision, Range, Potential** are cut on the pooled Letterboxd users restricted to the films in our table (`tools/calibrate.py`, joined by film slug, output `tools/calibration.json`, merged by `tools/build_tables.py`). Power: shrunk offset against the displayed average, users with at least 50 eligible films. Precision: the difficulty curve h(gap) and the population accuracy are refitted on the same users. Range: 20 draws of 50 films with fractional countries (the spec's procedure; users with at least 50 films). Potential: see deviation 1.
- **Coverage caveat.** Pool users contribute only the films that are in our table, which are popular films. The product computes Range and Potential on every film it knows, including ones the TMDB proxy adds, so a user with many obscure films can score a little higher than the pool would have predicted.
- **Percentiles** (used by the ability engine to pick the most extreme stat) interpolate the probit of 13 population quantiles (1st to 99th) against the value and extend the end segments linearly.
- **Speed and Stamina** cuts are recomputed from `mt_user_metrics_N8.csv` (users with at least 20 clean events and 12 months); the four spec cut-points are reproduced exactly. They stay provisional until real dated Letterboxd exports exist.
- **Symbols:** ∞ for Speed needs pace ≥ 26.2 with 24 months and 40 events; for Stamina, 60 months all active; for Range, the 99.9th percentile of the calibration users with 150 films. 完成: see deviation 1.
- **"Crowd missing for more than half the rated films" gives `?`** for Power and Precision. A film needs at least 30,000 Letterboxd ratings to count.

## Behaviour the spec left open

- **Join day** comes from `profile.csv` (`Date Joined`) when present, else the earliest `watched.csv` date. **Export date** comes from the ZIP file name, else the latest date in the export.
- **Type ∅ or ?** on a stat a rule reads skips that rule. A user with all-∅ stats (an empty export) matches the canon Stand whose cells are all ∅ (Gold Experience Requiem).
- **Matching** with unknown user stats rescales by 6/k and needs 4 known stats. Twins (identical charts) are split by FNV-1a of the lower-cased username; the top 3 are distinct charts.
- **Example film per stat** (the second line of each card cell): Power, the film furthest from the crowd in your direction; Speed, your highest-rated film of the busiest month; Range, your highest-rated film from your least common country; Stamina, your highest-rated film of the last month of your longest streak; Precision, the most-rated film you scored within 0.25 of the crowd; Potential, your highest-rated film by a director you saw only once.
- **Range draws** use a fixed SplitMix64 stream (seed 5), 20 draws of 50 films, so results are reproducible in Rust, Python and across platforms. Fewer than 50 films with countries gives `?`.
- **Shorts** (runtime of 40 minutes or less) are ignored for countries, directors and composers; TV entries are dropped by the table builder.
- **Names (accepted for v1).** Only the composer route (top composer by shrunk mean, at least two rated films, canon-collision filtered) and the Part 3 colour plus Tarot route exist offline. The music-title route (stages 4 to 8) needs tracklists from MusicBrainz or Discogs, which cannot be shipped and are not fetched. No katakana reading is produced.
- **Unclassified** is written 未分類 on the card (the spec names the type but gives no Japanese label).

## Privacy

- `profile.csv` is read through an allowlist (`Date Joined`, `Username`); the other columns, the e-mail address included, are skipped as raw bytes. The committed fixtures use a synthetic `profile.csv` with an invalid address.
- The real export was run without `profile.csv`; the join day was passed as `--joined 2022-07-06` (from the spec) and the username came from the file name.
- The web page sends only the title and year of films missing from the table, and only when the checkbox is ticked. Ratings, dates and the ZIP stay in the browser.
