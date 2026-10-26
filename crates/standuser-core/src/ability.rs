//! Deterministic Stand ability engine (port of swarm/round3/ability/ability_engine.py): 17 text templates, signature
//! film, weakness and a slot-only "why" line. No randomness, no network, no clock.

use crate::grading::{phi, probit, Grade, Status, Symbol, LABEL, STATS};
use crate::stats::{Facts, RatedFilm};
use serde::Serialize;

pub const TAU_BAL: f64 = 1.0;
pub const FLOOR: u64 = 30000;
pub const GAP_MIN_DIRECTIONAL: f64 = 1.0;

fn stat_ix(s: &str) -> usize {
    STATS.iter().position(|x| *x == s).unwrap()
}

pub fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Python's `f"{x:.0%}"`.
pub fn pct0(x: f64) -> String {
    format!("{:.0}%", x * 100.0)
}

/// Python's `ordinal`: round(p*100, 1) printed with :g, then th/st/nd/rd by its last digit.
pub fn ordinal(p: f64) -> String {
    let s = format!("{:.1}", p * 100.0);
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    let suf = if !(s.ends_with('1') || s.ends_with('2') || s.ends_with('3')) || s.ends_with("11") || s.ends_with("12") || s.ends_with("13") {
        "th"
    } else {
        match s.chars().last().unwrap() {
            '1' => "st",
            '2' => "nd",
            _ => "rd",
        }
    };
    format!("{s}{suf}")
}

#[derive(Debug, Clone, Serialize)]
pub struct Signature {
    /// "PWR".."DEV" or "BAL"
    pub stat: String,
    pub z: f64,
    pub top: Option<String>,
    pub runner: Option<String>,
    pub zs: Vec<(String, f64)>,
}

fn z_of(g: &Grade) -> f64 {
    match g.symbol {
        Some(Symbol::Inf) => 3.0,
        Some(Symbol::Complete) => -3.0,
        None => probit(g.p.unwrap_or(0.5).clamp(0.005, 0.995)),
    }
}

fn graded<'a>(grades: &'a [Grade; 6]) -> Vec<(usize, &'a Grade)> {
    grades.iter().enumerate().filter(|(_, g)| matches!(g.status, Status::Measured | Status::Provisional)).collect()
}

pub fn pick_signature(grades: &[Grade; 6]) -> Option<Signature> {
    let g = graded(grades);
    if g.is_empty() {
        return None;
    }
    let mut zs: Vec<(usize, f64)> = g.iter().map(|(i, gr)| (*i, z_of(gr))).collect();
    zs.sort_by(|a, b| b.1.abs().total_cmp(&a.1.abs()).then(a.0.cmp(&b.0)));
    let (top, runner) = (zs[0], zs.get(1).copied());
    let out = |stat: &str, top_stat: Option<&str>| Signature {
        stat: stat.into(),
        z: top.1,
        top: top_stat.map(String::from),
        runner: runner.map(|r| STATS[r.0].to_string()),
        zs: zs.iter().map(|(i, z)| (STATS[*i].to_string(), *z)).collect(),
    };
    if top.1.abs() < TAU_BAL {
        return Some(out("BAL", Some(STATS[top.0])));
    }
    Some(out(STATS[top.0], None))
}

pub struct Template {
    pub id: u8,
    pub name: &'static str,
    pub canon: &'static str,
    pub cg: &'static str,
    pub quote: &'static str,
}

