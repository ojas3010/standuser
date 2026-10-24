Constructed fixtures (no live site was reachable when these were written).

film_*.html     Letterboxd film pages built from the documented fields: <body data-tmdb-id data-tmdb-type>, TMDB and IMDb
                anchors (data-track-action), CDATA-wrapped JSON-LD with aggregateRating, twitter:data2 meta, and the
                inline ratings-histogram-chart markup copied in shape from the samlearner saved page.
csi_hist_*.html Rating-histogram fragments.  The K-abbreviated counts and the "No <stars> ratings" / singular "rating"
                wordings are GUESSES covering how the anonymous endpoint might print small and large counts.
cloudflare_challenge.html  A "Just a moment..." interstitial.
tmdb_*.json     TMDB /movie and /search responses with the documented field names; values are plausible, not exact.

test_collect.py also parses the real saved page from the samlearner repo when it exists on the machine (skipped otherwise).
