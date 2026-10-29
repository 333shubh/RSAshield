//! RSAShield command-line interface.
//!
//! Subcommands:
//!   audit <file>            audit one public key (PEM/DER file, or `-` for stdin)
//!   audit --hex "n:e"       audit one key given as hex modulus:exponent
//!   audit-corpus <file>     audit many keys (one PEM block or hex pair per record),
//!                           including corpus-level batch GCD
//!   bench [--seed N] [--bits N] [--out DIR]
//!                           generate a labelled synthetic corpus and report metrics

use rsashield::benchmark::evaluate::{format_metrics, run_benchmark};
use rsashield::benchmark::generate::CorpusSpec;
use rsashield::detectors::{batch_gcd, DetectorConfig};
use rsashield::key::{self, RsaPublicKey};
use rsashield::report;
use rsashield::scoring;
use std::env;
use std::fs;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        print_usage();
        return ExitCode::from(2);
    }
    match args[0].as_str() {
        "audit" => cmd_audit(&args[1..]),
        "audit-corpus" => cmd_audit_corpus(&args[1..]),
        "bench" => cmd_bench(&args[1..]),
        "study-ecm" => cmd_study_ecm(&args[1..]),
        "-h" | "--help" | "help" => {
            print_usage();
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("unknown subcommand: {other}\n");
            print_usage();
            ExitCode::from(2)
        }
    }
}

fn print_usage() {
    eprintln!(
        "rsashield {} — offline multi-signal RSA key auditor\n\n\
         USAGE:\n\
         \trsashield audit <file|-> [--forensic]\n\
         \trsashield audit --hex \"<modulus_hex>:<exponent_hex>\" [--forensic]\n\
         \trsashield audit-corpus <file>\n\
         \trsashield bench [--seed N] [--bits N] [--out DIR]\n\
         \trsashield study-ecm [--seed N] [--bits N]\n",
        env!("CARGO_PKG_VERSION")
    );
}

fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn opt<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(|s| s.as_str())
}

fn cmd_audit(args: &[String]) -> ExitCode {
    let forensic = flag(args, "--forensic");
    let parsed: Result<RsaPublicKey, String> = if let Some(hex) = opt(args, "--hex") {
        key::parse_any(hex, "cli-hex").map_err(|e| e.to_string())
    } else if let Some(path) = args.iter().find(|a| !a.starts_with("--")) {
        if path == "-" {
            use std::io::Read;
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf).ok();
            key::parse_any(&buf, "stdin").map_err(|e| e.to_string())
        } else {
            match fs::read_to_string(path) {
                Ok(s) => key::parse_any(&s, path.clone()).map_err(|e| e.to_string()),
                Err(e) => Err(format!("cannot read {path}: {e}")),
            }
        }
    } else {
        Err("no key given (use a file, `-`, or --hex)".to_string())
    };

    let key = match parsed {
        Ok(k) => k,
        Err(msg) => {
            // mirror the spec's "bad input -> clear error" behaviour
            eprintln!("{{\"error\":\"{}\"}}", msg.replace('"', "'"));
            return ExitCode::from(1);
        }
    };

    let cfg = DetectorConfig::default();
    let (assessment, signals) = rsashield::audit_single(&key, &cfg);

    if forensic {
        let fr = report::forensic_report(&key, &assessment, &signals);
        println!("{}", serde_json::to_string_pretty(&fr).unwrap_or_default());
    } else {
        let r = report::operator_report(&key, &assessment, &signals);
        println!("{}", report::to_json(&r));
    }
    ExitCode::SUCCESS
}

