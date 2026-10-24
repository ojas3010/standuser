//! Stand type by stats: three rules on canon-equivalent grades, first match wins (~60% leave-one-group-out canon agreement).

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StandType {
    Automatic,
    LongDistance,
    CloseRange,
    Unclassified,
}

impl StandType {
    pub fn jp(self) -> &'static str {
        match self {
            StandType::Automatic => "遠隔自動操縦型",
            StandType::LongDistance => "遠隔操作型",
            StandType::CloseRange => "近距離パワー型",
            StandType::Unclassified => "未分類",
        }
    }
    pub fn en(self) -> &'static str {
        match self {
            StandType::Automatic => "Automatic, Far-Distance Autopilot",
            StandType::LongDistance => "Long-Distance Operation",
            StandType::CloseRange => "Close-Range Power",
            StandType::Unclassified => "Unclassified",
        }
    }
    /// Automatic is the weak rule (AUC 0.59).
    pub fn low_confidence(self) -> bool {
        self == StandType::Automatic
    }
}

/// `g` = canon-equivalent grades PWR SPD RNG STA PRC DEV (E=1..A=5, ∞ counts as A, None when "?"/∅).
pub fn classify(g: &[Option<f64>; 6]) -> StandType {
    let cap = |x: Option<f64>| x.map(|v| v.min(5.0));
    let (pwr, rng, prc) = (cap(g[0]), cap(g[2]), cap(g[4]));
    // ∅ is 0 in the matcher but unknown for the type rule
    let known = |x: Option<f64>| x.filter(|&v| v >= 1.0);
    let (pwr, rng, prc) = (known(pwr), known(rng), known(prc));
    if let (Some(r), Some(p)) = (rng, prc) {
        if r == 5.0 && p == 1.0 {
            return StandType::Automatic;
        }
    }
    if let (Some(r), Some(p)) = (rng, pwr) {
        if r >= 4.0 && p <= 3.0 {
            return StandType::LongDistance;
        }
        if r <= 3.0 && p >= 2.0 {
            return StandType::CloseRange;
        }
    }
    StandType::Unclassified
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(p: f64, r: f64, c: f64) -> [Option<f64>; 6] {
        [Some(p), Some(3.0), Some(r), Some(3.0), Some(c), Some(3.0)]
    }

    #[test]
    fn rules_in_order() {
        assert_eq!(classify(&g(3.0, 5.0, 1.0)), StandType::Automatic);
        assert_eq!(classify(&g(3.0, 4.0, 3.0)), StandType::LongDistance);
        assert_eq!(classify(&g(2.0, 3.0, 3.0)), StandType::CloseRange);
        assert_eq!(classify(&g(4.0, 4.0, 3.0)), StandType::Unclassified, "Range B with Power B passes neither rule");
        assert_eq!(classify(&g(1.0, 3.0, 3.0)), StandType::Unclassified);
    }

    #[test]
    fn infinity_counts_as_a_and_unknowns_block() {
        assert_eq!(classify(&g(3.0, 6.0, 1.0)), StandType::Automatic);
        let mut u = g(4.0, 3.0, 3.0);
        assert_eq!(classify(&u), StandType::CloseRange);
        u[2] = None;
        assert_eq!(classify(&u), StandType::Unclassified);
        u[2] = Some(0.0); // ∅
        assert_eq!(classify(&u), StandType::Unclassified);
    }

    #[test]
    fn spec_example() {
        // Ojas, canon-equivalent B A C A A C -> Close-Range Power
        let u = [Some(4.0), Some(5.0), Some(3.0), Some(5.0), Some(5.0), Some(3.0)];
        assert_eq!(classify(&u), StandType::CloseRange);
    }
}
