//! Synthetic corpus generator with controlled fault injection.
//!
//! Every key is produced from a seeded RNG so the whole corpus is reproducible. For
//! each class we record the ground-truth factors so the evaluator can score detectors
//! against known answers — something internet-scan datasets cannot provide.

use super::{DefectClass, GroundTruth};
use crate::key::RsaPublicKey;
use crate::mathutil::{gcd, mod_inverse, next_prime, random_prime};
use num_bigint::{BigUint, RandBigInt};
use num_traits::One;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;

/// How many keys of each class to generate.
#[derive(Clone, Copy, Debug)]
pub struct CorpusSpec {
    pub bits: u64,
    pub healthy: usize,
    pub shared_prime: usize,
    pub close_prime: usize,
    pub small_factor: usize,
    pub small_d: usize,
}

impl Default for CorpusSpec {
    fn default() -> Self {
        CorpusSpec { bits: 512, healthy: 6, shared_prime: 3, close_prime: 3, small_factor: 3, small_d: 3 }
    }
}

pub struct Corpus {
    pub keys: Vec<RsaPublicKey>,
    pub truth: Vec<GroundTruth>,
}

const E_STD: u32 = 65537;

pub fn generate(spec: &CorpusSpec, seed: u64) -> Corpus {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let mut keys = Vec::new();
    let mut truth = Vec::new();
    let half = spec.bits / 2;
    let e_std = BigUint::from(E_STD);

    let push = |keys: &mut Vec<RsaPublicKey>,
                    truth: &mut Vec<GroundTruth>,
                    class: DefectClass,
                    n: BigUint,
                    e: BigUint,
                    p: BigUint,
                    q: BigUint| {
        let idx = keys.len();
        truth.push(GroundTruth {
            index: idx,
            class,
            bits: n.bits(),
            n_hex: n.to_str_radix(16),
            e_hex: e.to_str_radix(16),
            p_hex: p.to_str_radix(16),
            q_hex: q.to_str_radix(16),
        });
        keys.push(RsaPublicKey::new(n, e, format!("{class:?}#{idx}")));
    };

    // healthy: balanced primes, standard exponent
    for _ in 0..spec.healthy {
        let (n, p, q) = balanced_semiprime(half, &mut rng);
        push(&mut keys, &mut truth, DefectClass::Healthy, n, e_std.clone(), p, q);
    }

    // shared prime: one prime reused across a family
    if spec.shared_prime > 0 {
        let shared = random_prime(half, &mut rng);
        for _ in 0..spec.shared_prime {
            let q = random_prime(half, &mut rng);
            let n = &shared * &q;
            push(&mut keys, &mut truth, DefectClass::SharedPrime, n, e_std.clone(), shared.clone(), q);
        }
    }

    // close primes: q just above p
    for _ in 0..spec.close_prime {
        let p = random_prime(half, &mut rng);
        let delta = BigUint::from(2u32 + (rng.gen_biguint(10) % BigUint::from(500u32)).to_u32_digits().first().copied().unwrap_or(0));
        let q = next_prime(&(&p + &delta), &mut rng);
        let n = &p * &q;
        push(&mut keys, &mut truth, DefectClass::ClosePrime, n, e_std.clone(), p, q);
    }

    // small factor: a tiny prime times a large one (unbalanced)
    for _ in 0..spec.small_factor {
        let small = small_prime(&mut rng); // < trial-division bound
        let big = random_prime(spec.bits - small.bits(), &mut rng);
        let n = &small * &big;
        push(&mut keys, &mut truth, DefectClass::SmallFactor, n, e_std.clone(), small, big);
    }

    // small private exponent (Wiener)
    for _ in 0..spec.small_d {
        let (n, e, p, q) = wiener_key(spec.bits, half, &mut rng);
        push(&mut keys, &mut truth, DefectClass::SmallD, n, e, p, q);
    }

    Corpus { keys, truth }
}

fn balanced_semiprime(half_bits: u64, rng: &mut ChaCha20Rng) -> (BigUint, BigUint, BigUint) {
    loop {
        let p = random_prime(half_bits, rng);
        let q = random_prime(half_bits, rng);
        if p == q {
            continue;
        }
        let phi = (&p - BigUint::one()) * (&q - BigUint::one());
        if gcd(&BigUint::from(E_STD), &phi) == BigUint::one() {
            let n = &p * &q;
            return (n, p, q);
        }
    }
}

/// A prime small enough to be caught by trial division (< ~90000).
fn small_prime(rng: &mut ChaCha20Rng) -> BigUint {
    use crate::mathutil::is_probable_prime;
    loop {
        let v = (rng.gen_biguint(17) % BigUint::from(89000u32)) + BigUint::from(1000u32);
        if is_probable_prime(&v, 8, rng) {
            return v;
        }
    }
}

fn wiener_key(bits: u64, half: u64, rng: &mut ChaCha20Rng) -> (BigUint, BigUint, BigUint, BigUint) {
    let one = BigUint::one();
    loop {
        let p = random_prime(half, rng);
        let q = random_prime(half, rng);
        if p == q {
            continue;
        }
        let n = &p * &q;
        let phi = (&p - &one) * (&q - &one);
        // d must be smaller than ~ (1/3) n^(1/4); use (bits/4 - 3) bits to be safe
        let d_bits = (bits / 4).saturating_sub(3).max(16);
        let mut d = rng.gen_biguint(d_bits);
        d.set_bit(d_bits - 1, true);
        d |= &one; // odd
        if gcd(&d, &phi) != one {
            continue;
        }
        if let Some(e) = mod_inverse(&d, &phi) {
            if e > one {
                return (n, e, p, q);
            }
        }
    }
}
