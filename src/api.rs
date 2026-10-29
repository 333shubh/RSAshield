//! The audit API, independent of transport.
//!
//! [`handle`] routes one request to a JSON response with no I/O of its own, so the same
//! code answers the local HTTP server ([`crate::server`]) and the in-browser
//! WebAssembly build ([`crate::wasm`]), and is tested directly in `tests/server.rs`.
//!
//! Routes:
//!   GET    /health                        liveness probe, mode and detector budgets
//!   GET    /api/samples                   seeded demo keys, one per defect class
//!   POST   /api/audit[?budget=deep|standard]
//!                                         audit the key(s) in the body and store them
//!   GET    /api/keys                      summaries of stored keys
//!   DELETE /api/keys                      clear stored keys
//!   GET    /api/report/{id}               operator report for one stored key
//!   POST   /api/batch-gcd                 tree batch GCD across all stored keys
//!   POST   /api/bundle                    research bundle: benchmark + ECM yield study
//!
//! Every response is an operator report or a summary of one: no route can carry a
//! recovered private factor.

use crate::benchmark::ecm_study::run_ecm_study;
use crate::benchmark::evaluate::run_benchmark;
use crate::benchmark::generate::{generate, CorpusSpec};
use crate::benchmark::DefectClass;
use crate::clock::Instant;
use crate::detectors::{self, batch_gcd, DetectorConfig, Signal};
use crate::key::{self, RsaPublicKey};
use crate::mathutil::random_prime_seeded;
use crate::{report, scoring};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::{Mutex, OnceLock};

const MAX_STORED_KEYS: usize = 1000;

pub struct Response {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl Response {
    pub(crate) fn json(status: u16, value: &impl Serialize) -> Self {
        Response {
            status,
            content_type: "application/json",
            body: serde_json::to_vec_pretty(value).unwrap_or_else(|_| b"{}".to_vec()),
        }
    }

    pub(crate) fn error(status: u16, message: impl Into<String>) -> Self {
        Self::json(status, &json!({ "error": message.into() }))
    }

    /// The body as UTF-8 text (convenient for tests and logging).
    pub fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// How much detector work to spend per key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Budget {
    /// The CLI default: ECM B1=10,000 x 16 curves, Fermat 100k steps.
    Deep,
    /// About a tenth of the ECM work (B1=2,000 x 8 curves; still finds factors up to
    /// ~30 bits per the RQ2 study) and Fermat 20k steps; the browser default.
    Standard,
}

impl Budget {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "deep" => Some(Budget::Deep),
            "standard" => Some(Budget::Standard),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Budget::Deep => "deep",
            Budget::Standard => "standard",
        }
    }

    pub fn config(self) -> DetectorConfig {
        match self {
            Budget::Deep => DetectorConfig::default(),
            Budget::Standard => DetectorConfig {
                trial_division_bound: 100_000,
                fermat_max_iters: 20_000,
                ecm_b1: 2_000,
                ecm_curves: 8,
            },
        }
    }

    fn describe(self) -> Value {
        let c = self.config();
        json!({
            "name": self.name(),
            "trial_division_bound": c.trial_division_bound,
            "fermat_max_iters": c.fermat_max_iters,
            "ecm_b1": c.ecm_b1,
            "ecm_curves": c.ecm_curves,
        })
    }
}

/// A built-in demo key (or keys) for the dashboard.
#[derive(Clone, Serialize)]
pub struct Sample {
    pub id: &'static str,
    pub label: &'static str,
    pub expect: &'static str,
    pub description: &'static str,
    pub keys: Vec<String>,
}

struct StoredKey {
    id: usize,
    key: RsaPublicKey,
    signals: Vec<Signal>,
}

#[derive(Default)]
struct Store {
    next_id: usize,
    keys: Vec<StoredKey>,
}

/// Session state: the stored keys, the default budget and the lazily built samples.
pub struct AppState {
    mode: &'static str,
    budget: Budget,
    store: Mutex<Store>,
    samples: OnceLock<Vec<Sample>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    /// State for the local server: the full CLI detector budget.
    pub fn new() -> Self {
        Self::with(Budget::Deep, "offline")
    }

    /// State for the in-browser build: the lighter standard budget by default.
    pub fn browser() -> Self {
        Self::with(Budget::Standard, "browser")
    }

