//! Detector framework: each detector inspects a key (or the corpus) and emits a
//! typed [`Signal`]. The scorer and reports consume signals only — recovered private
//! factors are carried in a field that is never serialised into operator output.

pub mod batch_gcd;
pub mod fermat;
pub mod metadata;
pub mod small_factor;
pub mod wiener;

use crate::key::RsaPublicKey;
use num_bigint::BigUint;
use serde::Serialize;

/// Which key-generation defect a signal points to. ECM/structured-prime modes are
/// defined here even though the MVP does not yet ship those detectors (Phases 3–4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureMode {
    SharedPrime,
    ClosePrimes,
    SmallOrUnbalancedFactor,
    SmallPrivateExponent,
    StructuredPrime,
    WeakMetadata,
    None,
}

/// Strength of a detector's finding. Note there is deliberately no "secure" verdict:
/// the strongest *negative* result is [`Verdict::Clean`], meaning "this detector found
/// nothing under its budget", never a proof of security.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The modulus was actually split; the key is broken.
    FactorRecovered,
    /// A strong structural anomaly without a recovered factor.
    DefectSuspected,
    /// A weak, low-confidence indicator (policy/size/entropy).
    Advisory,
    /// Nothing found by this detector under its budget.
    Clean,
}

impl Verdict {
    /// Whether this verdict counts as a positive detection for scoring/metrics.
    pub fn is_positive(self) -> bool {
        matches!(self, Verdict::FactorRecovered | Verdict::DefectSuspected)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Signal {
    pub detector: String,
    pub verdict: Verdict,
    pub failure_mode: FailureMode,
    pub confidence: f64,
    /// Human-readable explanation. Must never contain private factors.
    pub detail: String,
    pub cost_ms: u128,
    /// A recovered prime factor, held for the forensic report and the benchmark
    /// ground-truth check. Never serialised in the operator report.
    #[serde(skip_serializing)]
    pub recovered_factor: Option<BigUint>,
}

impl Signal {
    pub fn clean(detector: &str, cost_ms: u128) -> Self {
        Signal {
            detector: detector.to_string(),
            verdict: Verdict::Clean,
            failure_mode: FailureMode::None,
            confidence: 0.0,
            detail: "no finding under configured budget".to_string(),
            cost_ms,
            recovered_factor: None,
        }
    }
}

/// Budget knobs for the bounded detectors.
#[derive(Clone, Copy, Debug)]
pub struct DetectorConfig {
    pub trial_division_bound: u64,
    pub fermat_max_iters: u64,
}

impl Default for DetectorConfig {
    fn default() -> Self {
        DetectorConfig { trial_division_bound: 100_000, fermat_max_iters: 100_000 }
    }
}

/// Run all per-key detectors (not the corpus-level batch GCD) and return their signals.
pub fn run_single_key(key: &RsaPublicKey, cfg: &DetectorConfig) -> Vec<Signal> {
    vec![
        metadata::run(key),
        small_factor::run(key, cfg.trial_division_bound),
        fermat::run(key, cfg.fermat_max_iters),
        wiener::run(key),
    ]
}
