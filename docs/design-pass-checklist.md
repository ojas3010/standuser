# Design pass: checklist against docs/DESIGN.md

Measured with `python tools/site_test/design_check.py` (Playwright, headless Chromium, the synthetic zips in local/review/zips10 only) and by eye on the screenshots. Screenshots are in `local/review/design/before` and `local/review/design/after` (not committed: 22 full-page PNGs per set: desktop 1280 px and phone 390 px, one idle page and ten dropped zips per width; regenerate with `tools/site_test/screenshots.py <out_dir>`).

## Measured

| Rule or ban | Result | Evidence |
|---|---|---|
| Self-host: no request outside the site's own files at load | pass | hosts: ['127.0.0.1:36999'] (the local server stands for standuser.pages.dev) |
| 0 requests after the drop | pass | 0 network requests (blob: card images are in-memory) |
| Fonts self-hosted: Overpass loads from fonts/ | pass | ['Overpass loaded'] |
| CSP allows only the site (no font or script host) | pass | default-src 'self'; no absolute URL in style.css or app.js |
| R1 chrome is quiet: no JoJo identity (no kana, kanji, 【】) in page text | pass | 0 such characters |
| R7 ban: no ゴゴゴ, To Be Continued arrow or Menacing overlay in the chrome | pass | none in text or markup |
| R3 ground is warm, not neutral grey (light) | pass | rgb(236, 231, 220) |
| R3 ban: no gradients | pass | background-image none on every element; no gradient in CSS |
| R3 ban: no shadows or glows | pass | box-shadow, text-shadow, filter all none on every element |
| R3 ground is warm in dark too | pass | rgb(29, 27, 23) |
| R5 interface sans is Overpass; no serif in the chrome | pass | body font-family: Overpass, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif |
| R5 the wordmark has one tilt; nothing else tilts | pass | 1 transformed element(s): ['H1'] |
| R6 at most one diagonal | pass | 1 rotated element(s) at rest |
| R6 order: stage (drop zone), then subject (card), then method (table) | pass | tops [204, 627, 1656] |
| R6 right angles: boxes have no rounded corners | pass | border-radius 0 on buttons, drop zone, inputs, card |
| R6 single column on mobile; no sideways scroll at 390 px | pass | scrollWidth 390 for viewport 390; 1 distinct left edge(s) |
| S3 Download PNG is the one filled button; SVG and JSON are quiet text buttons of the same height and radius | pass | filled [True, False, False], heights [40, 40, 40], radii ['0px', '0px', '0px'] |
| S3 no bordered container around the card (no double frame) | pass | img border 0px, parent border 0px |
| S3 TMDB disclaimer is on the site | pass | site footer |
| R2 one accent per user, from the card's signature stat | pass | 10 users give accents ['DEV', 'PRC', 'PWR'] |
| R2 accent passes WCAG AA (4.5:1) on the ground, and its button label on it, light and dark | pass | lowest ratio over 20 renders: 4.75 |
| R4 colour event: the card lands, holds, then one palette shift | pass | accent unset while the card lands (land 0.2s ease-out); one shift 1150 ms after the drop |
| R4 the colour event is off under prefers-reduced-motion | pass | accent at once (PWR), animation none |
| R8 all motion under 300 ms | pass | longest transition or animation 250 ms |
| R8 card reveal: opacity 0 and scale 0.98 to 1, 200 ms, ease-out | pass | keyframes land and #card.land in style.css |
| R8 buttons: scale(0.97) on :active in about 120 ms | pass | style.css |
| R8 drag-over state is instant | pass | #drop transition-duration ['0s', '0s'] |
| R8 ban: no hover lifts | pass | no transform, translate or shadow in any :hover rule |
| R8 ban: no scroll fades | pass | no scroll listener, observer or scroll-driven animation |
| R8 ban: no animated radar | pass | card SVG has no animate, set or keyframes |
| R9 ban: no emoji or icon sprinkles | pass | 0 found |
| R9 ban: no gradient text | pass | no background-clip:text, no gradient |
| R9 ban: no uniform rounded cards with shadows | pass | no shadow, no radius on boxes |
| R9 ban: no "unleash", "seamless", "powered by AI" copy | pass | none in text or markup |
| R9 ban: no centred hero with three feature cards | pass | header left-aligned; no card grid |
| R9 copy uses real numbers | pass | 9,787 films (films.json) and 34,523 users (calibration.json) both in the lede |

36 of 36 pass

## Checked by eye (screenshots desktop-0, phone-3, dark phone-7, after)