    fn with(budget: Budget, mode: &'static str) -> Self {
        AppState { mode, budget, store: Mutex::new(Store::default()), samples: OnceLock::new() }
    }

    fn samples(&self) -> &[Sample] {
        self.samples.get_or_init(build_samples)
    }
}

/* ------------------------------------------------------------------ router */

/// Route one API request. Pure apart from the state it is given.
pub fn handle(state: &AppState, method: &str, target: &str, body: &[u8]) -> Response {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    match (method, path) {
        ("GET", "/health") => health(state),
        ("GET", "/api/samples") => Response::json(200, &state.samples()),
        ("POST", "/api/audit") => audit(state, query, body),
        ("GET", "/api/keys") => list_keys(state),
        ("DELETE", "/api/keys") => clear_keys(state),
        ("POST", "/api/batch-gcd") => batch(state),
        ("POST", "/api/bundle") => bundle(body),
        ("GET", p) if p.starts_with("/api/report/") => report_for(state, &p["/api/report/".len()..]),
        (_, p) if p.starts_with("/api/") || p == "/health" => {
            if is_known_route(p) {
                Response::error(405, format!("{method} not allowed on {p}"))
            } else {
                Response::error(404, format!("no route for {p}"))
            }
        }
        (_, p) => Response::error(404, format!("no route for {method} {p}")),
    }
}

fn is_known_route(path: &str) -> bool {
    matches!(path, "/health" | "/api/samples" | "/api/audit" | "/api/keys" | "/api/batch-gcd" | "/api/bundle")
        || path.starts_with("/api/report/")
}

fn query_param<'a>(query: &'a str, name: &str) -> Option<&'a str> {
    query.split('&').find_map(|kv| kv.strip_prefix(name)?.strip_prefix('='))
}

fn health(state: &AppState) -> Response {
    let stored = state.store.lock().map(|s| s.keys.len()).unwrap_or(0);
    Response::json(
        200,
        &json!({
            "status": "ok",
            "tool": format!("rsashield {}", env!("CARGO_PKG_VERSION")),
            "mode": state.mode,
            "budget": state.budget.name(),
            "budgets": { "standard": Budget::Standard.describe(), "deep": Budget::Deep.describe() },
            "stored_keys": stored,
        }),
    )
}

/// Audit every key in the body (PEM, certificate, `ssh-rsa`, or hex `n:e`) and store them.
fn audit(state: &AppState, query: &str, body: &[u8]) -> Response {
    let text = match std::str::from_utf8(body) {
        Ok(t) if !t.trim().is_empty() => t,
        Ok(_) => return Response::error(400, "empty request: paste a PEM public key, certificate, ssh-rsa key or hex modulus:exponent pair"),
        Err(_) => return Response::error(400, "request body is not UTF-8 text"),
    };
    if key::looks_like_private_key(text) {
        // refuse before parsing anything; nothing is stored
        return Response::json(400, &json!({ "error": key::PRIVATE_KEY_REFUSAL, "private_key": true }));
    }
    let budget = match query_param(query, "budget") {
        None => state.budget,
        Some(b) => match Budget::parse(b) {
            Some(b) => b,
            None => return Response::error(400, "budget must be `standard` or `deep`"),
        },
    };

    let mut records = key::parse_many(text, "input");
    if records.is_empty() {
        // nothing record-shaped: report the precise reason for the whole input
        records.push(key::parse_any(text, "input"));
    }
    let mut parsed = Vec::new();
    let mut errors = Vec::new();
    for (i, r) in records.into_iter().enumerate() {
        match r {
            Ok(k) => parsed.push(k),
            Err(e) => errors.push(json!({ "record": i, "error": e.to_string() })),
        }
    }
    if parsed.is_empty() {
        let message = errors
            .first()
            .and_then(|e| e["error"].as_str())
            .unwrap_or("no key found")
            .to_string();
        return Response::json(400, &json!({ "error": message, "errors": errors }));
    }
    {
        let store = state.store.lock().expect("store lock");
        if store.keys.len() + parsed.len() > MAX_STORED_KEYS {
            return Response::error(409, format!("store limit of {MAX_STORED_KEYS} keys reached; clear stored keys first"));
        }
    }

    // run the detectors outside the lock; they can take a moment per key
    let cfg = budget.config();
    let audited: Vec<(RsaPublicKey, Vec<Signal>)> = parsed
        .into_iter()
        .map(|k| {
            let signals = detectors::run_single_key(&k, &cfg);
            (k, signals)
        })
        .collect();

    let mut store = state.store.lock().expect("store lock");
    let mut results = Vec::new();
    // Re-auditing a key already stored (say, again with the deep budget) replaces its
    // result rather than adding a copy that batch GCD would then flag as a duplicate.
    // Identical keys within one submitted list stay separate: that is a real finding.
    let previously_stored = store.keys.len();
    for (k, signals) in audited {
        if let Some(existing) = store.keys[..previously_stored].iter_mut().find(|s| s.key.n == k.n && s.key.e == k.e) {
            existing.signals = signals;
            results.push(json!({ "id": existing.id, "report": operator_report(&existing.key, &existing.signals), "reaudited": true }));
            continue;
        }
        store.next_id += 1;
        let id = store.next_id;
        results.push(json!({ "id": id, "report": operator_report(&k, &signals) }));
        store.keys.push(StoredKey { id, key: k, signals });
    }
    Response::json(200, &json!({ "results": results, "errors": errors, "budget": budget.name() }))
}

