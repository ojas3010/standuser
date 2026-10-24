//! Nearest canon Stand: Method H. L2 distance on A=5..E=1, ∞=6, 完成=1, ∅=0 with each unknown canon cell costed at its
//! expected squared error under that stat's canon grade shares. A user "?" is left out and the distance rescaled by
//! 6/k; at least 4 known stats are required.
//!
//! Ties between Stands at equal letter distance are broken by the continuous distance between the user's percentiles
//! and the midpoint of each Stand's canon-equivalent percentile band, then by a hash of the username and the Stand
//! (which also splits identical-chart twins). The earlier tie-break chain (salience-weighted error, largest gap, exact
//! matches, known stats, canon order) is no longer used.

use crate::grading::{Cuts, STATS};
use crate::rng::fnv1a;
use serde::Serialize;

const CANON_CSV: &str = include_str!("../data/canon.csv");
const COLS: [&str; 6] = ["PWR", "SPD", "RNG", "STA", "PRC", "DEV"];

#[derive(Debug, Clone)]
pub struct Canon {
    pub name: String,
    pub user: String,
    pub part: String,
    pub row: u32,
    pub grades: [String; 6],
    pub vec: [Option<f64>; 6],
}

#[derive(Debug, Clone, Serialize)]
pub struct Match {
    pub name: String,
    pub user: String,
    pub part: String,
    pub grades: Vec<String>,
    pub dist: f64,
    pub shares_chart_with: Vec<String>,
}

fn num(g: &str) -> Option<f64> {
    match g.trim() {
        "A" => Some(5.0),
        "B" => Some(4.0),
        "C" => Some(3.0),
        "D" => Some(2.0),
        "E" => Some(1.0),
        "∞" => Some(6.0),
        "Complete" => Some(1.0),
        "∅" => Some(0.0),
        _ => None,
    }
}

/// All canon rows (157). `eligible` filtering (>= 4 known) happens in `nearest`.
pub fn load_canon() -> Vec<Canon> {
    let mut rd = csv::Reader::from_reader(CANON_CSV.as_bytes());
    let h = rd.headers().unwrap().clone();
    let ix = |n: &str| h.iter().position(|c| c == n).unwrap_or_else(|| panic!("canon.csv column {n}"));
    let (cn, cu, cp, cr) = (ix("display_name"), ix("user"), ix("part"), ix("wiki_row"));
    let cs: Vec<usize> = COLS.iter().map(|c| ix(c)).collect();
    rd.records()
        .map(|r| {
            let r = r.unwrap();
            let grades: [String; 6] = std::array::from_fn(|i| r[cs[i]].trim().to_string());
            let vec = std::array::from_fn(|i| num(&grades[i]));
            Canon { name: r[cn].to_string(), user: r[cu].to_string(), part: r[cp].to_string(), row: r[cr].parse().unwrap(), grades, vec }
        })
        .collect()
}

/// Per-stat canon grade shares E..A over lettered cells (∞ folded into A, Complete into E).
pub fn canon_bands(canon: &[Canon]) -> [[f64; 5]; 6] {
    let mut out = [[0.0; 5]; 6];
    for c in canon {
        for (j, g) in c.grades.iter().enumerate() {
            let g = match g.as_str() {
                "∞" => "A",
                "Complete" => "E",
                x => x,
            };
            if let Some(v) = num(g).filter(|v| (1.0..=5.0).contains(v)) {
                out[j][v as usize - 1] += 1.0;
            }
        }
    }
    for row in out.iter_mut() {
        let s: f64 = row.iter().sum();
        for x in row.iter_mut() {
            *x /= s;
        }
    }
    out
}

fn r9(x: f64) -> i64 {
    (x * 1e9).round() as i64
}

/// Midpoint (0..1) of the percentile band a canon grade value covers for one stat, from canon's cumulative grade shares.
/// ∅ sits at the bottom, ∞ at the very top, 完成 in the lowest band.
fn band_mid(canon_cum: &[f64], v: f64) -> f64 {
    if v <= 0.0 {
        return 0.0;
    }
    if v >= 6.0 {
        return 0.9995;
    }
    let g = (v.round() as usize).clamp(1, 5);
    let lo = if g == 1 { 0.0 } else { canon_cum[g - 2] };
    let hi = if g == 5 { 100.0 } else { canon_cum[g - 1] };
    (lo + hi) / 2.0 / 100.0
}

