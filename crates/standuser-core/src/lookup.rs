//! The film lookup table that ships with the site, and the same format returned by the TMDB proxy function.
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

#[derive(Debug, Clone, Default)]
pub struct Lookup {
    pub films: Vec<FilmMeta>,
    by_key: HashMap<String, usize>,
    by_lid: HashMap<String, usize>,
    pub country_names: HashMap<String, String>,
    pub people: HashMap<u32, String>,
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

    /// Add rows (e.g. from the TMDB proxy). Existing keys are kept.
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
