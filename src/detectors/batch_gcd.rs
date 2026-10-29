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
use num_bigint::BigUint;
use num_traits::One;
use crate::clock::Instant;

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

// ---- Phase 3: Bernstein product-tree / remainder-tree batch GCD -------------
//
// Computes, for every modulus N_i, gcd(N_i, (P mod N_i^2)/N_i) where P is the product
// of all moduli. That gcd is the part of N_i shared with any other modulus — the same
// result as the pairwise scan, but in ~O(N log^2 N) big-integer work instead of O(N^2).

/// Per-index shared-factor outcome from the tree algorithm.
#[derive(Clone, Debug, PartialEq)]
pub enum Shared {
    /// No factor shared with the rest of the corpus.
    None,
    /// A non-trivial prime factor shared with at least one other modulus.
    Factor(BigUint),
    /// The whole modulus divides the product of the others (duplicate / both primes shared).
    Duplicate,
}

/// Core tree computation over raw moduli. Returns one [`Shared`] per input index.
pub fn shared_factors_tree(ns: &[BigUint]) -> Vec<Shared> {
    let k = ns.len();
    if k < 2 {
        return vec![Shared::None; k];
    }

    // product tree, bottom-up: levels[0] = leaves, levels[last] = [root product]
    let mut levels: Vec<Vec<BigUint>> = vec![ns.to_vec()];
    while levels.last().unwrap().len() > 1 {
        let cur = levels.last().unwrap();
        let mut next = Vec::with_capacity(cur.len().div_ceil(2));
        let mut i = 0;
        while i < cur.len() {
            if i + 1 < cur.len() {
                next.push(&cur[i] * &cur[i + 1]);
            } else {
                next.push(cur[i].clone()); // odd node carried up
            }
            i += 2;
        }
        levels.push(next);
    }

    // remainder tree, top-down: start with P at the root, reduce by each child's square
    let top = levels.len() - 1;
    let mut rem: Vec<BigUint> = vec![levels[top][0].clone()]; // P
    let mut level = top;
    while level > 0 {
        let child_level = &levels[level - 1];
        let mut child_rem = Vec::with_capacity(child_level.len());
        for (ci, child) in child_level.iter().enumerate() {
            let sq = child * child;
            child_rem.push(&rem[ci / 2] % &sq);
        }
        rem = child_rem;
        level -= 1;
    }

    // leaves now hold z_i = P mod N_i^2; shared part = gcd(N_i, z_i / N_i)
    ns.iter()
        .enumerate()
        .map(|(i, n)| {
            let q = &rem[i] / n; // exact: N_i | z_i
            let g = gcd(n, &q);
            if g.is_one() {
                Shared::None
            } else if &g == n {
                Shared::Duplicate
            } else {
                Shared::Factor(g)
            }
        })
        .collect()
}

/// Corpus-level shared-prime scan using the tree algorithm. Same result shape as [`run`].
pub fn run_tree(keys: &[RsaPublicKey]) -> BatchGcdResult {
    let start = Instant::now();
    let moduli: Vec<BigUint> = keys.iter().map(|k| k.n.clone()).collect();
    let shared = shared_factors_tree(&moduli);
    let elapsed_ms = start.elapsed().as_millis();

    let mut signals = Vec::new();
    for (idx, s) in shared.into_iter().enumerate() {
        let (detail, factor) = match s {
            Shared::None => continue,
            Shared::Factor(f) => (
                format!("shared {}-bit prime with another key in the corpus", f.bits()),
                Some(f),
            ),
            Shared::Duplicate => (
                "modulus divides the product of the others (duplicate or both primes shared)"
                    .to_string(),
                None,
            ),
        };
        signals.push((
            idx,
            Signal {
                detector: "batch_gcd".to_string(),
                verdict: Verdict::FactorRecovered,
                failure_mode: FailureMode::SharedPrime,
                confidence: 1.0,
                detail,
                cost_ms: elapsed_ms,
                recovered_factor: factor,
            },
        ));
    }

    let keys_affected = signals.len();
    // pairs_compared is pairwise-specific; report 0 for the tree method.
    BatchGcdResult { signals, pairs_compared: 0, keys_affected, elapsed_ms }
}
