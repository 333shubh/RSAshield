//! RSAShield — an offline, multi-signal RSA public-key quality auditor.
//!
//! The tool combines complementary detectors (metadata/structural checks, trial
//! division, Fermat close-prime factoring, Wiener small-d recovery, and corpus-level
//! batch GCD for shared primes) and fuses their signals into a risk assessment.
//!
//! Design principles:
//! - It analyses *public* keys only and runs fully offline.
//! - A clean result is never a proof of security — only "no defect found under budget".
//! - Recovered private factors never appear in operator reports (see [`report`]).
//! - ECM is a planned *bounded* detector for small/unbalanced factors, not a universal
//!   weak-key oracle; entropy failures are caught mainly by batch GCD and Fermat.

pub mod api;
pub mod benchmark;
pub mod clock;
pub mod detectors;
pub mod key;
pub mod mathutil;
pub mod report;
pub mod scoring;
#[cfg(not(target_arch = "wasm32"))]
pub mod server;
pub mod standards;
#[cfg(target_arch = "wasm32")]
pub mod wasm;

/// Convenience: run the per-key detector pipeline and fuse into an assessment.
pub fn audit_single(
    key: &key::RsaPublicKey,
    cfg: &detectors::DetectorConfig,
) -> (scoring::Assessment, Vec<detectors::Signal>) {
    let signals = detectors::run_single_key(key, cfg);
    let assessment = scoring::assess(&signals);
    (assessment, signals)
}
