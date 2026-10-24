# Decisions made while porting the spec

The Python prototypes and reports under `swarm/`, `reports/` and `research_notes/` were the specification. Where they were silent, contradictory or needed data that is not on the build machine, these are the choices made. Each is easy to change; the file that holds it is named.

## Stack

- **TMDB proxy runs on Vercel, not Cloudflare.** HANDOFF.md named a Cloudflare Worker; the v1 request named a Vercel function. `api/tmdb.js`.
- **Text is outlined, not embedded.** The card draws every glyph as a path from Noto Sans/Serif JP Bold subsets, so there is no `<text>` and no font-loading problem. The cost is a larger SVG (about 650 KB) and no support for scripts outside the subset (Latin, Greek, Cyrillic basics, kana and the card's own kanji); other characters become □. `crates/standuser-core/src/font.rs`, `tools/make_fonts.py`.
- **`gen_card.py` line 121 bug.** In embed mode the text lost the `f-` prefix on its class and rendered black. The Rust renderer has only the outlined mode and always emits `class="f-{cls}"`; a unit test fails if any bare class appears.
- **The site table ships with the page** (`site/films.json`) and the CLI embeds the same file. Films outside it can be filled in through the proxy, which returns the same JSON format.

## Calibration (what the card letters rest on)

- **Power and Precision** use the spec's measured cuts (pool of 24,929 users). The crowd is now the Letterboxd plain mean from the rating histogram, which runs higher than the pool's crowd (the spec expected about +0.06 stars). The cuts were not recomputed because the pooled user ratings are not on this machine. Effect: borderline Power letters can move by one grade after recalibration.
- **Percentiles** (used by the ability engine to pick the most extreme stat) interpolate the probit of the known quantiles against the value and extend the end segments linearly. Only a handful of quantiles survive in the reports for Power and Precision, so tail percentiles such as "97.8th" are approximations.
- **Speed and Stamina** cuts are recomputed from `mt_user_metrics_N8.csv` (users with at least 20 clean events and 12 months); the four spec cut-points are reproduced exactly. They stay provisional.
- **Range** uses the TMDB-5000 pool quantiles. They come from a US-heavy, 19%-coverage subset, so a user whose every film has country data gets a higher effective country count than the pool did. The spec marks these cuts "not valid"; they are used here because the type rule needs a Range letter, and the card marks it provisional. This is the weakest letter on the card.
- **Potential has no cut-points**, so it is graded `?` with its raw value shown, and the ability engine never picks it. The 完成 symbol therefore never appears in v1.
- **Symbols:** ∞ for Speed needs pace ≥ 26.2 with 24 months and 40 events; for Stamina, 60 months all active; for Range, D ≥ 5.7 with 150 films (the spec's own, unverified thresholds).
- **Crowd source rule.** The spec says one source per grade. The crowd is the histogram plain mean when that gives at least as many eligible films as the displayed average, else the displayed average. A film needs at least 30,000 Letterboxd ratings to count.
- **"Crowd missing for more than half the rated films" gives `?`** for Power and Precision. With a table of about 3,500 films this is the common case for users whose taste is far from popular films.

## Behaviour the spec left open

- **Join day** comes from `profile.csv` (`Date Joined`) when present, else the earliest `watched.csv` date. **Export date** comes from the ZIP file name, else the latest date in the export.
- **Type ∅ or ?** on a stat a rule reads skips that rule. A user with all-∅ stats (an empty export) matches the canon Stand whose cells are all ∅ (Gold Experience Requiem).
- **Matching** with unknown user stats rescales by 6/k and needs 4 known stats. Twins (identical charts) are split by FNV-1a of the lower-cased username; the top 3 are distinct charts.
- **Example film per stat** (the second line of each card cell): Power, the film furthest from the crowd in your direction; Speed, your highest-rated film of the busiest month; Range, your highest-rated film from your least common country; Stamina, your highest-rated film of the last month of your longest streak; Precision, the most-rated film you scored within 0.25 of the crowd; Potential, your highest-rated film by a director you saw only once.
- **Range draws** use a fixed SplitMix64 stream (seed 5), 20 draws of 50 films, so results are reproducible in Rust, Python and across platforms. Fewer than 50 films with countries gives `?`.
- **Shorts** (runtime of 40 minutes or less) are ignored for countries, directors and composers; TV entries are dropped by the table builder.
- **Names.** Only the composer route (top composer by shrunk mean, at least two rated films, canon-collision filtered) and the Part 3 colour plus Tarot route exist offline. The music-title route (stages 4 to 8) needs tracklists from MusicBrainz or Discogs, which cannot be shipped and are not fetched. No katakana reading is produced.
- **Unclassified** is written 未分類 on the card (the spec names the type but gives no Japanese label).

## Privacy

- `profile.csv` is read through an allowlist (`Date Joined`, `Username`); the other columns, the e-mail address included, are skipped as raw bytes. The committed fixtures use a synthetic `profile.csv` with an invalid address.
- The real export was run without `profile.csv`; the join day was passed as `--joined 2022-07-06` (from the spec) and the username came from the file name.
- The web page sends only the title and year of films missing from the table, and only when the checkbox is ticked. Ratings, dates and the ZIP stay in the browser.
