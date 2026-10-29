//! Reproducible benchmark: a synthetic corpus of RSA public keys with injected,
//! labelled key-generation defects and ground-truth factors, plus an evaluator that
//! measures each detector's precision/recall against those labels.
//!
//! Ground truth (private factors and defect labels) lives only here, in the harness.

pub mod evaluate;
pub mod generate;

use serde::Serialize;

/// The defect deliberately injected into a generated key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DefectClass {
    Healthy,
    SharedPrime,
    ClosePrime,
    SmallFactor,
    SmallD,
}

impl DefectClass {
    pub fn is_defective(self) -> bool {
        self != DefectClass::Healthy
    }
    /// Detector expected to catch this class in the MVP.
    pub fn expected_detector(self) -> &'static str {
        match self {
            DefectClass::Healthy => "(none)",
            DefectClass::SharedPrime => "batch_gcd",
            DefectClass::ClosePrime => "fermat",
            DefectClass::SmallFactor => "small_factor",
            DefectClass::SmallD => "wiener",
        }
    }
}

/// One ground-truth record. Private factors are kept here for scoring only.
#[derive(Clone, Debug, Serialize)]
pub struct GroundTruth {
    pub index: usize,
    pub class: DefectClass,
    pub bits: u64,
    pub n_hex: String,
    pub e_hex: String,
    pub p_hex: String,
    pub q_hex: String,
}
