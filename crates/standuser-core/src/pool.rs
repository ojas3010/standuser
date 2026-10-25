//! The personal pool and the name score (docs/DECISIONS.md, "Agreed v1 design (25 Oct)").
//!
//! Cool(n) and the per-template lexicon fit L(n, t) are computed offline (tools/naming) and shipped in names.json; this module
//! computes only the film affinity A(f), the score S, the median-Cool gate, the username-hash pick and the why line.
//!
//!   A(f) = 0.40 fav + 0.25 q_rating + 0.15 q_gap + 0.10 q_rewatch + 0.10 q_composer   (each q a mid-rank percentile among the
//!          user's own films; 0.5 when the film has no value)
//!   S(n) = 0.45 Cool + 0.35 F + 0.20 A(f(n)),   F = L for the user's ability template (1 core word, 0.5 ring word, else 0)
//!   pool = the 8 films of highest A that have candidates (equal A: higher crowd average, then lower TMDB id); a candidate below the pool's median Cool cannot win;
//!   among eligible candidates within 0.03 of the top S, FNV-1a of the lower-cased username picks.

use crate::ability::template_by_id;
use crate::lookup::{FilmNames, Lookup, NameCand};
use crate::parse::{boxd_code, Export};
use crate::rng::fnv1a;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};

pub const POOL: usize = 8;
pub const TIE_BAND: f64 = 0.03;
pub const W_COOL: f64 = 0.45;
pub const W_FIT: f64 = 0.35;
pub const W_AFF: f64 = 0.20;
const W_A: [f64; 5] = [0.40, 0.25, 0.15, 0.10, 0.10];

/// One of the user's films that the film table knows, with the five inputs of A(f).
#[derive(Debug, Clone)]
pub struct UserFilm {
    pub tmdb: u32,
    pub title: String,
    pub year: i32,
    pub rating: Option<f64>,
    pub crowd: Option<f64>,
    pub fav: bool,
    pub rewatch: u32,
    /// Best shrunk mean the user gives any composer of the film.
    pub composer: Option<f64>,
    pub a: f64,
    composers: Vec<u32>,
}

/// Mid-rank percentile of `v` among `vals` (which contain it): (below + half of equal) / n.
pub fn percentile(v: f64, vals: &[f64]) -> f64 {
    let n = vals.len() as f64;
    let below = vals.iter().filter(|x| **x < v).count() as f64;
    let equal = vals.iter().filter(|x| **x == v).count() as f64;
    (below + 0.5 * equal) / n
}

fn q(v: Option<f64>, vals: &[f64]) -> f64 {
    v.map_or(0.5, |v| percentile(v, vals))
}