fn operator_report(k: &RsaPublicKey, signals: &[Signal]) -> report::OperatorReport {
    let assessment = scoring::assess(signals);
    report::operator_report(k, &assessment, signals)
}

fn list_keys(state: &AppState) -> Response {
    let store = state.store.lock().expect("store lock");
    let rows: Vec<Value> = store
        .keys
        .iter()
        .map(|s| {
            let a = scoring::assess(&s.signals);
            json!({
                "id": s.id,
                "key": s.key.to_string(),
                "modulus_bits": s.key.modulus_bits(),
                "classification": a.classification.as_str(),
                "risk_score": a.risk_score,
                "failure_modes": a.failure_modes.iter().map(|m| format!("{m:?}")).collect::<Vec<_>>(),
            })
        })
        .collect();
    Response::json(200, &json!({ "keys": rows }))
}

fn clear_keys(state: &AppState) -> Response {
    let mut store = state.store.lock().expect("store lock");
    let cleared = store.keys.len();
    store.keys.clear();
    Response::json(200, &json!({ "cleared": cleared }))
}

fn report_for(state: &AppState, id: &str) -> Response {
    let Ok(id) = id.parse::<usize>() else {
        return Response::error(400, "report id must be a number");
    };
    let store = state.store.lock().expect("store lock");
    match store.keys.iter().find(|s| s.id == id) {
        Some(s) => Response::json(200, &operator_report(&s.key, &s.signals)),
        None => Response::error(404, format!("no stored key with id {id}")),
    }
}

/// Run tree batch GCD over every stored key and fold any shared-prime findings back
/// into those keys' signals, so their reports reflect the corpus-level result.
fn batch(state: &AppState) -> Response {
    let mut store = state.store.lock().expect("store lock");
    let n = store.keys.len();
    if n < 2 {
        return Response::error(400, "batch GCD needs at least two stored keys");
    }
    let keys: Vec<RsaPublicKey> = store.keys.iter().map(|s| s.key.clone()).collect();
    let result = batch_gcd::run_tree(&keys);

    for s in store.keys.iter_mut() {
        s.signals.retain(|sig| sig.detector != "batch_gcd");
    }
    let mut shared = Vec::new();
    for (idx, sig) in result.signals {
        let stored = &mut store.keys[idx];
        shared.push(json!({ "id": stored.id, "key": stored.key.to_string(), "detail": sig.detail }));
        stored.signals.push(sig);
    }
    Response::json(
        200,
        &json!({
            "keys": n,
            "pairs": n * (n - 1) / 2,
            "keys_affected": result.keys_affected,
            "shared": shared,
            "elapsed_ms": result.elapsed_ms,
        }),
    )
}

