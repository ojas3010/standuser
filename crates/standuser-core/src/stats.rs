//! The six stat values and the facts the ability text quotes. Spec: "Letterboxd JoJo Stand stats 2".
//!
//! Power   = -(shrunk mean of own - crowd), lambda 6
//! Speed   = films per active month on the clean timeline
//! Range   = rarefied effective number of production countries (20 seeded draws of 50 films)
//! Stamina = share of complete months with a film
//! Precision = difficulty-adjusted pairwise agreement (DPA)
//! Potential = Good-Turing share of director credits seen once

use crate::grading::{grade_value, grade_with, make_complete, make_inf, Cuts, Grade, Status};
use crate::lookup::{FilmMeta, Lookup};
use crate::parse::{boxd_code, Export};
use crate::rng::SplitMix64;
use crate::timeline::Timeline;
use serde::Serialize;
use std::collections::BTreeMap;

pub const GAP_BINS: usize = 36;
pub const RANGE_DRAWS: usize = 20;
pub const RANGE_SAMPLE: usize = 50;
pub const RANGE_SEED: u64 = 5;
pub const POTENTIAL_DRAWS: usize = 20;
pub const POTENTIAL_SAMPLE: usize = 60;
pub const POTENTIAL_SEED: u64 = 6;
/// Letterboxd's own "shorts" cut: films of 40 minutes or less are ignored for countries and directors.
pub const MIN_RUNTIME: u16 = 41;

#[derive(Debug, Clone, Serialize)]
pub struct RatedFilm {
    pub name: String,
    pub year: i32,
    pub own: f64,
    pub crowd: f64,
    /// Letterboxd ratings behind the crowd mean.
    pub n: u64,
    pub uri: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Facts {
    pub b: f64,
    pub n_elig: usize,
    pub n_rated: usize,
    pub n_rated_missing: usize,
    pub crowd_source: String,
    pub pace: f64,
    pub peak_n: usize,
    pub peak_month: String,
    pub months: i64,
    pub active: usize,
    pub streak: usize,
    pub start: String,
    pub silent: usize,
    pub silent_run: usize,
    pub d_range: f64,
    pub top_country: String,
    pub top_share: f64,
    pub n_films_cty: usize,
    pub pa25: f64,
    pub pairs: u64,
    pub dpa: f64,
    pub n1: usize,
    pub credits: usize,
    pub films_with_credits: usize,
    pub gt: f64,
    /// Good-Turing share on 20 seeded draws of 60 director-credited films (volume-matched; the Potential letter uses it).
    pub gt60: f64,
    /// Film-count band Potential was graded in ("60-100", "100-200", "200+").
    pub potential_band: String,
    pub top_dir: String,
    pub top_dir_n: usize,
    pub n_watched: usize,
    pub n_watched_in_table: usize,
    pub events: usize,
}

/// How much of the export a stat was graded on: `used` of `of` films, against the spec's minimum for the stat.
#[derive(Debug, Clone, Serialize)]
pub struct StatCoverage {
    pub stat: String,
    pub used: usize,
    pub of: usize,
    pub minimum: usize,
    pub meets_minimum: bool,
    /// What is being counted: "films", or "dated watches" for Speed and Stamina (bulk-import days excluded).
    pub unit: String,
    /// What is missing when the minimum is not met (None when it is met or only the count is short).
    pub shortfall: Option<String>,
}

impl StatCoverage {
    fn new(stat: &str, used: usize, of: usize, minimum: usize) -> Self {
        let unit = if matches!(stat, "SPD" | "STA") { "dated watches" } else { "films" };
        StatCoverage { stat: stat.into(), used, of, minimum, meets_minimum: used >= minimum, unit: unit.into(), shortfall: None }
    }

    /// Speed and Stamina: the same window and the same minimums as the grade itself (events inside the complete months).
    fn timeline(stat: &str, m: Option<&crate::timeline::DateMetrics>, of: usize) -> Self {
        let mut c = StatCoverage::new(stat, m.map_or(0, |m| m.events_in_window), of, 12);
        let Some(m) = m else {
            c.shortfall = Some("no complete month after your first clean watch".into());
            return c;
        };
        let (meets, shortfall) = if m.events_in_window < 12 {
            (false, Some("in complete months, needs 12".to_string()))
        } else if m.active < 6 {
            (false, Some(format!("in only {} active months, needs 6", m.active)))
        } else if stat == "STA" && m.months < 12 {
            (false, Some(format!("over only {} complete months, needs 12", m.months)))
        } else {
            (true, None)
        };
        c.meets_minimum = meets;
        c.shortfall = shortfall;
        c
    }