/// The user's films in the film table (watched or rated, runtime filter as in the stats), with A(f), best first.
pub fn user_films(ex: &Export, lk: &Lookup) -> Vec<UserFilm> {
    let fav: HashSet<&str> = ex.favorites.iter().map(String::as_str).collect();
    let ok_runtime = |m: &crate::lookup::FilmMeta| m.runtime == 0 || m.runtime >= crate::stats::MIN_RUNTIME;
    let mut by: BTreeMap<u32, UserFilm> = BTreeMap::new();
    let mut touch = |uri: &str, name: &str, year: i32, rating: Option<f64>| {
        let code = boxd_code(uri);
        let Some(m) = lk.find(code, name, year) else { return };
        if !ok_runtime(m) {
            return;
        }
        let e = by.entry(m.tmdb_id).or_insert_with(|| UserFilm { tmdb: m.tmdb_id, title: name.to_string(), year, rating: None, crowd: m.lb_avg, fav: false, rewatch: 0, composer: None, a: 0.0, composers: m.composers.clone() });
        if rating.is_some() {
            e.rating = rating;
        }
        if code.is_some_and(|c| fav.contains(c)) {
            e.fav = true;
        }
    };
    for r in &ex.ratings {
        touch(&r.uri, &r.name, r.year, Some(r.rating));
    }
    for w in &ex.watched {
        touch(&w.uri, &w.name, w.year, None);
    }
    for d in ex.diary.iter().filter(|d| d.rewatch) {
        if let Some(m) = lk.find(None, &d.name, d.year) {
            if let Some(f) = by.get_mut(&m.tmdb_id) {
                f.rewatch += 1;
            }
        }
    }
    // shrunk composer means over the user's rated films: (sum + 3 mu) / (n + 3)
    let mu = if ex.ratings.is_empty() { 0.0 } else { ex.ratings.iter().map(|r| r.rating).sum::<f64>() / ex.ratings.len() as f64 };
    let mut sums: HashMap<u32, (f64, usize)> = HashMap::new();
    for f in by.values_mut() {
        f.composers.sort_unstable();
        f.composers.dedup();
        if let Some(r) = f.rating {
            for c in &f.composers {
                let e = sums.entry(*c).or_insert((0.0, 0));
                e.0 += r;
                e.1 += 1;
            }
        }
    }
    let m_c = |c: u32| sums.get(&c).map(|(s, n)| (s + 3.0 * mu) / (*n as f64 + 3.0));
    for f in by.values_mut() {
        f.composer = f.composers.iter().filter_map(|c| m_c(*c)).max_by(|a, b| a.total_cmp(b));
    }
    let mut v: Vec<UserFilm> = by.into_values().collect();
    let rated: Vec<f64> = v.iter().filter_map(|f| f.rating).collect();
    let gaps: Vec<f64> = v.iter().filter_map(|f| f.rating.zip(f.crowd).map(|(r, c)| r - c)).collect();
    let rew: Vec<f64> = v.iter().map(|f| f.rewatch as f64).collect();
    let comp: Vec<f64> = v.iter().filter_map(|f| f.composer).collect();
    for f in v.iter_mut() {
        let qs = [if f.fav { 1.0 } else { 0.0 }, q(f.rating, &rated), q(f.rating.zip(f.crowd).map(|(r, c)| r - c), &gaps), percentile(f.rewatch as f64, &rew), q(f.composer, &comp)];
        f.a = qs.iter().zip(W_A).map(|(x, w)| x * w).sum();
    }
    // equal A (a user with no ratings, favourites or rewatches ties everywhere): the higher crowd average first, a film without one
    // last, then the lower TMDB id (spec stage 3 order)
    let crowd = |f: &UserFilm| f.crowd.unwrap_or(f64::NEG_INFINITY);
    v.sort_by(|a, b| b.a.total_cmp(&a.a).then_with(|| crowd(b).total_cmp(&crowd(a))).then(a.tmdb.cmp(&b.tmdb)));
    v
}

/// The words of a candidate name, split the way tools/naming/lexicons.py `name_words` splits them: runs of ASCII letters, with an
/// apostrophe kept inside a word (WOODY'S is one word). The lexicon codes index this list.
pub fn name_words(name: &str) -> Vec<&str> {
    let b = name.as_bytes();
    let (mut out, mut i) = (vec![], 0);
    while i < b.len() {
        if !b[i].is_ascii_alphabetic() {
            i += 1;
            continue;
        }
        let start = i;
        while i < b.len() && (b[i].is_ascii_alphabetic() || (b[i] == b'\'' && i + 1 < b.len() && b[i + 1].is_ascii_alphabetic())) {
            i += 1;
        }
        out.push(&name[start..i]);
    }
    out
}

/// L(n, t) and the index of the matched word, from the candidate's lexicon codes.
pub fn fit(c: &NameCand, template: u8) -> (f64, Option<usize>) {
    c.codes
        .iter()
        .find(|code| *code / 16 == template as u16)
        .map_or((0.0, None), |code| (if code % 2 == 1 { 1.0 } else { 0.5 }, Some(((code / 2) % 8) as usize)))
}

#[derive(Debug, Clone, Serialize)]
pub struct Scored {
    pub name: String,
    pub film: String,
    pub year: i32,
    pub track: u16,
    pub source: String,
    pub s: f64,
    pub cool: f64,
    pub f: f64,
    pub a: f64,
    pub eligible: bool,
    pub picked: bool,
    /// The source film is one of the user's favourites (report only).
    pub fav: bool,
    /// The word of the name that matched the ability lexicon, and whether it is a "core" or "ring" word (report only).
    pub matched: Option<String>,
    pub fit_kind: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FirstTrack {
    pub name: String,
    pub film: String,
    pub track: u16,
}

#[derive(Debug, Clone)]
pub struct Picked {
    pub name: String,
    pub why: String,
    /// The five best eligible candidates by S, the pick among them marked.
    pub top: Vec<Scored>,
    /// What the first-track rule would name the Stand: the first surviving track of the pool's highest-affinity film.
    pub first_track: Option<FirstTrack>,
    pub pool_films: usize,
    pub pool_cands: usize,
    pub median_cool: f64,
}

struct Row<'a> {
    film: &'a UserFilm,
    names: &'a FilmNames,
    cand: &'a NameCand,
    s: f64,
    cool: f64,
    f: f64,
    word: Option<usize>,
}

