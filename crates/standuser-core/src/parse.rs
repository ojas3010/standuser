//! Letterboxd export reading: ZIP (or a map of file name -> bytes) to plain rows.
//!
//! profile.csv is read through an allowlist of one column: `Date Joined`, in memory. Every other column (the e-mail
//! address, name, location, bio, favourite films, the username included) is never decoded, stored, shown or logged.
//! profile.csv or the column may be absent, and an unreadable profile.csv is treated as absent; the username then
//! comes from the ZIP file name or a flag.

use crate::dates::{parse_date, Day};
use std::collections::BTreeMap;
use std::io::Read;

#[derive(Debug, Clone, PartialEq)]
pub struct Watched {
    pub day: Day,
    pub name: String,
    pub year: i32,
    pub uri: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rating {
    pub name: String,
    pub year: i32,
    pub rating: f64,
    pub uri: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiaryEntry {
    /// Date the entry was logged.
    pub log_day: Day,
    pub name: String,
    pub year: i32,
    /// Watched Date (the viewing day); falls back to the log day when blank.
    pub view_day: Day,
}

#[derive(Debug, Clone, Default)]
pub struct Export {
    pub watched: Vec<Watched>,
    pub ratings: Vec<Rating>,
    pub diary: Vec<DiaryEntry>,
    pub joined: Option<Day>,
    pub username: Option<String>,
    /// Export date taken from the ZIP file name, else None.
    pub as_of: Option<Day>,
}

#[derive(Debug)]
pub enum ParseError {
    Zip(String),
    Missing(&'static str),
    Csv(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Zip(e) => write!(f, "cannot read the ZIP file: {e}"),
            ParseError::Missing(n) => write!(f, "{n} not found in the export (is this a Letterboxd export ZIP?)"),
            ParseError::Csv(e) => write!(f, "cannot read a CSV file: {e}"),
        }
    }
}

impl std::error::Error for ParseError {}

const MAX_ENTRY: u64 = 64 * 1024 * 1024;

/// Read the four files we use out of a Letterboxd export ZIP. Only top-level (or single-folder-deep) entries are read;
/// `deleted/`, `orphaned/`, `likes/` and `lists/` are ignored.
pub fn read_zip(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, ParseError> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| ParseError::Zip(e.to_string()))?;
    let mut out = BTreeMap::new();
    for i in 0..z.len() {
        let f = z.by_index(i).map_err(|e| ParseError::Zip(e.to_string()))?;
        let path = f.name().replace('\\', "/");
        if f.is_dir() || path.contains("..") || path.starts_with('/') {
            continue;
        }
        let parts: Vec<&str> = path.split('/').collect();
        let base = parts[parts.len() - 1].to_lowercase();
        let depth_ok = parts.len() == 1 || (parts.len() == 2 && !matches!(parts[0], "deleted" | "orphaned" | "likes" | "lists"));
        if !depth_ok || !matches!(base.as_str(), "watched.csv" | "ratings.csv" | "diary.csv" | "profile.csv") {
            continue;
        }
        let mut buf = Vec::new();
        f.take(MAX_ENTRY).read_to_end(&mut buf).map_err(|e| ParseError::Zip(e.to_string()))?;
        out.insert(base, buf);
    }
    Ok(out)
}

/// "letterboxd-<username>-2026-10-19-13-22-utc.zip" -> (username, export day)
pub fn parse_file_name(name: &str) -> (Option<String>, Option<Day>) {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let base = base.strip_suffix(".zip").unwrap_or(base);
    let rest = match base.strip_prefix("letterboxd-") {
        Some(r) => r,
        None => return (None, None),
    };
    let rest = rest.strip_suffix("-utc").unwrap_or(rest);
    // ...-YYYY-MM-DD-HH-MM : peel 5 numeric groups from the right
    let parts: Vec<&str> = rest.split('-').collect();
    if parts.len() < 6 {
        return (None, None);
    }
    let n = parts.len();
    let date = format!("{}-{}-{}", parts[n - 5], parts[n - 4], parts[n - 3]);
    let user = parts[..n - 5].join("-");
    (if user.is_empty() { None } else { Some(user) }, parse_date(&date))
}

fn reader(bytes: &[u8]) -> csv::Reader<&[u8]> {
    csv::ReaderBuilder::new().flexible(true).from_reader(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes))
}

fn col(h: &csv::StringRecord, name: &str) -> Option<usize> {
    h.iter().position(|c| c.trim().eq_ignore_ascii_case(name))
}

