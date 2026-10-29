//! Maps detected key-generation defects to indicative security-standard references for
//! power-system / ICS environments, to help an operator route a finding into their
//! existing remediation and assurance processes.
//!
//! These are **indicative triage references, not compliance determinations.** RSAShield
//! analyses public-key material only and cannot assess an organisation's compliance.

use crate::detectors::FailureMode;
use serde::Serialize;
use std::collections::HashSet;

#[derive(Clone, Serialize)]
pub struct StandardRef {
    pub framework: String,
    pub reference: String,
    pub relevance: String,
}

fn sr(framework: &str, reference: &str, relevance: &str) -> StandardRef {
    StandardRef {
        framework: framework.to_string(),
        reference: reference.to_string(),
        relevance: relevance.to_string(),
    }
}

/// Deduplicated standard references for a key's set of failure modes.
pub fn references_for(modes: &[FailureMode]) -> Vec<StandardRef> {
    let real: Vec<FailureMode> = modes.iter().copied().filter(|m| *m != FailureMode::None).collect();
    if real.is_empty() {
        return Vec::new();
    }

    let mut out: Vec<StandardRef> = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut add = |r: StandardRef| {
        if seen.insert((r.framework.clone(), r.reference.clone())) {
            out.push(r);
        }
    };

    // Any genuine defect: key-lifecycle management and the vulnerability-assessment process.
    let has_defect = real.iter().any(|m| *m != FailureMode::WeakMetadata);
    if has_defect {
        add(sr(
            "IEC 62351-9",
            "Key management for power system equipment",
            "Revoke and replace the affected key; review the key-generation process that produced it.",
        ));
        add(sr(
            "NERC CIP-010",
            "Configuration change management & vulnerability assessments (R3)",
            "Record the weak key as a vulnerability finding and track it to remediation.",
        ));
    }

    for m in &real {
        match m {
            FailureMode::SharedPrime => add(sr(
                "IEC 62351-9",
                "Random number generation at key generation",
                "A shared prime indicates insufficient entropy; audit the whole device batch/firmware, not just this unit.",
            )),
            FailureMode::ClosePrimes => add(sr(
                "IEC 62351-9",
                "Prime generation quality",
                "Primes drawn too close together; verify the key-generation library on the device.",
            )),
            FailureMode::SmallOrUnbalancedFactor => add(sr(
                "IEC 62351-9",
                "Key-generation integrity",
                "Modulus has a small/unbalanced factor; the key-generation implementation is defective.",
            )),
            FailureMode::SmallPrivateExponent => add(sr(
                "IEC 62351-9",
                "Key parameter selection",
                "A small private exponent was used; require a standard public exponent and a full-size private key.",
            )),
            FailureMode::StructuredPrime => {
                add(sr(
                    "Vendor advisory",
                    "Affected key-generation library/firmware",
                    "Structured primes point to a known library flaw; apply the vendor fix and reissue keys.",
                ));
            }
            FailureMode::WeakMetadata => {
                add(sr(
                    "NIST SP 800-57 Part 1",
                    "Key-size and algorithm recommendations",
                    "Modulus size or public exponent is below current policy; plan migration to compliant parameters.",
                ));
                add(sr(
                    "IEC 62351-3",
                    "TLS profile for power-system TCP/IP",
                    "Ensure keys used for substation TLS meet the profile's algorithm and key-length requirements.",
                ));
            }
            FailureMode::None => {}
        }
    }

    out
}

/// Standard disclaimer that accompanies any standards references in a report.
pub const DISCLAIMER: &str =
    "Indicative references for remediation triage only; not a compliance determination. \
     RSAShield analyses public-key material and cannot assess organisational compliance.";
