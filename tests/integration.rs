//! Integration tests: construct keys with known defects and assert the right detector
//! fires, that a healthy key is never labelled "secure", and that operator reports
//! never leak private factors.

use num_bigint::BigUint;
use num_traits::{Num, Zero};
use rsashield::detectors::{batch_gcd, ecm, DetectorConfig, FailureMode, Verdict};
use rsashield::key::RsaPublicKey;
use rsashield::{audit_single, report, scoring};

fn big(dec: &str) -> BigUint {
    BigUint::from_str_radix(dec, 10).unwrap()
}

fn key_from_primes(p: &BigUint, q: &BigUint, e: u32) -> RsaPublicKey {
    RsaPublicKey::new(p * q, BigUint::from(e), "test")
}

#[test]
fn small_factor_is_recovered() {
    // n = 101 * large prime
    let small = big("101");
    let large = big("32416190071"); // prime
    let key = key_from_primes(&small, &large, 65537);
    let (a, signals) = audit_single(&key, &DetectorConfig::default());
    assert_eq!(a.classification, scoring::Classification::FactorRecovered);
    assert!(signals
        .iter()
        .any(|s| s.detector == "small_factor" && s.verdict == Verdict::FactorRecovered));
}

#[test]
fn close_primes_caught_by_fermat() {
    // two consecutive primes -> tiny gap
    let p = big("1000000007");
    let q = big("1000000009");
    let key = key_from_primes(&p, &q, 65537);
    let (a, signals) = audit_single(&key, &DetectorConfig::default());
    assert_eq!(a.classification, scoring::Classification::FactorRecovered);
    assert!(signals
        .iter()
        .any(|s| s.detector == "fermat" && s.failure_mode == FailureMode::ClosePrimes));
}

#[test]
fn healthy_key_is_never_secure() {
    // two genuine, independent, well-separated 256-bit primes, standard exponent
    let p = rsashield::mathutil::random_prime_seeded(256, 1);
    let q = rsashield::mathutil::random_prime_seeded(256, 2);
    assert_ne!(p, q);
    let key = key_from_primes(&p, &q, 65537);
    let (a, _signals) = audit_single(&key, &DetectorConfig::default());
    // the strongest clean result is NO_DEFECT_FOUND — never a "secure" label
    assert_ne!(a.classification.as_str(), "SECURE");
    assert_eq!(a.classification, scoring::Classification::NoDefectFound);
}

#[test]
fn batch_gcd_finds_shared_prime() {
    let shared = big("1000000007");
    let q1 = big("1000000009");
    let q2 = big("1000000021");
    let k1 = key_from_primes(&shared, &q1, 65537);
    let k2 = key_from_primes(&shared, &q2, 65537);
    let res = batch_gcd::run(&[k1, k2]);
    assert_eq!(res.keys_affected, 2);
    assert!(res.signals.iter().all(|(_, s)| s.failure_mode == FailureMode::SharedPrime));
}

#[test]
fn tree_batch_gcd_matches_pairwise() {
    // corpus: two keys share a prime, two are independent
    let shared = big("1000000007");
    let a = key_from_primes(&shared, &big("1000000009"), 65537);
    let b = key_from_primes(&shared, &big("1000000021"), 65537);
    let c = key_from_primes(&big("1000000033"), &big("1000000087"), 65537);
    let d = key_from_primes(&big("1000000093"), &big("1000000097"), 65537);
    let keys = vec![a, b, c, d];

    let mut pair: Vec<usize> = batch_gcd::run(&keys).signals.iter().map(|(i, _)| *i).collect();
    let mut tree: Vec<usize> = batch_gcd::run_tree(&keys).signals.iter().map(|(i, _)| *i).collect();
    pair.sort();
    tree.sort();
    assert_eq!(pair, tree, "tree batch GCD must flag the same keys as pairwise");
    assert_eq!(tree, vec![0, 1], "only the two shared-prime keys should be flagged");
}

#[test]
fn ecm_recovers_unbalanced_factor() {
    // ~18-bit factor (above a 100k trial-division bound) times a large prime
    let mid = rsashield::mathutil::random_prime_seeded(18, 7);
    let big_p = rsashield::mathutil::random_prime_seeded(100, 8);
    let n = &mid * &big_p;
    let key = RsaPublicKey::new(n.clone(), BigUint::from(65537u32), "ecm-test");
    let sig = ecm::run(&key, 5_000, 40);
    assert_eq!(sig.verdict, Verdict::FactorRecovered, "ECM should recover the ~18-bit factor");
    let f = sig.recovered_factor.expect("factor present");
    assert!((&n % &f).is_zero() && f > BigUint::from(1u32) && f < n);
}

#[test]
fn ecm_yield_study_detects_small_factor_at_budget() {
    use rsashield::benchmark::ecm_study::run_ecm_study;
    // an 18-bit factor at a generous budget should be detected on every key
    let study = run_ecm_study(256, 1, &[18], &[(5_000, 40)], 4);
    assert_eq!(study.rows.len(), 1);
    assert_eq!(study.rows[0].detection_rate, 1.0, "18-bit factor must be found at B1=5000/40 curves");
}

#[test]
fn operator_report_has_no_private_factor() {
    // a broken key whose factor IS recovered; the operator report must not contain it
    let small = big("101");
    let large = big("32416190071");
    let key = key_from_primes(&small, &large, 65537);
    let (a, signals) = audit_single(&key, &DetectorConfig::default());
    let r = report::operator_report(&key, &a, &signals);
    let json = report::to_json(&r);
    assert!(!json.contains("32416190071"), "operator report leaked the cofactor");
    assert!(!json.contains("\"101\""), "operator report leaked the small factor");
    // but the forensic report is allowed to contain it
    let fr = report::forensic_report(&key, &a, &signals);
    let fjson = serde_json::to_string(&fr).unwrap();
    assert!(fjson.contains("65") || !fr.recovered_factors_hex.is_empty());
}
