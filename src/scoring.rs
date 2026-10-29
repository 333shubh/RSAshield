//! Fuse a key's signals into a single assessment.
//!
//! Classification is one of three states — and crucially there is no "SECURE" state.
//! The best outcome is `NoDefectFound`, which means only that no detector fired under
//! its configured budget, not that the key is sound.

use crate::detectors::{FailureMode, Signal, Verdict};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Classification {
    FactorRecovered,
    DefectSuspected,
    NoDefectFound,
}

impl Classification {
    pub fn as_str(self) -> &'static str {
        match self {
            Classification::FactorRecovered => "FACTOR_RECOVERED",
            Classification::DefectSuspected => "DEFECT_SUSPECTED",
            Classification::NoDefectFound => "NO_DEFECT_FOUND",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Assessment {
    pub classification: Classification,
    /// 0..100; 100 only when a factor was actually recovered.
    pub risk_score: u32,
    pub failure_modes: Vec<FailureMode>,
    /// Advisory note making the budget-bounded nature of a clean result explicit.
    pub caveat: String,
}

/// Combine signals (per-key detectors plus any corpus-level signals) into an assessment.
pub fn assess(signals: &[Signal]) -> Assessment {
    let mut classification = Classification::NoDefectFound;
    let mut score: f64 = 0.0;
    let mut modes: Vec<FailureMode> = Vec::new();

    for s in signals {
        match s.verdict {
            Verdict::FactorRecovered => {
                classification = Classification::FactorRecovered;
                score = 100.0;
                push_mode(&mut modes, s.failure_mode);
            }
            Verdict::DefectSuspected => {
                if classification != Classification::FactorRecovered {
                    classification = Classification::DefectSuspected;
                }
                score = score.max(60.0 + 40.0 * s.confidence);
                push_mode(&mut modes, s.failure_mode);
            }
            Verdict::Advisory => {
                score = score.max(25.0 * s.confidence);
                push_mode(&mut modes, s.failure_mode);
            }
            Verdict::Clean => {}
        }
    }

    let caveat = match classification {
        Classification::NoDefectFound => {
            "No defect found under the configured detector budgets. This is NOT a proof \
             of security: untested factoring paths (e.g. bounded ECM) and corpus-external \
             shared primes may still apply."
                .to_string()
        }
        _ => "Finding is budget-independent: a factor or strong structural anomaly was observed."
            .to_string(),
    };

    Assessment {
        classification,
        risk_score: score.round() as u32,
        failure_modes: modes,
        caveat,
    }
}

fn push_mode(modes: &mut Vec<FailureMode>, m: FailureMode) {
    if m != FailureMode::None && !modes.contains(&m) {
        modes.push(m);
    }
}
