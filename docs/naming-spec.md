# Name each Stand from a loved soundtrack

The best algorithm builds a small **personal pool** of soundtrack titles from the films a user loves most, strips out generic cues and anything that breaks canon's shape, and picks the title that best **fits the Stand's ability** (the template keyed by its top stat). Affinity and canon style only reorder near-ties. If no title survives, it falls back to the composer's name, then to Part 3's Tarot card plus colour. This copies how Araki works. He draws names from "groups and album names I liked" ([Comnavi](https://jojowiki.com/Interview:Comnavi_Vol._3_%28February_1998%29)), then chooses one that fits ("a word that fit soap bubbles" for Soft & Wet, [Billboard JAPAN](https://www.billboard-japan.com/special/detail/3283)). Canon numbers set the shape rules: **96% of Stands from Part 4 on are music names, 68% are used verbatim, and 93% have 1 to 3 words**. Data is available for most films, but no single source has it. A tracklist turned up for **11 of 12 of Ojas's test films, including all 5 Asian ones**, but 9 of those came from Discogs and 2 from Wikipedia. MusicBrainz, the one source with an exact film link, could not be read this session. A canon benchmark of **63 Stands named after songs or albums** can show whether the method beats random choice and the v0 "first track" rule. It is too small to rank close rivals: a 0.20 difference in top-1 accuracy needs about 57 Stands. Run on Ojas's favourites, the algorithm gives **「DIRTY WEATHER」**, a cue from Mark Korven's *The Lighthouse* score, matched to his "Air Reading" ability. Treat that as illustrative: the encoder was not run and one favourite's tracklist was not read.

Tags used below: **[computed]** = calculated this session from local files, **[illustrative]** = hand-applied, not measured, **[unverified]** = not confirmed from a source.

## The algorithm runs in nine deterministic stages

The algorithm runs offline once its fetches are cached. It uses only the export plus public film and music data, and every output carries a provenance line.

1. **Inputs and guards.** Read ratings.csv, watched.csv and diary.csv. Read profile.csv only through `usecols=["Date Joined","Username","Favorite Films"]`, so the email column is never parsed. Resolve each film to a TMDB `/movie` ID and an IMDb ID, then drop `/tv` entries and anything with runtime ≤ 40 minutes. Take the **signature stat** from the stats layer: the stat with the largest |z|, ties broken in canon order PWR, SPD, RNG, STA, PRC, DEV, and BALANCED if max |z| < 0.524. That stat selects one of the 17 ability templates *t*. Each template has a hand-written gloss g_t and a core lexicon of 10 to 30 words taken from its canon Stand's ability text. No LLM is used to build them.
2. **Mode.** Music mode is the default. Part 3 mode (stage 9c) runs when the user switches it on or when stages 3 to 7 return nothing. Egyptian-god names are never used.
3. **Personal pool.** P = the user's Favorite Films (up to 4), plus the highest-rated film of each of the top 3 composers. Composers are ranked by shrunk mean m_c = (Σr + 3μ_u)/(n_c + 3) with n_c ≥ 2. Ties go to a favourite, then the larger own-minus-crowd gap, then the higher crowd rating, then the lower TMDB ID. If |P| < 5, add the user's remaining top films by own-minus-crowd gap until |P| = 5. Cap |P| at 8.
   - Composer credits come from TMDB "Original Music Composer", else "Music", else Letterboxd "Composer". A film may have several composers.
   - Unrated users: favourites plus the films of their most frequent composers with the highest crowd rating.
   - Year cards: films in the window rated at or above the user's 80th percentile.
   - Affinity per film: **A(f) = 0.5·fav + 0.3·q_r + 0.2·q_c**, where fav = 1 for a Favorite Film, q_r is the mid-rank percentile of the user's rating among their own ratings, and q_c is the percentile of the film's best m_c among the user's composers (0.5 if none).
4. **Music per film.** Run a cascade per film and cache results by TMDB ID:
   - (a) MusicBrainz URL lookup on the IMDb page, `GET /ws/2/url?resource=https://www.imdb.com/title/{tt}/&resource=http://www.imdb.com/title/{tt}/&inc=release-group-rels&fmt=json`. Keep release groups that have a Soundtrack secondary type, then browse their releases with `inc=recordings&limit=100`.
   - (b) Discogs `/database/search` with `type=master`, `genre=Stage & Screen`, `release_title=<film>`, plus `artist=<composer>` when one is known. Accept a master whose normalised title contains the film title and whose year is no earlier than the film's year minus 1.
   - (c) The Wikipedia "(soundtrack)" article or the film article's track listing.
   - (d) Optionally, at the user's request, the film's Tunefind page for songs used in the film.
   - Keep at most two albums per film: the one whose artist credit contains a credited composer (the score) and a songs album.
5. **Candidates.** Normalise every track title with NFKD.
   - For bilingual "native = English" titles keep the Latin side, and drop titles that are CJK only.
   - Strip "(...)", "[...]" and anything after " - " or ": ". Split cue-sheet merges on " / " into separate candidates that keep their track number.
   - A remainder of ≤ 5 words and ≤ 28 characters is a **verbatim** candidate.
   - Otherwise make one **truncated** candidate: the shortest 2- or 3-word tail span that does not start with a function word (article, preposition, pronoun, "another", "one") and whose last token is tagged as a noun when the full title is POS-tagged.
   - Add each needle-drop artist and each credited composer as an **artist** candidate.
   - The optional "Araki twist" (off by default) swaps from a closed list (Mother↔Father, Queen↔King, Boy↔Girl, Band↔Hand, Rock↔Lock). It applies only when the new word is in t's core lexicon.
6. **Hard filters.** Reject a candidate if any of these hold:
   - It matches `(?i)^(main|end|opening|closing|title|love|final|finale|intro|outro)( titles?| credits| theme| music)?$`.
   - It contains suite, overture, prologue, epilogue, interlude, reprise, medley, variation(s), trailer, untitled, cue, "theme from" or "contains ... theme".
   - It ends in a 1- or 2-digit number, a roman numeral or "part N", or it matches `^\d+m\d+` or `^track \d+$`.
   - It starts with an all-caps token of 3 or more letters while the rest of the title is not all caps.
   - It contains the normalised film title or subtitle.
   - Fewer than 80% of its non-space, non-punctuation characters are ASCII letters or digits.
   - It matches a word on the LDNOOBW English list or a hand list, as a whole word.
   - It collides with a canon name. Compare after NFKD, case-folding, punctuation removal, leading-"the" removal and singularising the last word. Reject an exact match, or edit distance 1 when the name has 8 or more characters. Shorter near-misses are kept and noted in the why line.
   - It was already issued to this user.
   - Exception: "X's Theme" passes when X is a proper noun other than the film title.
7. **Score** every survivor n with source film f(n):

```
S(n) = F(n) + 0.20·A(f(n)) + 0.10·Y(n) + 0.05·M(n)

F(n) = 0.5·L(n) + 0.5·E(n)        fit to template t, in [0,1]
  L = 1 if a content-word lemma of n is in t's core lexicon;
      0.5 if it is in t's one-hop WordNet ring; else 0
  E = min-max over the pool of cos(enc(n), enc(g_t)), with a pinned
      bge-small-en-v1.5 or all-MiniLM-L6-v2, rounded to 4 dp
      (lexicon-only mode: F = L)
Y(n) = 0.6·W(words) + 0.4·T(type) canon shape, in [0,1]
  W = {1: 0.49, 2: 1.00, 3: 0.34, 4: 0.08, 5: 0.04}  (canon word-count frequency / mode)
  T = 1.0 song or cue title; 0.6 artist, composer or "X's Theme"
M(n) = 1 if n contains platinum, diamond, gold, silver, stone, pearl, crystal, iron or steel
```

   The non-fit terms add at most 0.35, so they can only reorder names whose fit is within 0.35 of the best. They cannot rescue a poor fit. The weights are v1 defaults. Tune F on the canon benchmark and the affinity and style weights by human A/B, never on canon.

8. **Select and render.**
   - Round S to 4 dp, then sort by S descending, A descending, length ascending, then normalised name ascending.
   - If S₁ − S₂ < 0.03 and the optional LLM tie-break is on, ask for a pairwise verdict on (g_t, n₁, n₂) in **both orders**. Accept it only if the two orders agree, and cache it under (prompt version, model id, template, sorted pair). Otherwise keep the deterministic order.
   - Show the top 3 so the user can switch.
   - Render the English name in upper case inside 「」, keeping ? ! & and digits. Add a katakana reading built by lookup, then alkana, then CMUdict+e2k P2K, then e2k C2K, with words joined by ・. Label the reading as auto-generated and editable.
   - Fill the one-line why from computed slots only: `Why 「{NAME}」: {verbatim | tail of | one cue of} "{source title}", track {k} of {artist}'s {film} {score|soundtrack} ({year}); {film} is {one of your Favourite Films | your top film by {composer}} ({r}★). "{matched word}" fits your {ability} ability; fit rank {i} of {N}.` Reject any rendering that contains a number or proper noun not in the slot dictionary. Assert that the cited reason is the largest term in S.
9. **Fallbacks.**
   - (a) If fewer than 3 candidates survive, extend P to 8 films and rerun stages 4 to 8.
   - (b) If still none survive, use the anchor composer's name (Route A).
   - (c) With no composer credits at all, use Part 3 mode: `{colour of the 2nd-ranked stat extreme} {card of t}` without "The".
     - Colours, high/low: PWR Red/Gray, SPD Silver/Ebony, RNG Green/Yellow, STA Black/Crimson, PRC Platinum/White, DEV Gold/Purple.
     - Cards for templates 1 to 17: Tower, Temperance, Chariot, World, Hanged Man, Hierophant, Sun, Strength, Wheel of Fortune, Death, High Priestess, Justice, Moon, Empress, Hermit, Star, Fool.
     - Tarot collisions such as YELLOW TEMPERANCE are kept as flagged easter eggs.
   - (d) With no signature stat, show 「???」.

## Canon supplies the pool rule, the fit rule and the shape rules

Canon names resonate in two ways. Some echo the **ability**: Heaven's Door "turns people into books for Rohan to read" ([Heaven's Door](https://jojowiki.com/Heaven's_Door)), and Araki took Sticky Fingers' zipper from the album cover ([Sticky Fingers](https://jojowiki.com/Sticky_Fingers)). Others echo the **user**: Killer Queen's user is a killer, Harvest's user Yangu matches Neil Young, and Atom Heart Father's user is a father ([List of References](https://jojowiki.com/List_of_References_in_JoJo)). The algorithm handles the user link through structure, since every candidate comes from the user's own films and the why line names that film. It scores the ability link directly. The table maps each rule to its evidence. Corpus figures are over 144 base rows of the 157-row canon table (canon_name_sources.csv, from the research session; not included here). The ability-link codes come from a single coder and are **[unverified]**.

| Rule | Canon evidence |
|---|---|
| Music mode by default; Tarot only in Part 3 mode | Part 3 is 22 Tarot and 9 Egyptian of 33 base rows; from Part 4 on, **107 of 111 (96%)** are music ([ja.wikipedia](https://ja.wikipedia.org/wiki/スタンド_%28ジョジョの奇妙な冒険%29); [Tarot Cards](https://jojowiki.com/Tarot_Cards)) |
| Affinity gates the pool | Names "taken from the names of groups and album names I liked in the olden days" ([Comnavi](https://jojowiki.com/Interview:Comnavi_Vol._3_%28February_1998%29)); 82 distinct artists across 108 music names, Hendrix 7 and Queen 5, so personal concentration is canon-like |
| Film scores and composers qualify | Super Fly names a 1972 film soundtrack album ([Super Fly](https://jojowiki.com/Super_Fly)); Tubular Bells is best known through *The Exorcist* ([Tubular Bells](https://jojowiki.com/Tubular_Bells)); Yo-Yo Ma is a cellist ([Yo-Yo Ma](https://jojowiki.com/Yo-Yo_Ma)) |
| Fit dominates S | 54% of names link directly to the ability or user and 38% loosely. Araki: "I find inspiration for characters and Stands through music", and on order, "It depends on the Stand" ([Shonen Jump 2017](https://jojowiki.com/Interview:Shonen_Jump_%28July_2017%29)) |
| Titles over artist names (T = 1.0 vs 0.6) | Direct links: songs 69%, albums 76%, artist names 43% (43/71 ≈ 0.6) |
| Verbatim by default; strip brackets; tail-truncate only over the cap | 68% verbatim, 5% truncated, 26% altered. All 5 truncations drop a prefix or a bracketed subtitle. Of 14 source titles with 4+ words, 6 stayed verbatim, 3 were truncated and 5 altered **[computed]** |
| 28-character cap and word-count prior W | Median 2 words and 12 characters, maximum 27; 93% have 1 to 3 words |
| Alterations off by default | Only 7 of 28 alterations are meaning-bearing swaps (Atom Heart Father, The Lock, Nut King Call); the rest are cosmetic |
| Mineral bonus M | Araki on Gold Experience: JoJo "is into diamonds, stones, minerals" ([Billboard JAPAN](https://www.billboard-japan.com/special/detail/3283)); all four protagonist Stands of Parts 3 to 6 carry one (7 of 144 names) **[computed]** |
| Canon-collision filter; one name per user | No base name is shared by two characters, except an alternate-universe Killer Queen ([Fandom list](https://jojo.fandom.com/wiki/List_of_cultural_references_in_JoJo's_Bizarre_Adventure)) |
| Vary source films across a user's cards | Araki limited Prince names: "If it were all Prince, it would look a bit calculated" ([Billboard JAPAN](https://www.billboard-japan.com/special/detail/3283)) |
| Romanised non-English titles allowed | Boku no Rhythm wo Kiitekure, Mandom, Kraft Work; "only one Stand's name has ever referenced Japanese language music" ([Stand](https://jojowiki.com/Stand)) |
| Offer the top 3, allow a change | Names were revised: Star Silver became Star Platinum, Stairway to Heaven became Made in Heaven ([Star Platinum](https://jojowiki.com/Star_Platinum); [Made in Heaven](https://jojowiki.com/Made_in_Heaven)) |
| No Egyptian names | Only 9 exist; their cards are 「作中オリジナルの設定」, an in-story invention ([ja.wikipedia](https://ja.wikipedia.org/wiki/スタンド_%28ジョジョの奇妙な冒険%29)); the wiki page never states an ability match ([Egypt 9 Glory Gods](https://jojowiki.com/Egypt_9_Glory_Gods)) |

Part 3 mode follows the Tarot-plus-colour pattern Japanese Wikipedia describes. 10 of 22 Tarot Stands carry a colour, metal or numeral. Canon word order is mixed (Silver Chariot puts the colour first, Hierophant Green puts it last), so the colour-first order is a readability choice, not a canon rule. Araki tied Stands to Tarot "because I wanted to make the Stands distinctive" ([JoJo 6251](https://jojowiki.com/Interview:JoJo_6251_%28December_1993%29)).

Katakana has no deterministic rule. A middle dot falls at every English word boundary in 58 of 70 sampled canon names (83%). Even so, the best library pipeline reproduces only **31 of 70 (44%)** exactly **[computed]**, because canon uses older spellings such as ウインド, ファイヤー and ドアー ([alkana](https://github.com/cod-sushi/alkana.py); [e2k](https://pypi.org/project/e2k/)). The English name is therefore primary, which matches Araki's description of English titles as a design that "hits you visually at a glance" ([Billboard JAPAN](https://www.billboard-japan.com/special/detail/3283)), and the katakana stays an editable reading.

Fit uses a hybrid scorer because neither half is enough alone. Lexicons are deterministic and explain themselves, and Roget-style measures match or beat WordNet on word similarity ([Jarmasz and Szpakowicz](https://arxiv.org/pdf/1204.0245)). They miss figurative titles, though. Small encoders cover more ground but are weak on idioms: a SemEval-2022 baseline scored **0.23 on idioms against 0.83 on plain similarity** ([SemEval-2022 Task 2](https://arxiv.org/pdf/2204.10050)). Both candidate encoders are small, permissively licensed and run on a CPU ([bge-small-en-v1.5](https://huggingface.co/BAAI/bge-small-en-v1.5), 33.4M parameters, MIT; [all-MiniLM-L6-v2](https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2), 22.7M, Apache-2.0).

LLMs stay out of the scoring path because they are neither order-stable nor repeatable. Swapping candidate order moved GPT-4's verdict from **51.3% to 23.8%**, and scoring both orders improved agreement with humans by 9.8 points ([Wang et al.](https://arxiv.org/pdf/2305.17926)). "Deterministic" LLM settings still varied accuracy by up to 15% across runs ([Atil et al.](https://arxiv.org/abs/2408.04667)). Hence the stage 8 design: the LLM only breaks ties, both orders must agree, and the verdict is cached.

## Film music exists for 11 of 12 films, scattered across sources

These 12 test films are all from Ojas's own diary. A tracklist was found for 11 of them: Discogs 9, Wikipedia 2. MusicBrainz releases appeared for only 2, through web search, because its API and HTML were blocked here, so that count says nothing about MusicBrainz's real coverage. Wikidata could not be read at all. "Generic" uses the stage 6 detector on tracklists read from Discogs and Wikipedia.

| Film | Region | Letterboxd composer | Tracklist found | Generic titles |
|---|---|---|---|---|
| [The Lighthouse](https://www.discogs.com/master/1623359-Mark-Korven-The-Lighthouse-Original-Soundtrack) | US | Mark Korven | Discogs score, 13 | 0 of 13 |
| [Chungking Express](https://www.discogs.com/release/23059832-Roel-A-Garcia-Frankie-Chan-Chungking-Express-Original-Motion-Picture-Soundtrack-%E9%87%8D%E6%85%B6%E6%A3%AE%E6%9E%97-%E9%9B%BB%E5%BD%B1) | Hong Kong | Garcia, Galasso, Chan | Discogs, 9 cues + 5 songs | 1 of 9 cues |
| [Another Round](https://www.tunefind.com/movie/another-round-2020) | Denmark | none ([page](https://letterboxd.com/film/another-round/)) | none; Tunefind lists 4 songs | n/a |
| [Call Me by Your Name](https://www.discogs.com/master/1300148-Various-Call-Me-By-Your-Name-Original-Motion-Picture-Soundtrack) | Italy | John Adams, Sufjan Stevens | Discogs songs, 17 | not checked |
| [Se7en](https://www.discogs.com/master/1805360-Howard-Shore-Seven-Complete-Original-Score) | US | Howard Shore | Discogs songs 11 + score 16 | 0 of 16 |
| [In the Mood for Love](https://www.discogs.com/master/167983-Various-In-The-Mood-For-Love-Original-Soundtrack-From-The-Motion-Picture) | Hong Kong | Galasso, Umebayashi | Discogs mixed, 21 | 5 of 10 cues |
| Dune: Part Two | US | Hans Zimmer | Discogs 25; MusicBrainz release seen | 0 of 25 |
| Across the Spider-Verse | US | Daniel Pemberton | Discogs score 34 + songs 13 | 2 of 34 |
| La Haine | France | "Assassin" (a rap act) | Discogs songs, 17 | not checked |
| [12th Fail](https://en.wikipedia.org/wiki/12th_Fail_(soundtrack)) | India | Shantanu Moitra | Wikipedia, 5 + 4 lofi | not checked |
| [Kai Po Che!](https://en.wikipedia.org/wiki/Kai_Po_Che!) | India | Amit Trivedi | Wikipedia, 3 songs | not checked |
| [Apur Sansar](https://www.discogs.com/master/3034250-Ravishankar-Apu-Trilogy) | India | Ravi Shankar | 1978 LP, 6 cues for this film | 6 of 20 on the LP |

Coverage of non-Western films is better than feared, with three caveats. All five Asian films had a tracklist, but none showed up on MusicBrainz, and two existed only on Wikipedia. Generic titles also cluster in older and non-Western scores. Over a 16-album, 280-track sample, the refined detector that stage 6 adopts flags **9.3%** of tracks overall **[computed]**, but **30% on the Apu LP and 50% of In the Mood for Love's cues**, against 0% on the modern Hollywood scores. Track 1 is generic on 4 of 16 albums, so the whole album must be scanned. v0's filter rejected 18.9% of tracks, and 6.8% of all tracks were lost to its length cap alone, among them *Dune: Part Two*'s opening cue "Beginnings Are Such Delicate Times". Indian films are song-driven (Kai Po Che has 3 songs), so stage 4 must accept a songs album when no score album exists.

Composer credits exist for 11 of 12 films, but on 3 of them (Call Me by Your Name, La Haine, Kai Po Che) the credit names the songwriters or song artists rather than a score composer. That is why stage 4 matches each album's artist credit against the credited composers. Albums also miss needle drops: Tunefind lists 24 songs for Call Me by Your Name against 17 on the album ([Tunefind](https://www.tunefind.com/movie/call-me-by-your-name-2017)).

Service terms shape the engineering in five places:

| Service | Term that matters | Engineering consequence |
|---|---|---|
| MusicBrainz | About 1 request/s; User-Agent with contact required; core data CC0 ([rate limiting](https://musicbrainz.org/doc/MusicBrainz_API/Rate_Limiting); [licence](https://musicbrainz.org/doc/About/Data_License)) | Fetch from a CLI, since browsers cannot set the header; results can be cached and shipped; an 8-film pool takes about 25 s **[computed arithmetic]** |
| Discogs | Token needed for search; 60 requests/min; cache no longer than "necessary to provide a service"; notice "Data provided by Discogs." ([developers](https://www.discogs.com/developers); [API terms](https://support.discogs.com/hc/en-us/articles/360009334593)) | Per-user cache only, never a shipped table; show the notice |
| Spotify | New apps need an owner with Premium; 5 users per app ([migration guide](https://developer.spotify.com/documentation/web-api/tutorials/february-2026-migration-guide)) | Excluded |
| IMDb | Scraping forbidden; datasets have no soundtrack file ([IMDb](https://help.imdb.com/article/imdb/general-information/can-i-use-imdb-data-in-my-software/G5JTRESSHJBBHTGX)) | Excluded |
| Tunefind | No open API; terms unreadable this session | Optional fetch the user starts |

## Worked example: Ojas gets 「DIRTY WEATHER」 [illustrative]

The signature stat comes from the earlier spec session: **PRC high** (ρ = 0.83 against a proxy crowd, z capped at +2.58), which selects template 12, "Air Reading". Its canon Stand is Manhattan Transfer, "able to read the air currents around it" ([Manhattan Transfer](https://jojowiki.com/Manhattan_Transfer)). The second stat is RNG low (z −1.05). That ranking is not robust to calibration.

His export rates the four favourites **The Lighthouse 4.5★, Chungking Express 5★, Another Round 5★ and Call Me by Your Name 5★**. Within his 512 ratings, 4.5★ sits at the 88th percentile and 5★ at the 97th **[computed]**. Howard Shore is his top composer (m = 4.19, n = 4), so Se7en joins the pool. Letterboxd credits Michael Galasso on both Chungking Express and In the Mood for Love, both rated 5★, which gives Galasso m = 4.06. That puts him second behind Shore and ahead of Randy Newman (4.04) in the earlier ranking, which used a TMDB 2017 snapshot and missed him **[computed; full composer table not rebuilt]**.

Candidates come from three of the pool films. The Lighthouse's 13 tracks become about 20 candidates after merge splitting. Chungking Express contributes 8 cues and 2 needle drops; Faye Wong's songs were not checked, and "What A Difference A Day Made" drops out because its only tail spans fail the function-word and noun tests. Se7en adds 16 score titles. Another Round adds nothing without Tunefind. With the optional Tunefind fetch it would add "What a Life" and "Cissy Strut". The Call Me by Your Name tracklist was not read **[unverified]**. E was not computed, so it is set to 0.5 for every candidate, and q_c = 0.5 except Shore's 1.0.

| Candidate | Source | L | F | A | Y | S |
|---|---|---|---|---|---|---|
| **DIRTY WEATHER** | Lighthouse, track 5 ("Curse Your Name / Dirty Weather") | 1 ("weather") | 0.75 | 0.87 | 1.00 | **1.02** |
| RAIN, TEAR AND SWEAT | Chungking Express cue | 0.5 ("rain", ring) | 0.50 | 0.89 | 0.45 | 0.72 |
| METAPHYSICAL EXPRESS | Tail of "Chasing The Metaphysical Express" (32 chars) | 0 | 0.25 | 0.89 | 1.00 | 0.53 |
| THE LAST SEVEN DAYS | Se7en score, track 1 | 0 | 0.25 | 0.49 | 0.45 | 0.39 |

Why line: `Why 「DIRTY WEATHER」: one cue of "Curse Your Name / Dirty Weather", track 5 of Mark Korven's The Lighthouse score (2019); The Lighthouse is one of your Favourite Films (4.5★). "Weather" fits your Air Reading ability, which reads the air; fit rank 1 of the pool.` The pipeline's katakana is ダーティー・ウェザー **[computed]**.

The same pool gives different names under other signature stats, which shows the fit stage doing the work. Under **RNG low** (Fixation, modelled on Kraft Work "completely freezing it in place", [Kraft Work](https://jojowiki.com/Kraft_Work)) the winner is **「STRANDED」**, Lighthouse track 7, S = 0.99. The pipeline renders it ストゥランディド, the kind of error the 44% kana score predicts. Under **SPD high** (Time Stop) the winner is **「THE LAST SEVEN DAYS」** through "days", S = 0.89, ahead of "Night Snack" at 0.78. That reproduces v0's name for a stated reason rather than by track order.

The fallbacks are 「HOWARD SHORE」 (Route A) and 「YELLOW JUSTICE」 (Part 3 mode). "The Wire" (Se7en) is one letter from canon's Wired. It passes because it is under 8 characters and would carry a homage note.

## A 63-Stand canon benchmark can catch failure, not rank close rivals

The benchmark is a known-item retrieval task: the query describes the ability, and the canon title is the single correct item among k candidates.

| Component | Design |
|---|---|
| Items | The 63 canon base rows named after a song or album **[computed]**: Part 4 13, Part 5 5, Part 6 12, Part 7 19, Part 8 6, spin-offs 8. Single-coder link codes give 40 direct, 15 loose, 1 none, 7 unclear; primary scoring uses the 55 direct or loose rows. 28 of the 63 rest only on the Fandom list, which agreed with JoJo Wiki on 24 of 26 cross-checked rows ([Fandom list](https://jojo.fandom.com/wiki/List_of_cultural_references_in_JoJo's_Bizarre_Adventure)). The 45 artist-name rows form a separate track, where semantic methods should score near random |
| Queries | Q1: the JoJo Wiki ability one-liner with the Stand's name tokens removed. Q2: the production input, i.e. the template gloss chosen by each Stand's own canon stats under the stage 1 rule; Q2 excludes the 16 Stands the templates cite, since including them would be circular |
| Candidate sets | A: the other tracks of the source album (random top-1 about 7 to 12%). B: the artist's other recordings. C: random title pools of k = 20 and 100 from the MusicBrainz CC0 dump, matched on length. D: canon names planted into film-score tracklists, used only to test the filters |
| Methods | Baselines: random, the first-track rule (v0), shortest title. Single components: style only (Y), lexicon only (L), mean GloVe vectors, encoder only (E). The hybrid F. An LLM re-ranking the top 5 in both orders, reported only as an upper bound because models have likely memorised Stand names |
| Metrics | Top-1, top-5, MRR and mean percentile rank, split by verbatim, truncated or altered name and by link strength |
| Statistics | Tune lexicons and weights on Parts 4 and 5 (18 rows) and test on Parts 6 to 8 (37 rows). Bootstrap over Stands 10,000 times, use a paired bootstrap for method differences, and redraw distractors 20 times per Stand. Random baselines are 1/k for top-1 and H_k/k for MRR: 0.293 at k = 10, 0.180 at k = 20, 0.052 at k = 100 **[computed]** |

The statistical ceiling is the main limit. With true top-1 accuracy of 0.40, the 95% Wilson interval spans **0.26 to 0.56 at n = 37** and 0.28 to 0.53 at n = 55 **[computed]**. Comparing two methods on the same Stands needs about 57 Stands to detect a 0.20 gap in top-1 and about 233 for a 0.10 gap (assuming 30% of Stands come out differently under the two methods). The benchmark can therefore show that a method beats random and the first-track rule by a wide margin, but it cannot choose between close variants. The decision rule follows from that: **ship the simplest method within the best method's interval**. If lexicon-only ties the hybrid, drop the encoder.

Canon cannot test personal fit, so that part needs a human check. Run pairwise A/B tests on about 20 real exports with at least 3 raters per pair and sides randomised, and report win rates with bootstrap intervals, plus Bradley-Terry scores when three or more variants are compared. Separately, compare the output names across many users with canon: 93% should have 1 to 3 words and about 68% should be verbatim.

Three filter checks already ran against canon **[computed]**. v0's filter rejects **5 of 144** canon names: Yo-Yo Ma, 20th Century BOY, Hey Ya! and Soft & Wet fail its letter-ratio test, whose denominator counts spaces, and Tomb of the Boom 1 2 3 fails on length. The stage 6 filter rejects **1 of 144**, Tomb of the Boom 1 2 3, through its trailing numeral. On the 63 canon song and album source titles, v0 rejects 5 and stage 6 rejects none. Both checks ran without the film-title and profanity rules. Finally, the stage 5 tail rule, using its function-word test alone, reproduces **5 of 5** canon truncations: Crazy Diamond, Heaven's Door, Bites the Dust, Scary Monsters and Voodoo Child. The noun guard has not been tested.

## Prior art and the measurements still owed

No tool found builds a Stand name from personal film or music data. [CodeItBro](https://www.codeitbro.com/tool/jojo-stand-generator) picks a random canon Stand. [TheStoryShack](https://thestoryshack.com/tools/jojo-stand-name-generator) "randomly combines" themes ("Currently, you can't specify details"). [BalisticPenguin](https://balisticpenguin.itch.io/jjba-stand-generator) ships "Over 1200 Stand names", and [BasedLabs](https://www.basedlabs.ai/tools/stand-generator) generates names with an LLM. The nearest research runs the other way, generating a playlist title from its tracks ([arXiv 2110.07354](https://arxiv.org/abs/2110.07354)). Searches covered Spotify-, Letterboxd- and song-based Stand generators. GitHub code search was unreachable, so "none found" is the honest claim. Deterministic provenance plus a canon-benchmarked choice is the differentiator.

The following items could not be measured this session:

| Gap | Effect on the algorithm | Probe |
|---|---|---|
| MusicBrainz IMDb-link coverage per film (0 of 12 read directly) | Decides whether stage 4a or 4b leads | P1 |
| Whether `inc=release-group-rels` works on URL lookups, and the stored IMDb URL form | Stage 4a query shape | P1 |
| Wikidata P406/P436 coverage | Possible exact film-to-album bridge | P2 |
| Discogs API hits for Another Round, 12th Fail, Kai Po Che (web search found none, inconclusive) | Non-Western coverage | P3 |
| TMDB composer jobs vs Letterboxd "Composer"; credits after 2017 | Stage 3 composer ranking | P4 |
| Encoder fit on canon; float determinism across machines | Whether E stays in F | P5 |
| Lexicon contents, all weights, the noun guard | Every score above | Benchmark |
| Tunefind and WhatSong terms and APIs | Needle-drop route | Read their terms pages |
| Japanese release-title lookup (MusicBrainz aliases) | Katakana accuracy above 44% | Untested idea |
| Part 5 to 9 namesakes resting on Fandom only, 70 years recalled from memory, about 28 Stands missing from the table | Benchmark labels | Read each Stand page |

(The probe scripts P1 to P5 from the original document are omitted: the soundtrack data now comes from the local MusicBrainz dump in tools/mbdump/.)

## Conclusion

The shift from v0 is that personal affinity now selects the **pool** and the ability selects the **name**, rather than taking the first track of the top film. That is also the order Araki describes: a fixed taste library, then a fitting pick. It explains why a canon-faithful generator can work on any user's data. Canon's own link rate (54% direct) sets a realistic ceiling, so an occasional loose match is in character, not a defect. Ojas's export also turned up a side benefit: Letterboxd composer credits surface composers like Galasso that a pre-2017 TMDB snapshot missed, so cross-checking credit sources changes whose music a card is built from.

Two numbers will settle the open design choices. The first is MusicBrainz's IMDb-link hit rate from probe P1; if it is low, Discogs becomes the primary source and its caching terms then govern the CLI's design. The second is whether the encoder beats lexicon-only on the 37-Stand test split. If it does not, the whole algorithm becomes rule-based and fully explainable, with the optional, order-checked LLM tie-break as the only learned component.

## Changes agreed on 25 Oct 2026 (override the sections above where they conflict)

- Soundtrack data comes from the local MusicBrainz dump (~/lb_collect_out/mb_local.jsonl), not the stage 4 cascade.
- Pool: A(f) = 0.40 favourite + 0.25 own rating percentile + 0.15 own-minus-crowd percentile + 0.10 rewatch percentile + 0.10 composer affinity; top 8 films that have candidates.
- Names must be evocative. Add Cool(n) in [0,1] (word rarity, phrase shape, concreteness or emotion ratings, lone-proper-name penalty, canon word-count prior), tuned on the canon album benchmark.
- Score: S = 0.45 Cool + 0.35 F + 0.20 A, lexicon-only F. A candidate below the pool's median Cool cannot win. Among scores within 0.03 of the top, a username hash picks.
- The Stand assigns its own name; no user choice is required.