| Rule or item | Result | Evidence |
|---|---|---|
| S3 why line has its own rows near the signature film, at body size, in full | pass | up to 3 rows at 20 px (the signature line's size), no "…"; tests: the 10 synthetic users, and the longest why `names.json` can produce (pool.rs); the lower half is laid out from the bottom (`svg::layout`) and a test covers every 1 to 3 ability, 1 to 2 weakness and 1 to 3 why rows |
| S3 PROVISIONAL DATA badge is not red | pass | outline and text in the muted ink; test `the_badge_is_not_red` |
| S3 type mix: serif only for the Stand name and the 【】 labels | pass | evidence lines, signature, ability box in sans; serif.ttf holds only Basic Latin, Latin-1, punctuation and the brackets (35 KB) |
| S3 footer readable, one credit line, TMDB disclaimer on the site | pass | one line at 16 px (was 12.5 px, three clauses); disclaimer in the site footer |
| S3 glyphs: 40 artist credits no longer draw boxes, names not folded or dropped | pass | test `every_artist_credit_in_names_json_is_drawable`; rendered 久石譲, 달파란 & 장영규, 谭盾, Ευανθία, Bregović, Milićević |
| S3 grade C legend (人間並 vs 人間と同じ) | skipped | by instruction: to be checked against a Part 5 volume |
| R6 centre kept clear, one sculpted object, right angles | pass | the card is the only image; no ornament on the page; every box is square |
| S3 chrome: the privacy line is in the drop zone | pass | names watched.csv, ratings.csv, diary.csv and the two columns of profile.csv (Date Joined, Favorite Films); asserted in `tools/site_test/selftest.js` |
| R2 accent visibly shifts once at the reveal | pass | wordmark, Download PNG and the table rule change from ink to the accent together |

Notes: a why line longer than 3 rows (about 5% of the worst-case combinations: a long film title quoted twice, a long artist, a medley's whole track list as the source) drops its rating, then the quoted source, then the film title in the link, then the artist, in that order, until it fits; only a line that is still too long would be cut with "…", and none in `names.json` is. The copy uses the real numbers, 9,787 films and 34,523 users, not the example figures in DESIGN.md (9,842 and 34,525). The card's ability box uses the one bold weight of the sans, since only weight 700 is embedded.

## Section 4: card palette pass

Screenshots (not committed, `local/review/card-palette`): `before/` is the page before this section and `after/` the page after, each with 22 full-page PNGs (1280 px and 390 px: one idle page and the ten synthetic zips of `local/review/zips10`; synthetic3, "PROMISING INFECTION", is the longest Stand name among them). `after/*-longest-name.png` also shows the widest name in `names.json`, "YORKTOWN AND THE RETURN HOME", on the page at both widths (the card is swapped in from `card_palettes`, since no zip produces it). Contrast: `python3 -I tools/site_test/card_contrast.py` (reads the card's own `<style>` and every outlined text path of the SVGs the renderer writes, 6 accents and the no-signature card, light and dark).

| Rule | Result | Evidence |
|---|---|---|
| 1 Card takes the page's palette | pass | `palette.rs` repeats `site/accent.js` and the test `accents_are_the_pages` reads that file and fails on any difference; the accent comes from the same signature rule as `accentKey` (`accent_key`); the page's purple on synthetic3 and the card's purple match |
| 2 Ground muted, warm, brick; one value per card theme | pass | light `#f0e1d7` (pale clay), dark `#4f281f` (deep brick brown). The light value is paler than a full brick tone on purpose: with `#e8d5c8` four of the six light accents fail AA (SPD 4.27, RNG 4.39, STA 4.10, DEV 4.12), and the dark ground at `#6c3125` fails PWR (3.79) and PRC (4.30); the card must reuse the page's accent values, so the ground gives way |
| 3 Radar fill, ring and grade letters from the user's pair; gold ring gone | pass | ring, ticks, inner frame, grade letters, 能力 label: the accent; radar fill: the complement (accent of the opposite axis, drawn in its brighter value on both cards so the scale letters over it stay readable); test `the_card_takes_the_users_pair` (no `gold`, no red) |
| 4 Flat fills only; no dot texture; no gradients, glows, shadows | pass | pattern removed, radar is a `<polygon>` with one fill and one opacity; test `the_name_is_the_one_tilt_and_the_fills_are_flat` bans Gradient, pattern, filter, shadow, mask, blur in every accent and theme |
| 5 One decisive tilt on the Stand name, the only diagonal, never clipped | pass | name at -5 degrees; exactly one `rotate(` in the SVG for all 7 accent cases and 3 themes; the stat labels (were at plus and minus 60 degrees) and the ゴゴゴ (were at plus and minus 6 degrees) are level now, the four side labels stand outside the medallion, the badge moved beside the note rows. The name is the largest size from 80 to 34 px whose tilted box stays in the header; test `every_name_in_names_json_fits_the_tilted_header_uncut` covers all 28,137 names (none cut, none below 34 px; the widest is drawn at 40 px) |
| 6 ゴゴゴ stays, in the palette colour at low contrast | pass | accent at 0.22 opacity, level; 1.34 to 1.63 against the ground |
| 7 Every text colour passes WCAG AA on the card ground, 12 accents, both card themes | pass | `card_contrast.py`: lowest accent-on-ground 4.56 (STA, light), ink 5.01 (dark, over the radar fill), mute 7.05; also checked on the ability box's paper and for the scale letters over the radar fill. Accents that fail: none on the final grounds (the script reports 7 failures on the `#e8d5c8` and `#6c3125` grounds, so it does catch them). The ゴゴゴ are exempt: rule 6 asks for low contrast |

Other checks run after this section: `cargo test --workspace` (all pass; the author-export golden is `ignored` as before), the naming tests (`python3 -I -m pytest tools/naming`, 94 passed), `tools/site_test/run.sh` (headless Chrome, "requests while processing: 0"), `design_check.py` (36 of 36 pass, "0 requests after the drop").