/// (stat, direction) -> template
pub fn template(stat: &str, dir: &str) -> Template {
    let t = |id, name, canon, cg, quote| Template { id, name, canon, cg, quote };
    match (stat, dir) {
        ("PWR", "hi") => t(1, "Bomb Transmutation", "Killer Queen", "A", "Killer Queen can turn anything it touches into a bomb."),
        ("PWR", "lo") => t(2, "Curative Cooking", "Pearl Jam", "E", "it can heal incurable illnesses provided the right ingredients are involved"),
        ("SPD", "hi") => t(3, "Hora Rush", "Silver Chariot", "A", "usually relying on attacking enemies with barrages of sword slashes and thrusts"),
        ("SPD", "inf") => t(4, "Time Acceleration", "Made in Heaven", "∞", "The only living beings who can move freely within time's acceleration are Made in Heaven itself and its user."),
        ("SPD", "lo") => t(5, "Accelerated Aging", "The Grateful Dead", "E", "The activation of this ability manifests itself as mist spreading from The Grateful Dead."),
        ("RNG", "hi") => t(6, "Coiled Body", "Hierophant Green", "A", "being able to distance itself from its user at least 100 meters"),
        ("RNG", "inf") => t(7, "Fictional Manifestation", "Bohemian Rhapsody", "∞", "a Stand ability that breaks the barrier between fiction and reality"),
        ("RNG", "lo") => t(8, "Position Locking", "Kraft Work", "E", "the ability to lock an object's position, completely freezing it in place"),
        ("STA", "hi") => t(9, "Heat-Seeking Detonation", "Sheer Heart Attack", "A", "it will continue to seek out targets until it explodes from a human"),
        ("STA", "inf") => t(10, "Energy Absorption", "Notorious B.I.G", "∞", "one of the few Stands able to exist even after its user's death"),
        ("STA", "lo") => t(11, "Time Erasure", "King Crimson", "E", "erase time for up to around ten seconds"),
        ("PRC", "hi") => t(12, "Wind Reading", "Manhattan Transfer", "A", "floating around and detecting the air movements near it"),
        ("PRC", "lo") => t(13, "Killer Virus Infection", "Purple Haze", "E", "Because the virus indiscriminately affects friends and foes"),
        ("DEV", "hi") => t(14, "Life Giver", "Gold Experience", "A", "the ability to create life from inanimate material"),
        ("DEV", "lo") => t(15, "Divination", "Hermit Purple", "E", "It allows its user to divine information and messages from other locations."),
        ("DEV", "complete") => t(16, "Complete Mastery", "Star Platinum (Part 6)", "Complete", "It decreases in rank as the user masters their Stand."),
        _ => t(17, "Mimicry", "Surface", "B B C B C C", "Surface's primary ability is to copy the appearance of another person."),
    }
}

/// The template with this id (1 to 17); the balanced one for anything else.
pub fn template_by_id(id: u8) -> Template {
    const KEYS: [(&str, &str); 17] = [("PWR", "hi"), ("PWR", "lo"), ("SPD", "hi"), ("SPD", "inf"), ("SPD", "lo"), ("RNG", "hi"), ("RNG", "inf"), ("RNG", "lo"), ("STA", "hi"),
        ("STA", "inf"), ("STA", "lo"), ("PRC", "hi"), ("PRC", "lo"), ("DEV", "hi"), ("DEV", "lo"), ("DEV", "complete"), ("BAL", "bal")];
    KEYS.iter().map(|(s, d)| template(s, d)).find(|t| t.id == id).unwrap_or_else(|| template("BAL", "bal"))
}

fn template_key(sig: &Signature, grades: &[Grade; 6]) -> (String, &'static str) {
    if sig.stat == "BAL" {
        return ("BAL".into(), "bal");
    }
    let g = &grades[stat_ix(&sig.stat)];
    match g.symbol {
        Some(Symbol::Inf) => (sig.stat.clone(), "inf"),
        Some(Symbol::Complete) => (sig.stat.clone(), "complete"),
        None => (sig.stat.clone(), if sig.z > 0.0 { "hi" } else { "lo" }),
    }
}