fn r4(x: f64) -> f64 {
    (x * 1e4).round() / 1e4
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 }
}

/// The pool's pick for a user with ability template `template`, or None when no film of theirs has candidates.
pub fn pick(ex: &Export, lk: &Lookup, template: u8, username: &str) -> Option<Picked> {
    pick_top(ex, lk, template, username, 5)
}

/// `pick` listing the `n` best eligible candidates instead of five (the pick itself does not depend on `n`).
pub fn pick_top(ex: &Export, lk: &Lookup, template: u8, username: &str, n: usize) -> Option<Picked> {
    if lk.names.is_empty() {
        return None;
    }
    let films = user_films(ex, lk);
    let pool: Vec<&UserFilm> = films.iter().filter(|f| lk.names.get(&f.tmdb).is_some_and(|n| !n.cands.is_empty())).take(POOL).collect();
    if pool.is_empty() {
        return None;
    }
    let mut rows: Vec<Row> = vec![];
    for f in &pool {
        let names = &lk.names[&f.tmdb];
        for c in &names.cands {
            let (l, word) = fit(c, template);
            let cool = c.cool as f64 / 100.0;
            rows.push(Row { film: f, names, cand: c, s: r4(W_COOL * cool + W_FIT * l + W_AFF * f.a), cool, f: l, word });
        }
    }
    let med = median(&mut rows.iter().map(|r| r.cool).collect::<Vec<_>>());
    let order = |a: &usize, b: &usize| {
        let (x, y) = (&rows[*a], &rows[*b]);
        y.s.total_cmp(&x.s).then_with(|| x.cand.name.cmp(&y.cand.name)).then(x.film.tmdb.cmp(&y.film.tmdb)).then(x.cand.track.cmp(&y.cand.track))
    };
    let mut elig: Vec<usize> = (0..rows.len()).filter(|i| rows[*i].cool >= med).collect();
    elig.sort_by(order);
    let top_s = rows[elig[0]].s;
    let band: Vec<usize> = elig.iter().copied().filter(|i| rows[*i].s >= top_s - TIE_BAND - 1e-9).collect();
    let win = band[(fnv1a(username) % band.len() as u64) as usize];

    let scored = |i: usize| {
        let r = &rows[i];
        Scored {
            name: r.cand.name.clone(),
            film: r.film.title.clone(),
            year: r.film.year,
            track: r.cand.track,
            source: if r.cand.src.is_empty() { r.cand.name.clone() } else { r.cand.src.clone() },
            s: r.s,
            cool: r.cool,
            f: r.f,
            a: r5(r.film.a),
            eligible: true,
            picked: i == win,
            fav: r.film.fav,
            matched: r.word.and_then(|k| name_words(&r.cand.name).get(k).map(|w| w.to_lowercase())),
            fit_kind: if r.f >= 1.0 { "core" } else if r.f > 0.0 { "ring" } else { "" }.to_string(),
        }
    };
    let mut top: Vec<Scored> = elig.iter().take(n).map(|i| scored(*i)).collect();
    if !top.iter().any(|t| t.picked) {
        top.pop();
        top.push(scored(win));
    }
    let first_track = rows
        .iter()
        .filter(|r| std::ptr::eq(r.film, pool[0]))
        .min_by_key(|r| (r.cand.track, r.cand.name.clone()))
        .map(|r| FirstTrack { name: r.cand.name.clone(), film: r.film.title.clone(), track: r.cand.track });
    let w = &rows[win];
    let why = why_line(w, &rows.iter().map(|r| (r.s, r.cool, r.f, r.film.tmdb, r.film.a)).collect::<Vec<_>>(), pool.len(), template);
    Some(Picked { name: w.cand.name.clone(), why, top, first_track, pool_films: pool.len(), pool_cands: rows.len(), median_cool: med })
}

fn r5(x: f64) -> f64 {
    (x * 1e5).round() / 1e5
}

// ---------------------------------------------------------------- why line
/// Words of the why template itself; every other number or capitalised word must come from a slot.
const TEMPLATE_WORDS: [&str; 5] = ["Why", "Favourite", "Films", "Cool", "Its"];

fn trim_tok(t: &str) -> &str {
    let t = t.trim_matches(|c: char| !c.is_alphanumeric());
    t.strip_suffix("'s").or_else(|| t.strip_suffix("’s")).unwrap_or(t)
}

