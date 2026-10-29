//! Wiener's attack screen: detects a dangerously small private exponent d.
//!
//! When d < (1/3) n^(1/4), the fraction e/n has a convergent k/d in its continued
//! fraction expansion that reveals d, and hence phi(n) and the factorisation. We walk
//! the convergents and, for each, test whether it yields an integer phi and integer
//! roots of x^2 - (n - phi + 1)x + n. A hit recovers the factors.

use super::{FailureMode, Signal, Verdict};
use crate::key::RsaPublicKey;
use crate::mathutil::is_perfect_square;
use num_bigint::BigUint;
use num_traits::{One, Zero};
use std::time::Instant;

pub fn run(key: &RsaPublicKey) -> Signal {
    let start = Instant::now();
    let n = &key.n;
    let e = &key.e;
    let one = BigUint::one();
    let two = BigUint::from(2u32);
    let four_n = n * BigUint::from(4u32);

    // continued-fraction coefficients of e/n
    let mut num = e.clone();
    let mut den = n.clone();
    let mut a_coeffs: Vec<BigUint> = Vec::new();
    let mut guard = 0u32;
    while !den.is_zero() && guard < 10_000 {
        let a = &num / &den;
        let rem = &num - &a * &den;
        a_coeffs.push(a);
        num = den;
        den = rem;
        guard += 1;
    }

    // convergents h_i / k_i
    let mut h_prev2 = BigUint::zero();
    let mut h_prev1 = one.clone();
    let mut k_prev2 = one.clone();
    let mut k_prev1 = BigUint::zero();

    for a in a_coeffs.iter() {
        let h = a * &h_prev1 + &h_prev2;
        let k = a * &k_prev1 + &k_prev2;
        h_prev2 = h_prev1;
        h_prev1 = h.clone();
        k_prev2 = k_prev1;
        k_prev1 = k.clone();

        // h plays the role of k (e*d = k*phi + 1); k plays the role of d
        let k_guess = &h;
        let d_guess = &k;
        if k_guess.is_zero() || d_guess.is_zero() {
            continue;
        }
        // phi must be an integer: (e*d - 1) divisible by k_guess
        let ed = e * d_guess;
        if ed <= one {
            continue;
        }
        let edm1 = &ed - &one;
        if !(&edm1 % k_guess).is_zero() {
            continue;
        }
        let phi = &edm1 / k_guess;
        if &phi >= n {
            continue;
        }
        // s = p + q = n - phi + 1
        let s = n - &phi + &one;
        let s2 = &s * &s;
        if s2 < four_n {
            continue;
        }
        let disc = &s2 - &four_n;
        if !is_perfect_square(&disc) {
            continue;
        }
        let t = disc.sqrt();
        // p = (s - t)/2, q = (s + t)/2
        if &s < &t {
            continue;
        }
        let p = (&s - &t) / &two;
        let q = (&s + &t) / &two;
        if &(&p * &q) == n && p > one {
            return Signal {
                detector: "wiener".to_string(),
                verdict: Verdict::FactorRecovered,
                failure_mode: FailureMode::SmallPrivateExponent,
                confidence: 1.0,
                detail: format!("small private exponent recovered via convergent; d is {}-bit", d_guess.bits()),
                cost_ms: start.elapsed().as_millis(),
                recovered_factor: Some(p),
            };
        }
    }

    Signal::clean("wiener", start.elapsed().as_millis())
}