    /// "graded on 450 of 512 films", "224 of 533 dated watches" for the timeline stats, or what is missing when the
    /// minimum is not met.
    pub fn text(&self) -> String {
        if self.meets_minimum {
            return if self.unit == "dated watches" { format!("{} of {} dated watches", self.used, self.of) } else { format!("graded on {} of {} films", self.used, self.of) };
        }
        match &self.shortfall {
            Some(s) => format!("only {} of {} {} {s}", self.used, self.of, self.unit).replace("  ", " "),
            None => format!("only {} of {} {}, needs {}", self.used, self.of, self.unit, self.minimum),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Stats {
    pub power: Grade,
    pub speed: Grade,
    pub range: Grade,
    pub stamina: Grade,
    pub precision: Grade,
    pub potential: Grade,
    pub facts: Facts,
    pub rated: Vec<RatedFilm>,
    /// One example film per stat (Power, Speed, Range, Stamina, Precision, Potential) for the card's evidence lines.
    pub examples: [Option<String>; 6],
    /// Power, Speed, Range, Stamina, Precision, Potential.
    pub coverage: Vec<StatCoverage>,
}

impl Stats {
    pub fn grades(&self) -> [&Grade; 6] {
        [&self.power, &self.speed, &self.range, &self.stamina, &self.precision, &self.potential]
    }
}

/// Pair-bin lower edges 0.25, 0.30 ... 2.00.
pub fn gap_edge(k: usize) -> f64 {
    (25 + 5 * k) as f64 / 100.0
}

/// (hits, counts) per crowd-gap bin over film pairs whose crowd values differ by at least 0.25.
/// hit = 1 if concordant, 0.5 if the user tied. rv: ratings in half-stars (1..10); c: crowd means.
pub fn pair_bins(rv: &[i32], c: &[f64]) -> ([f64; GAP_BINS], [f64; GAP_BINS]) {
    let mut hits = [0.0; GAP_BINS];
    let mut n = [0.0; GAP_BINS];
    let len = c.len();
    for i in 0..len {
        for j in (i + 1)..len {
            let (hi, lo, rh, rl) = if c[i] >= c[j] { (c[i], c[j], rv[i], rv[j]) } else { (c[j], c[i], rv[j], rv[i]) };
            if !(lo <= hi - gap_edge(0)) {
                continue;
            }
            let mut k = (((hi - lo) - 0.25) / 0.05).floor().clamp(0.0, (GAP_BINS - 1) as f64) as usize;
            while k + 1 < GAP_BINS && lo <= hi - gap_edge(k + 1) {
                k += 1;
            }
            while k > 0 && !(lo <= hi - gap_edge(k)) {
                k -= 1;
            }
            n[k] += 1.0;
            hits[k] += if rh > rl { 1.0 } else if rh == rl { 0.5 } else { 0.0 };
        }
    }
    (hits, n)
}

pub fn dpa(hits: &[f64; GAP_BINS], n: &[f64; GAP_BINS], h: &[f64], pa_pop: f64) -> Option<f64> {
    let tot: f64 = n.iter().sum();
    if tot < 1.0 {
        return None;
    }
    let observed: f64 = hits.iter().sum();
    let expected: f64 = n.iter().zip(h).map(|(a, b)| a * b).sum();
    Some((observed - expected) / tot + pa_pop)
}

/// Effective number of countries: mean over seeded draws of exp(entropy) of the fractional country weights.
pub fn rarefied_range(films: &[&Vec<String>], size: usize, draws: usize, seed: u64) -> f64 {
    let mut rng = SplitMix64::new(seed);
    let mut total = 0.0;
    for _ in 0..draws {
        let idx = rng.sample(films.len(), size);
        let mut w: BTreeMap<&str, f64> = BTreeMap::new();
        for &i in &idx {
            let cs = films[i];
            for c in cs.iter() {
                *w.entry(c.as_str()).or_default() += 1.0 / cs.len() as f64;
            }
        }
        let mut h = 0.0;
        for v in w.values() {
            let p = v / size as f64;
            h -= p * p.ln();
        }
        total += h.exp();
    }
    total / draws as f64
}

/// Volume-matched Good-Turing: mean over seeded draws of (directors seen once) / (director credits) on `size` films.
pub fn rarefied_gt(films: &[Vec<u32>], size: usize, draws: usize, seed: u64) -> f64 {
    let mut rng = SplitMix64::new(seed);
    let mut total = 0.0;
    for _ in 0..draws {
        let mut cnt: BTreeMap<u32, usize> = BTreeMap::new();
        for i in rng.sample(films.len(), size) {
            for d in &films[i] {
                *cnt.entry(*d).or_default() += 1;
            }
        }
        let credits: usize = cnt.values().sum();
        total += cnt.values().filter(|&&c| c == 1).count() as f64 / credits as f64;
    }
    total / draws as f64
}

fn gap_table() -> Vec<f64> {
    #[derive(serde::Deserialize)]
    struct G {
        h: Vec<f64>,
    }
    serde_json::from_str::<G>(include_str!("../data/gap_h.json")).expect("gap_h.json").h
}

pub fn compute(ex: &Export, lk: &Lookup, tl: &Timeline, cuts: &Cuts) -> Stats {
    let mut f = Facts { n_watched: ex.watched.len(), events: tl.events.len(), ..Default::default() };

    // ---- join watched and rated films with the table
    let meta_of = |name: &str, year: i32, uri: &str| -> Option<&FilmMeta> { lk.find(boxd_code(uri), name, year) };
    f.n_watched_in_table = ex.watched.iter().filter(|w| meta_of(&w.name, w.year, &w.uri).is_some()).count();

    // ---- crowd: the Letterboxd displayed average, the one value every table film has; the cut-points are calibrated on it
    let min_n = cuts.crowd_min_ratings;
    let mut pool: Vec<(&crate::parse::Rating, &FilmMeta)> = Vec::new();
    let mut missing = 0usize;
    for r in &ex.ratings {
        match meta_of(&r.name, r.year, &r.uri) {
            Some(m) => pool.push((r, m)),
            None => missing += 1,
        }
    }
    f.crowd_source = "Letterboxd displayed average".to_string();
    let mut rated: Vec<RatedFilm> = Vec::new();
    for (r, m) in &pool {
        let n = m.lb_count.unwrap_or(0);
        if let (Some(crowd), true) = (m.lb_avg, n >= min_n) {
            rated.push(RatedFilm { name: r.name.clone(), year: r.year, own: r.rating, crowd, n, uri: boxd_code(&r.uri).unwrap_or("").to_string() });
        }
    }
    f.n_rated = ex.ratings.len();
    f.n_rated_missing = missing;
    f.n_elig = rated.len();

    // ---- Power and Precision
    let too_sparse = f.n_rated > 0 && missing * 2 > f.n_rated;
    let n = rated.len();
    let (power, precision);
    if ex.ratings.is_empty() {
        power = Grade::none("PWR", "no ratings in the export");
        precision = Grade::none("PRC", "no ratings in the export");
    } else if too_sparse || n < 20 {
        let why = if too_sparse { "crowd data missing for most of your rated films" } else { "fewer than 20 rated films with crowd data" };
        power = Grade::unknown("PWR", None, why);
        precision = Grade::unknown("PRC", None, why);
    } else {
        let d_sum: f64 = rated.iter().map(|x| x.own - x.crowd).sum();
        let shrunk = (d_sum + cuts.lambda * cuts.mu_b) / (n as f64 + cuts.lambda);
        // Power is the negated shrunk offset; the card's wording quotes the raw mean offset (own - crowd)
        let pw = -shrunk;
        power = grade_value(cuts, "PWR", pw, n < 30, &format!("{n} rated films with crowd data"));

        let rv: Vec<i32> = rated.iter().map(|x| (x.own * 2.0).round() as i32).collect();
        let c: Vec<f64> = rated.iter().map(|x| x.crowd).collect();
        let (hits, cnt) = pair_bins(&rv, &c);
        let pairs: f64 = cnt.iter().sum();
        f.pairs = pairs as u64;
        f.pa25 = if pairs > 0.0 { hits.iter().sum::<f64>() / pairs } else { 0.0 };
        match dpa(&hits, &cnt, &gap_table(), cuts.pa_pop) {
            Some(d) if n >= 50 && pairs >= 500.0 => {
                f.dpa = d;
                precision = grade_value(cuts, "PRC", d, n < 100, &format!("{n} rated films, {} pairs", f.pairs));
            }
            _ => precision = Grade::unknown("PRC", None, "needs 50 rated films with crowd data and 500 separated pairs"),
        }
    }
    if n > 0 {
        f.b = rated.iter().map(|x| x.own - x.crowd).sum::<f64>() / n as f64;
    }

    // ---- Speed and Stamina
    let (speed, stamina);
    match &tl.metrics {
        None => {
            let note = if tl.events.is_empty() { "no clean timeline events" } else { "no complete month after your first clean event" };
            if tl.events.is_empty() {
                speed = Grade::none("SPD", note);
                stamina = Grade::none("STA", note);
            } else {
                speed = Grade::unknown("SPD", None, note);
                stamina = Grade::unknown("STA", None, note);
            }
        }
        Some(m) => {
            f.pace = m.pace;
            f.peak_n = m.peak_n;
            f.peak_month = m.peak_month.clone();
            f.months = m.months;
            f.active = m.active;
            f.streak = m.streak;
            f.start = m.start.clone();
            f.silent = m.silent;
            f.silent_run = m.silent_run;
            let ev = m.events_in_window;
            speed = if ev < 12 || m.active < 6 {
                Grade::unknown("SPD", Some(m.pace), "needs 12 clean events in 6 active months")
            } else {
                let g = grade_value(cuts, "SPD", m.pace, true, &format!("{ev} events over {} months", m.months));
                if m.pace >= 26.2 && m.months >= 24 && ev >= 40 { make_inf(g) } else { g }
            };
            stamina = if m.months < 12 || m.active < 6 || ev < 12 {
                Grade::unknown("STA", Some(m.share), "needs 12 complete months with 6 active")
            } else {
                let g = grade_value(cuts, "STA", m.share, true, &format!("{} of {} months", m.active, m.months));
                if m.months >= 60 && m.active == m.months as usize { make_inf(g) } else { g }
            };
        }
    }

    // ---- Range and Potential (watched films, shorts dropped)
    let mut cty_films: Vec<&Vec<String>> = Vec::new();
    let mut cty_count: BTreeMap<String, usize> = BTreeMap::new();
    let mut dir_count: BTreeMap<u32, usize> = BTreeMap::new();
    let mut credit_films = 0usize;
    let mut dir_films: Vec<Vec<u32>> = Vec::new();
    for w in &ex.watched {
        let Some(m) = meta_of(&w.name, w.year, &w.uri) else { continue };
        if m.runtime != 0 && m.runtime < MIN_RUNTIME {
            continue;
        }
        if !m.countries.is_empty() {
            cty_films.push(&m.countries);
            for c in &m.countries {
                *cty_count.entry(c.clone()).or_default() += 1;
            }
        }
        if !m.directors.is_empty() {
            credit_films += 1;
            let mut seen = m.directors.clone();
            seen.sort();
            seen.dedup();
            for d in &seen {
                *dir_count.entry(*d).or_default() += 1;
            }
            dir_films.push(seen);
        }
    }
    f.n_films_cty = cty_films.len();
    let range = if ex.watched.is_empty() {
        Grade::none("RNG", "no watched films")
    } else if cty_films.len() < 20 {
        Grade::unknown("RNG", None, "needs 50 films with country data")
    } else {
        let size = RANGE_SAMPLE.min(cty_films.len());
        let d = rarefied_range(&cty_films, size, RANGE_DRAWS, RANGE_SEED);
        f.d_range = d;
        let top = cty_count.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))).map(|(c, n)| (c.clone(), *n));
        if let Some((c, n)) = top {
            f.top_country = lk.country_phrase(&c);
            f.top_share = n as f64 / cty_films.len() as f64;
        }
        if cty_films.len() < 50 {
            Grade::unknown("RNG", Some(d), "needs 50 films with country data")
        } else {
            let g = grade_value(cuts, "RNG", d, cty_films.len() < 100, &format!("{} films with country data", cty_films.len()));
            if d >= cuts.stat("RNG").inf_cut.unwrap_or(5.7) && cty_films.len() >= 150 { make_inf(g) } else { g }
        }
    };
    f.films_with_credits = credit_films;
    f.credits = dir_count.values().sum();
    f.n1 = dir_count.values().filter(|&&c| c == 1).count();
    f.gt = if f.credits > 0 { f.n1 as f64 / f.credits as f64 } else { 0.0 };
    if let Some((id, n)) = dir_count.iter().max_by(|a, b| a.1.cmp(b.1).then(lk.person(*b.0).cmp(&lk.person(*a.0)))) {
        f.top_dir = lk.person(*id);
        f.top_dir_n = *n;
    }
    // Potential: the letter comes from the volume-matched Good-Turing share (60 films), so a long history is not
    // marked down for volume alone; the raw share stays in the text. 完成 needs 1,000 credited films and the lowest band.
    let potential = if ex.watched.is_empty() {
        Grade::none("DEV", "no watched films")
    } else if credit_films < 40 {
        Grade::unknown("DEV", if credit_films > 0 { Some(f.gt) } else { None }, "needs 40 films with director credits")
    } else {
        let size = POTENTIAL_SAMPLE.min(dir_films.len());
        f.gt60 = rarefied_gt(&dir_films, size, POTENTIAL_DRAWS, POTENTIAL_SEED);
        // graded against pooled users with a similar number of director-credited films
        let (sc, band) = cuts.stat("DEV").for_count(credit_films);
        f.potential_band = band.clone().unwrap_or_default();
        let note = format!("{} director credits, against users with {} films", f.credits, f.potential_band);
        let g = grade_with(&sc, "DEV", f.gt60, credit_films < 60, &note);
        if credit_films >= 1000 && g.letter == "E" { make_complete(g) } else { g }
    };

