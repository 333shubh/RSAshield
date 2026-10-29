//! Evaluate the detector pipeline against a labelled synthetic corpus and report
//! precision, recall, per-class detection, false-positive rate, and runtime.

use super::generate::{generate, Corpus, CorpusSpec};
use super::DefectClass;
use crate::detectors::{self, batch_gcd, DetectorConfig, Signal};
use serde::Serialize;
use std::collections::BTreeMap;
use std::time::Instant;

#[derive(Serialize)]
pub struct Metrics {
    pub total_keys: usize,
    pub defective: usize,
    pub healthy: usize,
    pub true_positives: usize,
    pub false_positives: usize,
    pub false_negatives: usize,
    pub true_negatives: usize,
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
    pub false_positive_rate: f64,
    pub secure_labels_emitted: usize,
    /// Highest risk score assigned to any healthy key (0 is ideal separation).
    pub healthy_score_max: u32,
    /// Lowest risk score assigned to any defective key.
    pub defective_score_min: u32,
    pub per_class_recall: BTreeMap<String, (usize, usize)>, // class -> (detected, total)
    pub batch_gcd_pairs: usize,
    pub batch_gcd_keys_flagged: usize,
    pub wall_ms: u128,
}

fn ratio(num: usize, den: usize) -> f64 {
    if den == 0 {
        0.0
    } else {
        (num as f64 / den as f64 * 1000.0).round() / 1000.0
    }
}

pub fn run_benchmark(spec: &CorpusSpec, seed: u64) -> (Metrics, Corpus) {
    let wall = Instant::now();
    let corpus = generate(spec, seed);
    // close primes are found in ~1 step, so a modest Fermat budget keeps this fast;
    // ECM runs a small curve budget sufficient for the ~18-bit unbalanced factors.
    let cfg = DetectorConfig {
        trial_division_bound: 100_000,
        fermat_max_iters: 20_000,
        ecm_b1: 3_000,
        ecm_curves: 12,
    };

    // per-key detectors
    let mut per_key_signals: Vec<Vec<Signal>> = corpus
        .keys
        .iter()
        .map(|k| detectors::run_single_key(k, &cfg))
        .collect();

    // corpus-level batch GCD (Phase 3 tree algorithm)
    let bg = batch_gcd::run_tree(&corpus.keys);
    for (idx, sig) in &bg.signals {
        per_key_signals[*idx].push(sig.clone());
    }

    // score each key
    let mut tp = 0usize;
    let mut fp = 0usize;
    let mut fn_ = 0usize;
    let mut tn = 0usize;
    let mut secure_labels = 0usize;
    let mut healthy_score_max = 0u32;
    let mut defective_score_min = 100u32;
    let mut per_class: BTreeMap<String, (usize, usize)> = BTreeMap::new();

    for (i, signals) in per_key_signals.iter().enumerate() {
        let class = corpus.truth[i].class;
        let detected = signals.iter().any(|s| s.verdict.is_positive());
        let assessment = crate::scoring::assess(signals);
        // invariant: the scorer must never emit a "secure" classification
        if assessment.classification.as_str().eq_ignore_ascii_case("secure") {
            secure_labels += 1;
        }
        if class.is_defective() {
            defective_score_min = defective_score_min.min(assessment.risk_score);
        } else {
            healthy_score_max = healthy_score_max.max(assessment.risk_score);
        }

        if class.is_defective() {
            let entry = per_class.entry(format!("{class:?}")).or_insert((0, 0));
            entry.1 += 1;
            if detected {
                entry.0 += 1;
                tp += 1;
            } else {
                fn_ += 1;
            }
        } else if detected {
            fp += 1;
        } else {
            tn += 1;
        }
    }

    let precision = ratio(tp, tp + fp);
    let recall = ratio(tp, tp + fn_);
    let f1 = if precision + recall > 0.0 {
        ((2.0 * precision * recall / (precision + recall)) * 1000.0).round() / 1000.0
    } else {
        0.0
    };
    let fpr = ratio(fp, fp + tn);

    let metrics = Metrics {
        total_keys: corpus.keys.len(),
        defective: tp + fn_,
        healthy: fp + tn,
        true_positives: tp,
        false_positives: fp,
        false_negatives: fn_,
        true_negatives: tn,
        precision,
        recall,
        f1,
        false_positive_rate: fpr,
        secure_labels_emitted: secure_labels,
        healthy_score_max,
        defective_score_min,
        per_class_recall: per_class,
        batch_gcd_pairs: bg.pairs_compared,
        batch_gcd_keys_flagged: bg.keys_affected,
        wall_ms: wall.elapsed().as_millis(),
    };
    (metrics, corpus)
}

/// Render a human-readable summary of the metrics.
pub fn format_metrics(m: &Metrics) -> String {
    let mut s = String::new();
    s.push_str("RSAShield benchmark results\n");
    s.push_str("===========================\n");
    s.push_str(&format!("keys: {}  (defective {}, healthy {})\n", m.total_keys, m.defective, m.healthy));
    s.push_str(&format!(
        "confusion: TP={} FP={} FN={} TN={}\n",
        m.true_positives, m.false_positives, m.false_negatives, m.true_negatives
    ));
    s.push_str(&format!(
        "precision={:.3}  recall={:.3}  F1={:.3}  FPR={:.3}\n",
        m.precision, m.recall, m.f1, m.false_positive_rate
    ));
    s.push_str(&format!("'secure' labels emitted: {} (must be 0 by design)\n", m.secure_labels_emitted));
    s.push_str(&format!(
        "risk-score separation: healthy <= {}, defective >= {} (margin {})\n",
        m.healthy_score_max,
        m.defective_score_min,
        m.defective_score_min as i64 - m.healthy_score_max as i64
    ));
    s.push_str(&format!(
        "batch GCD: {} pairs compared, {} keys flagged with shared primes\n",
        m.batch_gcd_pairs, m.batch_gcd_keys_flagged
    ));
    s.push_str("per-class recall (expected detector):\n");
    for (class, (det, total)) in &m.per_class_recall {
        let expected = match class.as_str() {
            "SharedPrime" => DefectClass::SharedPrime,
            "ClosePrime" => DefectClass::ClosePrime,
            "SmallFactor" => DefectClass::SmallFactor,
            "Unbalanced" => DefectClass::Unbalanced,
            "SmallD" => DefectClass::SmallD,
            _ => DefectClass::Healthy,
        };
        s.push_str(&format!(
            "  {:<12} {}/{}  -> {}\n",
            class,
            det,
            total,
            expected.expected_detector()
        ));
    }
    s.push_str(&format!("wall time: {} ms\n", m.wall_ms));
    s
}