fn ability_text(id: u8, f: &Facts, top_label: &str, top_letter: &str, top_pct: &str) -> String {
    let b_abs = f.b.abs();
    match id {
        1 => format!("Every film it touches becomes a bomb: it rates {:.2}★ below the crowd on average, across {} films.", b_abs, thousands(f.n_elig as u64)),
        2 => format!("It heals what the crowd wounded: it rates {:.2}★ above the crowd on average, across {} films.", b_abs, thousands(f.n_elig as u64)),
        3 => format!("A barrage too fast to follow: {:.1} films per active month, peaking at {} films in {}.", f.pace, f.peak_n, f.peak_month),
        4 => format!("Time speeds up around it: {:.1} films per active month, peaking at {} in {}, a pace almost no other user reaches.", f.pace, f.peak_n, f.peak_month),
        5 => format!("A slow mist drifts rather than strikes: {:.1} films per active month, peaking at {} in {}.", f.pace, f.peak_n, f.peak_month),
        6 => format!("It unravels into tendrils that cross borders: {:.1} effective production countries per 50 films, led by {} ({} of films).", f.d_range, f.top_country, pct0(f.top_share)),
        7 => format!("Fiction from every nation walks into the world: {:.1} effective production countries per 50 films, across {} films.", f.d_range, thousands(f.n_films_cty as u64)),
        8 => format!("It locks everything in place: only {:.1} effective production countries per 50 films, and {} of your films come from {}.", f.d_range, pct0(f.top_share), f.top_country),
        9 => format!("It never stops tracking: films in {} of {} months, with a streak of {} in a row.", f.active, f.months, f.streak),
        10 => format!("It keeps growing on whatever it touches: films in every one of the {} months since {}.", f.months, f.start),
        11 => format!("Time vanishes and only the result remains: {} of {} months hold no film, and the longest blank runs {} months.", f.silent, f.months, f.silent_run),
        12 => format!("It reads the air and answers it: it agrees with the crowd on {} of the {} film pairs it can compare (difficulty-adjusted score {:.2}).", pct0(f.pa25), thousands(f.pairs), f.dpa),
        13 => format!("Its verdicts scatter without regard for the crowd: it agrees with the crowd on only {} of the {} film pairs it can compare (difficulty-adjusted score {:.2}).", pct0(f.pa25), thousands(f.pairs), f.dpa),
        14 => format!("Every unfamiliar name becomes a new life: {} of your {} director credits are directors you have seen only once ({}).", f.n1, thousands(f.credits as u64), pct0(f.gt)),
        15 => format!("It divines through familiar vines: only {} of your {} director credits are one-offs, and it keeps returning to {} ({} films).", pct0(f.gt), thousands(f.credits as u64), f.top_dir, f.top_dir_n),
        16 => format!("Complete: across {} director credits the frontier has closed, with only {} one-offs.", thousands(f.credits as u64), pct0(f.gt)),
        _ => format!("It copies whoever stands before it: your strongest stat, {top_label} {top_letter}, sits at the {top_pct} percentile and nothing else stands out."),
    }
}

/// The longest sentence the card carries under the ability name, in words.
pub const CARD_LINE_WORDS: usize = 20;

/// The card's one sentence for template `id`: the template's image and the one figure behind it (`ability_text` keeps the full line).
fn card_line(id: u8, f: &Facts, top_label: &str, top_letter: &str, top_pct: &str) -> String {
    match id {
        1 => format!("Every film it touches becomes a bomb: it rates {:.2}★ below the crowd on average.", f.b.abs()),
        2 => format!("It heals what the crowd wounded: it rates {:.2}★ above the crowd on average.", f.b.abs()),
        3 => format!("A barrage too fast to follow: {:.1} films per active month.", f.pace),
        4 => format!("Time speeds up around it: {:.1} films per active month, a pace almost no one reaches.", f.pace),
        5 => format!("A slow mist drifts rather than strikes: {:.1} films per active month.", f.pace),
        6 => format!("It unravels into tendrils that cross borders: {:.1} effective production countries per 50 films.", f.d_range),
        7 => format!("Fiction from every nation walks into the world: {:.1} effective production countries per 50 films.", f.d_range),
        8 => format!("It locks everything in place: {} of your films come from {}.", pct0(f.top_share), f.top_country),
        9 => format!("It never stops tracking: films in {} of {} months.", f.active, f.months),
        10 => format!("It keeps growing on whatever it touches: films in every one of {} months.", f.months),
        11 => format!("Time vanishes and only the result remains: {} of {} months hold no film.", f.silent, f.months),
        12 => format!("It reads the air and answers it: it agrees with the crowd on {} of film pairs.", pct0(f.pa25)),
        13 => format!("Its verdicts scatter without regard for the crowd: it agrees on only {} of film pairs.", pct0(f.pa25)),
        14 => format!("Every unfamiliar name becomes a new life: {} of your director credits are directors seen once.", pct0(f.gt)),
        15 => format!("It divines through familiar vines: it keeps returning to {} ({} films).", f.top_dir, f.top_dir_n),
        16 => format!("Complete: the frontier has closed, with only {} of your director credits one-offs.", pct0(f.gt)),
        _ => format!("It copies whoever stands before it: your strongest stat is {top_label} {top_letter} ({top_pct} percentile)."),
    }
}

