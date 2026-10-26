# Design rules

This file governs the design pass. Every rule and every ban below is a checkable line. Each one gets a before and after screenshot at desktop width and phone width (Playwright, headless, in WSL).

## 1. What Araki actually does (sourced)

Sources: emilkowal.ski (motion), the ui-skills Scandinavian design skill (restraint, tokens), and research on Araki's design language. Most Araki sources are JoJo Wiki fan translations of interviews. The text of his craft book was not obtained, so the site rules in section 2 are inference from his stated method.

- Colour is decided per image, not fixed per character. He colours "by calculation"; Giorno is pink on one cover and blue on another. Fills are flat and matte inside outlines, which he compares to ukiyo-e.
- Named preferences: light blue + pink is his "golden combination" (sky and cherry blossoms). Green "gives me power". Blue beside pink "gives more power".
- Cover designer Mitsuru Kobayashi lists Araki's signature colours as yellow, green, hot pink, orange and purple ("Araki Purple"). Designer Noriko Narumi notes that JoJo skies are often yellow, green or pink.
- Backgrounds stay muted so the figure carries the colour. His exhibition background was mixed from five colours, like European brick, so that it does not overwhelm viewers.
- Anime colour system: a base palette per season, plus special scene or cut colouring only at emotional peaks.
- Poses use torsion (Roman sculpture, Bernini's Apollo and Daphne, fashion magazines). A figure that only stands is boring. A pose is "the shape that stays in your mind" and reads only when exaggerated.
- Composition: figures on crossing axes guide the gaze. The centre of a spread stays free of detail. Panels are right-angled by default and tilt only for motion. White pages and black panels give drama. The environment comes before the subject. The first page must make the reader turn.
- SFX are part of the picture. Anime director Tsuda said manga-style ゴゴゴ in a tense scene risks looking like a joke; staff placed SFX only in empty frames.
- There is no single JoJo typeface; each Part logo is custom. Narumi: logos are clean and legible, with motion through tilt. The JOJOLands logo uses Interstate + A1 Gothic; JoJolion uses a textured Shueido Mincho.
- The official JOJO Portal is a plain news site: quiet interface, loud art.
- Stand parameters debuted in Part 5 as chapter tailpieces, with six parameters clockwise. Anime eyecatches with them start from Stardust Crusaders. Clichés: the Menacing ゴゴゴ overlay and the "To Be Continued" arrow meme.
- Gold: no source ties gold to Araki's illustration colour (only Bulgari merchandise). The card's gold ring is our choice, not canon.

## 2. Rules for the site

1. The card stays loud and the page chrome goes quiet. Only the card carries JoJo identity.
2. Colour per user: derive the page's one accent from the user's card, for example a complementary pair from Araki's palette keyed to the signature stat.
3. Ground: muted and slightly warm, not pure neutral grey. Flat fills only. Ban: gradients, glows, shadows.
4. One colour event at the reveal: the card lands, holds, then one palette shift. It is off under prefers-reduced-motion.
5. Type: the card's Mincho-style serif is for display only. Interface sans is Overpass, replacing Inter. The wordmark gets one decisive tilt; nothing else tilts.
6. Layout order: stage first (drop zone), then subject (card), then method. One sculpted object, centre kept clear, right angles, at most one diagonal. Single column on mobile.
7. Ban in the chrome: ゴゴゴ (card only), the To Be Continued arrow, the Menacing overlay.
8. Motion: under 300 ms, ease-out for entrances. Card reveal: opacity 0 and scale 0.98 to 1 in about 200 ms. Buttons: scale(0.97) on :active in about 120 ms. Drag-over state is instant. Ban: hover lifts, scroll fades, animated radar.
9. Anti-slop bans: emoji or icon sprinkles, gradient text, uniform rounded cards with shadows, "unleash", "seamless" and "powered by AI" copy, a centred hero with three feature cards. Copy uses real numbers (9,842 films, 34,525 users).
10. Every rule and every ban in this section gets a before and after screenshot at desktop width and phone width.

## 3. Card-level fixes

- Why line: give it its own row near the signature film, at body size. It is now about 11 px, squeezed between the name and 本体.
- PROVISIONAL DATA badge: it is red like the grades. Make it neutral or drop it; the footer asterisk already covers it.
- Type mix: serif only for the Stand name and the 【】 labels. The card's sans for evidence lines and the ability box.
- Footer: about 9 px is unreadable on a phone PNG. Keep one credit line on the card and move the TMDB disclaimer to the site.
- Glyphs: 40 artist credits draw boxes because the card font subset lacks their glyphs (mostly Japanese, Korean and Chinese names, plus a few Latin Extended letters such as ć and ş). Add those glyphs to the subset. Do not fold or drop the names.
- Page chrome: Download PNG is the one filled button. SVG and JSON are quiet text buttons with the same height and radius. Remove the bordered container around the card (double frame).
- Grade C legend: the card says 人間並, JoJo Wiki gives 人間と同じ. Check a Part 5 volume before the AAT.

## 4. Card palette and lettering pass

Second pass, from the reference covers and pages in local/ref (git-ignored; never copied into site/, docs/ or any committed path). It replaces the muted brick ground of the first pass: Araki's brick-mixed background was an exhibition choice, the covers and Stand pages are flat saturated colour.

1. The card takes the same per-user palette as the page: the key comes from the signature stat (site/accent.js mapping), and each key has its cover colour (Power hot pink, Speed yellow, Range blue, Stamina green, Precision purple, Potential orange).
2. Ground: one flat saturated colour sampled from the covers, per key and per card theme. Never brown, brick or near-black. Every ground and radar fill is a bin colour that `tools/ref_palette.py` sampled from a named cover; `docs/ref-palette.json` lists the image of each, and a test in palette.rs checks the table against it.
3. Radar fill and ゴゴゴ: a second sampled cover colour (the complement of the ground). Ring, ticks and the accent text colour are gone: text is the ink of the ground (black or white, whichever reads better) and black on white paper.
4. Flat fills only. Black outlines are the manga's and are allowed (a stroked black path under the fill path); gradients, glows, shadows, patterns and opacity stay banned.
5. The Stand name takes one tilt, like a Part logo. It is the card's one rotated element. The name must not clip at any length in names.json.
6. ゴゴゴ: Dela Gothic One, heavy, in the second colour with the black outline, level, each smaller than the one above (as in the SFX reference).
7. Lettering: Dela Gothic One (SIL OFL) for katakana, the stat labels, the legend, the type names and the labels; Archivo Black (SIL OFL) for the Stand name, chosen against the Part 3, 5 and 6 logos (squared, wide, very heavy); Noto Sans JP Bold for body text. All are embedded as outlines in the SVG; the licence texts sit in crates/standuser-core/data.
8. Chart: the Part 5 stat chart: a hexagon with a ring per grade, circled A to E up the top axis, a circled grade at each vertex, and the labels in the Part 5 positions (破壊力 top, スピード upper right, 射程距離 lower right, 持続力 bottom, 精密動作性 lower left, 成長性 upper left).
9. Every data line stays on the card.
10. Contrast (WCAG AA, `tools/site_test/card_contrast.py` on the real SVGs, 6 accents and none, light and dark): text on the ground and on the paper 4.5:1; the outlined Stand name against its black outline 4.5:1; the ゴゴゴ, large decoration, 3:1 against their outline. For the outlined lettering the outline is what the glyphs sit against, so that is the measured pair.
