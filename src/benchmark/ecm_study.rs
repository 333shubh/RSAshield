//! ECM marginal-yield study (research question RQ2).
//!
//! ECM is a *bounded* detector: how much it finds depends on the budget `(B1, curves)`
//! and on how small the target factor is. This study sweeps factor sizes against budgets
//! on synthetic unbalanced keys and reports detection rate and average cost, so the
//! "what budget is worth spending" question can be answered with numbers rather than
//! asserted. All keys are seeded, so the table is reproducible.

use crate::detectors::{ecm, Verdict};
use crate::key::RsaPublicKey;
use crate::mathutil::random_prime;
use num_bigint::BigUint;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use serde::Serialize;
use crate::clock::Instant;

#[derive(Serialize)]
pub struct YieldRow {
    pub factor_bits: u64,
    pub b1: u64,
    pub curves: u32,
    pub keys: usize,
    pub detected: usize,
    pub detection_rate: f64,
    pub avg_ms: f64,
}

pub struct EcmStudy {
    pub modulus_bits: u64,
    pub rows: Vec<YieldRow>,
}

/// Generate `count` unbalanced keys whose small factor is exactly `factor_bits` bits.
fn gen_unbalanced(modulus_bits: u64, factor_bits: u64, count: usize, seed: u64) -> Vec<RsaPublicKey> {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let mut keys = Vec::with_capacity(count);
    for i in 0..count {
        let small = random_prime(factor_bits, &mut rng);
        let big = random_prime(modulus_bits - factor_bits, &mut rng);
        let n = &small * &big;
        keys.push(RsaPublicKey::new(
            n,
            BigUint::from(65537u32),
            format!("unbal-{factor_bits}b-{i}"),
        ));
    }
    keys
}

pub fn run_ecm_study(
    modulus_bits: u64,
    seed: u64,
    factor_sizes: &[u64],
    budgets: &[(u64, u32)],
    keys_per: usize,
) -> EcmStudy {
    let mut rows = Vec::new();
    for &fb in factor_sizes {
        // same key set across budgets for this factor size, for a fair comparison
        let keys = gen_unbalanced(modulus_bits, fb, keys_per, seed ^ fb);
        for &(b1, curves) in budgets {
            let start = Instant::now();
            let mut detected = 0usize;
            for k in &keys {
                if ecm::run(k, b1, curves).verdict == Verdict::FactorRecovered {
                    detected += 1;
                }
            }
            let total_ms = start.elapsed().as_secs_f64() * 1000.0;
            rows.push(YieldRow {
                factor_bits: fb,
                b1,
                curves,
                keys: keys.len(),
                detected,
                detection_rate: detected as f64 / keys.len() as f64,
                avg_ms: total_ms / keys.len() as f64,
            });
        }
    }
    EcmStudy { modulus_bits, rows }
}

pub fn format_study(s: &EcmStudy) -> String {
    let mut out = String::new();
    out.push_str("ECM marginal-yield study (RQ2)\n");
    out.push_str("==============================\n");
    out.push_str(&format!("modulus size: {} bits; factor = the small/unbalanced prime\n\n", s.modulus_bits));
    out.push_str(&format!(
        "{:>11} {:>7} {:>7} {:>10} {:>8} {:>10}\n",
        "factor_bits", "B1", "curves", "detected", "rate", "avg_ms"
    ));
    for r in &s.rows {
        out.push_str(&format!(
            "{:>11} {:>7} {:>7} {:>8}/{:<1} {:>7.2} {:>10.1}\n",
            r.factor_bits,
            r.b1,
            r.curves,
            r.detected,
            r.keys,
            r.detection_rate,
            r.avg_ms
        ));
    }
    out.push_str(
        "\nReading: detection rises with B1/curves and falls as the factor grows; cost rises\n\
         with B1. A factor near balanced size stays out of reach at any practical budget —\n\
         that is where batch GCD / Fermat, not ECM, do the work.\n",
    );
    out
}
