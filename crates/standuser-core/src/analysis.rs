//! One call from export to card: parse results + lookup table -> stats, grades, type, nearest Stands, ability, name, card.

use crate::ability::{self, Rendered};
use crate::dates::iso;
use crate::grading::{Cuts, Grade, Status};
use crate::lookup::Lookup;
use crate::matching::{self, Match};
use crate::name::{self, NameResult};
use crate::parse::Export;
use crate::stats::{self, Stats};
use crate::svg::CardData;
use crate::timeline::{self, Timeline};
use crate::types::{classify, StandType};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Coverage {
    pub watched: usize,
    pub watched_in_table: usize,
    pub rated: usize,
    pub rated_missing: usize,
    pub eligible: usize,
    /// Watched films the film table does not know; every stat that needs film data skips them.
    pub not_in_table: usize,
    /// Per stat: graded on N of M films, against the spec's minimum.
    pub per_stat: Vec<crate::stats::StatCoverage>,
    /// Build date of the film table, and its age when the caller gave today's date.
    pub table_built: Option<String>,
    pub table_age_days: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Analysis {
    pub username: String,
    pub joined: String,
    pub as_of: String,
    pub timeline: Timeline,
    pub stats: Stats,
    pub kind: StandType,
    pub nearest: Vec<Match>,
    pub ability: Rendered,
    pub name: NameResult,
    pub card: CardData,
    pub coverage: Coverage,
    pub notes: Vec<String>,
}

fn signed(v: f64) -> String {
    format!("{}{:.2}", if v < 0.0 { "−" } else { "+" }, v.abs())
}

fn metric(i: usize, g: &Grade, f: &stats::Facts) -> String {
    if g.status == Status::None {
        return format!("∅ {}", g.note);
    }
    if g.status == Status::Unknown {
        return format!("? {}", g.note);
    }
    match i {
        0 => format!("{}★ vs crowd", signed(f.b)),
        1 => format!("{:.1} films / active month", f.pace),
        2 => format!("{:.1} effective countries", f.d_range),
        3 => format!("{} of {} months active", f.active, f.months),
        4 => match g.p {
            Some(p) => format!("DPA {:.2}, top {:.0}%", f.dpa, ((1.0 - p) * 100.0).max(1.0)),
            None => format!("DPA {:.2}", f.dpa),
        },
        _ => format!("{} directors seen once", ability::pct0(f.gt)),
    }
}

pub fn analyze(ex: &Export, lk: &Lookup) -> Analysis {
    analyze_at(ex, lk, None)
}

/// `today` (days since 1970-01-01) lets the card warn when the film table is older than TMDB's six-month caching limit.
pub fn analyze_at(ex: &Export, lk: &Lookup, today: Option<crate::dates::Day>) -> Analysis {
    let cuts = Cuts::embedded();
    let joined = timeline::effective_join(ex);
    let as_of = timeline::effective_as_of(ex);
    let tl = timeline::build(ex, joined, as_of);
    let st = stats::compute(ex, lk, &tl, &cuts);
    let username = ex.username.clone().unwrap_or_else(|| "unknown".into());

    let grades = [st.power.clone(), st.speed.clone(), st.range.clone(), st.stamina.clone(), st.precision.clone(), st.potential.clone()];
    let canon: [Option<f64>; 6] = std::array::from_fn(|i| grades[i].canon);
    let kind = classify(&canon);
    let nearest = matching::nearest(&canon, &username, 3);
    let ab = ability::render(&grades, &st.facts, &st.rated);
    let nm = name::name_stand(ex, lk, ab.signature.as_ref(), ab.template);

    let mut notes = vec![];
    if ex.joined.is_none() {
        notes.push("profile.csv not found: the join day is taken as the earliest watched.csv date".to_string());
    }
    if ex.as_of.is_none() {
        notes.push("the export date could not be read from the file name: the latest date in the export is used".to_string());
    }
    if st.facts.n_rated_missing * 2 > st.facts.n_rated && st.facts.n_rated > 0 {
        notes.push(format!("{} of {} rated films are not in the lookup table, so Power and Precision are not graded", st.facts.n_rated_missing, st.facts.n_rated));
    }
    let age = today.and_then(|t| lk.age_days(t));
    if let Some(a) = age {
        if a > crate::lookup::MAX_AGE_DAYS {
            notes.push(format!("the film table is {a} days old; TMDB data may be kept for at most six months, so it should be rebuilt (tools/refresh_table.sh)"));
        }
    }
    let any_prov = grades.iter().any(|g| matches!(g.status, Status::Provisional | Status::Ungraded | Status::Unknown));
    let evidence: Vec<(String, String)> = (0..6)
        .map(|i| (metric(i, &grades[i], &st.facts), st.examples[i].clone().unwrap_or_default()))
        .collect();
    let card = CardData {
        name: nm.name.clone(),
        user: username.clone(),
        type_jp: kind.jp().into(),
        type_en: kind.en().into(),
        grades: grades.iter().map(|g| g.letter.clone()).collect(),
        plot: grades.iter().map(|g| g.plot).collect(),
        prov: grades.iter().map(|g| g.status == Status::Provisional).collect(),
        unknown: grades.iter().map(|g| g.letter == "?").collect(),
        coverage: st.coverage.iter().map(|c| c.text()).collect(),
        evidence,
        ability: ab.ability.clone(),
        weakness: ab.weakness.clone(),
        nearest: if nearest.is_empty() { vec!["needs 4 graded stats".into()] } else { nearest.iter().map(|m| m.name.clone()).collect() },
        signature: ab.film.clone().unwrap_or_else(|| "Signature film: not enough crowd data".into()),
        badge: any_prov,
    };
    let coverage = Coverage {
        watched: st.facts.n_watched,
        watched_in_table: st.facts.n_watched_in_table,
        rated: st.facts.n_rated,
        rated_missing: st.facts.n_rated_missing,
        eligible: st.facts.n_elig,
        not_in_table: st.facts.n_watched - st.facts.n_watched_in_table,
        per_stat: st.coverage.clone(),
        table_built: lk.built.map(iso),
        table_age_days: age,
    };
    Analysis { username, joined: iso(joined), as_of: iso(as_of), timeline: tl, stats: st, kind, nearest, ability: ab, name: nm, card, coverage, notes }
}