fn weak_phrase(stat: &str, tier: &str, l: &str, f: &Facts) -> String {
    let b_abs = f.b.abs();
    let b_dir = if f.b < 0.0 { "below" } else { "above" };
    match (stat, tier) {
        ("PWR", "weak_lo") => format!("gentle hands (Power {l}). It rates {:.2}★ above the crowd on average, so its blows rarely leave a mark.", b_abs),
        ("PWR", "weak_hi") => format!("blunt hands (Power {l}). It rates {:.2}★ below the crowd on average, so little survives it.", b_abs),
        ("PWR", _) => format!("ordinary hands (Power C, “same as a human”). It rates {:.2}★ {b_dir} the crowd on average, about what a human would.", b_abs),
        ("SPD", "weak") => format!("slow hands (Speed {l}). {:.1} films per active month, so anything faster outruns it.", f.pace),
        ("SPD", _) => format!("ordinary pace (Speed C, “same as a human”). {:.1} films per active month, about what a human would.", f.pace),
        ("RNG", "weak") => format!("short reach (Range {l}). {} of your films come from {}, so it loses its grip beyond it.", pct0(f.top_share), f.top_country),
        ("RNG", _) => format!("ordinary reach (Range C, “same as a human”). {:.1} effective production countries per 50 films.", f.d_range),
        ("STA", "weak") => format!("short breath (Stamina {l}). Films in {} of {} months, so the effect fades between bursts.", f.active, f.months),
        ("STA", _) => format!("ordinary breath (Stamina C, “same as a human”). Films in {} of {} months.", f.active, f.months),
        ("PRC", "weak") => format!("loose aim (Precision {l}). It agrees with the crowd on {} of film pairs, so it can hit what it did not mean to.", pct0(f.pa25)),
        ("PRC", _) => format!("ordinary aim (Precision C, “same as a human”). It agrees with the crowd on {} of film pairs.", pct0(f.pa25)),
        ("DEV", "weak") => format!("nothing left to learn (Potential {l}). Only {} of your director credits are one-offs, so there is little room to grow.", pct0(f.gt)),
        (_, _) => format!("ordinary growth (Potential C, “same as a human”). {} of your director credits are one-offs.", pct0(f.gt)),
    }
}

fn status_tag(s: Status) -> &'static str {
    if s == Status::Provisional { " [provisional]" } else { "" }
}

/// Every sentence the engine can put on the card, written with sample numbers (for the glyph-coverage test).
pub fn all_sample_sentences() -> Vec<String> {
    let f = Facts {
        b: -0.12, n_elig: 1234, pace: 6.5, peak_n: 14, peak_month: "Mar 2023".into(), months: 48, active: 40, streak: 17, start: "Jan 2023".into(),
        silent: 8, silent_run: 3, d_range: 3.4, top_country: "the United States".into(), top_share: 0.62, n_films_cty: 500, pa25: 0.81, pairs: 70211,
        dpa: 0.88, n1: 120, credits: 240, gt: 0.5, top_dir: "Director Seven".into(), top_dir_n: 6, ..Default::default()
    };
    let mut v = Vec::new();
    for (stat, dir) in [("PWR", "hi"), ("PWR", "lo"), ("SPD", "hi"), ("SPD", "inf"), ("SPD", "lo"), ("RNG", "hi"), ("RNG", "inf"), ("RNG", "lo"), ("STA", "hi"), ("STA", "inf"), ("STA", "lo"),
        ("PRC", "hi"), ("PRC", "lo"), ("DEV", "hi"), ("DEV", "lo"), ("DEV", "complete"), ("BAL", "bal")] {
        let t = template(stat, dir);
        v.push(format!("{}. {} [provisional]", t.name, ability_text(t.id, &f, "Precision", "A", "97.8th")));
        v.push(card_line(t.id, &f, "Precision", "A", "97.8th"));
    }
    for stat in STATS {
        for tier in ["weak", "weak_lo", "weak_hi", "mid"] {
            v.push(format!("Weakness: {} [provisional]", weak_phrase(stat, tier, "D", &f)));
            v.push(format!("Soft spot: {}", weak_phrase(stat, tier, "C", &f)));
        }
    }
    v.push("No weakness found: your lowest graded stat, Power, is still B. [provisional]".into());
    v.push("Signature film: Synthetic Film One (2008), 1.0★ against the crowd's 3.54★ (-2.54), 166,934 ratings.".into());
    v.push("Signature film: not enough crowd data".into());
    v.push("Ability: not yet revealed (?).".into());
    v.push("Weakness: not yet revealed (?).".into());
    v
}

#[derive(Debug, Clone, Serialize)]
pub struct Weakness {
    pub text: String,
    pub stat: Option<String>,
    pub letter: Option<String>,
}

