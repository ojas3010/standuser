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
