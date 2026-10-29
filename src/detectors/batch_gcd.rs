//! Shared-prime detection across a corpus of moduli.
//!
//! If two devices drew the same prime because of weak entropy, gcd(n_i, n_j) returns
//! that prime and factors both keys. This is the single most important detector for
//! the entropy-starvation threat model — and ECM cannot do it.
//!
//! The MVP uses the straightforward O(N^2) pairwise approach, which is correct and
//! fine for audit-sized corpora. Phase 3 replaces the inner loop with Bernstein's
//! product-tree / remainder-tree batch GCD for internet-scale corpora.

use super::{FailureMode, Signal, Verdict};
use crate::key::RsaPublicKey;
use crate::mathutil::gcd;
use num_traits::One;
use num_bigint::BigUint;
use std::time::Instant;

/// Result of a corpus-level shared-factor scan.
pub struct BatchGcdResult {
    /// One signal per key index that shared a factor with another key.
    pub signals: Vec<(usize, Signal)>,
    pub pairs_compared: usize,
    pub keys_affected: usize,
    pub elapsed_ms: u128,
}

pub fn run(keys: &[RsaPublicKey]) -> BatchGcdResult {
    let start = Instant::now();
    let one = BigUint::one();
    let n = keys.len();
    let mut shared: Vec<Option<BigUint>> = vec![None; n];
    let mut pairs = 0usize;

    for i in 0..n {
        for j in (i + 1)..n {
            pairs += 1;
            let g = gcd(&keys[i].n, &keys[j].n);
            if g > one && g < keys[i].n && g < keys[j].n {
                shared[i].get_or_insert_with(|| g.clone());
                shared[j].get_or_insert_with(|| g.clone());
            }
        }
    }

    let elapsed_ms = start.elapsed().as_millis();
    let mut signals = Vec::new();
    for (idx, maybe) in shared.into_iter().enumerate() {
        if let Some(factor) = maybe {
            signals.push((
                idx,
                Signal {
                    detector: "batch_gcd".to_string(),
                    verdict: Verdict::FactorRecovered,
                    failure_mode: FailureMode::SharedPrime,
                    confidence: 1.0,
                    detail: format!(
                        "shared {}-bit prime with another key in the corpus",
                        factor.bits()
                    ),
                    cost_ms: elapsed_ms,
                    recovered_factor: Some(factor),
                },
            ));
        }
    }

    let keys_affected = signals.len();
    BatchGcdResult { signals, pairs_compared: pairs, keys_affected, elapsed_ms }
}