pub fn weakness(grades: &[Grade; 6], sig: &Signature, f: &Facts) -> Weakness {
    let mut cands: Vec<(usize, &Grade)> = graded(grades).into_iter().filter(|(i, g)| STATS[*i] != sig.stat && g.symbol.is_none()).collect();
    if sig.stat == "BAL" {
        cands.retain(|(i, _)| Some(STATS[*i]) != sig.top.as_deref());
    }
    if cands.is_empty() {
        return Weakness { text: "Weakness: not yet revealed (?).".into(), stat: None, letter: None };
    }
    cands.sort_by(|a, b| a.1.p.unwrap_or(0.5).total_cmp(&b.1.p.unwrap_or(0.5)).then(a.0.cmp(&b.0)));
    let (i, g) = cands[0];
    let k = STATS[i];
    let l = g.letter.as_str();
    let txt = if l == "D" || l == "E" {
        let tier = if k == "PWR" { if f.b > 0.0 { "weak_lo" } else { "weak_hi" } } else { "weak" };
        format!("Weakness: {}", weak_phrase(k, tier, l, f))
    } else if l == "C" {
        format!("Soft spot: {}", weak_phrase(k, "mid", l, f))
    } else {
        format!("No weakness found: your lowest graded stat, {}, is still {}.", LABEL[i], l)
    };
    Weakness { text: format!("{txt}{}", status_tag(g.status)), stat: Some(k.into()), letter: Some(l.into()) }
}

/// Largest |own - crowd| among films with >= FLOOR Letterboxd ratings. Ties: gap rounded to 0.01, more ratings, uri.
pub fn signature_film(films: &[RatedFilm], sign: i32) -> Option<(RatedFilm, f64)> {
    let mut e: Vec<(RatedFilm, f64)> = films.iter().filter(|f| f.n >= FLOOR).map(|f| (f.clone(), f.own - f.crowd)).collect();
    if e.is_empty() {
        return None;
    }
    if sign != 0 {
        let d: Vec<(RatedFilm, f64)> = e.iter().filter(|(_, g)| g * sign as f64 > 0.0 && g.abs() >= GAP_MIN_DIRECTIONAL).cloned().collect();
        if !d.is_empty() {
            e = d;
        }
    }
    let cents = |g: f64| -> i64 { format!("{:.2}", g.abs()).replace('.', "").parse().unwrap() };
    e.sort_by(|a, b| cents(b.1).cmp(&cents(a.1)).then(b.0.n.cmp(&a.0.n)).then(a.0.uri.cmp(&b.0.uri)));
    e.into_iter().next()
}

#[derive(Debug, Clone, Serialize)]
pub struct Rendered {
    pub template: Option<u8>,
    pub ability_name: String,
    pub ability: String,
    /// The one sentence the card carries under the ability name: at most CARD_LINE_WORDS words, with the one figure that decides the template.
    pub card_line: String,
    pub film: Option<String>,
    pub film_name: Option<String>,
    pub weakness: String,
    pub why: String,
    pub signature: Option<Signature>,
    pub runner: Option<String>,
}

