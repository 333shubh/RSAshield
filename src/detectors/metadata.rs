//! Structural / metadata checks on the public key: modulus size policy, public
//! exponent sanity, and byte-level statistics of the modulus. These are cheap,
//! high-recall *screens* — advisory indicators, not cryptographic proofs.

use super::{FailureMode, Signal, Verdict};
use crate::key::RsaPublicKey;
use num_bigint::BigUint;
use num_traits::One;
use crate::clock::Instant;

/// Minimum modulus size we treat as current policy (bits).
const MIN_POLICY_BITS: u64 = 2048;

pub fn run(key: &RsaPublicKey) -> Signal {
    let start = Instant::now();
    let mut notes: Vec<String> = Vec::new();
    let mut verdict = Verdict::Clean;
    let mut mode = FailureMode::None;
    let mut confidence: f64 = 0.0;

    // --- public exponent sanity -------------------------------------------------
    let one = BigUint::one();
    let three = BigUint::from(3u32);
    let common = BigUint::from(65537u32);
    if key.e <= one {
        notes.push(format!("public exponent e={} is invalid (<= 1)", key.e));
        verdict = Verdict::DefectSuspected;
        mode = FailureMode::WeakMetadata;
        confidence = confidence.max(0.9);
    } else if key.e.bit(0) == false {
        notes.push(format!("public exponent e={} is even (cannot be coprime to phi)", key.e));
        verdict = Verdict::DefectSuspected;
        mode = FailureMode::WeakMetadata;
        confidence = confidence.max(0.9);
    } else if key.e != common && key.e != three {
        notes.push(format!("non-standard public exponent e={}", key.e));
        if verdict == Verdict::Clean {
            verdict = Verdict::Advisory;
            mode = FailureMode::WeakMetadata;
            confidence = confidence.max(0.2);
        }
    }

    // --- modulus size policy ----------------------------------------------------
    let bits = key.modulus_bits();
    if bits < MIN_POLICY_BITS {
        notes.push(format!("modulus is {bits}-bit, below the {MIN_POLICY_BITS}-bit policy floor"));
        if verdict == Verdict::Clean {
            verdict = Verdict::Advisory;
            mode = FailureMode::WeakMetadata;
        }
        confidence = confidence.max(0.35);
    }

    // --- modulus byte statistics ------------------------------------------------
    let bytes = key.n.to_bytes_be();
    let (entropy, ones_ratio) = byte_stats(&bytes);
    // A real semiprime modulus looks essentially random: ~7.9 bits/byte, ones ~0.5.
    if entropy < 7.0 {
        notes.push(format!("low modulus byte entropy ({entropy:.2} bits/byte)"));
        if verdict == Verdict::Clean {
            verdict = Verdict::Advisory;
            mode = FailureMode::WeakMetadata;
        }
        confidence = confidence.max(0.4);
    }
    if !(0.40..=0.60).contains(&ones_ratio) {
        notes.push(format!("skewed bit balance (ones ratio {ones_ratio:.3})"));
        if verdict == Verdict::Clean {
            verdict = Verdict::Advisory;
            mode = FailureMode::WeakMetadata;
        }
        confidence = confidence.max(0.3);
    }

    let detail = if notes.is_empty() {
        format!("{bits}-bit modulus, e={}, byte entropy {entropy:.2}, ones {ones_ratio:.3}", key.e)
    } else {
        notes.join("; ")
    };

    Signal {
        detector: "metadata".to_string(),
        verdict,
        failure_mode: mode,
        confidence,
        detail,
        cost_ms: start.elapsed().as_millis(),
        recovered_factor: None,
    }
}

/// Shannon entropy (bits/byte) and the ratio of set bits over the byte slice.
fn byte_stats(bytes: &[u8]) -> (f64, f64) {
    if bytes.is_empty() {
        return (0.0, 0.0);
    }
    let mut counts = [0u64; 256];
    let mut ones = 0u64;
    for &b in bytes {
        counts[b as usize] += 1;
        ones += b.count_ones() as u64;
    }
    let total = bytes.len() as f64;
    let mut entropy = 0.0f64;
    for &c in counts.iter() {
        if c > 0 {
            let p = c as f64 / total;
            entropy -= p * p.log2();
        }
    }
    let ones_ratio = ones as f64 / (total * 8.0);
    (entropy, ones_ratio)
}