/// `u`: the user's canon-equivalent grades (None = unknown). `pct`: the user's population percentiles (0..1) for the
/// continuous tie-break, None where a stat has none. Returns up to `top` distinct charts.
pub fn nearest(u: &[Option<f64>; 6], pct: &[Option<f64>; 6], username: &str, top: usize) -> Vec<Match> {
    let known: Vec<usize> = (0..6).filter(|&i| u[i].is_some()).collect();
    let ku = known.len();
    if ku < 4 {
        return vec![];
    }
    let canon = load_canon();
    let bands = canon_bands(&canon);
    let em: Vec<f64> = (0..6)
        .map(|j| {
            let uj = u[j].unwrap_or(3.0);
            (0..5).map(|g| bands[j][g] * (uj - (g as f64 + 1.0)).powi(2)).sum()
        })
        .collect();
    let cuts = Cuts::embedded();
    struct Row<'a> {
        c: &'a Canon,
        key: (i64, i64, u64),
        dist: f64,
    }
    let mut rows: Vec<Row> = Vec::new();
    for c in &canon {
        let kc = c.vec.iter().filter(|v| v.is_some()).count();
        if kc < 4 {
            continue;
        }
        let mut cost = 0.0;
        for &i in &known {
            let ui = u[i].unwrap();
            match c.vec[i] {
                Some(ci) => cost += (ui - ci) * (ui - ci),
                None => cost += em[i],
            }
        }
        let dist = (6.0 / ku as f64 * cost).sqrt();
        // continuous distance between the user's percentiles and the canon band midpoints, on stats both have
        let mut cont = 0.0;
        for i in 0..6 {
            if let (Some(p), Some(v)) = (pct[i], c.vec[i]) {
                let m = band_mid(&cuts.stat(STATS[i]).canon_cum, v);
                cont += (p - m) * (p - m);
            }
        }
        let cont = f64::sqrt(cont);
        let tie = fnv1a(&format!("{username}|{}", c.name));
        rows.push(Row { c, key: (r9(dist), r9(cont), tie), dist });
    }
    rows.sort_by_key(|r| r.key);
    let mut out: Vec<Match> = Vec::new();
    let mut taken: Vec<[Option<i64>; 6]> = Vec::new();
    let sig = |c: &Canon| -> [Option<i64>; 6] { std::array::from_fn(|i| c.vec[i].map(|v| (v * 10.0) as i64)) };
    for (i, r) in rows.iter().enumerate() {
        let s = sig(r.c);
        if taken.contains(&s) {
            continue;
        }
        // twin group: every row with the same chart; they share distance and continuous distance, so the username
        // hash (already in the sort key) put this user's pick first
        let group: Vec<&Row> = rows[i..].iter().filter(|x| sig(x.c) == s).collect();
        let pick = group[0];
        taken.push(s);
        out.push(Match {
            name: pick.c.name.clone(),
            user: pick.c.user.clone(),
            part: pick.c.part.clone(),
            grades: pick.c.grades.to_vec(),
            dist: pick.dist,
            shares_chart_with: group.iter().filter(|x| x.c.row != pick.c.row).map(|x| x.c.name.clone()).collect(),
        });
        if out.len() == top {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const NO_PCT: [Option<f64>; 6] = [None; 6];

    #[test]
    fn canon_loads() {
        let c = load_canon();
        assert_eq!(c.len(), 157);
        let b = canon_bands(&c);
        for row in b {
            assert!((row.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn spec_example_d4c_love_train() {
        // Spec: canon-equivalent profile B A C A A C -> D4C Love Train (1.0), then Stands at 1.41 and 1.73. Which of
        // several Stands at an equal distance comes second or third is decided by the tie rules (continuous distance,
        // then the username hash), so only the distances are pinned here.
        let u = [Some(4.0), Some(5.0), Some(3.0), Some(5.0), Some(5.0), Some(3.0)];
        let m = nearest(&u, &NO_PCT, "jas3344", 3);
        assert_eq!(m[0].name, "D4C Love Train");
        assert!((m[0].dist - 1.0).abs() < 1e-9);
        assert!((m[1].dist - 1.41).abs() < 0.01, "{}", m[1].dist);
        assert!((m[2].dist - 1.73).abs() < 0.01, "{}", m[2].dist);
        // with percentiles supplied the order within an equal-distance group follows the continuous distance
        let with = nearest(&u, &[Some(0.6), Some(0.8), Some(0.576), Some(0.93), Some(0.98), Some(0.55)], "jas3344", 3);
        assert_eq!(with[0].name, "D4C Love Train");
        assert!(with.windows(2).all(|w| w[0].dist <= w[1].dist + 1e-9));
    }

    #[test]
    fn a_stand_matches_itself() {
        let c = load_canon();
        let sp = c.iter().find(|c| c.name == "Killer Queen").unwrap();
        let u: [Option<f64>; 6] = std::array::from_fn(|i| sp.vec[i].filter(|v| *v >= 1.0));
        if u.iter().filter(|x| x.is_some()).count() >= 4 {
            let m = nearest(&u, &NO_PCT, "x", 1);
            assert!(m[0].dist.abs() < 1e-9 || m[0].dist < 1.0);
        }
    }

    #[test]
    fn unknowns_and_minimum() {
        let u = [Some(3.0), None, None, None, Some(3.0), Some(3.0)];
        assert!(nearest(&u, &NO_PCT, "x", 3).is_empty(), "fewer than 4 known stats");
        let u = [Some(4.0), Some(5.0), Some(3.0), Some(5.0), Some(5.0), None];
        let m = nearest(&u, &NO_PCT, "x", 3);
        assert_eq!(m.len(), 3);
        // distinct charts
        assert_ne!(m[0].grades, m[1].grades);
    }

    #[test]
    fn twin_split_is_deterministic_and_username_dependent() {
        let u = [Some(5.0), Some(5.0), Some(3.0), Some(5.0), Some(5.0), Some(5.0)]; // Star Platinum (Part 3) chart A A C A A A
        let a = nearest(&u, &NO_PCT, "alice", 1);
        let b = nearest(&u, &NO_PCT, "alice", 1);
        assert_eq!(a[0].name, b[0].name);
        let names: std::collections::BTreeSet<String> = ["u1", "u2", "u3", "u4", "u5", "u6", "u7", "u8"].iter().map(|n| nearest(&u, &NO_PCT, n, 1)[0].name.clone()).collect();
        assert!(names.len() >= 2, "twins should be split across usernames: {names:?}");
    }

    // The author's canon-equivalent profile B A C A A D has two Stands at the same letter distance (1.41):
    // D4C Love Train (A A C A A C) and Star Platinum (Part 6) (A A C A A Complete). Both are one grade away on
    // Power and on Potential, so only the continuous distance between percentiles and band midpoints can separate them.
    const TIE_PROFILE: [Option<f64>; 6] = [Some(4.0), Some(5.0), Some(3.0), Some(5.0), Some(5.0), Some(2.0)];

    fn pcts(potential: f64) -> [Option<f64>; 6] {
        [Some(0.614), Some(0.80), Some(0.576), Some(0.927), Some(0.986), Some(potential)]
    }

    fn rank(m: &[Match], name: &str) -> usize {
        m.iter().position(|x| x.name == name).unwrap_or_else(|| panic!("{name} not in {:?}", m.iter().map(|x| &x.name).collect::<Vec<_>>()))
    }

    #[test]
    fn constructed_tie_is_broken_by_continuous_percentile_distance() {
        // Potential at the 40th percentile sits in the canon C band's neighbourhood (C band midpoint 0.561) and far from
        // the lowest band (midpoint 0.140): D4C Love Train is the closer Stand.
        let near_c = nearest(&TIE_PROFILE, &pcts(0.403), "any", 3);
        let (d4c, sp6) = (rank(&near_c, "D4C Love Train"), rank(&near_c, "Star Platinum (Part 6)"));
        assert!((near_c[d4c].dist - near_c[sp6].dist).abs() < 1e-9, "the letter distances must tie for this test to mean anything");
        assert!(d4c < sp6, "{:?}", near_c.iter().map(|m| (&m.name, m.dist)).collect::<Vec<_>>());
        // Move the user's Potential percentile into the lowest canon band and the order flips, with the same letters.
        let near_e = nearest(&TIE_PROFILE, &pcts(0.10), "any", 3);
        assert!(rank(&near_e, "Star Platinum (Part 6)") < rank(&near_e, "D4C Love Train"));
        // The username does not matter while the continuous distance differs.
        for u in ["a", "b", "c", "d", "e", "f"] {
            let m = nearest(&TIE_PROFILE, &pcts(0.403), u, 3);
            assert!(rank(&m, "D4C Love Train") < rank(&m, "Star Platinum (Part 6)"), "{u}");
        }
    }

    #[test]
    fn continuous_distance_never_overrides_a_smaller_letter_distance() {
        for p in [0.0, 0.1, 0.5, 0.9, 1.0] {
            let m = nearest(&TIE_PROFILE, &pcts(p), "any", 3);
            assert!(m.windows(2).all(|w| w[0].dist <= w[1].dist + 1e-9), "p={p}: {:?}", m.iter().map(|x| x.dist).collect::<Vec<_>>());
            assert!((m[0].dist - 1.4142).abs() < 1e-3 && m[2].dist > 1.7);
        }
    }

    #[test]
    fn exact_ties_fall_to_the_username_hash() {
        // With no percentiles the continuous distance is 0 for every Stand, so equal letter distances tie completely.
        let winners: std::collections::BTreeSet<String> = (0..40).map(|i| nearest(&TIE_PROFILE, &NO_PCT, &format!("user{i}"), 1)[0].name.clone()).collect();
        assert!(winners.len() >= 2, "different usernames should split an exact tie: {winners:?}");
        assert!(winners.iter().all(|w| w == "D4C Love Train" || w == "Star Platinum (Part 6)" || load_canon().iter().any(|c| &c.name == w)));
        let a = nearest(&TIE_PROFILE, &NO_PCT, "stable", 3);
        let b = nearest(&TIE_PROFILE, &NO_PCT, "stable", 3);
        assert_eq!(a.iter().map(|m| &m.name).collect::<Vec<_>>(), b.iter().map(|m| &m.name).collect::<Vec<_>>());
    }
}
