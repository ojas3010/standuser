//! Grades: card letters from population percentiles (E10/D30/C70/B90/A) and canon-equivalent grades for typing and matching.
//!
//! Cut-points and percentile knots come from data/cuts.json (built by tools/build_tables.py). Percentiles are read by
//! interpolating the probit of the knot percentiles against the value, with the end segments extended linearly.

use serde::{Deserialize, Serialize};

pub const STATS: [&str; 6] = ["PWR", "SPD", "RNG", "STA", "PRC", "DEV"];
pub const LABEL: [&str; 6] = ["Power", "Speed", "Range", "Stamina", "Precision", "Potential"];

#[derive(Debug, Clone, Deserialize)]
pub struct StatCuts {
    pub knots: Option<Vec<[f64; 2]>>,
    pub letter_cuts: Option<Vec<f64>>,
    pub canon_cuts: Option<Vec<f64>>,
    pub canon_cum: Vec<f64>,
    /// Value above which a stat becomes ∞ (99.9th percentile of the calibration population).
    #[serde(default)]
    pub inf_cut: Option<f64>,
    pub status: String,
    pub source: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Cuts {
    pub stats: std::collections::BTreeMap<String, StatCuts>,
    pub mu_b: f64,
    #[serde(rename = "lambda_")]
    pub lambda: f64,
    pub pa_pop: f64,
    pub crowd_min_ratings: u64,
}

impl Cuts {
    pub fn embedded() -> Cuts {
        serde_json::from_str(include_str!("../data/cuts.json")).expect("cuts.json")
    }

    pub fn stat(&self, s: &str) -> &StatCuts {
        &self.stats[s]
    }
}

/// Standard normal CDF through the erf approximation below (error below 2e-7).
pub fn phi(x: f64) -> f64 {
    0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
}

fn erf(x: f64) -> f64 {
    // Numerical Recipes erfc Chebyshev fit (fractional error below 1.2e-7 everywhere).
    let z = x.abs();
    let t = 1.0 / (1.0 + 0.5 * z);
    let r = t * (-z * z - 1.26551223 + t * (1.00002368 + t * (0.37409196 + t * (0.09678418 + t * (-0.18628806 + t * (0.27886807 + t * (-1.13520398 + t * (1.48851587 + t * (-0.82215223 + t * 0.17087277))))))))).exp();
    if x >= 0.0 { 1.0 - r } else { r - 1.0 }
}

/// Inverse normal CDF (Acklam's rational approximation, relative error below 1.15e-9).
pub fn probit(p: f64) -> f64 {
    let p = p.clamp(1e-12, 1.0 - 1e-12);
    const A: [f64; 6] = [-3.969683028665376e+01, 2.209460984245205e+02, -2.759285104469687e+02, 1.383577518672690e+02, -3.066479806614716e+01, 2.506628277459239e+00];
    const B: [f64; 5] = [-5.447609879822406e+01, 1.615858368580409e+02, -1.556989798598866e+02, 6.680131188771972e+01, -1.328068155288572e+01];
    const C: [f64; 6] = [-7.784894002430293e-03, -3.223964580411365e-01, -2.400758277161838e+00, -2.549732539343734e+00, 4.374664141464968e+00, 2.938163982698783e+00];
    const D: [f64; 4] = [7.784695709041462e-03, 3.224671290700398e-01, 2.445134137142996e+00, 3.754408661907416e+00];
    let pl = 0.02425;
    let x = if p < pl {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5]) / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p <= 1.0 - pl {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5]) / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    };
    x
}

impl StatCuts {
    /// Population percentile (0..1) of `v`, or None when the stat has no knots.
    pub fn percentile(&self, v: f64) -> Option<f64> {
        let k = self.knots.as_ref()?;
        if k.len() < 2 {
            return None;
        }
        let z: Vec<f64> = k.iter().map(|kn| probit(kn[1])).collect();
        let x: Vec<f64> = k.iter().map(|kn| kn[0]).collect();
        let n = x.len();
        let i = if v <= x[0] {
            0
        } else if v >= x[n - 1] {
            n - 2
        } else {
            x.iter().rposition(|&a| a <= v).unwrap().min(n - 2)
        };
        let zv = z[i] + (z[i + 1] - z[i]) * (v - x[i]) / (x[i + 1] - x[i]);
        Some(phi(zv))
    }

    /// Card letter 1..5 (E..A) by the percentile cut-points; a value on a cut goes to the higher grade.
    pub fn letter(&self, v: f64) -> Option<u8> {
        let c = self.letter_cuts.as_ref()?;
        Some(1 + c.iter().filter(|&&x| v >= x).count() as u8)
    }

    /// Canon-equivalent grade 1..5: value cuts when shipped, else the percentile through canon's cumulative shares.
    pub fn canon_grade(&self, v: f64) -> Option<u8> {
        if let Some(c) = &self.canon_cuts {
            return Some(1 + c.iter().filter(|&&x| v >= x).count() as u8);
        }
        let p = self.percentile(v)? * 100.0;
        Some(1 + self.canon_cum.iter().filter(|&&x| p >= x).count() as u8)
    }
}

