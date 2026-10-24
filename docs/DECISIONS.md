# Decisions made while porting the spec

The Python prototypes and reports under `swarm/`, `reports/` and `research_notes/` were the specification. Where they were silent, contradictory or needed data that is not on the build machine, these are the choices made. Each is easy to change; the file that holds it is named.

## Stack

- **Static site on Cloudflare Pages, no server.** HANDOFF.md planned a Cloudflare Worker proxying TMDB, and an intermediate plan a Vercel function; both were dropped. The site is the committed `site/` folder (no build step in the dashboard, `_headers` for the content security policy), and nothing in the page or the CLI makes a network request while an export is processed. The export never leaves the browser.
- **Text is outlined, not embedded.** The card draws every glyph as a path from Noto Sans/Serif JP Bold subsets, so there is no `<text>` and no font-loading problem. The cost is a larger SVG (about 650 KB) and no support for scripts outside the subset (Latin, Greek, Cyrillic basics, kana and the card's own kanji); other characters become □. `crates/standuser-core/src/font.rs`, `tools/make_fonts.py`.
- **`gen_card.py` line 121 bug.** In embed mode the text lost the `f-` prefix on its class and rendered black. The Rust renderer has only the outlined mode and always emits `class="f-{cls}"`; a unit test fails if any bare class appears.
- **The film table ships with the page** (`site/films.json`) and the CLI embeds the same file. Films outside it are skipped by every stat that needs film data (Power, Precision, Range, Potential); Speed and Stamina read only the viewing timeline, so they count every watched film that gave a clean event; their coverage reads "241 of 533 dated watches" (bulk-import days excluded) so it is not mistaken for missing table data. Each stat reports "graded on N of M films" on the card and in `stand.json` (`coverage.per_stat`), and shows `?` when N is below the spec's minimum for that stat. The table carries its build date, and the CLI and page warn once it is older than six months, the longest TMDB's terms allow data to be cached.

## Deviations from the spec

Two choices contradict the written spec ("Letterboxd JoJo Stand stats 2"). Figures below come from `tools/evidence.py` run on the pooled Letterboxd users (sdl60660/letterboxd_recommendations) restricted to the films in the final table (9,847 movies with a Letterboxd slug, built 2026-10-24).

### 1. Potential is graded on volume-matched Good-Turing (60 random films), against users with a similar number of films

- **Spec:** Potential = N1 / director credits over all of a user's films, with no cut-points; the volume-matched value was only an evidence line.
- **Now:** the card letter comes from the mean of 20 seeded draws of 60 director-credited films (N1 / credits per draw), graded within film-count bands (director-credited films: 60-100, 100-200, 200+). Each band has its own cut-points, cut on the pooled users in that band; a band with under 1,000 pooled users merges into its neighbour (none did). A user with 40 to 59 credited films is drawn on all their films, graded against the first band and marked provisional. The raw share stays in the card text ("47% directors seen once"), and the note names the band ("against users with 200+ films"). The 完成 symbol needs 1,000 credited films and the lowest band of the user's band, as in the spec.
- **Reason:** the raw share has a mechanical dependence on how many films were seen, so one set of cut-points cannot be applied fairly to a 60-film and a 500-film history. Drawing the same number of films per user removes the mechanical part, and banding by film count removes most of the remaining real dependence (users with long histories watch more varied directors).
- **Evidence.** Spearman correlation of the graded value with film count, over the pooled users restricted to the final table (33,643 users with at least 60 director-credited films):

| | Spearman with film count |
|---|---|
| raw Good-Turing, one population | −0.39 |
| volume-matched (60 films), one population | +0.34 |
| volume-matched, within the 60-100 band | −0.054 |
| volume-matched, within the 100-200 band | +0.103 |
| volume-matched, within the 200+ band | +0.184 |

  Per-band cut-points (10th, 30th, 70th, 90th percentile):

| Films | Pooled users | Spearman with film count | Cut-points |
|---|---|---|---|
| 60-100 | 3,035 | -0.053 | 0.609, 0.731, 0.885, 0.945 |
| 100-200 | 5,515 | +0.103 | 0.675, 0.757, 0.858, 0.915 |
| 200+ | 25,093 | +0.184 | 0.798, 0.854, 0.908, 0.934 |

- **Provisional rule.** Potential stays provisional while any band shows |Spearman| above 0.2. None does (largest 0.184, the 200+ band), so users with 60 or more credited films get an unmarked Potential grade. The 200+ band runs from 200 to a few thousand films and is the closest to the line, so a longer history could still drift upward within it; `tools/calibrate.py` prints the figures again whenever the table is rebuilt, and the rule is applied from them (`any_band_over_0_2` in `cuts.json`).
- **Earlier result.** On the partial 920-film table the single-population volume-matched value looked flat against film count (−0.05); on the final table it is not (+0.34), which is why bands were added.
- **Cost:** a user is compared with others of a similar history length, not with everyone, so "C in the 200+ band" means middling among long histories. The canon-equivalent grade (used for the type and the nearest Stand) is cut within the band too, which can move a Stand match near a band's boundary: the author's Potential is 40.3rd percentile in the 200+ band, just under the canon boundary of 42.2, and the nearest Stand changed from D4C Love Train to Star Platinum (Part 6) at the same distance.

### 2. The crowd baseline is Letterboxd's displayed average, not the histogram plain mean

- **Spec:** the crowd is the plain mean from the rating histogram ("production uses the plain mean"), else the displayed average, else a bundled pool table, one source per grade.
- **Now:** every Power and Precision grade uses the displayed average (`lb_avg`), and the cut-points are calibrated on it.
- **Reason:** the collector reads a histogram for every one of the user's own films but, as the run was specified (`--hist-n 1000`), for only the first 1,000 sample films. Most table films therefore have a displayed average and no plain mean, and a cut-point is only valid for the crowd it was calibrated on. Mixing sources inside one grade is what the spec forbids; using the plain mean alone would leave most other users with no eligible films.
- **Evidence:**
  - Coverage: 1,465 of 9,847 table films (15%) have a histogram plain mean (the author's own films and the first 1,000 sample films); 9,839 (100%) have a displayed average.
  - Agreement on the 1,465 films that have both: displayed minus plain mean averages −0.015 stars, RMSE 0.085, largest difference 0.51 (the spec measured RMSE 0.056 on 15 films).
  - The cut-points moved with the baseline, as the spec expected: pooled users' mean offset to the displayed average is −0.045 stars, so the Power cuts sit higher than the spec's pool-scale ones (grade B starts at +0.20 instead of +0.14).
- **Cost:** the displayed average is Letterboxd's weighted score, which discounts unusual rating patterns, so it is slightly less exact than the plain mean. The histograms stay in the film table so the baseline can be switched back once they cover most of it (`crowd` in `stats.rs`, then recalibrate with `tools/calibrate.py --source hist`).

## Calibration (what the card letters rest on)

- **Power, Precision, Range, Potential** are cut on the pooled Letterboxd users restricted to the films in our table (`tools/calibrate.py`, joined by film slug, output `tools/calibration.json`, merged by `tools/build_tables.py`). Power: shrunk offset against the displayed average, users with at least 50 eligible films. Precision: the difficulty curve h(gap) and the population accuracy are refitted on the same users. Range: 20 draws of 50 films with fractional countries (the spec's procedure; users with at least 50 films). Potential: see deviation 1.
- **Final cut-points** (10th, 30th, 70th and 90th percentile of the pooled users): Power −0.367, −0.115, +0.200, +0.458 (34,525 users); Precision 0.654, 0.722, 0.796, 0.840; Range 2.14, 2.63, 3.74, 5.02 (34,721 users, median 3.10); Potential per film-count band, see deviation 1 (33,643 users; pooled fallback 0.749, 0.834, 0.903, 0.933). The Range cuts moved up sharply from the US-heavy proxy the spec used (median 2.47, cuts 1.9 to 3.3) once every pooled user's films have country data.
- **Table coverage** (`tools/coverage.py`, pooled users, TV removed). Share of a user's rated films found in the table: median 95.9%, 10th percentile 80.9% (users with at least 50 ratings; 35,718 users). Share also usable for Power and Precision (at least 30,000 Letterboxd ratings): median 93.2%, 10th percentile 72.7%. 1.2% of users would see `?` for Power and Precision because more than half of their rated films are missing. These figures are favourable by construction, since the table is the pool's own most-rated films; an independent check is the author's export, where 89.9% of watched films are in the sample part of the table alone (the author's own films were added regardless). A user with obscure taste will see lower coverage, and the card says how much.
- **Coverage caveat.** Pool users contribute only the films that are in our table, which are popular films, and the product likewise uses only table films for Range and Potential, so the two are on the same footing. A user whose taste is mostly outside the table is graded on fewer films, and that is shown on the card.
- **Percentiles** (used by the ability engine to pick the most extreme stat) interpolate the probit of 13 population quantiles (1st to 99th) against the value and extend the end segments linearly.
- **Speed and Stamina** cuts are recomputed from `mt_user_metrics_N8.csv` (users with at least 20 clean events and 12 months); the four spec cut-points are reproduced exactly. They stay provisional until real dated Letterboxd exports exist.
- **Symbols:** ∞ for Speed needs pace ≥ 26.2 with 24 months and 40 events; for Stamina, 60 months all active; for Range, the 99.9th percentile of the calibration users with 150 films. 完成: see deviation 1.
- **"Crowd missing for more than half the rated films" gives `?`** for Power and Precision. A film needs at least 30,000 Letterboxd ratings to count.

## Behaviour the spec left open

- **Join day** comes from the `Date Joined` column of `profile.csv`; only when the file or that column is missing (or the value is unreadable) is it the earliest `watched.csv` date, and `stand.json` says so in `notes`. **Export date** comes from the ZIP file name, else the latest date in the export.
- **Type ∅ or ?** on a stat a rule reads skips that rule. A user with all-∅ stats (an empty export) matches the canon Stand whose cells are all ∅ (Gold Experience Requiem).
- **Matching** with unknown user stats rescales by 6/k and needs 4 known stats. Ties between Stands at equal letter distance are broken by the continuous distance between the user's percentiles and the midpoint of each Stand's canon-equivalent percentile band (from canon's cumulative grade shares; ∅ at the bottom, ∞ at the top, 完成 in the lowest band; only stats where both have a value), then by a hash of the username and the Stand name, which also splits identical-chart twins. The top 3 are distinct charts. This replaces the earlier tie-break chain (salience-weighted error, largest single gap, exact matches, known stats, canon order). For the author's profile (canon-equivalent B A C A A D) D4C Love Train and Star Platinum (Part 6) were at the same letter distance (1.41) and equal on every earlier key, so canon order (wiki row 97 against 124) had put Star Platinum (Part 6) first; the continuous distance now puts D4C Love Train first (Potential at the 40th percentile is 0.16 from the C band's midpoint and 0.26 from the lowest band's).
- **Example film per stat** (the second line of each card cell): Power, the film furthest from the crowd in your direction; Speed, your highest-rated film of the busiest month; Range, your highest-rated film from your least common country; Stamina, your highest-rated film of the last month of your longest streak; Precision, the most-rated film you scored within 0.25 of the crowd; Potential, your highest-rated film by a director you saw only once.
- **Range draws** use a fixed SplitMix64 stream (seed 5), 20 draws of 50 films, so results are reproducible in Rust, Python and across platforms. Fewer than 50 films with countries gives `?`.
- **Shorts** (runtime of 40 minutes or less) are ignored for countries, directors and composers; TV entries are dropped by the table builder.
- **Names (accepted for v1).** Only the composer route (top composer by shrunk mean, at least two rated films, canon-collision filtered) and the Part 3 colour plus Tarot route exist offline. The music-title route (stages 4 to 8) needs tracklists from MusicBrainz or Discogs, which cannot be shipped and are not fetched. No katakana reading is produced.
- **Unclassified** is written 未分類 on the card (the spec names the type but gives no Japanese label).

## Privacy

- `profile.csv` is read through an allowlist of one column, `Date Joined`, in memory; every other column (the e-mail address and the username included) is never decoded, stored, shown or logged, and an unreadable `profile.csv` counts as absent. The username comes from the ZIP file name (or `--username`). The committed fixtures use a synthetic `profile.csv` with an invalid address.
- The real export was run without `profile.csv`; the join day was passed as `--joined 2022-07-06` (from the spec) and the username came from the file name.
- The export never leaves the browser, and the page makes no request after it has loaded: no proxy, no analytics, no fonts or images from other sites. A browser test asserts zero requests while an export is processed. The only outbound thing on the page is the TMDB logo link, which the visitor has to click.
