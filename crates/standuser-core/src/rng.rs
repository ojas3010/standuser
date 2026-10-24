//! Deterministic RNG shared with the Python reference (tools/golden/ref.py): SplitMix64 and a partial Fisher-Yates.

pub struct SplitMix64(u64);

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        SplitMix64(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform integer in 0..n.
    pub fn below(&mut self, n: u64) -> u64 {
        (((self.next_u64() >> 11) as u128 * n as u128) >> 53) as u64
    }

    /// k distinct indices out of 0..n, in draw order.
    pub fn sample(&mut self, n: usize, k: usize) -> Vec<usize> {
        let mut v: Vec<usize> = (0..n).collect();
        for i in 0..k.min(n) {
            let j = i + self.below((n - i) as u64) as usize;
            v.swap(i, j);
        }
        v.truncate(k.min(n));
        v
    }
}

/// FNV-1a 64 over the lower-cased UTF-8 bytes; splits identical-chart Stand twins by username.
pub fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.to_lowercase().bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix_reference_values() {
        // Reference values from the published SplitMix64 for seed 1234567.
        let mut r = SplitMix64::new(1234567);
        assert_eq!(r.next_u64(), 6457827717110365317);
        assert_eq!(r.next_u64(), 3203168211198807973);
    }

    #[test]
    fn sample_is_distinct() {
        let mut r = SplitMix64::new(5);
        let s = r.sample(100, 50);
        let mut u = s.clone();
        u.sort();
        u.dedup();
        assert_eq!(u.len(), 50);
    }

    #[test]
    fn fnv_known() {
        assert_eq!(fnv1a(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a("a"), 0xaf63_dc4c_8601_ec8c);
    }
}
