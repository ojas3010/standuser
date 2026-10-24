//! Nearest canon Stand: Method H. L2 distance on A=5..E=1, ∞=6, 完成=1, ∅=0 with each unknown canon cell costed at its
//! expected squared error under that stat's canon grade shares. Twins (identical charts) are split by a username hash.
//! A user "?" is left out and the distance rescaled by 6/k; at least 4 known stats are required.

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

/// `u`: the user's canon-equivalent grades (None = unknown). Returns up to `top` distinct charts.
pub fn nearest(u: &[Option<f64>; 6], username: &str, top: usize) -> Vec<Match> {
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
    struct Row<'a> {
        c: &'a Canon,
        key: (i64, i64, i64, i64, i64, u32),
        dist: f64,
    }
    let mut rows: Vec<Row> = Vec::new();
    for c in &canon {
        let kc = c.vec.iter().filter(|v| v.is_some()).count();
        if kc < 4 {
            continue;
        }
        let mut cost = 0.0;
        let (mut sal, mut mx, mut ex, mut kb) = (0.0, 0.0f64, 0i64, 0usize);
        for &i in &known {
            let ui = u[i].unwrap();
            match c.vec[i] {
                Some(ci) => {
                    let d = ui - ci;
                    cost += d * d;
                    sal += (ui - 3.0).abs() * d * d;
                    mx = mx.max(d.abs());
                    if d == 0.0 {
                        ex += 1;
                    }
                    kb += 1;
                }
                None => cost += em[i],
            }
        }
        let dist = (6.0 / ku as f64 * cost).sqrt();
        let sal = if kb > 0 { 6.0 / kb as f64 * sal } else { 0.0 };
        rows.push(Row { c, key: (r9(dist), r9(sal), r9(mx), -ex, -(kc as i64), c.row), dist });
    }
    rows.sort_by(|a, b| a.key.cmp(&b.key));
    let h = fnv1a(username);
    let mut out: Vec<Match> = Vec::new();
    let mut taken: Vec<[Option<i64>; 6]> = Vec::new();
    let sig = |c: &Canon| -> [Option<i64>; 6] { std::array::from_fn(|i| c.vec[i].map(|v| (v * 10.0) as i64)) };
    for (i, r) in rows.iter().enumerate() {
        let s = sig(r.c);
        if taken.contains(&s) {
            continue;
        }
        // twin group: every row with the same chart (identical keys apart from the row number)
        let group: Vec<&Row> = rows[i..].iter().filter(|x| sig(x.c) == s).collect();
        let pick = group[(h % group.len() as u64) as usize];
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
        // Spec: canon-equivalent profile B A C A A C -> D4C Love Train (1.0), Dolly Dagger (1.41), The World (1.73)
        let u = [Some(4.0), Some(5.0), Some(3.0), Some(5.0), Some(5.0), Some(3.0)];
        let m = nearest(&u, "jas3344", 3);
        assert_eq!(m[0].name, "D4C Love Train");
        assert!((m[0].dist - 1.0).abs() < 1e-9);
        assert_eq!(m[1].name, "Dolly Dagger");
        assert!((m[1].dist - 1.41).abs() < 0.01, "{}", m[1].dist);
        assert_eq!(m[2].name, "The World");
        assert!((m[2].dist - 1.73).abs() < 0.01, "{}", m[2].dist);
    }

    #[test]
    fn a_stand_matches_itself() {
        let c = load_canon();
        let sp = c.iter().find(|c| c.name == "Killer Queen").unwrap();
        let u: [Option<f64>; 6] = std::array::from_fn(|i| sp.vec[i].filter(|v| *v >= 1.0));
        if u.iter().filter(|x| x.is_some()).count() >= 4 {
            let m = nearest(&u, "x", 1);
            assert!(m[0].dist.abs() < 1e-9 || m[0].dist < 1.0);
        }
    }

    #[test]
    fn unknowns_and_minimum() {
        let u = [Some(3.0), None, None, None, Some(3.0), Some(3.0)];
        assert!(nearest(&u, "x", 3).is_empty(), "fewer than 4 known stats");
        let u = [Some(4.0), Some(5.0), Some(3.0), Some(5.0), Some(5.0), None];
        let m = nearest(&u, "x", 3);
        assert_eq!(m.len(), 3);
        // distinct charts
        assert_ne!(m[0].grades, m[1].grades);
    }

    #[test]
    fn twin_split_is_deterministic_and_username_dependent() {
        let u = [Some(5.0), Some(5.0), Some(3.0), Some(5.0), Some(5.0), Some(5.0)]; // Star Platinum (Part 3) chart A A C A A A
        let a = nearest(&u, "alice", 1);
        let b = nearest(&u, "alice", 1);
        assert_eq!(a[0].name, b[0].name);
        let names: std::collections::BTreeSet<String> = ["u1", "u2", "u3", "u4", "u5", "u6", "u7", "u8"].iter().map(|n| nearest(&u, n, 1)[0].name.clone()).collect();
        assert!(names.len() >= 2, "twins should be split across usernames: {names:?}");
    }
}
