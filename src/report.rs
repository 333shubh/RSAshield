//! Report generation with structural redaction.
//!
//! The operator report is what leaves the tool in normal use: it names the finding,
//! the failure mode, and remediation guidance, but has NO field capable of carrying a
//! private factor. The forensic report is a separate type used only inside the test
//! harness / authorised forensic review, and it may include recovered factors in hex.

use crate::detectors::Signal;
use crate::key::RsaPublicKey;
use crate::scoring::Assessment;
use serde::Serialize;

#[derive(Serialize)]
pub struct SignalView {
    detector: String,
    verdict: String,
    failure_mode: String,
    confidence: f64,
    detail: String,
    cost_ms: u128,
}

impl SignalView {
    fn from(s: &Signal) -> Self {
        SignalView {
            detector: s.detector.clone(),
            verdict: format!("{:?}", s.verdict),
            failure_mode: format!("{:?}", s.failure_mode),
            confidence: (s.confidence * 1000.0).round() / 1000.0,
            detail: s.detail.clone(),
            cost_ms: s.cost_ms,
        }
    }
}

/// Operator-facing report. Deliberately contains no recovered factors.
#[derive(Serialize)]
pub struct OperatorReport {
    pub tool: String,
    pub key: String,
    pub modulus_bits: u64,
    pub classification: String,
    pub risk_score: u32,
    pub failure_modes: Vec<String>,
    pub caveat: String,
    pub remediation: Vec<String>,
    pub standards: Vec<crate::standards::StandardRef>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub standards_note: String,
    pub signals: Vec<SignalView>,
}

pub fn operator_report(key: &RsaPublicKey, assessment: &Assessment, signals: &[Signal]) -> OperatorReport {
    let standards = crate::standards::references_for(&assessment.failure_modes);
    let standards_note = if standards.is_empty() {
        String::new()
    } else {
        crate::standards::DISCLAIMER.to_string()
    };
    OperatorReport {
        tool: format!("rsashield {}", env!("CARGO_PKG_VERSION")),
        key: key.to_string(),
        modulus_bits: key.modulus_bits(),
        classification: assessment.classification.as_str().to_string(),
        risk_score: assessment.risk_score,
        failure_modes: assessment.failure_modes.iter().map(|m| format!("{m:?}")).collect(),
        caveat: assessment.caveat.clone(),
        remediation: remediation_for(assessment),
        standards,
        standards_note,
        signals: signals.iter().map(SignalView::from).collect(),
    }
}

fn remediation_for(a: &Assessment) -> Vec<String> {
    use crate::scoring::Classification::*;
    match a.classification {
        FactorRecovered => vec![
            "Treat this key as fully compromised. Revoke and replace it.".to_string(),
            "Regenerate on a device with a properly seeded CSPRNG; verify entropy at provisioning.".to_string(),
            "Audit sibling devices from the same batch/firmware — the generation defect is likely shared.".to_string(),
        ],
        DefectSuspected => vec![
            "Quarantine for review; prioritise replacement before the next maintenance window.".to_string(),
            "Confirm the key-generation path and firmware version against the vendor advisory.".to_string(),
        ],
        NoDefectFound => vec![
            "No action forced by this audit, but do not infer security from a clean result.".to_string(),
            "Re-audit with a wider budget (ECM) and against a larger corpus when feasible.".to_string(),
        ],
    }
}

/// Serialise an operator report as pretty JSON.
pub fn to_json(report: &OperatorReport) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

// ---- forensic (harness-only) ------------------------------------------------

/// Forensic report used only in the test harness / authorised forensic review.
/// Unlike the operator report, this MAY include recovered factors.
#[derive(Serialize)]
pub struct ForensicReport {
    pub key: String,
    pub classification: String,
    pub recovered_factors_hex: Vec<String>,
}

pub fn forensic_report(key: &RsaPublicKey, assessment: &Assessment, signals: &[Signal]) -> ForensicReport {
    let factors = signals
        .iter()
        .filter_map(|s| s.recovered_factor.as_ref().map(|f| f.to_str_radix(16)))
        .collect();
    ForensicReport {
        key: key.to_string(),
        classification: assessment.classification.as_str().to_string(),
        recovered_factors_hex: factors,
    }
}
