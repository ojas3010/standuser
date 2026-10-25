//! The film lookup table that ships with the site (built ahead of time by tools/build_table.py from the collector's
//! output; nothing is looked up at run time).
//!
//! JSON: {"v":1,"countries":{"US":"United States of America"},"people":{"1317730":"Daniel Scheinert"},
//!        "films":[[key,lid,tmdb,year,runtime,lang,"US|PL","dirIds","composerIds",avg,histMean,count], ...]}

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;

/// Normalised "title|year" join key: NFKD, letters and digits only, lower case.
pub fn film_key(name: &str, year: i32) -> String {
    let t: String = name.nfkd().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
    format!("{t}|{year}")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Row(
    String,         // key
    String,         // Letterboxd short code ("" if unknown)
    u32,            // TMDB id
    i32,            // year
    u16,            // runtime minutes (0 = unknown)
    String,         // original language
    String,         // countries "US|PL"
    String,         // director ids "1,2"
    String,         // composer ids "3,4"
    Option<f64>,    // Letterboxd displayed average
    Option<f64>,    // Letterboxd plain mean from the rating histogram
    Option<u64>,    // Letterboxd rating count
);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Table {
    #[serde(default)]
    v: u32,
    /// Date the table was built from TMDB and Letterboxd data ("YYYY-MM-DD").
    #[serde(default)]
    built: Option<String>,
    #[serde(default)]
    countries: HashMap<String, String>,
    #[serde(default)]
    people: HashMap<String, String>,
    #[serde(default)]
    films: Vec<Row>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FilmMeta {
    pub key: String,
    pub lid: String,
    pub tmdb_id: u32,
    pub year: i32,
    pub runtime: u16,
    pub lang: String,
    pub countries: Vec<String>,
    pub directors: Vec<u32>,
    pub composers: Vec<u32>,
    pub lb_avg: Option<f64>,
    pub lb_hist_mean: Option<f64>,
    pub lb_count: Option<u64>,
}

/// One name candidate of a film (tools/naming/build_names_data.py): a soundtrack track title, scored offline.
#[derive(Debug, Clone, PartialEq)]
pub struct NameCand {
    /// Upper-case name the card can draw.
    pub name: String,
    /// Cool(n) x 100, 0 to 100.
    pub cool: u8,
    /// 1-based track number on the album.
    pub track: u16,
    /// v verbatim, s brackets stripped, c one cue of a merged title, t tail of a longer title.
    pub kind: char,
    /// The source track title when it differs from the name, else empty.
    pub src: String,
    /// Lexicon matches: template * 16 + word index * 2 + (1 for a core word, 0 for a WordNet ring word).
    pub codes: Vec<u16>,
    /// Index into the film's `albums`: the album this track is on.
    pub album: usize,
}

/// One soundtrack album a film's candidates come from.
#[derive(Debug, Clone, PartialEq)]
pub struct Album {
    /// The album's artist credit ("" when none was kept).
    pub artist: String,
    /// The album credits the film's composer (a score) rather than a songs album or various artists.
    pub score: bool,
}

/// A film's albums (a film can have a score, a songs album and a various-artists album) and the candidates of all of them.
#[derive(Debug, Clone, PartialEq)]
pub struct FilmNames {
    pub albums: Vec<Album>,
    pub cands: Vec<NameCand>,
}

/// The names file format: [[[artist, flag], ...], [[name, cool, track, kind, src, codes, album], ...]] per TMDB id.
type NameRow = (Vec<(String, String)>, Vec<(String, u8, u16, String, String, Vec<u16>, usize)>);

#[derive(Deserialize)]
struct NamesHead {
    #[serde(default)]
    v: u32,
}

#[derive(Deserialize)]
struct NamesFile {
    #[serde(default)]
    names: HashMap<String, NameRow>,
}

/// The version of the names file this build reads.
pub const NAMES_FORMAT: u32 = 2;

#[derive(Debug, Clone, Default)]
pub struct Lookup {
    pub films: Vec<FilmMeta>,
    by_key: HashMap<String, usize>,
    by_lid: HashMap<String, usize>,
    pub country_names: HashMap<String, String>,
    pub people: HashMap<u32, String>,
    /// Name candidates by TMDB id (names.json); empty when none were loaded, which leaves the composer route.
    pub names: HashMap<u32, FilmNames>,
    /// Build date of the table (the oldest one when tables were merged).
    pub built: Option<crate::dates::Day>,
}

/// TMDB's terms allow caching its data for at most six months.
pub const MAX_AGE_DAYS: i64 = 183;

fn ids(s: &str) -> Vec<u32> {
    s.split(',').filter_map(|x| x.trim().parse().ok()).collect()
}

impl Lookup {
    pub fn from_json(json: &str) -> Result<Lookup, String> {
        let mut l = Lookup::default();
        l.merge_json(json)?;
        Ok(l)
    }

    /// Add the rows of another table in the same format. Existing keys are kept, and the older build date wins.
    pub fn merge_json(&mut self, json: &str) -> Result<(), String> {
        let t: Table = serde_json::from_str(json).map_err(|e| format!("bad lookup table: {e}"))?;
        if let Some(b) = t.built.as_deref().and_then(crate::dates::parse_date) {
            self.built = Some(self.built.map_or(b, |x| x.min(b)));
        }
        for (k, v) in t.countries {
            self.country_names.entry(k).or_insert(v);
        }
        for (k, v) in t.people {
            if let Ok(id) = k.parse::<u32>() {
                self.people.entry(id).or_insert(v);
            }
        }
        for r in t.films {
            if self.by_key.contains_key(&r.0) {
                continue;
            }
            let i = self.films.len();
            self.by_key.insert(r.0.clone(), i);
            if !r.1.is_empty() {
                self.by_lid.entry(r.1.clone()).or_insert(i);
            }
            self.films.push(FilmMeta {
                key: r.0,
                lid: r.1,
                tmdb_id: r.2,
                year: r.3,
                runtime: r.4,
                lang: r.5,
                countries: r.6.split('|').filter(|s| !s.is_empty()).map(String::from).collect(),
                directors: ids(&r.7),
                composers: ids(&r.8),
                lb_avg: r.9,
                lb_hist_mean: r.10,
                lb_count: r.11,
            });
        }
        Ok(())
    }

    /// Add the name candidates of names.json (a separate file; the film table's format is unchanged).
    pub fn load_names(&mut self, json: &str) -> Result<(), String> {
        // the version first, so that a file in another format says so instead of failing on its shape
        let head: NamesHead = serde_json::from_str(json).map_err(|e| format!("bad names file: {e}"))?;
        if head.v != NAMES_FORMAT {
            return Err(format!("bad names file: format {} (this build reads {NAMES_FORMAT})", head.v));
        }
        let f: NamesFile = serde_json::from_str(json).map_err(|e| format!("bad names file: {e}"))?;
        let mut loaded = vec![];
        for (k, (albums, cands)) in f.names {
            let Ok(id) = k.parse::<u32>() else { continue };
            if let Some(c) = cands.iter().find(|c| c.6 >= albums.len()) {
                return Err(format!("bad names file: {} of film {id} points at album {} of {}", c.0, c.6, albums.len()));
            }
            let albums = albums.into_iter().map(|(artist, flag)| Album { artist, score: flag == "c" }).collect();
            let cands = cands
                .into_iter()
                .map(|(name, cool, track, kind, src, codes, album)| NameCand { name, cool, track, kind: kind.chars().next().unwrap_or('v'), src, codes, album })
                .collect();
            loaded.push((id, FilmNames { albums, cands }));
        }
        self.names.extend(loaded);
        Ok(())
    }

    /// Letterboxd short code first (exact), then the normalised title and year.
    pub fn find(&self, lid: Option<&str>, name: &str, year: i32) -> Option<&FilmMeta> {
        if let Some(l) = lid {
            if let Some(&i) = self.by_lid.get(l) {
                return Some(&self.films[i]);
            }
        }
        self.by_key.get(&film_key(name, year)).map(|&i| &self.films[i])
    }

    /// Days since the table was built, when it carries a build date.
    pub fn age_days(&self, today: crate::dates::Day) -> Option<i64> {
        self.built.map(|b| today - b)
    }

    pub fn len(&self) -> usize {
        self.films.len()
    }

    pub fn is_empty(&self) -> bool {
        self.films.is_empty()
    }

    pub fn person(&self, id: u32) -> String {
        self.people.get(&id).cloned().unwrap_or_else(|| format!("#{id}"))
    }

    /// Country display name for a TMDB code, with the article the card text needs ("the United States").
    pub fn country_phrase(&self, code: &str) -> String {
        let n = self.country_names.get(code).map(String::as_str).unwrap_or(code);
        let n = match n {
            "United States of America" => "United States",
            x => x,
        };
        match n {
            "United States" | "United Kingdom" | "Netherlands" | "Philippines" | "Czech Republic" | "United Arab Emirates" => format!("the {n}"),
            x => x.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_normalisation() {
        assert_eq!(film_key("Amélie", 2001), "amelie|2001");
        assert_eq!(film_key("Borat: Cultural Learnings", 2006), "boratculturallearnings|2006");
        assert_eq!(film_key("Se7en", 1995), "se7en|1995");
    }

    #[test]
    fn names_load_separately_from_the_table() {
        let mut l = Lookup::from_json(r#"{"v":1,"films":[]}"#).unwrap();
        assert!(l.names.is_empty());
        l.load_names(r#"{"v":2,"meta":{},"names":{"7840":[[["A B","c"]],[["WOUNDED HUNTER",88,11,"v","",[33,48],0],["WALKING","74",2,"c","x / y",[],0]]]}}"#).unwrap_err();
        l.load_names(r#"{"v":2,"meta":{},"names":{"7840":[[["A B","c"],["Various Artists","v"]],[["WOUNDED HUNTER",88,11,"v","",[33,48],0],["WALKING",74,2,"c","x / y",[],1]]],"x":[[],[]]}}"#).unwrap();
        let n = &l.names[&7840];
        assert_eq!(n.albums, vec![Album { artist: "A B".into(), score: true }, Album { artist: "Various Artists".into(), score: false }]);
        assert_eq!(n.cands.len(), 2);
        assert_eq!(n.cands[1], NameCand { name: "WALKING".into(), cool: 74, track: 2, kind: 'c', src: "x / y".into(), codes: vec![], album: 1 });
        assert_eq!(l.names.len(), 1, "a non-numeric id is skipped");
        assert!(l.load_names("not json").is_err());
        let old = r#"{"v":1,"names":{"7840":["A B","c",[["WALKING",74,2,"c","",[]]]]}}"#;
        assert!(l.load_names(old).unwrap_err().contains("format 1"), "the old one-artist format is refused");
        let dangling = r#"{"v":2,"names":{"7840":[[["A B","c"]],[["WALKING",74,2,"c","",[],1]]]}}"#;
        assert!(l.load_names(dangling).unwrap_err().contains("album 1 of 1"), "an album index with no album is refused");
    }

    #[test]
    fn load_find_merge() {
        let j = r#"{"v":1,"built":"2026-10-24","countries":{"US":"United States of America"},"people":{"1":"A B"},
          "films":[["seven|1995","7x",807,1995,127,"en","US","1","2,3",3.9,3.91,2000000]]}"#;
        let mut l = Lookup::from_json(j).unwrap();
        let f = l.find(Some("7x"), "zzz", 0).unwrap();
        assert_eq!(f.tmdb_id, 807);
        assert_eq!(f.directors, vec![1]);
        assert_eq!(f.composers, vec![2, 3]);
        assert!(l.find(None, "Seven", 1995).is_some());
        assert_eq!(l.age_days(crate::dates::parse_date("2026-11-03").unwrap()), Some(10));
        assert_eq!(l.country_phrase("US"), "the United States");
        l.merge_json(r#"{"films":[["seven|1995","",1,1995,1,"x","","","",null,null,null],["new|2000","",5,2000,90,"fr","FR","","",null,null,null]]}"#).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l.find(None, "Seven", 1995).unwrap().tmdb_id, 807, "existing rows win");
    }
}
