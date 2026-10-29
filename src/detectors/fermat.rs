//! Fermat factorization: recovers p, q when they are too close together.
//!
//! Writes n = a^2 - b^2 = (a-b)(a+b) by scanning a upward from ceil(sqrt(n)) and
//! testing whether a^2 - n is a perfect square. Cheap when |p - q| is small; bounded
//! by `max_iters` so a well-separated modulus returns quickly with no finding.

use super::{FailureMode, Signal, Verdict};
use crate::key::RsaPublicKey;
use crate::mathutil::{is_perfect_square, isqrt};
use num_bigint::BigUint;
use num_traits::One;
use std::time::Instant;

pub fn run(key: &RsaPublicKey, max_iters: u64) -> Signal {
    let start = Instant::now();
    let n = &key.n;
    let one = BigUint::one();

    let root = isqrt(n);
    // a starts at ceil(sqrt(n))
    let mut a = if &(&root * &root) < n { &root + &one } else { root };

    let mut iters = 0u64;
    while iters < max_iters {
        let a2 = &a * &a;
        // a2 >= n here by construction
        let b2 = &a2 - n;
        if is_perfect_square(&b2) {
            let b = b2.sqrt();
            let p = &a - &b;
            let q = &a + &b;
            if &(&p * &q) == n && p > one {
                return Signal {
                    detector: "fermat".to_string(),
                    verdict: Verdict::FactorRecovered,
                    failure_mode: FailureMode::ClosePrimes,
                    confidence: 1.0,
                    detail: format!(
                        "close primes: factored after {iters} step(s); prime gap is {}-bit",
                        if q > p { (&q - &p).bits() } else { 0 }
                    ),
                    cost_ms: start.elapsed().as_millis(),
                    recovered_factor: Some(p),
                };
            }
        }
        a += &one;
        iters += 1;
    }

    Signal::clean("fermat", start.elapsed().as_millis())
}