pub fn render(grades: &[Grade; 6], f: &Facts, films: &[RatedFilm]) -> Rendered {
    let Some(sig) = pick_signature(grades) else {
        return Rendered {
            template: None,
            ability_name: "Not yet revealed".into(),
            ability: "Ability: not yet revealed (?).".into(),
            card_line: "Too few stats could be graded to reveal an ability (?).".into(),
            film: None,
            film_name: None,
            weakness: "Weakness: not yet revealed (?).".into(),
            why: "No stat could be graded.".into(),
            signature: None,
            runner: None,
        };
    };
    let key = template_key(&sig, grades);
    let t = template(&key.0, key.1);
    let (tl, tp, tpct) = if key.0 == "BAL" {
        let top = sig.top.as_deref().unwrap();
        let g = &grades[stat_ix(top)];
        (LABEL[stat_ix(top)].to_string(), g.letter.clone(), ordinal(g.p.unwrap_or(0.5)))
    } else {
        (String::new(), String::new(), String::new())
    };
    let mut ability = format!("{}. {}", t.name, ability_text(t.id, f, &tl, &tp, &tpct));
    if key.0 != "BAL" && grades[stat_ix(&sig.stat)].status == Status::Provisional {
        ability.push_str(" [provisional]");
    }
    let sign = match (key.0.as_str(), key.1) {
        ("PWR", "hi") => -1,
        ("PWR", "lo") => 1,
        _ => 0,
    };
    let sf = signature_film(films, sign);
    let film = sf.as_ref().map(|(sf, gap)| format!("Signature film: {} ({}), {:.1}★ against the crowd's {:.2}★ ({:+.2}), {} ratings.", sf.name, sf.year, sf.own, sf.crowd, gap, thousands(sf.n)));
    let wk = weakness(grades, &sig, f);
    let mut why = if key.0 == "BAL" {
        format!("Why: no graded stat leaves the central band (largest |z| {:.2} < {}), so the balanced template applies; {} follows {} (canon chart {}). ", sig.z.abs(), TAU_BAL, t.name, t.canon, t.cg)
    } else {
        let i = stat_ix(&sig.stat);
        let g = &grades[i];
        let rn = match &sig.runner {
            Some(r) => {
                let z = sig.zs.iter().find(|(s, _)| s == r).unwrap().1;
                format!("; runner-up {} z {:+.2}", LABEL[stat_ix(r)], z)
            }
            None => String::new(),
        };
        let stat_txt = match g.p {
            Some(p) => format!("{} {} ({} percentile, z {:+.2}{})", LABEL[i], g.letter, ordinal(p), sig.z, rn),
            None => format!("{} {}", LABEL[i], g.letter),
        };
        format!("Why: {} is your most extreme graded stat, which selects {} after {}, whose canon {} is {}. Canon: “{}”. ", stat_txt, t.name, t.canon, LABEL[i], t.cg, t.quote.trim_end_matches('.'))
    };
    if let (Some(s), Some(l)) = (&wk.stat, &wk.letter) {
        why.push_str(&format!("{} {} is your lowest graded stat, so the limitation comes from it.", LABEL[stat_ix(s)], l));
    }
    let _ = phi;
    Rendered { template: Some(t.id), ability_name: t.name.into(), ability, card_line: card_line(t.id, f, &tl, &tp, &tpct), film_name: sf.as_ref().map(|(s, _)| s.name.clone()), film, weakness: wk.text, why, runner: sig.runner.clone(), signature: Some(sig) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grading::Grade;

    fn g(stat: &str, p: f64, letter: &str, status: Status) -> Grade {
        Grade { stat: stat.into(), value: Some(0.0), p: Some(p), letter: letter.into(), status, symbol: None, canon: Some(3.0), plot: 3.0, note: String::new() }
    }
    fn ungraded(stat: &str) -> Grade {
        Grade { stat: stat.into(), value: Some(0.5), p: None, letter: "?".into(), status: Status::Ungraded, symbol: None, canon: None, plot: 3.0, note: String::new() }
    }

    /// The prototype engine's sample profile (tools/golden/proto/ability_engine.py OJAS), with invented films and numbers.
    fn sample() -> ([Grade; 6], Facts, Vec<RatedFilm>) {
        let grades = [
            g("PWR", 0.597, "C", Status::Measured),
            g("SPD", 0.803, "B", Status::Provisional),
            Grade { letter: "C".into(), ..ungraded("RNG") },
            g("STA", 0.923, "A", Status::Provisional),
            g("PRC", 0.978, "A", Status::Measured),
            Grade { letter: "C".into(), ..ungraded("DEV") },
        ];
        let f = Facts {
            b: -0.0612, n_elig: 410, pace: 4.2, peak_n: 12, peak_month: "Mar 2023".into(), months: 44, active: 40, streak: 18, start: "Feb 2023".into(),
            silent: 4, silent_run: 2, d_range: 2.9, top_country: "the United States".into(), top_share: 0.8, n_films_cty: 200, pa25: 0.894, pairs: 70211,
            dpa: 0.883, n1: 120, credits: 240, gt: 0.5, top_dir: "Director Seven".into(), top_dir_n: 6, ..Default::default()
        };
        let film = |n: &str, y, own, crowd, cnt: u64, uri: &str| RatedFilm { name: n.into(), year: y, own, crowd, n: cnt, uri: uri.into() };
        let films = vec![
            film("Synthetic Film One", 2008, 1.0, 3.544, 166934, "synthetic-one"),
            film("Synthetic Film Two: The Long Subtitle of a Film", 2006, 5.0, 3.67, 2879 * 300, "synthetic-two"),
            film("Synthetic Film Three", 2010, 1.5, 2.82, 4020 * 300, "synthetic-three"),
            film("Synthetic Film Four", 2000, 2.0, 3.24, 347 * 300, "synthetic-four"),
            film("Synthetic Film Five", 2017, 5.0, 3.84, 5287 * 300, "synthetic-five"),
        ];
        (grades, f, films)
    }

    #[test]
    fn sample_matches_python_engine_output() {
        let (grades, f, films) = sample();
        let r = render(&grades, &f, &films);
        assert_eq!(r.template, Some(12));
        assert_eq!(r.ability, "Wind Reading. It reads the air and answers it: it agrees with the crowd on 89% of the 70,211 film pairs it can compare (difficulty-adjusted score 0.88).");
        assert_eq!(r.film.as_deref(), Some("Signature film: Synthetic Film One (2008), 1.0★ against the crowd's 3.54★ (-2.54), 166,934 ratings."));
        assert_eq!(r.weakness, "Soft spot: ordinary hands (Power C, “same as a human”). It rates 0.06★ below the crowd on average, about what a human would.");
        assert_eq!(r.why, "Why: Precision A (97.8th percentile, z +2.01; runner-up Stamina z +1.43) is your most extreme graded stat, which selects Wind Reading after Manhattan Transfer, whose canon Precision is A. Canon: “floating around and detecting the air movements near it”. Power C is your lowest graded stat, so the limitation comes from it.");
    }

    #[test]
    fn the_card_sentence_is_one_sentence_of_at_most_20_words() {
        let f = Facts {
            b: -0.12, n_elig: 1234, pace: 6.5, peak_n: 14, peak_month: "Mar 2023".into(), months: 48, active: 40, streak: 17, start: "Jan 2023".into(),
            silent: 8, silent_run: 3, d_range: 3.4, top_country: "the United Arab Emirates".into(), top_share: 0.62, n_films_cty: 500, pa25: 0.81, pairs: 70211,
            dpa: 0.88, n1: 120, credits: 240, gt: 0.5, top_dir: "Apichatpong Weerasethakul".into(), top_dir_n: 16, ..Default::default()
        };
        for id in 1..=17 {
            let s = card_line(id, &f, "Precision", "A", "97.8th");
            let words = s.split_whitespace().count();
            assert!(words >= 6 && words <= CARD_LINE_WORDS, "{id}: {words} words: {s}");
            // one sentence: it ends at its single closing full stop, and has no other sentence break or bracketed tag
            assert!(s.ends_with('.') && !s[..s.len() - 1].contains(". ") && !s.contains('['), "{id}: {s}");
        }
    }

    #[test]
    fn signature_rules() {
        let (mut grades, _, films) = sample();
        assert_eq!(pick_signature(&grades).unwrap().stat, "PRC");
        // tie in |z| -> canon order (STA before PRC)
        grades[3].p = Some(0.978);
        assert_eq!(pick_signature(&grades).unwrap().stat, "STA");
        // film tie-break: larger n, then uri ascending; floor
        let mk = |n: &str, c: f64, cnt: u64, uri: &str| RatedFilm { name: n.into(), year: 1, own: 1.0, crowd: c, n: cnt, uri: uri.into() };
        let f = vec![mk("B", 3.0, 40000, "b"), mk("A", 3.0, 50000, "z"), mk("C", 3.0, 50000, "a")];
        assert_eq!(signature_film(&f, 0).unwrap().0.uri, "a");
        assert!(signature_film(&[mk("Obscure", 4.0, 900, "o")], 0).is_none());
        let _ = films;
    }

    #[test]
    fn unrated_user_and_nothing_gradable() {
        let (mut grades, f, _) = sample();
        grades[0] = Grade::none("PWR", "");
        grades[4] = Grade::none("PRC", "");
        let r = render(&grades, &f, &[]);
        assert_eq!(r.template, Some(9));
        assert!(r.film.is_none());
        assert!(r.ability.ends_with("[provisional]"));
        let all: [Grade; 6] = std::array::from_fn(|i| Grade::unknown(STATS[i], None, ""));
        assert!(render(&all, &f, &[]).template.is_none());
    }

    #[test]
    fn ordinals() {
        assert_eq!(ordinal(0.978), "97.8th");
        assert_eq!(ordinal(0.7), "70th");
        assert_eq!(ordinal(0.043), "4.3rd");
        assert_eq!(ordinal(0.001), "0.1st");
        assert_eq!(thousands(1210), "1,210");
        assert_eq!(thousands(999), "999");
    }
}