pub fn letter_char(g: u8) -> &'static str {
    match g {
        1 => "E",
        2 => "D",
        3 => "C",
        4 => "B",
        _ => "A",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Measured,
    Provisional,
    /// A value exists but there are no valid cut-points for it.
    Ungraded,
    /// Below the stat's minimum data ("?").
    Unknown,
    /// The input feature was never used ("∅").
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Symbol {
    Inf,
    Complete,
}

/// One graded stat as the card, type rule, matcher and ability engine consume it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grade {
    pub stat: String,
    pub value: Option<f64>,
    /// Population percentile 0..1 (None without knots).
    pub p: Option<f64>,
    /// Card letter: "A".."E", "∞", "完成", "?", "∅".
    pub letter: String,
    pub status: Status,
    pub symbol: Option<Symbol>,
    /// Canon-equivalent numeric grade (E=1..A=5, ∞=6, 完成=1, ∅=0). None when unknown.
    pub canon: Option<f64>,
    /// Card polygon radius step (as `canon` but "?" = 3).
    pub plot: f64,
    /// Why the stat has this status, shown as the evidence line when not measured.
    pub note: String,
}

impl Grade {
    pub fn unknown(stat: &str, value: Option<f64>, note: &str) -> Grade {
        Grade { stat: stat.into(), value, p: None, letter: "?".into(), status: Status::Unknown, symbol: None, canon: None, plot: 3.0, note: note.into() }
    }
    pub fn none(stat: &str, note: &str) -> Grade {
        Grade { stat: stat.into(), value: None, p: None, letter: "∅".into(), status: Status::None, symbol: None, canon: Some(0.0), plot: 0.0, note: note.into() }
    }
    pub fn is_graded(&self) -> bool {
        matches!(self.status, Status::Measured | Status::Provisional)
    }
}

/// Grade a computed value. `provisional` marks sample-size or calibration caveats.
pub fn grade_value(cuts: &Cuts, stat: &str, v: f64, provisional: bool, note: &str) -> Grade {
    let sc = cuts.stat(stat);
    let calibrated = sc.letter_cuts.is_some();
    let g = sc.letter(v);
    let status = if !calibrated {
        Status::Ungraded
    } else if provisional || sc.status == "provisional" {
        Status::Provisional
    } else {
        Status::Measured
    };
    match g {
        Some(g) => Grade {
            stat: stat.into(),
            value: Some(v),
            p: sc.percentile(v),
            letter: letter_char(g).into(),
            status,
            symbol: None,
            canon: sc.canon_grade(v).map(f64::from),
            plot: g as f64,
            note: note.into(),
        },
        None => Grade { stat: stat.into(), value: Some(v), p: None, letter: "?".into(), status: Status::Ungraded, symbol: None, canon: None, plot: 3.0, note: note.into() },
    }
}

/// Turn Potential into 完成: the vertex stays at E level, the percentile is the extreme low end.
pub fn make_complete(mut g: Grade) -> Grade {
    g.letter = "完成".into();
    g.symbol = Some(Symbol::Complete);
    g.canon = Some(1.0);
    g.plot = 1.0;
    g.p = Some(0.001);
    g
}

/// Turn a graded stat into the ∞ symbol (vertex on the ring).
pub fn make_inf(mut g: Grade) -> Grade {
    g.letter = "∞".into();
    g.symbol = Some(Symbol::Inf);
    g.canon = Some(6.0);
    g.plot = 6.0;
    g.p = Some(0.999);
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probit_phi_roundtrip() {
        for p in [0.001, 0.05, 0.3, 0.5, 0.9, 0.999] {
            assert!((phi(probit(p)) - p).abs() < 2e-7, "{p}");
        }
    }

    #[test]
    fn knots_reproduce_and_extrapolate() {
        let c = Cuts::embedded();
        for stat in ["PWR", "PRC", "RNG", "SPD", "STA", "DEV"] {
            let sc = c.stat(stat);
            let k = sc.knots.as_ref().unwrap();
            for kn in k {
                assert!((sc.percentile(kn[0]).unwrap() - kn[1]).abs() < 1e-6, "{stat} {kn:?}");
            }
            // monotone, and tails extend past the last knot
            let (lo, hi) = (k[0][0], k[k.len() - 1][0]);
            assert!(sc.percentile(lo - 1.0).unwrap() < k[0][1]);
            assert!(sc.percentile(hi + 1.0).unwrap() > k[k.len() - 1][1]);
        }
    }

    #[test]
    fn letters_follow_the_cuts() {
        let c = Cuts::embedded();
        for stat in ["PWR", "PRC", "RNG", "SPD", "STA", "DEV"] {
            let cuts = c.stat(stat).letter_cuts.clone().unwrap();
            let sc = c.stat(stat);
            assert_eq!(sc.letter(cuts[0] - 1e-3), Some(1), "{stat}");
            for (i, &x) in cuts.iter().enumerate() {
                assert_eq!(sc.letter(x), Some(i as u8 + 2), "{stat}: a value on a cut goes to the higher grade");
            }
            assert_eq!(sc.letter(cuts[3] + 1.0), Some(5));
            // the letter cuts sit at the 10/30/70/90th percentiles of the population they were cut from
            for (x, p) in cuts.iter().zip([0.10, 0.30, 0.70, 0.90]) {
                assert!((sc.percentile(*x).unwrap() - p).abs() < 0.02, "{stat} {x} {p}");
            }
            // canon-equivalent grade is monotone in the value
            let cg = |v: f64| sc.canon_grade(v).unwrap();
            assert!(cg(cuts[0] - 10.0) <= cg(cuts[1]) && cg(cuts[1]) <= cg(cuts[3] + 10.0));
        }
        // Speed and Stamina keep the MovieTweetings cuts of the spec
        assert_eq!(c.stat("SPD").letter(4.978), Some(4));
        assert_eq!(c.stat("STA").letter(0.900), Some(5));
        assert_eq!(c.stat("SPD").canon_grade(4.978), Some(5));
    }
}