fn cmd_audit_corpus(args: &[String]) -> ExitCode {
    let path = match args.iter().find(|a| !a.starts_with("--")) {
        Some(p) => p,
        None => {
            eprintln!("audit-corpus needs a file path");
            return ExitCode::from(2);
        }
    };
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            return ExitCode::from(1);
        }
    };

    let keys = parse_corpus(&text);
    if keys.is_empty() {
        eprintln!("no keys parsed from {path}");
        return ExitCode::from(1);
    }

    let cfg = DetectorConfig::default();
    let mut per_key: Vec<Vec<_>> = keys
        .iter()
        .map(|k| rsashield::detectors::run_single_key(k, &cfg))
        .collect();
    let bg = batch_gcd::run_tree(&keys);
    for (idx, sig) in &bg.signals {
        per_key[*idx].push(sig.clone());
    }

    println!("audited {} keys; tree batch GCD flagged {} keys with shared primes\n", keys.len(), bg.keys_affected);
    for (i, signals) in per_key.iter().enumerate() {
        let assessment = scoring::assess(signals);
        let r = report::operator_report(&keys[i], &assessment, signals);
        println!("{}", report::to_json(&r));
    }
    ExitCode::SUCCESS
}

fn parse_corpus(text: &str) -> Vec<RsaPublicKey> {
    let mut keys = Vec::new();
    // PEM blocks first
    let mut buf = String::new();
    let mut inside = false;
    for line in text.lines() {
        if line.contains("-----BEGIN") {
            inside = true;
            buf.clear();
            buf.push_str(line);
            buf.push('\n');
            continue;
        }
        if inside {
            buf.push_str(line);
            buf.push('\n');
            if line.contains("-----END") {
                inside = false;
                if let Ok(k) = key::parse_any(&buf, format!("corpus#{}", keys.len())) {
                    keys.push(k);
                }
            }
            continue;
        }
        // non-PEM line: try hex n:e
        let l = line.trim();
        if l.contains(':') || l.contains(',') {
            if let Ok(k) = key::parse_any(l, format!("corpus#{}", keys.len())) {
                keys.push(k);
            }
        }
    }
    keys
}

fn cmd_bench(args: &[String]) -> ExitCode {
    let seed: u64 = opt(args, "--seed").and_then(|s| s.parse().ok()).unwrap_or(1);
    let bits: u64 = opt(args, "--bits").and_then(|s| s.parse().ok()).unwrap_or(512);
    let spec = CorpusSpec { bits, ..Default::default() };

    eprintln!("generating corpus (bits={bits}, seed={seed}) and evaluating...");
    let (metrics, corpus) = run_benchmark(&spec, seed);
    println!("{}", format_metrics(&metrics));

    if let Some(dir) = opt(args, "--out") {
        if let Err(e) = fs::create_dir_all(dir) {
            eprintln!("cannot create {dir}: {e}");
            return ExitCode::from(1);
        }
        let manifest = serde_json::to_string_pretty(&corpus.truth).unwrap_or_default();
        let _ = fs::write(format!("{dir}/manifest.json"), manifest);
        let metrics_json = serde_json::to_string_pretty(&metrics).unwrap_or_default();
        let _ = fs::write(format!("{dir}/metrics.json"), metrics_json);
        eprintln!("wrote {dir}/manifest.json (ground truth) and {dir}/metrics.json");
    }
    ExitCode::SUCCESS
}

fn cmd_study_ecm(args: &[String]) -> ExitCode {
    use rsashield::benchmark::ecm_study::{format_study, run_ecm_study};
    let seed: u64 = opt(args, "--seed").and_then(|s| s.parse().ok()).unwrap_or(1);
    let bits: u64 = opt(args, "--bits").and_then(|s| s.parse().ok()).unwrap_or(512);
    let factor_sizes = [18u64, 22, 26, 30];
    let budgets = [(500u64, 10u32), (2_000, 10), (5_000, 16), (10_000, 16)];
    eprintln!("running ECM yield study (bits={bits}, seed={seed})...");
    let study = run_ecm_study(bits, seed, &factor_sizes, &budgets, 6);
    println!("{}", format_study(&study));
    ExitCode::SUCCESS
}
