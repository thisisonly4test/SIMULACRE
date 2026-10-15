//! Deterministic randomness. A persona is a pure function of its [`Recipe`]:
//! every section draws from its own ChaCha stream keyed by
//! `sha256(seed ‖ section ‖ reroll-count)`, so rerolling one section never
//! disturbs the others.

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use sha2::{Digest, Sha256};

pub type SectionRng = ChaCha8Rng;

pub fn section_rng(seed: &str, section: &str, reroll: u32) -> SectionRng {
    let mut h = Sha256::new();
    h.update(seed.trim().as_bytes());
    h.update([0u8]);
    h.update(section.as_bytes());
    h.update([0u8]);
    h.update(reroll.to_le_bytes());
    ChaCha8Rng::from_seed(h.finalize().into())
}

const ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ"; // Crockford base32

/// A fresh human-friendly seed such as `SIM-7Q4K-M2XD`: 40 bits, ~1.1 trillion
/// legends before the first repeat.
pub fn random_seed() -> String {
    let mut rng = ChaCha8Rng::from_entropy();
    seed_from_u64(rng.r#gen::<u64>())
}

pub fn seed_from_u64(v: u64) -> String {
    let mut s = String::from("SIM-");
    for i in 0..8 {
        if i == 4 {
            s.push('-');
        }
        let idx = ((v >> (5 * (7 - i))) & 31) as usize;
        s.push(ALPHABET[idx] as char);
    }
    s
}

/// Derives the n-th seed of a pool (infinite scroll): stable for a given pool
/// seed, so the same page always shows the same candidates.
pub fn pool_seed(pool: &str, index: u64) -> String {
    let mut h = Sha256::new();
    h.update(pool.as_bytes());
    h.update(index.to_le_bytes());
    let d = h.finalize();
    seed_from_u64(u64::from_le_bytes(d[..8].try_into().expect("8 bytes")))
}

pub trait RngExt: Rng {
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.gen_range(0..items.len())]
    }

    fn pick_weighted<'a, T>(&mut self, items: &'a [(T, f64)]) -> &'a T {
        let total: f64 = items.iter().map(|(_, w)| w.max(0.0)).sum();
        if total <= 0.0 {
            return &items[0].0;
        }
        let mut x = self.gen_range(0.0..total);
        for (item, w) in items {
            let w = w.max(0.0);
            if x < w {
                return item;
            }
            x -= w;
        }
        &items[items.len() - 1].0
    }

    /// Draws `k` distinct items, each draw weighted, without replacement.
    fn sample_weighted<T: Clone>(&mut self, items: &[(T, f64)], k: usize) -> Vec<T> {
        let mut pool: Vec<(T, f64)> = items.to_vec();
        let mut out = Vec::with_capacity(k);
        while out.len() < k && !pool.is_empty() {
            let total: f64 = pool.iter().map(|(_, w)| w.max(0.0)).sum();
            if total <= 0.0 {
                break;
            }
            let mut x = self.gen_range(0.0..total);
            let mut idx = pool.len() - 1;
            for (i, (_, w)) in pool.iter().enumerate() {
                if x < w.max(0.0) {
                    idx = i;
                    break;
                }
                x -= w.max(0.0);
            }
            out.push(pool.remove(idx).0);
        }
        out
    }

    fn chance(&mut self, p: f64) -> bool {
        self.gen_bool(p.clamp(0.0, 1.0))
    }

    /// Box–Muller normal sample.
    fn normal(&mut self, mean: f64, sd: f64) -> f64 {
        let u1: f64 = self.gen_range(f64::EPSILON..1.0);
        let u2: f64 = self.r#gen();
        mean + sd * (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }

    fn digits(&mut self, pattern: &str) -> String {
        pattern.chars().map(|c| if c == '#' { char::from(b'0' + self.gen_range(0..10u8)) } else { c }).collect()
    }
}

impl<R: Rng + ?Sized> RngExt for R {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_inputs_same_stream() {
        let a: u64 = section_rng("SIM-1", "identity", 0).r#gen();
        let b: u64 = section_rng("SIM-1", "identity", 0).r#gen();
        assert_eq!(a, b);
    }

    #[test]
    fn sections_and_rerolls_are_independent() {
        let base: u64 = section_rng("SIM-1", "identity", 0).r#gen();
        let other: u64 = section_rng("SIM-1", "career", 0).r#gen();
        let rerolled: u64 = section_rng("SIM-1", "identity", 1).r#gen();
        assert_ne!(base, other);
        assert_ne!(base, rerolled);
    }

    #[test]
    fn seed_format() {
        let s = seed_from_u64(0xDEAD_BEEF_CAFE);
        assert_eq!(s.len(), 13);
        assert!(s.starts_with("SIM-"));
        assert_eq!(&s[8..9], "-");
        assert!(random_seed().chars().skip(4).all(|c| c == '-' || ALPHABET.contains(&(c as u8))));
    }

    #[test]
    fn pool_seeds_stable_and_distinct() {
        assert_eq!(pool_seed("p", 3), pool_seed("p", 3));
        assert_ne!(pool_seed("p", 3), pool_seed("p", 4));
    }

    #[test]
    fn weighted_sampling_without_replacement() {
        let mut r = section_rng("x", "y", 0);
        let items = vec![("a", 1.0), ("b", 1.0), ("c", 0.0)];
        let got = r.sample_weighted(&items, 3);
        assert_eq!(got.len(), 2, "zero weight never drawn");
        assert!(got.contains(&"a") && got.contains(&"b"));
    }

    #[test]
    fn digits_fill_pattern() {
        let mut r = section_rng("x", "y", 0);
        let s = r.digits("06 39 98 ## ##");
        assert!(s.starts_with("06 39 98 "));
        assert!(s.chars().filter(|c| c.is_ascii_digit()).count() == 10);
    }
}
