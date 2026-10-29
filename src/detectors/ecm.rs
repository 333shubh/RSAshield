//! Bounded Lenstra Elliptic Curve Method (ECM), Montgomery form.
//!
//! ECM's cost depends on the size of the *smallest* factor, not on `n`, so it is the
//! right tool for **small/unbalanced** factors that trial division misses — and the
//! wrong tool for a balanced weak-entropy modulus, which it cannot touch. It is run here
//! under a strict budget `(B1, curves)`; exhausting the budget proves nothing.
//!
//! Montgomery curves `B·y^2 = x^3 + A·x^2 + x` are used with projective XZ coordinates and
//! the Montgomery ladder, so each curve needs only **one** gcd (on the final Z) rather
//! than a modular inverse per step — the standard reason ECM uses this form. A factor is
//! revealed when that gcd (or a Suyama setup inverse) is a proper divisor of `n`. Curves
//! are seeded from `n`, so results are reproducible.

use super::{FailureMode, Signal, Verdict};
use crate::key::RsaPublicKey;
use crate::mathutil::{gcd, mod_inverse};
use num_bigint::{BigUint, RandBigInt};
use num_traits::{One, Zero};
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use std::time::Instant;

#[inline]
fn mulm(a: &BigUint, b: &BigUint, n: &BigUint) -> BigUint {
    (a * b) % n
}
#[inline]
fn addm(a: &BigUint, b: &BigUint, n: &BigUint) -> BigUint {
    (a + b) % n
}
#[inline]
fn subm(a: &BigUint, b: &BigUint, n: &BigUint) -> BigUint {
    let a = a % n;
    let b = b % n;
    (a + n - b) % n
}
#[inline]
fn sqrm(a: &BigUint, n: &BigUint) -> BigUint {
    mulm(a, a, n)
}

type Pt = (BigUint, BigUint);

/// Montgomery point doubling in XZ coordinates, using a24 = (A+2)/4.
fn x_dbl(p: &Pt, a24: &BigUint, n: &BigUint) -> Pt {
    let s1 = sqrm(&addm(&p.0, &p.1, n), n); // (X+Z)^2
    let s2 = sqrm(&subm(&p.0, &p.1, n), n); // (X-Z)^2
    let x = mulm(&s1, &s2, n);
    let d = subm(&s1, &s2, n); // 4XZ
    let z = mulm(&d, &addm(&s2, &mulm(a24, &d, n), n), n);
    (x, z)
}

/// Differential addition: returns A+B given the difference point `diff` = A-B.
fn x_add(a: &Pt, b: &Pt, diff: &Pt, n: &BigUint) -> Pt {
    let t1 = mulm(&subm(&a.0, &a.1, n), &addm(&b.0, &b.1, n), n); // (Xa-Za)(Xb+Zb)
    let t2 = mulm(&addm(&a.0, &a.1, n), &subm(&b.0, &b.1, n), n); // (Xa+Za)(Xb-Zb)
    let x = mulm(&diff.1, &sqrm(&addm(&t1, &t2, n), n), n); // Zd*(t1+t2)^2
    let z = mulm(&diff.0, &sqrm(&subm(&t1, &t2, n), n), n); // Xd*(t1-t2)^2
    (x, z)
}

/// Montgomery ladder: returns [k]·P.
fn ladder(k: &BigUint, p: &Pt, a24: &BigUint, n: &BigUint) -> Pt {
    let bits = k.bits();
    if bits == 0 {
        return (BigUint::one(), BigUint::zero()); // [0]P = identity
    }
    let mut r0 = p.clone(); // 1P
    let mut r1 = x_dbl(p, a24, n); // 2P
    for i in (0..(bits - 1)).rev() {
        if k.bit(i) {
            r0 = x_add(&r0, &r1, p, n);
            r1 = x_dbl(&r1, a24, n);
        } else {
            r1 = x_add(&r0, &r1, p, n);
            r0 = x_dbl(&r0, a24, n);
        }
    }
    r0
}