/// The research bundle: the labelled benchmark plus the ECM marginal-yield study.
/// Body (optional JSON): `{"seed": 1, "bits": 512, "quick": false}`.
fn bundle(body: &[u8]) -> Response {
    let opts: Value = if body.iter().all(u8::is_ascii_whitespace) {
        json!({})
    } else {
        match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => return Response::error(400, format!("bundle options must be JSON: {e}")),
        }
    };
    let seed = opts["seed"].as_u64().unwrap_or(1);
    let bits = opts["bits"].as_u64().unwrap_or(512);
    let quick = opts["quick"].as_bool().unwrap_or(false);
    if !(256..=1024).contains(&bits) {
        return Response::error(400, "bits must be between 256 and 1024");
    }

    let start = Instant::now();
    let spec = CorpusSpec { bits, ..Default::default() };
    let (metrics, _corpus) = run_benchmark(&spec, seed);

    let (factor_sizes, budgets, per_cell): (&[u64], &[(u64, u32)], usize) = if quick {
        (&[18, 22], &[(500, 10), (2_000, 10)], 3)
    } else {
        (&[18, 22, 26, 30], &[(500, 10), (2_000, 10), (5_000, 16), (10_000, 16)], 6)
    };
    let study = run_ecm_study(bits, seed, factor_sizes, budgets, per_cell);

    Response::json(
        200,
        &json!({
            "seed": seed,
            "bits": bits,
            "metrics": metrics,
            "ecm_study": { "modulus_bits": study.modulus_bits, "rows": study.rows },
            "elapsed_ms": start.elapsed().as_millis(),
        }),
    )
}

/* ----------------------------------------------------------------- samples */

fn hex_pair(k: &RsaPublicKey) -> String {
    format!("{}:{}", k.n.to_str_radix(16), k.e.to_str_radix(16))
}

/// One seeded synthetic key per defect class (public halves only), plus a legacy
/// 1024-bit key and a malformed PEM for the error path.
fn build_samples() -> Vec<Sample> {
    let corpus = generate(&CorpusSpec::default(), 7);
    let of = |class: DefectClass, count: usize| -> Vec<String> {
        corpus
            .truth
            .iter()
            .filter(|t| t.class == class)
            .take(count)
            .map(|t| hex_pair(&corpus.keys[t.index]))
            .collect()
    };
    let legacy = {
        let p = random_prime_seeded(512, 41);
        let q = random_prime_seeded(512, 42);
        RsaPublicKey::new(&p * &q, 65537u32.into(), "legacy-1024")
    };

    vec![
        Sample {
            id: "small-factor",
            label: "Small factor",
            expect: "FACTOR_RECOVERED",
            description: "One prime is tiny; trial division splits the modulus.",
            keys: of(DefectClass::SmallFactor, 1),
        },
        Sample {
            id: "close-primes",
            label: "Close primes",
            expect: "FACTOR_RECOVERED",
            description: "p and q drawn almost equal; Fermat factors it in a step.",
            keys: of(DefectClass::ClosePrime, 1),
        },
        Sample {
            id: "small-d",
            label: "Small private exponent",
            expect: "FACTOR_RECOVERED",
            description: "d chosen too small; Wiener's attack recovers it.",
            keys: of(DefectClass::SmallD, 1),
        },
        Sample {
            id: "unbalanced",
            label: "Unbalanced factor",
            expect: "FACTOR_RECOVERED",
            description: "An ~18-bit factor beyond trial division, found by bounded ECM.",
            keys: of(DefectClass::Unbalanced, 1),
        },
        Sample {
            id: "shared-prime",
            label: "Shared prime (2 keys)",
            expect: "FACTOR_RECOVERED after batch GCD",
            description: "Two keys reuse a prime; each looks clean alone until batch GCD runs.",
            keys: of(DefectClass::SharedPrime, 2),
        },
        Sample {
            id: "healthy",
            label: "Normal 512-bit",
            expect: "NO_DEFECT_FOUND",
            description: "Independent primes; only the undersized-modulus advisory fires.",
            keys: of(DefectClass::Healthy, 1),
        },
        Sample {
            id: "legacy-1024",
            label: "Legacy 1024-bit",
            expect: "NO_DEFECT_FOUND",
            description: "Sound primes but below the 2048-bit policy floor.",
            keys: vec![hex_pair(&legacy)],
        },
        Sample {
            id: "bad-pem",
            label: "Malformed PEM",
            expect: "400 parse error",
            description: "Not a key at all; the API rejects it with a clear error.",
            keys: vec!["-----BEGIN PUBLIC KEY-----\nTm90IGEga2V5IGF0IGFsbA==\n-----END PUBLIC KEY-----".to_string()],
        },
    ]
}