pub fn parse_files(files: &BTreeMap<String, Vec<u8>>, file_name: Option<&str>) -> Result<Export, ParseError> {
    let mut ex = Export::default();
    if let Some(n) = file_name {
        let (u, d) = parse_file_name(n);
        ex.username = u;
        ex.as_of = d;
    }

    let w = files.get("watched.csv").ok_or(ParseError::Missing("watched.csv"))?;
    let mut rd = reader(w);
    let h = rd.headers().map_err(|e| ParseError::Csv(e.to_string()))?.clone();
    let (cd, cn, cy, cu) = (col(&h, "Date"), col(&h, "Name"), col(&h, "Year"), col(&h, "Letterboxd URI"));
    let (cd, cn, cy) = match (cd, cn, cy) {
        (Some(a), Some(b), Some(c)) => (a, b, c),
        _ => return Err(ParseError::Csv("watched.csv lacks Date, Name or Year".into())),
    };
    for rec in rd.records() {
        let r = rec.map_err(|e| ParseError::Csv(e.to_string()))?;
        let (Some(day), Some(name), Some(year)) = (r.get(cd).and_then(parse_date), r.get(cn), r.get(cy).and_then(|y| y.trim().parse::<i32>().ok())) else {
            continue;
        };
        ex.watched.push(Watched { day, name: name.to_string(), year, uri: cu.and_then(|i| r.get(i)).unwrap_or("").to_string() });
    }

    if let Some(b) = files.get("ratings.csv") {
        let mut rd = reader(b);
        let h = rd.headers().map_err(|e| ParseError::Csv(e.to_string()))?.clone();
        if let (Some(cn), Some(cy), Some(cr)) = (col(&h, "Name"), col(&h, "Year"), col(&h, "Rating")) {
            let cu = col(&h, "Letterboxd URI");
            for rec in rd.records() {
                let r = rec.map_err(|e| ParseError::Csv(e.to_string()))?;
                let (Some(name), Some(year), Some(rating)) = (r.get(cn), r.get(cy).and_then(|y| y.trim().parse::<i32>().ok()), r.get(cr).and_then(|x| x.trim().parse::<f64>().ok())) else {
                    continue;
                };
                ex.ratings.push(Rating { name: name.to_string(), year, rating, uri: cu.and_then(|i| r.get(i)).unwrap_or("").to_string() });
            }
        }
    }

    if let Some(b) = files.get("diary.csv") {
        let mut rd = reader(b);
        let h = rd.headers().map_err(|e| ParseError::Csv(e.to_string()))?.clone();
        if let (Some(cd), Some(cn), Some(cy)) = (col(&h, "Date"), col(&h, "Name"), col(&h, "Year")) {
            let cv = col(&h, "Watched Date");
            for rec in rd.records() {
                let r = rec.map_err(|e| ParseError::Csv(e.to_string()))?;
                let (Some(log_day), Some(name), Some(year)) = (r.get(cd).and_then(parse_date), r.get(cn), r.get(cy).and_then(|y| y.trim().parse::<i32>().ok())) else {
                    continue;
                };
                let view_day = cv.and_then(|i| r.get(i)).and_then(parse_date).unwrap_or(log_day);
                ex.diary.push(DiaryEntry { log_day, name: name.to_string(), year, view_day });
            }
        }
    }

    if let Some(b) = files.get("profile.csv") {
        ex.joined = joined_from_profile(b);
    }
    Ok(ex)
}

/// The `Date Joined` value of profile.csv, or None when the column or value is missing or the file cannot be read.
/// Only the header names and that one field are decoded; errors carry no file content.
fn joined_from_profile(bytes: &[u8]) -> Option<Day> {
    let mut rd = reader(bytes);
    let cj = {
        let h = rd.headers().ok()?;
        col(h, "Date Joined")?
    };
    let mut br = csv::ByteRecord::new();
    if !rd.read_byte_record(&mut br).ok()? {
        return None;
    }
    br.get(cj).and_then(|f| std::str::from_utf8(f).ok()).and_then(parse_date)
}

pub fn parse_zip(bytes: &[u8], file_name: Option<&str>) -> Result<Export, ParseError> {
    parse_files(&read_zip(bytes)?, file_name)
}