fn primes_up_to(bound: u64) -> Vec<u64> {
    if bound < 2 {
        return vec![];
    }
    let n = bound as usize;
    let mut sieve = vec![true; n + 1];
    sieve[0] = false;
    sieve[1] = false;
    let mut i = 2usize;
    while i * i <= n {
        if sieve[i] {
            let mut j = i * i;
            while j <= n {
                sieve[j] = false;
                j += i;
            }
        }
        i += 1;
    }
    (2..=n as u64).filter(|&x| sieve[x as usize]).collect()
}

/// Stage-1 scalar k = lcm(1..=B1) = product of prime powers p^e <= B1.
fn stage1_scalar(b1: u64) -> BigUint {
    let mut k = BigUint::one();
    for p in primes_up_to(b1) {
        let mut pe = p;
        while let Some(next) = pe.checked_mul(p) {
            if next > b1 {
                break;
            }
            pe = next;
        }
        k *= BigUint::from(pe);
    }
    k
}

/// Run bounded ECM. Deterministic given `n` (curve seed is derived from the modulus).
pub fn run(key: &RsaPublicKey, b1: u64, curves: u32) -> Signal {
    let start = Instant::now();
    let n = &key.n;

    // ECM is for medium/unbalanced factors; trial division owns the tiny ones, and the
    // curve arithmetic assumes n is odd and not trivially small.
    if curves == 0 || n.bits() < 80 || !n.bit(0) {
        return Signal::clean("ecm", start.elapsed().as_millis());
    }

    let k = stage1_scalar(b1);
    let seed = {
        let d = n.to_u64_digits();
        d.first().copied().unwrap_or(1) ^ (d.len() as u64)
    };
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let one = BigUint::one();
    let five = BigUint::from(5u32);
    let six = BigUint::from(6u32);

    let found = |f: BigUint, b1: u64, start: &Instant| -> Option<Signal> {
        if f > one && &f < n {
            let cof = n / &f;
            Some(Signal {
                detector: "ecm".to_string(),
                verdict: Verdict::FactorRecovered,
                failure_mode: FailureMode::SmallOrUnbalancedFactor,
                confidence: 1.0,
                detail: format!(
                    "bounded ECM (B1={b1}) recovered a {}-bit factor; cofactor is {}-bit",
                    f.bits(),
                    cof.bits()
                ),
                cost_ms: start.elapsed().as_millis(),
                recovered_factor: Some(f),
            })
        } else {
            None
        }
    };

    for _ in 0..curves {
        // Suyama parameterisation
        let sigma = rng.gen_biguint_range(&six, &(n - &one));
        let u = subm(&sqrm(&sigma, n), &five, n);
        let v = mulm(&BigUint::from(4u32), &sigma, n);
        let u3 = mulm(&sqrm(&u, n), &u, n);
        let vmu = subm(&v, &u, n);
        let vmu3 = mulm(&sqrm(&vmu, n), &vmu, n);
        let three_u_plus_v = addm(&mulm(&BigUint::from(3u32), &u, n), &v, n);
        let num = mulm(&vmu3, &three_u_plus_v, n);
        let den = mulm(&mulm(&BigUint::from(16u32), &u3, n), &v, n);

        // a24 = num/den; a non-invertible den hands us a factor directly
        let dr = &den % n;
        if dr.is_zero() {
            continue;
        }
        let g = gcd(&dr, n);
        let a24 = if g.is_one() {
            mulm(&num, &mod_inverse(&dr, n).expect("coprime => invertible"), n)
        } else {
            if let Some(sig) = found(g, b1, &start) {
                return sig;
            }
            continue;
        };

        let p: Pt = (u3.clone(), mulm(&sqrm(&v, n), &v, n)); // (u^3 : v^3)
        let res = ladder(&k, &p, &a24, n);
        let g = gcd(&res.1, n); // gcd(Z, n)
        if let Some(sig) = found(g, b1, &start) {
            return sig;
        }
    }

    Signal::clean("ecm", start.elapsed().as_millis())
}