/// True when every token of `text` that has a digit or starts with a capital letter is a template word or a token of a slot value.
pub fn slots_ok(text: &str, slots: &[String]) -> bool {
    let mut allowed: HashSet<&str> = TEMPLATE_WORDS.iter().copied().collect();
    for s in slots {
        for t in s.split_whitespace() {
            allowed.insert(trim_tok(t));
        }
    }
    text.split_whitespace().map(trim_tok).filter(|t| !t.is_empty()).all(|t| {
        let flagged = t.chars().any(|c| c.is_ascii_digit()) || t.chars().next().is_some_and(char::is_uppercase);
        !flagged || allowed.contains(t)
    })
}

fn why_line(w: &Row, all: &[(f64, f64, f64, u32, f64)], pool_films: usize, template: u8) -> String {
    let ability = template_by_id(template).name;
    let name = &w.cand.name;
    let film = &w.film.title;
    let src = if w.cand.src.is_empty() { name.clone() } else { w.cand.src.clone() };
    let how = match w.cand.kind {
        't' => "tail of",
        'c' => "one cue of",
        's' => "trimmed from",
        _ => "verbatim:",
    };
    // the artist and the score / soundtrack flag are those of the candidate's own album: a film can have several
    let album = &w.names.albums[w.cand.album];
    let kind = if album.score { "score" } else { "soundtrack" };
    let year = w.film.year.to_string();
    let track = w.cand.track.to_string();
    let rating = w.film.rating.map(|r| format!("{r:.1}"));
    let mut slots: Vec<String> = vec![name.clone(), src.clone(), film.clone(), year.clone(), track.clone(), ability.to_string()];
    let place = if album.artist.is_empty() || album.artist == "Various Artists" {
        format!("track {track} of the {film} {kind} ({year})")
    } else {
        slots.push(album.artist.clone());
        format!("track {track} of {}'s {film} {kind} ({year})", album.artist)
    };
    let mut origin = if w.film.fav { format!("{film} is one of your Favourite Films") } else { format!("{film} is one of your highest-affinity films") };
    if let Some(r) = &rating {
        origin.push_str(&format!(" ({r}★)"));
        slots.push(r.clone());
    }
    // the cited reason is the largest term of S
    let terms = [W_COOL * w.cool, W_FIT * w.f, W_AFF * w.film.a];
    let top = if terms[0] >= terms[1] && terms[0] >= terms[2] { 0 } else if terms[1] >= terms[2] { 1 } else { 2 };
    let n = all.len();
    let reason = match top {
        1 => {
            let rank = 1 + all.iter().filter(|r| r.2 > w.f).count();
            let word = w.word.and_then(|i| name_words(&w.cand.name).get(i).map(|x| x.to_lowercase())).unwrap_or_default();
            let verb = if w.f >= 1.0 { "fits" } else { "is close to" };
            slots.extend([rank.to_string(), n.to_string(), word.clone()]);
            format!("\"{word}\" {verb} your {ability} ability; fit rank {rank} of {n}.")
        }
        0 => {
            let rank = 1 + all.iter().filter(|r| r.1 > w.cool).count();
            let c = format!("{:.2}", w.cool);
            slots.extend([rank.to_string(), n.to_string(), c.clone()]);
            format!("Its Cool score of {c} ranks {rank} of {n} in your pool.")
        }
        _ => {
            let mut films: Vec<(u32, f64)> = all.iter().map(|r| (r.3, r.4)).collect();
            films.sort_by(|a, b| a.0.cmp(&b.0));
            films.dedup_by_key(|x| x.0);
            let rank = 1 + films.iter().filter(|x| x.1 > w.film.a).count();
            slots.extend([rank.to_string(), pool_films.to_string()]);
            format!("{film} ranks {rank} of {pool_films} films in your pool by affinity.")
        }
    };
    let text = format!("Why 「{name}」: {how} \"{src}\", {place}; {origin}. {reason}");
    if slots_ok(&text.replace('「', " ").replace('」', " "), &slots) {
        text
    } else {
        // a slot the dictionary does not hold (should not happen): fall back to the bare facts
        format!("Why 「{name}」: track {track} of the {film} {kind} ({year}).")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{DiaryEntry, Rating, Watched};

    #[test]
    fn name_words_keep_apostrophes_inside_a_word_and_split_on_everything_else() {
        assert_eq!(name_words("WOODY'S GONE"), vec!["WOODY'S", "GONE"]);
        assert_eq!(name_words("DON'T PANIC!"), vec!["DON'T", "PANIC"]);
        assert_eq!(name_words("T1000 TERMINATED"), vec!["T", "TERMINATED"]);
        assert_eq!(name_words("SPIDER-MAN 'ROUND"), vec!["SPIDER", "MAN", "ROUND"]);
        assert_eq!(name_words("ROCKIN' ON"), vec!["ROCKIN", "ON"], "a trailing apostrophe is not inside a word");
        assert!(name_words("123 !!").is_empty());
    }

    #[test]
    fn the_cited_word_is_the_one_the_code_points_at_even_after_an_apostrophe() {
        // WOODY'S GONE: word 1 is GONE (core hit for template 3)
        let mut lk = table();
        lk.load_names(&names_json(&[(1, "", "o", &[("WOODY'S GONE", 60, 2, "v", &[(3 << 4) | (1 << 1) | 1]), ("PAD ONE", 10, 3, "v", &[]), ("PAD TWO", 10, 4, "v", &[])])])).unwrap();
        let p = pick(&export(&[(0, 4.0)], &[0]), &lk, 3, "u").unwrap();
        assert_eq!(p.name, "WOODY'S GONE");
        assert_eq!(p.top[0].matched.as_deref(), Some("gone"));
        assert!(p.why.contains("\"gone\" fits your Hora Rush ability"), "{}", p.why);
    }

    #[test]
    fn percentile_is_mid_rank() {
        let v = [1.0, 2.0, 2.0, 3.0];
        assert_eq!(percentile(1.0, &v), 0.125);
        assert_eq!(percentile(2.0, &v), 0.5);
        assert_eq!(percentile(3.0, &v), 0.875);
        assert_eq!(percentile(7.0, &[7.0]), 0.5);
    }

    /// Eight films A..H (TMDB 1..8), then the table rows; names for films 1..=8 unless `names` says otherwise.
    fn table() -> Lookup {
        let mut rows = vec![];
        for (i, c) in "ABCDEFGHIJ".chars().enumerate() {
            rows.push(format!(r#"["{}|{}","",{},{},100,"en","US","","{}",{},null,100000]"#, c.to_lowercase(), 2000 + i, i + 1, 2000 + i, if i % 2 == 0 { "9" } else { "8" }, 3.5));
        }
        Lookup::from_json(&format!(r#"{{"people":{{"9":"Nine","8":"Eight"}},"films":[{}]}}"#, rows.join(","))).unwrap()
    }

    fn export(rated: &[(usize, f64)], favs: &[usize]) -> Export {
        let mut ex = Export::default();
        ex.username = Some("someone".into());
        for (i, c) in "ABCDEFGHIJ".chars().enumerate() {
            let uri = format!("https://boxd.it/c{i}");
            ex.watched.push(Watched { day: 19000 + i as i64, name: c.to_string(), year: 2000 + i as i32, uri: uri.clone() });
            if let Some((_, r)) = rated.iter().find(|(j, _)| *j == i) {
                ex.ratings.push(Rating { name: c.to_string(), year: 2000 + i as i32, rating: *r, uri });
            }
        }
        ex.favorites = favs.iter().map(|i| format!("c{i}")).collect();
        ex
    }

    #[test]
    fn favourites_and_ratings_rank_the_pool() {
        let lk = table();
        let ex = export(&[(0, 5.0), (1, 1.0), (2, 3.0)], &[1]);
        let v = user_films(&ex, &lk);
        assert_eq!(v.len(), 10);
        let a = |t: u32| v.iter().find(|f| f.tmdb == t).unwrap().a;
        assert!(a(2) > a(1), "a favourite rated 1 star outranks a 5-star non-favourite: fav carries 0.40");
        assert!(a(1) > a(3), "5 stars beats 3 stars when neither is a favourite");
        assert!(a(4) > 0.0 && a(4) < a(3), "unrated films sit at the neutral 0.5 on the rating terms");
        assert_eq!(v[0].tmdb, 2, "sorted by A, best first");
        assert!(v.windows(2).all(|w| w[0].a >= w[1].a));
    }

    #[test]
    fn equal_affinity_is_broken_by_crowd_average_then_tmdb_id() {
        // five films A..E = TMDB 1..5 with crowd averages 3.0, 4.2, 3.6, 4.2 and none
        let avgs = ["3.0", "4.2", "3.6", "4.2", "null"];
        let rows: Vec<String> = "ABCDE".chars().enumerate().map(|(i, c)| format!(r#"["{}|{}","",{},{},100,"en","US","","",{},null,100000]"#, c.to_lowercase(), 2000 + i, i + 1, 2000 + i, avgs[i])).collect();
        let mut lk = Lookup::from_json(&format!(r#"{{"films":[{}]}}"#, rows.join(","))).unwrap();
        // a user with no ratings, no favourites and no rewatches: every film has the same A
        let ex = export(&[], &[]);
        let v = user_films(&ex, &lk);
        assert!(v.iter().all(|f| f.a == v[0].a), "setup: A is equal for all: {:?}", v.iter().map(|f| f.a).collect::<Vec<_>>());
        assert_eq!(v.iter().map(|f| f.tmdb).collect::<Vec<_>>(), vec![2, 4, 3, 1, 5], "4.2 (TMDB 2 before 4), 3.6, 3.0, then no crowd average");
        // the pool starts with the same film, so the first-track comparison uses it
        let one: &[(&str, u8, u16, &str, &[u16])] = &[("X", 50, 1, "v", &[])];
        let spec: Vec<(u32, &str, &str, &[(&str, u8, u16, &str, &[u16])])> = (1..=5).map(|i| (i as u32, "", "o", one)).collect();
        lk.load_names(&names_json(&spec)).unwrap();
        let p = pick(&ex, &lk, 1, "u").unwrap();
        assert_eq!(p.first_track.unwrap().film, "B");
        // the tie-break only orders equal A: a film the user rates 5 stars over a 1-star one comes first despite the lowest crowd average
        let ex = export(&[(0, 5.0), (1, 1.0)], &[]);
        assert_eq!(user_films(&ex, &lk)[0].tmdb, 1);
    }

    #[test]
    fn rewatch_counts_come_from_the_diary() {
        let lk = table();
        let mut ex = export(&[(0, 4.0), (1, 4.0)], &[]);
        for _ in 0..2 {
            ex.diary.push(DiaryEntry { log_day: 19000, name: "B".into(), year: 2001, view_day: 19000, rewatch: true });
        }
        ex.diary.push(DiaryEntry { log_day: 19000, name: "A".into(), year: 2000, view_day: 19000, rewatch: false });
        let v = user_films(&ex, &lk);
        assert_eq!(v.iter().find(|f| f.tmdb == 2).unwrap().rewatch, 2);
        assert_eq!(v.iter().find(|f| f.tmdb == 1).unwrap().rewatch, 0);
        assert!(v.iter().find(|f| f.tmdb == 2).unwrap().a > v.iter().find(|f| f.tmdb == 1).unwrap().a);
    }

    /// Films with one album each (artist, flag "c" score / "v" various / "o" other) holding all the candidates.
    fn names_json(films: &[(u32, &str, &str, &[(&str, u8, u16, &str, &[u16])])]) -> String {
        let mut m = serde_json::Map::new();
        for (id, artist, flag, cands) in films {
            let cs: Vec<serde_json::Value> = cands.iter().map(|(n, c, t, k, codes)| serde_json::json!([n, c, t, k, "", codes, 0])).collect();
            m.insert(id.to_string(), serde_json::json!([[[artist, flag]], cs]));
        }
        serde_json::json!({"v": 2, "names": m}).to_string()
    }

    #[test]
    fn candidates_below_the_pool_median_cool_cannot_win() {
        let mut lk = table();
        // film 1 (highest A): one high-affinity but dull candidate with a perfect fit; others are cool but fit nothing
        lk.load_names(&names_json(&[
            (1, "Nine", "c", &[("DULL BUT FITTING", 50, 1, "v", &[(3 << 4) | 1]), ("COOL ONE", 90, 2, "v", &[]), ("COOL TWO", 80, 3, "v", &[]), ("COOL THREE", 70, 4, "v", &[])]),
        ])).unwrap();
        let ex = export(&[(0, 5.0)], &[0]);
        let p = pick(&ex, &lk, 3, "someone").unwrap();
        // without the gate DULL BUT FITTING would score 0.45*0.10 + 0.35 + 0.2*A, far above the cool ones
        let a = user_films(&ex, &lk)[0].a;
        assert!(r4(0.45 * 0.5 + 0.35 + 0.2 * a) > r4(0.45 * 0.9 + 0.2 * a) + TIE_BAND, "setup: the gate is what decides this");
        assert_ne!(p.name, "DULL BUT FITTING");
        assert_eq!(p.median_cool, 0.75);
        assert!(p.top.iter().all(|t| t.cool >= 0.75));
        assert_eq!(p.pool_cands, 4);
    }

    #[test]
    fn the_username_hash_picks_among_scores_within_the_band_and_is_stable() {
        let mut lk = table();
        let cands: Vec<(&str, u8, u16, &str, &[u16])> = vec![("ALPHA", 80, 1, "v", &[]), ("BETA", 80, 2, "v", &[]), ("GAMMA", 80, 3, "v", &[]), ("DELTA", 80, 4, "v", &[]), ("LOW", 10, 5, "v", &[])];
        lk.load_names(&names_json(&[(1, "", "o", &cands)])).unwrap();
        let ex = export(&[(0, 4.0)], &[]);
        let mut seen = HashSet::new();
        for u in ["ann", "bob", "cyd", "dee", "eve", "fay", "gil", "hal", "ivy", "jon", "kay", "lou"] {
            let mut e = ex.clone();
            e.username = Some(u.into());
            let p1 = pick(&e, &lk, 1, u).unwrap();
            let p2 = pick(&e, &lk, 1, u).unwrap();
            assert_eq!(p1.name, p2.name, "same user, same name");
            assert_ne!(p1.name, "LOW");
            seen.insert(p1.name);
        }
        assert!(seen.len() >= 2, "different usernames split the tie: {seen:?}");
        // a clear winner is never overridden by the hash: 0.03 is the whole band
        let cands: Vec<(&str, u8, u16, &str, &[u16])> = vec![("TOP", 90, 1, "v", &[]), ("NEAR", 80, 2, "v", &[]), ("OTHER", 70, 3, "v", &[])];
        lk.load_names(&names_json(&[(1, "", "o", &cands)])).unwrap();
        for u in ["ann", "bob", "cyd", "dee"] {
            let mut e = ex.clone();
            e.username = Some(u.into());
            assert_eq!(pick(&e, &lk, 1, u).unwrap().name, "TOP");
        }
    }

    #[test]
    fn the_pool_is_the_eight_best_films_that_have_candidates() {
        let mut lk = table();
        // film 1 has no candidates; films 2..=10 do (9 films): the 9th-ranked stays out
        let one: &[(&str, u8, u16, &str, &[u16])] = &[("X", 50, 1, "v", &[])];
        let spec: Vec<(u32, &str, &str, &[(&str, u8, u16, &str, &[u16])])> = (2..=10).map(|i| (i as u32, "", "o", one)).collect();
        lk.load_names(&names_json(&spec)).unwrap();
        let ex = export(&[(0, 5.0)], &[0]);
        let p = pick(&ex, &lk, 1, "u").unwrap();
        assert_eq!(p.pool_films, 8);
        assert_eq!(p.pool_cands, 8);
        // no film of the user's has candidates: no pick, so the composer route runs
        let mut empty = table();
        empty.load_names(&names_json(&[(99, "", "o", one)])).unwrap();
        assert!(pick(&ex, &empty, 1, "u").is_none());
        assert!(pick(&ex, &table(), 1, "u").is_none());
    }

    #[test]
    fn why_line_cites_the_largest_term_and_only_slot_values() {
        let mut lk = table();
        lk.load_names(&names_json(&[(1, "Nine Nine", "c", &[("WOUNDED HUNTER", 20, 11, "v", &[(3 << 4) | (1 << 1) | 1]), ("FILLER ONE", 20, 2, "v", &[]), ("FILLER TWO", 20, 3, "v", &[])])])).unwrap();
        let ex = export(&[(0, 4.5)], &[0]);
        let p = pick(&ex, &lk, 3, "someone").unwrap();
        assert_eq!(p.name, "WOUNDED HUNTER");
        assert!(p.why.starts_with("Why 「WOUNDED HUNTER」: verbatim:"), "{}", p.why);
        assert!(p.why.contains("track 11 of Nine Nine's A score (2000)"), "{}", p.why);
        assert!(p.why.contains("one of your Favourite Films (4.5★)"), "{}", p.why);
        assert!(p.why.contains("\"hunter\" fits your Hora Rush ability; fit rank 1 of 3."), "{}", p.why);
        let slots: Vec<String> = ["WOUNDED HUNTER", "A", "2000", "11", "Hora Rush", "Nine Nine", "4.5", "hunter", "1", "3"].map(String::from).to_vec();
        assert!(slots_ok(&p.why.replace(['「', '」'], " "), &slots));
    }

    #[test]
    fn the_why_line_takes_artist_and_kind_from_the_candidates_own_album() {
        // 12 Years a Slave: LETTER WRITING is on Hans Zimmer's score (album 0), FREIGHT TRAIN on the songs album by various artists (album 1)
        let names = |winner: &str| {
            let cool = |n: &str| if n == winner { 90 } else { 10 };
            serde_json::json!({"v": 2, "names": {"1": [[["Hans Zimmer", "c"], ["Various Artists", "v"]], [
                ["LETTER WRITING", cool("LETTER WRITING"), 17, "v", "", [], 0], ["FREIGHT TRAIN", cool("FREIGHT TRAIN"), 3, "v", "", [], 1], ["FILLER", 10, 4, "v", "", [], 1]]]}}).to_string()
        };
        let ex = export(&[(0, 4.0)], &[0]);
        let mut lk = table();
        lk.load_names(&names("LETTER WRITING")).unwrap();
        let p = pick(&ex, &lk, 1, "u").unwrap();
        assert_eq!(p.name, "LETTER WRITING");
        assert!(p.why.contains("track 17 of Hans Zimmer's A score (2000)"), "{}", p.why);
        lk.load_names(&names("FREIGHT TRAIN")).unwrap();
        let p = pick(&ex, &lk, 1, "u").unwrap();
        assert_eq!(p.name, "FREIGHT TRAIN");
        assert!(p.why.contains("track 3 of the A soundtrack (2000)"), "{}", p.why);
        assert!(!p.why.contains("Hans Zimmer") && !p.why.contains("A score"), "not credited to the other album's artist: {}", p.why);
    }

    #[test]
    fn every_reason_branch_passes_its_own_slot_check_and_is_not_the_bare_fallback() {
        let mut lk = table();
        // cool wins: a high-Cool candidate with no fit; affinity wins: low Cool and no fit on a high-A film
        lk.load_names(&names_json(&[(1, "Nine", "c", &[("BRIGHT TRACK", 95, 4, "t", &[]), ("PLAIN TRACK", 90, 2, "c", &[])])])).unwrap();
        let ex = export(&[(0, 4.5)], &[0]);
        let p = pick(&ex, &lk, 5, "u").unwrap();
        assert!(p.why.contains("Its Cool score of 0.9"), "{}", p.why);
        assert!(p.why.contains("tail of") || p.why.contains("one cue of"), "{}", p.why);
        assert!(!p.why.ends_with("(2000)."), "not the bare fallback: {}", p.why);
        // affinity is the largest term when Cool and F are both small
        let mut lk = table();
        lk.load_names(&names_json(&[(1, "", "o", &[("QUIET ONE", 2, 1, "s", &[]), ("QUIET TWO", 1, 2, "v", &[])])])).unwrap();
        let p = pick(&export(&[(0, 5.0)], &[0]), &lk, 5, "u").unwrap();
        assert!(p.why.contains("ranks 1 of 1 films in your pool by affinity."), "{}", p.why);
        assert!(p.why.contains("trimmed from") || p.why.contains("verbatim:"), "{}", p.why);
    }

    #[test]
    fn the_slot_check_rejects_a_number_or_a_proper_noun_that_no_slot_holds() {
        let slots: Vec<String> = ["Blade Runner", "1982", "4.5"].map(String::from).to_vec();
        assert!(slots_ok("Why: Blade Runner (1982) is one of your Favourite Films (4.5★).", &slots));
        assert!(!slots_ok("Why: Blade Runner (1983) is one of your Favourite Films.", &slots), "a year no slot holds");
        assert!(!slots_ok("Why: Blade Runner is Ridley Scott's best.", &slots), "a proper noun no slot holds");
        assert!(!slots_ok("Why: Blade Runner was seen 12 times.", &slots), "a number no slot holds");
        assert!(slots_ok("Why: the track of Blade Runner's score.", &slots), "a possessive of a slot word is fine");
    }

    #[test]
    fn first_track_is_the_first_surviving_track_of_the_top_film() {
        let mut lk = table();
        lk.load_names(&names_json(&[(1, "", "o", &[("LATE", 90, 9, "v", &[]), ("EARLY", 30, 2, "v", &[]), ("MID", 60, 5, "v", &[])]), (2, "", "o", &[("OTHER", 99, 1, "v", &[])])])).unwrap();
        let ex = export(&[(0, 5.0), (1, 2.0)], &[0]);
        let p = pick(&ex, &lk, 1, "u").unwrap();
        let ft = p.first_track.unwrap();
        assert_eq!((ft.name.as_str(), ft.track), ("EARLY", 2));
    }
}