    let examples = examples(ex, lk, tl, &rated, &dir_count, f.b);
    // Films missing from the table are skipped by every stat that needs film data. Speed and Stamina read only the
    // viewing timeline, so they count every watched film that gave a clean event.
    let coverage = vec![
        StatCoverage::new("PWR", rated.len(), ex.ratings.len(), 20),
        StatCoverage::timeline("SPD", tl.metrics.as_ref(), ex.watched.len()),
        StatCoverage::new("RNG", f.n_films_cty, ex.watched.len(), 50),
        StatCoverage::timeline("STA", tl.metrics.as_ref(), ex.watched.len()),
        StatCoverage::new("PRC", rated.len(), ex.ratings.len(), 50),
        StatCoverage::new("DEV", f.films_with_credits, ex.watched.len(), 40),
    ];
    Stats { power, speed, range, stamina, precision, potential, facts: f, rated, examples, coverage }
}

/// The film that best shows each stat. Ties always fall to the earlier day, then the title.
fn examples(ex: &Export, lk: &Lookup, tl: &Timeline, rated: &[RatedFilm], dir_count: &BTreeMap<u32, usize>, b: f64) -> [Option<String>; 6] {
    use std::collections::HashMap;
    let own: HashMap<(&str, i32), f64> = ex.ratings.iter().map(|r| (r.name.as_str(), r.year)).zip(ex.ratings.iter().map(|r| r.rating)).collect();
    let rating = |name: &str, year: i32| own.get(&(name, year)).copied();
    // highest-rated watched film among candidates (name, year, day); unrated films rank below rated ones
    let best = |c: &[(usize, i64)]| -> Option<String> {
        c.iter()
            .map(|&(wi, day)| {
                let w = &ex.watched[wi];
                (rating(&w.name, w.year).unwrap_or(-1.0), day, w.name.clone())
            })
            .min_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)))
            .map(|x| x.2)
    };
    let in_month = |m: i64| -> Vec<(usize, i64)> { tl.events.iter().filter(|e| crate::dates::month_of(e.day) == m).map(|e| (e.film, e.day)).collect() };
    let power = crate::ability::signature_film(rated, if b < 0.0 { -1 } else { 1 }).map(|x| x.0.name);
    let (speed, stamina) = match &tl.metrics {
        Some(m) => (best(&in_month(m.peak_month_index)), best(&in_month(m.streak_end_month))),
        None => (None, None),
    };
    // Range: the least common country (ties: code), via its highest-rated film
    let mut cc: BTreeMap<String, Vec<(usize, i64)>> = BTreeMap::new();
    let mut once: Vec<(usize, i64)> = Vec::new();
    for (wi, w) in ex.watched.iter().enumerate() {
        let Some(m) = lk.find(boxd_code(&w.uri), &w.name, w.year) else { continue };
        if m.runtime != 0 && m.runtime < MIN_RUNTIME {
            continue;
        }
        for c in &m.countries {
            cc.entry(c.clone()).or_default().push((wi, w.day));
        }
        if !m.directors.is_empty() && m.directors.iter().any(|d| dir_count.get(d) == Some(&1)) {
            once.push((wi, w.day));
        }
    }
    let range = cc.iter().min_by(|a, b| a.1.len().cmp(&b.1.len()).then(a.0.cmp(b.0))).and_then(|(code, v)| {
        best(v).map(|n| format!("{} ({})", n, lk.country_names.get(code).map(String::as_str).unwrap_or(code.as_str())))
    });
    // Precision: the best-known film you rated within 0.25 of the crowd
    let prc = rated.iter().filter(|f| (f.own - f.crowd).abs() <= 0.25).min_by(|a, b| b.n.cmp(&a.n).then(a.uri.cmp(&b.uri))).map(|f| f.name.clone());
    [power, speed, range, stamina, prc, best(&once)]
}

