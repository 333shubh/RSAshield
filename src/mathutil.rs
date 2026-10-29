//! Big-integer number-theory helpers used across detectors and the benchmark.
//!
//! All factoring-relevant math goes through here so the detectors stay small and
//! auditable. Nothing here is secret-dependent beyond the modulus being analysed.

use num_bigint::{BigUint, RandBigInt};
use num_integer::Integer;
use num_traits::{One, Zero};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;

/// Integer square root (floor) of `n`.
pub fn isqrt(n: &BigUint) -> BigUint {
    n.sqrt()
}

/// True if `n` is a perfect square.
pub fn is_perfect_square(n: &BigUint) -> bool {
    let r = n.sqrt();
    &(&r * &r) == n
}

/// Greatest common divisor.
pub fn gcd(a: &BigUint, b: &BigUint) -> BigUint {
    a.gcd(b)
}

/// First `SMALL_PRIMES` primes, used for trial division and as Miller-Rabin bases.
pub const SMALL_PRIMES: [u64; 25] = [
    2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
];

/// Miller-Rabin primality test. Deterministic over the small-prime bases plus
/// `extra_rounds` random-base rounds. Good enough for generating and sanity-checking
/// synthetic benchmark keys; not a certified primality proof.
pub fn is_probable_prime<R: Rng + ?Sized>(n: &BigUint, extra_rounds: u32, rng: &mut R) -> bool {
    let two = BigUint::from(2u32);
    if n < &two {
        return false;
    }
    for &p in SMALL_PRIMES.iter() {
        let bp = BigUint::from(p);
        if n == &bp {
            return true;
        }
        if (n % &bp).is_zero() {
            return false;
        }
    }
    // write n-1 = d * 2^s
    let one = BigUint::one();
    let n_minus_1 = n - &one;
    let mut d = n_minus_1.clone();
    let mut s = 0u32;
    while d.is_even() {
        d >>= 1;
        s += 1;
    }

    let mut witnesses: Vec<BigUint> = SMALL_PRIMES.iter().map(|&p| BigUint::from(p)).collect();
    for _ in 0..extra_rounds {
        // random base in [2, n-2]
        let a = rng.gen_biguint_range(&two, &(n - &two));
        witnesses.push(a);
    }

    'witness: for a in witnesses {
        if &a >= n {
            continue;
        }
        let mut x = a.modpow(&d, n);
        if x == one || x == n_minus_1 {
            continue;
        }
        for _ in 0..s.saturating_sub(1) {
            x = x.modpow(&two, n);
            if x == n_minus_1 {
                continue 'witness;
            }
        }
        return false;
    }
    true
}

/// Generate a random probable prime of exactly `bits` bits (top bit set, odd).
pub fn random_prime<R: Rng + ?Sized>(bits: u64, rng: &mut R) -> BigUint {
    let one = BigUint::one();
    loop {
        let mut candidate = rng.gen_biguint(bits);
        // force top bit (exact size) and odd
        candidate.set_bit(bits - 1, true);
        candidate |= &one;
        if is_probable_prime(&candidate, 8, rng) {
            return candidate;
        }
    }
}

/// Generate a reproducible prime of `bits` bits from a `u64` seed (for tests/demos).
pub fn random_prime_seeded(bits: u64, seed: u64) -> BigUint {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    random_prime(bits, &mut rng)
}

/// Return the next probable prime >= `start`.
pub fn next_prime<R: Rng + ?Sized>(start: &BigUint, rng: &mut R) -> BigUint {
    let one = BigUint::one();
    let two = BigUint::from(2u32);
    let mut c = if start.is_even() { start + &one } else { start.clone() };
    loop {
        if is_probable_prime(&c, 8, rng) {
            return c;
        }
        c += &two;
    }
}

/// Modular inverse of `a` mod `m`, if it exists, via the extended Euclidean algorithm
/// on signed integers.
pub fn mod_inverse(a: &BigUint, m: &BigUint) -> Option<BigUint> {
    use num_bigint::BigInt;
    use num_integer::Integer as _;
    let a = BigInt::from(a.clone());
    let m = BigInt::from(m.clone());
    let g = a.extended_gcd(&m);
    if !g.gcd.is_one() {
        return None;
    }
    let mut x = g.x % &m;
    if x < BigInt::zero() {
        x += &m;
    }
    x.to_biguint()
}