/// Letterboxd short code from "https://boxd.it/jUk4".
pub fn boxd_code(uri: &str) -> Option<&str> {
    let c = uri.trim().trim_end_matches('/').rsplit('/').next()?;
    if c.is_empty() { None } else { Some(c) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_name() {
        let (u, d) = parse_file_name("/x/letterboxd-jas3344-2026-10-19-13-22-utc.zip");
        assert_eq!(u.as_deref(), Some("jas3344"));
        assert_eq!(crate::dates::iso(d.unwrap()), "2026-10-19");
        let (u, _) = parse_file_name("letterboxd-a-b-2026-10-19-13-22-utc.zip");
        assert_eq!(u.as_deref(), Some("a-b"));
        assert_eq!(parse_file_name("export.zip"), (None, None));
    }

    fn base() -> BTreeMap<String, Vec<u8>> {
        let mut f = BTreeMap::new();
        f.insert("watched.csv".into(), b"Date,Name,Year,Letterboxd URI\n2022-07-06,\"Film, The\",1999,https://boxd.it/abc\n".to_vec());
        f.insert("ratings.csv".into(), b"Date,Name,Year,Letterboxd URI,Rating\n2022-07-06,\"Film, The\",1999,https://boxd.it/abc,4.5\n".to_vec());
        f.insert("diary.csv".into(), b"Date,Name,Year,Letterboxd URI,Rating,Rewatch,Tags,Watched Date\n2022-07-07,\"Film, The\",1999,u,4.5,,,2022-07-05\n".to_vec());
        f
    }

    #[test]
    fn rows_and_only_the_join_date_from_profile() {
        let mut f = base();
        f.insert("profile.csv".into(), b"Date Joined,Username,Given Name,Family Name,Email Address,Location\n2022-07-06,someone,Gina,Sentinel,secret@example.com,Nowhere\n".to_vec());
        let e = parse_files(&f, None).unwrap();
        assert_eq!(e.watched[0].name, "Film, The");
        assert_eq!(e.ratings[0].rating, 4.5);
        assert_eq!(e.diary[0].view_day, crate::dates::parse_date("2022-07-05").unwrap());
        assert_eq!(crate::dates::iso(e.joined.unwrap()), "2022-07-06");
        // the profile username is NOT read: it comes only from the file name or a flag
        assert_eq!(e.username, None);
        let dump = format!("{e:?}");
        for secret in ["secret@example.com", "someone", "Gina", "Sentinel", "Nowhere"] {
            assert!(!dump.contains(secret), "{secret} leaked into the parsed export");
        }
        let named = parse_files(&f, Some("letterboxd-fromname-2026-10-19-13-22-utc.zip")).unwrap();
        assert_eq!(named.username.as_deref(), Some("fromname"));
        assert_eq!(boxd_code("https://boxd.it/abc"), Some("abc"));
    }

    #[test]
    fn join_date_column_anywhere_quoted_with_bom() {
        let mut f = base();
        f.insert("profile.csv".into(), b"\xEF\xBB\xBFEmail Address,\"Bio\",Date Joined,Website\n\"a@b.invalid\",\"likes, commas\",2021-03-09,x\n".to_vec());
        assert_eq!(crate::dates::iso(parse_files(&f, None).unwrap().joined.unwrap()), "2021-03-09");
    }

    #[test]
    fn missing_or_unusable_profile_means_no_join_date() {
        for body in [
            &b"Username,Email Address\nsomeone,secret@example.com\n"[..], // no Date Joined column
            b"Date Joined,Username\n",                                     // header only
            b"Date Joined,Username\nnot-a-date,someone\n",                // unparsable value
            b"\xff\xfe\x00garbage\n\xff",                                 // not UTF-8 / not CSV
            b"",
        ] {
            let mut f = base();
            f.insert("profile.csv".into(), body.to_vec());
            let e = parse_files(&f, None).expect("an unreadable profile.csv must not fail the export");
            assert!(e.joined.is_none());
            assert!(!format!("{e:?}").contains("secret@example.com"));
        }
        // no profile.csv at all
        assert!(parse_files(&base(), None).unwrap().joined.is_none());
    }

    #[test]
    fn watched_required_profile_optional() {
        let mut f = BTreeMap::new();
        assert!(parse_files(&f, None).is_err());
        f.insert("watched.csv".into(), b"Date,Name,Year,Letterboxd URI\n".to_vec());
        let e = parse_files(&f, None).unwrap();
        assert!(e.joined.is_none() && e.watched.is_empty());
    }
}