pub fn is_ungraded(g: &Grade) -> bool {
    g.status == Status::Ungraded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges() {
        assert_eq!(gap_edge(0), 0.25);
        assert_eq!(gap_edge(1), 0.3);
        assert_eq!(gap_edge(35), 2.0);
    }

    #[test]
    fn pair_bins_basic() {
        // three films: crowd 4.0, 3.0, 3.9; user 5, 2, 3
        let rv = [10, 4, 6];
        let c = [4.0, 3.0, 3.9];
        let (h, n) = pair_bins(&rv, &c);
        // gaps: (4.0,3.0)=1.0 concordant; (4.0,3.9)=0.1 ignored; (3.9,3.0)=0.9 concordant
        assert_eq!(n.iter().sum::<f64>(), 2.0);
        assert_eq!(h.iter().sum::<f64>(), 2.0);
        // user ties count half; reversed order counts zero
        let (h, _) = pair_bins(&[4, 4], &[4.0, 3.0]);
        assert_eq!(h.iter().sum::<f64>(), 0.5);
        let (h, _) = pair_bins(&[2, 6], &[4.0, 3.0]);
        assert_eq!(h.iter().sum::<f64>(), 0.0);
    }

    #[test]
    fn bin_index_follows_float_edges() {
        // gap exactly 0.30 lands in bin 1, gap 0.2999 in bin 0
        let (_, n) = pair_bins(&[2, 4], &[3.30, 3.0]);
        assert_eq!(n[1], 1.0);
        let (_, n) = pair_bins(&[2, 4], &[3.2999, 3.0]);
        assert_eq!(n[0], 1.0);
        // >= 2.0 is the last bin
        let (_, n) = pair_bins(&[2, 4], &[4.5, 1.0]);
        assert_eq!(n[35], 1.0);
    }

    #[test]
    fn rarefied_range_known() {
        // 100 single-country films over 4 equal countries: any 50-draw has D close to 4
        let cs: Vec<Vec<String>> = (0..100).map(|i| vec![["A", "B", "C", "D"][i % 4].to_string()]).collect();
        let refs: Vec<&Vec<String>> = cs.iter().collect();
        let d = rarefied_range(&refs, 50, 20, 5);
        assert!(d > 3.8 && d <= 4.0, "{d}");
        // one country -> exactly 1
        let one: Vec<Vec<String>> = (0..60).map(|_| vec!["US".to_string()]).collect();
        let refs: Vec<&Vec<String>> = one.iter().collect();
        assert!((rarefied_range(&refs, 50, 20, 5) - 1.0).abs() < 1e-12);
    }
}
