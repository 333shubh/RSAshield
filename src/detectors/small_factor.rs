//! Trial division against small primes and small odd numbers up to a bound.
//!
//! This recovers *tiny* factors (implementation bugs, truncated primes). It is the
//! cheap front end to the "small/unbalanced factor" failure mode; medium unbalanced
//! factors are ECM's territory (Phase 4) and are intentionally out of reach here, so
//! the benchmark can show where trial division stops and ECM would be needed.

use super::{FailureMode, Signal, Verdict};
use crate::key::RsaPublicKey;
use crate::mathutil::SMALL_PRIMES;
use num_bigint::BigUint;
use num_traits::Zero;
use crate::clock::Instant;

pub fn run(key: &RsaPublicKey, bound: u64) -> Signal {
    let start = Instant::now();

    let mut found: Option<u64> = None;
    for &p in SMALL_PRIMES.iter() {
        if (&key.n % BigUint::from(p)).is_zero() && &key.n != &BigUint::from(p) {
            found = Some(p);
            break;
        }
    }
    if found.is_none() {
        // scan odd numbers from 101 up to the bound
        let mut d: u64 = 101;
        while d <= bound {
            if (&key.n % BigUint::from(d)).is_zero() {
                found = Some(d);
                break;
            }
            d += 2;
        }
    }

    match found {
        Some(f) => {
            let factor = BigUint::from(f);
            let cofactor = &key.n / &factor;
            Signal {
                detector: "small_factor".to_string(),
                verdict: Verdict::FactorRecovered,
                failure_mode: FailureMode::SmallOrUnbalancedFactor,
                confidence: 1.0,
                detail: format!(
                    "modulus has a {}-bit small factor (recoverable by trial division); cofactor is {}-bit",
                    factor.bits(),
                    cofactor.bits()
                ),
                cost_ms: start.elapsed().as_millis(),
                recovered_factor: Some(factor),
            }
        }
        None => Signal::clean("small_factor", start.elapsed().as_millis()),
    }
}
